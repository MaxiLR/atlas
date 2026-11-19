import { useId } from "react";

import type { CourseView } from "@/core/generated/CourseView";
import type { SectionNode } from "@/core/generated/SectionNode";
import { cn } from "@/lib/utils";

/**
 * Hidden while the rail is folded below 1024 px; the titles overlay shows it
 * again.
 */
const foldedAway = "max-lg:hidden";

function SectionRow({
  section,
  depth,
  currentId,
  folded,
  onOpen,
}: {
  section: SectionNode;
  depth: number;
  currentId: string | null;
  folded: boolean;
  onOpen: (sectionId: string) => void;
}) {
  const stateId = useId();
  const current = section.id === currentId;
  // Deep dives branch one step right of the line; deeper ones stay there, so
  // every mark fits the folded rail.
  const indent = depth === 0 ? 0 : 24;
  return (
    <li>
      <button
        type="button"
        aria-label={section.title}
        aria-describedby={stateId}
        aria-current={current ? "page" : undefined}
        onClick={() => onOpen(section.id)}
        className="group/row relative flex h-7.5 w-full cursor-pointer items-center rounded-sm text-left text-13 outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
      >
        <span
          aria-hidden
          style={{ left: 24 + indent }}
          className={cn(
            "absolute top-1/2 -translate-x-1/2 -translate-y-1/2 rounded-full transition-[scale] group-hover/row:scale-125",
            current
              ? "flex size-[11px] items-center justify-center border-[1.5px] border-atlas-accent bg-panel before:size-[5px] before:rounded-full before:bg-atlas-accent"
              : section.read
                ? "size-1.5 bg-body"
                : "size-1.5 border border-faint bg-panel",
          )}
        />
        <span
          style={{ marginLeft: 36 + indent }}
          className={cn(
            "flex h-full min-w-0 flex-1 items-center rounded-sm px-2 transition-[background-color,color] group-hover/row:bg-wash group-hover/row:text-heading",
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
      </span>
      {section.deepDives.length > 0 && (
        <ol>
          {section.deepDives.map((deepDive) => (
            <SectionRow
              key={deepDive.id}
              section={deepDive}
              depth={depth + 1}
              currentId={currentId}
              folded={folded}
              onOpen={onOpen}
            />
          ))}
        </ol>
      )}
    </li>
  );
}

/**
 * The index: each chapter with its sections and their deep dives, the read
 * state of each section as text for assistive technology and as a mark,
 * hollow when unread and filled when read. Folded, only the marks show.
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
  return (
    <nav
      aria-label="Index"
      className={cn(
        "min-h-0 flex-1 overflow-x-hidden overflow-y-auto py-3.5 pr-3",
        folded && "max-lg:pr-0",
      )}
    >
      <ol>
        {course.chapters.map((chapter) => {
          const travelled = chapter.sections.some((section) => section.read);
          return (
            <li key={chapter.id}>
              <div className="relative flex h-7.5 items-center">
                <span
                  aria-hidden
                  className={cn(
                    "absolute top-1/2 left-6 size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full border bg-panel",
                    travelled ? "border-body" : "border-faint",
                  )}
                />
                <h3
                  className={cn(
                    "ml-9 min-w-0 truncate px-2 text-13 font-medium text-heading",
                    folded && foldedAway,
                  )}
                >
                  {chapter.title}
                </h3>
              </div>
              <ol>
                {chapter.sections.map((section) => (
                  <SectionRow
                    key={section.id}
                    section={section}
                    depth={0}
                    currentId={currentId}
                    folded={folded}
                    onOpen={onOpen}
                  />
                ))}
              </ol>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
