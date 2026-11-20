import { CornerDownLeftIcon } from "lucide-react";
import { useEffect, useRef, useState, type Ref } from "react";

import { Button } from "@/components/ui/button";
import {
  Questionnaire,
  QuestionnaireChoice,
  QuestionnaireChoices,
  QuestionnaireItem,
  QuestionnaireTitle,
} from "@/components/ui/questionnaire";
import type { Goal } from "@/core/generated/Goal";
import type { SelfRating } from "@/core/generated/SelfRating";

import { Actions, continueStyle, groupFocusStyle } from "./actions";

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

/**
 * One single-choice group as option rows. Enter on a chosen row asks the
 * screen to continue.
 */
function ChoiceGroup<Value extends string>({
  name,
  title,
  options,
  value,
  onChange,
  onEnter,
  ref,
}: {
  name: string;
  title: string;
  options: [Value, string][];
  value: Value | null;
  onChange: (value: Value) => void;
  onEnter: () => void;
  ref?: Ref<HTMLFieldSetElement>;
}) {
  return (
    <Questionnaire
      item={name}
      onSubmit={(event) => {
        event.preventDefault();
        onEnter();
      }}
    >
      <QuestionnaireItem name={name} ref={ref} className={groupFocusStyle}>
        <QuestionnaireTitle>{title}</QuestionnaireTitle>
        <QuestionnaireChoices className="mt-4">
          {options.map(([option, label]) => (
            <QuestionnaireChoice
              key={option}
              value={option}
              checked={value === option}
              onChange={() => onChange(option)}
            >
              {label}
            </QuestionnaireChoice>
          ))}
        </QuestionnaireChoices>
      </QuestionnaireItem>
    </Questionnaire>
  );
}

/**
 * The self-rating and the goal on one screen; Continue needs both. Focus
 * starts on the self-rating.
 */
export function RatingForm({
  busy,
  failure,
  questions,
  onSubmit,
}: {
  busy: boolean;
  failure: string | null;
  /** How many questions follow, as "4 to 6". */
  questions: string;
  onSubmit: (rating: SelfRating, goal: Goal) => void;
}) {
  const [rating, setRating] = useState<SelfRating | null>(null);
  const [goal, setGoal] = useState<Goal | null>(null);
  const ratingGroup = useRef<HTMLFieldSetElement>(null);
  const goalGroup = useRef<HTMLFieldSetElement>(null);

  useEffect(() => ratingGroup.current?.focus(), []);

  function submit() {
    if (rating === null) ratingGroup.current?.focus();
    else if (goal === null) goalGroup.current?.focus();
    else if (!busy) onSubmit(rating, goal);
  }

  return (
    <div className="flex flex-col gap-10" aria-busy={busy || undefined}>
      <ChoiceGroup
        ref={ratingGroup}
        name="rating"
        title="How well do you know it?"
        options={ratings}
        value={rating}
        onChange={setRating}
        onEnter={submit}
      />
      <div>
        <ChoiceGroup
          ref={goalGroup}
          name="goal"
          title="What is your goal?"
          options={goals}
          value={goal}
          onChange={setGoal}
          onEnter={submit}
        />
        <Actions
          failure={failure}
          note={`Next come ${questions} questions. They only decide where the course starts.`}
        >
          <Button
            variant="primary"
            size="lg"
            className={continueStyle}
            disabled={busy || rating === null || goal === null}
            onClick={submit}
          >
            Continue
            <CornerDownLeftIcon aria-hidden className="opacity-60" />
          </Button>
        </Actions>
      </div>
    </div>
  );
}
