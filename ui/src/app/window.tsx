import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

/**
 * A 44 px strip the window is dragged by. Its first 80 px stay empty: macOS
 * draws the window controls over them.
 */
export function WindowBar({
  className,
  children,
}: {
  className?: string;
  children?: ReactNode;
}) {
  return (
    <div
      data-tauri-drag-region
      className={cn(
        "flex h-11 shrink-0 items-center gap-3 pr-3 pl-20 text-13 text-muted-foreground",
        className,
      )}
    >
      {children}
    </div>
  );
}

/**
 * A screen outside a course: the window bar over one centred column, 760 px
 * for lists and 640 px for a single task. The bar stays put; the column
 * scrolls under it.
 */
export function WindowFrame({
  bar,
  width,
  children,
}: {
  bar?: ReactNode;
  width: "list" | "task";
  children: ReactNode;
}) {
  return (
    <div className="grid h-svh grid-rows-[auto_minmax(0,1fr)]">
      <header className="border-b border-edge">
        <WindowBar>{bar}</WindowBar>
      </header>
      <main
        className={cn(
          "min-w-0 overflow-y-auto px-5 pt-10 pb-8 min-[761px]:px-6 min-[761px]:pb-16",
          width === "list" ? "min-[761px]:pt-20" : "min-[761px]:pt-24",
        )}
      >
        <div
          className={cn(
            "mx-auto flex flex-col gap-8",
            width === "list" ? "max-w-[760px]" : "max-w-[640px]",
          )}
        >
          {children}
        </div>
      </main>
    </div>
  );
}

/**
 * The bar's name for where the window is; the page's heading when the
 * column has none of its own.
 */
export function WindowTitle({
  heading = false,
  children,
}: {
  heading?: boolean;
  children: ReactNode;
}) {
  const Name = heading ? "h1" : "span";
  return (
    <Name
      data-tauri-drag-region
      className="min-w-0 truncate text-13 font-medium text-heading"
    >
      {children}
    </Name>
  );
}
