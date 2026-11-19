import { ChevronLeftIcon } from "lucide-react";
import { useState } from "react";

import { useAtlas, useAtlasStore } from "@/app/store";
import { WindowFrame } from "@/app/window";
import { Button } from "@/components/ui/button";
import { describeError } from "@/core/client";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { PlanUsage } from "@/core/generated/PlanUsage";

const planUsageText: Record<PlanUsage, string> = {
  On: "Plan usage is on.",
  Off: "Plan usage is off.",
  NotAllowed: "Plan usage is not allowed for this account or workspace.",
};

/** The connected account, Manage usage and Sign out. */
export function Settings() {
  const { send, navigate } = useAtlasStore();
  const session = useAtlas((state) => state.session.session);
  const [error, setError] = useState<unknown>(null);

  function request(request: CoreRequest) {
    setError(null);
    send(request).catch(setError);
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
      <h1 className="text-20 font-medium text-heading">Settings</h1>
      <section
        aria-labelledby="account-heading"
        className="flex flex-col gap-2"
      >
        <h2 id="account-heading" className="text-sm font-medium">
          Account
        </h2>
        {session?.type === "SignedIn" ? (
          <>
            <p>{session.account.email}</p>
            <p>{planUsageText[session.planUsage]}</p>
          </>
        ) : (
          <p>Not signed in.</p>
        )}
      </section>
      <div className="flex gap-2">
        <Button onClick={() => request({ type: "OpenManageUsage" })}>
          Manage usage
        </Button>
        <Button onClick={() => request({ type: "SignOut" })}>Sign out</Button>
      </div>
      {error !== null && <p role="alert">{describeError(error)}</p>}
    </WindowFrame>
  );
}
