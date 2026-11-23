import { SettingsIcon } from "lucide-react";
import { useCallback, useEffect, useId, useRef, useState } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { WindowFrame, WindowTitle } from "@/app/window";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { describeError } from "@/core/client";
import type { CourseSummary } from "@/core/generated/CourseSummary";

import { CourseRow } from "./course-row";
import { NewCourse } from "./new-course";

/**
 * Home: the question that starts a course, and the courses already started,
 * most recently opened first. With no courses, only the question shows.
 */
export function Library() {
  const { send, navigate, screenChanged } = useAtlasStore();
  const courses = useAtlas((state) => state.library.courses);
  const session = useAtlas((state) => state.session.session);
  const planUsage = session?.type === "SignedIn" ? session.planUsage : null;
  const [listError, setListError] = useState<unknown>(null);
  const [rowError, setRowError] = useState<string | null>(null);
  const field = useRef<HTMLInputElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const headingId = useId();
  const now = new Date();

  const listCourses = useCallback(() => {
    send({ type: "ListCourses" }).catch(setListError);
  }, [send]);

  useEffect(listCourses, [listCourses]);

  // Reached from another screen, the library puts focus in the topic field,
  // or on its question while no course can start.
  useEffect(() => {
    if (!screenChanged()) return;
    const input = field.current;
    if (input !== null && !input.disabled) input.focus();
    else heading.current?.focus();
  }, [screenChanged]);

  function tryAgain() {
    setListError(null);
    listCourses();
  }

  async function open(course: CourseSummary) {
    setRowError(null);
    try {
      const { course: opened } = await send({
        type: "OpenCourse",
        courseId: course.id,
      });
      // Placement resumes on its own; a course with an index resumes at
      // the section last read.
      if (opened.stage === "Ready" && opened.resumeAt !== null) {
        send({
          type: "OpenSection",
          courseId: opened.id,
          sectionId: opened.resumeAt,
        }).catch(failureInStore);
      }
    } catch (error) {
      setRowError(
        `${course.topic} could not be opened. ${describeError(error)}`,
      );
    }
  }

  async function exportPdf(course: CourseSummary) {
    setRowError(null);
    // A refused export and one that failed both say so in the row line.
    let reason: string;
    try {
      const { result } = await send({
        type: "ExportPdf",
        courseId: course.id,
      });
      if (result.type !== "Failed") return;
      reason = result.reason;
    } catch (error) {
      reason = describeError(error);
    }
    setRowError(`${course.topic} could not be exported. ${reason}`);
  }

  async function remove(course: CourseSummary) {
    setRowError(null);
    // Focus moves to the row that takes its place, else the topic field.
    const rows = [
      ...(list.current?.querySelectorAll<HTMLElement>("[data-course-open]") ??
        []),
    ];
    const at = rows.findIndex((row) => row.dataset.courseOpen === course.id);
    const next = rows[at + 1] ?? rows[at - 1] ?? field.current;
    try {
      await send({ type: "DeleteCourse", courseId: course.id });
      next?.focus();
    } catch (error) {
      setRowError(
        `${course.topic} could not be deleted. ${describeError(error)}`,
      );
    }
  }

  let shelf = null;
  if (courses === null && listError !== null) {
    shelf = (
      <div
        role="alert"
        className="flex flex-wrap items-center gap-x-3 gap-y-2 border-t border-edge px-3 pt-4 text-13 text-body"
      >
        <span>Courses could not be loaded. {describeError(listError)}</span>
        <Button onClick={tryAgain}>Try again</Button>
      </div>
    );
  } else if (courses === null) {
    shelf = (
      <div role="status" className="border-t border-edge">
        <span className="sr-only">Loading courses…</span>
        {[0, 1, 2].map((row) => (
          <div
            key={row}
            className="flex flex-col gap-2 border-b border-edge py-4 pr-2 pl-3"
          >
            <Skeleton className="h-4 w-2/5" />
            <Skeleton className="h-3 w-1/4" />
          </div>
        ))}
      </div>
    );
  } else if (courses.length > 0) {
    shelf = (
      <section aria-labelledby={headingId}>
        <div className="flex items-baseline gap-2 border-b border-edge px-3 pb-2.5">
          <h2 id={headingId} className="text-13 font-medium text-heading">
            Courses
          </h2>
          <span className="text-13 text-muted-foreground tabular-nums">
            {courses.length}
          </span>
        </div>
        {rowError !== null && (
          <p
            role="alert"
            className="border-b border-edge px-3 py-2.5 text-13 text-body"
          >
            {rowError}
          </p>
        )}
        <ul ref={list}>
          {courses.map((course) => (
            <CourseRow
              key={course.id}
              course={course}
              now={now}
              onOpen={open}
              onExport={exportPdf}
              onDelete={remove}
            />
          ))}
        </ul>
      </section>
    );
  }

  return (
    <WindowFrame
      width="list"
      bar={
        <>
          <WindowTitle>Atlas</WindowTitle>
          <span data-tauri-drag-region className="flex-1 self-stretch" />
          <Button
            variant="ghost"
            onClick={() => navigate({ type: "ShowSettings" })}
          >
            <SettingsIcon aria-hidden />
            Settings
          </Button>
        </>
      }
    >
      <NewCourse planUsage={planUsage} field={field} heading={heading} />
      {shelf !== null && <div className="mt-8 max-[760px]:mt-0">{shelf}</div>}
    </WindowFrame>
  );
}
