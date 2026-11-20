/**
 * The text a range covers as a selection reports it: formulas as their TeX
 * source and citation markers left out, so offsets into it line up with the
 * passage the block list reports.
 */
function readText(range: Range): string {
  const container = range.commonAncestorContainer;
  const scope =
    container instanceof Element ? container : container.parentElement;
  if (scope === null) return "";
  const formulas = new Set<Element>();
  let text = "";
  const walker = document.createTreeWalker(scope, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
    if (!range.intersectsNode(node) || node.parentElement === null) continue;
    if (node.parentElement.closest("[data-citation]") !== null) continue;
    const formula = node.parentElement.closest("[data-formula]");
    if (formula !== null) {
      if (!formulas.has(formula)) {
        formulas.add(formula);
        text += formula.getAttribute("data-formula");
      }
      continue;
    }
    const value = node.nodeValue ?? "";
    const from = node === range.startContainer ? range.startOffset : 0;
    const to = node === range.endContainer ? range.endOffset : value.length;
    text += value.slice(from, to);
  }
  return text;
}

/**
 * The sentence or sentences around a selection, within the paragraph (or
 * other top-level element of a prose block) it starts in. Sentences are
 * found by `Intl.Segmenter`, with no rules of any one language.
 */
export function sentenceAround(selection: Range): string {
  const start = selection.startContainer;
  const element = start instanceof Element ? start : start.parentElement;
  const paragraph = element?.closest("[data-prose-block]");
  if (paragraph == null) return "";

  const whole = document.createRange();
  whole.selectNodeContents(paragraph);
  const before = whole.cloneRange();
  before.setEnd(selection.startContainer, selection.startOffset);
  const text = readText(whole);
  const from = readText(before).length;
  const to = Math.min(text.length, from + readText(selection).length);

  const segmenter = new Intl.Segmenter(undefined, { granularity: "sentence" });
  let sentence = "";
  for (const { segment, index } of segmenter.segment(text)) {
    if (index < Math.max(to, from + 1) && index + segment.length > from) {
      sentence += segment;
    }
  }
  return sentence.replace(/\s+/g, " ").trim();
}
