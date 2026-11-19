import { describe, expect, it } from "vitest";

import type { CoreRequest } from "../generated/CoreRequest";
import { createFakeCore } from "./index";

const courseId = "00000000-0000-0000-0000-000000000001";
const sectionId = "00000000-0000-0000-0000-000000000101";
const versionId = "00000000-0000-0000-0000-000000001101";
const chatId = "00000000-0000-0000-0000-000000000080";
const anchor = { versionId, block: 0, quote: "the mantle" };

/** Every request the core does not build yet, with the feature it names. */
const unbuilt: [CoreRequest, string][] = [
  [{ type: "GetSession" }, "session"],
  [{ type: "SignIn" }, "session"],
  [{ type: "CancelSignIn" }, "session"],
  [{ type: "EnablePlanUsage" }, "session"],
  [{ type: "SignOut" }, "session"],
  [{ type: "OpenManageUsage" }, "external"],
  [{ type: "OpenExternal", url: "https://example.com/" }, "external"],
  [{ type: "ListModels" }, "models"],
  [{ type: "SelectModel", slug: "gpt-test" }, "models"],
  [{ type: "DeleteCourse", courseId }, "library"],
  [
    {
      type: "SubmitSelfRating",
      courseId,
      rating: "KnowTheBasics",
      goal: "WorkingKnowledge",
    },
    "placement",
  ],
  [
    {
      type: "AnswerPlacement",
      courseId,
      questionId: chatId,
      answer: { type: "Option", index: 2 },
    },
    "placement",
  ],
  [{ type: "RetryPlacementStep", courseId }, "placement"],
  [{ type: "StopGeneration", courseId, sectionId }, "sections"],
  [{ type: "RetrySection", courseId, sectionId }, "sections"],
  [
    { type: "ExplainDifferently", courseId, sectionId, style: "MoreExamples" },
    "versions",
  ],
  [{ type: "SelectVersion", courseId, sectionId, versionId }, "versions"],
  [
    { type: "AnswerCheck", versionId, block: 4, question: 0, choice: 1 },
    "checks",
  ],
  [{ type: "GoDeeper", courseId, sectionId }, "deep_dives"],
  [{ type: "Define", anchor, sentence: "The mantle flows." }, "define"],
  [{ type: "LookupWikipedia", entryId: chatId }, "wikipedia"],
  [{ type: "ListGlossary", courseId }, "define"],
  [{ type: "ListSideChats", courseId }, "side_chats"],
  [{ type: "StartSideChat", courseId, anchor }, "side_chats"],
  [{ type: "OpenSideChat", chatId }, "side_chats"],
  [{ type: "SendSideChatMessage", chatId, text: "Why?" }, "side_chats"],
  [{ type: "StopSideChatReply", chatId }, "side_chats"],
  [{ type: "GetUsage" }, "usage"],
  [{ type: "ConnectUsage" }, "usage"],
  [{ type: "CancelConnectUsage" }, "usage"],
  [{ type: "SetWindowFocused", focused: true }, "usage"],
  [{ type: "GetExportDocument", courseId }, "export"],
  [{ type: "ExportPdf", courseId }, "export"],
];

describe("the fake core", () => {
  it("lists every request it does not build", () => {
    expect(unbuilt).toHaveLength(33);
  });

  it.each(unbuilt)(
    "answers %j as unavailable, naming the feature %s like the real core",
    async (request, feature) => {
      const core = createFakeCore();
      await expect(core.call(request)).rejects.toEqual({
        type: "Unavailable",
        feature,
      });
    },
  );
});
