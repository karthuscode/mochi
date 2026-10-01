# Mochi V1 learning model

Baseline: 1.0. Authority: reconstruction, concept selection, debriefs, assessment generation and grading. Knowledge transitions belong to [KNOWLEDGE_MODEL.md](KNOWLEDGE_MODEL.md); remote consent belongs to [security](../security/PRIVACY_SECURITY.md).

## Principles

Teach from the user's actual project evidence. Prioritize a few transferable ideas, ask the learner to retrieve/explain/apply them, and distinguish exposure from demonstrated understanding. Avoid a wall of summaries, unsupported claims, dependency-name trivia, or treating AI-generated code as learner skill.

The pipeline is staged, typed, validated, and revisioned. An LLM assists reconstruction and explanation; deterministic core logic handles limits, ranking, evidence eligibility, knowledge transitions, and scheduling.

## Analysis input contract

`LearningAnalysisInput` version 1 contains:

- `sessionId` and `inputRevision` as opaque identifiers.
- Safe project alias, coverage/capabilities, observed episode duration if known, stop reason, attribution warning.
- Sanitized selected prompts/actions/errors/test results, each with an event reference.
- Filtered before/after changes and relevant source excerpts, each with immutable code evidence ID, relative path, snapshot line/hash, truncation status.
- Concept registry candidates/aliases and minimal knowledge state/counts for ranking; no full unrelated history.
- Explicit omissions and unknowns, target lesson language (English V1), stage contract version.

No absolute paths, API keys, raw provider payloads, unrelated project files, binary data, excluded content, configuration backups, or hidden reasoning transcripts. Limits: ≤128 KiB serialized UTF-8 and ≤24,000 estimated tokens, lower if provider/model budget requires. Include enough space for instructions/output. Evidence-preserving reduction order: remove repetitive activity/output, shrink code excerpts, reduce low-impact diffs; never erase uncertainty labels. Too little useful evidence results in `insufficient_context`.

## Pipeline contracts

| Stage | Output / validation |
|---|---|
| 1. Local preparation | Finalized evidence manifest, sanitized bundle, policy/consent/revision snapshot |
| 2. Technical reconstruction | Goal; observed changes; decisions; failures; fixes; unresolved issues; each claim with references and `observed/inferred/unknown` confidence category |
| 3. Candidate extraction | 0–5 concepts; canonical key/alias proposal; short meaning; evidence references; relevance/importance/impact estimates |
| 4. Local ranking | Registry resolution, 5 score components, deterministic ranked 0–3 priorities |
| 5. Debrief generation | Required sections and concept cards grounded in provided evidence |
| 6. Assessment generation | ≥1 self-check per priority plus exactly one mini challenge when priorities exist |
| 7. Validation/publication | Strict schemas, reference resolution, count/length checks, answer/rubric validity, output sanitizer; atomic publish and exposure |

