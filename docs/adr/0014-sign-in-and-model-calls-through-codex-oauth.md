# ADR-0014: Sign-in and model calls through Codex OAuth

- Status: Accepted; its line that the usage meter keeps its own login is
  superseded by ADR-0015
- Date: 2026-11-20
- Supersedes: the sign-in and transport of ADR-0002

## Context

ADR-0002 chose ChatGPT plan sharing: a dynamically registered client, the
`chatgpt.tokens.use.direct` scope and the public Responses API. Plan sharing
needs enterprise-account access that the engineer's account does not have.
The engineer chose OpenAI's Codex OAuth and the Codex backend instead, the way
the OMP coding agent signs in and calls models.

## Decision

The existing ChatGPT sign-in is replaced, not joined by a second route:

- Sign-in uses the Codex OAuth client `app_EMoamEEZ73f0CkXaXp7hrann`: PKCE
  (S256) with a fresh state per attempt, `{auth}/oauth/authorize` and
  `{auth}/oauth/token`, the scopes
  `openid profile email offline_access api.connectors.read api.connectors.invoke`
  and Codex's authorisation parameters. Atlas sends `originator=atlas`; it does
  not pose as OMP or the Codex CLI.
- The callback is the one OpenAI allowlists for that client,
  `http://localhost:1455/auth/callback`. `localhost` resolves to both
  loopback addresses and browsers often try `::1` first, so Atlas listens on
  `127.0.0.1:1455` and `[::1]:1455`; otherwise a program holding only `::1`
  would receive the code. When either address cannot be bound the sign-in
  fails and says so; only a system without IPv6 loopback is served on
  `127.0.0.1` alone. Atlas never falls back to another port, never binds a
  wildcard address and never stops whatever holds the port.
- The identity comes from the ID token's claims, which must name the
  authorisation server and the Codex client. The tokens arrive straight from
  the token endpoint over TLS, so no signature is checked.
- A Codex login always spends the plan: there is no plan permission to grant
  or enable. Refresh goes through the Codex client, rotates the refresh token,
  is serialised, and keeps the tokens on transient failures. Signing out clears
  the login locally; the Codex client has no revocation to call.
- Model calls go to the Codex backend, `https://chatgpt.com/backend-api/codex`:
  `POST /responses` and `GET /models?client_version=…`, with the headers the
  OMP Codex provider sends over SSE: the bearer token, `chatgpt-account-id`
  from the token's claims, `x-openai-internal-codex-residency` for a workspace
  pinned to a region (none for `no_constraint`), `originator`, `version`, a
  user agent of Atlas's own and, on Responses, `x-codex-routing-hint`.
- Codex errors map to the existing outcomes: `usage_limit_reached` is the plan
  limit with its reset time, `usage_not_included` is not allowed, rate limits
  and overloads are retried, `unsupported_parameter` drives the
  structured-output fallback, `token_expired` refreshes once.
- A login saved by plan sharing is not a Codex login: it reads as signed out,
  its tokens are never sent or refreshed, and courses stay. Nothing is
  migrated and nothing is requested at startup; the candidate signs in again.
  The client id plan sharing kept in its own Keychain item is no longer read.

The usage meter keeps its own login (ADR-0007).

## Tradeoff

- The Codex backend is not a documented third-party API. It can change, and
  it can enforce account or client restrictions that matching OMP does not
  prove Atlas meets, for an enterprise workspace in particular.
- The client version Atlas declares is pinned; the backend gates the models it
  lists by it, so an old pin can hide newer models.
- Only one sign-in can wait at a time on the machine: another program's Codex
  sign-in on port 1455 blocks Atlas's until it ends.
