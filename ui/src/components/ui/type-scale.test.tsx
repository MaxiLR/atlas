import { render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";

import { applyStyles, clearStyles } from "@/test/styles";

import { Button } from "./button";
import { Input } from "./input";
import {
  Questionnaire,
  QuestionnaireChoice,
  QuestionnaireChoices,
  QuestionnaireItem,
  QuestionnaireTitle,
} from "./questionnaire";
import { Tabs, TabsList, TabsTrigger } from "./tabs";

afterEach(clearStyles);

// A size and a colour on one element must both survive class merging:
// `text-13 text-body` is a size and a colour, not two colours.
it("sets each control's text on the type scale beside its colour", async () => {
  render(
    <>
      <Button>Glossary</Button>
      <Button variant="ghost">Library</Button>
      <Input aria-label="Topic" />
      <Tabs defaultValue="chats">
        <TabsList>
          <TabsTrigger value="chats">Side chats</TabsTrigger>
        </TabsList>
      </Tabs>
      <Questionnaire>
        <QuestionnaireItem name="goal">
          <QuestionnaireTitle>
            What do you want from this course?
          </QuestionnaireTitle>
          <QuestionnaireChoices>
            <QuestionnaireChoice value="overview">
              An overview
            </QuestionnaireChoice>
          </QuestionnaireChoices>
        </QuestionnaireItem>
      </Questionnaire>
    </>,
  );
  await applyStyles("dark");

  const size = (element: Element) => getComputedStyle(element).fontSize;
  expect(size(screen.getByRole("button", { name: "Glossary" }))).toBe("13px");
  expect(size(screen.getByRole("button", { name: "Library" }))).toBe("13px");
  expect(size(screen.getByRole("textbox", { name: "Topic" }))).toBe("15px");
  expect(size(screen.getByRole("tab", { name: "Side chats" }))).toBe("13px");
  expect(
    size(screen.getByRole("radio", { name: "An overview" }).closest("label")!),
  ).toBe("15px");
  expect(size(screen.getByText("What do you want from this course?"))).toBe(
    "20px",
  );
});
