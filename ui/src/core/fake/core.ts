import { createCoreClient, type CoreClient } from "../client";
import type { Anchor } from "../generated/Anchor";
import type { ContentsEntry } from "../generated/ContentsEntry";
import type { CoreError } from "../generated/CoreError";
import type { CoreEvent } from "../generated/CoreEvent";
import type { CoreRequest } from "../generated/CoreRequest";
import type { CoreResponse } from "../generated/CoreResponse";
import type { ExportSection } from "../generated/ExportSection";
import type { GenerationEvent } from "../generated/GenerationEvent";
import type { GenerationOutcome } from "../generated/GenerationOutcome";
import type { GenerationTarget } from "../generated/GenerationTarget";
import type { PlacementState } from "../generated/PlacementState";
import type { PlacementStep } from "../generated/PlacementStep";
import type { SectionView } from "../generated/SectionView";
import type { SideChat } from "../generated/SideChat";
import type { VersionStyle } from "../generated/VersionStyle";
import { systemClock, type FakeClock } from "./clock";
import {
  articles,
  definitions,
  followUps,
  genericDeepDives,
  plateTectonicsQuestions,
  sideChatReply,
  styled,
  type SampleSection,
} from "./sample-course";
import { scenarioNames, scenarios } from "./scenario";
import {
  blockOf,
  chaptersOf,
  chatSummary,
  citationsOf,
  courseView,
  createWorld,
  current,
  iso,
  kindOf,
  librarySummaries,
  newCourse,
  newSection,
  nodeOf,
  proseText,
  reading,
  sampleChapters,
  sectionsOf,
  sectionView,
  storeVersion,
  versionOf,
  type FakeCourse,
  type FakePlacement,
  type FakeSection,
  type FakeWorld,
} from "./world";

export interface FakeCoreOptions {
  /** A scenario by its file name in `scenarios/`; the page's `?scenario=`, else `default`. */
  scenario?: string;
  /** Stamps records and times the usage readings; the wall clock by default. */
  clock?: FakeClock;
  /**
   * Awaited before each step that takes time: each streamed delta or
   * visual, a search, drafting the questions, building the index, the
   * browser coming back from a sign-in.
   */
  pace?: () => Promise<void>;
}

/** The fake core: a `CoreClient`, and what a test may look at or change. */
export interface FakeCore extends CoreClient {
  /** Every URL the core opened in the system browser, in order. */
  readonly opened: readonly string[];
  /** What the core holds; a test may change the conditions between requests. */
  readonly world: FakeWorld;
}

const MINUTE = 60_000;
const MANAGE_USAGE = "https://chatgpt.com/settings/usage";
const SIGN_IN = "https://auth.openai.com/oauth/authorize";

/** How a stopped generation refuses the request that started it. */
const STOPPED: CoreError = {
  type: "Provider",
  message: "the generation was stopped",
};

function scenarioFromPage(): string {
  return (
    new URLSearchParams(globalThis.location?.search ?? "").get("scenario") ??
    "default"
  );
}

function delay(ms: number): () => Promise<void> {
  return () => {
    const { promise, resolve } = Promise.withResolvers<void>();
    setTimeout(resolve, ms);
    return promise;
  };
}

function fail(error: CoreError): never {
  throw error;
}

function outcomeError(outcome: GenerationOutcome): CoreError {
  switch (outcome.type) {
    case "Failed":
      return outcome.error;
    case "Incomplete":
      return {
        type: "Provider",
        message: outcome.reason ?? "the response ended early",
      };
    case "Stopped":
    case "Completed":
      return STOPPED;
  }
}

function placementState(
  step: PlacementStep,
  error: CoreError | null = null,
): PlacementState {
  return { step, minQuestions: 4, maxQuestions: 6, error };
}

/** A definition the fake writes for a term it has none for. */
function genericDefinition(term: string, sentence: string): string {
  const named = term.charAt(0).toUpperCase() + term.slice(1);
  return `${named} is what this sentence turns on: ${sentence.trim()} Read the rest of the passage with that in mind.`;
}

/** A text as prose arriving sentence by sentence. */
function sentences(text: string): SampleSection {
  return {
    title: "",
    summary: "",
    sources: [],
    blocks: [{ type: "Prose", chunks: text.match(/[^.]+\.?\s*/g) ?? [text] }],
  };
}

