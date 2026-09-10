# Tasks

This file is the current cross-project execution board, not a changelog.
Completed work belongs in Git history and the durable architecture/design pages.
Detailed bootstrap and optimizer work lives only in
[`TASKS_BOOTSTRAP.md`](TASKS_BOOTSTRAP.md) and
[`TASKS_OPTIMIZER.md`](TASKS_OPTIMIZER.md).
Package-manager work belongs here when there is a concrete remaining task.

A task remains here only when it names:

- unfinished work;
- its owning code/design area;
- a real blocker, if one exists; and
- a concrete acceptance condition.

Remove a task when its acceptance condition passes. During an active change,
retain only the context needed to resume it. Do not append landed substeps,
version-bump history, test counts, or release notes here. If a task grows beyond
roughly three short paragraphs, move the design detail to its owning document
and leave a link plus the next executable step.

Before starting work, fetch `main`, inspect the newest commits in that lane, and
avoid overlapping an active change. Commit and push coherent milestones.
Engineering difficulty is not a design blocker. Owner decisions belong in
`OWNER_QUESTIONS.md`. Research without a current customer does not belong on
the execution board; recover it from its design document or Git history when a
real customer appears. Do not mirror owner-question or customer-gated indexes
here.

## Ownership firewall

Psi operates on Omega source and owns parsing plus all target-neutral semantics
through Terminal Psi. Omega consumes Terminal Psi and owns provider selection,
optimization, target realization, native emission, and general execution
machinery. Target backends own unavoidable ISA, ABI, object-format, and
relocation encoding. Cathedral owns OS data structures, policies, protocols,
and lifecycle.

Compiler guarantees are established by checking and artifact verification;
they require neither an accepted package lock nor a proof-bearing
`PackageInstance`. Unfinished native realization does not block source package
installation. Unsupported compiler forms reject at their owning stage.

If Cathedral cannot express a subsystem, identify the missing general Omega
primitive or mark the slice blocked. Do not implement page tables, descriptor
tables, schedulers, process tables, timer queues, or drivers as compiler-owned
Rust models. Compiler validation and code generation may consume general plans;
they must not acquire customer-shaped semantic types or lifecycle protocols.

## Compiler throughput

These are bounded work-removal tasks, not a new performance framework. Preserve
the [pipeline ownership and independent checks](omega-rust/pipeline.md).
Source inspection identified the costs below; it did not establish their share
of whole-compilation time. Record compared inputs, work counts and timings;
do not claim a faster compiler from a smaller helper alone.

- **FLOW-DIRTY-STATES.** In Psi
  `pipeline/typed-trees-to-checked-trees/src/flow/{builder,state_values}.rs`,
  assess and remove remaining semantic-baseline content copies between dirty-state
  sweeps where measured checking cost warrants it. Output arenas and whole-plan
  replacement already reuse allocations; copying baseline contents remains.
  Reset derived context-point links as well as arena contents; no scratch handle
  may escape or resurrect. Preserve the first-pass fast path and final source-order
  materialization rather than appending stale state evidence into live output.
  Preserve all-predecessor intersection, absorbing unknown, stable evidence and
  conservative nonconvergence fallback. Acceptance: a reverse-ordered/cyclic
  chain beside many independent machines revisits only affected transfers during
  convergence, without cloning the baseline per sweep; facts, rejection and
  evidence agree with the test-only whole-pass reference. Use the builder's
  `complete_checking_matches_reference_with_reverse_chain_and_cycle` regression
  through crate-scoped nextest on macOS; it compares complete checking
  and reports state builds/timing. Measure with
  `cargo run -p typed-trees-to-checked-trees --example checking_allocations`:
  macOS comparison against `14e9fd8173` reduced requested bytes from 10,178,523 to
  8,177,845 (12 reverse states/64 contract machines) and 83,801,017 to 63,158,867
  (32/256), without establishing a timing win. Direct baseline-restoration probes
  were a small share of checking time on these fixtures. Before a suffix-reset
  design, establish a customer-sized copying bottleneck that justifies repairing
  prefix context links and preserving generation/span invariants; do not add a
  rewind framework solely to close this item. No thread-pool or unrelated IR
  redesign is part of this task.

- **PACKAGE-PREPARATION-REUSE.** In
  `omega-rust/omega/packages/manager/src/review/candidate/compilation.rs` and its compiler
  source-preparation owners, identify and retain binding-independent source
  preparation across discovery and final checking. Consumer bindings really
  change (including std), so do not reuse an obsolete checked result or replay
  build effects without their authority/accounting. First attribute phase self
  time and select one demonstrably repeated phase with exact reuse inputs.
  Acceptance: the already-built release CLI's `cli_mvp` package-review command
  performs that phase once for unchanged inputs, invalidates on source/binding
  changes as appropriate, and preserves complete findings, generated-source
  custody and admission. Compare whole-route time with the SAMPLE-CORPUS probe;
  no generic persistent cache or second package-review workflow.

  Immutable parse-checkpoint exposure is deferred on measured cost, not a design
  blocker. At `9d9075eb61`, the already-built release CLI's macOS ARM64 `cli_mvp`
  route took 19.37 seconds; a temporary phase probe took 19.36 seconds with the
  same missing-acceptance findings. All four source preparations totaled 46.248 ms
  while recorded checking phases totaled 12.674 seconds. Reusing preparation
  across the two passes can save only part of that 46 ms and still clones child
  syntax. Do not add a public preparation handle or review cache for that cost.
  Resume only when a repeated binding-independent phase has material measured
  self cost; preserve the existing session, generated-source and policy checks.

Optimizer revision/analysis reuse is tracked only in `TASKS_OPTIMIZER.md`.

## Semantic reflection

Implement [semantic reflection](wiki/spec/language/reflection.md) in Psi schema
construction, hermetic evaluation, checked per-member calls, and Terminal replay,
with ordinary library inspector/encoder policies. Use named callbacks, explicit
context data, and existing callable-family checking; do not add a closure IR or
format-specific compiler. Anonymous syntax is not an implementation dependency.
First deliver owned qualified schema graphs, authorized projections, and scoped
typed selections frozen into independently checked result snapshots. Continue
with recursive derivation, explicit runtime metadata/adapters, and authorized
Placed access under the same contract. Fixed callback ceilings do not depend on
the separate callback-forwarding owner question.

Acceptance: one inspector and one serializer use the same visitation mechanism;
a 40-field record with five field types uses five reusable policy rules plus
an explicit same-type member override. Missing, duplicate, wrong-type, stale-key,
and insufficient-contract selections reject. Cover full qualified field types,
ordinary predicate weakening and explicit semantic erasure in selected adapters,
no owned-claim erasure, erased-field runtime exclusion, zero-sized runtime fields,
empty records, distinct nullary cases, common fields once, and active payload only.

Test owner-delegated visitation across three packages and reject unauthorized
library enumeration; descriptions confer no grant or private access. Test fresh
reborrows and escape rejection, dynamic-index schema relationships, snapshot
independence from compiler storage, explicit exhaustion without truncation, and
recursive reference types reusing only the exact pending derivation while still
checking all obligations. Runtime recursive encoders use provisioned work storage,
not hidden stack growth. Retained adapters preserve loans, resources, and exact
Placed access; decoding and editing still require owner construction/update
contracts. Run corruption/replay controls independently of the producer.

## Process-exit contract

Implement the [canonical process-exit contract](wiki/spec/language/process_exit.md)
for CLI programs that deliberately exit, including conditional exits through
helpers. Core must declare the exact `ProcessExit::exit_process(i32)` requirement
and separate its authority/reach from console I/O. Psi owns canonical recognition,
checking, conditional control and ownership, Terminal completion, codec,
independent verification, observation, and interpreter support. Omega owns
provider conformance and realization; targets supply domain-bound authority and
physical exit under the same contract. Preserve the current trace distinction;
do not infer terminal meaning from the transitional Unit call or a native syscall.

Acceptance: source-produced unconditional and conditional exit cases retain
exact requirement/argument identity through canonical replay and target
realization. Returning branches retain cleanup and postconditions; exit branches
abandon without discharge receipts. Reject missing authority/reach, lookalike
requirements, returning or aborting substitutes, unmet progress premises, exit
from automatic cleanup, and unsupported survivor contracts. Root return cannot
hide unfinished task custody. The interpreter ends only its simulated domain;
native tests on each available host preserve status mapping and ordered output,
with unavailable-host coverage explicit. Tampered identity, arguments, completion
rows, or provider evidence reject independently. General completion syntax and
other terminal services are not prerequisites.

## Immediate product closure

These are the next product-level priorities for the maintained Rust
implementation. They take precedence over adding another evidence carrier that
has no exercising program. The finite definition of Rust-product completion is
the [Rust compiler completion plan](wiki/drafts/rust_compiler_completion.md).

