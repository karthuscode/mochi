# Mochi V1 knowledge and review model

Baseline: 1.0. Authority: evidence eligibility, five knowledge states, deterministic transitions and spaced review. Rule version: 1. State is concept-specific evidence, not a certificate or language mastery percentage.

## Evidence categories

| Kind | Meaning | Credit |
|---|---|---|
| Exposure | Valid priority concept in published debrief for an episode | EXPOSED only; once per session/concept |
| Practice | Any submitted self-check/response, including incorrect/assisted | Activity history; state floor PRACTICING, no delayed mastery credit |
| Recall | Correct/incorrect self-check completed independently ≥24h after the concept's first exposure or, if no exposure, first practice | Eligible delayed evidence if outcome is certain and question unseen this day |
| Challenge | Submitted primary-concept solution | Independent correct application credit only if reported independent, no hint/reveal, and rubric certain |

An evidence row represents one attempt or one exposure; a correct recall/challenge also counts as practice for display/derivation. No separate duplicate practice row is inserted. `independenceBasis=user_report` is always visible for challenges and applicable independent practice. Reading lessons, AI use, session count and dwell time are not skill proof.

Eligible evidence requires retained provenance, final grade `correct/incorrect`, no assistance/solution exposure, valid question, and matching concept. State derivation counts distinct UTC days with certain independent correct recall/application, not repeated clicks; scheduler advancement separately aggregates at most one outcome per concept/day. A previously revealed same question can be practiced but cannot become eligible again; use an alternate unseen cached question or explicitly authorized new generation. If none exists, offer practice without promotion.

Determine assistance and prior question exposure **at submission time**. Feedback that reveals the answer after this attempt does not retroactively invalidate its evidence, but makes subsequent attempts at that question practice-only. Persist hint/solution/feedback exposures before display, including exposures without a submission. Concept reset removes credit, not the fact that an answer was seen; it cannot make a known question independent again.

## State derivation

Recompute from retained evidence in chronological order; choose the highest supported state. Missing/removed evidence can lower state; mistakes do not silently erase established evidence. A concept reset imposes a progress-reset cutoff, preventing prior lessons/regeneration from restoring old exposure/attempt credit.

| State | Deterministic support |
|---|---|
| `NEW` | No exposure and no submitted practice since latest reset |
| `EXPOSED` | ≥1 valid exposure; no practice |
| `PRACTICING` | ≥1 submitted attempt, including incorrect/assisted; DEMONSTRATED criteria not met |
| `DEMONSTRATED` | Certain independent correct evidence on ≥2 distinct UTC days separated by ≥24h; at least one is eligible delayed recall; every counted attempt occurred after latest reset |
| `STRONG` | DEMONSTRATED plus ≥4 distinct independent-correct days spanning ≥30 days, ≥2 eligible delayed recalls, and ≥1 certain independent-correct primary-concept challenge |

If someone answers a correct immediate quiz, they are PRACTICING, not DEMONSTRATED. A failed first answer also starts practice; the history states the outcome plainly. Only later independent evidence can demonstrate retention. STRONG includes a self-reported application requirement, so the UI explains its basis.

Exposure-free practice from a retained lesson after reset is allowed; first post-reset practice becomes the timing anchor. It cannot receive delayed credit on its first attempt. No artificial session is needed.

`needsRefresh=true` when the latest eligible delayed result is incorrect, or when a DEMONSTRATED/STRONG concept has no eligible correct evidence for 30 days. Time staleness never deletes achievement. Clear refresh on a subsequent eligible correct result. Incorrect review shortens the interval and leaves an honest visible refresh flag; historic demonstrated state remains unless its evidence is deleted or invalidated.

## Example progression

