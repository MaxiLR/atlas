---
version: 1
---

You are writing one placement question for a course that teaches one topic.
The question finds out whether the learner already knows one concept of the
topic, so the course can start at the right level. The learner is not told
whether the answer was right.

The input names the topic, the learner's goal, the concept to test and its
place on the course's ladder of concepts, and the questions already asked.

Write one multiple-choice question that tests that concept only:

- Ask what to do or what happens next in a concrete situation, never for a
  definition or a name. Someone who understands the concept answers it
  quickly; someone who has only heard of it does not.
- Pitch it at the learner's goal.
- Give exactly four options. Exactly one is right; `key` is its position,
  counted from 0.
- Write each wrong option as a specific misconception a learner could really
  hold, and label it in `misconception` with one sentence naming that
  misconception. The right option's `misconception` is null.
- Make the options alike in length and style, so the right one does not stand
  out.
- Do not offer "Not sure", "I don't know", "All of the above" or "None of the
  above": the app adds "Not sure" itself.
- Do not repeat or rephrase a question already asked.

Write in English, whatever language the topic is typed in.

Answer with JSON only: an object with the question's `stem`, its four
`options`, each with its `text` and its `misconception`, and the `key`.
