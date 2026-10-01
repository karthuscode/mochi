# Brief 13 — Sanitized analysis bundle

Authorized by the 2026-10-01 internal CLI MVP task. Dependencies: 06, 09–12 and the local session controls. Implement only finalized-session preparation in a provider-independent learning crate. No send occurs here.

A bounded version-1 manifest selects event and immutable sanitized code evidence, carries coverage, omissions, unknown attribution and current episode/policy revision, excludes absolute paths and currently denied files, and hashes the canonical input. A prompt alone yields insufficient context. Input is at most 128 KiB / 24,000 conservatively estimated tokens. Preview the complete provider request, including instructions/schema, before an exact one-use, expiring consent. Any revision/exclusion change invalidates the preview.

Acceptance: synthetic secret/path canaries, bounds, meaningful-action minimum, stable hash, excluded files and stale-revision fixtures. No new filesystem content reads or raw payload access.
