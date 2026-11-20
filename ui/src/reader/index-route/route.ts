import type { ChapterNode } from "@/core/generated/ChapterNode";
import type { CourseView } from "@/core/generated/CourseView";
import type { SectionNode } from "@/core/generated/SectionNode";

/** Every row of the route is this tall, in px. */
export const ROW_HEIGHT = 30;

/**
 * Where a node sits across the rail, by depth: the broad pass on the line,
 * a deep dive 20 px in, and each further level half the previous step again,
 * so any depth branches off its parent and still fits the 72 px folded rail.
 */
export function nodeX(depth: number): number {
  return 24 + 40 * (1 - 0.5 ** depth);
}

/** The middle of row `row`, down the figure. */
export function rowY(row: number): number {
  return row * ROW_HEIGHT + ROW_HEIGHT / 2;
}

export type RouteRow =
  | {
      type: "chapter";
      chapter: ChapterNode;
      /** The section selecting the chapter opens. */
      firstSectionId: string | null;
      /** Further along than the candidate: a station with its count. */
      collapsed: boolean;
    }
  | { type: "section"; section: SectionNode; depth: number };

/** A deep dive's connector: from its section's node down to its last child. */
export interface Branch {
  sectionId: string;
  depth: number;
  fromRow: number;
  firstRow: number;
  lastRow: number;
}

export interface Route {
  rows: RouteRow[];
  /** The last row on the line itself, a chapter or a broad-pass section. */
  lineEnd: number;
  /** The furthest broad-pass section read, where the bright part ends. */
  travelled: { sectionId: string; row: number } | null;
  branches: Branch[];
}

function holds(
  sections: SectionNode[],
  test: (section: SectionNode) => boolean,
): boolean {
  return sections.some(
    (section) => test(section) || holds(section.deepDives, test),
  );
}

/**
 * The index laid out as a route: each chapter a station, its sections and
 * their deep dives below it in index order. Chapters up to the candidate's
 * position (the open section or the furthest read, whichever lies further)
 * and the one after it show their sections; later ones show as stations.
 */
export function layOut(course: CourseView, currentId: string | null): Route {
  const position = course.chapters.reduce(
    (furthest, chapter, at) =>
      holds(
        chapter.sections,
        (section) => section.read || section.id === currentId,
      )
        ? at
        : furthest,
    0,
  );
  const rows: RouteRow[] = [];
  const branches: Branch[] = [];
  let lineEnd = 0;
  let travelled: Route["travelled"] = null;

  const place = (sections: SectionNode[], depth: number) => {
    for (const section of sections) {
      const row = rows.length;
      rows.push({ type: "section", section, depth });
      if (depth === 0) {
        lineEnd = row;
        if (section.read) travelled = { sectionId: section.id, row };
      }
      if (section.deepDives.length === 0) continue;
      const branch: Branch = {
        sectionId: section.id,
        depth,
        fromRow: row,
        firstRow: row + 1,
        lastRow: row + 1,
      };
      branches.push(branch);
      for (const deepDive of section.deepDives) {
        branch.lastRow = rows.length;
        place([deepDive], depth + 1);
      }
    }
  };

  course.chapters.forEach((chapter, at) => {
    const collapsed = at > position + 1;
    lineEnd = rows.length;
    rows.push({
      type: "chapter",
      chapter,
      firstSectionId: chapter.sections[0]?.id ?? null,
      collapsed,
    });
    if (!collapsed) place(chapter.sections, 0);
  });

  return { rows, lineEnd, travelled, branches };
}

/** The path of a branch: a curve off its section's node, then down. */
export function branchPath(branch: Branch): string {
  const fromX = nodeX(branch.depth);
  const toX = nodeX(branch.depth + 1);
  const fromY = rowY(branch.fromRow);
  const firstY = rowY(branch.firstRow);
  const lastY = rowY(branch.lastRow);
  return `M${fromX} ${fromY}C${fromX} ${fromY + 18} ${toX} ${firstY - 18} ${toX} ${firstY}V${lastY}`;
}
