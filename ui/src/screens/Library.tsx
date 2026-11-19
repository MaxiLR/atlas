import { useEffect, useState, type FormEvent } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { describeError } from "@/core/client";

/** Home: the topic field and the courses already started. */
export function Library() {
  const { send, navigate } = useAtlasStore();
  const { courses, error } = useAtlas((state) => state.library);
  const session = useAtlas((state) => state.session.session);
  const [topic, setTopic] = useState("");
  const [creating, setCreating] = useState(false);
  const planUsage = session?.type === "SignedIn" ? session.planUsage : null;
  const planRefusal =
    error?.type === "PlanUsageOff" || error?.type === "PlanNotAllowed";

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
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-medium">Atlas</h1>
        <Button
          variant="ghost"
          onClick={() => navigate({ type: "ShowSettings" })}
        >
          Settings
        </Button>
      </div>
      {planUsage === "Off" && (
        <div className="flex flex-col gap-2">
          <p role="status">
            Plan usage is off for this account, so no course can start.
          </p>
          <Button
            className="self-start"
            onClick={() =>
              send({ type: "EnablePlanUsage" }).catch(failureInStore)
            }
          >
            Enable plan usage
          </Button>
        </div>
      )}
      {planUsage === "NotAllowed" && (
        <p role="status">
          Plan usage is not allowed for this account or workspace, so Atlas
          cannot start courses with it.
        </p>
      )}
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
        {creating && <p role="status">Starting the course…</p>}
      </form>
      {error !== null && !(planRefusal && planUsage !== "On") && (
        <p role="alert">{describeError(error)}</p>
      )}
      {courses !== null && courses.length > 0 && (
        <section aria-labelledby="courses-heading">
          <h2 id="courses-heading" className="text-sm font-medium">
            Courses
          </h2>
          <ul>
            {courses.map((course) => (
              <li key={course.id} className="flex items-baseline gap-2">
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
                <span className="text-sm">
                  {course.stage === "Placement"
                    ? "In placement"
                    : `${course.progress.read} of ${course.progress.total} read`}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </main>
  );
}
