# Recorded Responses streams

These `.sse` files are server-sent-event streams of `POST /v1/responses` with
`stream: true`. They are **written from OpenAI's documentation**, not captured
from the real service: the event names and shapes follow the
[streaming guide](https://developers.openai.com/api/docs/guides/streaming-responses),
the streaming events of the
[Responses API reference](https://developers.openai.com/api/reference/resources/responses),
the [web search guide](https://developers.openai.com/api/docs/guides/tools-web-search)
(`web_search_call` items and `url_citation` annotations) and the plan route's
[models and inference](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference)
and
[errors and recovery](https://developers.openai.com/siwc/token-sharing-open-source/errors-and-recovery)
pages. Ids, text and sequence numbers are made up. Real streams may differ;
replace these with recordings once the live command has run against a real plan
login.

| File                           | Stream                                                                                                                                                                     |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `completed.sse`                | Plain text, then `response.completed`.                                                                                                                                     |
| `web-search-citations.sse`     | A short message, one `web_search_call`, then a message with two `url_citation` annotations, then `response.completed`.                                                     |
| `failed-usage-limit.sse`       | Text, then `response.failed` with `subscription_sharing_usage_limit_exceeded`.                                                                                             |
| `failed-usage-unavailable.sse` | `response.failed` with `subscription_sharing_usage_unavailable` before any text.                                                                                           |
| `incomplete.sse`               | Text, then `response.incomplete` with the reason `max_output_tokens`.                                                                                                      |
| `broken-mid-event.sse`         | Text, then the connection drops in the middle of an event: no terminal event, no final newline.                                                                            |
| `unknown-events.sse`           | Comments, `id` and `retry` fields, event types Atlas does not know, data without a space, data over several lines, then `response.completed` without a closing blank line. |

Annotation `start_index` and `end_index` count characters of their own
`output_text` part, as in the web search guide's example.
