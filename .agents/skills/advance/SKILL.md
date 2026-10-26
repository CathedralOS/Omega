---
name: advance
description: >-
  Advance an Omega capability outcome through executable samples, landing
  complete repairs and retaining its remaining acceptance. Use for advance,
  continue the compiler, or pick the next compiler work. Not for a named bug
  fix, code question, review, or repository-wide health check.
---

# Advance the Omega compiler

Finish a selected capability through executable witnesses and published repairs,
following [the sample loop](../../../AGENTS.md#the-sample-loop). A named sample
or explicitly bounded request retains its narrower scope.
Follow [AGENTS.md](../../../AGENTS.md) for repository ownership,
validation, board rules, and publication.

## Choose the capability and its next witness

Record the start time, starting checkout, and base. Fetch main (`git fetch origin main`)
and inspect recent lane commits. Never fast-forward, pull, or modify the starting checkout;
preserve it intact as a stable reference. Create and work exclusively in an isolated worktree
based on fetched `origin/main`.

Apply the selection order in [AGENTS.md](../../../AGENTS.md#the-sample-loop):
explicit user scope, the existing capability continuation, an unclaimed active
`main` regression (only when unassigned and the newest completed `Post-merge smoke`
run on `main` failed), then an actionable board outcome by priority and release dependencies. Compiler
advancement then selects the witness only from `samples/cli/facts`. One fact
is one folder. The nine places are local, field, parameter, call result,
match join, const name, another state, another machine, and another target
body. Those places are one row. Do not expand them into a combinatorial
matrix. `samples/cli/facts/length/` is the filled row that met this bar. The
next witness fills one other fact across those nine places, or takes the next
uncovered fact. Programs under the rest of `samples/` stay out of this
selection and may stay broken while this layer is in progress.

A fact folder is done only when the same fact survives all nine places. One
sentence names the fact and the operation, and every place copies that
sentence. Changing the fact, the bound, or the operation so a place passes
is a different fact and does not count. When a place fails because the
producing layer dropped a requirement, that requirement is the fact. The
sentence names every requirement that layer records on the value it emits
(saved scalars, write set, crash routes, domain facts, or the selected body),
and the consuming operation uses that recorded list. A compiler edit that
recognizes one production and rebuilds one requirement for it leaves the fact
open. The next witness is another production of that same layer, in this
folder, copying the same sentence. A separate fact folder for that production
does not count. Every positive place (and root
`main.omg` when present) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares
`Expected exit: 0`, and passes `omega run --both` (Terminal Psi and native
execution agreement); compile-only positive places do not count. A red place
is a checking or lowering repair: keep the program the same shape, and record
the diagnostic before editing the compiler. Each place needs a reject twin
that breaks only that fact or obligation and still rejects. A place with no
twin does not count, and a row is not closed until its nine positive places
and nine reject twins are pinned under `tests/omega/{pass,fail}/facts/`. A
landing that only rewrites the failing place is not acceptance. Exact
subtraction, alias transfer, and dependent subtract are examples of that
repair path.

The nine places carry each language construct end to end—from source checking
and proof through Terminal Psi semantics to native realization and execution
(`omega run --both`)—as one row per construct, never a combinatorial matrix or
a stage-isolated task. Each fact folder exists to prove that **one distinct
compiler mechanism**—one semantic rule, layout representation, ownership or loan
transition, ABI placement lane, or instruction-lowering path—preserves its
static meaning and runtime behavior across all nine transport places:
width/signedness scalars, shifts,
narrow-to-wide sign/zero extension, signed/unsigned division and remainder,
IEEE floats, refinement domains, dependent value parameters, quotient
projections, mathematical bindings, inductive structural recursion, termination
measures, fixed-size arrays, nested structural records, tagged case
payloads, slice views, explicit layout recasts/codecs, `size_of`/`align_of`
layout geometry, borrowed projections (`&`, `&mut`), `&write` write-only
borrows, affine cleanup, linear claim retirement, borrowed-storage restoration
windows, content custody, placed/MMIO access, atomic concurrency orderings,
named-implementation/`dyn` dispatch, multi-argument calling plans across
register and stack slots, live-across-call preservation, disjoint stack-frame
homes, boundary/provider calls, foreign bindings, private callbacks, process
exit, and checked `asm`.
Once a row proves a mechanism across the nine places, repeating that same
mechanism at a larger count adds no new evidence about the compiler. Another
production that drops the same requirements is that same row: the repair
records the requirements on the value the layer emits, and the next layer reads
that list. A new row is a different requirement, a different layer, or a real
physical boundary where representation, ABI placement, or instruction encoding
changes. When a repair adds or changes a construct, carrier, access
mode, cleanup disposition, or target rule, complete the entire source-to-native
path in the same change rather than splitting work by compiler stage.

Read the whole item's acceptance and verify its remaining gap with an optimized
exact-source producer for the fixed recorded base. Reuse warm `build/gate-cli`
artifacts with verified inputs; build only for changed producer inputs or a
missing valid producer. Never use an unverified cached binary. Record the parent
task and the exact requirement the next fact will discharge. Do not attach a task ID to an unrelated program after
choosing it. Never filter discovery solely to folders that already have all nine
`<place>/main.omg` files and already pass on current main (which yields
coverage-only commits without advancing `omega-rust/`), and never treat fixing
an isolated `tests/omega/` fixture as end-to-end completion without carrying
the `TASKS.md` capability across all nine transport places in
`samples/cli/facts/<fact>/`. Query incomplete `samples/cli/facts/<fact>/`
folders (`< 9` standard places present) and open `TASKS.md` requirements whose
mechanism lacks a complete nine-place row; transporting a `TASKS.md` construct
through all nine places (`local`, `field`, `parameter`, `call_result`,
`match_join`, `const_name`, `other_state`, `other_machine`, `target_body`)
routinely exposes real checking, proof, and lowering gaps in `omega-rust/` that
a single root witness missed.
Apply [requirements and evidence](../../../AGENTS.md#requirements-and-evidence):
each sample fact README lists how its nine places and reject twins prove out
the eight end-to-end specification requirement layers—**establishment**,
**identity and lifetime**, **composition**, **soundness boundaries**,
**checked-to-Terminal Psi lowering (`Psi 05–07`)** (proof-term erasure, affine
cleanup actions, borrow restoration windows, `dyn` selector vs. data lanes,
strongly connected ranking/control edges, boundary demands, and `PSITERM\0`
interpreter execution), **provider settlement, layout geometry, and ABI calling
plans (`Omega 00–02`)** (Build companion joins, `size_of`/`align_of` and
field/variant/view layout plans, and `CallPlan`/`StatePlan`/`BoundaryEntryPlan`
register/stack/`sret`/`&write` placements), **instruction selection, register
homes, and activation frame realization (`Omega 03–06`)** (target ISA
selection, register homes, disjoint stack-frame homes, live-across-call spill
slots, WCSU/bounded activation claims, and entry prologue + multi-return
epilogue insertion), and **form encoding, branch relaxation, relocations, and
native image emission (`Omega 07–09` + emission)** (symbolic frame-displacement
resolution, function-relative block layout and short/near branch relaxation with
offset reflow, byte-pool/dispatch-table/import relocations, machine-state
footprint evidence, and direct PE/ELF/Mach-O execution agreement via
`omega run --both`)—plus linked positives/reject twins and remaining open scope.
The construct under test must establish and carry the static proof fact into the
consuming operation itself: never re-establish the fact in the caller after the
construct completes using a post-hoc equality guard (`transition x == 100`), and
verify via `git diff` that `<place>/main.omg` vs. `<place>/invalid/main.omg`
differ only in the construct's own value, domain, or contract. Keep
producer/target-bound run records in the existing qualification/claim record or
commit. Close only the acceptance that evidence actually establishes.

Use `python3 tools/sample_queue.py --facts` as witness evidence inside
`samples/cli/facts`, not a global smallest-first assignment policy and not the
unfiltered program queue. Prefer an existing fact folder; use a focused
reproduction when the task already names one. Run
`python3 tools/sample_native_gate.py --programs-only --find-work --facts` when
discovery is needed and no recent facts record exists. The same commands
without `--facts` still discover ordinary programs and are not this selection.
The facts queue lists projects the sample gate admits. Also read the
directories under `samples/cli/facts`, including folders the gate classifies
as libraries. An absent or green facts queue does not close the task and does
not mean an unfilled or one-place fact is done: write the next missing place,
or take the next uncovered fact. Take a new fact from an open `TASKS.md`
item's remaining acceptance and name that item in the README; a folder that
changes only a count, width, arity or extension pair of an existing row repeats
that row and is not a new fact. Several places with one demonstrated root
cause may belong to one repair. Do not leave `samples/cli/facts` to fill
compiler-advance capacity.

Size the work to finish in this session. When the fact is too large to
trace, write a smaller witness inside that fact folder that isolates its
failing construct with its own oracle, make that run and land it, then return
to the fact in this same session. The smaller witness keeps the same fact,
bound, and operation. Landing that smaller witness and stopping
leaves the fact unfinished. A place that
already passes establishes only its exercised requirement, and only when its
reject twin still rejects. A place with no twin does not count. Reconcile that
evidence and continue the same fact's remaining places; do not
manufacture a failure or compiler edit. Changing the fact, the bound, or the
operation so a place passes is a different fact and does not count. Do not carve a new sample out of a
fact that already meets its oracle. If the full acceptance already passes,
close the stale item through normal board reconciliation and select the next
outcome only when the user's scope permits it.

Squalr and Cathedral are customers, not source shapes to special-case or
rewrite around gaps. `TASKS_BOOTSTRAP.md` and `TASKS_OPTIMIZER.md` work, when the
user names it, keeps its own acceptance. After landing a sample, measured
iteration performance or test-environment friction may be the next piece of
work. Name its workflow, baseline and adequate target first; a minor
incidental slowdown is not permission to abandon a sample.
Read live claims and in-flight assignments before choosing unassigned work.
Prefer distinct capability outcomes with demonstrated acceptance; shared
implementation paths do not exclude them. An explicit user-named assignment or
same-session continuation keeps its priority even when another session claims
it: read the note, reuse available repairs and reconcile at rebase. A `main`
regression is single-owner (`REGRESSION-<sha>`): if another live session has
already claimed it, skip it and take normal unassigned work. Sessions with an
active capability or user assignment never pivot to unrelated `main`
regressions or check out historical `main` commits. Do not invent unrelated
programs to fill capacity when the current fact witnesses are already assigned.

Claim the parent item (`claim --board TASKS.md --item <ID> --owner "<label>"
--path <paths>`) and record the current witness in its note. For explicitly
standalone sample work, claim `--item <sample path>` without inventing a board
entry. Existing sample tickets remain valid; add the parent context in the note
rather than discarding ownership. Inspect the registration's `overlaps`: if an
automatic assignment (including a regression) duplicates a just-claimed outcome,
choose another ready outcome before implementation. This preference
for distinct automatic assignments grants no veto and requires no permission;
explicitly user-named samples and same-session continuations still proceed.
Overlapping paths are informational.
Renew long work and release it when finished for any reason, recording the
remaining frontier if interrupted. An unavailable registry means ownership is
unknown, never that every sample is free. Report that uncertainty and retain the
isolated scope required by AGENTS.md; do not claim registration succeeded.

Keep the recorded base fixed after initial preflight. Inspect peer claims/PRs
to avoid duplicate repairs. Integrate only a demonstrated actual merge conflict,
concrete prerequisite or changed affected source, then validate that affected
closure. Main or queue SHA movement alone does not justify local integration,
builds or qualification. An explicitly named
program is the customer for this session. A smaller witness isolates the
current failure; it does not replace that program. Return to it in the same
session until its oracle holds. Do not open a second feature while it still
fails. State the
sample, its oracle, the current failure and the acceptance command.

Keep that short record in the claim note: parent task, remaining acceptance,
current sample and the requirement it discharges, failure, next action and
dependencies. Name what of the customer now runs and what still fails, in
words a person can read without the diff. Reread it after compaction
and before changing direction. Reuse the session that already has the
checkout. After a landed repair, compact when the next assignment is a
different outcome, and leave the context in place when the next run branches
from that repair. The rule for cloud pods is in
[cursor-swarm](../cursor-swarm/SKILL.md#reuse-the-session). When resuming
saved work after a crash or compaction, inspect its existing checkpoint, PR and
receipts before reapplying a patch or rerunning validation. Preserve its fixed
base and warm producers; resumption does not require a rebase. A new obstacle
updates this frontier; it does not create a task for someone else. Do not split work or
mine tasks to keep a slot occupied.

Choose the capability by its source-to-execution acceptance: "a helper mutates
the caller's field and the caller observes it after return," stated as the
program's observable behavior. A checking, proof, or lowering repair is not
done until the fact-place fixtures under
[`tests/omega/pass/facts`](../../../tests/omega/pass/facts) and
[`tests/omega/fail/facts`](../../../tests/omega/fail/facts) pass, including a
native run. A miss means the repair recognized a shape; generalize the
operation. Trace the remaining producer and consumer
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
consistent with the design so that the general case runs. When the failure is
a requirement the producing layer dropped, the sample sentence is the list
that layer records on the value it emits, and the edit records that list for
the next layer to read. One production's recognizer leaves the fact open.
Follow [the sample loop](../../../AGENTS.md#the-sample-loop).

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

Never edit files, switch branches, or build in the primary repository checkout.
Use an isolated worktree under `<repository>/.agents/worktree/<short-name>`
created in detached HEAD mode directly from the fetched `origin/main`:
`git worktree add --detach .agents/worktree/<short-name> origin/main`.
Local branches are banned: do not create local named branches.
Record that initial base once; do not refresh it merely as main moves.
A continuation retains its existing worktree base. Keep its name short; if a
generated Windows path fails, measure the actual path before relocating work or
changing compiler architecture.

Keep the loop fast. Run samples and the corpus with an optimized compiler in its
own short target directory (`CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=<dir>
mbx build -p omega`; on Windows, keep `<dir>` short such as `C:\tmp\tgt-<short>`
so deep crate paths stay below MSVC `link.exe`'s 260-character `MAX_PATH`), and
close stdin (`< /dev/null` in sh, or `$null | ...` in PowerShell) on every direct
`omega` command you run in the background (the gates already do): `omega run`
forwards stdin to stdin-reading programs and otherwise waits for EOF at zero CPU.
A step that still takes minutes is a finding to measure and remove, not a wait.
Optimization is enforced by the compiler build and by gate preflight, including
supplied `--cli`/`--runner` binaries. For manual validation, `omega --build-profile`
reports the loaded producer's actual profile; an environment override around an
old executable proves nothing. Compiler-backed pinned qualification declares
`producer.executables` within `artifacts` and binds verified profiles to their
binary digests. Preserve historical observations; do not relabel an older
unoptimized run as optimized or reuse it as a newly qualified producer.

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
regression signal. Always filter: an unfiltered corpus gate or the full
baseline is a manual trigger that only the user starts, never an agent. The
shared golden also carries other lanes' drift and other
hosts' rows, so its moves are a signal to attribute, not evidence against this
change; attribute them with `--compare` (below) before acting on them. Record
only the fixtures this change adds or moves (`--record --filter <fixtures>`),
which splices those rows into the golden and leaves the rest unchanged. `--jev` on the same run gives the advisory record-safety
verdict before any `--record`. Include affected crate and integration
checks. Reuse successful results on unchanged inputs, including at landing.
Qualify on a fixed recorded revision. Main advancing does not invalidate every
check: apply [main-change validation](../../../tools/testing.md#main-change-validation)
to the actual changed inputs. Native queue admission replaces local landing receipts.
Use [qualification.py](../../../tools/qualification.md) to capture the reviewed
source/compiler/artifact closure and measured checks. Keep the qualification
checkpoint fixed; when actual conflicts or changed assumptions require source
integration, assess and run affected checks. Reused observations
retain their actual producer/revision. Required review covers the submitted
change; queue-created SHA movement alone does not restart author qualification.
Read [execution evidence and replay inputs](../../../tools/testing.md#execution-evidence-and-replay-inputs)
before preparing projects: copy authored inputs and declared resources, preserve
historical generated state, and record actual engine observations. Apply the
branch/consumer applicability rules in main-change validation before attesting
reuse. Use a completed replay's receipts where supported instead of duplicating
an entire preparation pass just for a review manifest.
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
Use the [independent semantic review boundary](../../../tools/testing.md#independent-semantic-review)
for its exact candidate, canonical context, current/historical evidence and
findings. The implementation owner retains review fixes and publication.
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

Validate the submitted change, push HEAD directly to origin (`git push origin HEAD:refs/heads/work/<name>`), open/reuse a PR, and use
the native merge queue under the linked protocol. Put actual agent/model
provenance in the PR body when known. Do not invent unknown metadata. Release
local build slots before waiting. Never chase main with gates, rebases, or rebuilds:
GitHub Actions CI does the integration build and checks in the merge queue. No local publication
lease or exact-SHA landing receipt remains; main movement alone does not restart author review.

Focused acceptance on the submitted source plus required GitHub integration CI
and verified publication establishes the landed repair. Postlanding verification
is a read-only PR/commit/CI observation, not a compiler rebuild or acceptance
rerun. Update its parent item in the same change following [board update discipline](../../../TASKS.md#updating-the-boards):
subtract only requirements established by the evidence, delete the item only
when its full acceptance is met, and do not add a success narrative or a new
item per witness. Keep the next remaining requirement in the claim note. A
sample that still fails keeps its current failure in its README.

Landing one witness is not a switch to an unrelated outcome. Continue the
parent's next required witness within authorized scope; use a fresh worktree
after the landed lane's cleanup below. Reuse the session and relevant context.
An explicit one-sample stop remains a stop for other work, with parent
acceptance reported unfinished. A checking, proof, or lowering repair is not
that stop: it is not done until the fact-place fixtures pass, including a
native run, and a miss means generalize the operation. A genuine external
prerequisite pauses only the affected work;
preserve its frontier and continue other feasible work within user scope.

After publication, observe the merged PR's exact submitted head, published commit
and GitHub CI. Fetching main to verify published ancestry is read-only observation;
it does not require moving the starting checkout or building that revision.
Preserve the starting checkout and any dirty or unpublished work.

Retain the worktree as a warm build workbench for subsequent tasks rather than
tearing it down. To reuse it: reset its detached HEAD to the new `origin/main`
(`git checkout --detach origin/main`) and clean scratch files (`git clean -fd`),
keeping the pre-compiled `target/` cache warm. If teardown is explicitly requested
or disk must be reclaimed: confirm the worktree is clean, verify the PR's published
commit is an ancestor of fetched `origin/main`, stop any running processes, and
run `git worktree remove <path>`. Because work was done in detached HEAD, no local
branch exists to delete. Pre-existing worktrees, the starting checkout, and anything
dirty or unpublished stay.

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

Report the program's behavior before and after, the parent requirement removed,
remaining capability acceptance, commit/publication state, checks and dependencies.
For development-health work report the
workflow's before/after measurements and preserved validation. Stop after the
owned outcome unless the user requested continuing work.

For skill maintenance, use the [evaluation guide](evals/README.md).
