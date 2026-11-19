import { ChevronDownIcon } from "lucide-react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { VersionStyle } from "@/core/generated/VersionStyle";
import { selectSection } from "@/store";

const styleNames: Record<VersionStyle, string> = {
  Original: "Original",
  Simpler: "Simpler",
  MoreTechnical: "More technical",
  MoreExamples: "More examples",
};

/**
 * The version switch: nothing while a section has one version; with more,
 * the current version and its position, and a menu of them, oldest first,
 * that shows the chosen one. A generation that runs, stopped or failed
 * leaves the current version as it was; while one runs the switch waits.
 */
export function VersionSwitch({
  courseId,
  sectionId,
}: {
  courseId: string;
  sectionId: string;
}) {
  const { send } = useAtlasStore();
  const display = useAtlas((state) => selectSection(state, sectionId));
  const currentId = useAtlas(
    (state) => state.sections.byId[sectionId]?.currentVersionId ?? null,
  );
  if (display === null || display.versions.length < 2) return null;

  const { versions } = display;
  const at = versions.findIndex((version) => version.id === currentId);
  if (at === -1) return null;
  const name = styleNames[versions[at].style];
  const position = `${at + 1} of ${versions.length}`;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild disabled={display.status === "streaming"}>
        <Button aria-label={`Version: ${name}, ${position}`}>
          {/* The header's container: a column the side panel narrows. */}
          <span className="max-lg:hidden @max-xl:hidden">{name}</span>
          <em className="font-normal text-muted-foreground not-italic tabular-nums">
            {position}
          </em>
          <ChevronDownIcon aria-hidden />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup
          value={versions[at].id}
          onValueChange={(chosen) =>
            send({
              type: "SelectVersion",
              courseId,
              sectionId,
              versionId: chosen,
            }).catch(failureInStore)
          }
        >
          {versions.map((version, number) => (
            <DropdownMenuRadioItem key={version.id} value={version.id}>
              {styleNames[version.style]}
              <span className="text-muted-foreground tabular-nums">
                {number + 1}
              </span>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
