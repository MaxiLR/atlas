import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { CourseView } from "@/core/generated/CourseView";
import { IndexRoute } from "@/reader/index-route/index-route";
import {
  findSection,
  openCourse,
  placeCourse,
  startFake,
} from "@/test/fake-core";

/** The course on `topic` in `scenario`, as the store holds it once opened. */
async function courseIn(scenario: string, topic: string) {
  const fake = startFake(scenario);
  return { ...fake, course: await openCourse(fake.store, topic) };
}

/** The index drawn for `course` with `current` open; `onOpen` records selections. */
function draw(course: CourseView, current: string | null = null) {
  const onOpen = vi.fn<(sectionId: string) => void>();
  const view = render(
    <IndexRoute
      course={course}
      currentId={current}
      folded={false}
      onOpen={onOpen}
    />,
  );
  const redraw = (next: CourseView, nextCurrent: string | null = current) =>
    view.rerender(
      <IndexRoute
        course={next}
        currentId={nextCurrent}
        folded={false}
        onOpen={onOpen}
      />,
    );
  return { onOpen, redraw };
}

function index() {
  return screen.getByRole("navigation", { name: "Index" });
}

function row(title: string) {
  return within(index()).getByRole("button", { name: title });
}

/** Every row of the index, in the order it shows them. */
function rows() {
  return within(index()).queryAllByRole("button");
}

/** The rows are exactly these, in this order. */
function expectRows(titles: string[]) {
  expect(rows()).toEqual(titles.map(row));
}

/** The drawn mark of a row's node: hollow when unread, filled when read. */
function markOf(title: string) {
  return row(title).querySelector("[data-mark]")?.getAttribute("data-mark");
}

function branchesAt(title: string) {
  return (
    row(title).querySelector("[data-mark]")?.hasAttribute("data-branch") ??
    false
  );
}

/** How far right a row's node sits. */
function nodeX(title: string) {
  return Number(
    row(title).querySelector("[data-mark] circle")?.getAttribute("cx"),
  );
}

/** The section the bright part of the line reaches; null when none is read. */
function travelledTo() {
  return (
    index().querySelector('[data-line="travelled"]')?.getAttribute("data-to") ??
    null
  );
}

function connectorFrom(sectionId: string) {
  return index().querySelector(
    `[data-line="branch"][data-from="${sectionId}"]`,
  );
}

const plateTectonics = [
  "The solid Earth",
  "Layers by composition",
  "Lithosphere and asthenosphere",
  "Plates and their boundaries",
  "Divergent boundaries",
  "Convergent boundaries",
  "Subduction, step by step",
  "Why the denser plate sinks",
  "Continental collision",
  "Transform boundaries",
  "What moves the plates",
  "Mantle convection",
  "Slab pull and ridge push",
];

