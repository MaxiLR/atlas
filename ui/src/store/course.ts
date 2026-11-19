import type { ChapterNode } from "@/core/generated/ChapterNode";
import type { CourseView } from "@/core/generated/CourseView";
import type { SectionNode } from "@/core/generated/SectionNode";

export interface CourseSlice {
  /** Each course opened: its index tree and its glossary and side-chat counts. */
  byId: Record<string, CourseView>;
}

export const initialCourse: CourseSlice = { byId: {} };

export function courseOpened(
  slice: CourseSlice,
  course: CourseView,
): CourseSlice {
  return { byId: { ...slice.byId, [course.id]: course } };
}

/** The course with `change` applied, if it has been opened. */
export function courseChanged(
  slice: CourseSlice,
  courseId: string,
  change: (course: CourseView) => CourseView,
): CourseSlice {
  const course = slice.byId[courseId];
  return course === undefined ? slice : courseOpened(slice, change(course));
}

/** The tree with `deepDives` under section `sectionId`, wherever it sits. */
export function withDeepDives(
  chapters: ChapterNode[],
  sectionId: string,
  deepDives: SectionNode[],
): ChapterNode[] {
  const place = (sections: SectionNode[]): SectionNode[] =>
    sections.map((section) =>
      section.id === sectionId
        ? { ...section, deepDives }
        : { ...section, deepDives: place(section.deepDives) },
    );
  return chapters.map((chapter) => ({
    ...chapter,
    sections: place(chapter.sections),
  }));
}
