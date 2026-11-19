import { describe, expect, it } from "vitest";

import type { Anchor } from "@/core/generated/Anchor";
import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { SectionView } from "@/core/generated/SectionView";
import type { VersionStyle } from "@/core/generated/VersionStyle";

import {
  applyEvent,
  applyResponse,
  initialState,
  selectAnchors,
  selectSection,
  type AtlasState,
} from "./index";

const courseId = "course-1";
const at = "2026-11-19T10:00:00Z";

function section(
  sectionId: string,
  current: { id: string; style: VersionStyle; text: string },
  versions: { id: string; style: VersionStyle }[],
): SectionView {
  return {
    id: sectionId,
    courseId,
    title: `Section ${sectionId}`,
    summary: "One line.",
    version: {
      id: current.id,
      style: current.style,
      blocks: [{ index: 0, content: { type: "Prose", text: current.text } }],
      citations: [],
      sources: [],
      writtenWithoutSearch: false,
      model: null,
      createdAt: at,
    },
    versions: versions.map((version) => ({ ...version, createdAt: at })),
  };
}

const original = { id: "v-original", style: "Original" as const };
const simpler = { id: "v-simpler", style: "Simpler" as const };

/** Section a with two versions (Simpler current), section b with one. */
function twoSections(): AtlasState {
  let state = applyResponse(
    initialState,
    { type: "OpenSection", courseId, sectionId: "a" },
    {
      type: "Section",
      section: section("a", { ...original, text: "Original a." }, [original]),
    },
  );
  state = applyResponse(
    state,
    {
      type: "ExplainDifferently",
      courseId,
      sectionId: "a",
      style: "Simpler",
    },
    {
      type: "Section",
      section: section("a", { ...simpler, text: "Simpler a." }, [
        original,
        simpler,
      ]),
    },
  );
  return applyResponse(
    state,
    { type: "OpenSection", courseId, sectionId: "b" },
    {
      type: "Section",
      section: section(
        "b",
        { id: "v-b", style: "Original", text: "Original b." },
        [{ id: "v-b", style: "Original" }],
      ),
    },
  );
}

function selectVersion(state: AtlasState, versionId: string): AtlasState {
  const view = state.sections.byId["a"]!.view!;
  const text = versionId === original.id ? "Original a." : "Simpler a.";
  const style = versionId === original.id ? "Original" : "Simpler";
  return applyResponse(
    state,
    { type: "SelectVersion", courseId, sectionId: "a", versionId },
    {
      type: "Section",
      section: {
        ...section("a", { id: versionId, style, text }, []),
        versions: view.versions,
      },
    },
  );
}

function anchor(versionId: string, quote: string): Anchor {
  return { versionId, block: 0, quote };
}

function defined(term: string, on: Anchor): CoreEvent {
  return {
    type: "GlossaryChanged",
    courseId,
    entry: {
      id: `entry-${term}`,
      courseId,
      sectionId: "a",
      sectionTitle: "Section a",
      term,
      definition: `What ${term} means.`,
      anchor: on,
      wikipedia: null,
      createdAt: at,
    },
  };
}

function asked(chatId: string, on: Anchor | null): CoreEvent {
  return {
    type: "SideChatsChanged",
    courseId,
    chat: {
      id: chatId,
      courseId,
      anchor: on,
      sectionTitle: on === null ? null : "Section a",
      opening: on?.quote ?? "A question.",
      createdAt: at,
      updatedAt: at,
    },
  };
}

describe("selecting a version", () => {
  it("changes the current version of that section only", () => {
    const before = twoSections();
    expect(selectSection(before, "a")?.versionId).toBe(simpler.id);

    const after = selectVersion(before, original.id);

    const shown = selectSection(after, "a");
    expect(shown?.versionId).toBe(original.id);
    expect(shown?.blocks).toEqual([
      {
        state: "complete",
        index: 0,
        content: { type: "Prose", text: "Original a." },
      },
    ]);
    expect(shown?.versions.map((version) => version.id)).toEqual([
      original.id,
      simpler.id,
    ]);
    expect(after.sections.byId["b"]).toBe(before.sections.byId["b"]);
    expect(selectSection(after, "b")?.versionId).toBe("v-b");
  });

  it("selects anchors per version", () => {
    const state = [
      defined("lithosphere", anchor(original.id, "lithosphere")),
      defined("plate", anchor(simpler.id, "plate")),
      asked("chat-simpler", anchor(simpler.id, "the plates drift")),
      asked("chat-course", null),
    ].reduce(applyEvent, twoSections());

    const onSimpler = selectAnchors(state, "a");
    expect(onSimpler.terms.map((entry) => entry.term)).toEqual(["plate"]);
    expect(onSimpler.chats.map((chat) => chat.id)).toEqual(["chat-simpler"]);

    const onOriginal = selectAnchors(selectVersion(state, original.id), "a");
    expect(onOriginal.terms.map((entry) => entry.term)).toEqual([
      "lithosphere",
    ]);
    expect(onOriginal.chats).toEqual([]);

    expect(selectAnchors(state, "b")).toEqual({ terms: [], chats: [] });
  });
});
