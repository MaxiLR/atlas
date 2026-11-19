import { useEffect, useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { describeError, expectResponse } from "@/core/client";
import { useCore } from "@/core/context";
import type { CourseSummary } from "@/core/generated/CourseSummary";
import type { CourseView } from "@/core/generated/CourseView";

/** Home: the topic field and the courses already started. */
export function Library({ onOpen }: { onOpen: (course: CourseView) => void }) {
  const core = useCore();
  const [courses, setCourses] = useState<CourseSummary[]>([]);
  const [topic, setTopic] = useState("");
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    core
      .call({ type: "ListCourses" })
      .then((response) => {
        if (live) setCourses(expectResponse(response, "Courses").courses);
      })
      .catch((reason: unknown) => {
        if (live) setError(describeError(reason));
      });
    return () => {
      live = false;
    };
  }, [core]);

  async function startCourse(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setCreating(true);
    setError(null);
    try {
      const response = await core.call({ type: "CreateCourse", topic });
      onOpen(expectResponse(response, "Course").course);
    } catch (reason) {
      setError(describeError(reason));
      setCreating(false);
    }
  }

  async function openCourse(courseId: string) {
    setError(null);
    try {
      const response = await core.call({ type: "OpenCourse", courseId });
      onOpen(expectResponse(response, "Course").course);
    } catch (reason) {
      setError(describeError(reason));
    }
  }

  return (
    <main className="mx-auto flex max-w-xl flex-col gap-8 p-8">
      <h1 className="text-xl font-medium">Atlas</h1>
      <form className="flex flex-col gap-2" onSubmit={startCourse}>
        <label htmlFor="topic" className="text-sm">
          Topic
        </label>
        <div className="flex gap-2">
          <Input
            id="topic"
            value={topic}
            onChange={(event) => setTopic(event.target.value)}
            required
            disabled={creating}
          />
          <Button type="submit" disabled={creating || topic.trim() === ""}>
            Start course
          </Button>
        </div>
        {creating && <p role="status">Building the index…</p>}
      </form>
      {error !== null && <p role="alert">{error}</p>}
      {courses.length > 0 && (
        <section aria-labelledby="courses-heading">
          <h2 id="courses-heading" className="text-sm font-medium">
            Courses
          </h2>
          <ul>
            {courses.map((course) => (
              <li key={course.id}>
                <Button variant="link" onClick={() => openCourse(course.id)}>
                  {course.topic}
                </Button>
              </li>
            ))}
          </ul>
        </section>
      )}
    </main>
  );
}
