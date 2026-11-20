import { useEffect, useId, useRef, useState, type RefObject } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { Button } from "@/components/ui/button";
import { describeError } from "@/core/client";
import type { CourseView } from "@/core/generated/CourseView";
import type { SectionNode } from "@/core/generated/SectionNode";
import { cn } from "@/lib/utils";
import { placeOf } from "@/reader/course-tree";
import { selectSection } from "@/store";

/**
 * The section Next leads to, as the index shows it: inside a deep dive the
 * next sibling, otherwise (and after the last sibling) the broad-pass
 * section after the branch's root. Null at the end of the course.
 */
function nextOnTheLine(
  course: CourseView,
  trail: SectionNode[],
): SectionNode | null {
  const section = trail.at(-1);
  const parent = trail.at(-2);
  if (section !== undefined && parent !== undefined) {
    const at = parent.deepDives.findIndex((dive) => dive.id === section.id);
    const sibling = parent.deepDives[at + 1];
    if (sibling !== undefined) return sibling;
  }
  const broadPass = course.chapters.flatMap((chapter) => chapter.sections);
  const root = broadPass.findIndex((each) => each.id === trail[0]?.id);
  return root === -1 ? null : (broadPass[root + 1] ?? null);
}

/** Where the branch stands under the current section. */
type Branch =
  | { type: "none" }
  | { type: "requesting" }
  | { type: "failed"; error: unknown }
  | { type: "present"; deepDives: SectionNode[] };

/** The second line of the foot, for a branch that is not failed. */
function branchText(branch: Branch): string {
  switch (branch.type) {
    case "requesting":
      return "Or branch here: proposing deep dives…";
    case "present": {
      const total = branch.deepDives.length;
      const unread = branch.deepDives.filter((dive) => !dive.read).length;
      const noun = total === 1 ? "deep dive" : "deep dives";
      return unread === 0
        ? `Or branch here: ${total === 1 ? "its" : `all ${total}`} ${noun} read`
        : `Or branch here: ${unread} of ${total} ${noun} unread`;
    }
    default:
      return "Or branch here: no deep dives yet";
  }
}

/** Expo ease-out, the reader's one curve for things that arrive. */
const ARRIVE = "cubic-bezier(0.16, 1, 0.3, 1)";

/** Where the deep dives sit on the small line: one step apart, after the curve. */
const DIVE_STEP = 12;
const CURVE = "M8 14C30 14 26 32 48 32";

/**
 * What the branch is drawing: its curve while the deep dives are asked for,
 * then the proposals as they arrive.
 */
type Drawing = "requesting" | "arrived" | null;

/**
 * Draws the branch whenever `drawing` changes: the curve from the current
 * node while the deep dives are asked for; once they arrive, the line on
 * from the curve's end and each deep dive's node in turn. Not at all under
 * reduced motion, where the branch is simply there.
 */
function useDrawBranch(group: RefObject<SVGGElement | null>, drawing: Drawing) {
  useEffect(() => {
    const element = group.current;
    if (drawing === null || element === null) return;
    if (typeof element.animate !== "function") return;
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const path = element.querySelector("path");
    const nodes = [...element.querySelectorAll("circle")];
    // The stroke runs over a length of 1; the curve was drawn already.
    const hidden =
      drawing === "arrived" && path !== null
        ? (DIVE_STEP * nodes.length) / path.getTotalLength()
        : 1;
    const running = [
      ...(path === null
        ? []
        : [
            path.animate(
              [
                { strokeDasharray: "1", strokeDashoffset: hidden },
                { strokeDasharray: "1", strokeDashoffset: 0 },
              ],
              { duration: 250, easing: ARRIVE },
            ),
          ]),
      ...nodes.map((node, index) =>
        node.animate([{ opacity: 0 }, { opacity: 1 }], {
          duration: 150,
          delay: 100 + index * 50,
          easing: ARRIVE,
          fill: "backwards",
        }),
      ),
    ];
    return () => {
      for (const animation of running) animation.cancel();
    };
  }, [group, drawing]);
}

