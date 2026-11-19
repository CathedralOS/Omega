---
name: typesafe-experiments
description: Tune and evaluate TypeSafe/Jev judgments in Omega agent workflows, or continue the context-selection, evidence-navigation, and architecture-binding experiments. Use for model experiments, not routine compiler development or proof approval.
---

# TypeSafe experiments in Omega

Find a decision where a model's judgment is worth its cost, then improve the
workflow that uses it. Jarod wants repeated tuning toward a large gain measured
end to end. Jev is TypeSafe's judgment model.

When continuing, read [the experiment record](references/experiment-record.md)
for decisions, results and artifact locations. When turning a
judgment into a shipped tool, read
[the deployment playbook](references/jev-deployment-playbook.md): what made
tools pay off, which question forms held up, and the rules every such tool
follows. This skill supplements the general `typesafe-ai`
skill; use current TypeSafe documentation for API details, with the saved local
docs as fallback.

## Tune the actual decision

Jarod prefers to start from an example: adapt the nearest official cookbook
recipe to one repository case, look at the result, then vary it. Build a broad evaluation only after a worked example justifies it. A
narrow answer does not need a tiny input; judge context by sufficiency, not size.

Work backward from what the caller should do differently. Keep exact
comparisons, execution status, revision identity and authorization in code.
Try the model on finding candidates, judging relevance, linking claims to
evidence and suggesting reviews. A typed answer or confident probability does
not prove correctness.

To learn what caused a result, tune input state, candidate list, question
wording and downstream policy separately. Independent questions can share
input but not see each other's answers. Send the names and relationships the
judgment needs, not a whole code trace. Keep tool output records and short
rationales, not private model reasoning. Check the context limit before scaling.

To review a whole Omega machine, ask one Noul per explicit behavior
requirement over the same source. One "is anything violated" Noul plus a Choice
naming the culprit flags correct programs. A state-indexed graph was no more
accurate than raw source, so do not build a graph extractor. Count the cost of getting
requirements and checking runtime behavior before calling it a gain.

Treat tuning examples as development data. Fix new cases, expected outcomes
and the next pass/fail rule before seeing answers; once examined, the cases are
no longer fresh. Keep synthetic controls separate from recorded work. For retrieval, include cases with no match.

## Measure the result that matters

Keep a no-model baseline and, for coding help, a worker given no extra
context. Count context selection, worker execution, verification and
failed attempts. Report clean model runs separately from operational totals.
Cached prompt tokens are not billed cost. Tool calls are not automatically
waste.

For expensive worker trials, first settle cheaply what the extra context does.
Fix model, permissions, context limits and required validation across variants.
Use isolated seeds, fresh sessions, assertion failures you have seen, and
acceptance tests the worker cannot change. Before launching, check each
checkout's files and the allowed read and test commands. Keep the repository's
mandatory instructions in every variant.

For these owner-authorized local Devin experiments, Jarod explicitly asked for
bypass ("YOLO") mode after repeated headless permission failures. Invoke Devin with
`--permission-mode dangerous` in the isolated experiment worktree; do not keep
patching individual harmless command allowlists. Use the same mode in every new
variant. It removes confirmation prompts, not task limits: live claims,
publishing, unrelated edits and global permission changes stay unauthorized.
When switching an existing trial, keep its interrupted attempts and costs.

Inspect failures before tuning: missing evidence, cut-off candidates, unclear
criteria, policy mistakes, model errors and runner failures need different
repairs. Keep every attempt and result; count and cap infrastructure retries.

Only an end-to-end gain justifies adding a tool to the workflow; a promising
retrieval score justifies a worker trial. Publishing and required checks follow
normal repository rules whatever the measured speedup.

## Bind generated architectures to requirements

Jev returns typed judgments, not Omega source text. Code can build source from
a finite set of grammar or rewrite choices the caller supplies with a renderer;
that does not show the model can implement correctly on its own. In the kept
pipeline, a generator
proposes program elements, Jev judges each against the spec, code assembles
the program, and `omega --check` decides. The measured runs are in
`build/experiments/arch-bind`, `arch-live`, `arch-corpus` (2026-09-19).

Questions that measured well: `bind::<element>` Choice over
requirements plus an explicit `none` criterion; `type::<element>` Choice
over code-supplied options (include the declared type, plausible
distractors, and bound/domain variants); `covers_data::<req>` and
`covers_prog::<req>` nouls; `copyable::<block>` noul. `covers_prog` reads
`program_source` and proved most useful: on damaged variants it named the right
requirement every time, including reordering no exit code reveals. Keep data
and whole-program coverage as separate questions; asking the data question
about behavior was the arch-bind v1 pass/fail bug.

Read a `none` verdict as "no stated requirement constrains this," not
"this should not exist"; real programs have fields no clause names. Whether
`none` means unneeded depends on whether the requirement list is complete,
which code must judge. Drop an element only
if its verdict is `none` AND no code references it; flag for review, never drop,
an element code uses that no requirement justifies. Scratch temporaries (`idx`, `tmp`,
unnamed accumulators) get inconsistent verdicts; treat them as advice only.

Where requirements come from, cheapest first: clauses extracted mechanically
from existing programs (`domain X requires pred`, `ensures`, `requires`, `[lo..=hi]`
bounds, `in Domain`, `[copy]` — free and already correctly labeled); single requirements
written by hand; requirements the generator writes (risky: you trust its split).
Type recovery measured best: given plain types, Jev restored the exact
constrained types from clause text alone, which proofs need and the generator
omitted.

Infrastructure (2026-09-19): `omega run` on a new package required interactive
package review (`update` wrote pending decisions; each run regenerated them),
with no headless route, so behavior was judged from `--check` plus how the
program was built.
`omega_language_std` imports need a sibling `build.omg` package edge;
`omega::language::std` is the bundled single-file form. The local
`devin` CLI refused headless runs after OAuth login (its stored credentials
were in a format its dispatcher rejects); use subagent workers and record that.

## Claim-first citation experiments

In the kept citation-warning trial, the answering model emits claims, each
with its own exact citation, in its normal generation call. Those claims ARE
the answer; do not add an unchecked prose summary. Keep each claim's subject,
conditions and negation. Check separately that the answer covers the whole
question; checking emitted facts cannot find missing ones.

Use [claim_checks.py](scripts/claim_checks.py) for the checks that run without a
model. Its
`claim_cases(answers, tasks)` returns semantic cases and local warnings for
missing/nonliteral citations or abstentions; pass the cases through the existing
TypeSafe runner and unchanged support question. `unsupported_claims` validates
verdict coverage and retains every unsupported claim. Keep BOTH the local and the model's
warnings; never average a defect away, and never treat no warnings as proof. Run the file
with Python 3 for its offline checks. It requires no SDK or credential.

The prompt asks for one fact per claim; the JSON shape does not enforce it.
Keep this a nonblocking experiment, not a publication gate. In an A/B test,
keep the variants' inputs apart: a claim breakdown beside the whole answer may
sway its judge.

## Repair evidence before regenerating answers

When a citation does not support its claim, try selecting better existing
source text while keeping the claim. Read
[evidence-only repair](references/evidence-repair.md) for the recipe, when to
abstain and how to compare. The speed gain is skipping generation; do not credit
Jev for a fix plain text matching made. False claims and missing evidence stay
unresolved.

## Keep knowledge portable

Maintain this project skill in `.agents/skills/`. Dated measurements go in the
linked record. Raw experiments live in ignored `build/experiments/`; keep enough
sanitized results here to survive their loss. Keys, raw session exports,
and personal provider configuration stay out of the repository. Reuse the
existing experiment helpers.
