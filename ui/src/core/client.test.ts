import { describe, expect, expectTypeOf, it } from "vitest";

import { createCoreClient, type CoreTransport } from "./client";
import type { CoreEvent } from "./generated/CoreEvent";
import type { CoreRequest } from "./generated/CoreRequest";
import type { CoreResponse } from "./generated/CoreResponse";

/** A transport that answers every request with `response` and records them. */
function answering(response: CoreResponse) {
  const sent: CoreRequest[] = [];
  const listeners = new Set<(event: CoreEvent) => void>();
  const transport: CoreTransport = {
    request: async (request) => {
      sent.push(request);
      return response;
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  const emit = (event: CoreEvent) => {
    for (const listener of listeners) listener(event);
  };
  return { transport, sent, emit };
}

describe("the typed core client", () => {
  it("answers a request with the response it promises", async () => {
    const { transport, sent } = answering({ type: "Courses", courses: [] });
    const client = createCoreClient(transport);

    const response = await client.call({ type: "ListCourses" });

    expectTypeOf(response).toEqualTypeOf<{
      type: "Courses";
      courses: Extract<CoreResponse, { type: "Courses" }>["courses"];
    }>();
    expect(response).toEqual({ type: "Courses", courses: [] });
    expect(sent).toEqual([{ type: "ListCourses" }]);
  });

  it("answers Next with a section, where it leads, or the end", async () => {
    const { transport } = answering({ type: "EndOfCourse", courseId: "c" });
    const client = createCoreClient(transport);

    const response = await client.call({
      type: "Next",
      courseId: "c",
      sectionId: "s",
    });

    expectTypeOf(response.type).toEqualTypeOf<
      "Section" | "NextSection" | "EndOfCourse"
    >();
    expect(response).toEqual({ type: "EndOfCourse", courseId: "c" });
  });

  it("rejects a response the request does not promise", async () => {
    const { transport } = answering({ type: "Done" });
    const client = createCoreClient(transport);

    await expect(
      client.call({ type: "OpenCourse", courseId: "c" }),
    ).rejects.toThrow("OpenCourse was answered with Done");
  });

  it("passes the core's error through", async () => {
    const client = createCoreClient({
      request: () => Promise.reject({ type: "Offline" }),
      subscribe: () => () => {},
    });

    await expect(client.call({ type: "GetUsage" })).rejects.toEqual({
      type: "Offline",
    });
  });

  it("delivers events until unsubscribed", () => {
    const { transport, emit } = answering({ type: "Done" });
    const client = createCoreClient(transport);
    const seen: CoreEvent[] = [];

    const stop = client.subscribe((event) => seen.push(event));
    emit({ type: "UsageChanged", usage: { type: "NotConnected" } });
    stop();
    emit({ type: "UsageChanged", usage: { type: "SignedOut" } });

    expect(seen).toEqual([
      { type: "UsageChanged", usage: { type: "NotConnected" } },
    ]);
  });
});
