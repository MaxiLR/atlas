import type * as React from "react";
import { CircleCheckIcon, OctagonXIcon, XIcon } from "lucide-react";
import { Toaster as Sonner, type ToasterProps } from "sonner";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useTheme } from "@/theme/provider";

/**
 * Sonner's toaster in Atlas's theme, at the bottom right. Its toasts are
 * unstyled: each one's body is a `Notice`. Sonner's motion and focus ring
 * are restyled in `styles/notices.css`.
 */
const Toaster = ({ className, ...props }: ToasterProps) => {
  const { theme } = useTheme();

  return (
    <Sonner
      theme={theme}
      position="bottom-right"
      className={cn("toaster group max-w-[calc(100vw-32px)]", className)}
      toastOptions={{ unstyled: true, classNames: { toast: "w-full" } }}
      {...props}
    />
  );
};

type NoticeProps = Omit<
  React.ComponentProps<"div">,
  "title" | "role" | "children"
> & {
  kind: "error" | "success";
  title: string;
  description: string;
  action?: { label: string; onClick: () => void } | null;
  onDismiss: () => void;
};

const icons = { error: OctagonXIcon, success: CircleCheckIcon };

/**
 * One notice: a floating surface with the state's icon, a short title, the
 * reason under it, an optional action, and Dismiss. The icon's shape and the
 * words carry the state, never a state hue. An error is announced as an
 * alert, a success as a status.
 */
function Notice({
  kind,
  title,
  description,
  action,
  onDismiss,
  className,
  ...props
}: NoticeProps) {
  const Icon = icons[kind];
  return (
    <div
      data-slot="notice"
      data-surface="raised"
      role={kind === "error" ? "alert" : "status"}
      className={cn(
        "w-full rounded-md bg-popover py-3.5 pr-3 pl-4 font-sans text-13 text-popover-foreground shadow-float-soft ring-1 ring-edge-strong",
        className,
      )}
      {...props}
    >
      <div className="flex items-center gap-2">
        <Icon aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
        <p className="min-w-0 flex-1 font-medium text-heading">{title}</p>
        <Button
          variant="ghost"
          size="icon"
          className="size-6"
          aria-label="Dismiss"
          onClick={onDismiss}
        >
          <XIcon aria-hidden />
        </Button>
      </div>
      <p className="mt-1 ml-5.5 text-muted-foreground wrap-anywhere">
        {description}
      </p>
      {action != null && (
        <div className="mt-1.5 ml-3">
          <Button
            variant="ghost"
            className="text-heading"
            onClick={action.onClick}
          >
            {action.label}
          </Button>
        </div>
      )}
    </div>
  );
}

export { Notice, Toaster };
