import type { ReactNode } from "react";

/**
 * Continue: 36 px tall where it spans the column. Its fill and label switch
 * at once when a choice enables it, so the label never passes through an
 * unreadable mix of the disabled and enabled colours.
 */
export const continueStyle = "transition-[scale] max-[760px]:h-9";

/**
 * A question group that takes focus when it appears: the accent outline,
 * drawn clear of its options.
 */
export const groupFocusStyle =
  "rounded-md focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-focus focus-visible:outline-solid";

/**
 * The foot of a placement form: a muted note, or what went wrong, beside
 * Continue. Below 760 px Continue spans the column above the note.
 */
export function Actions({
  note,
  failure,
  children,
}: {
  note: ReactNode;
  /** Why the last request did not go through, when it did not. */
  failure: string | null;
  children: ReactNode;
}) {
  return (
    <div className="mt-8 flex items-center justify-between gap-6 max-[760px]:flex-col-reverse max-[760px]:items-stretch max-[760px]:gap-4">
      <div className="flex max-w-[46ch] flex-col gap-1 text-13 text-muted-foreground">
        {failure !== null && (
          <p role="alert" className="text-body">
            {failure}
          </p>
        )}
        <p>{note}</p>
      </div>
      {children}
    </div>
  );
}
