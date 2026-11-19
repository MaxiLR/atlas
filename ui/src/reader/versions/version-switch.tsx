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
 * The version switch: nothing while a section has one version; with more, a
 * menu of them, oldest first, that shows the chosen one.
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
  if (display === null || display.versions.length < 2) return null;

  const { versions, versionId } = display;
  const at = versions.findIndex((version) => version.id === versionId);
  const name = at === -1 ? "Not saved" : styleNames[versions[at].style];
  const position =
    at === -1 ? `${versions.length} saved` : `${at + 1} of ${versions.length}`;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
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
          value={versionId ?? ""}
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
