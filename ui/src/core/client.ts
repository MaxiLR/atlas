import type { CoreError } from "./generated/CoreError";
import type { CoreEvent } from "./generated/CoreEvent";
import type { CoreRequest } from "./generated/CoreRequest";
import type { CoreResponse } from "./generated/CoreResponse";

/** The responses the core answers each request with, as `Core::handle` routes them. */
export const responseTypes = {
  GetSession: ["Session"],
  SignIn: ["Session"],
  CancelSignIn: ["Session"],
  EnablePlanUsage: ["Session"],
  SignOut: ["Session"],
  OpenManageUsage: ["Done"],
  OpenExternal: ["Done"],
  ListModels: ["Models"],
  SelectModel: ["Models"],
  ListCourses: ["Courses"],
  CreateCourse: ["Course"],
  OpenCourse: ["Course"],
  DeleteCourse: ["Done"],
  SubmitSelfRating: ["Placement"],
  AnswerPlacement: ["Placement"],
  RetryPlacementStep: ["Placement"],
  OpenSection: ["Section"],
  StopGeneration: ["Done"],
  RetrySection: ["Section"],
  ExplainDifferently: ["Section"],
  SelectVersion: ["Section"],
  AnswerCheck: ["CheckAnswered"],
  Next: ["Section", "NextSection", "EndOfCourse"],
  GoDeeper: ["DeepDives"],
  Define: ["Definition"],
  LookupWikipedia: ["Wikipedia"],
  ListGlossary: ["Glossary"],
  ListSideChats: ["SideChats"],
  StartSideChat: ["SideChat"],
  OpenSideChat: ["SideChat"],
  SendSideChatMessage: ["SideChatReply"],
  StopSideChatReply: ["Done"],
  GetUsage: ["Usage"],
  ConnectUsage: ["Usage"],
  CancelConnectUsage: ["Usage"],
  SetWindowFocused: ["Done"],
  GetExportDocument: ["ExportDocument"],
  ExportPdf: ["Exported"],
} as const satisfies {
  [Type in CoreRequest["type"]]: readonly CoreResponse["type"][];
};

/** The response a request is answered with. */
export type ResponseTo<Request extends CoreRequest> = Extract<
  CoreResponse,
  { type: (typeof responseTypes)[Request["type"]][number] }
>;

/**
 * The interface's only way to reach the core: one typed request in, the
 * response that request promises out, events on the side. A failed request
 * rejects with a `CoreError`.
 */
export interface CoreClient {
  call<Request extends CoreRequest>(
    request: Request,
  ): Promise<ResponseTo<Request>>;
  /** Calls `listener` for every core event; returns the unsubscribe. */
  subscribe(listener: (event: CoreEvent) => void): () => void;
}

/** The untyped channel a client is built over: Tauri's command, or the fake core. */
export interface CoreTransport {
  request(request: CoreRequest): Promise<CoreResponse>;
  subscribe(listener: (event: CoreEvent) => void): () => void;
}

/** A client that rejects any response its request does not promise. */
export function createCoreClient(transport: CoreTransport): CoreClient {
  return {
    async call<Request extends CoreRequest>(request: Request) {
      const response = await transport.request(request);
      const promised: readonly CoreResponse["type"][] =
        responseTypes[request.type];
      if (!promised.includes(response.type)) {
        throw new Error(`${request.type} was answered with ${response.type}`);
      }
      return response as ResponseTo<Request>;
    },
    subscribe: (listener) => transport.subscribe(listener),
  };
}

/** A sentence for a failed request, whether the core or the transport failed. */
export function describeError(error: unknown): string {
  if (typeof error === "object" && error !== null && "type" in error) {
    const coreError = error as CoreError;
    switch (coreError.type) {
      case "NotFound":
        return `Not found: ${coreError.what}.`;
      case "Unavailable":
        return `${coreError.feature} is not available yet.`;
      case "InvalidRequest":
      case "Provider":
      case "Store":
        return coreError.message;
      case "PlanLimit":
        return "The plan's usage limit is reached.";
      case "PlanUsageOff":
        return "Plan usage is off.";
      case "PlanNotAllowed":
        return "Plan usage is not allowed for this account.";
      case "SignInRequired":
        return "Sign in with ChatGPT first.";
      case "Offline":
        return "The app is offline.";
      case "FailedAfterRetries":
        return `The request failed after ${coreError.attempts} attempts.`;
    }
  }
  return error instanceof Error ? error.message : String(error);
}
