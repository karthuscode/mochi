# Secret detection and redaction

Brief 06 implements one local, provider-independent `RedactionEngine` in `crates/privacy`. Its rule identifier is `redaction-v1`. It protects supported capture text before the first spool write and validates imported spool records with the same rules before SQLite content insertion. It is best-effort pattern detection, not a guarantee that every private value can be recognized.

## Threat model and pipeline

User prompts, agent messages, commands, command output, tool responses, and approved Git/source content may contain secrets even when their paths are allowed. A same-user process may also forge a structurally valid spool record. The supported pipeline is bounded provider input → Brief 05 path decision → shared redaction → adapter path masking and 8 KiB field bound → normalized event → defensive spool validation → private spool → shared validation at SQLite import. The import pass rejects unsafe records as metadata-only rejections; it does not persist a redacted copy of an untrusted forged record. Assembly consumes already-safe evidence and does not perform redaction.

Path `Deny` always wins: Mochi does not read a prohibited file to redact it afterward. The engine is also public for a future Analysis Bundle Builder, which must separately recheck consent and sanitize its final outbound text.

## Supported classes and replacements

| Evidence | Replacement |
|---|---|
| OpenAI-style `sk-`, `pk-`, `rk-` keys and contextual `OPENAI_API_KEY` | `[REDACTED:OPENAI_API_KEY]` |
| GitHub `ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`, `github_pat_` tokens | `[REDACTED:GITHUB_TOKEN]` |
| AWS `AKIA`/`ASIA` access key IDs and `AWS_SECRET_ACCESS_KEY` assignments | `[REDACTED:AWS_ACCESS_KEY_ID]`, `[REDACTED:AWS_SECRET_ACCESS_KEY]` |
| `Bearer` authorization values | `[REDACTED:BEARER_TOKEN]` |
| Plausible three-segment JWTs with JSON-like encoded header and payload | `[REDACTED:JWT]` |
| Credential-bearing Postgres, MySQL, MongoDB, and MongoDB+SRV URLs | scheme plus `[REDACTED:DATABASE_CREDENTIAL]`; username, host, and path are omitted |
| Password/passwd assignments and structured fields | `[REDACTED:PASSWORD]` |
| Contextual API-key, token, secret, credential, cookie, and private-key fields | `[REDACTED:GENERIC_SECRET]` or a more specific class |
| Complete RSA, EC, OpenSSH, DSA, and generic private-key blocks | `[REDACTED:PRIVATE_KEY]` |

Assignments retain a safe key name and surrounding prose where possible. Values may be quoted or unquoted. Markers contain no fragment, length, or hash of the original value. The result returns safe finding classes/counts, a changed flag, and bounded text; detected values are never retained for audit. Re-running the engine on its output is stable.

## Structured values and false positives

The engine walks JSON strings, arrays, and objects with key context. Sensitive keys replace the entire value; other strings receive the same text detectors. A serialized JSON string is parsed and checked as structured text when possible. An unsafe key name, excessive depth/size, or malformed high-risk private-key block fails closed. This allows ordinary fields such as `keyboardLayout` and `primaryKeyColumn` while protecting `client_secret`, `access_token`, and `db_password`.

No universal entropy heuristic is used. Long UUIDs, Git SHAs, SHA-256 hashes, package integrity strings, CSS values, version numbers, and ordinary URLs are not classified solely by length. Known prefixes, assignment context, authorization syntax, URL credentials, and private-key delimiters are the primary evidence. This reduces destructive false positives, but unknown secret formats remain possible.

## Bounds and failure behavior

Text and aggregate structured input are capped at 1 MiB. Structured traversal is capped at depth 32, 256 items per container, and 4,096 total nodes. Capture still limits a normalized text field to 8 KiB **after** complete bounded redaction, marking truncation. The provider message cap remains 1 MiB; an oversized message is rejected without storing any unscanned tail. Git retains its 64 KiB file-read bound and existing file/path restrictions.

On redaction failure, capture emits a metadata-only `sanitization_failed` gap where possible. A direct caller of the spool cannot persist recognized unsanitized content: the record becomes a metadata-only gap. A forged spool record is rejected before SQL content insertion, and only an opaque rejection token, safe reason, receive sequence when valid, and ingress UUID tombstone are saved. No rejected raw text, secret hash, or source identity derived from unsafe metadata is persisted. Errors and diagnostics use fixed safe variants; no raw payload or matched value is logged.

## Limits

Supported patterns cannot identify every credential, arbitrary confidential business data, or secrets deliberately split into unrelated fields. No semantic shell-command parsing is attempted; command **text and output** are scanned as bounded text. The engine does not retroactively scrub pre-Brief-06 databases, external copies, or previous exports. Same-user malware can still read application-owned files; local permissions are not encryption. Automatic capture, analysis, and UI remain outside this brief.
