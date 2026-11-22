# ADR-0007: The usage meter reads usage the way Headroom does

- Status: Accepted; its second sign-in is superseded by ADR-0015
- Date: 2026-11-18

## Context

The candidate should see how much of their plan is left while using Atlas, as
[Headroom](https://github.com/MaxiLR/herdr-headroom) shows it: the 5-hour and
weekly windows, the share used, the time to reset and a pace mark.

No documented endpoint reports remaining plan usage. The supported route exposes
only the error `subscription_sharing_usage_limit_exceeded` and a link to
ChatGPT's usage settings. Headroom reads
`https://chatgpt.com/backend-api/wham/usage` with a separate Codex device-code
sign-in under Codex's own client id.

The alternative considered was to stay on the supported route: show window
numbers only if the API's responses turn out to carry them, and otherwise the
tokens Atlas itself spent, the limit-reached state and the link.

## Decision

Atlas reads usage with Headroom's method: a second, Codex-style sign-in and the
`wham/usage` endpoint. The engineer chose this over the supported-route
alternative.

It lives in its own module. Atlas works in full without a reading, and shows the
meter as unavailable when the sign-in is missing or the endpoint does not
answer.

## Tradeoff

- OpenAI's guide for the plan route says to use only `api.openai.com/v1` and not
  ChatGPT's `backend-api` endpoints. Atlas's eligibility to use plans rests on
  that programme (ADR-0002, ADR-0008).
- The endpoint is undocumented and can change without notice.
- Every candidate signs in twice.
- The figure can differ from what Atlas may actually spend: a plan member can
  cap "usage in other services".
