import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import { useId, useMemo, useState } from "react";

import { CheckPlaceContext } from "@/catalog/components/check-question/check-place";
import { Button } from "@/components/ui/button";
import type { CheckQuestion } from "@/core/generated/CheckQuestion";
import { cn } from "@/lib/utils";

import { RegistryEntry } from "./registry-entry";

/**
 * Where the check block sits: its stored version (null while written) and
 * index, and whether its questions can be answered there.
 */
export interface CheckBlock {
  versionId: string | null;
  block: number;
  answerable: boolean;
}

/** An arrow at an end of the block: the disabled look, focus kept. */
const atEnd =
  "aria-disabled:cursor-not-allowed aria-disabled:border-transparent aria-disabled:bg-transparent aria-disabled:opacity-50 aria-disabled:hover:bg-transparent aria-disabled:hover:text-body aria-disabled:active:scale-100";

/**
 * A section's check questions under one heading. Where they can be
 * answered, one question shows at a time, with arrows between them and the
 * position when there are two or more; elsewhere every question is listed.
 */
export function Check({
  questions,
  ...at
}: CheckBlock & { questions: CheckQuestion[] }) {
  const heading = questions.length === 1 ? "Check question" : "Check questions";
  if (!at.answerable) {
    return (
      <>
        <h2>{heading}</h2>
        {questions.map((question) => (
          <CheckEntry key={question.index} question={question} {...at} />
        ))}
      </>
    );
  }
  return <Paged heading={heading} questions={questions} {...at} />;
}

/**
 * The answerable block. It opens on the first question not yet answered
 * (the first when all are) and moves only through its arrows, which stop at
 * the ends. Every question stays mounted in one grid cell, so each keeps its
 * choice and the block keeps its tallest question's height; only the
 * current one is visible and within reach.
 */
function Paged({
  heading,
  questions,
  ...at
}: CheckBlock & { heading: string; questions: CheckQuestion[] }) {
  const headingId = useId();
  const [shown, setShown] = useState(() =>
    Math.max(
      0,
      questions.findIndex((question) => question.answer === null),
    ),
  );
  // The side the shown question came in from; null until the first move.
  const [from, setFrom] = useState<"left" | "right" | null>(null);
  const last = questions.length - 1;

  const move = (step: -1 | 1) => {
    const to = shown + step;
    if (to < 0 || to > last) return;
    setFrom(step > 0 ? "right" : "left");
    setShown(to);
  };

  return (
    <section aria-labelledby={headingId} className="mt-[2.75em]">
      <div className="mb-5 flex items-center gap-3">
        <h2 id={headingId} className="mt-0 min-w-0 flex-1">
          {heading}
        </h2>
        {questions.length > 1 && (
          <div
            data-slot="check-pager"
            className="flex shrink-0 items-center gap-1"
          >
            <Button
              size="icon"
              aria-label="Previous question"
              aria-disabled={shown === 0 || undefined}
              onClick={() => move(-1)}
              className={atEnd}
            >
              <ChevronLeftIcon aria-hidden />
            </Button>
            <output
              aria-live="polite"
              className="min-w-13 text-center text-13 text-muted-foreground tabular-nums"
            >
              {shown + 1} of {questions.length}
            </output>
            <Button
              size="icon"
              aria-label="Next question"
              aria-disabled={shown === last || undefined}
              onClick={() => move(1)}
              className={atEnd}
            >
              <ChevronRightIcon aria-hidden />
            </Button>
          </div>
        )}
      </div>
      <div className="grid">
        {questions.map((question, index) => {
          const current = index === shown;
          return (
            // A question not shown keeps its space and its state, and is out
            // of reach: not painted, inert, hidden from assistive technology
            // and its controls disabled.
            <fieldset
              key={question.index}
              role="none"
              disabled={!current}
              inert={!current}
              aria-hidden={!current || undefined}
              className={cn(
                "col-start-1 row-start-1 min-w-0",
                !current && "invisible",
                current &&
                  from !== null &&
                  "animate-in duration-150 ease-out fade-in-0 motion-reduce:animate-none",
                current && from === "right" && "slide-in-from-right-2",
                current && from === "left" && "slide-in-from-left-2",
              )}
            >
              <CheckEntry question={question} {...at} />
            </fieldset>
          );
        })}
      </div>
    </section>
  );
}

function CheckEntry({
  question,
  versionId,
  block,
  answerable,
}: CheckBlock & { question: CheckQuestion }) {
  const { stem, options, index, answer } = question;
  const props = useMemo(
    () => ({ stem, options: options.map((text) => ({ text })) }),
    [stem, options],
  );
  const place = useMemo(
    () => ({ versionId, block, question: index, answer }),
    [versionId, block, index, answer],
  );
  // Outside a place to answer it, the registry shows the stem and options.
  if (!answerable) {
    return <RegistryEntry type="check-question" props={props} />;
  }
  return (
    <CheckPlaceContext value={place}>
      <RegistryEntry type="check-question" props={props} />
    </CheckPlaceContext>
  );
}
