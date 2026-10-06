---
name: advance
description: >-
  Own one Omega compiler or development-health outcome from the execution boards or
  an existing customer example. Use for advance, continue the compiler, work the
  boards, or pick the next compiler task. Not for a named bug fix, code question,
  review, or repository-wide health check.
---

# Advance the Omega compiler

Own one outcome through demonstrated acceptance and publication.
Follow [AGENTS.md](../../../AGENTS.md) for repository ownership,
validation, board rules, and publication.

## Choose and retain the outcome

Record the start time, starting checkout, and branch. Fetch main and inspect
status and recent lane commits. If the starting checkout is clean and on main,
fast-forward it with `git merge --ff-only origin/main` before reading its boards.
Otherwise preserve it and read from an isolated worktree based on fetched main.

For a continuation, retain the same outcome until acceptance, explicit handoff,
or a user priority change. Missing implementation is part of the assignment,
not a reason to move elsewhere. Without prior continuity, choose from
`TASKS.md`, `TASKS_BOOTSTRAP.md`, or `TASKS_OPTIMIZER.md`; honor a named board.
Choose a general capability that closes release requirements; real programs
and relevant invalid controls demonstrate it. Squalr and Cathedral are customers,
not source shapes to special-case or rewrite around gaps. After a completed
round or explicit handoff, measured iteration performance or test-environment
friction may be the next owned outcome. Name its workflow, baseline and adequate
target first; a minor incidental slowdown is not permission to abandon a feature.
Check live assignments with `python tools/claims.py status`. Claim your item
(`claim --board <board> --item <item> --owner "<label>" --path <paths>`), renew
a long task before its lease expires, and release it when finished for any
reason, recording the remaining frontier if interrupted. Same-outcome ownership
requires a handoff; overlapping paths do not. Inform peers where practical,
proceed in isolation and reconcile at landing. A named board limits selection
to that board; an explicitly named
program limits it to that program and the dependencies it needs to run. State
the outcome, missing dependencies and acceptance command.

Keep that short assignment in the claim note or session handoff: outcome,
acceptance, current failure, next necessary action and dependencies. Reread it
after compaction and before changing direction. A new obstacle updates this
frontier; it does not automatically create a task. Split genuinely independent
work only with an integration DRI. Do not mine tasks to keep a slot occupied.

Choose the capability by its source-to-execution acceptance: "a helper mutates
the caller's field and the caller observes it after return," stated as the
program's observable behavior. Trace the remaining producer and consumer
dependencies before the first repair. Own the outcome through the program
running; when the next missing stage appears, that stage is the next step of
the same assignment. Intermediate code checkpoints are fine; the assignment
ends when the capability's acceptance passes, not merely one customer subcase.
For development-health work, stop at the agreed usable workflow rather than
collecting every hotspot or expanding test matrices.

