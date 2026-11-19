import { PlusIcon, XIcon } from "lucide-react";
import type { ReactNode, Ref } from "react";

import { failureInStore, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { CourseView } from "@/core/generated/CourseView";
import { cn } from "@/lib/utils";
import type { PanelTab } from "@/store";

/**
 * The side panel at the right of the reading column: the Side chats and
 * Glossary tabs, New side chat and Close. Below 1024 px it opens over the
 * column, at most 420 px wide.
 */
export function SidePanel({
  ref,
  inert,
  course,
  tab,
  onClose,
  sideChats,
  glossary,
  className,
}: {
  ref?: Ref<HTMLElement>;
  /** Under the titles overlay, the panel takes no focus or input. */
  inert?: boolean;
  course: CourseView;
  tab: PanelTab;
  onClose: () => void;
  sideChats: ReactNode;
  glossary: ReactNode;
  className?: string;
}) {
  const { send, navigate } = useAtlasStore();
  return (
    <aside
      ref={ref}
      inert={inert}
      aria-label="Side panel"
      className={cn(
        "flex min-h-0 flex-col border-l border-edge bg-panel max-lg:fixed max-lg:inset-y-0 max-lg:right-0 max-lg:z-30 max-lg:w-[min(420px,calc(100%-72px))] max-lg:shadow-float",
        className,
      )}
    >
      <Tabs
        value={tab}
        onValueChange={(value) =>
          navigate({ type: "OpenPanel", tab: value as PanelTab })
        }
        className="min-h-0 flex-1 gap-0"
      >
        <div className="flex h-11 shrink-0 items-center gap-0.5 border-b border-edge px-2">
          <TabsList>
            <TabsTrigger value="sideChats">
              Side chats <em>{course.sideChatCount}</em>
            </TabsTrigger>
            <TabsTrigger value="glossary">
              Glossary <em>{course.glossaryCount}</em>
            </TabsTrigger>
          </TabsList>
          <span className="flex-1" />
          <Button
            variant="ghost"
            size="icon"
            aria-label="New side chat"
            onClick={() => {
              navigate({ type: "OpenPanel", tab: "sideChats" });
              send({
                type: "StartSideChat",
                courseId: course.id,
                anchor: null,
              }).catch(failureInStore);
            }}
          >
            <PlusIcon aria-hidden />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            aria-label="Close the side panel"
            onClick={onClose}
          >
            <XIcon aria-hidden />
          </Button>
        </div>
        <TabsContent value="sideChats" className="flex min-h-0 flex-col">
          {sideChats}
        </TabsContent>
        <TabsContent value="glossary" className="min-h-0 overflow-y-auto">
          {glossary}
        </TabsContent>
      </Tabs>
    </aside>
  );
}