/**
 * The small drawn line: the current node, the line on to the next node and,
 * when it is asked for or exists, the branch with a node per deep dive.
 * Hidden below 1024 px, and in a foot too narrow to hold it beside the texts
 * and the controls (the side panel open beside a narrow column).
 */
function SmallLine({
  next,
  branch,
}: {
  next: SectionNode | null;
  branch: Branch;
}) {
  const group = useRef<SVGGElement>(null);
  const deepDives = branch.type === "present" ? branch.deepDives : [];
  // Deep dives already there when the line is shown are not drawn again.
  const [drawing, setDrawing] = useState<Drawing>(null);
  const phase: Drawing =
    branch.type === "requesting"
      ? "requesting"
      : branch.type === "present" && drawing !== null
        ? "arrived"
        : null;
  if (phase !== drawing) setDrawing(phase);
  useDrawBranch(group, drawing);

  const end = 48 + DIVE_STEP * deepDives.length;
  return (
    <svg
      viewBox="0 0 180 40"
      aria-hidden
      className="h-10 w-45 shrink-0 fill-none max-lg:hidden @max-[32rem]:hidden"
    >
      {/* The line runs on to the next node, or stops at a bar where the
          course ends. */}
      <path
        d={next === null ? "M8 14H40M40 9V19" : "M8 14H172"}
        className="stroke-faint"
      />
      {branch.type !== "none" && branch.type !== "failed" && (
        <g ref={group}>
          <path
            d={deepDives.length === 0 ? CURVE : `${CURVE}H${end}`}
            pathLength={1}
            className={cn(
              "stroke-muted-foreground",
              branch.type === "requesting" && "motion-safe:animate-pulse",
            )}
          />
          {deepDives.map((dive, index) => (
            <circle
              key={dive.id}
              cx={48 + DIVE_STEP * (index + 1)}
              cy={32}
              r={3}
              className={dive.read ? "fill-body" : "fill-canvas stroke-faint"}
            />
          ))}
        </g>
      )}
      <circle
        cx={8}
        cy={14}
        r={5.5}
        strokeWidth={1.5}
        className="fill-canvas stroke-atlas-accent"
      />
      <circle cx={8} cy={14} r={2.5} className="fill-atlas-accent" />
      {next !== null && (
        <circle
          cx={172}
          cy={14}
          r={3}
          className={next.read ? "fill-body" : "fill-canvas stroke-faint"}
        />
      )}
    </svg>
  );
}

/**
 * A control that cannot act right now keeps its place and its focus: it is
 * marked unavailable rather than disabled, and looks as a disabled one does.
 */
const unavailable = {
  default:
    "aria-disabled:cursor-not-allowed aria-disabled:border-transparent aria-disabled:text-muted-foreground aria-disabled:hover:bg-wash aria-disabled:hover:text-muted-foreground aria-disabled:active:scale-100",
  primary:
    "aria-disabled:cursor-not-allowed aria-disabled:bg-wash-strong aria-disabled:text-muted-foreground aria-disabled:hover:bg-wash-strong aria-disabled:active:scale-100",
};

/** A request of the foot the core refused, kept for the section it was for. */
interface Refusal {
  type: "Next" | "GoDeeper";
  sectionId: string;
  error: unknown;
}

/**
 * The foot of the reading column, under the open section: the small drawn
 * line with the current node, the next node and the branch; the next section
 * on the line and the state of the branch; Go deeper; and Next as the
 * primary action at the column's right edge, or the end of the course.
 *
 * Next is unavailable while the section is generating, Go deeper until it is
 * complete. Go deeper asks for the deep dives, then opens the first unread
 * one. A refused request says so, and its control tries again.
 */
