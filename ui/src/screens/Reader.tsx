import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import { describeError } from "@/core/client";
import type { CourseView } from "@/core/generated/CourseView";
import { selectSection } from "@/store";

/** A course: its index beside the open section, with Next at its foot. */
export function Reader({ course }: { course: CourseView }) {
  const { send, navigate } = useAtlasStore();
  const { sectionId, ended } = useAtlas((state) => ({
    sectionId: state.navigation.sectionId,
    ended: state.navigation.endOfCourse,
  }));
  const shown = useAtlas((state) =>
    sectionId === null ? null : selectSection(state, sectionId),
  );
  const entry = course.chapters
    .flatMap((chapter) => chapter.sections)
    .find((section) => section.id === sectionId);
  const text =
    shown?.blocks
      .map((block) =>
        block.state === "open"
          ? block.text
          : block.content.type === "Prose"
            ? block.content.text
            : "",
      )
      .join("") ?? "";
  const generating = shown === null || shown.status === "streaming";

  return (
    <div className="flex min-h-svh gap-8 p-8">
      <aside className="flex w-64 shrink-0 flex-col gap-4">
        <Button
          variant="ghost"
          className="self-start"
          onClick={() => navigate({ type: "ShowLibrary" })}
        >
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
                {chapter.sections.map((section) => (
                  <li key={section.id}>
                    <Button
                      variant="link"
                      aria-current={
                        section.id === sectionId ? "page" : undefined
                      }
                      onClick={() =>
                        send({
                          type: "OpenSection",
                          courseId: course.id,
                          sectionId: section.id,
                        }).catch(failureInStore)
                      }
                    >
                      {section.title}
                    </Button>
                  </li>
                ))}
              </ol>
            </section>
          ))}
        </nav>
      </aside>
      <main className="flex max-w-2xl flex-1 flex-col gap-4">
        {sectionId === null ? (
          <p>Choose a section in the index.</p>
        ) : (
          <article
            key={sectionId}
            aria-labelledby={`section-${sectionId}`}
            aria-busy={generating}
            className="flex flex-col gap-4"
          >
            <h2 id={`section-${sectionId}`} className="text-xl font-medium">
              {entry?.title}
            </h2>
            <div className="typeset whitespace-pre-wrap">{text}</div>
            {generating && text === "" && (
              <p role="status">Writing this section…</p>
            )}
            {shown?.status === "incomplete" && (
              <p role="status">This section is incomplete.</p>
            )}
            {shown?.error != null && (
              <p role="alert">{describeError(shown.error)}</p>
            )}
            <footer>
              <Button
                onClick={() =>
                  send({
                    type: "Next",
                    courseId: course.id,
                    sectionId,
                  }).catch(failureInStore)
                }
                disabled={generating}
              >
                Next
              </Button>
            </footer>
          </article>
        )}
        {ended && <p role="status">This is the end of the course.</p>}
      </main>
    </div>
  );
}
