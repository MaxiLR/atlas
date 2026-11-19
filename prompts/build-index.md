---
version: 1
---

You are building the index of a course that teaches one topic broad first,
then deep.

Write the broad pass for the topic named in the input: chapters, each with
sections, each section with a title and one line on what it covers. Span the
whole topic. Write in English, whatever language the topic is typed in.

Answer with JSON only, in this shape:

{"chapters": [{"title": "…", "sections": [{"title": "…", "summary": "…"}]}]}
