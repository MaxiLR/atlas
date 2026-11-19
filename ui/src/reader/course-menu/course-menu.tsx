import { CheckIcon, EllipsisIcon, FileDownIcon } from "lucide-react";
import { useEffect, useState } from "react";

import { useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Spinner } from "@/components/ui/spinner";
import { describeError } from "@/core/client";
import type { ExportResult } from "@/core/generated/ExportResult";
import { unknownTag } from "@/store/unknown-tag";

/** How the last export ended, when it is worth saying. */
type Outcome =
  { type: "saved"; file: string } | { type: "failed"; reason: string };

/** How long the saved file's name stays beside the menu. */
const SAVED_SHOWN_MS = 8_000;

function outcomeOf(result: ExportResult): Outcome | null {
  switch (result.type) {
    case "Saved":
      return { type: "saved", file: result.path.split(/[\\/]/).at(-1) ?? "" };
    case "Cancelled":
      return null;
    case "Failed":
      return { type: "failed", reason: result.reason };
    default:
      return unknownTag(result);
  }
}

/**
 * The course menu at the end of the header: Export PDF. While the export
 * runs the entry is busy; a saved PDF is announced by its file name beside
 * the menu, a cancelled one not at all, and a failure with its reason until
 * the menu is opened again.
 */
export function CourseMenu({ courseId }: { courseId: string }) {
  const { send } = useAtlasStore();
  const [exporting, setExporting] = useState(false);
  const [outcome, setOutcome] = useState<Outcome | null>(null);

  useEffect(() => {
    if (outcome?.type !== "saved") return;
    const timer = setTimeout(() => setOutcome(null), SAVED_SHOWN_MS);
    return () => clearTimeout(timer);
  }, [outcome]);

  async function exportPdf() {
    setExporting(true);
    setOutcome(null);
    try {
      const { result } = await send({ type: "ExportPdf", courseId });
      setOutcome(outcomeOf(result));
    } catch (error) {
      setOutcome({ type: "failed", reason: describeError(error) });
    } finally {
      setExporting(false);
    }
  }

  const failure =
    outcome?.type === "failed"
      ? `The PDF could not be exported. ${outcome.reason}`
      : null;
  return (
    <>
      <p
        role="status"
        className="flex min-w-0 items-center gap-1.5 text-13 text-muted-foreground"
      >
        {outcome?.type === "saved" && (
          <>
            <CheckIcon aria-hidden className="size-3.5 shrink-0" />
            <span className="max-w-64 truncate" title={outcome.file}>
              {`Saved ${outcome.file}`}
            </span>
          </>
        )}
      </p>
      {failure !== null && (
        <p
          role="alert"
          title={failure}
          className="max-w-80 min-w-0 truncate text-13 text-body"
        >
          {failure}
        </p>
      )}
      <DropdownMenu
        onOpenChange={(open) => {
          if (open && failure !== null) setOutcome(null);
        }}
      >
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon" aria-label="Course menu">
            <EllipsisIcon aria-hidden />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem
            disabled={exporting}
            aria-busy={exporting}
            onSelect={() => void exportPdf()}
          >
            {exporting ? <Spinner aria-hidden /> : <FileDownIcon aria-hidden />}
            {exporting ? "Exporting PDF…" : "Export PDF"}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  );
}
