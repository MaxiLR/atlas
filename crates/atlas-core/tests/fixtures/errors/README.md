# Error bodies of the plan route

These JSON files are bodies the plan route answers with when it refuses a
request. They are **written from OpenAI's documentation**, not captured from
the real service: the shapes and codes follow the plan route's
[errors and recovery](https://developers.openai.com/siwc/token-sharing-open-source/errors-and-recovery)
page. Before a stream opens, direct-route admission can answer `{"detail": …}`
without an error object; otherwise the body carries the standard API error
object, whose `code` and `param` decide the recovery. The same error object
arrives inside a stream as the `error` of `response.failed`. Messages are made
up.

| File                                 | Status | Code                                          |
| ------------------------------------ | ------ | --------------------------------------------- |
| `detail-401.json`                    | 401    | none (`detail` only)                          |
| `detail-403.json`                    | 403    | none (`detail` only)                          |
| `detail-503.json`                    | 503    | none (`detail` only)                          |
| `user-not-eligible.json`             | 403    | `subscription_sharing_user_not_eligible`      |
| `usage-limit-exceeded.json`          | 429    | `subscription_sharing_usage_limit_exceeded`   |
| `usage-unavailable.json`             | 503    | `subscription_sharing_usage_unavailable`      |
| `unsupported-capability.json`        | 400    | `subscription_sharing_unsupported_capability` |
| `route-not-supported.json`           | 403    | `subscription_sharing_route_not_supported`    |
| `invalid-user.json`                  | 401    | `subscription_sharing_invalid_user`           |
| `scope-not-authorized.json`          | 403    | `chatpass_v2_scope_not_authorized`            |
| `invalid-authorization-context.json` | 403    | `chatpass_v2_invalid_authorization_context`   |
| `user-unavailable.json`              | 503    | `subscription_sharing_user_unavailable`       |
| `unknown-code.json`                  | 400    | a code with no documented recovery            |