| Time | Action | State / due behavior |
|---|---|---|
| Day 0 | JWT observed in validated lesson | EXPOSED; due in 1 day |
| Day 0 | Correct independent immediate quiz | PRACTICING; due remains day 1 |
| Day 1 | Correct unseen delayed review ≥24h later | DEMONSTRATED if first quiz and review meet distinct-day rules; interval moves to 3 days |
| Day 4 | Wrong eligible review | State retains evidence; needsRefresh; interval steps back to 1 day |
| Day 5 | Correct independent review | Refresh clears; interval moves to 3 days |
| Day 31+ | Enough distinct-day evidence plus correct independent challenge | STRONG only when every threshold is satisfied |

This rule table is a specified V1 default, not a validated psychological measurement model. Evaluate usefulness with learners; avoid overstating certainty.

## Review scheduler

One `ReviewItem` per concept. Intervals indexed 0–4: **[1, 3, 7, 14, 30] days**. Due timestamps use exact 24h multiples in UTC, localized only for display. Inject the clock for tests. No randomness, opaque mastery percentages, or advanced adaptive algorithm.

| Trigger | Deterministic action |
|---|---|
| First valid exposure/practice | Create index 0, dueAt=anchor+1 day |
| Immediate/assisted/repeated practice | Keep interval/due; record history only |
| Eligible correct delayed recall | index=min(index+1,4); dueAt=attemptAt+interval[index] |
| Eligible incorrect delayed recall | index=max(index-1,0); dueAt=attemptAt+interval[index]; needsRefresh=true |
| Eligible independent challenge, even when delayed | Record application/state evidence; does not advance review interval unless it is explicitly the scheduled review task |
| Uncertain/ungraded/skipped review | Keep dueAt/index; explain status |
| Snooze | Explicit user preference hides reminder until chosen date; no evidence/interval advancement |
| Pause concept review | Keep schedule/history, omit from queue until resumed |
| Delete evidence | Chronologically replay scheduler from retained exposure/practice anchor and eligible review results |

For a scheduled challenge review, the same eligible delayed rules apply; record the attempt as challenge with scheduled-review provenance. One advancement per concept per UTC day, guarded transactionally. An incorrect eligible scheduled response on that day takes priority over an earlier same-day correct response for refresh/due calculation; do not farm interval increases by retrying. Canonical replay applies one daily scheduler outcome: incorrect if any eligible incorrect scheduled response exists, otherwise first eligible correct scheduled response. Immediate lesson questions qualify for delayed state evidence only if timing/unseen rules hold, but scheduling advancement is from an actual due review task.

Choose due unpaused concepts sorted by needsRefresh first, dueAt oldest next, then canonical key. Default daily batch ≤3 concepts or estimated 10 minutes; remaining due items stay visible. Notifications are optional, at most one review reminder per local day, never a streak penalty. V1 can review offline with cached MCQ/output questions; if only rubric questions exist, allow ungraded practice and defer eligible evaluation.

Generate alternate questions while authorized lesson generation is running when possible; no hidden remote call simply because a due queue opens. Cached question pool must contain at least one unseen delayed-review variant per selected concept in ready lessons, in addition to immediate self-check. When exhausted, explain practice-only or request authorized generation.

## Knowledge presentation

Display title/state, encounters, submitted practices, eligible correct review days, independent challenges, last review, due date, refresh indication and evidence history. Distinguish correct/incorrect/uncertain, immediate/delayed, and assisted/independent. Explain “DEMONSTRATED — independent correct answers on two days, including delayed recall.” Never show “JavaScript 87%” or translate model confidence into mastery.

## Invariants and verification

- Exposure idempotent per session/concept; attempts/evidence unique by submission owner.
- Single LLM grade cannot jump to STRONG.
- Immediate retries and solution reveals do not count as independent retained learning.
- Restart/replay yields identical state and dueAt.
- Delete/reset removes affected evidence, refreshes derived state and due queue, and prevents regeneration restoring pre-reset credit.
- UTC day boundaries, ≥24h spans, leap days/timezone changes and 30-day staleness are covered by deterministic fixtures.
- Counts shown in UI are derived from evidence, never model-generated.

Persistence details: [DATA_MODEL.md](../architecture/DATA_MODEL.md). Release scenarios: [VALIDATION_PLAN.md](../implementation/VALIDATION_PLAN.md).
