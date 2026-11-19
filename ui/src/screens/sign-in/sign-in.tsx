import { useEffect, useRef, useState, type ReactNode, type Ref } from "react";

import { useAtlas, useAtlasStore } from "@/app/store";
import { WindowFrame } from "@/app/window";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { describeError } from "@/core/client";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { Session } from "@/core/generated/Session";
import type { SignedOutReason } from "@/core/generated/SignedOutReason";
import { unknownTag } from "@/store/unknown-tag";

/** Why the candidate is signed out, in one line. */
function reasonText(reason: SignedOutReason): string {
  switch (reason.type) {
    case "SignedOut":
      return "You signed out.";
    case "LoginEnded":
      return "Your ChatGPT login ended. Sign in again; your courses are kept.";
    case "Declined":
      return "Permission to use your plan was declined in the browser.";
    case "Failed":
      return `Signing in did not finish. ${describeError(reason.error)}`;
    default:
      return unknownTag(reason);
  }
}

/** A request the core refused, and which one. */
interface Refusal {
  request: CoreRequest["type"];
  error: unknown;
}

function refusalText({ request, error }: Refusal): string {
  return request === "SignIn"
    ? `Signing in did not start. ${describeError(error)}`
    : describeError(error);
}

/** Which of the screen's states the session is in. */
function stateOf(session: Session | null): string {
  if (session?.type === "SignedIn") return `SignedIn:${session.planUsage}`;
  return session?.type ?? "Unknown";
}

/**
 * The screen's one task: a heading and a sentence, then one line of what
 * happened above the actions, beside a muted note. Below 760 px the actions
 * stack above the note at full width.
 */
function Task({
  heading,
  lead,
  line,
  note,
  actions,
}: {
  heading: string;
  lead: ReactNode;
  line: string | null;
  note: ReactNode;
  actions: ReactNode;
}) {
  return (
    <WindowFrame width="task">
      <div className="flex flex-col gap-3">
        <h1 className="text-20 font-medium text-balance text-heading">
          {heading}
        </h1>
        <p className="max-w-[60ch] text-15 text-pretty text-body">{lead}</p>
      </div>
      <div className="flex flex-col">
        <p role="status" className="text-13 text-body not-empty:mb-4">
          {line}
        </p>
        <div className="flex items-center justify-between gap-6 max-[760px]:flex-col-reverse max-[760px]:items-stretch max-[760px]:gap-4">
          <div className="flex max-w-[46ch] items-center gap-2 text-13 text-muted-foreground">
            {note}
          </div>
          <div className="flex shrink-0 items-center gap-2 max-[760px]:flex-col-reverse max-[760px]:items-stretch">
            {actions}
          </div>
        </div>
      </div>
    </WindowFrame>
  );
}

/** The primary action: the one filled button on the screen. */
function PrimaryAction({
  ref,
  onClick,
  children,
}: {
  ref: Ref<HTMLButtonElement>;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Button
      ref={ref}
      variant="primary"
      size="lg"
      className="max-[760px]:h-9"
      onClick={onClick}
    >
      {children}
    </Button>
  );
}

/** A secondary action beside the primary one. */
function OtherAction({
  ref,
  onClick,
  children,
}: {
  ref?: Ref<HTMLButtonElement>;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Button
      ref={ref}
      variant="ghost"
      size="lg"
      className="max-[760px]:h-9"
      onClick={onClick}
    >
      {children}
    </Button>
  );
}

/**
 * Signing in with ChatGPT, and what came of it: signed out with one action,
 * the browser open with Cancel, or signed in with plan usage off or not
 * allowed, each said once with the action that moves on from it.
 */
export function SignIn() {
  const { send, navigate, screenChanged } = useAtlasStore();
  const session = useAtlas((state) => state.session.session);
  const [refusal, setRefusal] = useState<Refusal | null>(null);
  const action = useRef<HTMLButtonElement>(null);
  const state = stateOf(session);
  const shown = useRef(state);

  // When the action that had focus gives way to the next state's, focus
  // follows to the new one. It is not taken on arrival at launch, but is
  // when another screen led here, as signing out does.
  useEffect(() => {
    if (screenChanged()) action.current?.focus();
  }, [screenChanged]);
  useEffect(() => {
    if (shown.current === state) return;
    shown.current = state;
    const focused = document.activeElement;
    if (focused === null || focused === document.body) action.current?.focus();
  }, [state]);

  function request(request: CoreRequest) {
    setRefusal(null);
    send(request).catch((error: unknown) =>
      setRefusal({ request: request.type, error }),
    );
  }

  const refused = refusal === null ? null : refusalText(refusal);
  const toLibrary = () => navigate({ type: "ShowLibrary" });

  if (session?.type === "SigningIn") {
    return (
      <Task
        heading="Atlas"
        lead="Your browser has opened on the sign-in page. Finish there and Atlas carries on by itself."
        line={refused}
        note={
          <>
            <Spinner role="presentation" aria-hidden />
            Waiting for the browser
          </>
        }
        actions={
          <Button
            ref={action}
            size="lg"
            className="max-[760px]:h-9"
            onClick={() => request({ type: "CancelSignIn" })}
          >
            Cancel
          </Button>
        }
      />
    );
  }

  if (session?.type === "SignedIn") {
    const signedInAs = `Signed in as ${session.account.email}.`;
    switch (session.planUsage) {
      case "Off":
        return (
          <Task
            heading="Plan usage is off"
            lead="Atlas may not use your ChatGPT plan yet. No course can start until plan usage is on; your courses stay readable."
            line={refused}
            note={signedInAs}
            actions={
              <>
                <OtherAction onClick={toLibrary}>
                  Continue to library
                </OtherAction>
                <PrimaryAction
                  ref={action}
                  onClick={() => request({ type: "EnablePlanUsage" })}
                >
                  Enable plan usage
                </PrimaryAction>
              </>
            }
          />
        );
      case "NotAllowed":
        return (
          <Task
            heading="Plan usage is not allowed"
            lead="Plan usage is not allowed for this account or workspace, so Atlas cannot write courses with it. Signing in again would end the same way; the setting is under Manage usage, or with the workspace's admin."
            line={refused}
            note={signedInAs}
            actions={
              <>
                <OtherAction onClick={toLibrary}>
                  Continue to library
                </OtherAction>
                <PrimaryAction
                  ref={action}
                  onClick={() => request({ type: "OpenManageUsage" })}
                >
                  Manage usage
                </PrimaryAction>
              </>
            }
          />
        );
      case "On":
        return (
          <Task
            heading="Atlas"
            lead="You are signed in and plan usage is on."
            line={refused}
            note={signedInAs}
            actions={
              <PrimaryAction ref={action} onClick={toLibrary}>
                Continue to library
              </PrimaryAction>
            }
          />
        );
      default:
        return unknownTag(session.planUsage);
    }
  }

  const reason = session?.type === "SignedOut" ? session.reason : null;
  return (
    <Task
      heading="Atlas"
      lead="Signing in lets Atlas write your courses on your own ChatGPT plan, with no API key."
      line={refused ?? (reason === null ? null : reasonText(reason))}
      note="Atlas opens your browser to sign in. It has no account or server of its own, and your courses stay on this Mac."
      actions={
        <PrimaryAction ref={action} onClick={() => request({ type: "SignIn" })}>
          Continue with ChatGPT
        </PrimaryAction>
      }
    />
  );
}
