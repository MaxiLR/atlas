# ADR-0002: Model calls through the ChatGPT plan only, for now

- Status: Accepted
- Date: 2026-11-18

## Context

Candidates must be able to pay for model calls with a subscription they already
have, while the harness (prompts, context, tools) stays ours.

The two providers are not symmetric:

- **OpenAI** supports this. [Sign in with ChatGPT](https://developers.openai.com/cookbook/articles/sign-in-with-chatgpt)
  lets open-source and local personal projects use a ChatGPT plan: PKCE with a
  localhost callback, `client_id=dynamic_agent_client`, the scope
  `chatgpt.tokens.use.direct`, then the public Responses API with the user's
  token.
- **Anthropic** prohibits it. The
  [Claude Code legal page](https://code.claude.com/docs/en/legal-and-compliance)
  says third parties may not "offer Claude.ai login into their own applications"
  or "collect, store, or intermediate Claude.ai credentials or session tokens",
  and the Usage Policy in force since 2026-11-12 names services that "route
  requests through consumer subscriptions or misrepresent the product or client
  being used". The one route left open is an end user signing in to the
  unmodified Claude Code binary.

## Decision

The first version signs in with ChatGPT only and calls
`POST https://api.openai.com/v1/responses` directly. The model provider sits
behind one interface in the core so another can be added.

Claude is not supported in the first version. When it is taken up, the
candidates are driving the user's installed Claude Code as a transport (it
accepts a replacement system prompt, no built-in tools and a JSON schema), or an
API key. Performing Claude's OAuth inside Atlas is not a candidate.

## Tradeoff

Claude subscribers cannot use Atlas yet.

The plan route constrains the harness
([preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)):
`store: false` and `stream: true` on every call; no `previous_response_id`, so
the full context is sent each time; `temperature`, `max_output_tokens` and
system-role messages are rejected (the prompt goes in `instructions`); function
tools must be grouped in namespaces; audio input and transcription are
unsupported.

## To verify with a real plan login

- Structured output is accepted (it is not on the unsupported list).
- The hosted web-search tool is allowed for the account.
- The first model `GET /v1/models` lists is the newest.
