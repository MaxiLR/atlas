import { useId } from "react";

import { failureInStore, useAtlas, useAtlasStore } from "@/app/store";
import { CitationScope } from "@/catalog/components/citation/citation";
import type { CourseView } from "@/core/generated/CourseView";
import { placeOf } from "@/reader/course-tree";
import { SelectableBlocks } from "@/reader/selection/selectable-blocks";
import { selectSection } from "@/store";

import { Explain } from "./explain/explain";
import { SectionStatus } from "./status/section-status";
import { Sources } from "./sources/sources";

/**
 * The open section in the reading column: its title, the one-line summary
 * once the core has answered with the section, where it stands, its blocks,
 * its sources and Explain differently, in a 680 px measure.
 */
export function SectionView({
  course,
  sectionId,
}: {
  course: CourseView;
  sectionId: string;
}) {
  const { send } = useAtlasStore();
  const display = useAtlas((state) => selectSection(state, sectionId));
  const summary = useAtlas(
    (state) => state.sections.byId[sectionId]?.view?.summary,
  );
  const titleId = useId();
  const node = placeOf(course, sectionId)?.trail.at(-1);
  const generating = display === null || display.status === "streaming";
  const ids = { courseId: course.id, sectionId };

  return (
    <article
      aria-labelledby={titleId}
      aria-busy={generating}
      className="mx-auto max-w-180 px-5 pt-14 pb-24 max-lg:px-7 max-lg:pt-10 max-lg:pb-18"
    >
      <header className="mb-7">
        <h1
          id={titleId}
          className="text-28 font-medium text-balance text-heading"
        >
          {node?.title}
        </h1>
        {summary !== undefined && (
          <p className="mt-2 text-16 text-muted-foreground">{summary}</p>
        )}
      </header>
      <SectionStatus courseId={course.id} sectionId={sectionId} />
      {display !== null && (
        <CitationScope>
          <SelectableBlocks
            courseId={course.id}
            versionId={display.versionId}
            blocks={display.blocks}
            versionId={display.versionId}
            citations={display.citations}
            sources={display.sources}
          />
          <Sources sources={display.sources} />
        </CitationScope>
      )}
      {display?.status === "stored" && (
        <Explain
          onExplain={(style) =>
            send({ type: "ExplainDifferently", ...ids, style }).catch(
              failureInStore,
            )
          }
        />
      )}
    </article>
  );
}