Read the owning design and [completion plan](../../../wiki/drafts/reference/rust_compiler_completion.md).
For bootstrap work, also read [whole-chain minimization](../../../bootstrap/MINIMIZATION.md)
and apply [scope and escalation](../../../AGENTS.md#scope-and-escalation).

## Work from a should-be-working example

For a development-health outcome, the example is the obstructed edit/build/test
workflow and its measured acceptance, not a fabricated program failure. Apply
the investigation and validation steps below to that workflow; do not require
unrelated native/interpreter gates just to fit the compiler-work template.

Resolve unfamiliar paths with `rg --files <existing-directory> -g '<pattern>'`.
For content searches, pass an existing directory and use `-g` for filename
patterns; wildcard path arguments such as `src/values*` fail under Windows
PowerShell. This form also works in macOS shells.

Locate relevant headings or symbols before reading large boards or source files,
then read their complete relevant sections within the output budget. Avoid dumping
all boards together. If output is truncated, retrieve the missing relevant section
before relying on it; do not treat an omitted section as absent evidence.

Use the program's actual `omega run` command as the outer loop, including
package preparation and execution where applicable. Trace expected output and
exit status backward through the producer and consumers. Existing logs and one
focused probe locate the first failure.

The language and architecture govern the solution. An example exposes a missing
capability; the design defines its limits. Implement the smallest capability
consistent with the design so that the general case runs, not the one spelling.

Witness the failure before editing. Read existing diagnostics before
instrumenting compiler code. Report the observed boundary, and report a
checked-tree milestone as exactly that when native production is still ahead.
On a proof-rejection diagnostic (`cannot prove`, `disproved`, `no entailment
tier`, `structurally false`), `python tools/proof_advisor.py --omega
<binary> <root.omg>` annotates it with a Jev contradiction-vs-capability
classification — advisory only: it orders investigation, never discharges
anything, and admission fences stay out of its scope by design.

Apply [compositional lowering](../../../omega-rust/pipeline.md#compositional-lowering).
If supported operations fail only because of their surrounding arrangement,
repair the shared sequencing or semantic join and retire the replaced
recognizer. Carry the same program across the repair. Preserve language
ownership, effect, mathematical proof and ABI obligations; do not resurrect
the independently replayed PCC checks retired by the owning tasks.

Carry the reproduction and source findings forward across handoffs.

Keep the outer command in the sample's README or owning design document and the
current first failure in its existing board item. After a milestone, rerun the
command. The program running is the result; the program's source stays as
authored.

## Implement and validate

Use an isolated worktree under `<repository>/.codex/worktrees/<short-name>`
created from the fetched `origin/main`, not from the starting checkout's HEAD
or local `main`: a checkout that could not fast-forward leaves local `main`
behind, and a stale base rereads landed work and lengthens the landing rebase.
A continuation retains its existing worktree base. Keep its name short; if a
generated Windows path fails, measure the actual path before relocating work or
changing compiler architecture.

Read the existing module entrance and decision-site explanations before tracing
the shared implementation and callers, preserving the Psi/Omega firewall
and proof, custody, and trust checks. Use the existing representation and
provider route. A single bounded change needs one agent.

For an explicitly assigned discoverability/architecture change, apply
[Discoverability architecture](../../../omega-rust/pipeline.md#discoverability-architecture):
the outcome is a clearer route from crate entry to a named operation. Compare
the route with [main.rs](../../../omega-rust/omega/application/cli/src/main.rs) and
[compiler.rs](../../../omega-rust/omega/compilation/driver/src/compiler.rs), keep
orchestration visible at the crate entry and at subordinate multi-step owners,
and verify affected test selectors and source readers. Stop when the named
navigation problem is resolved.

Select checks from the affected behavior under
[validation scope](../../../tools/testing.md#validation-scope). A bug fix needs a witnessed
regression. Read the harness before filtering: fixture-path filters and nextest
test-name filters select different things. For e2e-visible changes — diagnostics,
pipeline stages, acceptance behavior — `python3 tools/corpus_gate.py --filter
<domain>` diffs the touched fixture domain against the recorded corpus golden in
seconds-to-minutes; prefer it to chaining per-crate nextest suites as the first
regression signal. Include affected crate and integration
checks. Reuse successful results on unchanged inputs, including at landing.

Before landing, challenge the assumptions that make this change correct, even
when working alone. For each assumption the change's correctness depends on,
trace where the code establishes it and construct a realistic source case that
could falsify it. Exercise that case through the public entrypoint where
practical, choosing checks from the affected behavior and consumers.

Review the premise that justified a decision, not just whether the code follows
that premise. For example, a call spelled `abs` need not select the builtin:
a callback with the same name challenges that assumption, and resolution must
establish the callable's identity before a builtin rewrite. Check that the
mechanism handles the general operation rather than only the reproduction's
spelling or arrangement.

Passing existing tests establishes their covered cases, not untested premises.
Report which assumptions were challenged, the observed evidence, and any that
remain unverified. If the change's correctness still depends on an unsupported
assumption, resolve it or name the missing dependency before claiming completion.

Attribute unexpected failures using a focused baseline comparison on the base
commit. A failure that reproduces on the base is outside this change; record
its command and revision. A failure introduced by this change blocks landing.
Use [testing](../../../tools/testing.md) for scope and
[test-cycle measurements](../../../wiki/drafts/measurements/test_cycle_measurements.md) for build diagnosis.
Run one host build at a time.

## Preserve reasoning on the common reading path

When this change required reconstructing a non-obvious design decision, write
the verified explanation beside the implementation before landing: the
problem, the constraints, why the mechanism fits, and any rejected alternative
or revisit condition that would change the next edit. Check recent commits and
tests against current code and the owning contract before citing them.

Put the module-wide explanation at the top of its natural entry file and a
local invariant at the decision site. Explain what the code alone does not
reveal: why passes run in that order, why a reusable primitive is safe here,
why an apparent simplification would lose required evidence. The reasoning
lives inline in the source; links to deeper contracts supplement it.

Before landing, check that an agent opening the obvious file meets the
reasoning this invocation had to reconstruct. Add what is missing in the
touched design area and update what is stale; existing comments that already
answer the question stay as they are.

## Delegate a bounded assignment

Follow [delegation](../../../AGENTS.md#delegation). Give the worker its exact
worktree/revision and canonical skill path, objective, design anchors, edit
ownership, dependencies, acceptance, and escalation conditions.

For discoverability assignments, include the reader's question, the affected
entry-to-operation route, and the orchestration requirement above.

Give each worker a distinct question or edit responsibility. Independent review
challenges assumptions and checks correctness.

The coordinator owns board updates from returned evidence. Worker evidence
reaches the board through `python tools/claims.py note --ticket <ticket>
--text <finding>` on its claim ticket plus the session report.

Use the actual agent tool and returned ID before reporting a worker as launched.
Monitor through completion, respond to findings, and inspect the returned diff
and evidence before integration. Worker IDs stay in the conversation.

Evidence identifies revision, host, command, exit, and remaining dependencies.
The assigned acceptance is the required capability demonstrated on the target,
or the measured development workflow restored to its agreed bound; continue
implementation when the design answers the next question, or hand off the exact
missing dependency and next acceptance.

## Land and restore the checkout

Invoking advance authorizes committing and publishing the change through
[the landing protocol](../../../tools/landing.md).

Validate before entering the queue. Follow the protocol for the local wait,
claim, rebase, exact verified candidate, and release on an affected failure.
Hold the lease only for the publish itself.

Read the entire owning item and apply [board update discipline](../../../TASKS.md#updating-the-boards).
Subtract completed requirements; do not replace them with a success narrative.
If work remains, retain its current failure, reproduction and host, owning
implementation, dependencies and acceptance. Keep a revision only when needed
to reproduce or qualify that unresolved observation. Preserve regression commands
that constrain the remaining repair, not a ledger of passing runs. Delete the
item only when its full acceptance is satisfied, not merely the chosen subcase.
Search all three boards for the repaired capability and affected program, and
reconcile stale references. Before committing, ask which unfinished requirement
the change removed; explain any growth by new unfinished work or necessary
context, not by the amount of work performed. Board edits ride with the code
change that earned them.

After publication, return to the recorded starting checkout and recheck its
branch and status. If it is still clean and on main, fetch and run
`git merge --ff-only origin/main`, then verify HEAD equals fetched `origin/main`.
This fast-forward creates no merge commit; the candidate rebase above integrates
local work. Never reset, auto-stash, switch branches, or rebase unrelated local
commits to synchronize the starting checkout. If synchronization cannot proceed,
report why and distinguish published remote main from the local checkout state.

Then remove the worktree and local branch this invocation created. From the
starting checkout, confirm the worktree is clean (including untracked files)
and its HEAD is an ancestor of fetched `origin/main`, stop any processes
running there, run `git worktree remove <path>`, then `git branch -d
<branch>`. Pre-existing worktrees, the starting checkout, and anything dirty
or unpublished stay; report a retained worktree and the reason. Verify the path
is absent from `git worktree list` and the branch is gone.

## When acceptance is not complete

Decompose cross-stage work into the smallest dependency that moves the same
program's observable behavior or a named proof obligation.

When blocked, name the attempted slice, the missing contract or unavailable
dependency, why existing mechanisms cannot deliver it, and the next executable
step. Language/architecture decisions go to `OWNER_QUESTIONS.md` with the
dependency referenced on the owning task; engineering choices are the agent's.

Do not switch assignments because a required file is shared, the next stage
is unfinished or a checkpoint landed. Continue necessary work. A genuine owner
decision, unavailable environment, budget limit or user stop may end this run;
preserve the exact frontier and handoff without marking the outcome complete.

Report the program's behavior before and after, commit/publication state,
checks run, and remaining dependencies. For development-health work report the
workflow's before/after measurements and preserved validation. Stop after the
owned outcome unless the user requested continuing work.

For skill maintenance, use the [evaluation guide](evals/README.md).
