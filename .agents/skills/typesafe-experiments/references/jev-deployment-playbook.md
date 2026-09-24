# Jev deployment playbook

Distilled from the 2026-09-24 campaign: five instruments shipped to main in
one session (~$0.02 total model cost). This file is the repeatable pattern;
the [experiment record](experiment-record.md) holds the measured history.

## The win pattern — where Jev pays

A Jev deployment is worth building when all three hold:

1. **A deterministic tool hits a vocabulary limit.** The check/judgment
   exists but the mechanical layer cannot express it (the test selector
   can't emit `--test` targets or binary invocations; `uncovered_mentions`
   sees literal path mentions but not prose-described surfaces).
2. **The miss costs more than the call.** Debug time, attribution
   archaeology, a pinned regression, a sibling collision — hours against
   ~$0.0005. If the miss costs minutes, fix the mechanical layer instead.
3. **The answer can be advisory.** Every shipped instrument adds signal and
   never gates. If the judgment must be authoritative, this is the wrong
   tool — Jev augments floors; it doesn't replace them.

Where to hunt: coverage gaps a selector can't express, attribution over
recent history, "is this diff safe to pin" decisions, manifest fences vs
prose scope, doc claims vs code reality. The generalization that produced
the campaign: *name the asymmetry, then let the repo's vocabulary gaps
enumerate themselves.*

## Question shapes that work

- **Descriptions say what BREAKS the thing, not what it reads.** The single
  biggest recall lever in test selection: rewriting catalog entries from
  "reads X" to "breaks when X" moved a detection from miss to flag (0.62).
- **Batch contamination is real.** One dangerous member poisons sibling
  labels — timing-only diffs scored `intended` at confidence 1.0 alone but
  `accidental` beside a dangerous diff. Batch for the aggregate verdict;
  send solo calls for localization that must be clean.
- **Ask the causal question solo.** A solo intended/accidental re-ask
  resolves "unexplained by this commit" as "intended" — the dangerous case
  escaped. "Could THIS commit plausibly produce this diff?" caught it
  (0.43). Causality, not classification, localizes.
- **Jev never hedges through the escape option.** It answered `distinct` on
  every genuinely-ambiguous dedup pair rather than `needs_review`.
  Decisiveness is a failure mode: don't offer escape valves expecting use;
  measure whether decisive answers are defensible instead.
- **`unverifiable` on partial evidence is honest, not failure.** It refused
  to confirm a claim whose evidence excerpt omitted the deciding call —
  correct epistemics. Treat hedge-answers as "gather more evidence," not
  misses.
- **It out-labels weak expectations.** Four times this campaign Jev's answer
  was more defensible than the frozen label (crate choice, scoped
  cross-board items, truncated evidence). Before scoring a miss, check
  whether the model is right.
- **Ask "would the mechanical check already be correct?" when deciding
  replace-vs-augment.** High = the deterministic rule is already right and
  the judgment should augment or skip it; low = the mechanical rule is a
  judgment call wearing a costume and is a replacement candidate. Our
  augment-only deployments never needed this question — use it before
  proposing a swap.

## The deployment contract

Every shipped instrument shares these; deviation needs a reason:

- Advisory only — annotates, never gates, never auto-records.
- Silent without `TYPESAFE_API_KEY`; `OMEGA_JEV_OFFLINE=1` suppresses.
- Every failure mode (network, timeout, malformed response) degrades to
  the deterministic baseline intact.
- Union-only where it augments an existing selector — adds coverage, never
  removes a baseline command.
- Cache by request payload where the call repeats (test-select cache under
  `build/test-select-cache/`).
- Refuse output if the response echoes the credential (proof_advisor's
  leak check).

## The pipeline

1. **Freeze expectations before any API call** — cases AND expected labels.
   Worked examples are development data; once examined they are no longer
   fresh for tuning.
2. **Worked example**: 3+ cases with real ground truth (witnessed failures,
   real manifests, hand-verified claims). Include the negative case
   (infrastructure failure, clean doc, unrelated pair) — anti-false-positive
   is the deployment-critical property.
3. **Dogfood live input before shipping** — real HEAD state, real manifest,
   real failure output. This is where the solo-re-ask and path-resolution
   failures surfaced; a passing worked example alone would have shipped
   both bugs.
4. **Deploy or defer honestly.** Deferred deployments are legitimate
   outcomes — board dedup proved zero-false-merge precision but the board
   was clean by same-day consolidation; shipping would be machinery
   without a customer. Record the recipe instead.

## Shipped instruments (2026-09-24)

| Surface | File | Answers |
|---|---|---|
| Pre-landing test selection | `tools/test_affected.py` (`jev` block) | "did I run the checks that catch this?" |
| Post-landing attribution | `tools/triage_advisor.py` | "which commit/layer broke this?" |
| Golden pinning | `tools/corpus_gate.py --jev` | "is this diff safe to `--record`?" |
| Wave launch fences | `tools/swarm/launch.py` (`jev_fence` hint) | "does the fence cover the real edit surface?" |
| Doc claim audit | `tools/spec_drift_advisor.py` | "does this claim still match reality?" |

Proven-deferred: board dedup (recipe in experiment record). Falsified:
Jev-alone scoping for test-time savings — it hedges to run-everything
exactly where savings live. The win is debug time, not test time.

## Hunting judgment-call surfaces mechanically

Before asking a model to read files, grep the shapes where code makes a
judgment it can't prove: string-`contains` chains and keyword-list
membership, `match` on `&str`, early-return classifier chains,
edit-distance/fuzzy matching, `..n` take-caps guessing "the best N", and
hardcoded thresholds guarding behavior. These are where a semantic
judgment can replace or augment a brittle rule — the same places the
campaign's instruments already live.

If an instrument ever gains a write path (none should without an explicit
decision): skip files with the user's uncommitted edits, back up before
writing, run the project's own checks before AND after, re-apply changes
one at a time and keep only the ones that pass, and keep an undo path.
