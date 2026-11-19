import {
  BookAIcon,
  MessagesSquareIcon,
  TextAlignStartIcon,
} from "lucide-react";
import type { ReactNode, Ref } from "react";

import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Button } from "@/components/ui/button";
import type { CourseView } from "@/core/generated/CourseView";
import { placeOf } from "@/reader/course-tree";
import type { PanelTab } from "@/store";

/** A count beside a button's name, in tabular figures. */
function Count({ children }: { children: number }) {
  return (
    <em className="font-normal text-muted-foreground not-italic tabular-nums">
      {children}
    </em>
  );
}

/**
 * The reader's header: the breadcrumb on the left; on the right the version
 * switch, Glossary and Side chats with their counts, which open the side
 * panel on their tab, and the course menu. Below 1024 px a button at its
 * start opens the index titles over the column.
 *
 * When the side panel narrows the column below 576 px, Glossary and Side
 * chats show an icon and their count, their names kept for assistive
 * technology, so every control stays in the column beside the panel.
 */
export function ReaderHeader({
  course,
  sectionId,
  indexOpen,
  indexToggleRef,
  railId,
  onToggleIndex,
  panel,
  onPanelButton,
  versions,
  menu,
}: {
  course: CourseView;
  sectionId: string | null;
  indexOpen: boolean;
  indexToggleRef: Ref<HTMLButtonElement>;
  railId: string;
  onToggleIndex: () => void;
  panel: { open: boolean; tab: PanelTab };
  onPanelButton: (tab: PanelTab, button: HTMLButtonElement) => void;
  versions: ReactNode;
  menu: ReactNode;
}) {
  const place = sectionId === null ? null : placeOf(course, sectionId);
  const ancestors =
    place === null
      ? []
      : [place.chapter.title, ...place.trail.slice(0, -1).map((s) => s.title)];
  const tab = panel.open ? panel.tab : null;
  // The chapter and parent sections give way to the open section's title in
  // a narrow window or a column narrowed by the side panel.
  const upward = "max-lg:hidden @max-xl:hidden";
  // In a column narrowed by the side panel a button's name gives way to its
  // icon.
  const name = "@max-xl:sr-only";
  const icon = "@xl:hidden";

  return (
    <header
      data-tauri-drag-region
      className="@container flex min-w-0 items-center gap-1.5 border-b border-edge pr-3 pl-5 text-muted-foreground max-lg:pl-2"
    >
      <Button
        ref={indexToggleRef}
        variant="ghost"
        size="icon"
        className="lg:hidden"
        aria-label="Show the index"
        aria-expanded={indexOpen}
        aria-controls={railId}
        onClick={onToggleIndex}
      >
        <TextAlignStartIcon aria-hidden />
      </Button>
      {place !== null && (
        <Breadcrumb className="min-w-0">
          <BreadcrumbList>
            {ancestors.flatMap((title, at) => [
              <BreadcrumbItem key={`item-${at}`} className={upward}>
                <span className="truncate">{title}</span>
              </BreadcrumbItem>,
              <BreadcrumbSeparator
                key={`separator-${at}`}
                className={upward}
              />,
            ])}
            <BreadcrumbItem>
              <BreadcrumbPage>{place.trail.at(-1)?.title}</BreadcrumbPage>
            </BreadcrumbItem>
          </BreadcrumbList>
        </Breadcrumb>
      )}
      <span data-tauri-drag-region className="flex-1 self-stretch" />
      {versions}
      <Button
        aria-pressed={tab === "glossary"}
        onClick={(event) => onPanelButton("glossary", event.currentTarget)}
      >
        <BookAIcon aria-hidden className={icon} />
        <span className={name}>Glossary</span>{" "}
        <Count>{course.glossaryCount}</Count>
      </Button>
      <Button
        aria-pressed={tab === "sideChats"}
        onClick={(event) => onPanelButton("sideChats", event.currentTarget)}
      >
        <MessagesSquareIcon aria-hidden className={icon} />
        <span className={name}>Side chats</span>{" "}
        <Count>{course.sideChatCount}</Count>
      </Button>
      {menu}
    </header>
  );
}
