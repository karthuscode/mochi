# Briefs 20–21 — Durable self-check

Dependencies: 17, 19, 03, 14. Generate an immutable self-check and an unseen delayed variant per selected concept. Support 3–4-choice MCQ, bounded deterministic JavaScript arithmetic/equality predict-output authoring fixtures, and 2–4-criterion explain-why. Reject invalid keys, duplicate choices, nondeterministic/unsupported prediction snippets and unsupported references. At least one selected question is prediction or explanation.

Submit bounded, sanitized answers transactionally using a caller attempt UUID for idempotence. Track self-reported independent/assisted/unknown and reveal state. MCQ and supported output grading are local/offline. Explanation grading is advisory, requires a separate exact answer/rubric send preview, and remains pending offline. Confidence below .8, uncertain criteria or blocking misconception preserve uncertainty. Retry is practice; no mastery or qualifying knowledge promotion is inferred. Questions, attempts and exposure records cascade with session deletion. Delayed variants are stored but no review scheduler is introduced.

Acceptance: double submission/replay, changed-ID reuse rejection, correct/incorrect/uncertain, reveal assistance, answer canary, reload/cascade and offline fixtures. No challenge execution or user repository edits.
