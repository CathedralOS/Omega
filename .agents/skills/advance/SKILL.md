---
name: advance
description: >-
  Take one failing Omega sample end to end: make it compile and run with its
  authored result, landing every compiler repair it needs. Use for advance,
  continue the compiler, or pick the next compiler work. Not for a named bug
  fix, code question, review, or repository-wide health check.
---

# Advance the Omega compiler

Make one sample run, through demonstrated acceptance and publication, following
[the sample loop](../../../AGENTS.md#the-sample-loop).
Follow [AGENTS.md](../../../AGENTS.md) for repository ownership,
validation, board rules, and publication.

## Choose the sample

Record the start time, starting checkout, and branch. Fetch main and inspect
status and recent lane commits. If the starting checkout is clean and on main,
fast-forward it with `git merge --ff-only origin/main` before reading its boards.
Otherwise preserve it and read from an isolated worktree based on fetched main.

For a continuation, keep the same sample until it runs and lands, unless the
user changes priority. Missing implementation is part of the work, not a reason
to move elsewhere. Without prior continuity, take work in this order:

1. A regression `tools/native_watch.py` reports on main, or a sample that ran
   before and no longer does: bisect it and repair it.
2. A sample the user names.
3. The smallest unclaimed failing sample from `python3 tools/sample_queue.py` (run
   `python3 tools/sample_native_gate.py --programs-only --find-work` first when no recent run exists).
   Several queued samples with one demonstrated root cause are one piece of
   work.
4. When the queue is empty, write a harder sample at the language frontier
   and take its failure.

Size the work to finish in this session. When the sample is too large to
trace, write a smaller sample that isolates its failing construct with its own
oracle, make that run and land it, then return to the large one. A sample that
passes on its first run is not work; take the next one.

Squalr and Cathedral are customers, not source shapes to special-case or
rewrite around gaps. `TASKS_BOOTSTRAP.md` and `TASKS_OPTIMIZER.md` work, when the
user names it, keeps its own acceptance. After landing a sample, measured
iteration performance or test-environment friction may be the next piece of
work. Name its workflow, baseline and adequate target first; a minor
incidental slowdown is not permission to abandon a sample.
Read the sample queue's live owners and notes before choosing unassigned work.
It ranks unclaimed samples first; an occupied sample remains visible. Use
`python tools/claims.py status` to read other live outcomes when their refusals
suggest a related root cause. Prefer another complete sample outcome already
outside the live sessions' work; shared implementation paths do not exclude it.
If every queued failure is already being pursued, write a harder distinct sample
as in step 4. A named sample, continuation or known main regression keeps its
priority even when another session claims it: read the note, reuse available repairs and reconcile at rebase.

Claim your sample (`claim --item <sample path> --owner "<label>" --path <paths>`)
before implementation. Inspect the registration's `overlaps`: if another session
claimed the same sample between selection and registration, release this ticket
and choose another unassigned sample before starting its repair. This preference
for distinct automatic assignments grants no veto and requires no permission;
named samples, continuations and known main regressions still proceed.
Overlapping paths are informational.
Renew long work and release it when finished for any reason, recording the
remaining frontier if interrupted. An unavailable registry means ownership is
unknown, never that every sample is free. Report that uncertainty and retain the
isolated scope required by AGENTS.md; do not claim registration succeeded.

Fetch and inspect incoming main after reducing the failure to its root cause,
before committing to a long repair or validation run. If another session already
landed the repair, verify that sample on main and use it rather than duplicating
its implementation. Repeat integration near publication. An explicitly named
program limits the work to that program and its necessary repairs. State the
sample, its oracle, the current failure and the acceptance command.

Keep that short record in the claim note: sample, acceptance, current
failure, next necessary action and dependencies. Reread it after compaction
and before changing direction. When resuming saved work after a crash or
compaction, rebase first and check whether main already
carries it (`git log origin/main -S '<distinctive line>'`) before reapplying a
patch or rerunning its validation. A new obstacle updates this
frontier; it does not create a task for someone else. Do not split work or
mine tasks to keep a slot occupied.

Choose the capability by its source-to-execution acceptance: "a helper mutates
the caller's field and the caller observes it after return," stated as the
program's observable behavior. Acceptance covers the capability's
variants, not one spelling: before landing, run mechanical variations of the
program -- the value through a local, a field, a parameter or a call result;
the operation in another state, machine or target body; an equivalent
spelling -- and make them work through the same repair. A variant that still
fails shows the repair recognized a shape instead of generalizing the
operation; fix the operation, never by adding the variant's shape. Trace the remaining producer and consumer
dependencies before the first repair. Own the outcome through the program
running; when the next missing stage appears, that stage is the next step of
the same assignment. Intermediate code checkpoints are fine; the assignment
ends when the capability's acceptance passes, not merely one customer subcase.
For sample-driven work, retain the complete application and necessary compiler
repairs in this outcome under [scope and escalation](../../../AGENTS.md#scope-and-escalation).
Group related witnesses by a demonstrated common capability or root cause;
pipeline stages and shared paths do not create worker dependencies.
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

### Trace a refusal with probes

Cut a refusal down before reading compiler code. Write scratch projects of a
few lines that keep only the failing construct, then vary one thing per probe:
the value through a field or a separate parameter, the operation in the same
state or across states, inside a loop or on a straight line. The passing and
failing probe that differ in one construct name the cause. Those probes later
become the fixtures and invalid controls that pin the repair.

Before repairing a checker refusal, decide whether it is a rule or a gap. Read
the spec and the comment at the decision site of the prover that declined; a
refusal can be deliberate (a state inside a loop proves from its own
`requires` contract, not from first-arrival facts). A rule changes the program
to the language's idiom, recorded in the sample header. A precision gap is a
compiler repair under the rules below.

A diagnostic that does not say where it failed (machine, state, statement) is
part of the work: fix it so it does, since the next agent meets the same
message. Temporary tracing goes behind an environment variable in an
`eprintln!` and is removed before commit.

Apply [compositional lowering](../../../omega-rust/pipeline.md#compositional-lowering).
If supported operations fail only because of their surrounding arrangement,
repair the shared sequencing or semantic join and retire the replaced
recognizer. Carry the same program across the repair. Preserve language
ownership, effect, mathematical proof and ABI obligations; do not resurrect
the independently replayed PCC checks retired by the owning tasks.

Apply the [law of completion](../../../AGENTS.md#law-of-completion).
Retain the reproduction, source findings and outcome context through complete
acceptance, required review and authorized verified main landing. Do not leave
necessary compiler/library stages for another agent; there are no handoffs.
Follow the concrete path through completion; let working programs reveal design.

Keep the outer command and the current first failure in the sample's README.
After a milestone, rerun the
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

Keep the loop fast. Run samples and the corpus with an optimized compiler in its
own target directory (`CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=<dir>
mbx build -p omega`), and close stdin (`< /dev/null`) on every direct `omega`
command you run in the background (the gates already do): `omega run` forwards
stdin and otherwise waits forever at zero CPU. A step that still takes minutes is a finding to
measure and remove, not a wait.

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
regression signal. The shared golden also carries other lanes' drift and other
hosts' rows, so its moves are a signal to attribute, not evidence against this
change; attribute them with `--compare` (below) before acting on them. Record
only the fixtures this change adds or moves (`--record --filter <fixtures>`),
which splices those rows into the golden and leaves the rest unchanged. `--jev` on the same run gives the advisory record-safety
verdict before any `--record`. Include affected crate and integration
checks. Reuse successful results on unchanged inputs, including at landing.
Qualify on a fixed recorded revision. Main advancing does not invalidate every
check: apply [main-change validation](../../../tools/testing.md#main-change-validation)
to the actual changed inputs, retaining the exact-SHA landing receipt requirement.
Use [qualification.py](../../../tools/qualification.md) to capture the reviewed
source/compiler/artifact closure and measured checks. Keep the qualification
checkpoint fixed, integrate main near publication outside the queue, then assess
and run only invalidated or required final-candidate checks. Reused observations
retain their actual producer/revision; final independent review and exact-SHA
landing verification are not reused across candidates.
Before a semantic repair, include the focused canonical passages themselves,
their source revision/blob hashes and the behavior-to-contract mapping in the
implementation and independent-review context (`qualification.py handoff`
with `--role implementation` or `--role review`). Save the packet; on resume
use `--role continuation --previous SAVED_PACKET_JSON` with the integrated
base to verify focused fingerprints. Without retained-delivery proof, include
the focused passages again. Repository-managed dispatch requires this context
by default; genuine non-semantic work records its classification and reason.
Distinguish missing implementation from an unapproved new rule; tests and a spec
clause authored in the same patch do not supply language authority. Missing or
stale context requires identifying the exact semantic gap before coding.

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

Attribute unexpected failures using a focused comparison on the base commit:
`python3 tools/corpus_gate.py --compare <base> --filter <domain>` builds the base
in its own worktree and lists every lost outcome, expected fragment and
execution fact. Compare against the commit your series sits on, never a newer
`main`. A failure that reproduces on the base is outside this change; record
its command and revision. A failure introduced by this change blocks landing.
For a failure already red on landed main, `tools/triage_advisor.py` (failure
output plus command -> suspect commit and layer) and `tools/jev_attribute.py`
(signature plus commit window -> ranked blame) order the investigation before
a bisect; advisory only, see [testing](../../../tools/testing.md).
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

## Delegate without fragmenting the sample

Follow [sessions and subagents](../../../AGENTS.md#sessions-and-subagents). Give the worker its exact
worktree/revision and canonical skill path, objective, design anchors, edit
ownership, dependencies, acceptance, and escalation conditions.

For discoverability assignments, include the reader's question, the affected
entry-to-operation route, and the orchestration requirement above.

Give each implementation worker a distinct sample to make run, never one stage
or file of a sample. The worker makes the necessary cross-layer repairs and
lands its own result; retain any explicitly required independent review.
Independent read-only review challenges assumptions and checks correctness.
Workers record progress through `python tools/claims.py note --ticket <ticket>
--text <finding>` on their claim ticket plus the session report.

Use the actual agent tool and returned ID before reporting a worker as launched.
Monitor through completion, respond to findings, and inspect the returned diff
and evidence before integration. Worker IDs stay in the conversation.

Evidence identifies revision, host, command, exit, and remaining dependencies.
The assigned acceptance is the required capability demonstrated on the target,
or the measured development workflow restored to its agreed bound; continue
implementation when the design answers the next question. Escalate genuine
external decisions with the exact missing prerequisite and next acceptance,
and keep the work marked unfinished while continuing feasible engineering.

## Land and restore the checkout

Invoking advance authorizes committing and publishing the change through
[the landing protocol](../../../tools/landing.md).

Validate before entering the queue. Follow the protocol for the local wait,
claim, rebase, exact verified candidate, and release on an affected failure.
Publish with `--provenance "agent=<tool> model=<id> effort=<level>"` naming the
actual model. When the session does not know it, read the harness configuration
rather than writing `unspecified`: Codex records it as `model` and
`model_reasoning_effort` in `~/.codex/config.toml`.
Hold the lease only for the publish itself.

The sample running is the record of the work; no board entry is needed. If a
`TASKS.md` item names this sample or its root cause, update it in the same
change following [board update discipline](../../../TASKS.md#updating-the-boards):
subtract what now works, delete the item when its acceptance is met, and do
not replace it with a success narrative. A sample that still fails keeps its
current failure in its README.

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
preserve the exact frontier for the same owner's continuation without marking
the outcome complete.

Report the program's behavior before and after, commit/publication state,
checks run, and remaining dependencies. For development-health work report the
workflow's before/after measurements and preserved validation. Stop after the
owned outcome unless the user requested continuing work.

For skill maintenance, use the [evaluation guide](evals/README.md).
