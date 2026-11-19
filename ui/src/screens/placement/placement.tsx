import { ChevronLeftIcon } from "lucide-react";
import { useState } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { WindowFrame, WindowTitle } from "@/app/window";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { describeError } from "@/core/client";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CourseView } from "@/core/generated/CourseView";
import type { PlacementState } from "@/core/generated/PlacementState";
import type { PlacementStep } from "@/core/generated/PlacementStep";
import { cn } from "@/lib/utils";

import { QuestionForm } from "./question";
import { RatingForm } from "./rating";

type PlacementRequest = Extract<
  CoreRequest,
  { type: "SubmitSelfRating" | "AnswerPlacement" | "RetryPlacementStep" }
>;

type Mark = "answered" | "current" | "to come" | "may not be asked";

const markStyle: Record<Mark, string> = {
  answered: "bg-muted-foreground",
  current: "bg-atlas-accent",
  "to come": "bg-edge-strong",
  "may not be asked": "inset-ring inset-ring-edge-strong",
};

/**
 * One mark per question placement may ask: those answered, the one shown,
 * those still to come up to the least it asks, and the ones past it drawn
 * as possible.
 */
function StepMarks({
  number,
  state,
}: {
  number: number;
  state: PlacementState;
}) {
  const steps = Array.from({ length: state.maxQuestions }, (_, at) => at + 1);
  return (
    <ol aria-label="Questions" className="flex shrink-0 gap-1">
      {steps.map((step) => {
        const mark: Mark =
          step < number
            ? "answered"
            : step === number
              ? "current"
              : step <= state.minQuestions
                ? "to come"
                : "may not be asked";
        return (
          <li
            key={step}
            data-tauri-drag-region
            className={cn("h-1 w-5 rounded-[2px]", markStyle[mark])}
          >
            <span className="sr-only">
              Question {step}, {mark}
            </span>
          </li>
        );
      })}
    </ol>
  );
}

/** A step that takes a while: what is happening and what comes of it. */
function Waiting({ title, line }: { title: string; line: string }) {
  return (
    <div role="status" className="flex flex-col gap-2">
      <h2 className="flex items-center gap-3 text-20 font-medium text-heading">
        <Spinner aria-hidden className="size-4" />
        {title}
      </h2>
      <p className="max-w-[52ch] text-15 text-muted-foreground">{line}</p>
    </div>
  );
}

/** What a failed step was doing. */
function failedStep(step: PlacementStep): string {
  switch (step.type) {
    case "PreparingQuestions":
      return "The questions could not be prepared.";
    case "BuildingIndex":
      return "The index could not be built.";
    default:
      return "Placement could not go on.";
  }
}

/** A failed step: what failed and why, with Retry. */
function FailedStep({
  step,
  error,
  busy,
  failure,
  onRetry,
}: {
  step: PlacementStep;
  error: CoreError;
  busy: boolean;
  failure: string | null;
  onRetry: () => void;
}) {
  return (
    <div className="flex flex-col items-start gap-6">
      <p role="alert" className="flex flex-col gap-2">
        <span className="text-20 font-medium text-heading">
          {failedStep(step)}
        </span>{" "}
        <span className="text-15 text-body">{describeError(error)}</span>
      </p>
      {failure !== null && <p className="text-13 text-body">{failure}</p>}
      <Button
        variant="primary"
        size="lg"
        className="max-[760px]:h-9 max-[760px]:self-stretch"
        disabled={busy}
        onClick={onRetry}
      >
        Retry
      </Button>
    </div>
  );
}

/**
 * Placement: the self-rating and the goal, the questions being prepared, one
 * question at a time, the index being built, and a failed step with Retry.
 * When the index is built, the reader opens on the first section.
 */
export function Placement({ course }: { course: CourseView }) {
  const store = useAtlasStore();
  const { send, navigate } = store;
  const placement = useAtlas(
    (state) => state.placement.byCourse[course.id] ?? course.placement,
  );
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  /** Opens the first section, if the window still shows this course. */
  function openFirstSection() {
    const { navigation, course: courses } = store.state.getState();
    const first = courses.byId[course.id]?.chapters[0]?.sections[0];
    if (
      first === undefined ||
      navigation.screen !== "reader" ||
      navigation.courseId !== course.id ||
      navigation.sectionId !== null
    )
      return;
    send({
      type: "OpenSection",
      courseId: course.id,
      sectionId: first.id,
    }).catch(failureInStore);
  }

  async function request(request: PlacementRequest, refused: string) {
    setBusy(true);
    setFailure(null);
    try {
      const answer = await send(request);
      if (answer.placement.step.type === "Done") openFirstSection();
    } catch (error) {
      setFailure(`${refused} ${describeError(error)}`);
    } finally {
      setBusy(false);
    }
  }

  const step = placement?.step;
  const questions =
    placement === null
      ? ""
      : `${placement.minQuestions} to ${placement.maxQuestions}`;

  let body = null;
  if (placement !== null && placement.error !== null) {
    body = (
      <FailedStep
        step={placement.step}
        error={placement.error}
        busy={busy}
        failure={failure}
        onRetry={() =>
          request(
            { type: "RetryPlacementStep", courseId: course.id },
            "Retry did not go through.",
          )
        }
      />
    );
  } else if (step?.type === "ChooseRating") {
    body = (
      <RatingForm
        busy={busy}
        failure={failure}
        questions={questions}
        onSubmit={(rating, goal) =>
          request(
            { type: "SubmitSelfRating", courseId: course.id, rating, goal },
            "Placement could not start.",
          )
        }
      />
    );
  } else if (step?.type === "PreparingQuestions") {
    body = (
      <Waiting
        title="Preparing a few questions"
        line={`They come one at a time, ${questions} in all, and only decide where the course starts.`}
      />
    );
  } else if (step?.type === "Question") {
    body = (
      <QuestionForm
        key={step.question.id}
        question={step.question}
        busy={busy}
        failure={failure}
        onAnswer={(answer) =>
          request(
            {
              type: "AnswerPlacement",
              courseId: course.id,
              questionId: step.question.id,
              answer,
            },
            "The answer was not sent.",
          )
        }
      />
    );
  } else if (step?.type === "BuildingIndex" || step?.type === "Done") {
    body = (
      <Waiting
        title="Building the index"
        line="The whole topic, laid out before any reading. The first section opens when it is ready."
      />
    );
  }

  return (
    <WindowFrame
      width="task"
      bar={
        <>
          <Button
            variant="ghost"
            onClick={() => navigate({ type: "ShowLibrary" })}
          >
            <ChevronLeftIcon aria-hidden />
            Library
          </Button>
          <div
            data-tauri-drag-region
            className="flex min-w-0 items-baseline gap-2"
          >
            <WindowTitle heading>{course.topic}</WindowTitle>
            <span
              data-tauri-drag-region
              className="shrink-0 max-[760px]:hidden"
            >
              Placement
            </span>
          </div>
          <span data-tauri-drag-region className="flex-1 self-stretch" />
          {step?.type === "Question" && placement !== null && (
            <>
              <span
                data-tauri-drag-region
                className="shrink-0 whitespace-nowrap tabular-nums"
              >
                Question {step.question.number} of {placement.minQuestions} to{" "}
                {placement.maxQuestions}
              </span>
              <StepMarks number={step.question.number} state={placement} />
            </>
          )}
        </>
      }
    >
      {body}
    </WindowFrame>
  );
}
