import { ChevronLeftIcon, XIcon } from "lucide-react";
import { useCallback, useEffect, useId, useLayoutEffect, useRef } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button, buttonVariants } from "@/components/ui/button";
import type { CourseView } from "@/core/generated/CourseView";
import { cn } from "@/lib/utils";
import { CourseMenu } from "@/reader/course-menu/course-menu";
import { readCount } from "@/reader/course-tree";
import { ReaderFoot } from "@/reader/foot/foot";
import { GlossaryList } from "@/reader/glossary/glossary-list";
import { ReaderHeader } from "@/reader/header/header";
import { IndexRoute } from "@/reader/index-route/index-route";
import { useFocusLoop, useNarrow } from "@/reader/narrow";
import { SectionView } from "@/reader/section/section-view";
import { SideChats } from "@/reader/side-chat/side-chats";
import { SidePanel } from "@/reader/side-panel/side-panel";
import { UsageMeter } from "@/reader/usage-meter/usage-meter";
import { VersionSwitch } from "@/reader/versions/version-switch";
import type { PanelTab } from "@/store";

/**
 * A course in the reader: the rail (Library, the course title and read
 * count, the index, the usage meter), the reading column under its header
 * with the foot below it, and the side panel on demand.
 *
 * At 1024 px and wider the three sit side by side. Narrower, the rail folds
 * to 72 px of marks, the header's index button opens it at full width over
 * the column, and the side panel opens over the column too; while either is
 * open over the column, the rest is inert and Tab goes round inside it.
 * Escape closes the overlay or the panel and returns focus to the button
 * that opened it.
 *
 * A section that opens starts at the top of the column with focus on its
 * title, except that Next and Go deeper keep focus where it was pressed. The
 * rail's first stop skips to the column.
 */
