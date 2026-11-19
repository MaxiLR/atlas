import { EllipsisIcon, FileDownIcon, Trash2Icon } from "lucide-react";
import { useId, useRef, useState } from "react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Progress } from "@/components/ui/progress";
import type { CourseSummary } from "@/core/generated/CourseSummary";

import { lastOpened } from "./last-opened";

/** Where the course picks up, as its row says it. */
function resumesAt(course: CourseSummary): string {
  if (course.stage === "Placement") return "In placement";
  return course.resumeTitle === null
    ? "Starts at its first section"
    : `Resumes at ${course.resumeTitle}`;
}

/**
 * One course in the library: its topic, which opens it, where it resumes,
 * its reading progress, when it was last opened and its menu. From 761 px the
 * four sit in aligned columns; narrower, progress and date go under the topic.
 */
export function CourseRow({
  course,
  now,
  onOpen,
  onExport,
  onDelete,
}: {
  course: CourseSummary;
  now: Date;
  onOpen(course: CourseSummary): void;
  onExport(course: CourseSummary): void;
  /** The deletion is confirmed. */
  onDelete(course: CourseSummary): void;
}) {
  const topicId = useId();
  const whereId = useId();
  const menuButton = useRef<HTMLButtonElement>(null);
  const [confirming, setConfirming] = useState(false);
  const { read, total } = course.progress;
  const openedAt = new Date(course.lastOpenedAt ?? course.createdAt);

  return (
    <li className="relative grid grid-cols-[minmax(0,1fr)_168px_104px_28px] items-center gap-x-6 border-b border-edge py-3.5 pr-2 pl-3 transition-colors duration-150 ease-out hover:bg-wash motion-reduce:transition-none max-[760px]:grid-cols-[minmax(0,1fr)_28px] max-[760px]:gap-x-3 max-[760px]:gap-y-2">
      <div className="min-w-0">
        <button
          type="button"
          id={topicId}
          data-course-open={course.id}
          title={course.topic}
          aria-describedby={whereId}
          onClick={() => onOpen(course)}
          className="block w-full cursor-pointer truncate text-left text-15 font-medium text-heading outline-none after:absolute after:inset-0 focus-visible:after:outline-2 focus-visible:after:-outline-offset-2 focus-visible:after:outline-focus focus-visible:after:outline-solid"
        >
          {course.topic}
        </button>
        <p id={whereId} className="truncate text-13 text-muted-foreground">
          {resumesAt(course)}
        </p>
      </div>
      <div className="contents max-[760px]:col-start-1 max-[760px]:row-start-2 max-[760px]:flex max-[760px]:items-center max-[760px]:gap-2.5">
        <div className="grid grid-cols-[56px_auto] items-center gap-2.5 text-13 whitespace-nowrap text-muted-foreground tabular-nums">
          {course.stage === "Ready" && total > 0 && (
            <>
              <Progress
                value={read}
                max={total}
                aria-label="Reading progress"
                getValueLabel={(value, max) => `${value} of ${max} read`}
              />
              <span>
                {read} of {total} read
              </span>
            </>
          )}
        </div>
        <time
          dateTime={openedAt.toISOString()}
          className="text-right text-13 whitespace-nowrap text-muted-foreground tabular-nums max-[760px]:ml-auto"
        >
          {lastOpened(openedAt, now)}
        </time>
      </div>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            ref={menuButton}
            variant="ghost"
            size="icon"
            aria-label="Course menu"
            aria-describedby={topicId}
            className="relative z-10 max-[760px]:col-start-2 max-[760px]:row-start-1"
          >
            <EllipsisIcon aria-hidden />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={() => onExport(course)}>
            <FileDownIcon aria-hidden />
            Export PDF
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => setConfirming(true)}>
            <Trash2Icon aria-hidden className="text-destructive" />
            Delete course
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <AlertDialog open={confirming} onOpenChange={setConfirming}>
        <AlertDialogContent
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            menuButton.current?.focus();
          }}
        >
          <AlertDialogHeader>
            <AlertDialogTitle className="break-words">
              Delete {course.topic}?
            </AlertDialogTitle>
            <AlertDialogDescription>
              Its index, every section and version, its glossary and its side
              chats are erased. Deletion is permanent and cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="default"
              onClick={() => onDelete(course)}
            >
              <Trash2Icon aria-hidden className="text-destructive" />
              Delete course
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </li>
  );
}
