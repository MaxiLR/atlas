import { CornerDownLeftIcon } from "lucide-react";
import {
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
} from "react";

import {
  Questionnaire,
  QuestionnaireChoice,
  QuestionnaireChoices,
  QuestionnaireItem,
  QuestionnaireSubmit,
  QuestionnaireTitle,
} from "@/components/ui/questionnaire";
import type { PlacementAnswer } from "@/core/generated/PlacementAnswer";
import type { PlacementQuestion } from "@/core/generated/PlacementQuestion";

import { Actions, continueStyle } from "./actions";

const NOT_SURE = "not-sure";

/** The option a key chooses: 1 to 4 the options in order, 0 Not sure. */
function choiceForKey(key: string, options: number): string | null {
  if (key === "0") return NOT_SURE;
  const index = Number(key) - 1;
  return /^[1-9]$/.test(key) && index < options ? String(index) : null;
}

/** A field that takes typed text, where digits are text and not choices. */
function takesText(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable || target instanceof HTMLTextAreaElement)
    return true;
  return (
    target instanceof HTMLInputElement &&
    !["radio", "checkbox", "button", "submit", "reset"].includes(target.type)
  );
}

/**
 * One placement question: the question, its four options with the keys 1 to
 * 4 and Not sure with 0, set apart. Enter continues once an option is chosen.
 * Focus starts on the question. Nothing marks an answer right or wrong.
 */
export function QuestionForm({
  question,
  busy,
  failure,
  onAnswer,
}: {
  question: PlacementQuestion;
  busy: boolean;
  failure: string | null;
  onAnswer: (answer: PlacementAnswer) => void;
}) {
  const [choice, setChoice] = useState<string | null>(null);
  const form = useRef<HTMLFormElement>(null);
  const group = useRef<HTMLFieldSetElement>(null);
  const options = question.options.length;
  const name = `question-${question.id}`;

  useEffect(() => group.current?.focus(), []);

  useEffect(() => {
    if (busy) return;
    function choose(event: globalThis.KeyboardEvent) {
      if (event.defaultPrevented || event.repeat) return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (takesText(event.target)) return;
      const chosen = choiceForKey(event.key, options);
      if (chosen === null) return;
      event.preventDefault();
      setChoice(chosen);
      form.current
        ?.querySelector<HTMLInputElement>(`input[value="${chosen}"]`)
        ?.focus();
    }
    window.addEventListener("keydown", choose);
    return () => window.removeEventListener("keydown", choose);
  }, [busy, options]);

  function continueOnEnter(event: KeyboardEvent<HTMLFormElement>) {
    if (event.key !== "Enter" || event.target instanceof HTMLButtonElement)
      return;
    event.preventDefault();
    if (choice !== null && !busy) event.currentTarget.requestSubmit();
  }

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (choice === null || busy) return;
    onAnswer(
      choice === NOT_SURE
        ? { type: "NotSure" }
        : { type: "Option", index: Number(choice) },
    );
  }

  return (
    <Questionnaire
      ref={form}
      item={name}
      onSubmit={submit}
      onKeyDown={continueOnEnter}
      aria-busy={busy || undefined}
      className="gap-0"
    >
      <QuestionnaireItem name={name} ref={group}>
        <QuestionnaireTitle>{question.stem}</QuestionnaireTitle>
        <QuestionnaireChoices className="mt-4">
          {question.options.map((text, index) => (
            <QuestionnaireChoice
              key={index}
              value={String(index)}
              shortcut={String(index + 1)}
              checked={choice === String(index)}
              onChange={() => setChoice(String(index))}
            >
              {text}
            </QuestionnaireChoice>
          ))}
          <QuestionnaireChoice
            value={NOT_SURE}
            shortcut="0"
            checked={choice === NOT_SURE}
            onChange={() => setChoice(NOT_SURE)}
            className="mt-3 border-transparent bg-wash text-muted-foreground hover:not-data-checked:border-transparent hover:not-data-checked:bg-wash-strong"
          >
            Not sure
          </QuestionnaireChoice>
        </QuestionnaireChoices>
      </QuestionnaireItem>
      <Actions
        failure={failure}
        note="Nothing is marked right or wrong here. Your answers only decide where the course starts."
      >
        <QuestionnaireSubmit
          className={continueStyle}
          disabled={busy || choice === null}
        >
          Continue
          <CornerDownLeftIcon aria-hidden className="opacity-60" />
        </QuestionnaireSubmit>
      </Actions>
    </Questionnaire>
  );
}
