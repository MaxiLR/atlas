import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { describeError, expectResponse } from "@/core/client";
import { useCore } from "@/core/context";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CourseView } from "@/core/generated/CourseView";
import type { SectionEntry } from "@/core/generated/SectionEntry";

interface ShownSection {
  id: string;
  title: string;
  text: string;
  generating: boolean;
}

/** A course: its index beside the open section, with Next at its foot. */
export function Reader({
  course,
  onBack,
}: {
  course: CourseView;
  onBack: () => void;
}) {
  const core = useCore();
  const [section, setSection] = useState<ShownSection | null>(null);
  const [ended, setEnded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const entries = course.chapters.flatMap((chapter) => chapter.sections);

  useEffect(
    () =>
      core.subscribe((event) =>
        setSection((current) => {
          if (current === null || current.id !== event.sectionId) {
            return current;
          }
          switch (event.type) {
            case "SectionDelta":
              return { ...current, text: current.text + event.text };
            case "SectionCompleted":
              return { ...current, generating: false };
          }
        }),
      ),
    [core],
  );

  /** Shows `entry` at once (streaming into it), then the core's answer. */
  async function show(entry: SectionEntry | null, request: CoreRequest) {
    setEnded(false);
    setError(null);
    if (entry !== null) {
      setSection({
        id: entry.id,
        title: entry.title,
        text: "",
        generating: true,
      });
    }
    try {
      const response = await core.call(request);
      if (response.type === "EndOfCourse") {
        setEnded(true);
        return;
      }
      const { id, title, content } = expectResponse(
        response,
        "Section",
      ).section;
      setSection((current) =>
        current === null || current.id === id
          ? { id, title, text: content, generating: false }
          : current,
      );
    } catch (reason) {
      setError(describeError(reason));
      setSection((current) => current && { ...current, generating: false });
    }
  }

  function next() {
    if (section === null) return;
    const position = entries.findIndex((entry) => entry.id === section.id);
    void show(entries[position + 1] ?? null, {
      type: "Next",
      courseId: course.id,
      sectionId: section.id,
    });
  }

  return (
    <div className="flex min-h-svh gap-8 p-8">
      <aside className="flex w-64 shrink-0 flex-col gap-4">
        <Button variant="ghost" className="self-start" onClick={onBack}>
          Library
        </Button>
        <h1 className="text-lg font-medium">{course.topic}</h1>
        <nav aria-label="Index">
          {course.chapters.map((chapter) => (
            <section key={chapter.id} aria-labelledby={`chapter-${chapter.id}`}>
              <h2 id={`chapter-${chapter.id}`} className="text-sm font-medium">
                {chapter.title}
              </h2>
              <ol>
                {chapter.sections.map((entry) => (
                  <li key={entry.id}>
                    <Button
                      variant="link"
                      aria-current={
                        entry.id === section?.id ? "page" : undefined
                      }
                      onClick={() =>
                        void show(entry, {
                          type: "OpenSection",
                          courseId: course.id,
                          sectionId: entry.id,
                        })
                      }
                    >
                      {entry.title}
                    </Button>
                  </li>
                ))}
              </ol>
            </section>
          ))}
        </nav>
      </aside>
      <main className="flex max-w-2xl flex-1 flex-col gap-4">
        {section === null ? (
          <p>Choose a section in the index.</p>
        ) : (
          <article
            key={section.id}
            aria-labelledby={`section-${section.id}`}
            aria-busy={section.generating}
            className="flex flex-col gap-4"
          >
            <h2 id={`section-${section.id}`} className="text-xl font-medium">
              {section.title}
            </h2>
            <div className="typeset whitespace-pre-wrap">{section.text}</div>
            {section.generating && section.text === "" && (
              <p role="status">Writing this section…</p>
            )}
            <footer>
              <Button onClick={next} disabled={section.generating}>
                Next
              </Button>
            </footer>
          </article>
        )}
        {ended && <p role="status">This is the end of the course.</p>}
        {error !== null && <p role="alert">{error}</p>}
      </main>
    </div>
  );
}
