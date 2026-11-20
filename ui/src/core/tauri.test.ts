import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";

import type { CoreEvent } from "./generated/CoreEvent";
import { CORE_COMMAND, CORE_EVENT, createTauriCore } from "./tauri";

afterEach(() => clearMocks());

describe("the Tauri core client", () => {
  it("sends each request through the one command", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((command, payload) => {
      calls.push([command, payload]);
      return { type: "Courses", courses: [] };
    });

    const response = await createTauriCore().call({ type: "ListCourses" });

    expect(response).toEqual({ type: "Courses", courses: [] });
    expect(calls).toEqual([
      [CORE_COMMAND, { request: { type: "ListCourses" } }],
    ]);
  });

  it("rejects with the core's error", async () => {
    mockIPC(() => Promise.reject({ type: "SignInRequired" }));

    await expect(
      createTauriCore().call({ type: "CreateCourse", topic: "Plates" }),
    ).rejects.toEqual({ type: "SignInRequired" });
  });

  it("delivers the one event's payloads", async () => {
    mockIPC(() => undefined, { shouldMockEvents: true });
    const seen: CoreEvent[] = [];
    const stop = createTauriCore().subscribe((event) => seen.push(event));

    await emit(CORE_EVENT, {
      type: "UsageChanged",
      usage: { type: "NotConnected" },
    });
    stop();

    expect(seen).toEqual<CoreEvent[]>([
      { type: "UsageChanged", usage: { type: "NotConnected" } },
    ]);
  });
});