- **NATIVE-DIFFERENTIAL-ACCEPTANCE.** Restore the remaining acceptance failures in
  `tests/native-differential/tests/{terminal_psi_source,pipeline_ownership}`.
  Resume on macOS AArch64 with `cargo nextest run -p omega-native-differential-test
  --test terminal_psi_source --test pipeline_ownership --no-fail-fast --no-tests fail`.
  Next: restore build-bound progress publication in
  `selected_source_entry_retains_build_bound_progress_for_terminal_publication`.
  On `b76d693e30` plus the selected-syscall handoff repair, the focused command
  above with only `--test terminal_psi_source` and
  `-E 'test(=selected_source_entry_retains_build_bound_progress_for_terminal_publication)'`
  reaches native receiving-policy validation and rejects unclassified syscall
  231 under policy version 7 (macOS AArch64, pinned nightly, `RUST_MIN_STACK=33554432`).
  The fixture still maps ordinary Unit `Scheduler::finish` to physical process
  exit. Reconcile that with [Process-exit contract](#process-exit-contract), then
  supply independent receiving policy and retain the existing missing-settlement,
  exact-provider, progress-attestation, and publication assertions. Selection or
  a syscall number must not confer canonical ProcessExit semantics or authority.
  Owners: `compiler/src/pipeline/checked_entry.rs` and native realization's
  `realization/providers/settlements/source_imports.rs` under
  `omega-rust/omega/compiler/`.
  Acceptance: both test targets and their scoped Clippy pass without filtering
  failures; this does not replace the completion plan's hosted native matrix.

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Establish the production compiler as two
  sibling Omega packages: target-neutral phases under `source/psi/` and the
  Terminal-Psi-consuming product under `source/omega/`, with hosted entrypoints
  at `source/omega/{build.omg,main.omg}`. The maintained Rust compiler is the
  differential implementation, not source for this task. Work backward from
  complete Omega behavior in small, live vertical slices; do not create a
  bootstrap-private dialect, file allowlist, or parallel source-to-native path.

  Acceptance: the exact Omega source closure implements the complete language
  and production pipeline, passes the shared product suite, and publishes a
  deterministic manifest of every transitive compiler/build input. Bootstrap
  construction of that closure belongs in `TASKS_BOOTSTRAP.md`.

- **MACOS-APPLICATION-PUBLICATION.** Implement the
  [settled publication contract](wiki/spec/build/macos_application.md)
  in build evaluation/realization inputs, Mach-O signing, command publication,
  and compilation reports. Preserve flat/report validation and flat v1 digests.
  Specify the identifier field in ordinary build vocabulary,
  carry `CheckedCompilation::application_intent()` through native realization
  separately from PE integers, and bind native signing identity before emission. Publish one whole validated `.app` with
  distinct checked package-root and inner-executable accessors.

  Acceptance: the specification's stage-requiredness, deterministic bytes, cross-invocation,
  tampering, partial-output, and flat-regression controls pass; GUI samples carry
  authored identifiers and consumers use reported paths. Validate the procedural
  GUI cohort on macOS, recording unavailable-host coverage explicitly. Resource
  inclusion/lookup for `image_viewer` remains outside v1; do not claim Finder
  runtime coverage for it or silently change its working directory.

  The macOS ARM64 `window_app` [outer command](samples/gui/window_app/README.md)
  at `bdf3195a4a` exits 1 at ordinary package review: acceptance is missing or
  current requirements need review. No phase artifacts are produced. Complete
  ordinary package review before identifying its next native failure; do not
  infer it from the build-intent tests. The checked intent owner is
  `build-evaluation/src/lib.rs`, carried by `compiler/src/pipeline/checked_entry.rs`.
  The retained native proposal still carries only the PE word.

- **SAMPLE-CORPUS.** `mbx test -p compiler --test samples_compile` is red.
  `cli/proofs/math_proofs` needs ordinary core multiset data and slice-to-proof
  extraction machines, then explicit imports and exact declaration selection
  for its `Bag(...)` claims. Chapter 10's Proof Views section makes these
  source-defined types and lemmas, not compiler-installed term formers.
  `source/library/core/seq.omg` supplies recursive sequence data but no Bag
  definition or slice extraction. Preserve the multiset-equality sample and
  its false twin; do not replace extraction with spelling-based proof atoms.
  Extraction equations additionally need structural proof terms for indexed
  values and subslices, retaining exact source place, index/window, and live
  revision. Equal lengths or display strings cannot establish equal contents.

  Source cases that write text buffers by index or transport nominal fields
  through collections also need `NOMINAL-FIELD-FLOW` below; their default-domain field
  obligations must be proved, not bypassed to restore sample acceptance.

  Work from the unchanged `print_squares` [outer command](samples/cli/basics/print_squares/README.md).
  Resume its native compiler-library probe with `OMEGA_SAMPLE_RUNTIME_FILTER=print_squares`
  and `cargo nextest run -p compiler --test samples_compile --no-fail-fast --no-tests fail -E 'test(=samples_with_documented_exit_run_correctly)'`.
  At `a8908f279b` on macOS arm64, this exits 100 before
  execution:
  `InvalidUnitMachinePlan` names `Main::main` with `attached Unit closure is missing a checked transitive machine plan`.
  Complete Main's checked transitive Unit closure without replacing its authored
  graph. Source inspection identifies remaining projected `self.out` call
  arguments and the raw fixed `pause` array passed to `read_line`; the probe
  still reports only the missing Main plan,
  not an individually diagnosed ordering of these gaps. The source owners are
  `typed-trees-to-checked-trees/src/flow/terminal_unit/state_graph.rs` and
  `typed-trees-to-checked-trees/src/flow/terminal_unit/control/statement_sequence.rs`
  with byte-field argument selection in `terminal_unit/calls.rs`.
  Retain the source live-prefix judgments in
  `typed-trees-to-checked-trees/src/checks/ranges/assignment_lengths.rs`;
  capacity must not substitute for live length in the portable plan.
  Native realization must consume the portable whole replacement, field-length
  observation, and indexed replacement; it still rejects all three before
  projection. Their producer owners are
  `checked-trees-to-lowered-psi/src/structural_byte_sequence_store.rs` and
  `checked-trees-to-lowered-psi/src/structural_byte_sequence_index_store.rs`.
  Reuse literal/length observations and checked source predicate
  obligations; predicate-only `Utf8` erasure does not itself require adding a
  projected qualification roster. At the same revision, the CLI outer command
  using Cargo separately exits 1 awaiting ordinary package review, with no phase
  reports or executable produced; do not manufacture acceptance.
  Producer widening alone cannot close this: general owned cyclic validation
  and native execution remain missing under `GENERAL-CYCLIC-EXECUTION` below.
  Retain the actual state graph, field arithmetic, text initialization, and
  runtime-indexed byte stores; do not synthesize separate one-state machines.

  Further helper-by-helper expansion for `print_squares` is paused under
  [scope checkpoints](AGENTS.md#scope-checkpoints): two dependency milestones
  left the customer probe at the same missing Main plan. The accumulated change
  reuses private argument-evaluation blocks and computation handles without a
  new Terminal format, but does not establish whole-customer feasibility.
  Before another implementation slice, reassess the complete source-closure and
  native dependencies above against a customer-level acceptance boundary.
  This is a strategy pause, not a language-design blocker; independent board
  work remains actionable.
  Main's unranked cycles do not need an invented termination witness.
  Acceptance remains native execution with the documented exit/output and
  unchanged checked text facts; the console dependencies below are also required.

  Work from the actual `cli_mvp` [command and route](samples/cli/basics/cli_mvp/README.md).
  The macOS ARM64 outer command
  `CARGO_INCREMENTAL=0 cargo run --release -p omega -- --target macos_arm64
  --build-dir build/cli-mvp-ranks samples/cli/basics/cli_mvp/main.omg`
  requires ordinary project acceptance before native production. Std's filesystem
  authority and external Console leaves remain review findings, not implicit
  grants. Do not manufacture acceptance merely to advance the sample. The inner
  sample harness bypasses this policy route; independent writer work remains
  actionable.

  Measure remaining package latency with an already-built release CLI, separately
  from Cargo build time. At `f96d04b5ec` on macOS ARM64, the command above
  (`--build-dir build/cli-mvp-lookups`) exits 1 at missing package acceptance
  in 17.30–17.45 seconds with unchanged complete findings. Further symbol-lookup
  micro-optimization is paused under the scope checkpoint: name grouping,
  accessor inlining, prepared exact-machine lookup, and avoiding impossible
  attached-data scans did not establish a useful whole-route gain. Resume that
  strategy only with phase-level evidence of material avoidable cost; select
  from self cost rather than inflated recursive stack counts, and confirm through
  this outer command. Do not add another cache or weaken arena validation on
  the strength of isolated query timings. This is not a language-design blocker;
  continue independent compiler work meanwhile.
  Std itself changes semantic bindings between discovery and final checking, so
  retaining unaffected dependencies would not eliminate its second compilation.
  Preserve distinct source/selection inputs, conservative unknown-write handling,
  complete findings, and admission checks. Acceptance remains prompt whole-route
  diagnosis with comparable timings and unchanged findings. Windows timing is
  unverified; this work does not block the native operand work below.

  Resume the downstream native `cli_mvp` probe:
  `OMEGA_SAMPLE_RUNTIME_FILTER=cli_mvp
  cargo nextest run -p compiler --test samples_compile
  samples_with_documented_exit_run_correctly --no-fail-fast` exits 100 before
  execution at `native artifact ProgramEntry receiver provisioning failed`
  (macOS ARM64, `48daf28aa4`). The entry retains `self`, but no root-backed
  bridge constructs and lends its receiver. Resume `ENTRY-CONTENT-ROOTS` below;
  retain the rejection in `native-realization/src/realization/native_artifact.rs`
  until that bridge actually provisions admitted storage and the activation loan.
  The fixed-array boundary mismatch no longer blocks this command. Its
  source-free initialized-buffer/provider regression lives in
  `checked-trees-to-lowered-psi/src/tests/fixed_array_boundary_providers.rs`.
  Preserve the real initialized array, exact field path, exclusive loan, fixed
  extent, and structural result cleanup. Do not substitute bounded-owner
  replacement, a line intrinsic, or selected direct calls in portable Psi.
  Later native installed-provider argument replay and borrowed-view/result
  realization remain separate dependencies in
  `terminal-psi-to-abstract-operations/src/provider_installation/replay.rs` and
  `abstract-operations-to-target-operations/src/lowering/unit/boundary_call/installed_provider.rs`;
  these are source-inspected fences, not the current observed failure.
  The native sample harness now supplies exact test-owned Console acceptance and
  receiving permissions through the shared canary helper. Byte-output and exit
  classification succeed; fixture acceptance does not replace the CLI's package review.
  Checking-only sample probes remain unaccepted, and stale-target bindings reject.
  The macOS byte leaf and callable line loop have separate native evidence below;
  they do not establish the complete entry-owned caller. Do not substitute Linux
  or interpreter output for that acceptance.
  Terminal production retains authored boundary calls; native provider selection
  owns adapter realization, independently of interpreter dispatch. Preserve the
  [borrowed-byte writer closure](omega-rust/psi/compiler/terminal-production/README.md#borrowed-byte-writer-composition)
  from `typed-trees-to-checked-trees/src/flow/terminal_unit/` and
  `checked-trees-to-lowered-psi/src/attached_unit/`.
  The private writer calls its concrete provider's byte leaf directly; extending
  plain boundary-trait or `Service` forwarding is not a prerequisite.
  The callable plan must retain exact intrinsic settlement, view/scalar transfers,
  length and guarded head/tail observations at selected edges, and
  slice-decrease evidence.
  Reuse the native writer regression floor in
  `tests/native-differential/tests/terminal_byte_views/natural_writer.rs`:
  source-produced grouped slice proofs, exact current-body replay, ordinary
  cyclic Unit control and descriptor arrivals, byte output and caller continuation.
  `cargo nextest run -p omega-native-differential-test --test terminal_byte_views
  natural_writer --no-fail-fast --no-tests fail` checks Linux x64/ARM64 and macOS
  object/image/installation publication plus published-text execution on the host.
  Keep the verified artifact through legalization; raw cyclic IR, changed proof
  groups, bodies, literals, effects and transfers must reject. Do not fabricate
  countdown certificates or fixed-work bounds. This test-owned output settlement
  is not package/provider acceptance for the actual sample.
  Linux runtime remains unverified on macOS. Mixed scalar/view expression and
  returned-call image publication remain unsupported and are not prerequisites
  for this Unit writer route.
  Use `terminal_byte_views/byte_output.rs` for the shared hosted `i32` byte leaf's
  selected/frame/object/image custody on Linux x64/ARM64 and macOS ARM64.
  `byte_output/hosted_runtime.rs` executes published text for all 256 byte values,
  noncanonical upper input bits, failed-write traps, widened calls and caller
  continuation on macOS ARM64; Linux execution requires a Linux host.
  Retain the byte widening, scalar-only calls, branches/joins and derived-view
  Unit-output and object/image/installation regression floor in
  `terminal_byte_views/byte_output/`.
  Preserve the defining operation, normalized byte ABI inputs, exact selected
  call evidence, output order and visible caller continuation; do not fabricate
  a descriptor or scalar return.
  Windows byte output remains a native realization dependency; do not substitute
  another host's provider or interpreter output.
  Connect selected imported-call/fixup custody and its call-frame effects before
  adding Windows API leaves: the current byte pseudo and structural publication
  are call-free and relocation-free, not an admitted `GetStdHandle`/`WriteFile` route.
  Acceptance: empty/nonempty bytes and both newline settings preserve exact
  output order and caller continuation; unguarded head reads and unchanged
  tails reject. Re-run the same sample before choosing its next dependency.

  `cli_mvp` also calls `Console::read_line(&mut self.pause)` from an attached
  `Main::main(&mut self)`. Remove the transitional ambient/retained-self retry
  in `typed-trees-to-checked-trees/src/flow/terminal_unit/control.rs` once the entry bridge
  passes the `ProgramEntry` loan as structural parameter 0, under
  `ENTRY-CONTENT-ROOTS` and
  `INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION` in P1. Use the shared result-disposition
  and receiver reconciliation described in the [statement sequencer](omega-rust/psi/compiler/terminal-production/README.md),
  not another caller/result/source-order admission family. Receiver-store sequences still need
  aggregate replacements and foreign-result assignments: `win64_direct_aggregate_import_compile`
  combines scalar writes, an aggregate replacement, and a foreign-result
  assignment. Extend the checked Unit statement sequence without dropping
  any write or its exact frame; ordered literal/parameter scalar field stores
  do not cover that complete body.

  Finish native integration of the settled
  [bounded byte input](wiki/spec/resources/bounded_input.md) contract for `cli_mvp`
  and line-reading callers. The bundled fixed-range `LineReadResult` API and shared
  checked provider are the source contract; native entry/caller composition must
  retain its exact result cases and writable range, without changing extent or
  owner live length. No resizable output descriptor or allocator is a prerequisite.

  Resume the native dependency from
  `tests/native-differential/tests/terminal_byte_views/mutable_writes.rs`.
  At `b5aadc0600` on macOS ARM64 (Cargo fallback),
  `cargo nextest run -p omega-native-differential-test --test terminal_byte_views
  --no-fail-fast --no-tests fail -E 'test(mutable_writes)'` passes: fixed-extent
  fill loops retain exclusive state transfers, exact bounds/source operands,
  one-byte stores and once-only fuel through artifact publication and replay.
  Direct and repeated Unit helper calls execute on original caller storage for
  every octet, empty views and multiple extents without altering descriptor words
  or neighboring bytes. Whole raw fixed arrays and nested field-only array loans
  now reach the same native writer using a call-local descriptor over original
  backing, including stack-passed descriptors and runtime scalar arguments.
  Resume those controls in `mutable_writes/fixed_arrays.rs`, including the byte
  argument in its actual outgoing stack position; scalar-result and
  indexed array presentations remain excluded. Publication covers Linux x64/ARM64, macOS ARM64 and
  Windows x64; only macOS runtime was exercised. No termination certificate or
  fixed-work bound is claimed. Source proof/interpretation lives in
  `checked-trees-to-lowered-psi/src/tests/byte_write_loop.rs` and
  `terminal-interpreter/src/structural_byte_arrays.rs`.
  Next dependencies are receiver provisioning under `ENTRY-CONTENT-ROOTS`
  and bundled line-result API/provider integration.
  `Main.pause` is a provisioned receiver field, so generic source-local array
  construction is not a prerequisite for this customer. Explicit host inputs
  supply raw array contents in callable tests; an opaque root supplies none. Zero-length
  fixed arrays retain their Terminal admission fence; empty borrowed views work.
  Do not substitute owner replacement for the fixed-range writer contract.

  Implement shared checked line assembly over `read_byte` with bounded indexed
  writes, or an exact conforming target provider. Preserve source place, path,
  borrowed extent, returned prefix/count, and once-only effects through Psi,
  Terminal, native realization, and replay. The current boundary-only
  `TerminalBoundaryByteBuffer::replace` path and source-produced
  `boundary_byte_buffers` tests establish owner replacement, not this fixed-range
  contract. Migrate its line-input uses without extending ordinary slices to
  spare capacity or retaining hidden length writeback. Preserve independently
  motivated whole-field operations and their own regression coverage.

  Resume bundled API/provider integration from the authored line loop in
  `tests/native-differential/tests/terminal_byte_views/read_line.omg` and its
  `byte_input/line_read.rs` regression. On the checkpoint based on `03bd1317e5`,
  macOS ARM64 (Cargo fallback), `cargo nextest run -p omega-native-differential-test
  --test terminal_byte_views --no-fail-fast --no-tests fail -E 'test(bounded_line_reader)'`
  passes: byte input, bounded stores, and full-width line outcomes compose through
  source checking and installation replay. Publication covers Linux x64/ARM64
  and macOS ARM64; runtime was exercised only on macOS. The loop preserves raw
  prefixes, LF/EOF/Full distinctions, zero-extent non-consumption, and unread
  suffixes across repeated calls. Installation's byte-result home is independent
  of the enclosing function's aggregate return frame; retain its exact frame and
  replay controls in `image-emission/src/installation.rs`.
  Ordinary scalar-sum caller transport remains covered by
  `tests/native-differential/tests/scalar_case_results.rs`.
  Windows byte input and indirect aggregate results remain realization dependencies.
  Concrete `ConsoleNativeProvider::read_byte` composition now has its own resume
  control: on the checkpoint based on `a6174dd31d`, macOS ARM64,
  `cargo nextest run -p omega-native-differential-test --test terminal_byte_views
  --no-fail-fast --no-tests fail -E 'test(concrete_byte_leaf_line_reader)'`
  passes source custody, three-target publication/replay, and matching macOS
  line-loop execution. Psi's `validation/src/intrinsic_boundaries.rs` rejoins
  the exact requirement/result symbol; the checked interpreter consumes each
  byte once, including case-subject payload extraction, without a global type-name
  lookup. Preserve its hostile schema/identity and host-effect controls in
  `checked-interpreter/src/evaluator/host_dispatch/`.
  Resume from `source/library/std/console.omg` and
  `compiler/tests/canary_suite/providers_float_and_console/console_reader.rs`.
  The selected reader executes as ordinary checked source over its exact byte
  leaf; independent legacy local boundaries still use the old interpreter fallback
  and are not an oracle for this API. Native `cli_mvp` acceptance below remains open.
  Preserve the checked API controls with `cargo nextest run -p compiler --test
  canary_suite --no-fail-fast --no-tests fail -E 'test(selected_console_line_reader)'`.
  Also reconcile the inherited hosted byte leaf's blocking/crash envelope with
  `wiki/spec/language/effects.md`: `exact_console_signature` in
  `build/selected-dispatch/src/compiler_intrinsic.rs` currently rejects `blocks`,
  although hosted input may wait and trap. Carry honest declarations, call-site
  acknowledgements, selected identity and native admission together; do not claim
  bounded wait or full operational-contract closure from interpreter success.
  The public result schema alone supplies no callable count-to-extent theorem;
  callers retain explicit prefix guards until that relational evidence is carried.
  Preserve the retained declaration ranges, selected-case scalar
  facts, and provider-return validation described in the
  [Terminal producer](omega-rust/psi/compiler/terminal-production/README.md).
  Restricted record construction and mutation require written-value obligations;
  opaque host values and successor annotations cannot establish those bounds.

  Recheck this floor with `cargo nextest run -p omega-native-differential-test
  --test terminal_byte_views --no-fail-fast --no-tests fail -E 'test(byte_input)'`
  (use `mbx` when available). macOS ARM64 execution covers exact copying of every
  octet, EOF, empty and repeated views; the classifier additionally checks failed
  reads and no overread. Linux x64/ARM64 coverage here is publication/replay only.
  General signed state-graph casts still need missing sign/16-bit normalization
  before unsupported carrier combinations can be admitted; this does not block
  the reader's proven `i32` to `u8` conversion.

  Acceptance: zero capacity returns `Full(0)` without reading; LF is stored and
  included in `LineComplete`, including at the last writable byte; EOF retains
  the partial count; filling without LF returns `Full` without one extra read.
  Test empty lines, initial EOF, exact fill followed by LF/EOF, repeated chunk
  reads, short OS reads, CRLF/NUL/non-ASCII preservation, untouched destination
  tails, alias/access rejection, and exact result identity. `Invalid` must not
  escape a completed call or fabricate EOF. Full is not a capacity trap or proof
  of an overlong line. Preserve the selected byte leaf's failed-read trap and
  required crash/progress contracts; do not invent recoverable errors or bounded
  wait from buffer size. Record native runtime passes separately on Windows,
  macOS, and Linux; unavailable hosts remain explicit validation gaps.

  Replace sample-local capacity-specific `Utf8` declarations and compiler-name
  `valid_utf8` dependence with imported checked library encoding vocabulary.
  Text consumers validate only the returned prefix; pause-only callers use raw
  fixed storage and explicitly discard the result. Migrate the byte-predicate recognizer in
  `psi/foundation/language-semantics/src/byte_predicates.rs` and its checked-tree
  readers under the [encoding contract](wiki/spec/language/domains.md#byte-containers-and-encoding-domains),
  rather than renaming a compiler primitive or duplicating it per capacity.
  The legacy checked interpreter's `byte as char` line construction is not UTF-8
  validation: preserve input bytes instead of re-encoding them, distinguish empty
  lines from EOF, and stop using its Boolean/whole-replacement result as an oracle.
  Acceptance includes valid multibyte sequences, invalid bytes, split codepoints
  at Full boundaries, and rejection of unproved output qualification. Native
  byte leaves and unrelated bounded-field operations remain independently actionable.
  Native byte input still needs matching Linux runtime evidence and a Windows
  realization. Retain the shared hosted read leaf and connected case-selection
  path as the regression floor:
  `cargo nextest run -p compiler --test canary_suite
  runtime_console_byte_inspection_replays_validated_cross_target_artifacts
  --no-fail-fast --no-tests fail` passes cross-emission and native artifact replay
  for Linux x64/ARM64 and macOS ARM64. The same command with
  `runtime_console_byte_read_return_catalog_replays_supported_hosted_targets`
  preserves uninspected result cleanup. For matching-host execution, use
  `hosted_read_inspection_executes_every_byte_eof_and_failed_read`: every byte
  echoes exactly with exit 70, EOF produces empty stdout with exit 70, and a
  failed read traps. This has run on macOS ARM64; Linux runtime remains unverified
  without a Linux host or emulator. Its selected-edge payload loads, exact
  destination definitions, and once-only cleanup/fuel must survive publication.
  Preserve branch-local normal-return cleanup with
  `hosted_read_returning_branches`: cross-target artifact replay and matching-host
  execution must retain distinct result homes, exactly two sequential reads,
  first-byte output, and zero-status Unit completion. The macOS completion
  adapter is a physical entry mapping, not closure of the canonical root contract.
  Preserve exact operation/result identity through the open native caller closure,
  frame home, layout, fuel, effects and cleanup through the existing selected
  instruction and `BoundaryStructuralResultRecord`; do not fabricate scalar
  results or replace the structural home with boundary scratch.
  Close this slice with migrated fixed-range round-trip and sequential-read
  native canaries, preserving bounds, exact bytes, prefix/count, access, and
  alias checks. The old hidden-replacement signature is not an acceptance target.

  Extend retained receiver forwarding to shared and indexed projections,
  owned/local receiver roots, composed control flow, and scalar-result receiver
  callees. Reconcile each exact receiver operand in
  `typed-trees-to-checked-trees/src/flow/terminal_unit/receiver_calls.rs`;
  preserve the source place, access, and ownership across each supported call
  shape and validate the resulting Terminal closure. Do not erase a required
  callee receiver to make argument counts agree. Remaining indexed write-only
  receiver work belongs to `WRITE-ONLY-BORROW` below.

  Acceptance: both tests pass, with every maintained sample reaching checked
  trees and every documented exit oracle observed on its matching host.

- **CANARY-CORPUS.** `mbx test -p compiler --test canary_suite` is red
  across most of its roster on a clean tree, while the `AGENTS.md` baseline
  gates are green — so the gate list does not measure this bar. The dominant
  cause is the same `checked-trees-to-lowered-psi` fence `SAMPLE-CORPUS` names
  above, `attached Unit closure is missing a checked transitive machine plan`,
  which owns the clear majority of all failing diagnostics; the rest fall behind
  `NOMINAL-FIELD-FLOW`, missing exact selected program entries, and
  index/subslice bound proofs. The checked-tree-valid
  `capabilities/win64_scalar_float_import_compile` fixture remains blocked by
  the Unit closure fence during native production. Owning areas
  are those entries, not this one. Blocker: clearing the fence advances each
  affected canary to its next failure rather than passing it outright, so the
  distribution must be re-ranked after it closes rather than assumed. Rank with
  the pipeline in the `advance` skill and attribute with the filter variables in
  `AGENTS.md`; a full run costs several minutes and cannot separate a session's
  own breakage from the standing state. Acceptance: every remaining red canary
  is attributed to a named entry on a board, and this entry is replaced by those.

- **TERMINATION-RANKING-CHECKS.** Complete the documented flow-dependent
  rank-range checks in
  `typed-trees-to-checked-trees/src/checks/termination/ranking/` and
  `validation/src/call_cycles/runtime_ranking/`.
  Computed-only paths with auxiliary-only multi-parameter arithmetic,
  transfers with diverging copies of rank inputs, and call components with
  internal state arrivals, or slice-length,
  bounded-distance, or custom views need
  exact arrival mappings and preserved premises for ranked subjects and pinned
  endpoints. Mutable premises need live write-frame evidence.
  Custom struct-view ranges involving borrowed or nested projections,
  constrained measure parameters, and authored named-state arrivals need exact
  view-application evidence. Retire generated operand-call states through
  STATE-LOCAL-VALUE-FRONTIER's checked computation route rather than add ranking
  provenance for those artificial edges. Flow-dependent computed
  endpoint formation needs its own arithmetic proof, not an unchecked
  polynomial. Non-polynomial endpoint substitutions beyond exact input
  forwarding need their own equality evidence.
  Scalar views beyond `u64` identity forwarding, and slice lengths
  over projected storage need their produced-rank facts.
  These are implementation gaps, not grounds to weaken the range obligation.

  Acceptance: named-state and call-component rank ranges accept proved
  constraints while changed endpoints and intervening writes invalidate their
  premises. Preserve the private-witness/public-guarantee split described in
  chapter 3 and the
  [termination contract](wiki/spec/language/termination.md).

`omega-rust/` remains the production implementation until that contract
closes. It may remain afterward as a differential implementation while it finds
real bugs, but Rust agreement is not bootstrap authority and Rust-specific
machinery must not migrate into the Omega-written compiler source.

## P1 - Authority, roots, and entry

Owners include
`wiki/spec/resources/authority.md` and
`wiki/spec/resources/storage.md`.

- **ENTRY-CONTENT-ROOTS.** Connect the generated target entry stub to the exact
  selected semantic continuation, consume the activation loan, and retain
  generated-bridge evidence without inventing roots. Migrate deployable
  fixtures to authored target-owned `ProgramEntry`; targetless checks select
  none. Acceptance is native execution from an authored entry with exact
  symbol/text/continuation replay and mutation failures for redirected or
  duplicated identities.

  Resume with exact hosted contract custody and admitted runtime storage, not
  array construction or an unchecked receiver allocation. The macOS authored
  contract is `source/library/std/targets/macos_arm64/entry.omg`: its distinct
  physical arrival and internal `ProgramStorageEntry` applications retain the
  actual record shapes and checked AAPCS64 plans. The focused regression is
  `cargo nextest run -p compiler --test calling_policy_plans macos_entry
  --no-fail-fast`; this establishes calling applications, not installed roots.
  Hosted `target::TargetProfile::program_entry_slot` still leaves physical
  requirement, contract package, boundary schema and calling conventions absent.
  Ordinary `cli_mvp` imports reach the target's provider modules, not the target
  definition that imports `entry.omg`. Route the selected contract through exact
  package source assembly; a slot binding alone does not load authored source.
  Join the macOS source/package custody and exact applications through that slot
  without weakening its independent acceptance checks or using report hashes as
  identity. The existing
  `program-entry-plan` optimized semantic entry/wrapper accepts receiver-free
  UEFI/Microsoft inputs with visible image/initial-storage parameters; it is not
  a hosted bridge. Close the physical adapter and independently backed stack
  plan through the actual `ProgramStorageEntry` root crossing,
  while retaining zero source-visible arrival parameters. Runtime geometry,
  rights and backing must come from that target's admitted arrival, joined to
  the installed artifact occurrence and lifecycle epoch. Reuse `external-roots`
  producer-schema, extent and epoch accounting. A plan-only helper or emitted
  writable section does not establish that crossing. This is implementation
  work under the settled [entry contract](wiki/spec/build/entry_roots.md), not
  an owner-decision blocker.

  Then exercise a plain ZII-valid receiver without provider fields, using its
  captured source signature and verified structural layout. Derive its range
  beneath admitted writable storage, keep receiver/stack/residual partitions
  disjoint, and retain exact continuation, activation loan and normal cleanup.
  Reject wrong receiver/continuation identity, non-ZII state, insufficient or
  misaligned backing, overlapping partitions, substituted occurrence/epoch,
  and prepared-input bypass. Keep the explicit rejection in
  `native-realization/src/realization/native_artifact.rs` until executable
  provisioning actually exists; its tests distinguish source-entry settlement
  from runtime storage. Acceptance must run a published process without a
  test-supplied receiver pointer, not merely a C caller over initialized storage.

  The stored-descriptor fixture
  `traits/runtime_local_named_dyn_stored_exit` already retains a borrowed
  receiver beside provider requirements. Finish zero-payload provider-field
  layout in `abstract-operations-to-target-operations` together with actual
  receiver provisioning: direct image construction currently names the
  semantic machine as the process entry, and incoming-argument staging does
  not allocate or initialize its receiver. Reserve the checked receiver layout
  beneath admitted initial storage, zero it into a ZII value, and pass its
  single activation loan through the generated bridge. Cross-emission alone
  is not acceptance; the fixture must execute with exit 70 on both Linux
  targets without an externally supplied `self` pointer.

- **UEFI-PHYSICAL-SEMANTIC-ENTRY.** Finish the two-surface UEFI bridge: the
  target-package physical firmware entry remains distinct from the semantic
  program continuation. Emit and validate the adapter, exact calling plan,
  stack/custody transfer, and return behavior. Application lookalikes and
  cross-target substitutions must reject.

  Resume with checked source through Terminal, child emission, and the entrance
  in `native-realization/src/optimized_semantic_wrapper_object/mod.rs`.
  That wrapper-publication entrance currently has no callers;
  its object tests use constructed contracts, so further isolated binder work
  is paused pending this exercising path. Native declaration preflight is not
  executable bridge acceptance. In `program-entry-plan/src/optimized_semantic_entry/validation.rs`,
  `validate_method` still compares schema application identity with raw ABI-plan
  identity; retain exact source-application custody through the native binder
  instead of dropping this check or adding an upward backend dependency.
  The last full Terminal probe of `build/uefi_program_entry_storage_roots` stopped
  in lowering because `Boot::launch` lacked a checked transitive Unit machine plan.
  Its checked-source/preflight regression is not complete Terminal or firmware
  execution acceptance. Close these joins before claiming an executable bridge.

- **UEFI-OS-HANDOFF.** Implement the nonreturning custody transfer from Boot
  Services to the selected OS entry. The bounded memory-map/key retry loop must
  return all custody on stale-key failure and consume boot-scoped services only
  on success. Acceptance includes stale-key, exhaustion, lost-custody,
  post-exit provider-use, and successful handoff canaries.

- **AP-BRINGUP.** Complete one secondary-processor entry through the executable
  installation and external-root owners. Acceptance covers low-memory and
  alignment constraints, CPU-regime transitions, placed-byte visibility,
  installed AP entry, and separately accounted per-CPU stack/state. An emitted
  trampoline alone does not satisfy the entry and custody contract.

- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Carry one real
  content-bearing program through checked source, Terminal Psi, provider
  selection, and native realization. Introductions and exits must bind exact
  subject, geometry, lineage, route, and installed occurrence; reshuffles may
  preserve identity, while partitions require authored proof. Acceptance:
  every surviving content claim traces to a reconstructed introduction or
  admitted provider issuance and every residual is accounted for.

- **INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION.** Derive enumerable program-local
  content roots from exact installed parameter positions, capacity, and epoch.
  Ordinary results with no parent lineage cannot mint roots. Acceptance:
  aggregate capacity is reconstructed for one artifact instance and lifecycle
  epoch, with no ambient provision or row-equality authority.

- **BOUNDARY-ISSUANCE.** After conservation closes, derive provider issuance
  geometry from exact invocation parameters, entry places, and results. Keep
  ownership, aliasing, issuance, custody, and partition succession distinct;
  providers may attest custody but not computable interval arithmetic.

## P2 - Materialization and placed access

- **PLAN-LAID-VIEWS.** Finish checked and native placement for plan-laid views
  without turning a physical address into semantic ownership. Layout identity,
  backing, range, access, and lifetime must rejoin at every use. Acceptance:
  valid views survive codec/native replay and stale plan, range, access, or
  backing substitutions reject.

- **ACCESS-PLAN-AND-PLACED.** Finish the public `AccessPlan` / `Placed<P, T>`
  model as an explicit relation among semantic value, target layout, backing,
  and placement. Do not infer authorization from equal offsets or compiler
  custody. Acceptance: source can express and consume one useful placed value
  through target lowering while arbitrary construction and cross-plan reuse
  remain impossible.

- **SYMBOLIC-MATERIALIZATION.** Complete symbolic field/index materialization
  and its target-dependent realization. Preserve exact paths and bounds until
  assignment; physical lowering may choose locations but not change semantic
  access. Recursive build-time projection/replay is shared across the currently
  admitted exact record depths through 23; extend that recursive owner rather
  than adding another copied depth implementation. Nested sum arrays, direct-
  sum coexistence, recursive shapes, and target-dependent placement remain
  fenced until their general rules land. Acceptance includes nested field/index
  canaries on both Linux ISAs.

## P3 - Terminal Psi, PCC, and observation

- **PSIIR.** Extend Terminal Psi only in complete vertical slices through
  canonical encoding, independent reconstruction, verification,
  interpretation, resource analysis, native lowering, artifact custody, and
  installation. [Terminal specification subjects](wiki/README.md#current-specification-subjects)
  own the vocabulary; complete operation/proof-node byte tables remain required
  by the [encoding contract](wiki/spec/terminal-psi/encoding.md).
  Acceptance: source and producer state can be discarded before an
  independent verifier reconstructs every obligation and executes or lowers
  the same artifact.

  Native/external execution, ABI, fixed native resource, and final-code replay
  claims additionally require exact final-realization evidence. Preserve
  complete standalone products without hidden `CheckedCompilation` state;
  checked API/capability results and opaque executable supply cannot establish
  those claims. Physical optimization replay belongs to
  `TRANSLATION-VALIDATION` in `TASKS_OPTIMIZER.md`.

- **GENERAL-CYCLIC-EXECUTION.** Implement the
  [settled cyclic control contract](wiki/spec/terminal-psi/control_flow.md)
  and [separate safety/progress rules](wiki/spec/language/termination.md)
  for the actual effectful state graphs used by `print_squares` and the
  Console writer. Reuse blocks, `Jump`, `Conditional`, successor arguments,
  and ordinary operations. No new loop opcode, fabricated per-state machine,
  private countdown, or second interpreter is needed.

  Extend `terminal-verifier/src/validation/control_flow.rs` beyond the
  claim-free mutable-receiver, immutable-byte-view, and whole plain-owned input
  slices to qualified/partial owned custody, structural results, projected claims,
  and required effectful call families.
  Reuse full-graph dominance, exact successor transfers, and ownership-frontier
  replay. Current-iteration guards are reconstructed after resetting incoming
  facts at every proof-scheduling cut target; general cyclic proposition
  invariants and ranking views beyond fixed unsigned natural ranks still need
  retained evidence. Guarded-crash path
  reconstruction must also cover cycles without exhausting path enumeration.
  Keep cycle safety independent from optional termination certificates and
  finite fuel. Existing bounded admission cannot
  authorize general effectful cyclic callers or callees by relaxing its shape
  guard alone.

  Extend shared state-body construction and lowering across ordinary/composed Unit
  plans in `typed-trees-to-checked-trees/src/flow/terminal_unit/` and
  `checked-trees-to-lowered-psi/src/attached_unit/`; replace graph-shape routing
  as the shared path closes, rather than adding another recognized topology.
  Generalize beyond direct scalar receiver fields and provider-field calls to
  ordinary projected helpers, the Console's structural operands,
  indexed/aggregate writes, and computed results in the unchanged customer.
  Preserve observable order, caller-visible writes, test-fuel suspension/resumption,
  and rejection of stale successor bindings, inconsistent ownership, and
  missing/reordered effects. Current source boundaries and the executable
  regression are documented in
  [Terminal production](omega-rust/psi/compiler/terminal-production/README.md#multi-state-control).

  Native completion must replace the straight-line/adjacent-fallthrough limits
  in `abstract-operations-to-target-operations/src/lowering/unit/`, preserve
  real edges through subsequent lowering and replay, and compose caller/callee
  resource evidence without borrowing the exact countdown's theorem from
  `terminal-psi-to-abstract-operations/src/artifact/ranked_native.rs`.
  Full acceptance is the unchanged `print_squares` native exit/output oracle
  on the hosted matrix. Its field/byte operations and the Console writer's
  borrowed-view native operations and retained ranking through native replay remain required
  dependencies under `SAMPLE-CORPUS`; interpreted loop support alone does not
  complete either customer.

- **CRASH-CONTRACT.** Complete invocation-specific crash obligations through
  nested structural paths, calls, cycles, and imported effects. Crash is an
  explicit observable outcome with a semantic cause; it is never represented
  as an ordinary return, missing cleanup, or backend trap inferred after the
  fact. Acceptance: safe calls discharge every route and mutations to guards,
  substitutions, or sites reject.

- **PROOF-CONTRACT-MIGRATION.** Migrate the proof surface to
  [ordinary machine contracts and trait bundles](wiki/spec/proofs/contracts.md#machines-and-bundles).
  Owners: Psi syntax/resolution/typing, contract proof semantics, Terminal
  evidence/codec/replay, and core mathematical traits. First specify the missing
  logical binders, arbitrary mathematical function/predicate parameters, and
  proof-only noncomputable values with worked proofs; do not substitute
  executable declaration enumeration or an optional-returning decider. Audit
  universe/equality commitments and selectable-axiom provenance explicitly;
  multiple-foundation compatibility remains a design dependency, not an assumed
  property of the current checker.

  Replace the dedicated formula-declaration and hidden-witness call machinery
  with ordinary contracts and named witness/law bundles, preserving exact
  substitution, result/path availability, erasure, validity, and transitive
  assumptions across trait calls and artifacts. This incorporates the former
  selected-witness and trait-named-witness work; do not widen those old surfaces
  independently. Retain useful checking rules, not mandatory wrapper syntax.

  Acceptance: actual proof scripts and false twins pass through source,
  Terminal serialization, and independent replay for all five cases:

  1. Composition of two witness/law bundles preserves exact substitutions and
     distinct witnesses.
  2. A higher-order theorem quantifies over arbitrary mathematical predicates or
     functions, not an enumeration of executable declarations.
  3. Nonconstructive existence uses an explicit axiom and cannot supply an
     executable witness without constructive implementation.
  4. Accepting and denying policies distinguish the same theorem, with exact
     transitive assumptions surviving import, erasure, serialization, and replay.
  5. A Cauchy/quotient proof preserves representative independence and explicit
     law selection.

  Migrate core relations, quotients, samples, and tests, remove obsolete parser/carrier/codec
  routes, and reject retired spellings. Candidate naming syntax is not a
  prerequisite. Do not claim full mathematical coverage from these controls.

- **PROOF-CERTIFICATION-BRIDGE.** Emit kernel-checkable certificates from
  source automation. Recursive certificates own one SCC and cite ranking and
  well-foundedness evidence once; normalization names exact laws and preserves
  transitive trust. Acceptance: changing an edge decrease, premise, law, or
  component identity rejects or changes the trust closure. For separately
  compiled dependencies, reconstruct the exact obligations and recheck retained
  certificates locally; propagate unresolved assumptions with their original
  owner. Missing or stale evidence cannot silently discharge an obligation or
  inherit a producer's admission decision.

- **SUBJECT-QUALIFIED-ARTIFACT-PROOFS.** Bind every proof to an exact semantic
  subject and observation profile through ledgers, artifact seals, deployment,
  replay, and reports. Producers may not choose the verifier's root subject.
  Acceptance: a proof or commitment valid for one source/model/profile cannot
  be replayed in another role even when compact coordinates coincide.

- **PCC-CANONICAL-SEMANTIC-LEDGER.** Replace trusted Rust fusion of artifact
  traversal and proof search with a small total canonical-ledger generator plus
  an untrusted certificate producer. The verifier reconstructs goals and only
  checks the supplied route, under the
  [verification contract](wiki/spec/terminal-psi/verification.md).
  Bootstrap discharge remains open under
  `BETA-DERIVATION-CHECKER` in `TASKS_BOOTSTRAP.md`; no current artifact may
  claim rooted-checker acceptance.

- **IRFUEL.** Keep fuel as analysis/evaluator evidence, never inserted runtime
  semantics. Extend installed-code correspondence from the bounded ranked
  countdown to ordinary admitted loops. Failure to derive a bound reports
  `Unknown` or `NoFiniteGuarantee`; it does not alter execution.

- **PROOF-RELEVANCE-MIGRATION.** Finish `[erased]` noninterference and
  erased-stripped layout across remaining carriers. Erased terms remain in
  semantic/proof identity but contribute no runtime storage, tags, ABI
  transfer, or execution. Runtime use and any layout-dependent erasure reject.

- **EFFECTFUL-TYPED-COMPUTATION.** Specify the value/computation judgments that
  connect effectful machines to the future typed proof calculus. This is
  semantic design work, not a prerequisite for extending unrelated Terminal
  operations.

## P4 - ABI, borrowing, and callbacks

- **NORMALIZED-ABI-LOWERING.** Finish target-independent signature
  normalization and target-owned calling/layout realization for aggregates,
  dynamic values, callbacks, and foreign boundaries. Acceptance: the ABI is
  independently reconstructible and no target placement leaks back into
  Terminal Psi.

- **OPAQUE-BY-VALUE-BOUNDARY-ABI.** Complete [representation agreement](wiki/spec/build/opaque_representations.md) at
  independently compiled by-value exchanges. Rejoin consumer demands to exact
  producer opaque/conformance/carrier declarations and immutable source;
  enforce strong selected-application equality at actual exchanges. Finish
  physical movement and lifecycle planning, including transitive
  inert-carrier proof and multiplicity checks. Equal size/alignment or compact
  fingerprints cannot establish agreement.

  Carry the application through native artifacts, replacement compatibility,
  stable-handle eras, and independently replaceable provider contracts.
  Acceptance: independently compiled producer/consumer and historical-selection
  canaries cover sealed `Ptr<T>` target semantics, proof-only `Real`,
  `EfiSystemTable`, provider/replay drift, cleanup, and multiplicity; incompatible
  by-value exchanges and replacements reject before execution.

- **WRITE-ONLY-BORROW.** Finish `&write T` through projected aggregates,
  calls, returns, dynamic dispatch, cleanup, and native lowering. It permits
  replacement without observation and must remain distinct from shared and
  mutable borrow. Acceptance includes read rejection, exact write coverage,
  unwind/return behavior, and both Linux targets.

  Run indexed write-only receiver caller observations on both Linux hosts and
  extend ordinary borrowed publication to the remaining control/lifecycle forms.
  Keep original referent identity and exclusive access through incoming homes,
  projected calls, callee frames and caller continuation. The owning path is
  `target-operations-to-selected-instructions/src/legalization/scalar_graph_input/`
  and ordinary selection construction/replay, not the retired emitter.
  The source-backed probe is `cargo nextest run -p omega-native-differential-test
  --test terminal_psi_indexed_receivers --no-fail-fast --no-tests fail`.
  Its register/stack-pointer caller observations execute on macOS ARM64,
  including runtime scalar values and a retained root across three calls;
  four-target cross-emission is not Linux runtime coverage. The publication
  probe `--test terminal_psi_indexed_receivers -E 'test(publication::)'` additionally
  checks ordinary object/image/installation records, mixed scalar/borrowed Unit
  calls, exact installed-image custody and spill-inclusive stack demand. Its
  implementation owners are `image-emission/src/function_fragments/structural.rs`
  and `installation/borrowed_structural.rs`; keep copied referents distinct from
  pointer identity while extending their bounded Unit admission. Extend native
  IEEE stores to computed floating sources without erasing format or
  selected-operation evidence. The same source-backed probe's `primitive_stores::`
  group checks whole primitive replacement,
  signed runtime inputs, both Boolean values, literal stores with unused inputs,
  and stack-passed primitive roots across three calls through publication.
  Preserve exact write widths, untouched bytes, and independent receiving
  replay; frame-slot stores or copied referents are not writeback.
  Extend Terminal receiver production beyond nonescaping static projected
  mutable/write-only alias chains ending at state exit to escaping carriers,
  early nested closure, restored-parent uses, and dynamic indexes; checked
  admission alone does not supply their portable address and lifetime evidence.
  Extend non-observing receiver admission to
  reference-bearing projections only where locating the receiver does not read
  a stored pointer or descriptor. Keep generic, sum, and dynamic dispatch tied
  to their corresponding shape/admission work; do not treat a receiver as
  readable merely to dispatch it.

  Native referent identity follows `STRUCTURAL-BORROW-IDENTITY` below; it is
  not an owner-policy blocker. Preserve write-only non-observation independently
  of the shared physical reference ABI.

- **STRUCTURAL-BORROW-IDENTITY.** Enforce the settled
  [structural borrow identity contract](wiki/spec/terminal-psi/structural_access.md)
  through call argument preparation and native validation/replay. Extend the
  [target signature checks](omega-rust/omega/pipeline/abstract-operations-to-target-operations/README.md)
  to embedded callee plans, projected argument/home identity, and standalone
  receiving entrances. All borrowed access modes retain `BorrowedReference`;
  shared physical shape cannot authorize access substitution. Acceptance: caller-
  visible writes, forwarded references, legal synchronized shared observations,
  write-only non-reading, and register/stack pointer passing work on both Linux
  targets. Independently formed or substituted access/shape/placement pairs
  reject; direct-home controls must use owned semantics or test rejection of
  borrowed copies. Do not claim copy equivalence merely because a
  following callee sees the staged write.

  Compose Boolean local borrows with scalar-returning control. A scalar-result
  `observe(initial: bool, replacement: bool) -> u64` that initializes `scratch`,
  invokes `replace(&mut scratch, replacement)`, then uses
  `transition scratch { true -> 1 false -> 0 }` needs ordinary Unit-call source
  production and Boolean-local branch legalization. A Unit helper currently
  rejects at checked-to-lowered scalar-plan admission; a scalar-returning helper
  gets through Terminal but still rejects in target-to-selected legalization.
  Acceptance: both helper signatures preserve the same authored call, fresh
  Boolean read and chosen result through canonical replay, four-target publication
  and matching-host execution. Extend the
  [primitive-local regressions](tests/native-differential/tests/primitive_locals.rs);
  do not substitute one helper signature for the other.

- **BORROW-PROOF-CONVERGENCE.** Make ordinary borrow checking proof-producing
  under the [loan contract](wiki/spec/terminal-psi/loans.md), without allowing
  proofs to create or amplify authority. Extend symbolic
  range ordering and containment beyond exact shared immutable boundaries, then
  admit explicit compatibility theorems over
  already-existing places and occurrences. Acceptance: proof evidence can
  establish disjointness/containment but cannot extend lifetime, duplicate a
  loan, or replace ownership accounting.

  Extend range-premise read dependencies to selected calls/indexing operators
  and atomic reads when their complete footprints and operation stability are
  established. Explicit arguments alone do not establish all callee reads;
  preserved numeric captures must remain independent of subsequent source writes.

- **CALLBACK-PARAMETER-REQUIREMENT.** Implement the nominal
  `where machine Selected satisfies Trait::requirement` binder and retain its
  exact requirement, conformance, envelope refinement, call site, and target
  entry recipe. Structural coincidence and overloaded/implicit selection
  reject.

- **CALLBACK-PRIVATE-MATERIALIZATION.** Add target-owned private callback slots
  selected through exact conformances and validated layout paths under the
  [private-callback contract](wiki/spec/build/private_callbacks.md). Authenticate
  the complete plan application and replay the authored-use-to-Terminal-operation
  join independently; retained producer digests alone are insufficient. Private
  slots must be absent from source-visible schema and inaccessible as ordinary
  fields or addresses. Acceptance: one outbound registrar closes without a raw
  code pointer or duplicated placement authority.

- **REGISTERED-CALLBACK-LIFETIME.** Model successful registration as a linear
  external root and unregister as the operation that ends it before releasing
  code/component leases. Capacity bounds live registrations, not emitted
  thunks. Acceptance covers rejection, retry, replacement, cleanup, and an
  actual Windows callback after the generic path closes.

- **FOREIGN-RETAINED-ARGUMENT-BACKING.** Generalize retained outbound arguments
  beyond callbacks with explicit call-scoped, lifetime-borrowed, moved, and
  snapshot dispositions. Every retained pointer needs exact stable backing,
  range, access, lifetime, and revision provenance; unknown or mutable ambient
  backing rejects.

## P5 - Cathedral over general Omega primitives

- **BUMP-ALLOCATOR-CANARY.** Build a package-level allocator over one qualified
  `Extent`, supporting two coexisting allocations, exact cleanup/recomposition,
  and reset only after full return under the
  [allocation contract](wiki/spec/resources/allocation.md). Use it to discover the real `Vec<T>`
  contract; do not add allocator semantics to the compiler.

- **ADDRESS-TRANSLATION-CANARY.** Continue Cathedral's page-table hierarchy,
  backing, policy, installation, and teardown in Omega source. Existing numeric
  page-walk validation grants no mapping authority. Acceptance: QEMU installs
  and tears down Cathedral-owned mappings with explicit Extent and TLB custody.

- **EXCEPTION-ROOTS-AND-TIMER.** Materialize all fatal exception entries,
  dedicated critical stacks, IDT installation, and a minimal timer root whose
  hard handler only acknowledges, records, and wakes ordinary work. Acceptance:
  QEMU reports timer ticks over owned output and halts between ticks.

- **BOUNDED-INSTALLATION-REACH-ROWS.** Finish unresolved-requirement fences for
  component contracts and the final carrier-owned invocation route. Concrete
  reach and conservative bounds remain separate; selected provider execution
  and token era, not row equality, authorize invocation.

## Parallel language and compiler lanes

- **MATCH-SELECTIVE-LOWERING.** Complete the general
  [value-dispatch contract](wiki/spec/language/patterns.md) on the retained
  source and scalar computation route. Remaining work: ownership-bearing result
  and conditional-transfer joins, nonnumeric Terminal results, structural/case/
  domain patterns and their coverage, anonymous-only numeric subject execution,
  semantic-domain and selected-operator result-type retention (shared with
  `STATE-LOCAL-VALUE-FRONTIER` numeric landing), and canonical
  package-review contract/index projection where dispatch is currently rejected.
  Current ownership/pattern fences in
  `validation/src/expression_types/match_dispatch.rs`
  are implementation limits, not narrower language semantics. Do not flatten
  conditional transfers into a whole-statement move/discard roster.

  Resume with `mbx nextest run -p checked-trees-to-lowered-psi --test value_dispatch
  --no-fail-fast` and the checker/interpreter `value_dispatch` regressions. These
  cover scalar first-match selection, exact subject-once execution, branch-local
  call premises, mutation invalidation, ordinary expression composition and
  independent Terminal replay. Extend the same result continuation and saved
  subject, not arithmetic expansion or manufactured source states. Acceptance
  still includes compatible owned/nonnumeric results, effectful subjects,
  unselected trapping arms, overlapping patterns and complete coverage through
  checking, artifact replay and target execution.

  Declared-result numeric probe: `mbx run -p omega -- --check --target macos_arm64
  tests/omega/pass/expressions/match_anonymous_result_landing/main.omg`.
  Operand/cast probe: `mbx run -p omega -- --check --target macos_arm64
  tests/omega/pass/expressions/numeric_operand_destinations/main.omg`.
  The shared query in `validation/src/expression_types/result_type.rs` still
  needs semantic-domain results, policy casts with explicit result predicates,
  and instantiated selected-operator results. Builtin arithmetic results use
  producer-retained carrier/policy references; input range predicates are not
  result facts. Unknown lookup must not stand in for anonymous numeric meaning.

- **MODULE-NAMESPACE-RESOLUTION.** Finish the
  [module/name contract](wiki/spec/language/modules.md) for pre-resolution
  computed Boolean and aggregate machine indices, aggregate/type-scoped constants,
  templates, trait defaults,
  operator homes, qualified constructors, and remaining declaration forms.
  Later syntax extensions also need retained base constant initializers; they
  currently retain only declaration identity. The explicit temporary fences live in
  `syntax-trees-to-symbol-resolved-trees/src/module_normalization.rs`; replace
  them with exact namespace-aware normalization, not bare-name fallback.
  Source-prefix imports, nominal/free-machine namespaces and scalar body
  constants have focused coverage in `tests/omega/pass/modules/qualified_declarations`,
  `tests/omega/pass/modules/qualified_constants` and the owning
  [source pipeline probes](omega-rust/psi/pipeline/README.md#resolution-and-closed-instance-normalization).
  Acceptance: remaining forms preserve exact module/package identity through
  canonical artifacts; same-leaf ambiguity rejects; private and transitive-only
  selection cannot gain authority through qualification. Extend package-alias
  lookup to other already-loaded sources of the same exact dependency while
  retaining exact-source validation of each import.

  Foreign-domain customers additionally need the settled
  [file-local import exposure](wiki/spec/language/modules.md#import-scope-and-exposure)
  and [exact attached paths](wiki/spec/language/modules.md#foreign-attached-declaration-paths).
  Acceptance: broad imports expose only directly declared public domains; narrow
  imports select one exact declaration; machine/carrier loading, sibling files,
  descendants, and transitive imports do not activate extensions. Preserve the
  declaring domain owner and resolved carrier independently; repeated exposure
  of one identity is valid, competing carrier-qualified names reject with both
  owners/imports, and carried qualifications do not grant source selection.

  Resume evidence: the working checkpoint based on `408bff9fa4`, macOS arm64
  with Cargo and `RUST_MIN_STACK=33554432`, checks
  `cargo run -p omega -- --check tests/omega/pass/modules/module_array_constant_indices/main.omg`:
  root/module arrays and scalar `settings::Sizes::MAX` retain distinct canonical
  values and exact declaration/carrier custody. Array body references also copy
  values under exact selection, retaining declared dimensions and element carriers
  at destinations, including empty arrays. Scoped constants require a
  nongeneric carrier in their declaring module; missing carriers and public exposure of a
  private carrier reject even on unused declarations. Continue from this customer
  with nominal record/sum initializers, public floating constant identities,
  foreign/generic attachments or nominal aggregate body substitution in `syntax-trees-to-symbol-resolved-trees`.
  Those next probes remain unrun at this checkpoint. Preserve the exact carrier
  selection in `constant.rs` and local/narrow-import lookup in `symbols/src/table/modules.rs`.
  Literal integer/Boolean arrays already use the canonical structural index path;
  module admission validates unused array and scoped scalar declarations too.
  Root/module machine scopes
  retain original lexical selection through `build-time-evaluation/src/const_generic_expressions.rs`.
  General array-value projection remains unfinished. Static integer/Boolean
  constant projections now retain declared element types and builtin selection
  through checked evaluation and independently decoded Terminal execution; the
  CLI customer includes a qualified scalar read. Continue with dynamic selectors,
  nonliteral value collections, borrowed projections and slicing while preserving evaluation order,
  bounds and view lifetimes. The remaining fence is covered by
  `dynamic_array_constant_projection_retains_the_value_indexing_boundary`;
  `values/scalar/constant_array_projection.rs` only selects closed literal leaves.
  General value projection needs its complete executable representation, not
  a source rewrite that makes a constant addressable storage.
  Array transport resume evidence (macOS AArch64, base `5d5333554e` plus the
  computation-argument change, Cargo with `RUST_MIN_STACK=33554432`):
  `cargo run -p omega -- inspect-terminal --machine computation_row tests/omega/pass/modules/module_array_constant_indices/main.omg`
  publishes verified Terminal Psi for the unchanged nested array customer.
  The [array production map](omega-rust/psi/compiler/terminal-production/README.md)
  retains the executable probes and source owners. Continue from its shared
  evaluation sequence, preserving authored order, selective construction, exact
  source custody, and actual payloads. Array-producing cycles and structural
  block-parameter payload transport remain explicit fences.
  Transitive scalar callees with ordered structural operation bodies also need
  the scalar-callee catalog join. Ordered scalar completion contracts and result
  refinements need their complete predicate/evidence path; preserve existing
  scalar-only contract lowering while extending that route.
  Borrowed/projected payloads, state transfers, and boundary-provider array
  results also remain unsupported. Native lowering rejects `EstablishScalarArray`,
  including empty payloads.
  Continue with complete value/storage paths and
  independent custody checks; do not
  substitute opaque structural identities for executable values.
  General slice-backed `.len` operands also need retained view formation and bounds
  obligations before folding; a known endpoint difference alone cannot erase that
  operation. The array operand correspondence owner rejects missing view evidence.
  Keep the `runtime_aggregate_index` and `runtime_fixed_array_index` rejection
  controls under `tests/omega/fail/modules/` while extending materialization.
  Conformance and static-requirement argument positions
  also need their complete owners, not a standalone root probe.
  Open-template computation, constrained destinations, authored operator execution
  and module-owned domain families also need their complete selection/evaluation
  contexts. Preserve per-node integer carriers, canonical result/selection
  separation, per-use exposure under specialization, and package authority before evaluation.

- **RUNTIME-VALUE-GENERICS.** Implement the settled
  [runtime-capable versus const binder contract](wiki/spec/language/generics.md#value-binders-and-const-requirements)
  for APIs whose result or stored qualification depends on an input value.
  Begin with scalar value binders and fixed-representation data/machine uses,
  not dynamic stack layouts or automatic SIMD specialization. Psi syntax,
  resolution, generic substitution, checked facts and Terminal production own
  the new path; calling/layout and artifact consumers retain the same exact
  runtime subjects alongside static applications. Keep the existing constant
  evaluator's runtime-input rejection for `const` binders. Module-owned forms
  depend on exact lexical selection from MODULE-NAMESPACE-RESOLUTION, not a
  source-spelling fallback or treating a runtime value as a static cache key.

  Acceptance: `<Count: u32>` accepts static and runtime arguments when the
  caller establishes its obligations; `<const Count: u32>` still requires a
  static specialization. One dynamic machine body handles distinct runtime
  counts without per-value code generation. Parameters/results and an indexed
  scalar field preserve the same captured subject, including after reassignment
  of its source variable. Equality-guarded uses retain their proof, and stale
  relationships, invalid bounds, duplicate/lost linear custody, and unsupported
  static-only uses reject. Reuse witnesses where valid; do not add heap boxing,
  reserve a range's maximum on the stack, or interpret integer finiteness as a
  specialization request. Check source diagnostics, representation, and
  interpreter/native replay for the supported slice before widening it.

  Squalr's 16/32/64-byte comparers and const-rotation bridges additionally motivate
  FINITE-GENERIC-DISPATCH below. The initial runtime-binder slice does not depend
  on completing that dynamic-interface work or general reflection.

- **STRUCTURAL-GENERIC-MATCHING.** Implement
  [static type equality](wiki/spec/language/generics.md#static-type-equality),
  [structural equations](wiki/spec/language/generics.md#structural-type-equations-and-inference),
  and [canonical ranges](wiki/spec/language/generics.md#canonical-integer-range-matching)
  for bounded containers deriving static backing from a declared length type.
  Psi parser/type-role resolution, generic-data substitution, machine inference,
  canonical type identity, and checked branch facts own the route; static
  evaluation/layout and artifact readers must use the same normalizer. Existing
  const-range substitution is not reverse endpoint extraction. Replace the
  range-argument exclusion in `generic_data/arguments.rs` only with exact identity
  and constrained-shell substitution, not a source-display cache key.

  Acceptance: TinyBytes' `Length == u64[0..=Capacity]` binds omitted Capacity from
  its supplied type before layout; inclusive/exclusive equivalent intervals
  select identical static capacity without runtime arithmetic overflow. Primitive
  equality and its static branches check all admitted alternatives. Repeat and
  explicit binder conflicts, absent/ambiguous endpoints, occurs cycles, and
  type/value-kind mismatch reject. Range-only call inference selects declared
  endpoints before ordinary compatibility; explicit larger call bounds remain
  distinct from exact type equations. Local flow narrowing cannot alter inferred
  layout; arbitrary domain predicates do not collapse nominal identity. Preserve
  const staging, initialization, stack supply, and artifact replay. Runtime endpoint
  applications depend on RUNTIME-VALUE-GENERICS; static matching can proceed first.

- **FINITE-GENERIC-DISPATCH.** Implement the
  [finite specialization contract](wiki/spec/language/generics.md#finite-specialization-boundary)
  and [dynamic method families](wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
  for Squalr-style scanner widths and runtime-selected datatype providers. Psi
  extracts exact tuple rosters from explicit OR/equality constraints, checks each
  body and membership edge, and retains requirement/index/result correspondence.
  Terminal dynamic-call/descriptor owners and Omega native table/replay consumers
  retain the same family rows and ordinary costs. Begin with one scalar value
  binder, a common concrete result, and dispatch around a region-sized operation;
  runtime arguments depend on RUNTIME-VALUE-GENERICS, not generic JIT execution.

  Acceptance: widths 16/32/64 need no handwritten suffix-method family; source
  alternative order and duplicates normalize deterministically. One selected
  conformance covers every required tuple; missing, wrong-width, mixed-provider,
  or shape-substituted rows reject. Const-only calls still reject dynamic inputs
  without a checked bridge. Short explicit tuple sets preserve correlations and
  do not enumerate arbitrary ranges; target-ineligible bodies and invented
  fallbacks reject. Preserve parameter effects, index identity, and once-only
  moves/cleanup through selection and replay. Escaping variable-shaped results
  require an explicit sum or eligible owned/borrowed descriptor, not implicit
  allocation. Retain compile/code-size evidence and scan-loop dispatch placement
  before claiming an improvement over explicit branches. No new reflection API
  or arbitrary generic virtual method is needed for this slice.

- **DOMAIN-ISSUER-ROUTES.** Implement the
  [requirement and exact-machine routes](wiki/spec/resources/authority.md#requirement-and-exact-machine-routes)
  and [private issuer catalogs](wiki/spec/resources/authority.md#private-issuer-routes)
  for public qualifications issued by owner-selected validators without an
  artificial trait. Psi route normalization, checked membership, and Terminal
  qualification evidence own target-kind/identity and exact result introduction;
  package source-selection capture owns the limited private-route metadata
  exception. Module-owned targets depend on MODULE-NAMESPACE-RESOLUTION; do not
  weaken its existing rejection fences to accept same-spelled identities.

  Acceptance: exact free/attached issuer calls establish only their authorized
  result provenance after carrier/predicate/custody checks; public ordinary
  requirements still permit valid downstream conformers. Public domains may name
  author-accessible private machines/requirements, and public wrappers forward
  issued values without becoming issuers. Reject ambiguous target kinds/overloads,
  self-justifying membership, forged result/route evidence, direct-call borrowing
  of admitted requirement authority, and outside private call/conformance access.
  Artifact/package replay retains exact private issuer identity and dependencies
  without exposing private types or hiding admissions. Resource capacity and
  classification-specific boundary-route restrictions remain unchanged.

- **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW.** Finish consumer permission rows and
  exact target-mechanism classification under the settled
  [filesystem control/lifecycle policy](wiki/spec/build/permissions.md#portable-filesystem-control-and-lifecycle-authority).
  Acceptance: remaining requirements have justified dispositions; every
  admitted leaf has one exact mechanism/contract row; unknowns and duplicates
  reject; exercised classes fit independently supplied service permissions.
  Explicit empties retain service reach and exact review identity. Retire the
  transitional broad `Filesystem` summary only after exact replacement closes.
  Generic close need not be supported to admit a separately proved constrained
  occurrence; do not fabricate a broad union to complete the table.

- **FILESYSTEM-RELEASE-CONTRACT.** Implement bounded occurrence-specific
  open/query/close evidence through checked flow and native realization replay.
  Prove the exact object/argument contract,
  handle/alias preservation through intervening calls, one applicable release,
  and no later use; authority classes alone are not preservation evidence.
  Acceptance: constrained ordinary close has one evidence-bound empty row;
  failed acquisition, escape, stale/substituted proof, invalidating calls,
  reused aliases, and attached deferred deletion prevent narrowing. External
  pending-deletion completion alone leaves ordinary close empty. Keep this
  bounded proof separate from general owned-handle design; no owner-policy
  blocker remains.

  Native acceptance also needs the checked transitive machine plan missing
  from `filesystem/windows_canonicalize_exit`: Terminal production currently
  refuses its attached Unit closure in
  `checked-trees-to-lowered-psi/src/attached_unit/call_closure.rs`. Resolve the
  general call-plan dependency before expecting this fixture to emit.

- **R5.** Finish exact inferred may-write summaries and relational candidates
  for unresolved receivers, boundary-result origins, conditional helper-body
  case refinement, mutable case-state transfer, graph-level aggregate result
  routes, type-generic carrier substitution,
  computed reference arguments outside proven helper-result relations, and
  other unsupported expression shapes.
  Prefer shared fixpoint and alias reasoning over syntax-shape exceptions.
  Acceptance: all supported finite source shapes converge without widening
  permissions, and unsupported recursion fails explicitly.

- **TPR6.** Finish subject-bearing progress-premise normalization through
  exported bodies, provider plans, recursive calls, and artifact evidence.
  Private ranking witnesses stay outside public identity. Acceptance: every
  used premise is reconstructed for the exact subject and no qualification or
  similarly shaped row mints one implicitly.

  Complete owned value loads through references, additional reference-boundary
  loads, indexed or replaced carriers, owned helper results, and
  reference-bearing helper results with unresolved control-flow or binding transfers in
  `checks/termination/progress/{origins.rs,lineage.rs}`. A mutated aggregate
  cannot use root correspondence as evidence for its previous field values;
  a may-write frame cannot identify a replacement value. Extend per-field
  arrivals through opaque reference, recursive-proof, and unresolved generic
  leaves when exact provenance is available. Acceptance: those finite
  projected arrivals and checked helper correspondences derive the replacement
  input's exact premise, while unknown writes and reference aliases without
  exact provenance retain no checked guarantee.

  Realize projected nested value-call operands guarded by
  `validation/src/calls/expression_scanning/result_realization.rs` through the
  checked/lowered value planning path. Borrow checking can transfer owned
  helper-result projections, but full checking still rejects the inner call's
  result as an unrealized operand.
  Complete result projections through the shared evaluator and result-binding
  lookup; extend the shared closure to general structural-result callees. Carry loans,
  qualifications, and projected claims through structural results without
  erasing their obligations.
  Acceptance:
  `select(forward_outer(outer).inner)` and `select(forward_array(values)[0])`
  evaluate each call once, retain the inner result home through projection and
  the outer call, and preserve every selected source loan and linear claim.
  Remove the nested-call gate only when those result uses have real producers;
  a correct declared type or source origin alone does not realize a value.

- **NOMINAL-FIELD-FLOW.** Complete declared-field domain evidence in Psi
  semantic facts, flow transfer, and contract consumption. Collection elements
  need explicit live coverage for their declared field predicates, transported
  through indexing, views, copies, and calls. Mutable calls must preserve or
  establish the appropriate returned field facts; an unchanged nominal type
  annotation cannot restore evidence retired by a write. Acceptance: the
  dungeon's `RoomLookup`, `MazeBuilder`, and game-state calls satisfy default
  field obligations, while corrupted elements and stale aliased fields reject
  at calls, transitions, and returns. Do not encode universal coverage as an
  unresolved index or assume arbitrary incoming storage is zero-initialized.
  Indexed text writers still need numeric conversion result evidence for
  unknown inputs, effectful nested arguments, nonlocal storage, and remaining
  cast policies beyond selected normal-return scalar snapshots consumed by
  `typed-trees-to-checked-trees/src/flow/transfers/byte_sequences.rs`.

- **CML4.** Complete `EdgeCleanupPlan` after outgoing materialization and
  transfer commitment, including structural sums, nested projections, cycles,
  calls, and partial initialization. Cleanup follows reverse establishment and
  exact residual custody; trap/abort edges clean nothing. Acceptance: no affine
  occurrence disappears, duplicates, or is cleaned after transfer.
  Implement native boundary call-result residual cleanup, including result homes
  and projected copies;
  whole-result disposal does not cover a projected result's residuals.
  Extend anonymous projected helper-result operands to multiple producers and
  other effects within one consumer's argument list and non-Unit consumers,
  preserving each temporary's exact dying continuation. Extend native
  Terminal Jump residual cleanup beyond acyclic Unit fallthrough with ordinary
  direct-register results and parameter roots: computed scalar bindings, boundary-result
  projections, and cyclic control need their own storage and edge replay without
  delaying cleanup until final return. Extend entry-origin scalar continuation
  storage to operation-result values and their exact defining identities;
  per-call argument shuffle snapshots do not preserve a result across earlier
  calls.
  Extend the
  type-directed record/array complement in
  [ownership contract](wiki/spec/terminal-psi/ownership.md) to construction-local
  roots and mixed dying-root schedules, preserving maximal untouched subtrees,
  empty complements, and reverse establishment order without runtime liveness
  flags. Entry-parameter cleanup alone cannot dispose a temporary's remainder.

- **STATE-LOCAL-VALUE-FRONTIER.** Complete live contract/value-fact transport
  across dynamic projections. Extend storage-value operands and executable
  paths to borrowed/projected places and the remaining scalar carriers.
  Finish executable lowering for state arguments whose evaluation invokes
  effects; materialize effectful returns and earlier call arguments at their exact
  evaluation points and activate staged loans at their evaluation points.
  Complete typed computation plans for remaining numeric policies and selected
  operator calls and borrowed/projected operands and writes.
  Extend computed scalar call operands to structural returned calls and
  structural arguments on composed internal
  calls. Retain exact evaluation order across guards and other argument effects,
  including projected and borrowed operand staging.
  Extend structural actuals of scalar-returning boundary callees to construction
  carriers beyond the existing empty-record prefix and single-i64-field local
  in ordinary Unit call closures.
  Extend result operands to mutable/write-only borrows, anonymous shared borrows
  with multiple arguments/producers per call and non-Unit or boundary consumers,
  projected routes, and self consumers without losing their exact result owner.
  Rejoin exact source/evaluation custody and remove only the cleanup transferred
  by the call; linear structural-result claims need their owning result plan.
  Complete mixed-signature runtime requirement transport for call-bearing
  arguments, remaining computation kinds, and mutable value snapshots.
  Retain evaluated arguments and prove
  computed-argument routes against caller formal ceilings; do not reread caller
  storage or retain callee-local IDs.
  Complete nonliteral contract arithmetic and callee-result bounds requiring
  caller-specific snapshots beyond direct scalar and owned-field comparisons, including
  contract-level borrowed collection lengths with exact entry observations and
  caller substitution, not body-local SSA values or unrelated length parameters; carry
  those facts into nested exact-cast obligations without rereading arguments.
  Transport dependent and public-trait call-result bounds into subslice proofs
  through their actual call-entry and public requirement identities, not caller
  fields or private realization types.
  Retire the remaining flat guarded-argument call hoisting once these paths use
  the same evaluation graph. Owning area: argument normalization and checked scalar
  computation lowering. Acceptance: selected arguments
  execute left-to-right once, skipped calls never execute, and dynamic RHS
  calls serialize, independently verify, and execute with their exact guards.
  Generalize mixed state signatures and borrowed loop formals through their exact
  ownership and arrival contracts rather than source-state duplication.
  Extend guarded scalar control to longer dispatches. Complete
  anonymous integer landing, width custody, and warnings for generic/evidence-adapted
  calls and boundary calls, and unsigned positions without an admitted consumer
  edge in `validation/src/literals/literal_widths.rs`, plus the remaining
  numeric operator/policy surface, so proof and execution
  consume the same values without rereading changed operands.
  Extend mutable owned parameter execution to the remaining scalar carriers and
  service-reaching Unit bodies, with their current storage represented through
  effects and state transfers. Acceptance: the delivered value is materialized
  once, reassignment changes subsequent reads, and a final guarantee about that
  mutable formal cannot prove equality with its earlier argument value.
  Complete entry-requirement crash implication beyond Boolean and fixed-integer
  predicates over direct scalar parameters and plain field paths: retain exact case-qualified,
  indexed, generic, and reference-valued intermediate structural identities,
  and carry arithmetic-expression and float entry evidence
  through their totality owners. Acceptance: those entry hypotheses cover an
  unconditional callee under the matching crash guard, and numeric coverage
  retains its checked totality evidence. The strict Boolean-formal entry reader
  and structural crash-predicate owner must retain their exact namespace and
  totality checks; current body observations are not entry hypotheses.
  None may change the callee's exact continuation routes.
  Retain exact entry-value origins for mutable scalar guard operands and
  unversioned structural observations on owned or mutable roots so unchanged
  entry values can establish published crash routes; a current storage predicate
  alone is not entry-snapshot evidence. Acceptance: an unchanged entry observation
  can prove its guarded route, while a later write or mutable call cannot prove
  that the new value existed at entry. Extend direct ranked crash-site proofs
  beyond entry requirements using independently checked all-path invariants;
  ignored-backedge first-pass facts must never authorize a loop crash guard.
  Complete [exact anonymous division and landing](wiki/language_guide/chapter_5_expressions_evaluation.md#exact-anonymous-division-and-landing)
  for the remaining parameter destinations,
  general aggregate production and proof consumers, numeric policies,
  remaining float destinations,
  remaining constant-argument destinations and policies, and their proof
  consumers. Preserve exact rational intermediates until an actual landing
  boundary. Carry fractional-intermediate warnings through ordinary suppression
  and compiler reports, retaining authored
  origins at successful integer landing. Acceptance:
  `7 / 2 * 2` lands as 7 with a warning, `7 / 2` cannot land in an integer,
  `7i32 / 2 * 2` is 6 without that warning, and mixed runtime/constant operands
  follow the guide's boundaries identically before and after optimization.
  Pin the practical alignment case: `(4097 / 4096) * 4096` is 4097 with a
  warning; `(4097u32 / 4096) * 4096` is 4096 without one.
  Implement [typed integer quotient and remainder](wiki/language_guide/chapter_5_expressions_evaluation.md#typed-integer-quotient-and-remainder)
  across operator resolution, constant evaluation, and proof consumption.
  Close authored const-operator selection before folding: an unrelated
  declaration must not suppress builtin `%` formation checks, and selected
  declarations must retain their own meaning during const normalization.
  The generic-data normalizer preserves possibly authored operator expressions;
  executing them still needs selected const evaluation. On macOS, the source
  probe `cargo run -p omega -- --check tests/omega/fail/generics/authored_const_operator_requires_selection/main.omg`
  now rejects with `requires an integer literal argument` instead of synthesizing
  `Buffer<1>` for a remainder provider returning zero. Owner:
  `syntax-trees-to-symbol-resolved-trees/src/generic_data/const_evaluation/`.
  The equivalent `Buffer<count()>` probe lives in
  `tests/omega/fail/generics/authored_const_call_operator_requires_selection/main.omg`;
  it rejects with `requires exact authored selection` on macOS. Semantic
  evaluation still needs exact selected execution across helper calls,
  independently of package permission. Admission is owned by
  `build-time-evaluation/src/admission/selection_authority.rs`.
  Next acceptance: evaluate that selected provider to `Buffer<0>` while keeping
  unrelated operator declarations from changing builtin arithmetic.
  Complete builtin proof `Int` division and remainder beyond exact constant
  operands, preserving their semantics in retained symbolic proof terms and
  independent checking. Source entailment of closed expressions and quotient bounds in
  `validation/src/contract_entailment/arithmetic_judgment.rs` does not close
  that replay boundary. Acceptance: positive/negative dividend and divisor
  combinations satisfy the paired integer law, zero divisors fail admission,
  `a: Int` selects integer operations with anonymous integral operands, and
  existing fixed-width policies and exact anonymous `/` remain unchanged.
  Audit all admission paths rather than treating one evaluator's decline as
  evidence of a language-wide rejection.
  Acceptance: implicit cross-state use rejects, while explicit renamed
  transfers retain exact contracts, field selection, ownership and cleanup
  without requiring a runtime copy. Wrong results, mismatched output origins,
  and invalidated writes reject scalar postconditions at every normal exit.

- **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.** Finish ordinary generic
  `drop<T>` and runtime cleanup invocation after exact owner-attached hook
  selection. Erased fields remain semantically present but never produce
  runtime cleanup. Acceptance: every path invokes the exact selected hook once
  or proves the value transferred/consumed.

- **EXTERNAL-ENTRY-STACK-EPOCHS.** Finish exact enter/body/exit stack epochs,
  context-specific provider dispositions, finite nesting, and installed-root
  binding. Acceptance: WCSU, stack leases, artifact entry, and runtime context
  independently rejoin; unresolved or cross-context dispositions reject.

- **TR3-TR8.** Finish whole-call-graph worst-case stack derivation, exact
  `StackPlan`, nonmoving `StackLease`, suspension/cancellation preservation,
  transactional arguments, park/resume lowering, and the suspension-safe loan
  subset. Bind an authoritative possibly-suspending crossing roster so coordinated
  deletion of both a Terminal site and plan cannot erase a required crossing.
  Acceptance: stack/control custody is never compiler-owned or lost across a
  suspension edge, and missing crossing demand rejects under the
  [call/outcome contract](wiki/spec/terminal-psi/calls_and_outcomes.md).
  Follow the [task runtime contract](wiki/spec/build/task_runtime.md) through
  `task-plans`, provider admission, and a real selected runtime. Acceptance also
  exercises concurrent start/park/resume/finish, rejection returning every moved
  argument and lease, cross-instance settlement rejection, and fresh storage
  eras on reuse. Static plans and lifecycle ledger tests alone do not establish
  executable activation or argument conservation.

- **ATOMIC-MEMORY-MODEL.** Complete the formal atomic/fence axioms and checked
  target refinement under the
  [concurrency contract](wiki/spec/language/concurrency.md). Owning areas are
  proof semantics, normalized atomic events, and target realization. Acceptance:
  reads-from/modification/global-order constraints and fence synchronization
  are independently checked; swap/fetch retain the instruction-observed prior;
  single-attempt failure retains its distinct outcome and custody. Add real
  concurrent-activation controls once TR3-TR8 supplies that execution route.
  Serial instruction tests and bounded exploration do not discharge these
  proof obligations or authorize a weaker acquire without its protocol proof.

- **BLOCKEXEC.** Implement a package-level blocking executor with bounded
  queues, moved custody, linear completion claims, suspension, and provider
  selection. Hung-worker recovery requiring termination must use process
  isolation.

- **QUOTIENT-THEOREM-LIFT.** Admit explicit representative operation,
  congruence theorem, and optional precondition transport for quotient-owned
  operations. No structural or effectful observer crosses the quotient unless
  its law is explicit and checked. Custody-bearing quotients remain fenced.

- **EVALUATED-FOREIGN-BINDINGS.** Replace string-backed import bootstrap with
  typed compile-time locator values for PE, versioned ELF, and Darwin/Mach-O.
  Carry normalized locator, evaluated plan, target applicability, and producer
  custody through provider selection and native emission. Raw foreign bytes are
  data, never Omega symbol names or ambient lookup authority.

  Extend [normalized-import evidence](wiki/spec/terminal-psi/boundary_calls.md#consumer-owned-settlement)
  from fixed-width scalar calls to a
  source-rooted flat-record argument, then ranked control and port-bearing
  artifacts. Acceptance: independent native replay preserves the exact
  survivor/physical-child bijection and rejects missing, duplicate, substituted,
  or role-swapped children. External realization claims require independently
  admitted concrete authority.

- **FLOAT-PROVIDERS.** Complete runtime Boolean/machine operations for exact
  `FloatMeaning`, kernel discharge, and remaining artifact-aware proof sources.
  Keep IEEE runtime comparison distinct from mathematical meaning equality;
  NaN payloads erase only in the meaning projection and signed zeros remain
  distinct there.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Materialize dynamic trait
  descriptors for pass-through, rebound, and escaping borrows from exact
  selected conformances. Calls may direct-devirtualize only when exact
  selection is proven; bodyless requirements and ambiguous carrier matches do
  not license `dyn`.

- **TARGET-SEMANTIC-APPLICATIONS.** Complete typed target observations,
  hermetic const evaluation, and [selected realization coverage](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children). Finish
  artifact-qualified symbolic substitution for separately compiled generics;
  recheck the reachable specialization's actual capability reach, proof
  obligations, target facts, and selected realization after closing every
  argument. Boundary-operator empty telescopes remain distinct from
  boundary-trait calls with no telescope. Acceptance: cross-artifact canaries
  preserve actual reach and transitive open obligations, reject stale or
  substituted applications, and grant no coverage to unresolved arguments.
  Physical-child binding belongs to `TRANSLATION-VALIDATION` in
  `TASKS_OPTIMIZER.md`.

- **BOUNDARY-OPERATOR-FAMILY-SELECTION.** Extend build selection from exact
  boundary traits to exact package-qualified boundary-operator families.
  Selection is atomic over every overload coordinate and retains target plus
  generic/exact-application coverage. Partial, duplicate, stale, substituted,
  or padded family rows reject; equality of provider assertions is never
  realization coverage.

- **TOP-LEVEL-BOUNDARY-REQUIREMENTS.** Finish explicit public boundary
  requirement declarations, external satisfiers, provider selection, and
  installed execution/era replay. Remove transitional undifferentiated
  bodyless-machine modes once their source migrations close.

- **BUILD-ADMISSION-CHECKPOINT.** Execute an admitted build machine against one
  coherent frontend/source/authority snapshot and append generated source in a
  later resolution stratum. Authored source may not resolve forward into output
  generated by its own build. Acceptance includes replay after serialization
  and drift rejection for the full activation.

  Finish compiler-owned publication of the retained native product built with
  generated source. Bind the exact application root, authored declaration role,
  requested target, and source/build/generated/native inputs; validate final
  realization before publishing. Acceptance: serialized replay reproduces the
  product, and source, role, target, or artifact drift prevents publication.

- **OPTIONAL-STDLIB-SEMANTIC-BINDINGS.** Finish the compiler/library migration
  to explicit ordinary std dependency edges. Std may be replaced, split, or
  absent; only core and compiler-injected vocabulary remain toolchain-owned.
  Migrate package-aware fixtures, keep freestanding UEFI roots dependency-free,
  and retain standalone compatibility only until fixtures acquire package
  roots. Replace std/alloc `Toolchain` classification when compiler consumers
  have exact source-byte catalog entries or explicit semantic bindings.

  Complete composed-Unit plans for trait-default, float, wire, arithmetic-helper,
  guarded-call, and looping-cast canaries and the target-correct non-Linux
  Console catalog entry. Structural writeback shares the blocker recorded in
  `WRITE-ONLY-BORROW`. Feed consumer-scoped Console, Filesystem, and UEFI
  bindings through normal package-aware compilation. Acceptance: removing a
  dependency rejects its imports/provider selections; name, alias, path, or
  same-spelled declarations cannot restore it, and stale or substituted
  semantic bindings reject without relying on accepted-lock replay.

- **COMPONENT-SUBSTRATE.** Implement independently selected component closure
  while keeping deployment/update policy in Cathedral. Componentization must
  bind exact imports, exports, services, mappings, stack demand, leases, and
  installed provider closure under the
  [component publication contract](wiki/spec/build/component_publication.md).
  Until that carrier is complete, every
  `Independent` selection fails at one explicit fence.

- **FFIVAL.** After the generic callback/runtime path closes, run the Windows
  `user32` boundary-coherence canary with no raw function pointer or Win32-only
  compiler escape.

- **WIRE-RUNTIME-AND-INSTALLATION.** Complete reusable artifact validation,
  consumed placement authority, W^X/coherence, physical invocation, and
  uninstall/replacement joins. Keep arbitrary runtime bytes-to-code, JIT, and
  raw executable addresses unsupported.

## Platform-gated verification

- Run Linux host/time/filesystem and `IntegerAt` runtime paths on AArch64;
  cross-target compilation is not runtime verification.
- Build and run the Windows GUI callback canary only through the generic ENT4
  path.
- Keep unavailable hosts structurally tested and report the missing runtime leg
  explicitly.
