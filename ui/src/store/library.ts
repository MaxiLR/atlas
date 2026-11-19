import type { CoreError } from "@/core/generated/CoreError";
import type { CourseSummary } from "@/core/generated/CourseSummary";

export interface LibrarySlice {
  /** Most recently opened first; null until the core has listed them. */
  courses: CourseSummary[] | null;
  /** Why the last library request failed. */
  error: CoreError | null;
}

export const initialLibrary: LibrarySlice = { courses: null, error: null };

export function coursesListed(courses: CourseSummary[]): LibrarySlice {
  return { courses, error: null };
}

export function courseDeleted(
  slice: LibrarySlice,
  courseId: string,
): LibrarySlice {
  return {
    courses: slice.courses?.filter((course) => course.id !== courseId) ?? null,
    error: null,
  };
}
