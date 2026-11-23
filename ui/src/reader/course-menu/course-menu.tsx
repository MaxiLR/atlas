import { EllipsisIcon, FileDownIcon } from "lucide-react";
import { useState } from "react";

import { failureInStore, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Spinner } from "@/components/ui/spinner";

/**
 * The course menu at the end of the header: Export PDF. While the export
 * runs the entry is busy; how it ended is the store's notice.
 */
export function CourseMenu({ courseId }: { courseId: string }) {
  const { send } = useAtlasStore();
  const [exporting, setExporting] = useState(false);

  async function exportPdf() {
    setExporting(true);
    await send({ type: "ExportPdf", courseId }).catch(failureInStore);
    setExporting(false);
  }

  return (
    <DropdownMenu>
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
  );
}
