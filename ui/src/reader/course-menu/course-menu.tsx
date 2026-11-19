import { EllipsisIcon, FileDownIcon } from "lucide-react";

import { failureInStore, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

/** The course menu at the end of the header: Export PDF. */
export function CourseMenu({ courseId }: { courseId: string }) {
  const { send } = useAtlasStore();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" aria-label="Course menu">
          <EllipsisIcon aria-hidden />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem
          onSelect={() =>
            send({ type: "ExportPdf", courseId }).catch(failureInStore)
          }
        >
          <FileDownIcon aria-hidden />
          Export PDF
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
