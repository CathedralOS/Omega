# AGENTS.md

This is the canonical instruction file for every coding agent working in this
repository. Agent-specific instruction files must point here rather than copy
these rules. Repository skills are canonical in `.agents/skills/`; read and
edit them there. Pi and Codex discover that directory directly. Do not
maintain a second copy. For Claude Code, create an ignored checkout-local
link to the same directory:

- Windows PowerShell: `New-Item -ItemType Directory -Force .claude`, then
  `New-Item -ItemType Junction -Path .claude/skills -Target (Resolve-Path .agents/skills)`.
- macOS shell: `mkdir -p .claude`, then `ln -s ../.agents/skills .claude/skills`.

Create the link only when `.claude/skills` is absent; inspect an existing path
before replacing it. Repeat in a new checkout if using Claude there. The Windows
junction avoids administrator or Developer Mode requirements; it is local, not a
tracked symlink that Git may check out as a plain text file.

Omega is a proof-checked systems language whose programs are data-oriented
state machines. This repository holds the language, its Rust reference
compiler, its Omega-written product compiler, and the Alpha-to-Omega bootstrap
chain.

**The goal is a compiler that ships.** Choose a capability outcome from
`TASKS.md`, then carry its executable witnesses end to end
([the sample loop](#the-sample-loop)), not a file or a stage.
Compiler-advance witnesses are fact folders under `samples/cli/facts`.
Demonstrate compiler work through real programs and
relevant invalid controls; `omega run <program.omg>` must produce the authored
result. Squalr and Cathedral expose general compiler gaps, not special cases
to recognize or applications to rewrite around missing language support.
Iteration performance and the test environment are also first-class outcomes
when a measured workflow obstructs development. They need a concrete acceptance
and stopping point, not an open-ended optimization or testing campaign. A step
that makes the sample-to-native loop slow is a workflow defect: measure where
the time goes and remove the cause rather than waiting on it or working around it.

- `omega-rust/` — the Rust reference producer: the working development
  compiler and differential comparator. Not canonical, not a language rung.
- `source/psi/` + `source/omega/` — the Omega-written product compiler, split
  along the same firewall. This is the destination.
- `source/library/` — bundled `omega::` packages (`name.omg` or `name/mod.omg`).
- `bootstrap/{0_alpha,…,5_omega}/` — the trust-minimizing bootstrap chain;
  `bootstrap/proofs/` holds Gamma-written derivation checking beside the
  rungs. Bootstrap scripts resolve cross-owner locations through
  `tools/bootstrap/paths.sh` — never hard-code sibling-relative paths — and
  may invoke, stamp, compare, and report but never parse, lower, manufacture
  semantic evidence, or decide trust.

## Law of completion

Our superhuman advantage is carrying the entire difficult task end to end.
Difficulty is a reason to investigate and persist, not to abandon the work,
lose confidence without evidence, or redefine done. Confidence is demonstrated
by working behavior, not by declaring success.

We are capable of taking an uncompilable sample through every necessary compiler
layer to a compilable, executable, validated program landed on main. The agent
owns that entire transformation, not just the first repair. Treat the hard
remaining steps as engineering to finish, not evidence that the task is beyond
reach.

Follow the river to its end; do not carve every possible branch along the way.
Working paths reveal the design. Build and validate what this journey needs;
adjacent journeys will reveal their own needs when undertaken.

Unfinished work loses the context that made it tractable when the session
doing it stops. The session that starts a piece of work therefore carries it
through acceptance, integration, required review, authorized landing and
remote verification on main. A checkpoint, component pass, archive, JSON
report or handoff document preserves evidence; none completes the work.
There are no handoffs: nobody else is waiting to finish it.

The working session finishes the entire path, including the necessary
compiler, library and prerequisite repairs. Do not manufacture dependencies on
other agents by dividing work into stages or waiting for another session to
finish missing functionality. Reuse available repairs when practical;
otherwise continue the necessary work in the isolated worktree. Overlapping
edits and merge conflicts are expected and reconciled at rebase.

When the user authorizes end-to-end publication, the session lands the
completed path through the normal landing protocol; returning a partial
candidate for someone else to finish is not a substitute. Do not publish
half-finished functionality or an unfinished workflow. Read-only
investigations remain read-only.

Persistence does not authorize fabricated results, narrowed customer acceptance,
permission changes, custody bypasses or weakened review and validation gates.
Raise genuine external authorization or environment blockers precisely, keep
the work marked unfinished, and continue all other feasible engineering. A
user stop or exhausted runtime budget preserves the work for a later
continuation; it does not turn unfinished work into completion.

## The sample loop

Capability outcomes determine priority; samples provide executable evidence.
Compiler advancement runs `samples/cli/facts` only. One fact is one folder.
The nine places are local, field, parameter, call result, match join, const
name, another state, another machine, and another target body. Those places
are one row. Do not expand them into a combinatorial matrix.
`samples/cli/facts/length/` is a reference row. Inspect each folder's current
coverage and applicable evidence to distinguish filled rows from missing places.
The next advance fills a missing place of the selected fact, or takes the next
uncovered fact, instead of picking an unrelated program.

A fact folder is done only when the same fact survives all nine places. One
sentence names the fact and the operation, and every place copies that
sentence. Changing the fact, the bound, or the operation so a place passes
is a different fact and does not count. Every positive place (and root
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

When a place fails because the producing layer dropped a requirement, that
requirement is the fact. The sentence names every requirement that layer
records on the value it emits: the saved scalars, the write set, the crash
routes, the domain facts, or the selected body this layer owes. The consuming
operation is written against that recorded list. Every place transports the
value and copies the sentence. The reject twin removes one requirement from
the list, and the diagnostic is the missing requirement. A compiler edit that
recognizes one production and rebuilds one requirement for it leaves the fact
open. The next witness is another production of that same layer, in this
folder, copying the same sentence. A separate fact folder for that production
does not count.

The nine places carry each language construct end to end—from source checking
and proof through Terminal Psi semantics to native realization and execution
(`omega run --both`)—as one row per construct, never a combinatorial matrix or
a stage-isolated task. Each fact folder exists to prove that **one distinct
compiler mechanism**—one semantic rule, layout representation, ownership or loan
transition, ABI placement lane, or instruction-lowering path—preserves its
static meaning and runtime behavior across all nine transport places:
width/signedness scalars, shifts, narrow-to-wide sign/zero
extension, signed/unsigned division and remainder, and IEEE floats
([numeric values](wiki/spec/language/numeric_values.md)); refinement domains,
dependent value parameters, quotient projections, mathematical bindings,
inductive structural recursion, and termination measures
([domains](wiki/spec/language/domains.md),
[dependent values](wiki/spec/language/dependent_values.md),
[quotients](wiki/spec/proofs/quotients.md),
[mathematical bindings](wiki/spec/proofs/mathematical_bindings.md),
[inductive profile](wiki/spec/proofs/inductive_profile.md),
[termination](wiki/spec/language/termination.md)); fixed-size arrays,
nested structural records, tagged case payloads, slice views, explicit layout
recasts/codecs, and `size_of`/`align_of` layout geometry
([layout plans](wiki/spec/layouts/plans.md),
[recasts](wiki/spec/layouts/recasts.md),
[codecs](wiki/spec/layouts/codecs.md),
[counts and addresses](wiki/spec/language/counts_and_addresses.md)); borrowed
projections (`&`, `&mut`), `&write` write-only borrows, affine cleanup, linear
claim retirement, borrowed-storage restoration windows, content custody,
placed/MMIO access, and atomic concurrency orderings
([structural access](wiki/spec/terminal-psi/structural_access.md),
[ownership](wiki/spec/terminal-psi/ownership.md),
[loans](wiki/spec/terminal-psi/loans.md),
[content custody](wiki/spec/resources/content_custody.md),
[placed access](wiki/spec/resources/placed_access.md),
[concurrency](wiki/spec/language/concurrency.md)); and named-implementation/`dyn`
dispatch, multi-argument calling plans across register and stack slots,
live-across-call preservation, disjoint stack-frame homes, boundary/provider
calls, foreign bindings, private callbacks, process exit, and checked `asm`
([dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md),
[calling plans](wiki/spec/build/calling_plans.md),
[foreign bindings](wiki/spec/build/foreign_bindings.md),
[private callbacks](wiki/spec/build/private_callbacks.md),
[process exit](wiki/spec/language/process_exit.md),
[storage](wiki/spec/resources/storage.md),
[checked assembly](wiki/spec/language/assembly.md)). Once a row proves a
mechanism across the nine places, repeating that same mechanism at a larger
count teaches the compiler nothing new. Another production that drops the same
requirements is that same row: the repair records the requirements on the value
the layer emits, and the next layer reads that list. A new row is a different
requirement, a different layer, or a real physical boundary where
representation, ABI placement, or instruction encoding changes (such as
crossing from register arguments into stack slots, widening a discriminant tag,
or relaxing a short branch into a near displacement). When a repair adds or
changes a construct, carrier, access mode, cleanup disposition, or target rule,
complete the entire source-to-native path in the same change rather than
stopping at an intermediate compiler stage. Completion also follows the
[requirements and evidence rule](#requirements-and-evidence): each completed
fact row names the requirement it establishes, and each requirement closes only
when its full acceptance is demonstrated.

Programs under the rest of `samples/` stay out of compiler-advance selection.
They may stay broken while this layer is in progress. A named assignment
outside that tree keeps the oracle on its own program: a runnable program
whose header documents `Expected exit` or `Expected output contains`. A
passing witness completes that repair, not every requirement of its parent
capability.

1. **Choose the outcome, then its witness.** Repair known main regressions
   first, honor explicit user scope, and otherwise retain the current capability
   assignment. For new automatic work, choose an actionable `TASKS.md` outcome
   by its stated priority and release dependencies, then discharge it inside
   `samples/cli/facts`. Read its full acceptance
   and verify its remaining gap against a fresh current-main compiler build
   (`build/gate-cli`), never a stale cached binary (`omega-latest.exe`). Name
   the exact requirement the next witness will discharge. Never filter discovery
   solely to folders that already have all nine `<place>/main.omg` files and
   already pass on current main (which produces coverage-only commits without
   advancing `omega-rust/`), and never treat fixing an isolated `tests/omega/`
   fixture as end-to-end completion without carrying the `TASKS.md` capability
   across all nine transport places in `samples/cli/facts/<fact>/`. Query both
   incomplete fact folders (`< 9` standard places present) and open `TASKS.md`
   requirements whose mechanism lacks a complete nine-place row. `python3
   tools/sample_queue.py --facts` supplies failing fact witnesses; its
   smallest-first order does not set product priority, and a green facts list
   does not fill an unfilled fact. Use existing evidence or a focused run for
   the selected fact; run `python3 tools/sample_native_gate.py --programs-only
   --find-work --facts` when discovery is needed and no recent facts record
   exists. The same commands without `--facts` remain the ordinary program queue
   and are not compiler-advance selection. A genuinely missing owner decision
   is cited in the sample README through `OWNER_QUESTIONS.md`.
2. **Make it run and land.** Done for this witness means it compiles and runs
   natively with its authored result on main, with every compiler, library and fixture repair
   it needed landed with it, and its invalid controls still rejecting.
   Nothing short of that is done: a stage, checkpoint, diagnosis or board edit
   is progress, not completion. One place running does not finish the fact.
   A landing that only rewrites the failing place is not acceptance.
3. **Land complete steps toward the same outcome.** Size each executable
   repair to finish in one session; the capability may require several landed
   repairs. When a fact is too large to trace (its size hides the
   failing construct, or its first failure leads into an unrelated second
   gap), cut it down: write a smaller witness inside that fact folder that
   isolates the failing construct with its own oracle, make that run, land it,
   then return to the fact. The smaller witness keeps the same fact, bound,
   and operation. Facts advance as a ladder of those places,
   never as stage tasks.
4. **Return to the remaining acceptance.** After a witness lands or already
   passes, subtract only the requirement its evidence establishes and take the
   next required place of the same fact, or the next uncovered fact under
   `samples/cli/facts`. Write a new witness when an
   uncovered requirement needs one, not to manufacture a failure or fill a slot.
   Close the task only when its full acceptance passes. Switch capabilities on
   closure, explicit reprioritization, or a genuine external prerequisite after
   exhausting feasible work on that outcome. Regressions may interrupt this
   chain; return afterward. Respect user stops, budgets and explicitly bounded
   assignments, preserving the parent outcome and next requirement for resumption.
   A green facts queue does not establish capability closure. Open-ended
   advancement stays inside `samples/cli/facts` and takes the next uncovered
   fact there. Draw that fact from an open `TASKS.md` item's remaining
   acceptance and name the item in its README, not from the mechanism list
   above. A folder that changes only a count, width, arity or extension pair
   of an existing row repeats that row. By 2026-10-21, 24 folders differed
   only in argument count (9 through 32), and 128 belonged to families that
   differ only in bit width or extension pair.

A smaller witness is a rung inside the fact that is too large to trace.
Land it in the same session that returns to that fact. A session that
lands the rung and stops has not finished. Do not carve a new sample out of
a fact that already meets its oracle.

Cathedral is a customer. A session driving Cathedral fixes the Omega defect
that blocks its current acceptance, then returns to Cathedral. It does not
stay in the compiler collecting adjacent repairs.

The repair inside the loop is ordinary compiler engineering and follows every
rule below: generalize the operation, check the fact-place fixtures, pin the
behavior with fixtures under `tests/omega/`, and keep main green.

A change that makes the language reject source it used to accept migrates
every sample and fixture it breaks in the same change. Run the sample gate on
the affected samples before landing; a sample left failing by a language
change becomes someone else's queue entry with no compiler work in it.

### Requirements and evidence

Facts supply executable evidence; the accepted specification requirements define
what must be complete end to end. Read the governing specification and the owning
task's full acceptance. Prove out and list these obligations in the sample fact:

- **Establishment:** construction, guards, entry contracts, completed calls or
  checked theorem evidence on the construct under test justify introducing the
  fact; an unproved goal does not establish itself, and a post-hoc caller
  equality guard (`transition x == 100`) after the construct completes does not
  substitute for the construct establishing or preserving the static fact
  ([state contracts](wiki/spec/language/state_contracts.md),
  [proof contracts](wiki/spec/proofs/contracts.md),
  [domains](wiki/spec/language/domains.md),
  [dependent values](wiki/spec/language/dependent_values.md)).
- **Identity and lifetime:** the fact names an exact value or storage version;
  snapshots retain their meaning, and overlapping writes invalidate facts about
  changed storage without discarding facts about provably unrelated storage
  ([fact validity](wiki/spec/terminal-psi/fact_validity.md),
  [lifetimes](wiki/spec/language/lifetimes.md),
  [ownership](wiki/spec/language/ownership.md),
  [content custody](wiki/spec/resources/content_custody.md)).
- **Composition:** calls substitute exact actuals, joins retain only justified
  facts on every incoming path, and state transitions prove their arrival
  obligations; the consuming operation (such as `total - 100u64`) is proved
  directly from the fact carried through the place, and `<place>/main.omg` vs.
  `<place>/invalid/main.omg` differ only in the construct's own input, domain,
  or contract—never in a caller guard
  ([machines](wiki/spec/language/machines.md),
  [evaluation](wiki/spec/language/evaluation.md),
  [patterns](wiki/spec/language/patterns.md),
  [structural predicates](wiki/spec/terminal-psi/structural_predicates.md)).
- **Soundness boundaries:** proof preserves ownership, access authority and
  termination obligations, tracks transitive assumptions, and respects the
  boundary between erased mathematics and executable values
  ([mathematical bindings](wiki/spec/proofs/mathematical_bindings.md),
  [mathematical values](wiki/spec/terminal-psi/mathematical_values.md),
  [quotients](wiki/spec/proofs/quotients.md),
  [inductive profile](wiki/spec/proofs/inductive_profile.md),
  [termination](wiki/spec/language/termination.md),
  [authority](wiki/spec/resources/authority.md)).
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** source constructs lower
  without loss into canonical target-neutral Terminal Psi (`PSITERM\0`) and
  execute in the Terminal Psi interpreter (`omega run --both`), erasing
  mathematical/proof-only bindings while preserving erased proof-formal tag lanes
  (`proof-term-tags`), materializing explicit affine cleanup actions
  (`DiscardRoot`, `DiscardResidual`, `InvokeNominal`, `RetireHome`) and
  borrowed-storage restoration windows (`reborrow restored call uses`), lowering
  state transitions, crash routes, and strongly connected ranking/scalar-block-invariant
  edges, separating explicit `dyn` selector vs. data lanes, and retaining
  boundary-application demands, placed/atomic observations, integer arithmetic
  modes/certificates, and checked target-scoped body candidates
  ([product](wiki/spec/terminal-psi/product.md),
  [encoding](wiki/spec/terminal-psi/encoding.md),
  [calls and outcomes](wiki/spec/terminal-psi/calls_and_outcomes.md),
  [control flow](wiki/spec/terminal-psi/control_flow.md),
  [dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md),
  [ownership](wiki/spec/terminal-psi/ownership.md),
  [loans](wiki/spec/terminal-psi/loans.md),
  [boundary calls](wiki/spec/terminal-psi/boundary_calls.md),
  [observations](wiki/spec/terminal-psi/observations.md),
  [integer certificates](wiki/spec/terminal-psi/integer_certificates.md)).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
  target realization joins Build companion rows (`ProgramEntry`, `ProviderPlan`,
  `CompilerBuiltinExecution`, `BoundaryExecutionBinding`, assumed binding
  contracts, and target-scoped body selection) to the Terminal catalog without
  reopening source, evaluates and validates target layout plans (`size_of`/`align_of`,
  field/bit offsets, variant tags/payloads, `IntegerAt` stored widths,
  element/byte view descriptors, explicit recasts/codecs, and erased zero-byte
  carriers), and lowers calls, returns, foreign bindings, private callbacks, and
  boundary entries into validated `CallPlan`/`StatePlan`/`BoundaryEntryPlan`
  placements across register vs. stack lanes, indirect `sret` aggregate return
  homes, and `&write` write-only destination pointers
  ([provider selection](wiki/spec/build/provider_selection.md),
  [assumed binding contracts](wiki/spec/build/assumed_binding_contracts.md),
  [foreign bindings](wiki/spec/build/foreign_bindings.md),
  [private callbacks](wiki/spec/build/private_callbacks.md),
  [layout plans](wiki/spec/layouts/plans.md),
  [recasts](wiki/spec/layouts/recasts.md),
  [codecs](wiki/spec/layouts/codecs.md),
  [calling plans](wiki/spec/build/calling_plans.md),
  [byte views](wiki/spec/terminal-psi/byte_views.md)).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):**
  backend lowering selects target ISA instructions (`x86_64` / `arm64`,
  exact/wrapping/saturating integer ops, narrow-to-wide sign/zero extension,
  shift masks, signed/unsigned division and remainder, IEEE float comparisons,
  and checked `asm`), assigns register homes and disjoint stack-frame activation
  storage with live-across-call spill slots, composes worst-case stack usage
  (WCSU) and bounded activation claims (`committed <= bound`), and inserts entry
  prologues and multi-return callee-saved restoration epilogues honoring the
  `StatePlan` ceiling ([storage](wiki/spec/resources/storage.md),
  [activation storage](wiki/spec/resources/activation_storage.md),
  [machine-state evidence](wiki/spec/build/machine_state_evidence.md),
  [checked assembly](wiki/spec/language/assembly.md)).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):**
  final encoding resolves symbolic stack-frame displacements, lays out
  function-relative basic blocks with short/near branch relaxation and offset
  reflow after prologue/epilogue insertion, binds read-only byte-pool, `dyn`
  dispatch-table, import, and private-callback relocations (`physical children`),
  validates final instruction bytes and transitive machine-state footprint
  evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`), and emits
  direct PE/ELF/Mach-O images or admitted executable placements that execute and
  agree with Terminal Psi (`omega run --both`) across the four hosted targets
  ([machine-state evidence](wiki/spec/build/machine_state_evidence.md),
  [executable installation](wiki/spec/build/executable_installation.md),
  [calling plans](wiki/spec/build/calling_plans.md)).

These are requirement categories, not a new test matrix or a claim that they
exhaust the accepted language. Keep the concrete remaining requirements and
acceptance on the existing task board. In the fact folder's README, name the
owning task and exact requirement, link the positive witnesses and fact-sensitive
reject twins, and list how each applicable requirement above is established and
what remains open.
Keep run records in the existing qualification/claim record or commit, with
producer revision and target; distinguish fresh results, applicable retained
evidence and unverified cases. A missing or unrelated reject diagnostic is not
evidence that the fact is enforced. Subtract only the acceptance demonstrated.
A filled row does not close its parent requirement or the check/proof layer
unless all of that acceptance is accounted for.

A compiler-repair assignment needs a demonstrated current-baseline defect:
a valid positive wrongly refuses or executes with the wrong outcome, or a
well-formed invalid control wrongly accepts. Freeze the source/oracle and record
that result before implementation.
An already passing witness may establish coverage; do not manufacture a failure
or count it as a necessary repair. Preserve formation obligations and the whole
published contract: a condition cannot justify forming its own partial operand,
and a necessary consequence is not an equivalent replacement without its converse.

Implementers and reviewers follow [execution evidence and replay inputs](tools/testing.md#execution-evidence-and-replay-inputs)
and [main-change validation](tools/testing.md#main-change-validation). Runtime
claims require actual engine observations; evidence reuse needs a consumer-based
applicability argument. Required independent review follows the
[exact-candidate review boundary](tools/testing.md#independent-semantic-review).
The implementation owner resolves findings and finishes authorized landing.

## Commands

The Rust toolchain is pinned in `rust-toolchain.toml`; `rustup` selects it.

Compiler execution requires optimization. Ordinary dev/test builds default to
`opt-level = 1`; the compiler build rejects effective `opt-level = 0`, including
Rust flag overrides. Sample/corpus gates verify the supplied binary's build
profile before execution, including `--cli`/`--runner`. Pinned compiler-backed
qualification declares `producer.executables` and captures their verified
profiles bound to the actual binary digests. See
[optimized validation compilers](tools/testing.md#optimized-validation-compilers).

- Use `mbx` in place of Cargo for compile and test commands: `cargo check`
  becomes `mbx check`, `cargo test` becomes `mbx nextest run` (needs
  cargo-nextest 0.9.140 or newer). If `mbx` is unavailable, use Cargo without
  asking. Nextest does not run doctests; use `mbx test --doc`.
- Wrap a lane's local build/test chain in one `tools/build_slot.py`
  hold; release it before waiting on GitHub. Nested holds short-circuit
  ([build slots](tools/build_slot.md#one-hold-for-a-whole-chain)).
- Keep using `cargo fmt` and `cargo clean` directly — `mbx clean` differs.
  Whole-workspace formatting is `python tools/fmt.py`: `cargo fmt --all`
  exceeds the Windows command-line limit at this workspace's size.
- Compile a program: `mbx run -p omega -- --check <root.omg>`; run it:
  `mbx run -p omega -- run <root.omg>`. The CLI package is `omega` (binary
  `omega`) — there is no `omega-cli` package. The full flag surface:
  [omega-rust/omega/application/cli/README.md](omega-rust/omega/application/cli/README.md#command-surface).
- The compiler's end-to-end test is the corpus gate:
  `python tools/corpus_gate.py --filter <group/name fragment>`; a crate's own
  Rust tests run by package and name filter
  (`mbx nextest run -p <crate> <filter>`).
- Validation policy — what a change must run, baseline gates, corpus and
  bootstrap gates, test selection, slow builds — lives in
  [tools/testing.md](tools/testing.md). Read it before running more than the
  affected crate's check.

## The Psi/Omega firewall

The load-bearing split, and the most common way to put code in the wrong
place:

- **Psi** owns everything target-neutral on Omega source: lexing, parsing,
  resolution, typing, checking, proof, target-neutral optimization, and
  production of **Terminal Psi**.
- **Omega** consumes Terminal Psi and owns provider selection, Omega-side
  optimization, target realization, ABI, native emission, and execution.
- Target backends own only unavoidable ISA, ABI, object-format, and
  relocation detail. Cathedral (the downstream OS) owns OS structures — do
  not model page tables, schedulers, or drivers as compiler-owned types.

Terminal Psi is the only portable boundary. Targets are data, not control
flow: Psi checks every target-scoped machine body in every compilation, and
`--target` narrows what Omega realizes, never what Psi checks. The full
architecture contract — deferred Psi audit policy, crate placement,
discoverability, compositional lowering — lives in
[omega-rust/pipeline.md](omega-rust/pipeline.md).

Psi groups compilation machinery under `compilation/`, execution engines under
`interpreter/`, and shared primitives under `foundation/`. Compile-time admission
and result elaboration belong to `compilation/compile-time/`, not an interpreter.
The product has one visible route composed from two driver facades: Psi produces
Terminal Psi, then Omega realizes it. These domain folders do not create another
driver, and consumers do not assemble stages behind either facade.

Durable IRs stay in independent `compilation/representations/` packages grouped by actual
pipeline program family. Shared vocabulary, target/ABI descriptions, planning
records and standalone evidence belong to their foundation, checking or backend
owners; they are not extra IR stages. Shared foundations and utilities stay
independently owned. Do not absorb them into producing
stages or the application crate; the owner rejected that consolidation on
2026-09-26.

## Conventions and workflow

The rules that touch every edit:

- Avoid unnecessary secondary labels and descriptions. Let the layout do the explaining.
- Data and code are independent: named implementations satisfy traits; data
  does not. Trait data types are ordinary explicit parameters. There is no
  conformance subject, `Self` type binding, or implicit `self` value. `dyn`
  selects an implementation at runtime; data arguments stay separate. Authored
  implementations declare member machines inline; reuse is an ordinary call,
  not a `Trait::member = Existing::machine;` reference row. Follow
  [named implementations](wiki/spec/language/conformances.md#data-and-code-are-independent)
  and remove legacy support rather than preserving it for old tests.
- Real words in code (`character`, `expression`, `arguments` — not `ch`,
  `expr`, `args`). Names must read outside compiler insiders: `pipeline`
  over `driver`.
- Arena-backed data with generational handles over scattered heap objects;
  ZII — the zero handle is the absence state, so do not wrap handles in
  `Optional`; no `RefCell` as an ownership escape. Durable lowered children
  use `Handle<T>`/`HandleSpan<T>`; `Vec` is construction scratch, not the
  durable shape. Arena/vector-backed symbol tables over local hash maps; a
  map keyed by compiler-owned objects is a review trigger for missing
  representation structure. Symbols are handle-first — string names are
  debug/export payload, not identity. Source text stays in loading,
  diagnostic, and debug payload roles.
- Entrypoints stay prominent: `main.rs` owns startup and visible dispatch
  to real subsystems. Split at responsibility boundaries.
- Comments explain non-obvious intent; `lib.rs` headers carry the reasoning
  signatures cannot — mechanism, justification, honest boundaries,
  provenance.
- Commit subjects are `lane: statement` — pick the lane by changed
  responsibility per [commit naming](#commit-naming) below.
- `TASKS.md` supplies capability priorities and remaining acceptance; the
  sample gate supplies executable witnesses, not the priority order. Keep
  capability outcomes, external prerequisites, release acceptance and unresolved
  regressions on the board, not one item per sample or diagnostic.
  `TASKS_BOOTSTRAP.md` and `TASKS_OPTIMIZER.md` keep the bootstrap chain and
  optimizer work, which have their own acceptance. An item exists only while
  it names unfinished work and is deleted when its acceptance passes; state
  its current frontier once, replacing the paragraph rather than appending
  history. Owner decisions go to `OWNER_QUESTIONS.md`. Nobody owns the boards;
  reconcile concurrent edits at rebase.
- Windows and macOS are both supported development hosts; shared tooling
  needs a documented usable entrypoint on each — a `.ps1` alone is not
  macOS support; either document `pwsh` as an explicit prerequisite or
  provide the portable route. Omega is the intended implementation language
  for tooling; existing Python/shell implementations are removed when Omega
  replaces them.
- Language fixtures name the compiler behavior they pin
  (`tests/omega/{pass,fail,run}/<behavior>/`), not the sample that exposed
  it; `samples/` are language pressure tests: runnable programs whose header
  documents an `Expected exit` or `Expected output contains` oracle. A
  check-only probe that binds no program entry pins one behavior and belongs
  in the fixture tree. Existing projects and separate invalid controls retain
  their [discovery roles](samples/README.md#project-roles-and-discovery). Samples
  may evolve through use into complete programs or reusable kernels; a stable
  contract does not freeze implementation. One owner extracts an approved kernel
  into `source/library` and migrates its consumers together, preserving oracles
  and negative coverage. Do not create another permanent hierarchy or stale
  inventory. A Rust test of one crate stays beside that crate.
- Write direct prose: state the idea in literal words.
- Read the matching skill before starting: `.agents/skills/` — `advance` for
  the sample loop, `rust-systems-programmer` for Rust implementation and review,
  `architecture-cleanup`, `task-board-cleanup`, `local-swarm`, `devin-swarm`,
  `codex-swarm`, `cursor-swarm`.

Deliberate configuration — do not "fix" it: `clippy.toml` thresholds are
raised and dev/test profiles use `debug = 0` and `opt-level = 1`;
`proof-admission` and `sha2` use `opt-level = 3`. Check `Cargo.toml` and session
overrides before interpreting compiler timings. Debug information can be
restored with `CARGO_PROFILE_DEV_DEBUG=2` or `CARGO_PROFILE_TEST_DEBUG=2`
while retaining optimization. An unoptimized compiler is a build error, not a
supported validation producer. Runtime environment settings do not change the
profile embedded in an existing binary.
`.gitattributes` forces LF because canonical source and evidence identities
are byte-sensitive.

## Scope and escalation

- The session that starts work finishes it. A claim records what a session
  is doing so other sessions can see it; it grants nothing and blocks nothing.
  Parked work is available to any session once its prerequisite is met.
- A sample's work includes the complete application and the compiler repairs
  it needs to run; carry it through source, native/Terminal execution, invalid
  controls and publication. Several failing samples with one demonstrated root
  cause are one piece of work; shared files or pipeline stages are not a
  reason to group. Never divide a sample into stages for different agents. Only an actual external prerequisite (an owner decision, an
  unavailable environment) gates work.
- Keep a short record in the claim note: parent task, remaining acceptance,
  current witness and the requirement it discharges, current failure, next
  necessary action and prerequisites. Write it in words a person can read
  without the diff: what of the customer now runs, and what of it still fails.
  Explicitly named standalone work need not
  invent a parent task. Reread the note after context compaction
  and before changing direction. A checkpoint is not completion. A user stop
  or exhausted budget preserves the frontier without claiming success.
- A newly discovered obstacle belongs to the work that found it: fix it
  rather than filing a task for someone else. Do not split work or mine
  tasks to keep agents busy; narrow parallel slices build duplicate routes for
  the same operation. An empty facts queue calls for the next uncovered place,
  or a new fact drawn from an open `TASKS.md` item's remaining acceptance
  (step 4 of the sample loop).
- Overlap is expected and needs no permission. A Git conflict is a design
  signal: two sessions are changing one decision point, so reconcile them into
  one implementation at rebase. Conflicts only show textual overlap; semantic
  breakage can still land without one; post-merge checks detect those regressions.
- Keep regressions visible and repair or revert them. Required merge admission is
  the small GitHub `compiler-check`, not the entire corpus or customer matrix.
  The hosted Compiler regression workflow runs Linux native/Terminal smoke after
  merge and nightly. The regressing PR's author owns repair/revert; until
  attributed, the first observer records the failure and takes triage ownership.
  No machine runs all three OSes, so each host with
  a native golden runs `python tools/native_watch.py` on a
  schedule ([testing](tools/testing.md#corpus-outcome-gate)). The watch runs
  the corpus only, and admission checks compilation only, so the Windows
  x86-64 development host also runs the full
  `python tools/sample_native_gate.py --programs-only` once a day from Task
  Scheduler ([testing](tools/testing.md#corpus-outcome-gate)); a sample that
  passed in the previous full record in `build/sample-gate-runs/` and fails
  now is a regression. Agent sessions, local or cloud, never start a full
  programs-only run for this check. Between 1483ea14d01 and 50ace165363, at
  least 10 passing samples, including `basics/print_number`, stopped
  building and no check saw it. Whoever sees a
  regression attributes it to its first bad commit and drives a focused repair
  or revert through the queue. Record it in `TASKS.md` only when it cannot be
  repaired in that session, and say exactly what prevents the repair.
- After landing a sample, an agent may take a bounded
  iteration-performance or test-environment outcome based on an experienced
  obstruction. Record the workflow and adequate target before repair; return
  to the parent capability when it meets that target. No incidental profiling detours.
- Report which samples now run, which capability requirements their evidence
  removes, and what remains; do not count activity or commits as closure. Missing implementation
  and file overlap are work to resolve, not blockers. An actual missing owner
  decision, inaccessible environment or external dependency may pause that one
  piece of work; record it and continue everything else. Unrelated busywork is
  not a substitute.
- Language/architecture decisions go in `OWNER_QUESTIONS.md`, referenced from
  the affected sample's README; engineering decisions stay with the session doing
  the work. Only the owner answers, closes or rewrites an owner question. A
  session that believes the specification already answers one reports its
  citations to the user and changes nothing: not the entry, not the
  specification, not the boards. A guide example, a lower-level contract or the
  Rust compiler's behavior never ratifies a language rule. Gates pass on real behavior. The accepted bootstrap
  chain changes only through its proofs.

## Sessions and subagents

Before launching a new swarm or creating a new worker, successfully fetch remote
`main` and record its exact commit. Fast-forward a clean checkout on `main`;
preserve dirty or non-main checkouts and create new worktrees from the fetched
commit. New worktrees go in `<repository>/.agents/worktree/<short-name>`.
That directory is gitignored local state. An existing checkout stays where it is. Read `AGENTS.md` and the matching skills from that updated source before
selecting or dispatching work. A failed fetch stops the new launch; a cached
`origin/main` is never a fallback. Each independent cloud executor repeats this
preflight before implementation. Existing outcomes retain their worktrees and
unpublished work, integrating fetched main at a safe checkpoint outside an
active qualification capture. Offline plans and dry runs are previews, not a
completed launch preflight.

- Give a session or subagent a complete piece of work, never a stage: the
  parent capability and remaining acceptance, its next sample and oracle,
  and that it lands its own result and continues the capability within the
  authorized scope and stopping condition.
  State directions directly; never imply or insinuate them. The session
  carries authorized work through implementation, testing, review, normal
  main landing and remote verification, and keeps working until it is
  complete.
- Dot-created Omega cloud sessions and their native workers default to
  **GPT 6.1 Sol, medium reasoning** (`gpt-6.1-sol`, `medium`), unless the user
  specifies another model or effort. Select them through the runtime's exposed
  launch fields; a prompt alone cannot change a model. Before launching or
  resuming cloud work, follow the [cloud launch preflight](.agents/skills/dot-swarm/references/cloud-launch.md).
  A single advance remains one assignment; it does not authorize a swarm.
- A requested Cursor cloud wave follows
  [cursor-swarm](.agents/skills/cursor-swarm/SKILL.md). Each worker is a
  Cursor Task subagent with its own VM and branch, started from fetched
  remote main. Select its model through the exposed Task fields.
- Parallel sessions take distinct samples, chosen by root cause, not by
  file. Shared implementation paths are fine; each session works in its
  own worktree and reconciles at rebase. Reuse another session's repair when
  practical; never wait for permission to touch files.
- Read-only review or scouting does not authorize writes or publication. A
  user-required independent review remains required; no other approval or
  reviewer gate exists.
- Sessions record progress in their claim note
  (`python tools/claims.py note --ticket`) and their final report. Nobody
  reconciles another session's work on its behalf.
- When selection policy changes, coordinators explicitly send active workers
  the updated instructions and parent-outcome context at their next safe
  checkpoint. Do not assume a file edit refreshed a running session's context;
  preserve its current witness, unpublished work and original user limits.
- Continue implementation when the design answers the question; when it does
  not, put the exact missing decision to the project owner and keep working
  on everything else.

## Commit naming

Use `lane: statement`. Choose the lane from the changed responsibility
below — not the repository name, implementation language, or a prefix
copied from Git history. Omega names the project, a compiler stage, and a
bootstrap implementation; those meanings stay distinct in subjects.

| Lane | Responsibility and current path anchors |
| --- | --- |
| `alpha`, `beta`, `gamma`, `delta`, `epsilon` | The corresponding language rung in `bootstrap/<rung>/`, including its compiler/evaluator, Rust reference tooling, and rung-specific tests. Use the actual rung name. |
| `bootstrap-omega` | The Epsilon-written first Omega compiler D in `bootstrap/5_omega/` and tests specifically of that implementation. Never shorten this to `omega`. |
| `bootstrap` | Cross-rung reconstruction, trust-chain edges, artifact provenance, and chain hygiene; includes `tests/bootstrap/` and shared `tools/bootstrap/` orchestration. A helper specific to one rung uses that rung's lane. |
| `psi` | Target-neutral source semantics through Terminal Psi: parsing, resolution, typing, checking, proof, interpretation, and Psi optimization in `omega-rust/psi/` or `source/psi/`. |
| `omega` | The Terminal-Psi-consuming compiler stage: Omega representations, transforms, optimization, and realization semantics in `omega-rust/omega/compilation/{representations,pipeline,realization}/` or corresponding `source/omega/` implementation. Never a project-wide default. |
| `backend` | Target, ISA, ABI, object/image encoding, layout, and execution primitives in `omega-rust/omega/compilation/backend/` and their product-source equivalents. Transform policy in `pipeline/` remains `omega`. |
| `compiler` | Product compilation coordination and reports in `omega-rust/omega/compilation/driver/`, the source-free native driver in `omega-rust/omega/compilation/native-production/`, or an inseparable compiler contract change spanning Psi and Omega. A coordinator call-site adjustment accompanying a stage fix keeps the stage's lane. |
| `build` | The product's Build activation in `omega-rust/omega/compilation/build-evaluation/`, plus product build declarations such as `source/omega/build.omg`. Provider planning and trust admission under `realization/` use `omega`; CLI policy-file handling uses `cli`. Repository build commands and CI use `repo`. |
| `packages` | Package acquisition, graphs, review, admission, installation, and update workflows in `omega-rust/omega/packages/`. Package command wiring accompanying those changes keeps `packages`. |
| `cli` | Command parsing, flags, help, and command dispatch in `omega-rust/omega/application/cli/src/`, plus the corresponding product entrypoint. The command's underlying compiler or package behavior uses its owning lane. |
| `tooling` | Shipped developer tools and shared host support in `omega-rust/omega/foundation/`, including artifact file writing and measurements. Semantic trust records and admission under `realization/trust-model/` use `omega`. |
| `library` | Bundled packages in `source/library/`, including `core`, `alloc`, `std`, and ordinary domain packages. A compiler semantic fix exercised by library code keeps its compiler lane. |
| `samples` | Standalone sample/example content and presentation in `samples/` or example directories. A regression fixture for an implementation fix keeps that implementation's lane. |
| `tests` | Shared test infrastructure, corpus registration, fixtures, or architecture gates spanning multiple owners. Tests for one responsibility use that responsibility's lane, even in a test-only commit. |
| `tools` | Repository maintenance/development utilities in `tools/`, including merge-queue clients and sample-refresh scripts. Bootstrap tools follow the rung/chain rules above; shipped tooling uses `tooling`. |
| `repo` | Repository-wide workflow, agent instructions/skills, CI, workspace/dependency configuration, formatting policy, and Git settings. Includes standalone policy changes in `AGENTS.md` and `.claude/`. |
| `docs` | Explanatory documentation-only changes in `wiki/`, README files, or guides. Executable agent instructions use `repo`; documentation accompanying an implementation change keeps its implementation lane. |

Apply these selection rules in order:

1. Read the diff and identify the responsibility whose behavior or contract
   changed. The more specific table row wins over an enclosing directory:
   `omega-rust/omega/packages/package-manager/` is `packages`, not `omega` or `repo`.
2. Supporting tests, fixtures, documentation, task-board updates, and
   mechanical call-site edits inherit that responsibility's lane.
   `tests/omega/` names the language corpus; a typing regression there is
   `psi`, an encoding regression is `backend`. `TASKS*.md` and
   `OWNER_QUESTIONS.md` follow their subject; changes to the board workflow
   itself use `repo`.
3. For independently useful changes in different lanes, split the commits.
   For one inseparable change, use the responsibility whose contract
   required the other edits. File count does not decide the lane; do not
   invent combined prefixes such as `psi/omega` or use `repo` merely
   because many files changed.
4. New files in an existing responsibility use its existing lane. Only a
   new responsibility absent from this table needs a new lane: choose a
   short lowercase area name, document its scope here in the same change,
   and explain its ownership in the body. Do not create synonyms for
   listed lanes.

The statement is lowercase and declarative: name the resulting behavior or
structure. Aim for about 68 characters for the whole subject; 85 is the
maximum. No trailing period, ticket-only title, or bare `update`. No second
change-type prefix such as `feat:` or `fix:`. Fact-row and fixture commits
follow the same rule: they take the lane of the behavior they pin, or
`samples` when they add only sample content. `facts(...)`, `tests(facts):`,
`docs(workflow):` and a bare `Pin ...` or `Complete ...` subject are not lanes;
496 of 1,736 commits from Oct 15 to Oct 21 had no lane.

The body is prose paragraphs after a blank line, with numbers in place of
adjectives. Cover what applies: what the previous behavior was and why it
was wrong; which alternatives were rejected; which constraint shaped the
diff; which gates ran, named with counts rather than the word `pass`; what
is known red or deliberately left undone. Omit the body only when the
subject carries the full reasoning (typo, rename, mechanical revert).

Commit messages carry no tool attribution: no "Generated with ..." line, no
`Co-Authored-By` trailer naming an agent or vendor.

## Hard rules

- **Do not add required feature-combination special cases. Clean as you go.**
  Generalize the touched operation or representation and remove its superseded
  path in the same change; do not land another source-shape recognizer, fallback
  or mode with cleanup deferred to a task. Preserve genuine semantic and ABI
  distinctions. Optional optimization specialization requires a correct general
  path. See [compositional lowering](omega-rust/pipeline.md#compositional-lowering).
  A repair counts only when the same proved fact (a length, a bound, or a loan) works through a local, a field, a parameter, a call result, a match join, a const name, another state, another machine, and another target body, as pinned by the fact-place fixtures under [`tests/omega/pass/facts`](tests/omega/pass/facts) and [`tests/omega/fail/facts`](tests/omega/fail/facts). When the fact is a requirement the producing layer dropped, the sentence names the requirements recorded on the emitted value, the consuming operation uses that list, and another production that drops the same requirements stays in that fact folder.
- **Verify at trust boundaries, not after every stage.** Foreign package review
  and untrusted build-script execution verify; a stage never re-validates,
  replays or rebuilds output the same compile just produced. Verifiers stay
  exercised by the corpus runner and tests, off the default compile path. See
  [verification placement](omega-rust/pipeline.md#verification-placement).
- **Never push to `main` directly.** Publish a finished branch as a PR and use
  GitHub's native merge queue ([tools/landing.md](tools/landing.md)). The queue
  checks the integrated candidate and squash-merges one PR into one main commit.
  Do focused validation and any required review on the submitted change; do not
  repeat author rebase/build/review merely because main or the integration SHA
  moved. Resolve actual conflicts, failures and changed semantic premises.
  `tools/landing.py` only observes/enqueues the native queue: no local lease,
  exact-SHA build receipt or direct-publication path remains. An open PR is not
  completion. Verify MERGED, its exact submitted head and published main commit,
  then remove only task-owned clean worktrees/branches without later unpublished
  edits. Preserve run identities; CI admission is not full runtime qualification.
- Before editing shared paths, check `python tools/claims.py status` and
  record a claim for your work ([tools/claims](tools/claims.md)) so other
  sessions can see it. Claims never grant or block work. If coordination is
  unavailable or times out, report it and continue in an isolated worktree
  with the intended scope recorded locally. Do not wait repeatedly, treat an
  outage as a conflict, or claim registration succeeded. Path overlaps are
  informational; reconcile overlapping changes at rebase. Existing tickets
  remain valid; refresh old clients that reject path overlap. Publication
  still uses the landing queue and never overwrites incoming work.
- Keep sample coverage out of the shipped CLI.

## Design references

- [Omega Language Guide](wiki/language_guide/language_guide.md)
- [Compiler ownership and pipeline](omega-rust/pipeline.md)
- [Documentation index](wiki/README.md)
- [Terminal Psi product contract](wiki/spec/terminal-psi/product.md)
- [Optimization phases](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
- [Rust compiler completion](wiki/drafts/reference/rust_compiler_completion.md)
