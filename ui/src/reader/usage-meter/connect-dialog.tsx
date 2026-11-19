import { CheckIcon, CopyIcon } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

import { countdown } from "./pace";
import { useNow } from "./use-now";

/**
 * The usage login in progress: the device code to enter on OpenAI's page,
 * which the core has opened, and the time left to enter it. Closing it
 * cancels the login.
 */
export function ConnectUsageDialog({
  userCode,
  expiresAt,
  onCancel,
}: {
  userCode: string;
  expiresAt: string;
  onCancel: () => void;
}) {
  const now = useNow(1000);
  const [copied, setCopied] = useState(false);
  const left = Date.parse(expiresAt) - now;

  function copy() {
    navigator.clipboard.writeText(userCode).then(
      () => setCopied(true),
      () => setCopied(false),
    );
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onCancel()}>
      <DialogContent showCloseButton={false} className="gap-5 sm:max-w-sm">
        <DialogHeader className="pr-0">
          <DialogTitle>Connect usage</DialogTitle>
          <DialogDescription className="text-balance">
            OpenAI's page has opened in your browser. Enter this code there so
            Atlas can read your plan's usage.
          </DialogDescription>
        </DialogHeader>
        <div className="flex items-center justify-between gap-3 rounded-lg border border-edge py-2 pr-2 pl-4">
          <span className="text-28 font-medium tracking-[0.06em] text-heading select-all">
            {userCode}
          </span>
          <Button
            variant="ghost"
            size="icon"
            aria-label="Copy code"
            onClick={copy}
          >
            {copied ? <CheckIcon aria-hidden /> : <CopyIcon aria-hidden />}
          </Button>
          <span role="status" className="sr-only">
            {copied ? "Code copied" : ""}
          </span>
        </div>
        <p className="text-13 text-muted-foreground tabular-nums">
          {left > 0
            ? `Code expires in ${countdown(left)}`
            : "The code has expired. Cancel and connect again for a new one."}
        </p>
        <DialogFooter>
          <Button onClick={onCancel}>Cancel</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
