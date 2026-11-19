import { useState } from "react";

import { useAtlas, useAtlasStore } from "@/app/store";
import { WindowFrame } from "@/app/window";
import { Button } from "@/components/ui/button";
import { describeError } from "@/core/client";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { SignedOutReason } from "@/core/generated/SignedOutReason";

function reasonText(reason: SignedOutReason): string {
  switch (reason.type) {
    case "SignedOut":
      return "You signed out.";
    case "LoginEnded":
      return "Your ChatGPT login ended. Your courses are kept.";
    case "Declined":
      return "Permission to use your plan was declined in the browser.";
    case "Failed":
      return describeError(reason.error);
  }
}

/** Signed out: one action, Continue with ChatGPT. */
export function SignIn() {
  const { send } = useAtlasStore();
  const session = useAtlas((state) => state.session.session);
  const [error, setError] = useState<unknown>(null);
  const reason = session?.type === "SignedOut" ? session.reason : null;

  function request(request: CoreRequest) {
    setError(null);
    send(request).catch(setError);
  }

  return (
    <WindowFrame width="task">
      <h1 className="text-20 font-medium text-heading">Atlas</h1>
      <p>
        Learn any topic: the whole territory first, then deep where you choose.
      </p>
      {reason !== null && <p role="status">{reasonText(reason)}</p>}
      {session?.type === "SigningIn" ? (
        <div className="flex flex-col gap-2">
          <p role="status">Finish signing in in your browser.</p>
          <Button
            variant="ghost"
            className="self-start"
            onClick={() => request({ type: "CancelSignIn" })}
          >
            Cancel
          </Button>
        </div>
      ) : (
        <Button
          className="self-start"
          onClick={() => request({ type: "SignIn" })}
        >
          Continue with ChatGPT
        </Button>
      )}
      {error !== null && <p role="alert">{describeError(error)}</p>}
    </WindowFrame>
  );
}
