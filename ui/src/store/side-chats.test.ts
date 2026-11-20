import { describe, expect, it } from "vitest";

import type { CoreEvent } from "@/core/generated/CoreEvent";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { CourseView } from "@/core/generated/CourseView";
import type { GenerationEvent } from "@/core/generated/GenerationEvent";
import type { SideChatSummary } from "@/core/generated/SideChatSummary";

import {
  applyEvent,
  applyRequest,
  applyResponse,
  initialState,
  selectOpenCourse,
  selectSideChats,
  type AtlasState,
} from "./index";

const courseId = "course-1";
const at = "2026-11-19T10:00:00Z";
const later = "2026-11-19T11:00:00Z";

const course: CourseView = {
  id: courseId,
  topic: "Plate tectonics",
  stage: "Ready",
  chapters: [],
  resumeAt: null,
  glossaryCount: 0,
  sideChatCount: 2,
  placement: null,
};

function summary(chatId: string, updatedAt = at): SideChatSummary {
  return {
    id: chatId,
    courseId,
    anchor: null,
    sectionTitle: null,
    opening: `Question ${chatId}.`,
    createdAt: at,
    updatedAt,
  };
}

function changed(chat: SideChatSummary): CoreEvent {
  return { type: "SideChatsChanged", courseId, chat };
}

const start: CoreRequest = { type: "StartSideChat", courseId, anchor: null };

/** The core's answer to starting chat `chatId`. */
function started(state: AtlasState, chatId: string): AtlasState {
  return applyResponse(state, start, {
    type: "SideChat",
    chat: {
      id: chatId,
      courseId,
      anchor: null,
      sectionTitle: null,
      messages: [],
      followUps: [],
      createdAt: at,
      updatedAt: at,
    },
  });
}

/** The course open with two side chats, not listed yet. */
function opened(): AtlasState {
  return applyResponse(
    initialState,
    { type: "OpenCourse", courseId },
    { type: "Course", course },
  );
}

function listed(state: AtlasState, chats: SideChatSummary[]): AtlasState {
  return applyResponse(
    state,
    { type: "ListSideChats", courseId },
    { type: "SideChats", courseId, chats },
  );
}

describe("the side-chat count of a course whose chats are not listed", () => {
  it("counts each chat started, whether its event or its answer comes first", () => {
    let state = opened();
    state = started(applyEvent(state, changed(summary("new-1"))), "new-1");
    state = applyEvent(started(state, "new-2"), changed(summary("new-2")));

    expect(selectOpenCourse(state)?.sideChatCount).toBe(4);
  });

  it("does not count a message in a chat it has not listed", () => {
    const state = applyEvent(opened(), changed(summary("old-1", later)));

    expect(selectOpenCourse(state)?.sideChatCount).toBe(2);
  });

  it("does not present the chats it heard of as the course's list", () => {
    const state = started(
      applyEvent(opened(), changed(summary("new-1"))),
      "new-1",
    );

    expect(selectSideChats(state, courseId)).toBeNull();
  });
});

describe("the side-chat count of a course whose chats are listed", () => {
  it("counts each chat started once, whether its event or its answer comes first", () => {
    let state = listed(opened(), [summary("old-1"), summary("old-2")]);
    state = started(applyEvent(state, changed(summary("new-1"))), "new-1");
    state = applyEvent(started(state, "new-2"), changed(summary("new-2")));
    state = applyEvent(state, changed(summary("old-1", later)));

    expect(selectOpenCourse(state)?.sideChatCount).toBe(4);
    expect(selectSideChats(state, courseId)?.map((chat) => chat.id)).toEqual([
      "new-2",
      "new-1",
      "old-1",
      "old-2",
    ]);
  });
});

describe("a chat opened again while its reply is awaited", () => {
  it("still waits for the reply under the one question, with nothing gone wrong", () => {
    const question = "Why do continents not sink?";
    const send: CoreRequest = {
      type: "SendSideChatMessage",
      chatId: "new-1",
      text: question,
    };
    let state = applyRequest(started(opened(), "new-1"), send);
    state = applyResponse(
      state,
      { type: "OpenSideChat", chatId: "new-1" },
      {
        type: "SideChat",
        chat: {
          id: "new-1",
          courseId,
          anchor: null,
          sectionTitle: null,
          messages: [
            {
              id: "message-1",
              role: "Candidate",
              text: question,
              state: "Complete",
              citations: [],
              sources: [],
              createdAt: at,
            },
          ],
          followUps: [],
          createdAt: at,
          updatedAt: at,
        },
      },
    );

    const chat = state.sideChats.chats["new-1"];
    expect(chat?.thread).toEqual([{ type: "Sent", text: question }]);
    expect(chat?.reply).toBeNull();
    expect(chat?.error).toBeNull();
  });
});

describe("a reply answered before its last events arrive", () => {
  it("joins the thread once, and the late events start no stream beside it", () => {
    const send: CoreRequest = {
      type: "SendSideChatMessage",
      chatId: "new-1",
      text: "Why do continents not sink?",
    };
    const reply = (event: GenerationEvent): CoreEvent => ({
      type: "Generation",
      generationId: "reply-1",
      target: { type: "SideChat", courseId, chatId: "new-1" },
      event,
    });
    const text = "They are lighter than the mantle.";
    let state = applyRequest(started(opened(), "new-1"), send);
    state = [
      reply({ type: "Started" }),
      reply({ type: "BlockOpened", block: 0, kind: "Prose" }),
      reply({ type: "TextDelta", block: 0, text }),
    ].reduce(applyEvent, state);

    // The response overtakes the reply's last events.
    state = applyResponse(state, send, {
      type: "SideChatReply",
      chatId: "new-1",
      message: {
        id: "message-2",
        role: "Agent",
        text,
        state: "Complete",
        citations: [],
        sources: [],
        createdAt: at,
      },
    });
    state = [
      reply({
        type: "BlockCompleted",
        block: { index: 0, content: { type: "Prose", text } },
      }),
      reply({ type: "Finished", outcome: { type: "Completed" } }),
    ].reduce(applyEvent, state);

    const chat = state.sideChats.chats["new-1"];
    expect(chat?.reply).toBeNull();
    expect(
      chat?.thread.map((item) =>
        item.type === "Message" ? item.message.id : item.text,
      ),
    ).toEqual(["Why do continents not sink?", "message-2"]);
  });
});
