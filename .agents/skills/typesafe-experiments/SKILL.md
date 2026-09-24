---
name: typesafe-experiments
description: Tune and evaluate TypeSafe/Jev judgments in Omega agent workflows, or continue the context-selection, evidence-navigation, and architecture-binding experiments. Use for model experiments, not routine compiler development or proof approval.
---

# TypeSafe experiments in Omega

Find a decision where semantic understanding earns its cost, then improve the
consuming workflow. Jarod wants iterative tuning toward a substantial practical
win, not a catalogue of generic model limitations or a favorable benchmark alone.

Read [the experiment record](references/experiment-record.md) when continuing this
work: it preserves the conversation's decisions, measured results, and artifact
locations. When a semantic judgment is being turned into a shipped tool, read
[the deployment playbook](references/jev-deployment-playbook.md): the win pattern
(deterministic vocabulary limit + miss-cost asymmetry + advisory shape), the
question shapes that survived dogfooding (causal-over-classification, batch
contamination, why escape valves go unused), and the deployment contract every
shipped instrument shares. This skill supplements the general `typesafe-ai`
skill; use current TypeSafe documentation for API details, with the saved local
docs as fallback.

## Tune the actual decision

Jarod prefers example-first exploration: adapt the nearest official cookbook to
one concrete repository case, inspect what happens, then choose the next variation.
Avoid building a broad evaluation before a worked example earns it. Narrow output
does not require tiny input; compare context sufficiency separately from size.

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

For these owner-authorized local Devin experiments, Jarod explicitly requested
bypass/YOLO mode after repeated headless permission failures. Invoke Devin with
`--permission-mode dangerous` in the isolated experiment worktree; do not keep
patching individual harmless command allowlists. Use the same mode across new
comparison arms. This changes tool confirmation, not task scope: no live claims,
publication, unrelated edits or global permission-setting changes are authorized.
Preserve interrupted attempts and their costs when switching an existing trial.

Inspect failed outcomes before tuning: absent evidence, truncated candidates,
ambiguous criteria, policy mistakes, model errors, and runner failures need different
repairs. Preserve attempts; do not retry wrong answers until they pass. Narrow
infrastructure retries need explicit accounting and a bounded stopping condition.

Use end-to-end improvements to justify integration. A promising retrieval score
earns a worker trial, not an automatic approval gate. No observed speedup authorizes
publication, automatic retry cancellation, or suppressing required checks.

## Bind generated architectures to requirements

Jev cannot author Omega source (typed judgments only, no synthesized text),
but it binds candidate architecture to stated requirements: a generator
proposes elements, Jev judges each against the spec, code assembles and
`omega --check` arbitrates. Measured pipeline lives in
`build/experiments/arch-bind`, `arch-live`, `arch-corpus` (2026-09-19).

Question vocabulary that measured well: `bind::<element>` Choice over
requirements plus an explicit `none` criterion; `type::<element>` Choice
over code-supplied options (include the declared type, plausible
distractors, and bound/domain variants); `covers_data::<req>` and
`covers_prog::<req>` nouls; `copyable::<block>` noul. `covers_prog` reads
`program_source` and is the sleeper instrument: on damaged variants it
flagged the correct requirement every time, including order damage no
exit code could expose. Keep data-side and whole-program coverage as
separate questions — asking a data-side judgment about behavioral
requirements was the arch-bind v1 gate bug.

Interpret `none` verdicts as "unconstrained by the stated contract," not
"wrong to exist." Corpus programs legitimately carry fields no clause
names (utility carriers, slack); whether a `none` means orphan or merely
unconstrained depends on requirement-set completeness, which code must
judge — Jev does not see the completeness question. Drop only elements
that are both `none` AND code-unreferenced; used-but-unjustified elements
are review flags, never auto-drops. Scratch temporaries (`idx`, `tmp`,
anonymous accumulators) are the unstable element class — bound in one
experiment, dropped in another — so treat their verdicts as advisory.

Requirement sources, cheapest first: mechanically extracted corpus
clauses (`domain X requires pred`, `ensures`, `requires`, `[lo..=hi]`
bounds, `in Domain`, `[copy]` — free and gold-labeled); hand-authored
atomic requirements; generator-emitted requirements (trusting the
generator's decomposition is the risk). Type recovery is the strongest
measured value: propose stripped/plain types and Jev restored exact
constrained types from clause text alone — proof-relevant information the
generator did not emit.

Infrastructure: `omega run` on a fresh package requires the interactive
package-review ceremony (`update` emits pending decision rows; each run
regenerates them) — no headless execution path was found, so behavioral
ground truth came from `--check` plus construction arguments.
`omega_language_std` imports need a sibling `build.omg` package edge;
`omega::language::std` is the bundled single-file form. The local
`devin` CLI rejected headless dispatch despite completed OAuth login
(stored credentials in a format its dispatcher refuses) — subagent
workers are the proven fallback, with provenance noted.

## Claim-first citation experiments

For the retained citation-warning trial, have the answer producer emit claims
with their own exact citations in its normal generation call. Those claims ARE
the answer; do not append a separate unverified prose summary. Preserve subjects,
conditions and negation, and review query completeness separately: missing facts
cannot be detected by checking only the facts that were emitted.

Use [claim_checks.py](scripts/claim_checks.py) for the offline boundary. Its
`claim_cases(answers, tasks)` returns semantic cases and local warnings for
missing/nonliteral citations or abstentions; pass the cases through the existing
TypeSafe runner and unchanged support question. `unsupported_claims` validates
verdict coverage and retains every unsupported claim. Keep BOTH local and semantic
warnings; never average a defect away or treat no warnings as proof. Run the file
with Python 3 for its offline checks. It requires no SDK or credential.

Claim granularity is instructed, not guaranteed by the JSON shape. Keep this a
nonblocking experiment, not a publication gate. For an A/B, isolate competing
states: putting an explicit decomposition beside a whole answer may cue its judge.

## Repair evidence before regenerating answers

When the defect is citation support, try selecting existing source text while
preserving the claim. Read [evidence-only repair](references/evidence-repair.md)
for the retained recipe, abstention boundary and comparison requirements. The
speed opportunity is removing generation; lexical success must not be attributed
to Jev. False claims and missing evidence remain unresolved, not automatically fixed.

## Keep knowledge portable

Maintain this project skill only in `.agents/skills/`. Put dated measurements in
the linked record, not permanent model-capability rules. Raw experiments live in
ignored `build/experiments/`; those paths are local evidence, not shipped fixtures.
Preserve enough sanitized results here to survive their loss. Do not commit keys,
raw session exports, or personal provider configuration. Reuse existing experiment
helpers when present; do not build a second orchestration framework.
