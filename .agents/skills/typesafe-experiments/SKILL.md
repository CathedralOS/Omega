---
name: typesafe-experiments
description: Tune and evaluate TypeSafe/Jev judgments in Omega agent workflows, or continue the context-selection and evidence-navigation experiments. Use for model experiments, not routine compiler development or proof approval.
---

# TypeSafe experiments in Omega

Find a decision where semantic understanding earns its cost, then improve the
consuming workflow. Jarod wants iterative tuning toward a substantial practical
win, not a catalogue of generic model limitations or a favorable benchmark alone.

Read [the experiment record](references/experiment-record.md) when continuing this
work: it preserves the conversation's decisions, measured results, and artifact
locations. This skill supplements the general `typesafe-ai` skill; use current
TypeSafe documentation for API details, with the saved local docs as fallback.

## Tune the actual decision

Work backward from what the caller should do differently. Keep exact comparisons,
execution status, revision identity, and authorization in code. Candidate retrieval,
semantic relevance, evidence linking, and suggestions for review are useful trial
surfaces. A typed answer or concentrated probability is not proof of correctness.

Tune state, candidate coverage, question meaning, and downstream policy separately
when attribution matters. Independent questions can share state but cannot see one
another's answers. Include enough identities and relationships for the judgment;
do not send an entire code trace merely because it fits. Preserve tool receipts and
concise explicit rationales, not private model reasoning. The user reported a 32k
context window in this experiment; verify the active model's limit before scaling.

Treat tuning examples as development data. Freeze fresh cases, expected outcomes,
and the next acceptance rule before seeing answers. Once examined, those cases are
no longer fresh for further tuning. Synthetic controls are useful but must remain
separate from recorded work. Include irrelevant/no-match cases for retrieval.

## Measure the result that matters

Keep a deterministic baseline and, for coding assistance, a no-extra-context worker
baseline. Count selection overhead, worker execution, verification, and failed
attempts. Report clean model runs separately from operational totals. Cached prompt
tokens are not billed cost. Tool-call counts are not automatically wasted calls.

For expensive worker trials, settle payload behavior cheaply first. Freeze model,
permissions, context caps, and comparable required validation. Use isolated seeds,
fresh sessions, witnessed assertion failures, and trusted acceptance tests outside
worker control. Verify checkout population and permitted read/test commands before
launching the matrix. Keep all arms' mandatory repository instructions intact.

Inspect failed outcomes before tuning: absent evidence, truncated candidates,
ambiguous criteria, policy mistakes, model errors, and runner failures need different
repairs. Preserve attempts; do not retry wrong answers until they pass. Narrow
infrastructure retries need explicit accounting and a bounded stopping condition.

Use end-to-end improvements to justify integration. A promising retrieval score
earns a worker trial, not an automatic approval gate. No observed speedup authorizes
publication, automatic retry cancellation, or suppressing required checks.

## Keep knowledge portable

Maintain this project skill only in `.agents/skills/`. Put dated measurements in
the linked record, not permanent model-capability rules. Raw experiments live in
ignored `build/experiments/`; those paths are local evidence, not shipped fixtures.
Preserve enough sanitized results here to survive their loss. Do not commit keys,
raw session exports, or personal provider configuration. Reuse existing experiment
helpers when present; do not build a second orchestration framework.
