import { describe, expect, it, vi } from "vitest";

import type { CoreClient, ResponseTo } from "@/core/client";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CoreResponse } from "@/core/generated/CoreResponse";
import type { SectionView } from "@/core/generated/SectionView";
import { selectSection } from "@/store";

import { createAtlasStore } from "./store";

const courseId = "course-1";
const at = "2026-11-19T10:00:00Z";

function section(sectionId: string): SectionView {
  return {
    id: sectionId,
    courseId,
    title: `Section ${sectionId}`,
    summary: "One line.",
    version: {
      id: `v-${sectionId}`,
      style: "Original",
      blocks: [{ index: 0, content: { type: "Prose", text: "Plates move." } }],
      citations: [],
      sources: [],
      writtenWithoutSearch: false,
      model: null,
      createdAt: at,
    },
    versions: [{ id: `v-${sectionId}`, style: "Original", createdAt: at }],
  };
}

/** A core that answers each request with `answer`, recording the requests. */
function scripted(answer: (request: CoreRequest) => CoreResponse) {
  const requests: CoreRequest[] = [];
  const client: CoreClient = {
    call<Request extends CoreRequest>(request: Request) {
      requests.push(request);
      return Promise.resolve(answer(request) as ResponseTo<Request>);
    },
    subscribe: () => () => {},
  };
  return { client, requests };
}

describe("Next answered with where it leads", () => {
  it("opens that section", async () => {
    const { client, requests } = scripted((request) =>
      request.type === "Next"
        ? { type: "NextSection", courseId, sectionId: "b" }
        : { type: "Section", section: section("b") },
    );
    const store = createAtlasStore(client);

    await store.send({ type: "Next", courseId, sectionId: "a" });

    await vi.waitFor(() =>
      expect(selectSection(store.state.getState(), "b")?.status).toBe("stored"),
    );
    expect(requests).toEqual([
      { type: "Next", courseId, sectionId: "a" },
      { type: "OpenSection", courseId, sectionId: "b" },
    ]);
    expect(store.state.getState().navigation.sectionId).toBe("b");
  });
});