export function ReaderFoot({
  courseId,
  sectionId,
}: {
  courseId: string;
  sectionId: string | null;
}) {
  const { send } = useAtlasStore();
  const { course, status, nextFrom, ended } = useAtlas((state) => ({
    course: state.course.byId[courseId] ?? null,
    status:
      sectionId === null
        ? null
        : (selectSection(state, sectionId)?.status ?? null),
    nextFrom: state.navigation.nextFrom,
    ended: state.navigation.endOfCourse,
  }));
  // Sections whose deep dives are being asked for; several may be at once.
  const [requesting, setRequesting] = useState<readonly string[]>([]);
  const [refusal, setRefusal] = useState<Refusal | null>(null);
  const nextId = useId();
  const branchId = useId();

  const trail =
    course === null || sectionId === null
      ? null
      : (placeOf(course, sectionId)?.trail ?? null);
  const section = trail?.at(-1);
  if (course === null || sectionId === null || !trail || !section) {
    return <footer className="border-t border-edge bg-canvas px-5" />;
  }

  const next = ended ? null : nextOnTheLine(course, trail);
  const refused = refusal?.sectionId === sectionId ? refusal : null;
  const branch: Branch =
    refused?.type === "GoDeeper"
      ? { type: "failed", error: refused.error }
      : requesting.includes(sectionId)
        ? { type: "requesting" }
        : section.deepDives.length > 0
          ? { type: "present", deepDives: section.deepDives }
          : { type: "none" };
  const ids = { courseId, sectionId };
  const generating = status === null || status === "streaming";
  const asking = nextFrom === sectionId;
  const nextUnavailable = generating || asking;
  const deeperUnavailable = status !== "stored" || branch.type === "requesting";

  const goNext = () => {
    if (nextUnavailable) return;
    setRefusal(null);
    send({ type: "Next", ...ids }).catch((error: unknown) =>
      setRefusal({ type: "Next", sectionId, error }),
    );
  };

  const goDeeper = () => {
    if (deeperUnavailable) return;
    if (branch.type === "present") {
      const target =
        branch.deepDives.find((dive) => !dive.read) ?? branch.deepDives[0]!;
      send({ type: "OpenSection", courseId, sectionId: target.id }).catch(
        failureInStore,
      );
      return;
    }
    setRefusal(null);
    setRequesting((asked) => [...asked, sectionId]);
    send({ type: "GoDeeper", ...ids })
      .catch((error: unknown) =>
        setRefusal({ type: "GoDeeper", sectionId, error }),
      )
      .finally(() =>
        setRequesting((asked) => asked.filter((id) => id !== sectionId)),
      );
  };

  const nextLine =
    next === null ? "The course ends here." : `Next on the line: ${next.title}`;
  const refusalLine =
    refused === null
      ? null
      : `${refused.type === "Next" ? "The next section could not be found." : "Deep dives could not be proposed."} ${describeError(refused.error)}`;

  return (
    <footer className="@container border-t border-edge bg-canvas px-5">
      <div className="mx-auto flex h-full max-w-170 items-center gap-3 text-13 text-muted-foreground">
        <SmallLine key={sectionId} next={next} branch={branch} />
        <div className="grid min-w-0 flex-1 gap-px">
          <span
            id={nextId}
            title={nextLine}
            className="truncate font-medium text-heading"
          >
            {nextLine}
          </span>
          {refusalLine === null ? (
            <span
              id={branchId}
              role="status"
              title={branchText(branch)}
              className="truncate"
            >
              {branchText(branch)}
            </span>
          ) : (
            <span
              id={branchId}
              role="alert"
              title={refusalLine}
              className="truncate text-body"
            >
              {refusalLine}
            </span>
          )}
        </div>
        <Button
          aria-describedby={branchId}
          aria-disabled={deeperUnavailable || undefined}
          aria-busy={branch.type === "requesting" || undefined}
          className={unavailable.default}
          onClick={goDeeper}
        >
          {refused?.type === "GoDeeper" ? "Try again" : "Go deeper"}
        </Button>
        {next !== null && (
          <Button
            variant="primary"
            aria-describedby={refused?.type === "Next" ? branchId : nextId}
            aria-disabled={nextUnavailable || undefined}
            aria-busy={asking || undefined}
            className={unavailable.primary}
            onClick={goNext}
          >
            {refused?.type === "Next" ? "Try again" : "Next"}
          </Button>
        )}
      </div>
    </footer>
  );
}
