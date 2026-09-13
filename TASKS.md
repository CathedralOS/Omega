# Tasks

Unfinished product and language work for the Rust reference compiler. The
[completion plan](wiki/drafts/rust_compiler_completion.md) defines the full
acceptance bar; this board identifies the remaining work, not a passing baseline.

| Start here | Purpose |
| --- | --- |
| [Immediate product closure](#immediate-product-closure) | Unchanged sample/canary programs and the dependencies preventing native execution. |
| [P1–P5](#p1---authority-roots-and-entry) | Entry/storage, materialization, portable evidence, ABI, and Cathedral customers. |
| [Parallel language work](#parallel-language-and-compiler-lanes) | Remaining accepted language surface; independent work can proceed when a product strategy is paused. |
| [Compiler throughput](#compiler-throughput) | Measured work reduction; no speculative cache or framework. |
| [Optimizer board](TASKS_OPTIMIZER.md) / [bootstrap board](TASKS_BOOTSTRAP.md) | Separate execution owners, not duplicated here. |

Keep each task's missing behavior, owner, real dependencies, and acceptance
condition. Delete completed work; Git holds checkpoint results and superseded
diagnoses. Link the specification for semantics rather than restating it.
Retain a dated/revision-bound failure only when it determines where to resume;
rerun that customer before assuming the old diagnosis still applies.

The [workflow](AGENTS.md#workflow) governs scoped checks, strategy pauses, and
landing. A paused strategy is not a blocked language feature. Unsettled owner
decisions belong in [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md); implementation
difficulty alone is not an owner question.

## Ownership firewall

Follow the [connected pipeline](omega-rust/pipeline.md): Psi owns source semantics
through Terminal Psi; Omega owns target realization; backends own physical
details; Cathedral owns OS policy and structures. Complete ordinary operations
and transfers on the common graph, not another source-shape recognizer.

Compiler verification does not require an accepted package lock, and unfinished
native realization does not block source-package installation. Package acceptance,
build authority, and artifact verification remain separate obligations.

## Immediate product closure

These are the next product-level priorities for the maintained Rust
implementation. They take precedence over adding another evidence carrier that
has no exercising program. The finite definition of Rust-product completion is
the [Rust compiler completion plan](wiki/drafts/rust_compiler_completion.md).

- **SQUALR-HEADLESS.** Drive the independently versioned
  [Squalr application](samples/apps/README.md) through its nested workspace and
  package builds. Preserve the essentially 1:1 Rust port, its actual dependency
  graph, and native acceptance; do not flatten packages or substitute a fixed-size
  scanner to fit current lowering. Start with its geometry test application,
  then the real supplied-byte scan/filtered-result path. The submodule's TASKS
  owns port work; this board owns compiler blockers exposed by the unchanged app.

  Available for end-to-end integration under the existing design; no owner
  decision is required to take this work. The pause below rejects isolated helper
  milestones, not the customer. Coordinate one integration owner for the getter,
  count caller, and application command, checking live assignments to the shared
  **MATCH-SELECTIVE-LOWERING** and **STATE-LOCAL-VALUE-FRONTIER** paths before edits.
  Past swarm exclusions are not standing reservations. Recheck the recorded
  failure on fresh main, then state the revised integration plan before resuming.

  Acceptance: `python samples/apps/squalr/tools/verify.py native --timeout 600 --omega <binary>`
  executes the selected application with correct results. Package audit/source
  checking are separate evidence, not the native bar. Current invocation output
  is retained under the application's ignored `build/verification/`; record the
  owning diagnostic before expanding compiler work. Initialization is explicit
  and requires private repository access during in-house development.
  With compiler `1c320cc667` and temporary producer tracing on macOS ARM64, Python 3.13 and
  `RUST_MIN_STACK=67108864`, the native invocation completes ordinary package
  acceptance, then exits 200 at Terminal production after 206.802 seconds:
  `InvalidUnitMachinePlan` for `Main::main`, with reason
  `attached Unit closure is missing a checked transitive machine plan`.
  The application is at `7a272a896c85`
  with standard-package pin `a91d878cb9252647d977c45787969b16e6ef937a`.
  Trace the missing ordinary/composed checked body consumed by
  `checked-trees-to-lowered-psi/src/attached_unit/bodies.rs` through its producer;
  preserve exact closure validation and the authored multi-state geometry checks.
  The application lock records its reviewed macOS baseline. Checkout-specific
  local identities require ordinary update/review when relocated; see its README.
  Geometry execution remains unverified. The existing harness timeout option
  permits the measured package passes without changing compiler checking.

  Scope pause: do not publish another isolated getter/setter or argument helper
  milestone. The unchanged application's trace admits all 14 state signatures
  and constructs `aligned`, then stops at entry statement 1:
  `count = aligned.get_element_count(4, MemoryAlignment::Alignment4)`.
  This loses the caller plan before closure pruning; later mutation states are
  not the observed first cause. The mixed call needs a fresh owned case operand
  beside a shared local receiver and scalar actual, not a fabricated source
  place for the static case name.

  Before resuming implementation, plan one coherent integration through the
  actual four-case `MemoryAlignment::get_size_in_bytes` getter, its unchanged
  count caller, and the full application. Checked scalar terminators currently
  retain only binary conditional destinations; they cannot represent that
  getter's ordered guarded returns. The existing `state_graph/closed_sum.rs`
  route consumes an owned affine subject and routes payloads to named successor
  states. It cannot implement a borrowed tag observation. Consolidate guarded
  destinations with ordinary scalar evaluation and Terminal case observations;
  independently retain exact source guards, coverage, selected operands, and
  the getter's `u64[1..=8]` return obligations. No new Terminal operation or
  weakening of case custody is implied. The checked control representation,
  `flow/terminal_scalar.rs`, scalar graph lowering, and source replay own this
  join; **MATCH-SELECTIVE-LOWERING** shares its required semantics.

  A local experiment connecting case call operands reached the getter's missing
  body in source-to-artifact probes, but did not close execution; broad
  initializer rerouting also regressed established-result consumers. That
  experiment was backed out, not published as support. Resume with existing
  value homes and one complete call evaluation order. Also account for the
  remaining mutable-local setter/read and explicit shared-projection fences in
  `flow/terminal_unit/{state_graph.rs,calls/computation_arguments.rs}` before
  committing to another application integration attempt. These remain engineering
  dependencies, not owner questions. Independent board work remains actionable.

  Reuse the ordinary structural-value/evaluation path in
  `checked-trees-to-lowered-psi/src/attached_unit/structural_values/record.rs`
  and the unified Terminal `EstablishRecord` operands for nested owned fields.
  Local receivers retain their original structural homes through ordinary calls,
  including mutable nested fields; shared projected getters retain exact paths.
  **STATE-LOCAL-VALUE-FRONTIER** owns remaining joins exposed by the actual
  application. Preserve authored operation order and the full package graph;
  keep build-only packages explicitly unported. Source admission and focused
  native receiver coverage do not establish the application's remaining sum,
  refined-result, state-transition, and provider execution paths. This is
  engineering work, with `Squalr geometry: PASS` as the native acceptance marker.

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

- **SAMPLE-CORPUS.** Close the unchanged maintained programs through checked
  semantics and native execution. Start with the documented
  [cli_mvp](samples/cli/basics/cli_mvp/README.md) and
  [print_squares](samples/cli/basics/print_squares/README.md) commands.
  Ordinary CLI review needs real project acceptance; the native sample harness's
  test-owned acceptance does not establish that route. Run
  `mbx nextest run -p compiler --test samples_compile --no-fail-fast` with
  `OMEGA_SAMPLE_RUNTIME_FILTER=cli_mvp` or `print_squares` and
  `-E 'test(=samples_with_documented_exit_run_correctly)'` for the focused
  native probe. In PowerShell set `$env:OMEGA_SAMPLE_RUNTIME_FILTER = 'cli_mvp'`;
  in a macOS shell prefix the command with `OMEGA_SAMPLE_RUNTIME_FILTER=cli_mvp`.
  Old observations below are resume evidence, not a fresh
  baseline after the native planner cuts.

  | Customer/dependency | Remaining work and owning route |
  | --- | --- |
  | `cli_mvp` entry | The macOS probe at `48daf28aa4` stopped at `ProgramEntry receiver provisioning failed`. Resume **ENTRY-CONTENT-ROOTS** and **INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION**; remove the retained-self retry only once the real bridge supplies its activation loan. |
  | `print_squares` closure | The macOS probe at `a8908f279b` stopped at a missing checked transitive Unit plan. Psi's `flow/terminal_unit/` and `checked-trees-to-lowered-psi/src/attached_unit/` must retain projected calls, whole/indexed text writes, and runtime state values. **GENERAL-CYCLIC-EXECUTION** owns cyclic completion; **NOMINAL-FIELD-FLOW** owns declared field facts. |
  | Fixed-range Console input/output | Compose the selected source provider and real byte leaves with original receiver storage, exact returned cases/prefix, once-only effects, and cleanup. Use the ordinary graph and provider replay, not the deleted Unit/boundary planner. Windows byte I/O still needs imported-call/fixup/frame custody; Linux runtime evidence requires matching hosts. |
  | Receiver and aggregate operations | Finish shared/indexed projections, owned/local roots, scalar-result receiver calls, nested sum results and whole replacements, including mixed foreign-result assignments. Extend the shared statement sequencer; **WRITE-ONLY-BORROW**, **STATE-LOCAL-VALUE-FRONTIER**, and **CML4** own the corresponding joins. |
  | Text and field proofs | Replace sample-local `Utf8`/compiler-name `valid_utf8` recognition with the [library encoding contract](wiki/spec/language/domains.md#byte-containers-and-encoding-domains). Keep raw bytes and qualify only validated prefixes; no byte-to-character re-encoding, hidden length writeback, or capacity-as-live-length proof. |
  | `cli/proofs/math_proofs` | Supply ordinary core multiset data and slice extraction, exact selected laws, and structural proof terms for indexed values/subslices. `core/seq.omg` is not a Bag implementation; equal lengths cannot establish equal contents. Preserve the false twin. |

  **Scope pause:** do not resume helper-by-helper expansion for `print_squares`
  until a plan covers its complete source closure and native dependencies. Two
  earlier prerequisite milestones left the same missing Main plan. Preserve its
  source graph; unranked cycles require no invented termination witness.
  Independent operation work remains actionable. Package latency work belongs
  only to **PACKAGE-PREPARATION-REUSE**, not this execution task.

  Regression entrypoints: `tests/native-differential/tests/terminal_byte_views/`
  (`natural_writer`, `mutable_writes`, `byte_input`, `byte_output`),
  `scalar_case_results.rs`, and the selected Console reader controls in
  `compiler/tests/canary_suite/providers_float_and_console/console_reader.rs`.
  Use the [Terminal production map](omega-rust/psi/compiler/terminal-production/README.md)
  for current producers; past fixture publication is not current sample closure.
  General native joins/replay belong to **TRANSLATION-VALIDATION** in
  `TASKS_OPTIMIZER.md`. Keep exact source, ABI, occurrence, access and residual
  cleanup rather than restoring retired fixtures or whole-function families.

  Finish the [bounded-input](wiki/spec/resources/bounded_input.md) integration
  and honest hosted blocking/crash envelope in
  `build/selected-dispatch/src/compiler_intrinsic.rs`. The result schema does
  not itself supply a count-to-extent theorem; retain explicit prefix guards
  until callable relational evidence exists. Acceptance covers zero-capacity
  non-consumption, LF included at the last byte, EOF prefixes, Full without
  overread, repeated chunks, exact bytes including CRLF/NUL/multibyte input,
  failed-read traps, untouched tails, alias rejection, and caller continuation.
  Invalid result cases and unproved text qualification reject. Every maintained
  sample must check and every documented exit/output oracle must run on its
  matching host; record unavailable hosts separately from cross-emission.

- **CANARY-CORPUS.** Bring the language corpus to its promised checked/native
  stages, using `mbx nextest run -p compiler --test canary_suite --no-fail-fast`
  and the focused filters in [AGENTS.md](AGENTS.md#running-one-test).
  Repository/library gates do not establish corpus health. Reconstruct the
  current failure distribution rather than carrying forward the old claim that
  most failures are missing transitive Unit plans. Known dependency categories
  include **SAMPLE-CORPUS**, **NOMINAL-FIELD-FLOW**, selected entries and
  index/subslice proofs. Attribute each failure to its operation owner, not a new
  task per fixture. Passing one earlier checking phase is not full acceptance;
  rerun affected cases after each repair. Remove this umbrella once the corpus
  passes, not merely once every failure has a label.

- **TERMINATION-RANKING-CHECKS.** Complete the documented flow-dependent
  rank-range checks in
  `typed-trees-to-checked-trees/src/checks/termination/ranking/` and
  `validation/src/call_cycles/runtime_ranking/`.
  Transfers with diverging copies of rank inputs, and call components with
  internal state arrivals, or slice-length,
  bounded-distance, or custom views need
  exact arrival mappings and preserved premises for ranked subjects and pinned
  endpoints. Mutable premises need live write-frame evidence.
  Custom struct-view ranges involving borrowed or nested projections,
  constrained measure parameters, duplicated record roles, and dependency-free
  initial record arrivals need exact view-application evidence.
  Retire generated operand-call states through
  STATE-LOCAL-VALUE-FRONTIER's checked computation route rather than add ranking
  provenance for those artificial edges. Flow-dependent computed
  endpoint formation needs its own arithmetic proof, not an unchecked
  polynomial. Non-polynomial endpoint substitutions beyond exact input
  forwarding need their own equality evidence.
  Scalar views beyond bare unsigned identity forwarding, and slice lengths
  over projected storage need their produced-rank facts.
  These are implementation gaps, not grounds to weaken the range obligation.

  Acceptance: named-state and call-component rank ranges accept proved
  constraints while changed endpoints and intervening writes invalidate their
  premises. Preserve the private-witness/public-guarantee split described in
  chapter 3 and the
  [termination contract](wiki/spec/language/termination.md).

## Compiler throughput

These are bounded work-removal tasks, not a new performance framework. Preserve
the [pipeline ownership and independent checks](omega-rust/pipeline.md).
Source inspection identified the costs below; it did not establish their share
of whole-compilation time. Record compared inputs, work counts and timings;
do not claim a faster compiler from a smaller helper alone.

- **FLOW-DIRTY-STATES.** In
  `typed-trees-to-checked-trees/src/flow/{builder,state_values}.rs`, remove
  remaining semantic-baseline copies only if customer-sized measurement justifies
  the change. Comparisons against `14e9fd8173` reduced requested
  bytes but did not establish a timing win; direct restoration probes were a
  small share of checking. Do not add an arena rewind framework to close an item.
  Acceptance: cyclic/reverse-ordered dependencies beside independent machines
  revisit only affected transfers, agree with the whole-pass reference, and
  preserve first-pass cost, context links, generations, all-predecessor facts
  and conservative nonconvergence. Use `checking_allocations` and
  `complete_checking_matches_reference_with_reverse_chain_and_cycle` in the
  owning crate; measure full checking, not restoration alone.

- **PACKAGE-PREPARATION-REUSE.** Measurement-gated in
  `omega-rust/omega/packages/manager/src/review/candidate/compilation.rs` and
  compiler source preparation. The macOS ARM64 `cli_mvp` probe at `9d9075eb61`
  spent 46.248 ms across four source preparations in a 19.36 s route; this does
  not justify a public parse checkpoint or persistent cache. Further
  symbol-lookup tuning is likewise paused after failing to improve the outer
  command. Resume only with material phase self time on a real workload.
  Acceptance: an already-built release CLI prepares unchanged binding-independent
  inputs once, invalidates changed source/selections, and improves whole-route
  cost without changing findings, admission, or generated-source custody.
  Discovery/final checking can select different std bindings; neither checked
  results nor build effects may be reused merely because bytes match.

Optimizer revision/analysis reuse is tracked only in `TASKS_OPTIMIZER.md`.

## Automatic service reach

Carry [static callback reach dependencies](wiki/spec/language/effects.md#static-callback-reach-dependencies)
through Terminal evidence/replay. Preserve the original generic dependency and
exported binder telescope across the portable boundary. Use the finite union in
`flow-effects::ServiceReachInferencePlan` and checked service-reach facts; reuse
retained static-call contracts and exact specialization commitments rather than
introducing another selection identity. Closed applications must independently
replay exact substitutions from selected public contracts, including nested
generic/private helpers and recursive call components. Preserve direct boundary
declarations, pinned requirement bounds, and independent suspension/blocking
checks. No forwarding syntax, closure machinery, reach-prohibition syntax, or
backend effect inference.

Acceptance: the same named traversal with no-reach and Console callbacks publishes
empty and Console rows to ordinary callers and evaluation admission respectively;
the former evaluates when its other obligations hold. Missing direct boundary
declarations and selections exceeding requirement bounds reject. Cover nested
generic/private helpers, recursive call components, opaque/dynamic conservatism,
and unchanged summaries after call reordering/helper extraction. Adding Console
in a private helper or selected contract changes interface identity and invalidates
stale evidence; a no-reach requirement/evaluation context rejects it. Transitive
reach cannot be hidden by omission or a memberless clause. Fixed operational
markers and suspending-call positions remain enforced for every selection.
Keep `tests/omega/pass/effects/nominal_callback_const_reach/main.omg` checking
through the CLI and its structural/Console negative controls rejecting; constant
evaluation must not consume the source program's generic declarations.
Keep the source-to-canonical-policy regression in
`tests/omega/pass/effects/nominal_callback_dependency/README.md` passing when a
public generic also has closed applications; original interface identity must
not depend on which applications are present. Its private closed-application and
unused-generic controls establish template preservation, not Terminal custody.
Its `service_reach_contracts` command covers publication, reload and interpretation,
including inert declared reach, private helpers, and interleaved binder positions.
Run the README's Cargo/nextest commands on macOS ARM64 for full closed telescopes,
finite substitution, selected public rows, and two-sided emitted-call joins after
source discard.
Producer ownership is `checked-trees-to-lowered-psi/src/closed_reach_applications.rs`;
reload checks are `terminal-verifier/src/validation/reach_applications.rs`.
Selected generic schemas use per-call closed tuples and exact callee application
joins; exercise source-free execution and hostile controls with
`tests/omega/pass/effects/generic_callback_schema_reach/README.md`.
The same projection covers fixed-only type/const applications, isolated identity
callbacks, and unused selected ordinary or generic contracts without emitted bodies.
Keep the [linear structural callback regression](tests/omega/pass/effects/structural_callback_reach/README.md)
publishing and executing after source discard while rejecting stale call and claim
custody independently of its retained reach application.
Broader portable coverage for inlined/missing callees, unresolved installation
selections and independently checkable original-contract projection openings
remains required. Template/specialization commitments identify provenance;
they do not authenticate the structured projection. Original finite dependencies
replay against the retained source graph
in `validation/src/static_machine_call_contracts.rs`, but the remaining template
encoding contains frontend-local identities. Full source-contract reconstruction
and generic PCC remain open; ordinary relational replay does not establish them.

## Semantic reflection

Implement [semantic reflection](wiki/spec/language/reflection.md) in Psi schema
construction, hermetic evaluation, checked per-member calls, and Terminal replay,
with ordinary library inspector/encoder policies. Use named callbacks, explicit
context data, and existing callable-family checking; do not add a closure IR or
format-specific compiler. Anonymous syntax is not an implementation dependency.
First deliver owned qualified schema graphs, authorized projections, and scoped
typed selections frozen into independently checked result snapshots. Continue
with recursive derivation, explicit runtime metadata/adapters, and authorized
Placed access under the same contract. Fixed callbacks can proceed independently;
selection-dependent evaluation reach uses the Automatic service reach task above.

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

## Scoped build execution

Implement [scoped build execution](wiki/spec/build/scoped_execution.md) for
ordinary code generators and the topology composition customer. These are
accepted contracts, not claims of implementation. Existing
`BUILD-ADMISSION-CHECKPOINT` owns retained frontend/generated-source replay;
extend that route, not a second build language or plugin executor. Any semantic
or trust amendment found here or later goes through [owner questions](OWNER_QUESTIONS.md).

- **BUILD-DEPENDENCY-PURPOSES.** Extend `omega-rust/omega/packages/` acquisition,
  review and locks, `omega-rust/omega/build/` declaration projection, and Psi
  package-aware resolution to retain explicit build/product edges and checked
  occurrences. Preserve one root Build and direct discovery. Host libraries use
  their own ordinary dependencies; schedule the combined purpose/profile/target
  prerequisite graph and reject cycles before affected execution. Version the
  lock/review migration without broadening legacy product edges or inferring
  permissions from imports. Acceptance: real acquisition and multi-file builds
  exercise dual-purpose std, conflicting cross-scope aliases, host/product targets,
  missing edges, dependency cycles and cache separation. Removing a build or
  product edge rejects only its authorized selections; source sharing never
  substitutes host output or admission for target evidence.

- **BUILD-PRODUCT-REFERENCES.** In Psi source selection and the existing Build
  root/provider owners, implement designated product operands and qualified
  non-callable entry/provider/schema descriptions. Depends on purpose-aware
  resolution above and the retained authored checkpoint. Define exact source
  signatures under the accepted roles without inventing a general compiler-query
  world. Acceptance: a multi-file helper receives an owner's restricted private
  entry reference and binds it without executing target code. Wrong scope/target,
  lookalike operations, helper enumeration of caller-private declarations,
  description-to-callable forgery and same-build generated/layout cycles reject.
  `build-evaluation/src/selection/root_bindings.rs` now admits executed
  `roots.bind` requests lexically against the occurrence's own package and
  carries the exact machine symbol into `ProgramEntry` selection, so a foreign
  helper binds its own package's entry through the borrowed root Build while
  caller-private names reject; `compiler/tests/build_target_activation.rs`
  pins both. Remaining: the owner-selected restricted description handoff
  (`builder.product.entry(...)` and its non-callable description type), the
  same-name-across-packages fence in `selection.rs` until Terminal production
  and entry settlement select by symbol instead of machine name, and the
  computed-receiver implementation fence once ordinary call-result authority
  and effect/loan traversal can carry that use.
  Final admission must rejoin the exact selected identity after generation.

- **BUILD-SNAPSHOT-OUTPUTS.** In build evaluation, its host custody adapters and
  compiler publication, implement coherent captured inventories, narrowed inputs,
  deterministic snapshot reads, fresh append-and-seal staging, linear required
  outputs, and direct artifact-only discovery. Define exact facet signatures and
  protocol tags; no live-host grant extension or persistent writable cache.
  Acceptance: an ordinary generator reads a template and completes a required
  file; artifact-only and executable-with-companion routes both work. Exercise
  negative lookups, ordering/metadata, symlink and substitution escapes, sealed
  mutation, cross-occurrence receipts, failed completion/retry, omitted required
  members, interruption, and final-check failure without a partial committed set.
  Test isolation and failure on Windows/macOS, explicitly recording unavailable
  hosts. Measure retained input/output state and compare the separate-tool route;
  do not infer confinement or usability from parser tests.

## Build-level behavior exclusions

Implement [the accepted exclusion contract](wiki/spec/build/behavior_exclusions.md)
for one source library whose checking/no-op assertion selection changes product
crash behavior without edits to public ceilings. Assertions remain ordinary calls;
there is no special Assert cause or global permission to violate callable contracts.
These tasks use existing build selection, portable semantics, provider and
artifact-verification owners, not an assertion-specific interpreter or duplicate IR.

- **BUILD-SEMANTIC-EXCLUSIONS.** In `omega-rust/omega/build/`, define exact typed
  configuration and canonical union of crash/service exclusions. Psi checking and
  Terminal evidence own possible semantic operations, closed calls and guard
  proofs; Omega selection and product admission join exact selected implementations
  under the complete entry/dependency closure. Reuse existing evidence rather than
  copy public summaries into a purported actual-behavior verdict. Keep ordinary
  declarations unchanged, forbid effect masking, and reject incomplete or opaque
  evidence conservatively. Depends on the selected-call/entry evidence exercised
  by the customer; missing coverage must remain an explicit implementation limit.

  Acceptance: compile identical library source/public Trap ceilings with checking
  and no-op implementations; under a Trap exclusion only the no-op composition
  passes, with optional optimizations disabled and enabled. Eager predicate traps,
  unrelated traps, callbacks, cleanup and generated entries remain accounted.
  A silent ordinary logger can pass no-Console; a silent provider for an actual
  Console invocation cannot. A build permitting Trap never bypasses a no-crash
  intermediate interface. Independently consume retained evidence and reject
  substituted entries, providers, scopes, targets, policies and missing summaries.
  Report prohibited possible behavior separately from insufficient evidence.

  Resume: `build-evaluation::behavior_exclusions` holds the typed
  `BehaviorExclusion`/`BehaviorExclusions` canonical union and
  `establish_behavior_exclusions`, which walks the unoptimized Terminal static
  call closure per entry and reports prohibited sites (crash terminators,
  boundary fixed service reach and declared crash routes, port writes) separately
  from evidence gaps (dynamic calls, unknown entries/callees/boundaries), never
  consulting in-module public ceilings. Unit coverage:
  `cargo nextest run -p build-evaluation behavior_exclusions`. Next: the
  `Build` authoring surface for exclusions with retained declaration spans, then
  the product-admission join over the selected entry roster and provider
  candidates, then the compiled-corpus acceptance with optimizations on and off.

- **BUILD-EXCLUSION-REALIZATION.** Extend the existing receiving-policy and native
  admission route in `omega-rust/omega/build/`, `omega-rust/omega/semantics/` and
  target backends to enforce excluded physical terminal classes without inventing
  a second classifier. Join semantic exclusions from the preceding task through
  final realization and the component installation/replacement envelope. Reuse
  `COMPONENT-SUBSTRATE` and `WIRE-RUNTIME-AND-INSTALLATION` for actual custody and
  lifecycle; no build-time callback may inspect its own unfinished executable.

  Acceptance: distinguish no-Console from no physical process output, reject
  unknown/changed mechanism classifications and failed final checks without
  successful publication, and preserve exact policy/evidence identity through
  source-free consumption. Rebinding or replacing code with an excluded behavior
  must fail the existing envelope even when the old provider was benign. Keep
  target children and build-host activity scoped separately. Exercise physical
  provider and installation controls on each available supported host, explicitly
  reporting unavailable Windows/macOS coverage; a semantic-only pass establishes
  no physical absence claim. Specify versioned source/protocol fields before
  claiming compatibility. Any unresolved semantic or trust change goes through
  [owner questions](OWNER_QUESTIONS.md), not a weakened implementation verdict.

## Checked boundary topology

Implement the [reference-package contract](wiki/spec/packages/topology.md), not
a compiler graph stage. `COMPONENT-SUBSTRATE` owns the complete verified component
consumer below; `WIRE-RUNTIME-AND-INSTALLATION` owns generic executable custody.
Package policy and orchestration stay in ordinary Omega libraries, with native
details in providers. A topology-specific IR or new trusted graph axiom is not
an implementation shortcut.

- **TOPOLOGY-PLAN-VERIFICATION.** Deliver an ordinary build-only package and
  composition project over prebuilt component artifacts. Depends on the scoped
  build output route and independently verified complete descriptions; missing
  evidence must not be replaced with hand-authored inventories. Implement fixed
  `no_route`/`only_via`, bounded deterministic graph normalization and diagnostics,
  correctness evidence, exact owner-supplied `TopologyRequest`, and versioned
  codec tables/fixtures. Acceptance: the payment graph succeeds and direct/indirect
  bypasses reject with checked witnesses; cover cycles, disconnected sources,
  duplicates, stale subjects, forged completeness, missing owner policies and
  unselected policy executables without loading them. An independent source-free
  consumer reconstructs the same graph, rejects corrupt plans, and replays the
  selected predicates. Typechecking a graph algorithm alone is not correctness
  evidence, and successful publication is not an installation claim.

- **TOPOLOGY-PRIVATE-PIPE-INSTALLATION.** Build the package-owned installer and
  Windows/macOS pipe adapters for three checked local payment processes. Depends
  on plan verification, complete component admission, and generic executable
  installation. Use one bounded request/response pair per binding, one outstanding
  request, no general inheritance, dynamic delegation, discovery, or retries to
  another peer. Acceptance: independent current request authorization, exact
  all-import binding coverage and confined endpoints precede application entry;
  an actual ungranted invocation and substituted mapping are refused. Preparation
  failure returns/cleans custody; partial activation retains supervision and
  quiesces or reports cleanup failure without a success receipt. Replacement
  quiesces the old generation before starting the new one. Test both supported
  hosts when available and disclose loader/OS assumptions; graph-only tests,
  signatures, and ordinary process spawn do not prove physical confinement.

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

## P1 - Authority, roots, and entry

Owners include
`wiki/spec/resources/authority.md` and
`wiki/spec/resources/storage.md`.

- **ENTRY-CONTENT-ROOTS.** Connect the generated physical entry to the exact
  semantic continuation under the [entry contract](wiki/spec/build/entry_roots.md).
  Owners: target package source assembly, `target::TargetProfile::program_entry_slot`,
  `program-entry-plan`, `external-roots`, and
  `native-realization/src/realization/native_artifact.rs`. Targetless checks select
  no entry; deployment cannot substitute a semantic machine for a physical adapter.

  First join the authored hosted contract and its source/package/application
  identities through the selected slot. The macOS contract is
  `source/library/std/targets/macos_arm64/entry.omg`; its physical arrival and
  `ProgramStorageEntry` are distinct applications. Provider-module imports do
  not load that contract automatically. The `calling_policy_plans macos_entry`
  test exercises signatures, not installed roots. Derive real backing/rights,
  stack and receiver partitions from the admitted target arrival and installed
  occurrence/epoch, then construct a ZII-valid receiver, pass its single
  activation loan, and account for normal cleanup. A writable section, prepared
  input, or C caller supplying a receiver pointer is not that bridge.

  Acceptance: execute an authored receiver entry as a published process with no
  test-supplied `self`. Reject redirected continuation/receiver identities,
  non-ZII state, insufficient/misaligned backing, overlapping partitions, stale
  occurrence/epoch and bypassed provisioning. Retain the native admission
  rejection until this works. Extend to zero-payload provider fields and the
  `traits/runtime_local_named_dyn_stored_exit` customer (exit 70 on both Linux
  targets), subject to its separate descriptor-call dependency. Preserve exact
  symbol/text, source contract, runtime storage and continuation replay.

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

- **PCC-PRODUCT-PUBLICATION.** Implement the
  [optional proof product contract](wiki/spec/proofs/publication.md) through
  normalized root Build, cross-invocation inputs, artifact codecs, independent
  verification and final publication. Two independent off-by-default requests
  produce adjacent `.proof` sidecars, never an embedded-only route or a second
  compilation pipeline. Remove superseded embedded publication as this lands.
  Receiver requirements come from an independently pinned policy package and
  concrete configuration, not producer hints. Keep ordinary checking mandatory.
  First deliver bounded supported evidence end-to-end; general claims depend on
  `PROOF-KERNEL-CORE`, `PROOF-CERTIFICATION-BRIDGE` and completed profile rules,
  not a new policy DSL. Acceptance: all four request combinations; separate file
  sizes; native-only checking after source/Psi deletion; macOS inner sidecar;
  exact omitted dependency possession; wrong bytes, premises, policy, assumptions
  and stale sidecar reject; exhaustion reports `Incomplete`; no partial or
  uncertified success. Extend standalone output framing explicitly for pairs.

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

- **GENERAL-CYCLIC-EXECUTION.** Complete the
  [cyclic control contract](wiki/spec/terminal-psi/control_flow.md) and
  [separate safety/progress rules](wiki/spec/language/termination.md) for
  `print_squares` and the Console writer. Owners:
  `terminal-verifier/src/validation/control_flow.rs`, Psi's shared
  `flow/terminal_unit/` and `checked-trees-to-lowered-psi/src/attached_unit/`,
  and Omega's ordinary `lowering/control_flow/` plus receiving graph replay.
  Unit/scalar/aggregate functions already share that native graph; do not
  recreate the deleted Unit planner or unsigned-countdown native route.

  Extend bounded safety/proof admission to qualified and partial owned custody,
  structural results, projected claims and effectful calls. Preserve dominance,
  exact successor transfers, ownership frontiers, current-iteration guards and
  test-fuel suspension/resumption. General cyclic invariants/ranking views need
  retained evidence; guarded-crash checking must not enumerate unbounded paths.
  Source production must compose projected helpers, Console structural operands,
  indexed/aggregate writes and computed results without state duplication.
  Finite fuel or a relaxed shape check is not a safety proof.

  Acceptance: the unchanged customer reaches native exit/output on the hosted
  matrix, with exact caller/callee resource composition and independently checked
  ranking where declared. Corrupt arrivals, ownership, guards, effects and proof
  groups reject. **SAMPLE-CORPUS** owns whole-customer execution and its current
  strategy pause; interpreted loops or isolated graph tests cannot close it.

- **CRASH-CONTRACT.** Carry invocation-specific crash obligations through
  operators, nested structural paths, calls, cycles, execution and package review.
  Owners include `facts/operator_crashes.rs`, `CrashPlan::checked_operators`,
  captured operands in `flow/expression.rs`, and Terminal/native evidence
  consumers. Source checking alone is not portable proof. Retain exact selected
  requirement, saved actuals, Match arm and surviving route; do not invent calls,
  infer semantic crashes from emitted traps, or narrow opaque contracts by
  inspecting providers.

  Boundary declarations now retain scalar guarded ceilings through canonical
  encoding and independent actual-argument substitution; run
  `cargo nextest run -p checked-trees-to-lowered-psi --test scalar_boundary_arguments --no-fail-fast`.
  Reuse that requirement contract for selected operators, retaining exact occurrence
  evidence. Source crash predicates currently lower through fixed-width
  `ScalarTerm`; proof-only mathematical terms do not imply an authored
  mathematical guard route.
  Boundary crash observation profiles, guarded refinement of crashing providers,
  and Omega projection remain open. The edge-only `TerminalTraceV1` profile and
  Omega projection reject nonempty boundary routes; crash-free providers can
  already refine a may-crash requirement. Extend the source-to-execution controls above
  while preserving exact call sites, guard actuals, abandoned claims, staged
  writeback and no-result/no-cleanup behavior.
  Carry qualified scalar results and the remaining normal-contract vocabulary
  through ordered boundary completion. Reuse the machine-entry and scalar
  normal-guarantee path (`unit_scalar_result_source`'s `ordered_boundary`,
  `ordered_scalar`, `ordered_boolean`, `ordered_computed_boolean`,
  `ordered_nested_boolean`, `ordered_saved_boolean`,
  `ordered_call_produced_boolean`, `ordered_boolean_call_computations`,
  `ordered_boolean_guarantees_compose_through_dependent_joins`, and
  `ordered_boolean_completion_preserves_folded_source_meaning`, and
  `ordered_mutable_boolean_snapshot` tests).
  State/control contracts, mutable snapshots crossing state joins, and
  field/arithmetic predicates still need their evidence joins. Preserve
  authored callee contracts regardless of whether a helper is a direct closure
  root or a transitive dependency; a checked call identity is not contract proof.
  Mutable scalar inputs still need the shared signature/storage path beyond the
  invocation-entry read checker. Do not infer normal guarantees from crash ceilings
  or use current storage as an entry snapshot.

  Package contract review still needs exact carrier/value custody for declaration
  and result projections through indexes, case payloads and generic field
  substitution beyond ordinary declaration-owned field paths.
  Reuse Psi's exact carrier/case relation; do not manufacture a machine owner
  from the classifier. The retired `proposition` declaration surface belongs to
  **PROOF-CONTRACT-MIGRATION**, not an independent membership-extension task.
  Saved or call-produced result tags without a live predicate need ordinary
  value/effect custody; do not replay initializers or callee bodies to recover
  a tag after its evaluation point.
  `cargo nextest run -p package-evidence --test callable_policy case_membership --no-fail-fast`
  is the existing source-to-recovery control. Selected operator crash
  invocations now project through both package crash projections: review emits
  per-site rows (exact selected operator, caller state/statement, published and
  surviving buckets; an empty surviving set is a proved discharge) via
  `capture/behavior/crash/operator_projection.rs`, and callable policy consumes
  the same retained rows at cause level. Regression:
  `cargo nextest run -p package-evidence --test callable_policy --no-fail-fast`.

  Extend `facts/crash_entry_values.rs` beyond immutable stable-content roots to
  state arrivals, rebinding, mutable field versions and receiver/case projections,
  including the separate named-operator use path. Unknown
  provenance must remain conservative; current spelling/live storage is not a
  saved actual. This owns **MATCH-SELECTIVE-LOWERING**'s crash-qualified equality
  dependency and shares entry snapshots with **STATE-LOCAL-VALUE-FRONTIER**.

  Acceptance: source `operators/crash_routes` and crash-qualified float controls
  retain exact surviving-route evidence through independent Terminal replay and
  execution; package projections already carry the site rows. Safe uses
  discharge each route; changed guards, captures, substitutions, sites and stale
  writes reject. Preserve examined/discharged routes and caller coverage, not
  only the final cause set. The replay gap is concrete: the Terminal verifier
  reconstructs call crash continuations only from an ordinary callee's
  `contract.crash_routes`, operator surviving routes are invocation-specific
  and may carry no portable `scalar_expression` after conservative `Truth`
  widening, and boundary operator declarations do not yet carry a replayable
  Terminal crash contract; direct lowering rejects selected operator crash uses
  at `checked-trees-to-lowered-psi/src/lib.rs`. Copying checked rows onto
  `MachineContract` alone would not establish replay meaning. A Terminal row
  also has no producer yet: checked scalar computations admit a selected
  comparison only through `selected_float_comparison`
  (`values/scalar/computations/dispatch.rs`), so a selected integer boundary
  operator such as `Comparison::equal(i32, i32)` binds no
  `CheckedScalarComputationKind::SelectedComparison`; with the lowering fence
  bypassed, `may_crash`/`safe` in `operators/crash_routes` reject earlier with
  "scalar computation needs one checked expression and one source binding".
  The next slice is that checked computation (exact operator use plus
  published/surviving routes), then a Terminal operation-level carrier whose
  verifier substitutes operands as `validate_call_crash_coverage` does; design
  the row only after the producer exists.

- **PROOF-KERNEL-CORE.** Build the common mathematical term/declaration model
  and independent checker in Psi, under the
  [selected foundation](wiki/spec/proofs/foundation.md). Customer: library
  theorems about arbitrary types/predicates and dependent witnesses, not another
  extension to the bounded `Proposition` enum. Represent the reference core's
  universes, dependent terms and strict/relevant distinction once; source
  elaboration and certificate consumers must use it rather than invent parallel
  truths. Keep search outside the checker and preserve useful arithmetic rules
  as certificate producers or explicitly justified checked rules.

  Implement the pinned reference core and selected
  [W-based profile](wiki/spec/proofs/inductive_profile.md): relevant identity,
  two-element type, W-induction and checked derived indexed families. No second
  primitive indexed/strict-inductive checker. Prove the encoding scheme's
  formation, constructor, dependent-induction and computation correspondence,
  then independently check its declaration applications. Structural round trips
  do not establish meaning. General source punctuation is not a kernel blocker.
  Implement [typed function eta](wiki/spec/proofs/inductive_profile.md#typed-function-eta)
  in conversion, preserving exact Π types, freshness, levels and admitted sort
  combinations. Justify substitution, preservation, normalization and decidable
  conversion for the combined rules; no untyped wrapper-deletion shortcut.
  Acceptance: independently checked universe-polymorphic dependent functions
  and pairs, strict same-statement conversion and relevant witness separation;
  malformed universes, capture-changing substitution and illegal elimination
  reject. Include exact assumption closure through declaration types/statements
  without relying on unfolding. Measure conversion/storage on these terms; do
  not claim feasibility from empty receipts or compiler-authored success flags.
  The next milestone must connect this model to source or a real theorem
  certificate, not expand a disconnected proof framework.

  Demonstrate Vector length indices, derivation context/conclusion indices,
  mutual and nested strictly-positive families with definitional constructor
  computation with a visible constructor and arbitrary neutral child function,
  not only concrete lambdas. Include dependent motives and capture/type-mismatch
  controls; pointwise equality alone grants no function equality. Reject malformed
  encodings, negative recursion, bad universes and illegal strict elimination;
  separate source invalidity, unsupported valid
  encoding and producer defects. Measure term size, retained storage and checking
  cost in the application checker, not an assumed nested bootstrap environment.
  Complete rule/encoding metatheory and implementation evidence before claiming
  a verified profile. Reopen W only on demonstrated requirements/cost/audit failure.

- **PROOF-CONTRACT-MIGRATION.** Migrate the proof surface to
  [ordinary machine contracts and trait bundles](wiki/spec/proofs/contracts.md#machines-and-bundles).
  Owners: Psi syntax/resolution/typing, contract proof semantics, Terminal
  evidence/codec/replay, and core mathematical traits. Use `PROOF-KERNEL-CORE`,
  not a second general logical representation. Implement the selected
  [mathematical bindings](wiki/spec/proofs/mathematical_bindings.md): closed
  parameterized top-level `let`, dependent function types, curried prefix
  application, core-named universes/level binders and `boundary let` assumptions.
  Their general certificate integration follows the settled
  [PCC checking/publication contract](wiki/spec/proofs/publication.md).
  Preserve ordinary local bindings, complete machine calls and executable callback
  selection. Do not introduce quantifier keywords or substitute declaration
  enumeration or an optional-returning decider for general mathematical quantification.

  Replace the dedicated formula-declaration and hidden-witness call machinery
  with ordinary contracts and named witness/law bundles, preserving exact
  substitution, result/path availability, erasure, validity, and transitive
  assumptions across trait calls and artifacts. This incorporates the former
  selected-witness and trait-named-witness work; do not widen those old surfaces
  independently. Retain useful checking rules, not mandatory wrapper syntax.
  Include value/computation elaboration here: a total admitted invocation may
  denote mathematically; effectful calls expose outcome contracts, never effects
  executed by conversion. Mathematical result-bearing axioms are explicit in
  checked declarations, not missing-provider slots or a new domain qualifier.
  Enforce executable demand across data, calls and control flow, consistently
  at source use, evaluation and lowering; proof-only references retain trust
  without creating runtime demand.

  Acceptance: actual proof scripts and false twins pass through source,
  Terminal serialization, and independent replay for all five cases:

  1. Composition of two witness/law bundles preserves exact substitutions and
     distinct witnesses.
  2. A higher-order theorem quantifies over arbitrary mathematical predicates or
     functions, not an enumeration of executable declarations. Supply the
     machine-shaped logical hypothesis from derived Π-term evidence, with fixed
     subjects and discharged premises, not only a named proof declaration.
     Reject that supply for reset-through-borrow and executable callback contracts.
     Exercise `greater_than(limit)`, dependent result substitution, shadowing,
     independent universe levels and stable inferred public level telescopes.
     Partial mathematical applications are complete function terms; wrong expected
     types and missing machine arguments reject without implicit runtime closures.
  3. Nonconstructive existence uses an explicit axiom and cannot supply an
     executable witness without checked realization. Cover a chosen `u32`, a
     choice-dependent branch, and an erased theorem that mentions that value.
     Also prove squashed witness existence from another squashed witness and a
     logical hypothesis, with no choice assumption. Establish constrained records
     at construction, retain exact evidence dependencies, and keep relevant
     dependent witness bundles distinct from strict predicate gating.
  4. Accepting and denying policies distinguish the same theorem, with exact
     transitive assumptions surviving import, erasure, serialization, and replay.
  5. A Cauchy/quotient proof uses the selected
     [set-quotient interface](wiki/spec/proofs/quotients.md#set-quotient-foundation),
     with exact universes, set-valued dependent elimination, point computation
     as a law and explicitly admitted effectivity. Check derived lift, coverage,
     proposition induction and pointwise uniqueness; no hidden extensionality
     or kernel reduction. Squashed relations yield only squashed evidence.
     Preserve representative operations/congruence independently: a quotient-refusing
     policy accepts their quotient-free closure and rejects the assumption-bearing
     quotient proof. This control depends on transitive closure through helper
     statements/types; existing Rat comments do not establish that closure.

  Migrate core relations, quotients, samples, and tests, remove obsolete parser/carrier/codec
  routes, and reject retired spellings. Candidate naming syntax is not a
  prerequisite. Do not claim full mathematical coverage from these controls.

- **PROOF-CERTIFICATION-BRIDGE.** Turn source automation into an untrusted
  producer for the common `PROOF-KERNEL-CORE` checker, not a second authority.
  Retire superseded trusted success-only paths as their certificate route lands;
  do not keep legacy and general proof routes selected by source shape.
  Recursive certificates own one SCC and cite ranking and
  well-foundedness evidence once; normalization names exact laws and preserves
  transitive trust. Acceptance: changing an edge decrease, premise, law, or
  component identity rejects or changes the trust closure. For separately
  compiled dependencies, reconstruct the exact obligations and recheck retained
  certificates locally; propagate unresolved assumptions with their original
  owner. Missing or stale evidence cannot silently discharge an obligation or
  inherit a producer's admission decision.

  Bounded current-rule production and the selected kernel profile are actionable.
  General certificates depend on `PROOF-KERNEL-CORE`'s rule and encoding evidence,
  including the selected typed-eta conversion justification. Common checking
  authority is settled in the [publication contract](wiki/spec/proofs/publication.md).
  Retain closure over complete checked declaration dependencies,
  including statement/type references surviving neither erasure nor final normal
  forms. Acceptance also checks a well-founded denotation against a generated
  loop: measure decrease must not certify an incorrect accumulator update.

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
  Inventory the existing trusted surface in this ledger, not another manifest:
  primitive judgments, checker rules, reconstructed fact kinds, normalization,
  scope, write invalidation and call/cycle composition. Each entry binds exact
  premises/conclusion, dependencies, implementation identity and soundness status
  (`Proved`, `ExplicitlyTrusted`, `Unfinished`). Mechanically fail uncovered
  accepted dispatch/fact kinds; revalidate changes to existing arms too. Coverage
  is not soundness, and unfinished rows cannot establish independent claims.
  Application interpretation uses the common kernel and the selected
  [inductive profile](wiki/spec/proofs/inductive_profile.md). Its unfinished
  soundness/encoding proofs are dependencies, not permission to trust success.
  Bootstrap discharge belongs to `BETA-DERIVATION-CHECKER` in `TASKS_BOOTSTRAP.md`,
  not a prerequisite that forces general mathematics into the Gamma checker.
  End-to-end acceptance uses a theorem-dependent program obligation after
  deleting producer/source state, under accepting and rejecting assumption
  policies. Wrong goals, profile identities and omitted transitive assumptions
  reject. A mathematical theorem alone does not establish native refinement.

- **IRFUEL.** Keep fuel as analysis/evaluator evidence, never inserted runtime
  semantics. Complete installed-code correspondence for ordinary admitted loops
  on the common graph; do not restore the retired countdown-native carrier.
  Acceptance: independently checked costs follow actual operations and calls;
  failure to derive a bound reports `Unknown` or `NoFiniteGuarantee` without
  changing execution.

- **PROOF-RELEVANCE-MIGRATION.** Finish `[erased]` noninterference and
  erased-stripped layout across remaining carriers. Erased terms remain in
  semantic/proof identity but contribute no runtime storage, tags, ABI
  transfer, or execution. Runtime use and any layout-dependent erasure reject.
  Resume: `[erased]` parses only on data fields and case payload fields
  (`tokens-to-syntax-trees/src/parser/data.rs`); every runtime use of those
  two carriers rejects in `validation/src/relevance/`, layout strips them, and
  synthesized `Equatable` now skips them. Next acceptance: decide whether the
  spec's binding-occurrence wording requires `[erased]` on parameters and
  locals (`parser/state.rs`); if so, admit them with the same relevance walk
  and a fail canary for a runtime read.

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
  calls/results, dynamic dispatch, cleanup and native execution. Extend ordinary
  reference preparation/receiving replay in
  `target-operations-to-selected-instructions/src/legalization/scalar_graph_input/`
  and image/installation publication; **STRUCTURAL-BORROW-IDENTITY** owns the
  common reference ABI. Reuse `terminal_psi_indexed_receivers` native tests,
  including `publication::` and `primitive_stores::`, rather than a
  store-specific emitter. Cross-emission is not matching-host execution.

  Remaining work includes early loan closure/restored-parent use, escaping
  carriers, dynamic indexes and reference-bearing projections that can be
  located without reading a stored pointer/descriptor. Computed IEEE stores need
  a real source-selected operation, result transport and ordinary store
  composition, not widened admission: Psi's
  `flow/terminal_unit/selected_ieee_float.rs` and Omega's shared graph/provider
  route must retain format, selected occurrence and result evidence. The removed
  Unit/FMA planner is not a dependency to rebuild.

  Acceptance: writes affect the original caller referent across calls and
  register/stack passing; reads through write-only access reject. Cover exact
  width/write coverage, untouched neighbors, runtime signed/Boolean/floating
  sources, restoration/return behavior, access substitution and independent
  artifact replay. Observe computed floating stores on the caller, not just
  checking or a copied frame home. Run both Linux target runtime legs when
  available and record unavailable hosts.

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

- **CALLBACK-PARAMETER-REQUIREMENT.** Checked admission of the nominal
  `where machine Selected satisfies Trait::requirement` binder is pinned by
  `tests/omega/pass/generics/nominal_machine_parameter_satisfaction_compile`
  and the `fail/generics/*nominal_binder*` canaries (structural coincidence,
  overloaded requirement, implicit selection). Remaining: a native run canary
  witnessing that the retained selected entry, envelope refinement, and call
  site reach the target entry recipe. Native legs depending on `build.omg`
  need private resolver storage; on Linux 5.15 `fchmod` on an `O_PATH`
  directory descriptor fails with `EBADF`, so witness on a 6.6+ kernel or
  fix the storage opener first.

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
  Landed at 4d45081529: `tests/omega/pass/memory/bump_allocator_canary` checks
  two coexisting allocations and full-return recomposition over a package
  `ExtentPartition` boundary (Linux x86-64,
  `OMEGA_PASS_CANARY_FILTER=memory/bump_allocator_canary cargo nextest run -p compiler --test canary_suite entry_and_abi::pass_canaries_compile`).
  Discovered contract gaps, recorded in the canary header: a record returned by
  an ordinary machine does not carry its fields' `in Granted` facts to the
  caller, an ordinary machine cannot restate a boundary's `separate` law as its
  own `ensures`, and multi-input recomposition is admitted only through one
  record parameter. Next acceptance: chain `allocate`/`reset` as ordinary
  package machines with conserved custody, then a `Vec<T>` over that chain.

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
  and token era, not row equality, authorize invocation. Selected rows now
  reject a realization that still retains an unresolved installation-bound
  requirement (`provider-planning` `plans.rs`); the component-contract fence
  waits on the `COMPONENT-SUBSTRATE` carrier.

## Parallel language and compiler lanes

- **CODEC-LINEAGE-CLOSURE.** Complete policy-selected historical compatibility
  over immutable ordinary data under [codec durability and historical lineages](wiki/spec/layouts/codecs.md).
  Owners: `validation/src/wire`, `build-time-evaluation/src/wire_plans.rs`,
  `compiler/src/pipeline/reporting/wire.rs`, and the ordinary
  `FormatMigration<Lineage, Old, New>` library requirement. Current compatibility
  demands and checked migration machines exist; historical dispatch and
  retirement obligations must join the exact selected policy, published shapes,
  and migration route. Nested `version` declarations are not that join.
  Generated record codecs currently cannot establish whole-record `where`,
  declared-property, lifetime, or sum obligations; keep those codec applications
  rejected while the ordinary declarations remain usable.

  Landed: `tests/omega/pass/wire/wire_compatibility_migration_across_shapes`
  and `fail/wire/wire_compatibility_migration_route_missing` witness that a
  `CompleteMigration` demand across differing eras is satisfied only by the
  bound `FormatMigration` machine (`canary_suite
  reports_and_capabilities::wire_compatibility_complete_migration_demand_needs_bound_lineage_route`,
  Linux). Open: retired-identity reuse rejection and policy-chosen era dispatch.

  Acceptance: an old/new ordinary declaration pair and explicitly selected
  checked migration satisfy the requesting channel/store policy; missing routes
  and retired-identity reuse reject where that policy requires them. Check era
  dispatch chosen by the policy without implicit declaration-order versioning.
  Preserve the distinction between current-shape codec roundtrip and historical
  migration, plus decoder rejection when full destination validity is unproved.

- **CASE-CONSTRAINTS.** Implement [case-local `where` constraints](wiki/spec/language/data_and_literals.md#case-constraints)
  for typed requests/IR and case-specific payload invariants. Psi owns parsing,
  resolved case contracts, generic substitution, construction checking,
  arm-local facts and coverage through checked/Terminal publication. Reuse
  ordinary equality, default-domain establishment and invariant-window checking;
  do not add a separate GADT representation family or hidden type packaging.
  Connect to the existing case/match lowering work below rather than introduce
  a source-shape-specific evaluator.

  Landed: case members parse `where` facts, carried on `DataVariant` through
  syntax, symbol-resolved, and typed trees; the selected case's facts fold at
  construction through the default-domain literal fold
  (`pass/dependent/case_where_bound_literal_proves`,
  `fail/dependent/case_where_{reversed,unproved}_bound_rejected`, Linux). Case
  constraints on generic data are refused
  (`fail/dependent/case_where_generic_data_unsupported`) until generic instance
  synthesis carries variant facts. Open: generic `T == i32` establishment,
  match-contributed case facts, coverage, and stale facts after payload
  writes/case replacement.

  Acceptance: source tests establish `Value<T>::Integer where T == i32` and
  `Boolean where T == bool`, and a generic payload-returning match checks without
  casts; wrong-index construction rejects. `Range(lo, hi) where lo <= hi` accepts
  proved construction and rejects reversed/unproved bounds. Exercise common
  constraints, proved impossible-case coverage versus unknown predicates, first-case
  zero gating for `Value<bool>`, generic establishment with and without sufficient
  assumptions, stale facts after payload writes/case replacement, and ordinary
  move/borrow/linear-payload controls. Valid cases must execute through ordinary
  lowering and preserve checked facts in independent replay. No unrestricted
  coverage search, specialization-only rescue, or conformance discovery.

- **MATCH-SELECTIVE-LOWERING.** Complete the
  [value-dispatch contract](wiki/spec/language/patterns.md) for owned/nonnumeric
  results with parameter/projected/borrowed/linear custody, structural/case/domain
  patterns and coverage. Mixed conditional transfers admit fresh scalar-case arms
  beside existing locals (`owned_match_mixed_values`); fresh record and call arms
  still need their own residual carrier. Remaining existing-local joins need
  interleaved live-root ordering and residual transport across authored states;
  preserve exact origins and actual death edges.
  Owners: `validation/src/expression_types/{match_dispatch,result_type}.rs`,
  checked scalar computation/result continuations, Terminal production and
  canonical package-review contract/index projection. Preserve a once-evaluated
  subject, ordered first match, branch-local execution and exact result owners;
  do not flatten conditional ownership into a statement-wide move roster.

  Selected-operator and semantic-domain result types need full instantiated
  identity; input predicates are not arithmetic result facts. Indexed
  predicate/theorem and package membership applications need exact static
  arguments. Qualified callable-entry signatures, predicate/routed membership
  and erasure require real transport rather than payload-only projection.
  **OPERATOR-MACHINE-SUPPLY** owns declared operator execution;
  **CRASH-CONTRACT** owns crash-qualified equality; numeric landing is in
  **STATE-LOCAL-VALUE-FRONTIER**.

  Acceptance: `checked-trees-to-lowered-psi --test value_dispatch`,
  `omega-native-differential-test --test scalar_case_results`, corresponding
  checker/interpreter controls, and the
  [float Match native customer](omega-rust/omega/compiler/compiler/float_realization.md#operation-and-control-custody)
  preserve effects, skipped trapping arms, overlapping patterns, complete
  coverage and independent replay. Keep `match_anonymous_result_landing` and
  `numeric_operand_destinations` as source probes. Wrong qualifications,
  ownership, selected result types and incompatible arms must reject before an
  outer bare-carrier cast; source validity alone is not native completion.

- **OPERATOR-MACHINE-SUPPLY.** Implement the
  [machine token-binding and executable-supply contract](wiki/spec/language/expressions.md#executable-supply)
  across Psi parsing, resolution, checked body construction, evaluation,
  lowering, Terminal codec/verifier/interpreter, and native call realization.
  Accept an optional fixed token after `machine`; remove the separate operator
  declaration introducer rather than retaining two permanent source forms.
  Ordinary direct declarations own checked machine bodies. Reuse ordinary body,
  state, contract, and call machinery instead of adding an operator evaluator
  or searching for a unique satisfier. Bodyless requirements retain explicit
  trait/provider supply; exact compiler primitives retain automatic catalog
  supply and proof/runtime eligibility. Migrate core/library/canary declarations,
  including tokenless compiler primitives to ordinary named machines and
  tokenless boundary requirements to the existing required-body form. Preserve
  exact semantic identities or reject stale schema artifacts explicitly; do
  not match a compiler primitive by leaf name or legacy declaration kind.

  Enforce closed-family semantic-home ownership and owner-local duplicate checks.
  Replace the legacy `u8::sum` declaration-plus-satisfier fixture in
  `tests/omega/pass/expressions/declared_operator_match_result/main.omg` with a
  test-owned type/domain and a declaration-owned body returning `u64`.
  Keep the selected-call join in
  `typed-trees-to-checked-trees/src/values/scalar/computations.rs`,
  `computations/integers.rs`, and
  `checked-trees-to-lowered-psi/src/scalar_source_custody` compositional.

  Acceptance: wrapped 250 + 10 yields 260u64 in the selected true Match arm,
  the false arm yields 1 without invoking the operator, and independent Terminal
  replay and native execution agree. Cover token and named calls, generic and
  stateful bodies, once-only ordered operands, private helpers behind a public
  declaration, qualifiers and ordinary contract rejection. Missing body,
  bodyless-plus-satisfier, foreign primitive-family injection, duplicate owner
  shapes, and forged compiler primitive identity reject. Unrelated imports
  cannot change selection or cause a collision. Trait conformance selection,
  target-default/overridden float provider execution, and canonical compiler
  float-meaning evaluation retain their separate supply routes. Unsupported
  migration/execution paths must fail closed, not fall back to builtin arithmetic.

- **MODULE-NAMESPACE-RESOLUTION.** Finish the
  [module/name contract](wiki/spec/language/modules.md) in
  `syntax-trees-to-symbol-resolved-trees/src/module_normalization.rs`,
  `build-time-evaluation/src/const_initializers.rs`, and the shared generic
  evaluator. Remaining forms include aggregate-producing initializer expressions,
  open-template indices, specialized module templates,
  foreign/generic constant attachments, trait defaults, operator homes and
  qualified case membership in declared-domain proof facts. Preserve exact lexical/package selection before
  evaluation, per-use exposure under specialization and owner-local imports.
  The [source pipeline map](omega-rust/psi/pipeline/README.md#resolution-and-closed-instance-normalization)
  owns the current probes.

  Complete machine-call, aggregate-producing, constrained/target-dependent and floating/NaN
  declaration evaluation, including unused initializers. Calls need full
  invocation admission and floating identities need determined bits.
  `const_generic_expressions/value/match_dispatch.rs` still needs nonconstant
  divisor integrality beyond singleton sign intervals, nonzero proofs beyond
  retained lattice gaps, and exact fractional-warning evidence for
  independent dispatch operands; extend rational bounds and correlations, not
  Cartesian arm enumeration or evaluation of skipped subjects. Preserve exact
  selected operators (**OPERATOR-MACHINE-SUPPLY**) and proof arguments.

  General array-value execution remains a dependency, not namespace fallback:
  dynamic/nonliteral selectors, borrowed projections/slices, array-producing
  cycles, structural block parameters, boundary results and indirect
  arguments/results need real payload/storage and view evidence. Extend the
  shared evaluation sequence in the
  [Terminal production map](omega-rust/psi/compiler/terminal-production/README.md)
  and `lowering/control_flow/aggregate_results.rs`; selected aggregate homes
  cannot stand for unevaluated values. Empty values retain carrier/dimensions;
  a known slice length cannot erase view formation or bounds. Share state/call
  transport with **STATE-LOCAL-VALUE-FRONTIER**.

  Acceptance: `compiler --test module_machine_indices` (`nominal::` and
  `value_dispatch`), `terminal-psi-to-abstract-operations --test scalar_array_construction`,
  and native `scalar_array_results` exercise exact source selection through
  independent artifacts and matching-host execution. Preserve qualified
  declarations/constants, `match_constant_indices`, `nominal_constant_bodies`
  and `module_array_constant_indices` customers. Foreign exposure follows
  [file-local imports](wiki/spec/language/modules.md#import-scope-and-exposure):
  broad imports expose directly declared public domains only; narrow imports
  select exact declarations. Same-leaf competitors, private/transitive exposure,
  invalid unused initializers and unproved indexing reject. Keep the
  `runtime_aggregate_index`/`runtime_fixed_array_index` negative controls until
  their actual materialization obligations are met.

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

  Resume evidence (8c0d4dbb45, Linux x86-64, debug `omega --check` on a
  std-free probe): `machine Main::prefix_count<Count: u32>(&self, base: u32)
  -> u32 { Count }` rejects in the parser with "expected `satisfies`, found
  punctuation `>`", because `tokens-to-syntax-trees/src/parser/data.rs`
  reads `<Name: X ...>` only as the `T: Subject satisfies Carrier`
  conformance-binder form; the same program under `<const Count: u32>` with a
  static `<3>` argument compiles. Every representation carries one value-binder
  kind, `TypeParameterKind::Const { type_reference }` (syntax, symbol-resolved
  and typed trees), and conformance, callable-shape and review-evidence
  signature comparisons key on that variant, so a parser-only spelling that
  reuses `Const` would silently strengthen the public binder to `const`.
  Next acceptance: add the runtime-capable staging to the value-binder kind in
  all three representations and their identity/snapshot readers (about 55
  Rust files match `TypeParameterKind::Const {` without `..`), parse
  `<Count: u32>` into it, route static arguments through the existing const
  specialization, and reject a runtime argument with a diagnostic naming the
  missing dynamic realization; pin `tests/omega/pass/generics/` and
  `tests/omega/fail/generics/` fixtures for both. The one-body dynamic
  realization follows that representation landing.

- **STRUCTURAL-GENERIC-MATCHING.** Implement
  [static type equality](wiki/spec/language/generics.md#static-type-equality),
  [structural equations](wiki/spec/language/generics.md#structural-type-equations-and-inference),
  and [canonical ranges](wiki/spec/language/generics.md#canonical-integer-range-matching)
  for bounded containers deriving static backing from a declared length type.
  Psi parser/type-role resolution, generic-data substitution, machine inference,
  canonical type identity, and checked branch facts own the route; static
  evaluation/layout and artifact readers must use the same normalizer. Inference in
  `typed-trees-to-checked-trees/src/monomorphization/range_arguments.rs` must extend
  to named computations, owner-sensitive typed operations and open symbolic
  endpoints through shared semantic evaluation; do not use the i64 compatibility
  interval evaluator as canonical type identity.
  Keep `tests/omega/pass/generics/declared_range_endpoint_inference/main.omg`
  as the checked/Terminal call regression; the source pipeline map retains its CLI command.
  Closed free and type-qualified endpoints (`u64[0..=limit()]`
  and `u64[0..=Limits::capacity(256)]`) fold by resolved machine identity
  before checking in `build-time-evaluation/src/range_endpoints.rs` through the
  shared admission plan. Resolved calls admit closed integer arguments into
  exact builtin integer parameters with closed range refinements, preserving
  carrier checks, selection authority and fractional warnings. Nested calls and surrounding
  integer arithmetic retain exact result carriers and original call selections;
  failed evaluation restores temporary substitutions. Input/result range checks
  use exact concrete values; computed signature bounds are invocation dependencies.
  Noninteger arguments/results, nominal/policy qualifications, generic calls,
  owner-sensitive operations and open symbolic endpoints remain. Record and case-payload endpoint calls resolve in
  their declaration scope before folding; local bounded record construction and
  field reads retain range obligations through canonical Terminal execution.
  Generic-data range arguments retain structured interval observations from
  `build-time-evaluation/src/range_arguments.rs` and independently replay equality
  after complete typing; substitution retains the original constrained argument.
  Finish open/template-dependent and machine-computed argument ranges, type
  equations and omitted data binders without a source-display identity key.
  The fixture's `generic_forwarded_bound` checks and executes at build time but
  Terminal still rejects the missing checked scalar control plan. Nongeneric
  controls reproduce nested record projection and local record mutation gaps;
  whole local copies and full-width bounded fields already execute. Connect the
  remaining shared storage operations, then require the unchanged function and
  copy-after-mutation controls to execute from canonical Terminal bytes; do not
  invent generic-specific plans.

  Extend remaining named computations through build-time admission, using
  `typed-trees/src/typed_trees/type_system/closed_numeric.rs` and its existing
  identity/substitution consumers for the resulting endpoints. Preserve
  authored endpoint landing, exact binder identity and proof-integer exclusive
  normalization; do not introduce another arithmetic evaluator or infer layout
  from flow bounds. The source
  pipeline map records the maintained customer and remaining native boundary.
  Its hosted `Main::main` now produces Terminal; native publication reaches the
  retained receiver-provisioning rejection. Resume `ENTRY-CONTENT-ROOTS` for the
  physical bridge, then require this unchanged program to exit 70. The focused
  Resume evidence at `4641c0f055`: `cargo nextest run -p compiler --test
  canary_suite -E 'test(declared_range_inference)' --no-fail-fast` exercises the
  reviewed fixture and this boundary on macOS AArch64; it does not establish
  native execution.

  Acceptance: TinyBytes' `Length == u64[0..=Capacity]` binds omitted Capacity from
  its supplied type before layout; inclusive/exclusive equivalent intervals
  select identical static capacity without runtime arithmetic overflow. Primitive
  equality and its static branches check all admitted alternatives. Repeat and
  explicit binder conflicts, absent/ambiguous endpoints, occurs cycles, and
  type/value-kind mismatch reject. Extend the existing declared range-only call
  inference while keeping explicit larger compatible bounds distinct from exact
  type equations. Local flow narrowing cannot alter inferred
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
  `checked-trees-to-lowered-psi/src/attached_unit/call_closure.rs`. Its source
  dependencies include nested structural sum construction/transport
  (`UnitResult::Error` carries `ErrorKind`) and whole nominal receiver
  replacement (`self.unit_result = ...`). Resolve these through shared
  state/value planning before expecting this fixture to emit; a primitive
  field store or a fresh call-result home cannot substitute receiver replacement.

  Pause further isolated native prerequisite milestones for this fixture.
  Resume execution work with a plan covering its actual transitive closure:
  nested construction and extraction, borrowed case observation, and whole
  nominal replacement, including match-produced field assignments. Reuse
  existing recursive layouts and referent identity; do not erase structural
  operand custody or replace borrowed storage with snapshots. This is an
  implementation scope pause, not a language-design blocker.

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
  `checks/termination/progress/{origins.rs,lineage.rs}`. Captured record and
  array constructors now arrive from the selected field or element operand
  (`flow/value_origins.rs`); owned helper results such as
  `context.scheduler = pick(replacement)` still fail the requires proof before
  any premise is reconstructed. A mutated aggregate
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
  Resume at the checked/Terminal representation seam, not another evaluator
  source-shape gate: realize projected owned reference leaves with residual
  carrier cleanup, then nested result operands with their recursive loan custody.
  Whole owned record ingress and forwarding are available: at `5374ab3198`,
  `cargo nextest run -p checked-trees-to-lowered-psi --test reference_result_source --no-fail-fast`
  (macOS ARM64, `RUST_MIN_STACK=33554432`) checks canonical encoding, independent
  verification, and fuel-stepped execution of `forward(input)` followed by
  `replace(held.body)`, preserving the transferred leaf and restoring the caller's
  original backing after cleanup. `select(value: View) -> &mut i32 { value.body }`
  must move the selected permission and dispose the remainder, not create a
  reborrow whose parent dies at return.
  This is Terminal acceptance, not native acceptance. Reuse
  `validation/src/reference_result_custody.rs` for ordinary completion and
  independent source replay; preserve conservative lifetime unions when extending
  exact runtime origins beyond the current whole-record route. Transfer existing
  permissions through owned call/edge/result moves and residual cleanup;
  `EstablishReference` creates a child loan and cannot substitute for moving an
  existing leaf. Keep carrier location distinct from loan occurrence/parent,
  relocate runtime descriptors without copying referents, and reject nested
  reference host interfaces until their custody exists. Reuse existing typed
  projection/result maps; structural-element array construction remains a
  further dependency beyond the record route. The full-checking rejection
  controls in `typed-trees-to-checked-trees/src/tests/borrow/carrier_results.rs`
  cover nested `select(forward_outer(...).inner)` and
  `select(forward_array(...)[0])` calls, not the simpler `make_view` binding.
  Keep those controls until their corresponding unchanged source cases execute
  from encoded Terminal evidence, including projected moves and array ingress.

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
  Unknown scalar parameters and locals now bound conversion results by their
  primitive carrier (`values/bounds/sources.rs`; regression
  `unknown_scalar_inputs_bound_conversion_results_by_their_carrier`, Linux
  `cargo nextest run -p typed-trees-to-checked-trees`). Indexed text writers
  still need numeric conversion result evidence for effectful nested
  arguments, nonlocal storage, and remaining cast policies beyond selected
  normal-return scalar snapshots consumed by
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

- **STATE-LOCAL-VALUE-FRONTIER.** Complete ordinary evaluation/value transport
  in Psi argument normalization, checked scalar computations, call/result plans
  and Terminal production. Remaining operands include dynamic/borrowed/projected
  storage, effectful state arguments/returns, wider structural returned calls and
  mixed structural/scalar signatures, including boundary consumers. Materialize
  each value and activate its staged loan at the authored evaluation point.
  Retain exact result owners for shared/mutable/write-only temporary borrows,
  multiple argument producers, self consumers and projected claims; **CML4**
  owns residual cleanup. Replace remaining flat guarded-call hoisting with the
  same evaluation graph, not another source-order family.

  Extend the ordinary producer/consumer join in
  `effects/structural_callback_reach` to extracted projections, freshly
  established returned claims and claims from distinct owned inputs, including
  mixed scalar/structural operands. Returning a whole fixed array with several
  indexed claims is the regression baseline in `projected.omg`;
  matching claim identities cannot substitute for checked content guarantees.

  Complete caller-specific saved-argument and result facts: nonliteral contract
  arithmetic, borrowed collection lengths, dependent/public-trait results and
  subslice bounds need exact entry observations and substitutions. Mutable
  formals/storage must distinguish their incoming value from subsequent writes.
  **CRASH-CONTRACT** shares the capture path; case-qualified, indexed, generic,
  reference-valued and floating entry predicates need exact identities/totality.
  Unchanged entry observations may justify published routes; later writes and
  current body facts may not. Ranked-loop crash guards require independently
  checked all-path invariants, never first-pass facts ignoring backedges.

  Finish [exact anonymous division/landing](wiki/language_guide/chapter_5_expressions_evaluation.md#exact-anonymous-division-and-landing)
  across generic/evidence-adapted and boundary calls, aggregate/parameter/constant
  destinations, numeric policies, floats and proof consumers. Preserve result
  carrier/policy custody, exact rational intermediates and warning origins
  through suppression/reporting; coordinate selected result types with
  **MATCH-SELECTIVE-LOWERING**. Acceptance: `7 / 2 * 2` is 7 with a warning,
  `7 / 2` cannot land in an integer, and typed integer division truncates.
  `(4097 / 4096) * 4096` is 4097 with a warning; its `4097u32` form is 4096.

  Complete [typed quotient/remainder](wiki/language_guide/chapter_5_expressions_evaluation.md#typed-integer-quotient-and-remainder)
  in resolution, selected constant execution and symbolic proof replay.
  `generic_data/const_evaluation/` and
  `build-time-evaluation/src/admission/selection_authority.rs` must retain
  authored selection across helper calls instead of folding builtin meaning.
  Use the `authored_const_operator_requires_selection` and
  `authored_const_call_operator_requires_selection` controls: next positive
  acceptance evaluates the selected zero-returning provider to `Buffer<0>`,
  while unrelated declarations leave builtin arithmetic unchanged.
  **OPERATOR-MACHINE-SUPPLY** owns executable supply. Nonconstant proof `Int`
  terms need independent evidence beyond source entailment in
  `validation/src/contract_entailment/arithmetic_judgment.rs`.
  Positive/negative dividend/divisor combinations satisfy the paired integer
  law; zero divisors reject and exact anonymous division remains unchanged.

  Overall acceptance: selected arguments execute left-to-right once; skipped
  calls never execute; serialized/replayed guards and saved values agree with
  native execution. Explicit renamed state transfers preserve contracts,
  selected fields, ownership and cleanup without requiring physical copies;
  implicit cross-state use rejects. Longer dispatches, mixed state signatures,
  borrowed loop formals and mutable scalar carriers use ordinary joins.
  Stale writes, mismatched result origins and wrong normal-exit guarantees
  reject; callee-local IDs and rereads cannot replace captured values.

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

- **STRICT-FLOAT-RANGES.** Implement exclusive floating range evidence in
  validation's type-reference/cast readers, proof constraints and retained
  entry predicates. Preserve the authored endpoint and IEEE order; integer
  predecessor arithmetic is not floating range normalization. Accept values
  below the endpoint and reject the endpoint itself and NaN, including
  call/store delivery and independent replay. See
  [numeric qualifications](wiki/spec/language/numeric_values.md#value-qualifications-and-policy-adapters).
  Resume: the declaration, proof (`ProofConstraint::FloatRange::maximum_inclusive`
  in `omega-rust/psi/semantics/proof/src/obligations.rs`) and cast readers now
  retain the strict endpoint for call/store/return delivery
  (`tests/omega/{pass,fail}/float/exclusive_float_range_*`). Open: retained
  entry predicates and independent Terminal Psi replay; no float range
  predicate exists in checked-trees/lowered-psi/terminal-verifier yet.

- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Restore ordinary native
  descriptor invocation and forwarding, beginning with a non-entry helper that
  receives one borrowed two-word descriptor, forwards it once, invokes a
  requirement, and uses its result across computations and branches. Target-only
  consolidation is paused: two composition milestones left this native customer
  unsupported. The missing dependency is an ordinary indirect-call operand and
  its ABI, clobber, effect, and reach contract, not another whole-body recognizer.
  Owners: abstract-to-target signature/graph lowering; legalized/selected call
  representations and target-to-selected replay; ISA selected encoding/decoding
  and post-allocation emission; native-artifact/image table and relocation custody.
  Acceptance: a source-rooted closed-conformance native differential fixture
  publishes and independently replays Linux x86-64/AArch64, executes on matching
  hosts, selects distinct table implementations at runtime, and preserves the
  original referent and surrounding calculations. Reject substituted instance,
  table, slot, ABI, access, and code-span custody. Delete the superseded Unit/scalar
  parameter recognizers with that closure; do not substitute raw function pointers
  or unproved devirtualization. Receiver-backed executable entry provisioning
  remains a separate dependency for the existing receiver-entry canary.

- **TARGET-SEMANTIC-APPLICATIONS.** Complete typed target observations,
  hermetic const evaluation, and [selected realization coverage](wiki/spec/terminal-psi/boundary_calls.md#operator-applications-and-physical-children). Finish
  ordinary selected provider-body execution and earlier generic-application
  evaluation through the [semantic evaluation owner](omega-rust/psi/semantics/build-time-evaluation/README.md#semantic-admission-boundary),
  preserving exact provider selection and independently checked result custody.
  Complete the portable target capsule and its application identity; a checked
  source fold alone does not establish cross-artifact closure. Finish
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
  Join the [scoped build work](#scoped-build-execution): checkpoints and generated
  handoffs also bind dependency purpose and execution profile, and publication
  waits for every required output plus final product checking. Preserve the
  authored/generated boundary; no own-build final-component query or hidden
  post-compilation callback.

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
  while keeping deployment/update policy in runtime packages or Cathedral. Componentization must
  bind exact imports, exports, services, mappings, stack demand, leases, and
  installed provider closure under the
  [component publication contract](wiki/spec/build/component_publication.md).
  Until that carrier is complete, every
  `Independent` selection fails at one explicit fence.

  Expose the [verified-description consumer](wiki/spec/build/component_publication.md#verified-component-descriptions)
  from the existing verifier for independent admission/replacement and topology.
  First inventory actual producer/replay coverage; implement missing complete
  facts in their Psi, component, or provider owners, not a second topology census.
  Acceptance: an independent source-free consumer checks subject/profile/schema,
  all entries and outgoing authority, custody and inseparable assumptions; corrupt,
  omitted, early-frontier and forged-complete descriptions reject. Include startup,
  callbacks, timers, cleanup and retained providers. Reading descriptions grants no
  callable authority; installation-dependent facts remain obligations and require
  fresh per-occurrence resource/profile admission.

- **FFIVAL.** After the generic callback/runtime path closes, run the Windows
  `user32` boundary-coherence canary with no raw function pointer or Win32-only
  compiler escape.

- **WIRE-RUNTIME-AND-INSTALLATION.** Complete reusable artifact validation,
  consumed placement authority, W^X/coherence, physical invocation, and
  uninstall/replacement joins. Keep arbitrary runtime bytes-to-code, JIT, and
  raw executable addresses unsupported.

  Close imported-image placement before admitting imported installed runnables.
  Source inspection at `0fef6890cb`: `image-emission/src/installed_artifact.rs`
  projects only compiler-authored text/data prefixes, excluding image-writer
  import thunks and binding slots. The Mach-O import regression in
  `compiler/tests/source_evaluated_native_realization.rs` checks installation
  records, not complete installed-code custody. Bind every exercised thunk,
  slot, relocation destination, and loader/provider lifetime to real placement;
  reject omitted regions. A matching prefix and a resolver returning an
  uninstalled thunk address cannot establish that closure. Acceptance: a
  source-imported component reaches installed publication with complete custody,
  and missing or substituted thunk/slot placement rejects independently.

## Omega-written compiler (after Rust completion)

Finish the Rust [completion contract](wiki/drafts/rust_compiler_completion.md)
before starting the product-language migration. Rust can remain a differential
implementation afterward, but neither Rust agreement nor Rust-specific machinery
is bootstrap authority. Bootstrap construction stays on `TASKS_BOOTSTRAP.md`.

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

## Platform-gated verification

- Run Linux host/time/filesystem and `IntegerAt` runtime paths on AArch64;
  cross-target compilation is not runtime verification.
- Build and run the Windows GUI callback canary only through the generic ENT4
  path.
- Keep unavailable hosts structurally tested and report the missing runtime leg
  explicitly.
