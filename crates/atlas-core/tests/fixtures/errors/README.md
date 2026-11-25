# Error bodies of the Codex backend

These JSON files are bodies the Codex backend answers with when it refuses a
request. They are **written from the OMP Codex provider's error handling**, not
captured from the real service. Before a stream opens, admission can answer
`{"detail": …}` without an error object; otherwise the body carries an error
object whose `code` (or, for the usage limit and a plan without Codex, its
`type`) decides the recovery, with `param` and `resets_at` when present. The
same error object arrives inside a stream as the `error` of `response.failed`.
Messages are made up.

| File                          | Status | Code                                    |
| ----------------------------- | ------ | --------------------------------------- |
| `detail-401.json`             | 401    | none (`detail` only)                    |
| `detail-403.json`             | 403    | none (`detail` only)                    |
| `detail-503.json`             | 503    | none (`detail` only)                    |
| `usage-not-included.json`     | 403    | type `usage_not_included`               |
| `usage-limit-exceeded.json`   | 429    | type `usage_limit_reached`, `resets_at` |
| `server-overloaded.json`      | 503    | `server_is_overloaded`                  |
| `rate-limit-exceeded.json`    | 429    | `rate_limit_exceeded`                   |
| `unsupported-capability.json` | 400    | `unsupported_parameter`                 |
| `token-expired.json`          | 401    | `token_expired`                         |
| `unknown-code.json`           | 400    | a code with no known recovery           |