export function Reader({ course }: { course: CourseView }) {
  const { send, navigate } = useAtlasStore();
  const { sectionId, panel, overlay } = useAtlas((state) => ({
    sectionId: state.navigation.sectionId,
    panel: state.navigation.panel,
    overlay: state.navigation.overlay,
  }));
  const indexOpen = overlay === "index";
  const railId = useId();
  const rail = useRef<HTMLElement>(null);
  const sidePanel = useRef<HTMLElement>(null);
  const indexToggle = useRef<HTMLButtonElement>(null);
  const panelOpener = useRef<HTMLButtonElement | null>(null);
  // Set when an action opens a region whose first control takes focus.
  const focusIndex = useRef(false);
  const focusPanel = useRef(false);
  const column = useRef<HTMLDivElement>(null);
  const columnId = useId();
  const narrow = useNarrow();
  const overIndex = narrow && indexOpen;
  const overPanel = narrow && panel.open && !indexOpen;
  const { read, total } = readCount(course);

  useFocusLoop(rail, overIndex);
  useFocusLoop(sidePanel, overPanel);

  /** The open section's title, or the column without one. */
  const columnTarget = useCallback(() => {
    const scroller = column.current;
    return scroller?.querySelector<HTMLElement>("article h1") ?? scroller;
  }, []);

  useEffect(() => {
    const scroller = column.current;
    if (scroller === null) return;
    scroller.scrollTop = 0;
    // The foot's controls keep focus, so Next can be pressed again; the
    // side panel keeps it while the candidate works there.
    const focused = document.activeElement;
    if (
      focused?.closest("main > footer") != null ||
      sidePanel.current?.contains(focused) === true
    ) {
      return;
    }
    columnTarget()?.focus();
  }, [sectionId, columnTarget]);

  // Where focus goes back once an overlay has closed: only after the render
  // that lifts `inert` from the rest, which focus cannot enter before. A
  // layout effect, so that a margin mark that opened the panel can still
  // take focus back in its own effect after this.
  const giveBack = useRef<HTMLElement | null>(null);

  const closeIndex = useCallback(
    (returnFocus: boolean) => {
      navigate({ type: "CloseOverlay" });
      if (returnFocus) giveBack.current = indexToggle.current;
    },
    [navigate],
  );
  const closePanel = useCallback(() => {
    navigate({ type: "ClosePanel" });
    giveBack.current = panelOpener.current;
  }, [navigate]);

  useLayoutEffect(() => {
    giveBack.current?.focus();
    giveBack.current = null;
  }, [indexOpen, panel.open]);

  useEffect(() => {
    if (!indexOpen || !focusIndex.current) return;
    focusIndex.current = false;
    const index = rail.current?.querySelector("nav");
    (
      index?.querySelector<HTMLElement>('[aria-current="page"]') ??
      index?.querySelector<HTMLElement>("button")
    )?.focus();
  }, [indexOpen]);

  useEffect(() => {
    if (!panel.open || !focusPanel.current) return;
    focusPanel.current = false;
    sidePanel.current
      ?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')
      ?.focus();
  }, [panel.open]);

  useEffect(() => {
    // A menu that handles Escape itself marks the event handled.
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (indexOpen) closeIndex(true);
      else if (panel.open) closePanel();
      else return;
      event.preventDefault();
    };
    // A press outside the titles overlay and its button closes it.
    const pressOutside = (event: PointerEvent) => {
      const target = event.target as Node;
      if (
        indexOpen &&
        !rail.current?.contains(target) &&
        !indexToggle.current?.contains(target)
      ) {
        closeIndex(false);
      }
    };
    document.addEventListener("keydown", escape);
    document.addEventListener("pointerdown", pressOutside);
    return () => {
      document.removeEventListener("keydown", escape);
      document.removeEventListener("pointerdown", pressOutside);
    };
  }, [indexOpen, panel.open, closeIndex, closePanel]);

  function togglePanel(tab: PanelTab, button: HTMLButtonElement) {
    if (panel.open && panel.tab === tab) {
      navigate({ type: "ClosePanel" });
      return;
    }
    panelOpener.current = button;
    focusPanel.current = !panel.open;
    if (indexOpen) navigate({ type: "CloseOverlay" });
    navigate({ type: "OpenPanel", tab });
  }

  function openSection(id: string) {
    if (indexOpen) navigate({ type: "CloseOverlay" });
    send({ type: "OpenSection", courseId: course.id, sectionId: id }).catch(
      failureInStore,
    );
  }

  const folded = !indexOpen;
  const hiddenFolded = folded ? "max-lg:hidden" : undefined;

  return (
    <div
      className={cn(
        "grid h-svh grid-cols-[72px_minmax(0,1fr)] overflow-hidden bg-canvas lg:grid-cols-[320px_minmax(0,1fr)]",
        panel.open && "lg:grid-cols-[320px_minmax(0,1fr)_400px]",
      )}
    >
      <aside
        ref={rail}
        id={railId}
        aria-label="Course"
        inert={overPanel}
        className={cn(
          "col-start-1 row-start-1 flex min-h-0 min-w-0 flex-col border-r border-edge bg-panel",
          indexOpen &&
            "max-lg:fixed max-lg:inset-y-0 max-lg:left-0 max-lg:z-30 max-lg:w-80 max-lg:shadow-float",
        )}
      >
        <a
          href={`#${columnId}`}
          className={cn(
            buttonVariants({ size: "lg" }),
            "pointer-events-none fixed top-1.5 left-20 z-40 bg-panel opacity-0 shadow-float focus:pointer-events-auto focus:opacity-100",
          )}
          onClick={(event) => {
            event.preventDefault();
            // Over the column, the overlay keeps it inert until it closes.
            if (indexOpen) {
              navigate({ type: "CloseOverlay" });
              giveBack.current = columnTarget();
            } else {
              columnTarget()?.focus();
            }
          }}
        >
          Skip to the reading column
        </a>
        <div
          data-tauri-drag-region
          className={cn(
            "flex h-11 shrink-0 items-center gap-1.5 pr-3 pl-20",
            folded && "max-lg:pl-0",
          )}
        >
          <Button
            variant="ghost"
            className={hiddenFolded}
            onClick={() => navigate({ type: "ShowLibrary" })}
          >
            <ChevronLeftIcon aria-hidden />
            Library
          </Button>
          {indexOpen && (
            <Button
              variant="ghost"
              size="icon"
              className="ml-auto lg:hidden"
              aria-label="Hide the index"
              onClick={() => closeIndex(true)}
            >
              <XIcon aria-hidden />
            </Button>
          )}
        </div>
        <div
          className={cn(
            "shrink-0 border-b border-edge px-5 pt-2.5 pb-3.5",
            hiddenFolded,
          )}
        >
          <h2 className="truncate text-15 font-medium text-heading">
            {course.topic}
          </h2>
          <p className="text-13 text-muted-foreground tabular-nums">
            {read} of {total} sections read
          </p>
        </div>
        <IndexRoute
          course={course}
          currentId={sectionId}
          folded={folded}
          onOpen={openSection}
        />
        <UsageMeter folded={folded} />
      </aside>

      <main
        inert={overIndex || overPanel}
        className="col-start-2 row-start-1 grid min-h-0 min-w-0 grid-rows-[44px_minmax(0,1fr)_64px]"
      >
        <ReaderHeader
          course={course}
          sectionId={sectionId}
          indexOpen={indexOpen}
          indexToggleRef={indexToggle}
          railId={railId}
          onToggleIndex={() => {
            if (indexOpen) {
              closeIndex(false);
            } else {
              focusIndex.current = true;
              navigate({ type: "OpenOverlay", overlay: "index" });
            }
          }}
          panel={panel}
          onPanelButton={togglePanel}
          versions={
            sectionId !== null && (
              <VersionSwitch courseId={course.id} sectionId={sectionId} />
            )
          }
          menu={<CourseMenu courseId={course.id} />}
        />
        <div
          ref={column}
          id={columnId}
          tabIndex={-1}
          className="min-h-0 overflow-y-auto outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
        >
          {sectionId === null ? (
            <p className="mx-auto max-w-180 px-5 pt-14 text-16 text-muted-foreground max-lg:px-7 max-lg:pt-10">
              Choose a section in the index.
            </p>
          ) : (
            <SectionView
              key={sectionId}
              course={course}
              sectionId={sectionId}
            />
          )}
        </div>
        <ReaderFoot courseId={course.id} sectionId={sectionId} />
      </main>

      {panel.open && (
        <SidePanel
          ref={sidePanel}
          inert={overIndex}
          className="col-start-3 row-start-1"
          course={course}
          tab={panel.tab}
          onClose={closePanel}
          sideChats={<SideChats courseId={course.id} />}
          glossary={<GlossaryList courseId={course.id} />}
        />
      )}
    </div>
  );
}
