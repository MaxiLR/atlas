import * as React from "react";
import { Questionnaire as QuestionnairePrimitive } from "@shadcn/react/questionnaire";
import { cn } from "@/lib/utils";

import { buttonVariants, type Button } from "@/components/ui/button";
import { CheckIcon } from "lucide-react";

function Questionnaire({
  className,
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Root>) {
  return (
    <QuestionnairePrimitive.Root
      data-slot="questionnaire"
      className={cn("flex w-full min-w-0 flex-col gap-4", className)}
      {...props}
    />
  );
}

function QuestionnaireProgress({
  className,
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Progress>) {
  return (
    <QuestionnairePrimitive.Progress
      data-slot="questionnaire-progress"
      className={cn(
        "min-h-[1lh] w-fit min-w-[14ch] text-13 text-muted-foreground tabular-nums",
        className,
      )}
      {...props}
    />
  );
}

function QuestionnaireItem({
  className,
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Item>) {
  return (
    <QuestionnairePrimitive.Item
      data-slot="questionnaire-item"
      className={cn(
        "flex min-w-0 flex-col gap-4 border-0 p-0 outline-none",
        className,
      )}
      {...props}
    />
  );
}

function QuestionnaireTitle({
  className,
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Title>) {
  return (
    <QuestionnairePrimitive.Title
      data-slot="questionnaire-title"
      className={cn(
        "p-0 text-20 font-medium text-balance text-heading [&:not(:has(~[data-slot=questionnaire-description]))]:mb-3",
        className,
      )}
      {...props}
    />
  );
}

function QuestionnaireDescription({
  className,
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Description>) {
  return (
    <QuestionnairePrimitive.Description
      data-slot="questionnaire-description"
      className={cn("text-13 text-pretty text-muted-foreground", className)}
      {...props}
    />
  );
}

function QuestionnaireChoices({
  className,
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Choices>) {
  return (
    <QuestionnairePrimitive.Choices
      data-slot="questionnaire-choices"
      className={cn(
        "group/questionnaire-choices grid min-w-0 gap-2",
        className,
      )}
      {...props}
    />
  );
}

function QuestionnaireChoice({
  children,
  className,
  shortcut,
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Choice> & {
  /**
   * A key the screen handles itself, shown in the row's key chip, for keys
   * the root's `shortcuts` mode does not assign (0 for "Not sure").
   */
  shortcut?: string;
}) {
  return (
    <QuestionnairePrimitive.Choice
      data-slot="questionnaire-choice"
      data-shortcut={shortcut}
      className={cn(
        // An option row: 8 px corners and a hairline edge; the chosen one takes
        // the accent edge and wash.
        "group/questionnaire-choice relative grid min-w-0 cursor-pointer grid-cols-[1rem_minmax(0,1fr)_auto] items-start gap-3 rounded-md border border-edge px-3.5 py-3 text-start text-15 tracking-[-0.011em] text-body transition-[background-color,border-color] outline-none select-none hover:not-data-checked:border-edge-strong hover:not-data-checked:bg-wash active:not-data-disabled:scale-[0.995] has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-focus has-focus-visible:outline-solid data-checked:border-atlas-accent data-checked:bg-atlas-accent-wash data-checked:text-heading data-invalid:border-destructive",
        "data-disabled:cursor-not-allowed data-disabled:opacity-50",
        className,
      )}
      {...props}
    >
      <QuestionnairePrimitive.ChoiceInput
        data-slot="questionnaire-choice-input"
        aria-keyshortcuts={shortcut}
        className="absolute inset-0 z-10 size-full cursor-pointer opacity-0"
      />
      <span
        aria-hidden="true"
        data-slot="questionnaire-choice-indicator"
        className="pointer-events-none relative mt-[3px] flex size-4 shrink-0 items-center justify-center rounded-[4px] border border-input group-data-[type=radio]/questionnaire-choice:rounded-full group-data-checked/questionnaire-choice:border-atlas-accent group-data-[type=checkbox]/questionnaire-choice:group-data-checked/questionnaire-choice:bg-atlas-accent group-data-[type=checkbox]/questionnaire-choice:group-data-checked/questionnaire-choice:text-primary-foreground group-data-[type=radio]/questionnaire-choice:group-data-checked/questionnaire-choice:border-[5px]"
      >
        <CheckIcon
          data-slot="questionnaire-choice-indicator-check"
          className="hidden size-3 group-data-[type=radio]/questionnaire-choice:hidden group-data-checked/questionnaire-choice:block"
        />
      </span>
      <QuestionnairePrimitive.ChoiceLabel
        data-slot="questionnaire-choice-label"
        className="flex min-w-0 flex-col gap-0.5"
      >
        {children}
      </QuestionnairePrimitive.ChoiceLabel>
      <QuestionnairePrimitive.ChoiceShortcut
        data-slot="questionnaire-choice-shortcut"
        hidden={shortcut === undefined ? undefined : false}
        className="pointer-events-none mt-0.5 hidden h-[18px] min-w-[18px] shrink-0 items-center justify-center rounded-[4px] border border-edge-strong px-1 text-[11px] leading-none font-medium text-muted-foreground tabular-nums group-data-[shortcut]/questionnaire-choice:inline-flex"
      >
        {shortcut}
      </QuestionnairePrimitive.ChoiceShortcut>
    </QuestionnairePrimitive.Choice>
  );
}

function QuestionnaireChoiceDescription({
  className,
  ...props
}: React.ComponentProps<"span">) {
  return (
    <span
      data-slot="questionnaire-choice-description"
      className={cn("text-13 text-muted-foreground", className)}
      {...props}
    />
  );
}

function QuestionnaireInput({
  className,
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Input>) {
  return (
    <div
      data-slot="questionnaire-input-wrapper"
      className="group/questionnaire-input relative w-full min-w-0"
    >
      <QuestionnairePrimitive.Input
        data-slot="questionnaire-input"
        className={cn(
          "h-9 w-full min-w-0 rounded-lg border border-edge-strong bg-panel px-3 text-15 leading-none text-heading transition-[border-color,box-shadow] outline-none placeholder:text-muted-foreground focus-visible:border-focus focus-visible:ring-3 focus-visible:ring-atlas-accent-wash disabled:cursor-not-allowed disabled:bg-transparent disabled:text-muted-foreground aria-invalid:border-destructive",
          className,
        )}
        {...props}
      />
    </div>
  );
}

function QuestionnaireError({
  className,
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Error>) {
  return (
    <QuestionnairePrimitive.Error
      data-slot="questionnaire-error"
      className={cn("mt-2 text-13 text-destructive", className)}
      {...props}
    />
  );
}

function QuestionnaireActions({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="questionnaire-actions"
      className={cn(
        "grid w-full grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-2",
        className,
      )}
      {...props}
    />
  );
}

function QuestionnairePrevious({
  children,
  className,
  size = "default",
  variant = "ghost",
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Previous> &
  Pick<React.ComponentProps<typeof Button>, "size" | "variant">) {
  return (
    <QuestionnairePrimitive.Previous
      data-slot="questionnaire-previous"
      data-size={size}
      data-variant={variant}
      className={cn(
        buttonVariants({ size, variant }),
        "col-start-1 row-start-1 justify-self-start",
        className,
      )}
      {...props}
    >
      {children ?? "Previous"}
    </QuestionnairePrimitive.Previous>
  );
}

function QuestionnaireSkip({
  children,
  className,
  size = "default",
  variant = "ghost",
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Skip> &
  Pick<React.ComponentProps<typeof Button>, "size" | "variant">) {
  return (
    <QuestionnairePrimitive.Skip
      data-slot="questionnaire-skip"
      data-size={size}
      data-variant={variant}
      className={cn(
        buttonVariants({ size, variant }),
        "col-start-2 row-start-1 justify-self-end",
        className,
      )}
      {...props}
    >
      {children ?? "Skip"}
    </QuestionnairePrimitive.Skip>
  );
}

function QuestionnaireNext({
  children,
  className,
  size = "lg",
  variant = "primary",
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Next> &
  Pick<React.ComponentProps<typeof Button>, "size" | "variant">) {
  return (
    <QuestionnairePrimitive.Next
      data-slot="questionnaire-next"
      data-size={size}
      data-variant={variant}
      className={cn(
        buttonVariants({ size, variant }),
        "col-start-3 row-start-1 justify-self-end",
        className,
      )}
      {...props}
    >
      {children ?? "Next"}
    </QuestionnairePrimitive.Next>
  );
}

function QuestionnaireSubmit({
  children,
  className,
  size = "lg",
  variant = "primary",
  ...props
}: React.ComponentProps<typeof QuestionnairePrimitive.Submit> &
  Pick<React.ComponentProps<typeof Button>, "size" | "variant">) {
  return (
    <QuestionnairePrimitive.Submit
      data-slot="questionnaire-submit"
      data-size={size}
      data-variant={variant}
      className={cn(
        buttonVariants({ size, variant }),
        "col-start-3 row-start-1 justify-self-end",
        className,
      )}
      {...props}
    >
      {children ?? "Submit"}
    </QuestionnairePrimitive.Submit>
  );
}

export {
  Questionnaire,
  QuestionnaireActions,
  QuestionnaireChoice,
  QuestionnaireChoiceDescription,
  QuestionnaireChoices,
  QuestionnaireDescription,
  QuestionnaireError,
  QuestionnaireInput,
  QuestionnaireItem,
  QuestionnaireNext,
  QuestionnairePrevious,
  QuestionnaireProgress,
  QuestionnaireSkip,
  QuestionnaireSubmit,
  QuestionnaireTitle,
};