describe("the index as a route line", () => {
  it("says when the index will appear while a course has none", async () => {
    const { course } = await courseIn("default", "The French Revolution");
    draw(course);

    expect(
      within(index()).getByText("The index appears once placement is done."),
    ).toBeVisible();
    expect(rows()).toHaveLength(0);
    expect(within(index()).queryByRole("status")).toBeNull();
  });

  it("shows the index being built as a status, with no rows", async () => {
    const { course } = await courseIn("default", "The French Revolution");
    draw({
      ...course,
      placement: { ...course.placement!, step: { type: "BuildingIndex" } },
    });

    expect(within(index()).getByRole("status")).toHaveTextContent(
      "Building the index",
    );
    expect(rows()).toHaveLength(0);
  });

  it("draws a part-read course: rows in index order, marks, the travelled line and the current row", async () => {
    const { course } = await courseIn("default", "Plate tectonics");
    const convergent = findSection(course, "Convergent boundaries");
    draw(course, convergent.id);

    expectRows(plateTectonics);
    expect(row("The solid Earth")).toHaveAccessibleDescription(
      "Chapter, 2 sections",
    );
    expect(row("Plates and their boundaries")).toHaveAccessibleDescription(
      "Chapter, 3 sections",
    );
    expect(row("Layers by composition")).toHaveAccessibleDescription("Read");
    expect(row("Convergent boundaries")).toHaveAccessibleDescription(
      "Read, 3 deep dives",
    );
    expect(row("Subduction, step by step")).toHaveAccessibleDescription(
      "Unread",
    );
    expect(row("Transform boundaries")).toHaveAccessibleDescription("Unread");

    expect(markOf("Layers by composition")).toBe("read");
    expect(markOf("Divergent boundaries")).toBe("read");
    expect(markOf("Convergent boundaries")).toBe("read");
    expect(markOf("Subduction, step by step")).toBe("unread");
    expect(markOf("Transform boundaries")).toBe("unread");
    expect(branchesAt("Convergent boundaries")).toBe(true);
    expect(branchesAt("Divergent boundaries")).toBe(false);
    expect(connectorFrom(convergent.id)).not.toBeNull();

    expect(row("Convergent boundaries")).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(
      rows().filter((each) => each.hasAttribute("aria-current")),
    ).toHaveLength(1);
    // The last broad-pass section read, not a deep dive and not a later row.
    expect(travelledTo()).toBe(convergent.id);
  });

  it("keeps chapters further along as a station with their section count until reached or opened", async () => {
    const { store } = startFake("default");
    const course = await placeCourse(store, "Volcanoes");
    const { redraw } = draw(course);

    expectRows([
      "The solid Earth",
      "Layers by composition",
      "Lithosphere and asthenosphere",
      "Plates and their boundaries",
      "Divergent boundaries",
      "Convergent boundaries",
      "Transform boundaries",
      "What moves the plates",
    ]);
    expect(row("What moves the plates")).toHaveAccessibleDescription(
      "Chapter, 2 sections",
    );
    expect(row("What moves the plates")).toHaveTextContent("2");
    expect(markOf("Layers by composition")).toBe("unread");
    expect(markOf("Transform boundaries")).toBe("unread");
    expect(travelledTo()).toBeNull();

    redraw(course, findSection(course, "Mantle convection").id);
    expect(rows().slice(-3)).toEqual([
      row("What moves the plates"),
      row("Mantle convection"),
      row("Slab pull and ridge push"),
    ]);
  });

  it("opens a selected section, and a selected chapter at its first section", async () => {
    const { store } = startFake("default");
    const course = await placeCourse(store, "Volcanoes");
    const { onOpen } = draw(course);
    const user = userEvent.setup();

    await user.click(row("Divergent boundaries"));
    expect(onOpen).toHaveBeenLastCalledWith(
      findSection(course, "Divergent boundaries").id,
    );
    await user.click(row("What moves the plates"));
    expect(onOpen).toHaveBeenLastCalledWith(
      findSection(course, "Mantle convection").id,
    );
    await user.click(row("The solid Earth"));
    expect(onOpen).toHaveBeenLastCalledWith(
      findSection(course, "Layers by composition").id,
    );
    expect(onOpen).toHaveBeenCalledTimes(3);
  });

  it("is one tab stop at the current row, and the arrow keys move along the rows", async () => {
    const { course } = await courseIn("default", "Plate tectonics");
    const { onOpen } = draw(
      course,
      findSection(course, "Convergent boundaries").id,
    );
    const user = userEvent.setup();

    await user.tab();
    expect(row("Convergent boundaries")).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(row("Subduction, step by step")).toHaveFocus();
    await user.keyboard("{ArrowUp}{ArrowUp}");
    expect(row("Divergent boundaries")).toHaveFocus();
    await user.keyboard("{Home}");
    expect(row("The solid Earth")).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(row("The solid Earth")).toHaveFocus();
    await user.keyboard("{End}");
    expect(row("Slab pull and ridge push")).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(onOpen).toHaveBeenLastCalledWith(
      findSection(course, "Slab pull and ridge push").id,
    );

    await user.tab();
    expect(rows()).not.toContain(document.activeElement);
  });

  it("draws Go deeper's proposals as a branch under the section", async () => {
    const { store, state, course } = await courseIn(
      "default",
      "Plate tectonics",
    );
    const divergent = findSection(course, "Divergent boundaries");
    const { redraw } = draw(course, divergent.id);
    expect(connectorFrom(divergent.id)).toBeNull();

    await store.send({
      type: "GoDeeper",
      courseId: course.id,
      sectionId: divergent.id,
    });
    redraw(state().course.byId[course.id]!);

    const order = rows();
    const at = order.indexOf(row("Divergent boundaries"));
    expect(order.slice(at, at + 5)).toEqual([
      row("Divergent boundaries"),
      row("Divergent boundaries: in the field"),
      row("Divergent boundaries: measured"),
      row("Divergent boundaries: open questions"),
      row("Convergent boundaries"),
    ]);
    expect(row("Divergent boundaries")).toHaveAccessibleDescription(
      "Read, 3 deep dives",
    );
    expect(branchesAt("Divergent boundaries")).toBe(true);
    expect(connectorFrom(divergent.id)).not.toBeNull();
    expect(markOf("Divergent boundaries: measured")).toBe("unread");
    expect(nodeX("Divergent boundaries: measured")).toBeGreaterThan(
      nodeX("Divergent boundaries"),
    );
  });

  it("nests deep dives of a deep dive further in, and names long titles whole", async () => {
    const { course } = await courseIn(
      "index-route",
      "The history of mapmaking",
    );
    const guessing = findSection(
      course,
      "Guessing longitude from travel times",
    );
    draw(course, guessing.id);

    const order = rows();
    const at = order.indexOf(row("Ptolemy and the grid: methods"));
    expect(order.slice(at, at + 8)).toEqual([
      row("Ptolemy and the grid: methods"),
      row("Reading latitude from the stars"),
      row("Guessing longitude from travel times"),
      row("Itineraries and stages"),
      row("Marinus of Tyre"),
      row("Why Asia came out too wide"),
      row("Copying the tables by hand"),
      row("Ptolemy and the grid: makers"),
    ]);
    expect(
      row("Guessing longitude from travel times"),
    ).toHaveAccessibleDescription("Unread, 3 deep dives");
    expect(row("Guessing longitude from travel times")).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(row("Reading latitude from the stars")).toHaveAccessibleDescription(
      "Read",
    );
    expect(markOf("Reading latitude from the stars")).toBe("read");
    expect(connectorFrom(guessing.id)).not.toBeNull();
    expect(nodeX("Reading latitude from the stars")).toBeGreaterThan(
      nodeX("Ptolemy and the grid: methods"),
    );
    expect(nodeX("Guessing longitude from travel times")).toBe(
      nodeX("Reading latitude from the stars"),
    );
    expect(nodeX("Marinus of Tyre")).toBeGreaterThan(
      nodeX("Guessing longitude from travel times"),
    );
    // A deep dive read does not carry the line: it stays on the broad pass.
    expect(travelledTo()).toBe(
      findSection(course, "Ptolemy and the grid: legacy").id,
    );

    expect(
      row(
        "How a clay tablet from Babylon placed the known world inside a ring of ocean, and what that shape tells us",
      ),
    ).toHaveAccessibleDescription("Read");
    expect(
      row(
        "Measuring the Earth from Eratosthenes' shadows to the triangulation surveys of the eighteenth century",
      ),
    ).toHaveAccessibleDescription("Chapter, 5 sections");
  });

  it("lists eighty sections: reached chapters as rows, the thirteen further along as stations of five", async () => {
    const { course } = await courseIn(
      "index-route",
      "The history of mapmaking",
    );
    draw(course, findSection(course, "Ptolemy and the grid: legacy").id);

    // Sixteen chapters, the fifteen sections of the first three, and the six
    // deep dives under Ptolemy and the grid: methods.
    expect(rows()).toHaveLength(16 + 15 + 6);
    for (const title of [
      "Medieval world maps",
      "Mercator's projection",
      "Maps on every phone",
    ]) {
      expect(row(title)).toHaveAccessibleDescription("Chapter, 5 sections");
      expect(row(title)).toHaveTextContent("5");
      expect(
        within(index()).queryByRole("button", { name: `${title}: sources` }),
      ).toBeNull();
    }
    expect(
      row(
        "Measuring the Earth from Eratosthenes' shadows to the triangulation surveys of the eighteenth century",
      ),
    ).toBeVisible();
    expect(
      row(
        "Measuring the Earth from Eratosthenes' shadows to the triangulation surveys of the eighteenth century: legacy",
      ),
    ).toHaveAccessibleDescription("Unread");
  });
});
