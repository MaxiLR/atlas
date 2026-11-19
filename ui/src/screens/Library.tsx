import { useEffect, useState, type FormEvent } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { describeError } from "@/core/client";

/** Home: the topic field and the courses already started. */
export function Library() {
  const { send } = useAtlasStore();
  const { courses, error } = useAtlas((state) => state.library);
  const [topic, setTopic] = useState("");
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    send({ type: "ListCourses" }).catch(failureInStore);
  }, [send]);

  async function startCourse(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setCreating(true);
    try {
      await send({ type: "CreateCourse", topic });
    } catch {
      setCreating(false);
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
      {error !== null && <p role="alert">{describeError(error)}</p>}
      {courses !== null && courses.length > 0 && (
        <section aria-labelledby="courses-heading">
          <h2 id="courses-heading" className="text-sm font-medium">
            Courses
          </h2>
          <ul>
            {courses.map((course) => (
              <li key={course.id}>
                <Button
                  variant="ghost"
                  onClick={() =>
                    send({ type: "OpenCourse", courseId: course.id }).catch(
                      failureInStore,
                    )
                  }
                >
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
