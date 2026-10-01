# Brief 14 — OpenAI provider and Keychain

Dependencies: 13. Add a native provider port and fixed HTTPS Responses endpoint, no tools, store=false, strict structured output. Keychain is the only durable credential store; frontend may set/delete a key and read configured state, never retrieve it. Remote permission defaults off each application run. Exact request consent is separate from local capture and key presence.

Bound response bytes, connection/request durations, cancellation and retries. Distinguish auth, offline/timeout, rate limit, refusal, incomplete and malformed output with fixed safe errors. Recheck current permission, policy and input before sending and before publication. Cancellation/revocation discards results and stops retries. No provider bodies or keys in logs/errors. Initial G03 evaluation model: gpt-4o-mini-2024-07-18 (official Responses/Structured Outputs support); real quality/cost evidence remains pending until a user-approved BYOK run.

Acceptance: injected transport/credential fixtures; native Keychain round trip with disposable synthetic credential; no automatic requests. A live request is a separate user-authorized gate.
