import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import { selectSection } from "@/store";

/**
 * The foot of the reading column: Go deeper and Next for the open section,
 * and the end of the course once Next finds none.
 */
export function ReaderFoot({
  courseId,
  sectionId,
}: {
  courseId: string;
  sectionId: string | null;
}) {
  const { send } = useAtlasStore();
  const ended = useAtlas((state) => state.navigation.endOfCourse);
  const generating = useAtlas((state) => {
    if (sectionId === null) return false;
    const display = selectSection(state, sectionId);
    return display === null || display.status === "streaming";
  });

  return (
    <footer className="border-t border-edge bg-canvas px-5">
      <div className="mx-auto flex h-full max-w-170 items-center gap-3">
        <p
          role="status"
          className="min-w-0 flex-1 truncate text-muted-foreground"
        >
          {ended ? "This is the end of the course." : ""}
        </p>
        {sectionId !== null && (
          <>
            <Button
              onClick={() =>
                send({ type: "GoDeeper", courseId, sectionId }).catch(
                  failureInStore,
                )
              }
            >
              Go deeper
            </Button>
            <Button
              variant="primary"
              disabled={generating}
              onClick={() =>
                send({ type: "Next", courseId, sectionId }).catch(
                  failureInStore,
                )
              }
            >
              Next
            </Button>
          </>
        )}
      </div>
    </footer>
  );
}
