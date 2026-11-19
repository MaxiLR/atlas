import type { CourseView } from "@/core/generated/CourseView";

export interface CourseSlice {
  /** Each course opened: its index tree and its glossary and side-chat counts. */
  byId: Record<string, CourseView>;
}

export const initialCourse: CourseSlice = { byId: {} };
