import { useId, useState, type FormEvent, type Ref } from "react";

import { useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@/components/ui/input-group";
import { describeError } from "@/core/client";
import type { PlanUsage } from "@/core/generated/PlanUsage";

/**
 * The question that opens a course: the topic field with Start course, and
 * one line under it: the hint, or why no course can start while plan usage
 * is off or not allowed, or what went wrong.
 */
export function NewCourse({
  planUsage,
  field,
  heading,
}: {
  /** Null until the session is known. */
  planUsage: PlanUsage | null;
  field: Ref<HTMLInputElement>;
  /** The question, which takes focus while the field cannot. */
  heading: Ref<HTMLHeadingElement>;
}) {
  const { send } = useAtlasStore();
  const [topic, setTopic] = useState("");
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const lineId = useId();
  const blocked = planUsage === "Off" || planUsage === "NotAllowed";
  const trimmed = topic.trim();

  async function start(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (trimmed === "" || blocked || starting) return;
    setStarting(true);
    setError(null);
    try {
      // Placement follows from the store once the core answers.
      await send({ type: "CreateCourse", topic: trimmed });
    } catch (refused) {
      setError(refused);
      setStarting(false);
    }
  }

  function enablePlanUsage() {
    setError(null);
    send({ type: "EnablePlanUsage" }).catch(setError);
  }

  let line;
  if (error !== null) {
    line = (
      <p id={lineId} role="alert" className="text-body">
        The course could not start. {describeError(error)}
      </p>
    );
  } else if (planUsage === "Off") {
    line = (
      <p id={lineId} className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <span>Plan usage is off for this account, so no course can start.</span>
        <Button onClick={enablePlanUsage}>Enable plan usage</Button>
      </p>
    );
  } else if (planUsage === "NotAllowed") {
    line = (
      <p id={lineId}>
        Plan usage is not allowed for this account or workspace, so no course
        can start.
      </p>
    );
  } else if (starting) {
    line = (
      <p id={lineId} role="status">
        Starting the course…
      </p>
    );
  } else {
    line = (
      <p id={lineId}>
        Atlas asks a few questions first, then lays out the whole topic. Courses
        are written in English.
      </p>
    );
  }

  return (
    <form onSubmit={start}>
      <h1
        ref={heading}
        tabIndex={-1}
        className="rounded-xs text-20 font-medium tracking-[-0.012em] text-balance text-heading outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
      >
        What do you want to learn?
      </h1>
      <InputGroup className="mt-4 h-12 rounded-[10px] pr-1">
        <InputGroupInput
          ref={field}
          aria-label="Topic"
          aria-describedby={lineId}
          placeholder="A topic, in your own words"
          autoComplete="off"
          spellCheck={false}
          value={topic}
          onChange={(event) => setTopic(event.target.value)}
          disabled={blocked || starting}
          className="h-full pl-3.5"
        />
        <InputGroupAddon align="inline-end">
          <InputGroupButton
            type="submit"
            variant="primary"
            size="lg"
            disabled={trimmed === "" || blocked || starting}
          >
            Start course
          </InputGroupButton>
        </InputGroupAddon>
      </InputGroup>
      <div className="mt-2.5 text-13 text-muted-foreground">{line}</div>
    </form>
  );
}