interface Control {
  stopped: boolean;
}

/**
 * An in-memory core that answers every request as the real one does and
 * emits the same events, in the scenario's state. It exists only for tests
 * and Vite's `fake` mode; the production bundle never contains it.
 */
export function createFakeCore(options: FakeCoreOptions = {}): FakeCore {
  const name = options.scenario ?? scenarioFromPage();
  const scenario = scenarios[name];
  if (scenario === undefined) {
    throw new Error(
      `The fake core has no scenario "${name}"; it has ${scenarioNames.join(", ")}.`,
    );
  }
  const clock = options.clock ?? systemClock;
  const pace = options.pace ?? delay(80);
  const world = createWorld(clock);
  scenario.setup(world);

  const opened: string[] = [];
  const listeners = new Set<(event: CoreEvent) => void>();
  /** Section generations running, by section id. */
  const running = new Map<
    string,
    { control: Control; done: Promise<SectionView> }
  >();
  /** The style each section was last generated in. */
  const lastStyle = new Map<string, VersionStyle>();
  /** Side-chat replies running, by chat id, and when each has settled. */
  const replies = new Map<
    string,
    { control: Control; settled: Promise<unknown> }
  >();
  // A newer attempt makes an older one's late return a no-op.
  let signInAttempt = 0;
  let usageAttempt = 0;
  let focusRun = 0;

  const emit = (event: CoreEvent) => {
    const sent = structuredClone(event);
    for (const listener of listeners) listener(sent);
  };
  const libraryChanged = () =>
    emit({ type: "CoursesChanged", courses: librarySummaries(world) });
  const indexChanged = (course: FakeCourse) =>
    emit({
      type: "IndexChanged",
      courseId: course.id,
      chapters: chaptersOf(course),
    });

  // -- Guards and lookups -----------------------------------------------------

  /** The guard on every request that spends plan usage. */
  function requirePlan(): void {
    const { session } = world;
    if (session.type !== "SignedIn") return fail({ type: "SignInRequired" });
    if (session.planUsage === "Off") return fail({ type: "PlanUsageOff" });
    if (session.planUsage === "NotAllowed") {
      return fail({ type: "PlanNotAllowed" });
    }
  }

  function requireOnline(): void {
    if (world.offline) fail({ type: "Offline" });
  }

  function courseOf(courseId: string): FakeCourse {
    return (
      world.courses.find((course) => course.id === courseId) ??
      fail({ type: "NotFound", what: `course ${courseId}` })
    );
  }

  function sectionOf(course: FakeCourse, sectionId: string): FakeSection {
    return (
      sectionsOf(course).find((section) => section.id === sectionId) ??
      fail({ type: "NotFound", what: `section ${sectionId}` })
    );
  }

  /** The course and section a stored version belongs to. */
  function versionOwner(versionId: string) {
    for (const course of world.courses) {
      for (const section of sectionsOf(course)) {
        const version = section.versions.find(({ id }) => id === versionId);
        if (version !== undefined) return { course, section, version };
      }
    }
    return fail({ type: "NotFound", what: `version ${versionId}` });
  }

  function chatOf(chatId: string): SideChat {
    return (
      world.courses
        .flatMap((course) => course.chats)
        .find((chat) => chat.id === chatId) ??
      fail({ type: "NotFound", what: `side chat ${chatId}` })
    );
  }

  // -- Generation -------------------------------------------------------------

  /**
   * Streams `sample` as the events of one generation, a step at a time,
   * ending as the scenario's fault says or completed. Returns the outcome
   * and the prose text that arrived, by block.
   */
  async function stream(
    target: GenerationTarget,
    sample: SampleSection,
    control: Control,
  ): Promise<{ outcome: GenerationOutcome; arrived: string[] }> {
    const generationId = crypto.randomUUID();
    const send = (event: GenerationEvent) =>
      emit({ type: "Generation", generationId, target, event });
    const arrived: string[] = [];
    const finish = (outcome: GenerationOutcome) => {
      send({ type: "Finished", outcome });
      return { outcome, arrived };
    };
    const stopped = { type: "Stopped" } as const;
    const fault = world.fault;
    if (fault?.type === "Stop" && fault.once) world.fault = null;
    const search = !world.searchRefused;

    send({ type: "Started" });
    if (search && sample.sources.length > 0) {
      send({ type: "Activity", activity: { type: "Searching" } });
      await pace();
      if (control.stopped) return finish(stopped);
    }
    send({ type: "Activity", activity: { type: "Writing" } });

    if (fault?.type === "Fail") {
      for (let attempt = 2; attempt <= fault.retries + 1; attempt += 1) {
        await pace();
        if (control.stopped) return finish(stopped);
        send({ type: "Activity", activity: { type: "Retrying", attempt } });
      }
      const first = sample.blocks[0];
      if (fault.retries === 0 && first?.type === "Prose") {
        send({ type: "BlockOpened", block: 0, kind: "Prose" });
        await pace();
        if (control.stopped) return finish(stopped);
        arrived[0] = first.chunks[0] ?? "";
        send({ type: "TextDelta", block: 0, text: arrived[0] });
      }
      return finish({ type: "Failed", error: fault.error });
    }

    for (const [index, block] of sample.blocks.entries()) {
      send({ type: "BlockOpened", block: index, kind: kindOf(block) });
      if (block.type === "Prose") {
        arrived[index] = "";
        for (const text of block.chunks) {
          await pace();
          if (control.stopped) return finish(stopped);
          arrived[index] += text;
          send({ type: "TextDelta", block: index, text });
        }
      } else {
        await pace();
        if (control.stopped) return finish(stopped);
      }
      send({ type: "BlockCompleted", block: blockOf(block, index) });
      if (search) {
        for (const citation of citationsOf(block, index)) {
          const source = sample.sources.find(
            ({ number }) => number === citation.source,
          )!;
          send({ type: "CitationAdded", citation, source });
        }
      }
      if (fault?.type === "Stop") return finish(stopped);
    }
    return finish({ type: "Completed" });
  }

  /**
   * A usage reading with the plan login, when there is one; none while a
   * scenario holds the first reading back.
   */
  function takeReading(): void {
    if (world.session.type !== "SignedIn") return;
    if (world.firstReadingHeld && world.usage.type !== "Connected") return;
    if (world.usage.type === "Connected") {
      world.usage = {
        type: "Connected",
        reading:
          world.usageFailure === null
            ? reading(world)
            : { ...world.usage.reading, stale: world.usageFailure },
      };
    } else {
      world.usage =
        world.usageFailure === null
          ? { type: "Connected", reading: reading(world) }
          : { type: "FirstReadFailed", reason: world.usageFailure };
    }
    emit({ type: "UsageChanged", usage: world.usage });
  }

  /**
   * A complete version was opened or finished in front of the candidate:
   * the section is read and the course resumes at it.
   */
  function markRead(course: FakeCourse, section: FakeSection): void {
    const moved = course.resumeAt !== section.id;
    course.resumeAt = section.id;
    if (!section.read) {
      section.read = true;
      indexChanged(course);
    } else if (!moved) {
      return;
    }
    libraryChanged();
  }

  /**
   * Generates a version of a section in `style`; two opens of one section
   * share one generation. Stored, current and read once complete; otherwise
   * nothing is stored and the request is refused.
   */
  function generate(
    course: FakeCourse,
    section: FakeSection,
    style: VersionStyle,
  ): Promise<SectionView> {
    const existing = running.get(section.id);
    if (existing !== undefined) return existing.done;
    const control: Control = { stopped: false };
    const sample = styled(section.sample, style);
    lastStyle.set(section.id, style);
    const done = (async () => {
      const { outcome } = await stream(
        { type: "Section", courseId: course.id, sectionId: section.id, style },
        sample,
        control,
      );
      if (outcome.type !== "Completed") throw outcomeError(outcome);
      const version = versionOf(world, sample, style, iso(world));
      storeVersion(section, version);
      course.updatedAt = version.createdAt;
      markRead(course, section);
      takeReading();
      return sectionView(section, version);
    })().finally(() => running.delete(section.id));
    running.set(section.id, { control, done });
    return done;
  }

  // -- Placement --------------------------------------------------------------

  function placementOf(course: FakeCourse): FakePlacement {
    return course.stage === "Placement" && course.placement !== null
      ? course.placement
      : fail({
          type: "InvalidRequest",
          message: `course ${course.id} is not in placement`,
        });
  }

  function progress(
    course: FakeCourse,
    placement: FakePlacement,
    state: PlacementState,
  ): PlacementState {
    placement.state = state;
    emit({ type: "PlacementProgress", courseId: course.id, placement: state });
    return state;
  }

  /** Why a placement step fails in this world, if it does. */
  function stepError(
    step: "PreparingQuestions" | "BuildingIndex",
  ): CoreError | null {
    if (world.offline) return { type: "Offline" };
    const once = world.placementFailure;
    if (once?.step === step) {
      world.placementFailure = null;
      return once.error;
    }
    return world.fault?.type === "Fail" ? world.fault.error : null;
  }

  function askNext(course: FakeCourse, placement: FakePlacement) {
    const number = placement.answers.length + 1;
    const draft = placement.questions[number - 1]!;
    placement.questionId = crypto.randomUUID();
    return progress(
      course,
      placement,
      placementState({
        type: "Question",
        question: {
          id: placement.questionId,
          number,
          stem: draft.stem,
          options: draft.options,
        },
      }),
    );
  }

  async function prepareQuestions(
    course: FakeCourse,
    placement: FakePlacement,
  ): Promise<PlacementState> {
    const step = { type: "PreparingQuestions" } as const;
    progress(course, placement, placementState(step));
    const error = stepError(step.type);
    if (error !== null) {
      return progress(course, placement, placementState(step, error));
    }
    await pace();
    return askNext(course, placement);
  }

  async function buildIndex(
    course: FakeCourse,
    placement: FakePlacement,
  ): Promise<PlacementState> {
    const step = { type: "BuildingIndex" } as const;
    progress(course, placement, placementState(step));
    const error = stepError(step.type);
    if (error !== null) {
      return progress(course, placement, placementState(step, error));
    }
    await pace();
    course.chapters = sampleChapters(course.id);
    course.stage = "Ready";
    course.resumeAt = course.chapters[0]?.sections[0]?.id ?? null;
    course.updatedAt = iso(world);
    indexChanged(course);
    const done = progress(course, placement, placementState({ type: "Done" }));
    course.placement = null;
    libraryChanged();
    return done;
  }

  // -- Requests ---------------------------------------------------------------

  async function handle(request: CoreRequest): Promise<CoreResponse> {
    switch (request.type) {
      case "GetSession": {
        const unreadable = world.sessionUnreadable;
        if (unreadable !== null) {
          // Requests made together, as React's development mode makes on
          // mount, are one ask; one made after the failure was answered
          // reads the session.
          setTimeout(() => (world.sessionUnreadable = null));
          fail(unreadable);
        }
        return { type: "Session", session: world.session };
      }

      case "SignIn": {
        if (world.session.type !== "SignedOut") {
          return { type: "Session", session: world.session };
        }
        requireOnline();
        const attempt = ++signInAttempt;
        world.session = { type: "SigningIn" };
        opened.push(SIGN_IN);
        emit({ type: "SessionChanged", session: world.session });
        void pace().then(() => {
          if (attempt !== signInAttempt) return;
          world.session = {
            type: "SignedIn",
            account: { email: "candidate@example.com" },
            planUsage: world.grantedPlanUsage,
          };
          emit({ type: "SessionChanged", session: world.session });
          // The meter waits for its first reading, taken with the new login.
          if (world.usage.type !== "NoPlanLogin") return;
          const first = ++usageAttempt;
          world.usage = { type: "AwaitingReading" };
          emit({ type: "UsageChanged", usage: world.usage });
          void pace().then(() => {
            if (first === usageAttempt) takeReading();
          });
        });
        return { type: "Session", session: world.session };
      }

      case "CancelSignIn":
        if (world.session.type === "SigningIn") {
          signInAttempt += 1;
          world.session = { type: "SignedOut", reason: null };
          emit({ type: "SessionChanged", session: world.session });
        }
        return { type: "Session", session: world.session };

      case "EnablePlanUsage": {
        const { session } = world;
        if (session.type !== "SignedIn")
          return fail({ type: "SignInRequired" });
        if (session.planUsage === "Off") {
          world.grantedPlanUsage = "On";
          world.session = { ...session, planUsage: "On" };
          emit({ type: "SessionChanged", session: world.session });
        }
        return { type: "Session", session: world.session };
      }

      case "SignOut":
        signInAttempt += 1;
        usageAttempt += 1;
        world.session = { type: "SignedOut", reason: { type: "SignedOut" } };
        world.usage = { type: "NoPlanLogin" };
        emit({ type: "SessionChanged", session: world.session });
        emit({ type: "UsageChanged", usage: world.usage });
        return { type: "Session", session: world.session };

      case "OpenManageUsage":
        if (world.browserFailure !== null) {
          fail({ type: "Provider", message: world.browserFailure });
        }
        opened.push(MANAGE_USAGE);
        return { type: "Done" };

      case "OpenExternal": {
        const protocol = URL.canParse(request.url)
          ? new URL(request.url).protocol
          : null;
        if (protocol !== "http:" && protocol !== "https:") {
          return fail({
            type: "InvalidRequest",
            message: `only web links open in the browser, not ${request.url}`,
          });
        }
        if (world.browserFailure !== null) {
          fail({ type: "Provider", message: world.browserFailure });
        }
        opened.push(request.url);
        return { type: "Done" };
      }

      case "ListModels":
        return { type: "Models", list: world.models };

      case "SelectModel": {
        if (!world.models.models.some(({ slug }) => slug === request.slug)) {
          return fail({ type: "NotFound", what: `model ${request.slug}` });
        }
        world.models = { ...world.models, current: request.slug };
        emit({ type: "ModelsChanged", list: world.models });
        return { type: "Models", list: world.models };
      }

      case "ListCourses":
        return { type: "Courses", courses: librarySummaries(world) };

      case "CreateCourse": {
        const topic = request.topic.trim();
        if (topic === "") {
          return fail({
            type: "InvalidRequest",
            message: "the topic is empty",
          });
        }
        requirePlan();
        const course = newCourse(world, topic, "Placement");
        course.placement = {
          state: placementState({ type: "ChooseRating" }),
          questions: plateTectonicsQuestions.slice(0, world.placementQuestions),
          rating: null,
          goal: null,
          answers: [],
          questionId: null,
        };
        world.courses.push(course);
        libraryChanged();
        return { type: "Course", course: courseView(course) };
      }

      case "OpenCourse": {
        const course = courseOf(request.courseId);
        course.lastOpenedAt = iso(world);
        libraryChanged();
        return { type: "Course", course: courseView(course) };
      }

      case "DeleteCourse": {
        const course = courseOf(request.courseId);
        for (const section of sectionsOf(course)) {
          const run = running.get(section.id);
          if (run !== undefined) run.control.stopped = true;
        }
        world.courses = world.courses.filter(({ id }) => id !== course.id);
        libraryChanged();
        return { type: "Done" };
      }

      case "SubmitSelfRating": {
        const course = courseOf(request.courseId);
        requirePlan();
        const placement = placementOf(course);
        if (placement.state.step.type !== "ChooseRating") {
          return fail({
            type: "InvalidRequest",
            message: "the self-rating is already chosen",
          });
        }
        placement.rating = request.rating;
        placement.goal = request.goal;
        return {
          type: "Placement",
          courseId: course.id,
          placement: await prepareQuestions(course, placement),
        };
      }

      case "AnswerPlacement": {
        const course = courseOf(request.courseId);
        requirePlan();
        const placement = placementOf(course);
        if (
          placement.state.step.type !== "Question" ||
          placement.questionId !== request.questionId
        ) {
          return fail({
            type: "InvalidRequest",
            message: `question ${request.questionId} is not the one shown`,
          });
        }
        placement.answers.push(request.answer);
        placement.questionId = null;
        return {
          type: "Placement",
          courseId: course.id,
          placement:
            placement.answers.length >= placement.questions.length
              ? await buildIndex(course, placement)
              : askNext(course, placement),
        };
      }

      case "RetryPlacementStep": {
        const course = courseOf(request.courseId);
        requirePlan();
        const placement = placementOf(course);
        const { step, error } = placement.state;
        const retried =
          error === null
            ? placement.state
            : step.type === "PreparingQuestions"
              ? await prepareQuestions(course, placement)
              : step.type === "BuildingIndex"
                ? await buildIndex(course, placement)
                : placement.state;
        return { type: "Placement", courseId: course.id, placement: retried };
      }

      case "OpenSection": {
        const course = courseOf(request.courseId);
        const section = sectionOf(course, request.sectionId);
        const version = current(section);
        if (version !== null) {
          markRead(course, section);
          return { type: "Section", section: sectionView(section, version) };
        }
        if (!running.has(section.id)) {
          requirePlan();
          requireOnline();
        }
        return {
          type: "Section",
          section: await generate(course, section, "Original"),
        };
      }

      case "StopGeneration": {
        const run = running.get(request.sectionId);
        if (run !== undefined) run.control.stopped = true;
        return { type: "Done" };
      }

      case "RetrySection": {
        const course = courseOf(request.courseId);
        const section = sectionOf(course, request.sectionId);
        if (!running.has(section.id)) {
          requirePlan();
          requireOnline();
        }
        return {
          type: "Section",
          section: await generate(
            course,
            section,
            lastStyle.get(section.id) ?? "Original",
          ),
        };
      }

      case "ExplainDifferently": {
        const course = courseOf(request.courseId);
        const section = sectionOf(course, request.sectionId);
        requirePlan();
        requireOnline();
        return {
          type: "Section",
          section: await generate(course, section, request.style),
        };
      }

      case "SelectVersion": {
        const course = courseOf(request.courseId);
        const section = sectionOf(course, request.sectionId);
        const version =
          section.versions.find(({ id }) => id === request.versionId) ??
          fail({ type: "NotFound", what: `version ${request.versionId}` });
        section.currentVersionId = version.id;
        return { type: "Section", section: sectionView(section, version) };
      }

      case "AnswerCheck": {
        const { section, version } = versionOwner(request.versionId);
        const keyed = styled(section.sample, version.style).blocks[
          request.block
        ];
        const question =
          keyed?.type === "Check" ? keyed.questions[request.question] : null;
        if (question == null || request.choice >= question.options.length) {
          return fail({
            type: "InvalidRequest",
            message: "no such check question or option",
          });
        }
        const result = {
          choice: request.choice,
          correct: request.choice === question.key,
          key: question.key,
          explanation: question.explanation,
        };
        version.blocks = version.blocks.map((block) =>
          block.index === request.block && block.content.type === "Check"
            ? {
                ...block,
                content: {
                  ...block.content,
                  questions: block.content.questions.map((shown) =>
                    shown.index === request.question
                      ? { ...shown, answer: result }
                      : shown,
                  ),
                },
              }
            : block,
        );
        return { type: "CheckAnswered", result };
      }

      case "Next": {
        const course = courseOf(request.courseId);
        const section = sectionOf(course, request.sectionId);
        const broadPass = course.chapters.flatMap(
          (chapter) => chapter.sections,
        );
        const after = (root: FakeSection) =>
          broadPass[broadPass.indexOf(root) + 1] ?? null;
        let following: FakeSection | null;
        if (section.parent === null) {
          following = after(section);
        } else {
          const siblings = section.parent.deepDives ?? [];
          let root = section;
          while (root.parent !== null) root = root.parent;
          following = siblings[siblings.indexOf(section) + 1] ?? after(root);
        }
        return following === null
          ? { type: "EndOfCourse", courseId: course.id }
          : {
              type: "NextSection",
              courseId: course.id,
              sectionId: following.id,
            };
      }

      case "GoDeeper": {
        const course = courseOf(request.courseId);
        const section = sectionOf(course, request.sectionId);
        if (section.deepDives === null) {
          requirePlan();
          requireOnline();
          await pace();
          section.deepDives = (
            section.sample.deepDives ?? genericDeepDives(section.sample.title)
          ).map((sample) => newSection(course.id, sample, section));
          indexChanged(course);
          libraryChanged();
        }
        return {
          type: "DeepDives",
          courseId: course.id,
          sectionId: section.id,
          deepDives: section.deepDives.map(nodeOf),
        };
      }

      case "Define": {
        const { anchor, sentence } = request;
        const { course, section } = versionOwner(anchor.versionId);
        requirePlan();
        requireOnline();
        const term = anchor.quote.trim();
        const definition =
          definitions[term.toLowerCase()] ?? genericDefinition(term, sentence);
        const { outcome } = await stream(
          { type: "Define", courseId: course.id, anchor },
          sentences(definition),
          { stopped: false },
        );
        if (outcome.type !== "Completed") throw outcomeError(outcome);
        const entry = {
          id: crypto.randomUUID(),
          courseId: course.id,
          sectionId: section.id,
          sectionTitle: section.sample.title,
          term,
          definition,
          anchor,
          wikipedia: null,
          createdAt: iso(world),
        };
        course.glossary.push(entry);
        emit({ type: "GlossaryChanged", courseId: course.id, entry });
        takeReading();
        return { type: "Definition", entry };
      }

      case "LookupWikipedia": {
        const entry =
          world.courses
            .flatMap((course) => course.glossary)
            .find(({ id }) => id === request.entryId) ??
          fail({ type: "NotFound", what: `glossary entry ${request.entryId}` });
        if (world.offline) {
          return {
            type: "Wikipedia",
            entryId: entry.id,
            result: { type: "Failed", error: { type: "Offline" } },
          };
        }
        const article = articles[entry.term.toLowerCase()];
        entry.wikipedia =
          article === undefined
            ? { type: "NoArticle" }
            : { type: "Found", ...article };
        return {
          type: "Wikipedia",
          entryId: entry.id,
          result: entry.wikipedia,
        };
      }

      case "ListGlossary": {
        const course = courseOf(request.courseId);
        return {
          type: "Glossary",
          courseId: course.id,
          entries: course.glossary,
        };
      }

      case "ListSideChats": {
        const course = courseOf(request.courseId);
        return {
          type: "SideChats",
          courseId: course.id,
          chats: course.chats.map(chatSummary),
        };
      }

      case "StartSideChat": {
        const course = courseOf(request.courseId);
        const anchor: Anchor | null = request.anchor;
        const owner = anchor === null ? null : versionOwner(anchor.versionId);
        if (owner !== null && owner.course !== course) {
          return fail({
            type: "InvalidRequest",
            message: "the passage is not in this course",
          });
        }
        const now = iso(world);
        const chat: SideChat = {
          id: crypto.randomUUID(),
          courseId: course.id,
          anchor,
          sectionTitle: owner?.section.sample.title ?? null,
          messages: [],
          followUps: [],
          createdAt: now,
          updatedAt: now,
        };
        course.chats.unshift(chat);
        emit({
          type: "SideChatsChanged",
          courseId: course.id,
          chat: chatSummary(chat),
        });
        return { type: "SideChat", chat };
      }

      case "OpenSideChat":
        return { type: "SideChat", chat: chatOf(request.chatId) };

      case "SendSideChatMessage": {
        const chat = chatOf(request.chatId);
        requirePlan();
        requireOnline();
        const text = request.text.trim();
        if (text === "") {
          return fail({
            type: "InvalidRequest",
            message: "the message is empty",
          });
        }
        chat.messages.push({
          id: crypto.randomUUID(),
          role: "Candidate",
          text,
          state: "Complete",
          citations: [],
          sources: [],
          createdAt: iso(world),
        });
        chat.followUps = [];
        const control: Control = { stopped: false };
        const streaming = stream(
          { type: "SideChat", courseId: chat.courseId, chatId: chat.id },
          sideChatReply,
          control,
        );
        replies.set(chat.id, { control, settled: streaming.catch(() => {}) });
        const { outcome, arrived } = await streaming.finally(() =>
          replies.delete(chat.id),
        );
        if (outcome.type !== "Completed" && outcome.type !== "Stopped") {
          throw outcomeError(outcome);
        }
        // Deleted meanwhile, the chat stores and announces nothing more.
        chatOf(chat.id);
        const complete = outcome.type === "Completed";
        const block = sideChatReply.blocks[0]!;
        const cited = complete && !world.searchRefused;
        const message = {
          id: crypto.randomUUID(),
          role: "Agent" as const,
          text:
            complete && block.type === "Prose"
              ? proseText(block)
              : (arrived[0] ?? ""),
          state: complete ? ("Complete" as const) : ("Stopped" as const),
          citations: cited ? citationsOf(block, 0) : [],
          sources: cited ? sideChatReply.sources : [],
          createdAt: iso(world),
        };
        chat.messages.push(message);
        chat.updatedAt = message.createdAt;
        emit({
          type: "SideChatsChanged",
          courseId: chat.courseId,
          chat: chatSummary(chat),
        });
        if (complete) {
          chat.followUps = followUps;
          emit({
            type: "FollowUpPrompts",
            chatId: chat.id,
            prompts: followUps,
          });
          takeReading();
        }
        return { type: "SideChatReply", chatId: chat.id, message };
      }

      case "StopSideChatReply": {
        const running = replies.get(request.chatId);
        if (running !== undefined) running.control.stopped = true;
        return { type: "Done" };
      }

      case "DeleteGlossaryEntry": {
        const { entryId } = request;
        const course =
          world.courses.find(({ glossary }) =>
            glossary.some(({ id }) => id === entryId),
          ) ?? fail({ type: "NotFound", what: `glossary entry ${entryId}` });
        if (world.deleteFailure !== null) {
          fail({ type: "Store", message: world.deleteFailure });
        }
        course.glossary.splice(
          course.glossary.findIndex(({ id }) => id === entryId),
          1,
        );
        return { type: "Done" };
      }

      case "DeleteSideChat": {
        const chat = chatOf(request.chatId);
        if (world.deleteFailure !== null) {
          fail({ type: "Store", message: world.deleteFailure });
        }
        // Removed before its reply is stopped, so the reply finds no chat to
        // store into; the answer waits until that reply has settled.
        const { chats } = courseOf(chat.courseId);
        chats.splice(chats.indexOf(chat), 1);
        const running = replies.get(chat.id);
        if (running !== undefined) {
          running.control.stopped = true;
          await running.settled;
        }
        return { type: "Done" };
      }

      case "GetUsage":
        // Signed in without a reading yet, the first ask takes one, in place
        // of any first reading still on its way.
        if (world.usage.type !== "Connected") {
          usageAttempt += 1;
          takeReading();
        }
        return { type: "Usage", usage: world.usage };

      case "SetWindowFocused": {
        const run = ++focusRun;
        if (request.focused) {
          void (async () => {
            for (;;) {
              await clock.sleep(5 * MINUTE);
              if (run !== focusRun) return;
              takeReading();
            }
          })();
        }
        return { type: "Done" };
      }

      case "GetExportDocument": {
        const course = courseOf(request.courseId);
        const contents: ContentsEntry[] = [];
        const sections: ExportSection[] = [];
        const visit = (section: FakeSection, depth: number) => {
          const version = current(section);
          contents.push({
            nodeId: section.id,
            level: section.kind === "BroadPass" ? "Section" : "DeepDive",
            depth,
            title: section.sample.title,
            included: version !== null,
          });
          if (version !== null) {
            sections.push({
              nodeId: section.id,
              kind: section.kind,
              depth,
              title: section.sample.title,
              summary: section.sample.summary,
              version,
            });
          }
          for (const dive of section.deepDives ?? []) visit(dive, depth + 1);
        };
        for (const chapter of course.chapters) {
          contents.push({
            nodeId: chapter.id,
            level: "Chapter",
            depth: 0,
            title: chapter.title,
            included: true,
          });
          for (const section of chapter.sections) visit(section, 1);
        }
        return {
          type: "ExportDocument",
          document: {
            courseId: course.id,
            title: course.topic,
            contents,
            sections,
            glossary: course.glossary,
          },
        };
      }

      case "ExportPdf": {
        const course = courseOf(request.courseId);
        const failure = world.exportFailure;
        if (failure?.type === "Refused") fail(failure.error);
        return {
          type: "Exported",
          result:
            failure?.type === "Failed"
              ? { type: "Failed", reason: failure.reason }
              : {
                  type: "Saved",
                  path: `/Users/candidate/Documents/${course.topic}.pdf`,
                },
        };
      }
    }
  }

  const client = createCoreClient({
    request: async (request) => structuredClone(await handle(request)),
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  });
  return { ...client, opened, world };
}
