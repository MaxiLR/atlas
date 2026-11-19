/**
 * A concept map laid out in rows, from its spec alone: no physics, nothing
 * measured in the page, so the same spec always yields the same drawing.
 *
 * Nodes are ranked so every link runs down the page: in the order a
 * breadth-first walk from the first node meets them, each node goes to the
 * first row below the nodes it links back to that still has room for it. A
 * link that skips rows passes through each row it skips at a waypoint of its
 * own, so it runs between nodes rather than through them. Rows are ordered to
 * keep links short and uncrossed, then each node is pulled towards the nodes
 * it links to without ever moving past a neighbour.
 */
import {
  BASELINE,
  MARGIN,
  WIDTH,
  linesHeight,
  linesWidth,
  round,
  wrap,
} from "../drawing.tsx";

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Point {
  x: number;
  y: number;
}

export interface MapNode {
  id: string;
  label: string;
}

export interface MapLink {
  from: string;
  to: string;
  label: string;
  directed?: boolean;
}

export interface PlacedNode {
  label: string;
  lines: string[];
  face: Rect;
  /** The first line's baseline. */
  baseline: number;
}

export interface PlacedLink {
  /** The index of the link in the spec. */
  index: number;
  /** The path from the `from` node to the `to` node. */
  path: string;
  directed: boolean;
  label: { lines: string[]; box: Rect; baseline: number };
}

export interface MapLayout {
  height: number;
  nodes: PlacedNode[];
  links: PlacedLink[];
}

/** Node labels: up to three lines of fourteen characters. */
const NODE_CHARS = 14;
const NODE_LINES = 3;
const NODE_PAD_X = 12;
const NODE_PAD_Y = 8;

/** Link labels: up to two lines of eighteen characters. */
const LINK_CHARS = 18;
const LINK_LINES = 2;
const LINK_PAD_X = 4;
const LINK_PAD_Y = 2;

/** Space between neighbours in a row, and the room a waypoint takes. */
const GAP = 32;
const WAYPOINT = 8;

/** The width a row's nodes, waypoints and gaps may fill. */
const INNER = WIDTH - 2 * MARGIN;

/**
 * Where along a curve a link's label may sit, preferred first, measured from
 * the end where links spread apart: below a node many links leave, near the
 * nodes they reach; above a node many links reach, near where they leave.
 */
const LABEL_AT = [0.62, 0.5, 0.75, 0.4, 0.85, 0.3, 0.2, 0.1];

/** One item of a row: a node, or a waypoint of a link that skips the row. */
interface Item {
  node: number | null;
  width: number;
  height: number;
  up: Item[];
  down: Item[];
  /** Left edge, once placed. */
  x: number;
}

interface Edge {
  link: number;
  /** The upper end first. */
  chain: Item[];
  /** True when the link's `from` node is the lower end. */
  reversed: boolean;
}

function center(item: Item): number {
  return item.x + item.width / 2;
}

/** The nodes in the order a breadth-first walk meets them. */
function walkOrder(count: number, neighbours: number[][]): number[] {
  const order: number[] = [];
  const seen = new Set<number>();
  for (let root = 0; root < count; root++) {
    if (seen.has(root)) continue;
    seen.add(root);
    for (let at = order.push(root) - 1; at < order.length; at++) {
      for (const next of neighbours[order[at]]) {
        if (seen.has(next)) continue;
        seen.add(next);
        order.push(next);
      }
    }
  }
  return order;
}

/** Sorts a row by `key`, keeping the current order on ties. */
function sortRow(row: Item[], key: (item: Item) => number | null) {
  const keyed = row.map((item, index) => ({
    item,
    key: key(item) ?? index,
    index,
  }));
  keyed.sort((a, b) => a.key - b.key || a.index - b.index);
  row.splice(0, row.length, ...keyed.map(({ item }) => item));
}

function meanIndex(items: Item[], rowOf: Map<Item, Item[]>): number | null {
  if (items.length === 0) return null;
  const sum = items.reduce(
    (total, item) => total + rowOf.get(item)!.indexOf(item),
    0,
  );
  return sum / items.length;
}

