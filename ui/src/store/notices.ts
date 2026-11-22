import type { CoreRequest } from "@/core/generated/CoreRequest";

/** Whether a notice reports a failure or a success. */
export type NoticeKind = "error" | "success";

/** What a notice offers to do: a request sent to the core. */
export interface NoticeAction {
  label: string;
  request: CoreRequest;
}

/**
 * An outcome the candidate cannot see where they are looking. Its id names
 * its cause, so a later notice of the same cause replaces it.
 */
export interface Notice {
  id: string;
  kind: NoticeKind;
  title: string;
  description: string;
  action: NoticeAction | null;
}

/** The notices showing, oldest first. */
export type NoticesSlice = Notice[];

export const initialNotices: NoticesSlice = [];

/** Shows `notice`; one of the same cause is replaced where it stands. */
export function noticeShown(
  notices: NoticesSlice,
  notice: Notice,
): NoticesSlice {
  void notice;
  return notices;
}

/** Closes the notice of cause `id`, when one is showing. */
export function noticeClosed(notices: NoticesSlice, id: string): NoticesSlice {
  void id;
  return notices;
}
