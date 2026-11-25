import {
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
  type RefObject,
} from "react";

import type { CourseView } from "@/core/generated/CourseView";
import { cn } from "@/lib/utils";

import {
  branchPath,
  layOut,
  nodeX,
  ROW_HEIGHT,
  rowY,
  type Route,
  type RouteRow,
} from "./route";

/**
 * Hidden while the rail is folded below 1024 px; the titles overlay shows it
 * again.
 */
const foldedAway = "max-lg:hidden";

/** The figure's width: the folded rail. */
const FIGURE_WIDTH = 72;

const DRAW_MS = 250;
const ADVANCE_MS = 200;

function plural(count: number, noun: string) {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

function keyOf(row: RouteRow) {
  return row.type === "chapter" ? row.chapter.id : row.section.id;
}

function reducedMotion() {
  return (
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? true
  );
}

/** A row's node, drawn in the row so it moves and scales with it. */
function Node({
  x,
  children,
  ...data
}: {
  x: number;
  children: ReactNode;
  "data-mark": string;
  "data-branch"?: string;
  "data-node"?: string;
}) {
  return (
    <svg
      aria-hidden
      viewBox={`0 0 ${FIGURE_WIDTH} ${ROW_HEIGHT}`}
      className="pointer-events-none absolute top-0 left-0 h-7.5 w-18 overflow-visible"
      {...data}
    >
      <g
        style={{ transformOrigin: `${x}px ${ROW_HEIGHT / 2}px` }}
        className="transition-[scale] group-hover/row:scale-125 motion-reduce:transition-none"
      >
        {children}
      </g>
    </svg>
  );
}

function RowButton({
  row,
  route,
  at,
  currentId,
  tabbable,
  folded,
  onOpen,
  onFocus,
}: {
  row: RouteRow;
  route: Route;
  at: number;
  currentId: string | null;
  tabbable: boolean;
  folded: boolean;
  onOpen: (sectionId: string) => void;
  onFocus: () => void;
}) {
  const stateId = useId();
  const travelled = route.travelled !== null && at <= route.travelled.row;
  const shared = {
    type: "button" as const,
    "data-row": "",
    tabIndex: tabbable ? 0 : -1,
    onFocus,
    "aria-describedby": stateId,
    className:
      "group/row relative flex h-7.5 w-full cursor-pointer items-center rounded-sm text-left text-13 outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid disabled:cursor-default",
  };
  const label =
    "flex h-full min-w-0 flex-1 items-center rounded-sm px-2 transition-[background-color,color] group-hover/row:bg-wash group-hover/row:text-heading motion-reduce:transition-none";

  if (row.type === "chapter") {
    const { chapter, firstSectionId, collapsed } = row;
    const count = chapter.sections.length;
    return (
      <li>
        <button
          {...shared}
          aria-label={chapter.title}
          disabled={firstSectionId === null}
          onClick={() => firstSectionId !== null && onOpen(firstSectionId)}
        >
          <Node x={nodeX(0)} data-mark="station">
            <circle
              cx={nodeX(0)}
              cy={ROW_HEIGHT / 2}
              r={5}
              strokeWidth={1}
              className={cn(
                "fill-panel",
                travelled ? "stroke-body" : "stroke-faint",
              )}
            />
          </Node>
          <span
            style={{ marginLeft: nodeX(0) + 12 }}
            className={cn(
              label,
              "font-medium text-heading",
              folded && foldedAway,
            )}
          >
            <span className="truncate">{chapter.title}</span>
            {collapsed && (
              <span className="ml-auto pl-2 font-normal text-muted-foreground tabular-nums">
                {count}
              </span>
            )}
          </span>
        </button>
        <span id={stateId} className="sr-only">
          Chapter, {plural(count, "section")}
        </span>
      </li>
    );
  }

  const { section, depth } = row;
  const current = section.id === currentId;
  const deepDives = section.deepDives.length;
  const x = nodeX(depth);
  return (
    <li>
      <button
        {...shared}
        aria-label={section.title}
        aria-current={current ? "page" : undefined}
        onClick={() => onOpen(section.id)}
      >
        <Node
          x={x}
          data-mark={section.read ? "read" : "unread"}
          data-branch={deepDives > 0 ? "" : undefined}
          data-node={section.id}
        >
          {current ? (
            <>
              <circle
                cx={x}
                cy={ROW_HEIGHT / 2}
                r={5.5}
                strokeWidth={1.5}
                className="fill-panel stroke-atlas-accent"
              />
              <circle
                cx={x}
                cy={ROW_HEIGHT / 2}
                r={2.5}
                className="fill-atlas-accent stroke-none"
              />
            </>
          ) : section.read ? (
            <circle
              cx={x}
              cy={ROW_HEIGHT / 2}
              r={3}
              className="fill-body stroke-none"
            />
          ) : (
            // Its outer edge matches the read disc: only the fill differs.
            <circle
              cx={x}
              cy={ROW_HEIGHT / 2}
              r={2.5}
              strokeWidth={1}
              className="fill-panel stroke-faint"
            />
          )}
        </Node>
        <span
          style={{ marginLeft: x + 12 }}
          className={cn(
            label,
            current
              ? "bg-wash text-heading inset-ring inset-ring-edge"
              : section.read
                ? "text-body"
                : "text-muted-foreground",
            folded && foldedAway,
          )}
        >
          <span className="truncate">{section.title}</span>
        </span>
      </button>
      <span id={stateId} className="sr-only">
        {section.read ? "Read" : "Unread"}
        {deepDives > 0 && `, ${plural(deepDives, "deep dive")}`}
      </span>
    </li>
  );
}

/** What the figure drew last, to animate what changed since. */
interface Drawn {
  courseId: string;
  branched: Set<string>;
  travelledTo: number | null;
}

/**
 * Draws Go deeper's branch from its section's node along the connector, its
 * nodes following, and advances the bright part of the line when a further
 * section is read. Under reduced motion everything is already drawn.
 */
function useRouteMotion(
  courseId: string,
  route: Route,
  figure: RefObject<SVGSVGElement | null>,
  list: RefObject<HTMLOListElement | null>,
) {
  const drawn = useRef<Drawn | null>(null);
  useLayoutEffect(() => {
    const before = drawn.current;
    drawn.current = {
      courseId,
      branched: new Set(route.branches.map((branch) => branch.sectionId)),
      travelledTo: route.travelled?.row ?? null,
    };
    if (before === null || before.courseId !== courseId || reducedMotion()) {
      return;
    }
    const svg = figure.current;
    if (svg === null || typeof svg.animate !== "function") return;

    for (const branch of route.branches) {
      if (before.branched.has(branch.sectionId)) continue;
      svg
        .querySelector(`[data-line="branch"][data-from="${branch.sectionId}"]`)
        ?.animate([{ strokeDashoffset: 1 }, { strokeDashoffset: 0 }], {
          duration: DRAW_MS,
          easing: "ease-out",
        });
      const children = route.rows.slice(branch.firstRow, branch.lastRow + 1);
      children.forEach((row, at) => {
        if (row.type !== "section") return;
        list.current
          ?.querySelector(`[data-node="${row.section.id}"]`)
          ?.animate([{ opacity: 0 }, { opacity: 1 }], {
            duration: DRAW_MS - 100,
            delay: ((at + 1) / children.length) * (DRAW_MS - 100),
            easing: "ease-out",
            fill: "backwards",
          });
      });
    }

    const after = route.travelled?.row ?? null;
    if (after !== null && (before.travelledTo ?? -1) < after) {
      const length = rowY(after) - rowY(0);
      const from =
        before.travelledTo === null ? 0 : rowY(before.travelledTo) - rowY(0);
      svg
        .querySelector('[data-line="travelled"]')
        ?.animate(
          [
            { strokeDasharray: `${from} ${length}` },
            { strokeDasharray: `${length} ${length}` },
          ],
          { duration: ADVANCE_MS, easing: "ease-out" },
        );
    }
  }, [courseId, route, figure, list]);
}

/** Keeps the open section's row inside the rail's scrolled view. */
function useCurrentInView(
  nav: RefObject<HTMLElement | null>,
  currentId: string | null,
  folded: boolean,
) {
  useEffect(() => {
    const scroller = nav.current;
    const row = scroller?.querySelector('[aria-current="page"]');
    if (!scroller || !row) return;
    const view = scroller.getBoundingClientRect();
    const place = row.getBoundingClientRect();
    if (place.top < view.top) {
      scroller.scrollBy({ top: place.top - view.top - ROW_HEIGHT });
    } else if (place.bottom > view.bottom) {
      scroller.scrollBy({ top: place.bottom - view.bottom + ROW_HEIGHT });
    }
  }, [nav, currentId, folded]);
}

/** The keys that move focus along the rows, to the row they lead to. */
function target(key: string, at: number, count: number): number | null {
  switch (key) {
    case "ArrowDown":
      return Math.min(at + 1, count - 1);
    case "ArrowUp":
      return Math.max(at - 1, 0);
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}

function Empty({ building, folded }: { building: boolean; folded: boolean }) {
  const text = cn(
    "ml-9 px-2 text-13 text-muted-foreground",
    folded && "max-lg:sr-only",
  );
  return building ? (
    <div role="status" className="relative flex h-30 items-start">
      <svg
        aria-hidden
        viewBox="0 0 72 120"
        className="absolute top-0 left-0 h-30 w-18 fill-none"
      >
        <path
          d="M24 15V105"
          strokeWidth={1}
          strokeDasharray="2 4"
          className="stroke-faint"
        />
        {[15, 45, 75, 105].map((y) => (
          <circle
            key={y}
            cx={24}
            cy={y}
            r={y === 15 ? 5 : 3}
            strokeWidth={1}
            className="fill-panel stroke-faint"
          />
        ))}
      </svg>
      <p className={cn(text, "flex h-7.5 items-center")}>Building the index</p>
    </div>
  ) : (
    <p className={cn(text, "py-1.5")}>
      The index appears once placement is done.
    </p>
  );
}

/**
 * The index as a route line: one continuous line for the broad pass, a
 * station per chapter and a node per section, deep dives on branches that
 * leave the line under their section. State is a mark: hollow for unread,
 * filled for read, a branch for a section with deep dives; the line is
 * bright up to the furthest section read and the open section's node takes
 * the accent. Chapters further along are stations with their section count.
 *
 * The rows are one list in index order with one tab stop; the arrow keys,
 * Home and End move along it. Folded, only the line and its nodes show.
 */
export function IndexRoute({
  course,
  currentId,
  folded,
  onOpen,
}: {
  course: CourseView;
  currentId: string | null;
  folded: boolean;
  onOpen: (sectionId: string) => void;
}) {
  const route = useMemo(() => layOut(course, currentId), [course, currentId]);
  const nav = useRef<HTMLElement>(null);
  const figure = useRef<SVGSVGElement>(null);
  const list = useRef<HTMLOListElement>(null);
  // The row holding focus while focus is in the list; the open row otherwise.
  const [focusKey, setFocusKey] = useState<string | null>(null);
  useRouteMotion(course.id, route, figure, list);
  useCurrentInView(nav, currentId, folded);

  const keys = route.rows.map(keyOf);
  const tabStop =
    [focusKey, currentId].find((key) => key !== null && keys.includes(key)) ??
    keys[0];

  function move(event: KeyboardEvent<HTMLOListElement>) {
    const buttons = [
      ...event.currentTarget.querySelectorAll<HTMLButtonElement>(
        "button[data-row]",
      ),
    ];
    const at = buttons.findIndex((button) => button === document.activeElement);
    const to = at === -1 ? null : target(event.key, at, buttons.length);
    if (to === null) return;
    event.preventDefault();
    buttons[to]?.focus();
  }

  const travelled = route.travelled;
  return (
    <nav
      ref={nav}
      aria-label="Index"
      className={cn(
        "relative min-h-0 flex-1 overflow-x-hidden overflow-y-auto py-3.5 pr-3",
        folded && "max-lg:pr-0",
      )}
    >
      {route.rows.length === 0 ? (
        <Empty
          building={course.placement?.step.type === "BuildingIndex"}
          folded={folded}
        />
      ) : (
        <div className="relative">
          <svg
            ref={figure}
            aria-hidden
            width={FIGURE_WIDTH}
            height={route.rows.length * ROW_HEIGHT}
            className="pointer-events-none absolute top-0 left-0 fill-none"
          >
            {/* A 1 px band rather than a stroked path, so it has a width. */}
            <rect
              data-line="rest"
              x={nodeX(0) - 0.5}
              y={rowY(0)}
              width={1}
              height={rowY(route.lineEnd) - rowY(0)}
              className="fill-faint"
            />
            {route.branches.map((branch) => (
              <path
                key={branch.sectionId}
                data-line="branch"
                data-from={branch.sectionId}
                d={branchPath(branch)}
                pathLength={1}
                strokeDasharray="1"
                strokeWidth={1}
                className="stroke-muted-foreground"
              />
            ))}
            {travelled !== null && (
              <path
                data-line="travelled"
                data-to={travelled.sectionId}
                d={`M${nodeX(0)} ${rowY(0)}V${rowY(travelled.row)}`}
                strokeWidth={1}
                className="stroke-body"
              />
            )}
          </svg>
          <ol
            ref={list}
            className="relative"
            onKeyDown={move}
            onBlur={(event) => {
              if (!event.currentTarget.contains(event.relatedTarget)) {
                setFocusKey(null);
              }
            }}
          >
            {route.rows.map((row, at) => (
              <RowButton
                key={keyOf(row)}
                row={row}
                route={route}
                at={at}
                currentId={currentId}
                tabbable={keyOf(row) === tabStop}
                folded={folded}
                onOpen={onOpen}
                onFocus={() => setFocusKey(keyOf(row))}
              />
            ))}
          </ol>
        </div>
      )}
    </nav>
  );
}
