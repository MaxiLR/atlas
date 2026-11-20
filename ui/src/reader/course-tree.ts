import type { ChapterNode } from "@/core/generated/ChapterNode";
import type { CourseView } from "@/core/generated/CourseView";
import type { SectionNode } from "@/core/generated/SectionNode";

/**
 * Where a section sits in the index: its chapter, then each section from the
 * broad pass down to it, so a deep dive follows the section it branches from.
 */
export interface SectionPlace {
  chapter: ChapterNode;
  trail: SectionNode[];
}

export function placeOf(
  course: CourseView,
  sectionId: string,
): SectionPlace | null {
  const search = (sections: SectionNode[]): SectionNode[] | null => {
    for (const section of sections) {
      if (section.id === sectionId) return [section];
      const deeper = search(section.deepDives);
      if (deeper !== null) return [section, ...deeper];
    }
    return null;
  };
  for (const chapter of course.chapters) {
    const trail = search(chapter.sections);
    if (trail !== null) return { chapter, trail };
  }
  return null;
}

/** How many sections the course has, deep dives included, and how many are read. */
export function readCount(course: CourseView): { read: number; total: number } {
  let read = 0;
  let total = 0;
  const count = (sections: SectionNode[]) => {
    for (const section of sections) {
      total += 1;
      if (section.read) read += 1;
      count(section.deepDives);
    }
  };
  for (const chapter of course.chapters) count(chapter.sections);
  return { read, total };
}
