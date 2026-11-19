import { ArrowUpRightIcon, ChevronLeftIcon } from "lucide-react";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";

import { useAtlas, useAtlasStore } from "@/app/store";
import { WindowFrame } from "@/app/window";
import { Button } from "@/components/ui/button";
import { describeError } from "@/core/client";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { PlanUsage } from "@/core/generated/PlanUsage";
import { ModelPicker } from "@/model-picker/model-picker";

import { ThemeMenu } from "./theme-menu";

const planUsageText: Record<PlanUsage, string> = {
  On: "On. New sections are written with your ChatGPT plan.",
  Off: "Off. No course can start until plan usage is enabled.",
  NotAllowed: "Not allowed for this account or workspace.",
};

/** What each request is, as a failure names it. */
const actionName = {
  EnablePlanUsage: "Plan usage could not be enabled.",
  OpenManageUsage: "ChatGPT's usage settings could not be opened.",
  SignOut: "Signing out failed.",
} as const;

type SettingsRequest = Extract<CoreRequest, { type: keyof typeof actionName }>;

/** A titled group of rows, its heading over a hairline. */
function Group({ title, children }: { title: string; children: ReactNode }) {
  const headingId = useId();
  return (
    <section aria-labelledby={headingId}>
      <h2
        id={headingId}
        className="border-b border-edge px-3 pb-2.5 text-13 font-medium text-heading"
      >
        {title}
      </h2>
      <ul>{children}</ul>
    </section>
  );
}

/** One setting: its name and what it is on the left, its control on the right. */
function Row({
  label,
  description,
  control,
}: {
  label: string;
  description: ReactNode;
  control?: ReactNode;
}) {
  return (
    <li className="flex min-h-15 flex-wrap items-center justify-between gap-x-6 gap-y-2 border-b border-edge py-3 pr-2 pl-3">
      <div className="flex min-w-0 flex-[1_1_16rem] flex-col gap-0.5 text-13">
        <span className="text-heading">{label}</span>
        <span className="break-words text-muted-foreground">{description}</span>
      </div>
      {control !== undefined && (
        <div className="flex shrink-0 items-center gap-2">{control}</div>
      )}
    </li>
  );
}

/**
 * The few settings Atlas has: the connected account with its plan usage,
 * Manage usage and Sign out; the model in use; the theme. The bar and
 * Escape lead back to the library.
 */
export function Settings() {
  const { send, navigate } = useAtlasStore();
  const session = useAtlas((state) => state.session.session);
  const [failure, setFailure] = useState<string | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const signedIn = session?.type === "SignedIn" ? session : null;

  useEffect(() => heading.current?.focus(), []);

  useEffect(() => {
    // A menu that handles Escape itself marks the event handled.
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      event.preventDefault();
      navigate({ type: "ShowLibrary" });
    };
    document.addEventListener("keydown", escape);
    return () => document.removeEventListener("keydown", escape);
  }, [navigate]);

  function request(request: SettingsRequest) {
    setFailure(null);
    send(request).catch((error: unknown) =>
      setFailure(`${actionName[request.type]} ${describeError(error)}`),
    );
  }

  return (
    <WindowFrame
      width="list"
      bar={
        <Button
          variant="ghost"
          onClick={() => navigate({ type: "ShowLibrary" })}
        >
          <ChevronLeftIcon aria-hidden />
          Library
        </Button>
      }
    >
      <h1
        ref={heading}
        tabIndex={-1}
        className="text-20 font-medium tracking-[-0.012em] text-heading outline-none"
      >
        Settings
      </h1>
      {failure !== null && (
        <p role="alert" className="px-3 text-13 text-body">
          {failure}
        </p>
      )}
      <Group title="Account">
        <Row
          label="ChatGPT account"
          description={signedIn?.account.email ?? "Not signed in."}
          control={
            <Button onClick={() => request({ type: "SignOut" })}>
              Sign out
            </Button>
          }
        />
        <Row
          label="Plan usage"
          description={
            signedIn === null
              ? "Known once you sign in."
              : planUsageText[signedIn.planUsage]
          }
          control={
            signedIn?.planUsage === "Off" ? (
              <Button
                variant="primary"
                onClick={() => request({ type: "EnablePlanUsage" })}
              >
                Enable plan usage
              </Button>
            ) : undefined
          }
        />
        <Row
          label="Usage limits"
          description="Your plan's limits, in ChatGPT's settings in the browser."
          control={
            <Button onClick={() => request({ type: "OpenManageUsage" })}>
              Manage usage
              <ArrowUpRightIcon aria-hidden />
            </Button>
          }
        />
      </Group>
      <Group title="Preferences">
        <Row
          label="Model"
          description="Writes new sections and answers side chats."
          control={<ModelPicker />}
        />
        <Row
          label="Theme"
          description="Light, dark, or follow the system."
          control={<ThemeMenu />}
        />
      </Group>
    </WindowFrame>
  );
}