/** The spaces between a row's items: full between nodes, less if crowded. */
function gapsOf(row: Item[]): number[] {
  const gaps = row.slice(1).map(() => GAP);
  const used = row.reduce((total, item) => total + item.width, 0);
  const total = used + gaps.reduce((sum, gap) => sum + gap, 0);
  if (total <= INNER) return gaps;
  const soft = row
    .slice(1)
    .map((item, index) => item.node === null || row[index].node === null);
  const softCount = soft.filter(Boolean).length;
  const hard = gaps.length - softCount;
  if (softCount === 0) return gaps;
  const room = Math.max(0, (INNER - used - hard * GAP) / softCount);
  return soft.map((isSoft) => (isSoft ? Math.min(GAP, room) : GAP));
}

/**
 * Places a row's items as close to their wanted centres as their order and
 * spacing allow, inside the margins: least squares under order constraints,
 * by pooling adjacent violators.
 */
function placeRow(row: Item[], wanted: (item: Item) => number) {
  const gaps = gapsOf(row);
  const offsets: number[] = [];
  let offset = 0;
  row.forEach((item, index) => {
    offsets.push(offset);
    offset += item.width + (gaps[index] ?? 0);
  });
  const span = offsets[row.length - 1] + row[row.length - 1].width;
  const blocks: { sum: number; count: number }[] = [];
  row.forEach((item, index) => {
    const target = wanted(item) - item.width / 2 - offsets[index];
    blocks.push({ sum: target, count: 1 });
    while (blocks.length > 1) {
      const last = blocks[blocks.length - 1];
      const before = blocks[blocks.length - 2];
      if (before.sum / before.count <= last.sum / last.count) break;
      blocks.pop();
      before.sum += last.sum;
      before.count += last.count;
    }
  });
  let index = 0;
  for (const block of blocks) {
    const start = Math.min(
      Math.max(block.sum / block.count, MARGIN),
      WIDTH - MARGIN - span,
    );
    for (let n = 0; n < block.count; n++, index++) {
      row[index].x = start + offsets[index];
    }
  }
}

/** A cubic Bézier curve: its ends and its two control points. */
type Curve = [Point, Point, Point, Point];

function bezierAt([p0, p1, p2, p3]: Curve, t: number): Point {
  const u = 1 - t;
  const a = u * u * u;
  const b = 3 * u * u * t;
  const c = 3 * u * t * t;
  const d = t * t * t;
  return {
    x: a * p0.x + b * p1.x + c * p2.x + d * p3.x,
    y: a * p0.y + b * p1.y + c * p2.y + d * p3.y,
  };
}

/** How much of `a` lies within `clearance` of `b`, as an area. */
function overlapArea(a: Rect, b: Rect, clearance: number): number {
  const width =
    Math.min(a.x + a.width, b.x + b.width) + clearance - Math.max(a.x, b.x);
  const height =
    Math.min(a.y + a.height, b.y + b.height) + clearance - Math.max(a.y, b.y);
  return width > 0 && height > 0 ? width * height : 0;
}

type Segment =
  { kind: "curve"; points: Curve } | { kind: "line"; points: [Point, Point] };

function pathOf(segments: Segment[]): string {
  const at = (point: Point) => `${round(point.x)} ${round(point.y)}`;
  const start = segments[0].points[0];
  return [
    `M${at(start)}`,
    ...segments.map((segment) =>
      segment.kind === "curve"
        ? `C${segment.points.slice(1).map(at).join(" ")}`
        : `L${at(segment.points[1])}`,
    ),
  ].join(" ");
}

function reverse(segments: Segment[]): Segment[] {
  return [...segments].reverse().map((segment): Segment => {
    if (segment.kind === "line") {
      return { kind: "line", points: [segment.points[1], segment.points[0]] };
    }
    const [p0, p1, p2, p3] = segment.points;
    return { kind: "curve", points: [p3, p2, p1, p0] };
  });
}

