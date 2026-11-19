import { useState, type FormEvent, type ReactNode } from "react";

import { useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import { describeError } from "@/core/client";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CourseView } from "@/core/generated/CourseView";
import type { Goal } from "@/core/generated/Goal";
import type { PlacementAnswer } from "@/core/generated/PlacementAnswer";
import type { PlacementQuestion } from "@/core/generated/PlacementQuestion";
import type { SelfRating } from "@/core/generated/SelfRating";

const ratings: [SelfRating, string][] = [
  ["NewToThis", "New to this"],
  ["KnowTheBasics", "Know the basics"],
  ["UseItRegularly", "Use it regularly"],
  ["Expert", "Expert"],
];

const goals: [Goal, string][] = [
  ["Overview", "Overview"],
  ["WorkingKnowledge", "Working knowledge"],
  ["Expert", "Expert"],
];

/** One choice among radios, under a legend that names the group. */
function Choice<Value extends string>({
  legend,
  name,
  options,
  value,
  onChange,
}: {
  legend: ReactNode;
  name: string;
  options: [Value, string][];
  value: Value | null;
  onChange: (value: Value) => void;
}) {
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="mb-2 font-medium">{legend}</legend>
      {options.map(([option, label]) => (
        <label key={option} className="flex items-center gap-2">
          <input
            type="radio"
            name={name}
            value={option}
            checked={value === option}
            onChange={() => onChange(option)}
          />
          {label}
        </label>
      ))}
    </fieldset>
  );
}

function RatingForm({
  busy,
  onSubmit,
}: {
  busy: boolean;
  onSubmit: (rating: SelfRating, goal: Goal) => void;
}) {
  const [rating, setRating] = useState<SelfRating | null>(null);
  const [goal, setGoal] = useState<Goal | null>(null);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (rating !== null && goal !== null) onSubmit(rating, goal);
  }

  return (
    <form className="flex flex-col gap-6" onSubmit={submit}>
      <Choice
        legend="How well do you know it?"
        name="rating"
        options={ratings}
        value={rating}
        onChange={setRating}
      />
      <Choice
        legend="What is your goal?"
        name="goal"
        options={goals}
        value={goal}
        onChange={setGoal}
      />
      <Button
        type="submit"
        className="self-start"
        disabled={busy || rating === null || goal === null}
      >
        Continue
      </Button>
    </form>
  );
}

function QuestionForm({
  question,
  busy,
  onAnswer,
}: {
  question: PlacementQuestion;
  busy: boolean;
  onAnswer: (answer: PlacementAnswer) => void;
}) {
  const [choice, setChoice] = useState<string | null>(null);
  const options: [string, string][] = [
    ...question.options.map((text, index): [string, string] => [
      String(index),
      text,
    ]),
    ["NotSure", "Not sure"],
  ];

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (choice === null) return;
    onAnswer(
      choice === "NotSure"
        ? { type: "NotSure" }
        : { type: "Option", index: Number(choice) },
    );
  }

  return (
    <form className="flex flex-col gap-6" onSubmit={submit}>
      <Choice
        legend={question.stem}
        name={`question-${question.id}`}
        options={options}
        value={choice}
        onChange={setChoice}
      />
      <p className="text-sm">
        Nothing is marked right or wrong here. Your answers only decide where
        the course starts.
      </p>
      <Button
        type="submit"
        className="self-start"
        disabled={busy || choice === null}
      >
        Continue
      </Button>
    </form>
  );
}

/** Placement: the self-rating and the goal, then one question at a time. */
export function Placement({ course }: { course: CourseView }) {
  const { send, navigate } = useAtlasStore();
  const placement = useAtlas(
    (state) => state.placement.byCourse[course.id] ?? course.placement,
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);

  async function request(request: CoreRequest) {
    setBusy(true);
    setError(null);
    try {
      await send(request);
    } catch (refusal) {
      setError(refusal);
    } finally {
      setBusy(false);
    }
  }

  const step = placement?.step;
  return (
    <main className="mx-auto flex max-w-xl flex-col gap-6 p-8">
      <Button
        variant="ghost"
        className="self-start"
        onClick={() => navigate({ type: "ShowLibrary" })}
      >
        Library
      </Button>
      <h1 className="text-xl font-medium">{course.topic}</h1>
      <p className="text-sm">Placement</p>
      {step?.type === "ChooseRating" && (
        <RatingForm
          busy={busy}
          onSubmit={(rating, goal) =>
            request({
              type: "SubmitSelfRating",
              courseId: course.id,
              rating,
              goal,
            })
          }
        />
      )}
      {step?.type === "PreparingQuestions" && placement?.error == null && (
        <p role="status">Drafting the questions…</p>
      )}
      {step?.type === "Question" && placement != null && (
        <>
          <p>
            Question {step.question.number} of {placement.minQuestions} to{" "}
            {placement.maxQuestions}
          </p>
          <QuestionForm
            key={step.question.id}
            question={step.question}
            busy={busy}
            onAnswer={(answer) =>
              request({
                type: "AnswerPlacement",
                courseId: course.id,
                questionId: step.question.id,
                answer,
              })
            }
          />
        </>
      )}
      {step?.type === "BuildingIndex" && placement?.error == null && (
        <p role="status">Building the index…</p>
      )}
      {placement?.error != null && (
        <div className="flex flex-col gap-2">
          <p role="alert">{describeError(placement.error)}</p>
          <Button
            className="self-start"
            disabled={busy}
            onClick={() =>
              request({ type: "RetryPlacementStep", courseId: course.id })
            }
          >
            Retry
          </Button>
        </div>
      )}
      {error !== null && <p role="alert">{describeError(error)}</p>}
    </main>
  );
}
