import type { ComponentFn } from "@json-render/react";
import { CheckIcon, XIcon } from "lucide-react";
import { useId, useState, type FormEvent } from "react";

import { useAtlasStore } from "@/app/store";
import {
  Questionnaire,
  QuestionnaireChoice,
  QuestionnaireChoices,
  QuestionnaireItem,
  QuestionnaireSubmit,
  QuestionnaireTitle,
} from "@/components/ui/questionnaire";
import { Spinner } from "@/components/ui/spinner";
import { describeError } from "@/core/client";
import { cn } from "@/lib/utils";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";
import { useCheckPlace, type CheckPlace } from "./check-place.ts";

/**
 * A check question. In a section it is answerable: its options as a radio
 * group and Check answer, then the outcome from the core. Elsewhere, the
 * stem and the options; the key and the explanation stay untold.
 */
export const CheckQuestion: ComponentFn<AtlasCatalog, "check-question"> = ({
  props,
}) => {
  const place = useCheckPlace();
  if (place === null) {
    return (
      <PlainSection heading={props.stem}>
        <ol>
          {props.options.map((option, index) => (
            <li key={index}>{option.text}</li>
          ))}
        </ol>
      </PlainSection>
    );
  }
  return (
    <AnswerableCheck
      stem={props.stem}
      options={props.options.map((option) => option.text)}
      place={place}
    />
  );
};

/** What an option row says once the question is answered, if anything. */
type Mark = { correct: boolean; words: string } | null;

function markOf(place: CheckPlace, index: number): Mark {
  const { answer } = place;
  if (answer === null) return null;
  if (index === answer.choice) {
    return answer.correct
      ? { correct: true, words: "Your answer, correct" }
      : { correct: false, words: "Your answer, incorrect" };
  }
  return index === answer.key
    ? { correct: true, words: "Correct answer" }
    : null;
}

function OutcomeIcon({ correct }: { correct: boolean }) {
  const Icon = correct ? CheckIcon : XIcon;
  return (
    <Icon
      aria-hidden
      className={cn("size-3.5 shrink-0", correct ? "text-green" : "text-red")}
    />
  );
}

/**
 * The stem and four option rows. Check answer asks the core once an option
 * is chosen; the result marks the chosen and the right option with a mark
 * and words, shows the explanation and locks the group. A failed check
 * keeps the choice and offers to try again.
 */
function AnswerableCheck({
  stem,
  options,
  place,
}: {
  stem: string;
  options: string[];
  place: CheckPlace;
}) {
  const { send } = useAtlasStore();
  const name = useId();
  const [chosen, setChosen] = useState<number | null>(null);
  const [checking, setChecking] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const { answer, versionId } = place;
  const choice = answer?.choice ?? chosen;
  const locked = answer !== null || checking;

  const check = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (choice === null || locked || versionId === null) return;
    setChecking(true);
    setFailure(null);
    try {
      await send({
        type: "AnswerCheck",
        versionId,
        block: place.block,
        question: place.question,
        choice,
      });
    } catch (error) {
      setFailure(`The answer could not be checked. ${describeError(error)}`);
    } finally {
      setChecking(false);
    }
  };

  return (
    // Questionnaire flags an item invalid once submitted while none of its
    // enabled choices is chosen, which a locked group always is: mounting
    // it afresh when it locks keeps the locked group valid.
    <Questionnaire
      key={locked ? "locked" : "open"}
      onSubmit={check}
      aria-busy={checking}
      className="gap-0"
    >
      <QuestionnaireItem name={name}>
        <QuestionnaireTitle className="text-16 text-heading">
          {stem}
        </QuestionnaireTitle>
        <QuestionnaireChoices>
          {options.map((text, index) => {
            const mark = markOf(place, index);
            return (
              <QuestionnaireChoice
                key={index}
                value={String(index)}
                checked={choice === index}
                disabled={locked}
                onChange={() => setChosen(index)}
                className={cn(
                  answer !== null && "data-checked:bg-transparent",
                  (mark !== null || index === choice) &&
                    "data-disabled:opacity-100",
                  mark?.correct === true &&
                    "border-green data-checked:border-green",
                  mark?.correct === false &&
                    "border-red data-checked:border-red",
                )}
              >
                {text}
                {mark !== null && (
                  <span className="flex items-center gap-1.5 text-13 font-medium text-heading">
                    <OutcomeIcon correct={mark.correct} />
                    {mark.words}
                  </span>
                )}
              </QuestionnaireChoice>
            );
          })}
        </QuestionnaireChoices>
      </QuestionnaireItem>
      {/* Live before the outcome arrives, so it is announced; a status only
          once there is one. */}
      <div aria-live="polite" role={answer !== null ? "status" : undefined}>
        {answer !== null && (
          <div className="mt-5 border-t border-edge pt-4">
            <p className="flex items-center gap-2 text-15 font-medium text-heading">
              <OutcomeIcon correct={answer.correct} />
              {answer.correct ? "Correct" : "Incorrect"}
            </p>
            <p className="mt-1.5 text-16 text-body">{answer.explanation}</p>
          </div>
        )}
      </div>
      {answer === null && (
        <div className="mt-3.5 flex flex-col items-start gap-3">
          {failure !== null && (
            <p role="alert" className="text-13 text-body">
              {failure}
            </p>
          )}
          <QuestionnaireSubmit
            disabled={choice === null || checking || versionId === null}
          >
            {checking ? (
              <>
                <Spinner aria-hidden className="text-current" />
                Checking
              </>
            ) : failure !== null ? (
              "Try again"
            ) : (
              "Check answer"
            )}
          </QuestionnaireSubmit>
        </div>
      )}
    </Questionnaire>
  );
}