export function layoutConceptMap(
  nodes: MapNode[],
  links: MapLink[],
): MapLayout {
  const placed: PlacedNode[] = nodes.map(({ label }) => {
    const lines = wrap(label, NODE_CHARS, NODE_LINES);
    return {
      label,
      lines,
      face: {
        x: 0,
        y: 0,
        width: linesWidth(lines) + 2 * NODE_PAD_X,
        height: linesHeight(lines.length) + 2 * NODE_PAD_Y,
      },
      baseline: 0,
    };
  });

  const byId = new Map<string, number>();
  nodes.forEach(({ id }, index) => {
    if (!byId.has(id)) byId.set(id, index);
  });
  const drawn = links.flatMap((link, index) => {
    const from = byId.get(link.from);
    const to = byId.get(link.to);
    return from === undefined || to === undefined || from === to
      ? []
      : [{ index, from, to }];
  });

  const neighbours = nodes.map((): number[] => []);
  for (const { from, to } of drawn) {
    neighbours[from].push(to);
    neighbours[to].push(from);
  }
  const order = walkOrder(nodes.length, neighbours);
  const position = new Map(order.map((node, at) => [node, at]));
  const oriented = drawn.map(({ index, from, to }) => {
    const reversed = position.get(from)! > position.get(to)!;
    return {
      index,
      upper: reversed ? to : from,
      lower: reversed ? from : to,
      reversed,
    };
  });

  // Rows, filled in walk order; a node goes to the first row below its
  // upper neighbours with room for it.
  const rows: Item[][] = [];
  const rank = new Map<number, number>();
  const items = new Map<number, Item>();
  const used = (row: number) =>
    (rows[row] ?? []).reduce(
      (total, item, index) => total + item.width + (index > 0 ? GAP : 0),
      0,
    );
  const edges: Edge[] = [];
  for (const node of order) {
    const into = oriented.filter((edge) => edge.lower === node);
    let row = Math.max(0, ...into.map((edge) => rank.get(edge.upper)! + 1));
    const width = placed[node].face.width;
    while ((rows[row]?.length ?? 0) > 0 && used(row) + GAP + width > INNER) {
      row++;
    }
    const item: Item = {
      node,
      width,
      height: placed[node].face.height,
      up: [],
      down: [],
      x: 0,
    };
    (rows[row] ??= []).push(item);
    rank.set(node, row);
    items.set(node, item);
    for (const edge of into) {
      const chain = [items.get(edge.upper)!];
      for (let skipped = rank.get(edge.upper)! + 1; skipped < row; skipped++) {
        const waypoint: Item = {
          node: null,
          width: WAYPOINT,
          height: 0,
          up: [],
          down: [],
          x: 0,
        };
        (rows[skipped] ??= []).push(waypoint);
        chain.push(waypoint);
      }
      chain.push(item);
      chain.slice(1).forEach((lower, at) => {
        chain[at].down.push(lower);
        lower.up.push(chain[at]);
      });
      edges.push({ link: edge.index, chain, reversed: edge.reversed });
    }
  }

  // Order each row by where its links come from, then where they go.
  const rowOf = new Map<Item, Item[]>();
  for (const row of rows) for (const item of row) rowOf.set(item, row);
  for (let sweep = 0; sweep < 4; sweep++) {
    const down = sweep % 2 === 0;
    const sequence = down ? rows.slice(1) : rows.slice(0, -1).reverse();
    for (const row of sequence) {
      sortRow(row, (item) => meanIndex(down ? item.up : item.down, rowOf));
    }
  }

  // Centre every row, then pull items towards what they link to: down,
  // up and down again.
  for (const row of rows) placeRow(row, () => WIDTH / 2);
  const pull = (row: Item[], towards: (item: Item) => Item[]) =>
    placeRow(row, (item) => {
      const others = towards(item);
      return others.length === 0
        ? center(item)
        : others.reduce((sum, other) => sum + center(other), 0) / others.length;
    });
  for (const row of rows.slice(1)) pull(row, (item) => item.up);
  for (const row of rows.slice(0, -1).reverse()) pull(row, (item) => item.down);
  for (const row of rows.slice(1)) pull(row, (item) => item.up);

  // Rows top to bottom; the space under a row grows with the links crossing
  // it, to give their labels room.
  const tops: number[] = [];
  const heights = rows.map((row) =>
    Math.max(
      linesHeight(1) + 2 * NODE_PAD_Y,
      ...row.map((item) => item.height),
    ),
  );
  let y = MARGIN;
  rows.forEach((row, index) => {
    tops.push(y);
    const crossing = row.reduce((sum, item) => sum + item.down.length, 0);
    y += heights[index] + Math.min(120, Math.max(56, 40 + 16 * crossing));
  });
  const height = tops[tops.length - 1] + heights[heights.length - 1] + MARGIN;

  for (const [node, item] of items) {
    const row = rank.get(node)!;
    const face = placed[node].face;
    face.x = round(item.x);
    face.y = round(tops[row] + (heights[row] - item.height) / 2);
    placed[node].baseline = face.y + NODE_PAD_Y + BASELINE;
  }

  // Links: a curve between rows, a straight run through each skipped row.
  const faces = placed.map((node) => node.face);
  const labels: Rect[] = [];
  const placedLinks = edges
    .sort((a, b) => a.link - b.link)
    .map((edge): PlacedLink => {
      const segments: Segment[] = [];
      const curves: Curve[] = [];
      const first = placed[edge.chain[0].node!].face;
      let from: Point = {
        x: first.x + first.width / 2,
        y: first.y + first.height,
      };
      edge.chain.slice(1).forEach((item) => {
        let to: Point;
        let through: Point | null = null;
        if (item.node === null) {
          const row = rowOf.get(item)!;
          const index = rows.indexOf(row);
          to = { x: center(item), y: tops[index] };
          through = { x: center(item), y: tops[index] + heights[index] };
        } else {
          const face = placed[item.node].face;
          to = { x: face.x + face.width / 2, y: face.y };
        }
        const middle = (from.y + to.y) / 2;
        const curve: Curve = [
          from,
          { x: from.x, y: middle },
          { x: to.x, y: middle },
          to,
        ];
        curves.push(curve);
        segments.push({ kind: "curve", points: curve });
        if (through) {
          segments.push({ kind: "line", points: [to, through] });
          from = through;
        } else {
          from = to;
        }
      });

      const link = links[edge.link];
      const lines = wrap(link.label, LINK_CHARS, LINK_LINES);
      const width = linesWidth(lines) + 2 * LINK_PAD_X;
      const boxHeight = linesHeight(lines.length) + 2 * LINK_PAD_Y;
      // On the curve, then beside it on the side it heads to, then the other.
      const boxesAt = (curve: Curve, t: number): Rect[] => {
        const point = bezierAt(curve, t);
        const heading = Math.sign(curve[3].x - curve[0].x) || 1;
        const centred = point.x - width / 2;
        const ahead = heading < 0 ? point.x - 6 - width : point.x + 6;
        const behind = heading < 0 ? point.x + 6 : point.x - 6 - width;
        return [centred, ahead, behind].map((x) => ({
          x: round(Math.min(Math.max(x, MARGIN), WIDTH - MARGIN - width)),
          y: round(point.y - boxHeight / 2),
          width,
          height: boxHeight,
        }));
      };
      const fansOut =
        edge.chain[0].down.length > edge.chain[edge.chain.length - 1].up.length;
      const candidates = (fansOut ? [...curves].reverse() : curves).flatMap(
        (curve) => LABEL_AT.flatMap((t) => boxesAt(curve, fansOut ? t : 1 - t)),
      );
      // The first clear place, or the one that covers least.
      const cost = (box: Rect) =>
        faces.reduce((sum, face) => sum + 4 * overlapArea(box, face, 4), 0) +
        labels.reduce((sum, label) => sum + overlapArea(box, label, 2), 0);
      let box = candidates[0];
      let least = cost(box);
      for (const candidate of candidates.slice(1)) {
        if (least === 0) break;
        const covered = cost(candidate);
        if (covered < least) [box, least] = [candidate, covered];
      }
      labels.push(box);

      return {
        index: edge.link,
        path: pathOf(edge.reversed ? reverse(segments) : segments),
        directed: link.directed === true,
        label: { lines, box, baseline: box.y + LINK_PAD_Y + BASELINE },
      };
    });

  return { height, nodes: placed, links: placedLinks };
}