Stages may be combined into an API request after evaluation, but their typed outputs/validation boundaries remain. Persist version/model/input hash for reproducibility, not full raw API debug logs. Refusals, incomplete output, malformed JSON, and invalid provenance are distinct failures; strict structured outputs do not replace local semantic checks. Official [Structured Outputs guidance](https://developers.openai.com/api/docs/guides/structured-outputs) describes schema-constrained output and separate refusal handling; Mochi additionally validates factual references and teaching constraints.

Model content is data. No tools, browsing, repository edits, command execution, or permission escalation are enabled in the analysis request. Treat prompts, code comments, command output, and provider messages as quoted untrusted context. Do not disclose system instructions or secrets because captured text asks for them.

## Evidence and reconstruction

Git before/after shows observed changes; provider events show reported intent/activity. Neither proves the user's understanding. Never assert tests passed without reliable result evidence; an agent saying “fixed” is not a verified outcome. For inferred rationale, use “likely” and distinguish it from an explicit observed decision.

Minimum useful evidence: one material change or reliably observed technical action plus enough sanitized context to explain a concept. A prompt alone cannot prove code was implemented. Zero concepts is correct for empty/excluded/unsupported work. Full root/project context is never collected merely to improve a weak lesson.

Each selected concept needs ≥1 valid session evidence reference and a meaningful project connection. Code quotations must match sanitized snapshots; generated variants are labeled “practice example”, not “from your project”. If a cited source expires after publication, retain the sanitized lesson copy and show the snapshot date.

## Concept identity and ranking

Use a small versioned canonical registry with stable concept keys (for example `javascript.async-await`, `concurrency.race-condition`, `react.context`, `security.jwt`). Resolve exact keys/known aliases locally before a new key is accepted. New keys require valid namespace, concise definition, evidence, and duplicate check. A package name is not automatically a useful concept. V1 uses a concept list/history, not a complex knowledge graph or course prerequisite engine.

Score each dimension in [0,1]:

| Component | Weight | Source |
|---|---|---|
| Novelty | 0.20 | NEW=1, EXPOSED=.8, PRACTICING=.5, DEMONSTRATED=.2, STRONG=.1 |
| Relevance | 0.25 | Validated candidate estimate: direct connection to goal and evidence |
| Conceptual importance | 0.20 | Validated candidate estimate: transferable principle over package trivia |
| Session impact | 0.20 | Validated estimate: affects meaningful behavior/change/fix |
| User weakness | 0.15 | Eligible incorrect in last 30 days=1; otherwise NEW=.8, EXPOSED=.7, PRACTICING=.5, DEMONSTRATED=.2, STRONG=.1 |

`learningValue` is the weighted sum, rounded to 4 decimals. It is a selection score, never user mastery. Exclude candidates with no valid references or relevance <.5. Select at most 3 with score ≥.45; ties break by canonical key. If fewer qualify, teach fewer. Store components/reasons so selection is explainable. Model estimates need not pretend to be objective learning proof; evaluate them with the alpha rubric.

## Debrief structure

Every ready analyzed session has:

1. Session overview and evidence/coverage note.
2. What you built/requested versus what was actually observed.
3. What changed, with evidence-backed attribution.
4. Important decisions and alternatives, with inference labels.
5. What went wrong, what fixed it, and what remains unresolved (explicit “not observed” when absent).
6. 1–3 priority concept cards: definition, project code, why it works, why it matters, common misconception, self-check.
7. One “Try it yourself” mini challenge.

Target reading time 5–8 minutes. Each concept explanation is approximately 150–250 words; focus on clarity rather than padding. No generic multi-page curriculum. At zero qualified concepts, publish an `insufficient_context` summary/status without fake assessment/challenge or exposures.

## Assessment contracts

All assessments are immutable once attempted and tied to concept IDs/evidence/debrief revision. Never present an answer before first submission unless learner chooses reveal. Hints/reveal are allowed but mark assisted. All forms have a clear expected answer/rubric and feedback explaining the underlying idea.

| Kind | Required contract | Evaluation |
|---|---|---|
| `mcq` | 3–4 uniquely identified choices, exactly one correct choice, explanation for distractors | Deterministic local exact-choice check |
| `predict_output` | Small bounded snippet, stated runtime assumptions, expected output, narrow normalization rules | Deterministic local comparison; if nondeterministic/ambiguous, reject question |
| `explain_why` | One focused question; 2–4 rubric criteria; misconception/failure criteria | OpenAI advisory rubric evaluation; uncertainty preserved |
| `challenge` | One primary concept, 5–10 minute task, constraints, starter text if useful, 2–4 rubric criteria, solution/explanation | Text-only rubric evaluation; no execution or correctness claim from tests |

Each priority has at least one self-check. A ready lesson with suitable evidence includes at least one predict-output or explain-why question; V1 must demonstrate all self-check kinds across its acceptance fixtures. One shared challenge has a primary concept; secondary concepts are context and cannot all receive independent-application credit from a single task.

Before publishing MCQ/output questions, verify uniqueness, assumptions, and expected result in controlled authoring fixtures. Generated snippets never run automatically on user machines. Question quality evaluation must reject ambiguous answer keys instead of marking the learner wrong.

## Attempts and grading

Answer text cap 16 KiB for explanations, 32 KiB for challenge; sanitize before any attempt write. Reject rather than silently truncate semantically important submitted answers. Track `independent/assisted/unknown`, solutionSeen, local/AI grading method, and pending/final status. “Independent” means the user reports working without AI/hints/solution; it is not surveillance-verified.

AI rubric output has criteria pass/fail/uncertain, overall `correct/incorrect/uncertain`, numeric graderConfidence [0,1], short feedback, and misconception flags. Confidence <.8 or any uncertain required criterion yields `uncertain` regardless of the model's overall label. Correct requires every required criterion pass and no blocking misconception. Incorrect requires a clear failed criterion. Confidence is a grader signal, not a learner score.

Wrong answers receive explanation plus retry. Same-question immediate retries are practice, not separate delayed evidence. Uncertain/ungraded attempts do not promote state or advance review; allow revision/additional local check. Users may flag poor feedback or self-assess, but manual “I know this” is not qualifying recall evidence.

Remote grading has the same consent requirements as generation: disclosure includes learner answers, sanitized challenge text, and question/rubric. Automatic project analysis approval covers only the disclosed generation/grading categories. Without permission/network, local question types work; text rubric attempts remain ungraded and can be graded later only after consent.

## Quality validation

Evaluate reconstruction, grounding, relevance, explanation accuracy, ambiguous questions, correct and incorrect answer grading, unsupported source claims, and injection resistance. Thresholds and meaningful fixtures: [VALIDATION_PLAN.md](../implementation/VALIDATION_PLAN.md). Never publish a lesson merely because JSON parses.
