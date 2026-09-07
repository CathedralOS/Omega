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

## Immediate product closure

These are the next product-level priorities for the maintained Rust
implementation. They take precedence over adding another evidence carrier that
has no exercising program. The finite definition of Rust-product completion is
the [Rust Compiler Completion Contract](wiki/releases/rust_compiler_completion_contract.md).

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
  [settled publication contract](wiki/design_briefs/macos_application_publication.md)
  in build evaluation/realization inputs, Mach-O signing, command publication,
  and compilation reports. Retire the unproduced bundle-copy slot/helpers/tests
  and stale Rust module notes without removing flat/report validation or changing
  flat v1 digests. Specify the identifier field in ordinary build vocabulary,
  preserve portable GUI intent separately from PE integers, and bind native
  signing identity before emission. Publish one whole validated `.app` with
  distinct checked package-root and inner-executable accessors.

  Acceptance: the brief's stage-requiredness, deterministic bytes, cross-invocation,
  tampering, partial-output, and flat-regression controls pass; GUI samples carry
  authored identifiers and consumers use reported paths. Validate the procedural
  GUI cohort on macOS, recording unavailable-host coverage explicitly. Resource
  inclusion/lookup for `image_viewer` remains outside v1; do not claim Finder
  runtime coverage for it or silently change its working directory.

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
  through collections also need `NOMINAL-FIELD-FLOW` below; their default-field
  obligations must be proved, not bypassed to restore sample acceptance.

  `text_samples_compile_from_authored_program_entry_bindings` is red for
  `caesar_cipher` and `format_number` alone. Both build a `[u8; N] in Utf8`
  carrier out of raw `u8` element stores (`self.out[self.i] = self.ch as u8`,
  `self.buffer[0] = self.tens_byte`), so
  `typed-trees-to-checked-trees/src/checks/contracts/nominal_inputs.rs`
  rejects every later transition with `cannot prove default-domain field
  requirement ... requires [u8; N]::Utf8`. They are the indexed text writers
  `NOMINAL-FIELD-FLOW` already owns. Its whole-buffer half is live: a per-byte
  carrier class now survives an indexed store of a proved in-class byte. What
  still rejects in both is the replacement byte: `format_number` stores a
  `narrow_i32_to_u8_trapping` result and `caesar_cipher` a wrapping add over an
  element read, and neither shape proves a class. Close them there, not through
  a weakened field declaration. The `stdin_*` and `gui` cohorts pass.

  `samples_with_documented_exit_run_correctly` is separately red for all 136
  documented-exit samples; before the borrowed-self retry below, 86 failed
  native Terminal production in `checked-trees-to-lowered-psi` and the rest
  earlier under `NOMINAL-FIELD-FLOW`, a split not yet re-measured. 119 of the
  136 call `Console::read_line(&mut self.pause)` from an attached
  `Main::main(&mut self)`. That argument now has a root: when the
  ambient attachment cannot plan the body, `build_checked_machine`
  (`typed-trees-to-checked-trees/src/flow/terminal_unit/control.rs`)
  retries with the borrowed `self` retained as structural parameter 0 carrying
  the reference's access, beside the provider-specialized fields. The retry is
  transitional; retain a borrowed `self` unconditionally once the entry bridge
  passes the `ProgramEntry` loan as structural parameter 0, under
  `ENTRY-CONTENT-ROOTS` and
  `INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION` in P1. `cli_mvp` now stops at
  `checked Unit provider candidate has no complete terminal body plan`: the
  std adapter `ConsoleNativeProvider::write_line(console: Console, text: &[u8])`
  has no checked Unit plan because the fused-service signature in
  `structural_signature_with_affine_pair` admits only the service and scalar
  parameters, and its body `console_write_bytes` is a multi-state loop
  `terminates by bytes -> Slice::Length`. That adapter closure is the next
  slice, and it is independent of `read_line`: the sample without that call
  stops at the same candidate. Behind it, the projected `&mut self.pause`
  argument lowers to Terminal but `terminal-verifier` rejects it with
  `InvalidStructuralArgumentPath`: `resolve_structural_path` walks only
  `Structural` fields and `pause` is a `ByteSequence(BoundedOwned)` field; an
  owned `self` stops at the same verifier site. General receiver-store
  sequences still need planning: `win64_direct_aggregate_import_compile`
  combines scalar writes, an aggregate replacement, and a foreign-result
  assignment. Extend the checked Unit statement sequence without dropping
  any write or its exact frame; the bounded single-scalar-store form is not
  a plan for that complete body.

  Byte-carrier boundary forwarding must retain the exact source place, path,
  capacity, and live-length writeback separately from the borrowed-view
  parameter. `terminal-interpreter::resolve_structural_arguments` and Omega's
  `abstract-operations-to-target-operations/src/lowering/structural_layout.rs`
  also reject byte-field projections; accepting them in the verifier alone
  does not implement the call. Bounded inline storage and borrowed descriptors
  have different layouts. Add runtime buffer support to the interpreter and
  an admitted `read_line` realization through native emission and installation
  replay; each target's `console_impl.omg` declares a bodyless intrinsic.
  Close this slice with the existing carrier round-trip and sequential-read
  native canaries, preserving capacity, overwrite, access, and alias checks.

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
  `typed-trees-to-checked-trees/src/checks/termination/ranking/`.
  Named-state and machine-call-component ranges need exact arrival mappings
  for ranked subjects and pinned endpoints. Mutable premises need live
  write-frame evidence.
  Custom scalar views and slice-length ranges need their produced-rank facts.
  These are implementation gaps, not grounds to weaken the range obligation.

  Acceptance: named-state and call-component rank ranges accept proved
  constraints while changed endpoints and intervening writes invalidate their
  premises. Preserve the private-witness/public-guarantee split described in
  chapter 3 and the
  [termination brief](wiki/design_briefs/termination_ranking_and_progress.md).

`omega-rust/` remains the production implementation until that contract
closes. It may remain afterward as a differential implementation while it finds
real bugs, but Rust agreement is not bootstrap authority and Rust-specific
machinery must not migrate into the Omega-written compiler source.

## P1 - Authority, roots, and entry

Owners include
`wiki/design_briefs/authority_values_and_boundary_evidence.md` and
`wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md`.

- **ENTRY-CONTENT-ROOTS.** Connect the generated target entry stub to the exact
  selected semantic continuation, consume the activation loan, and retain
  generated-bridge evidence without inventing roots. Migrate deployable
  fixtures to authored target-owned `ProgramEntry`; targetless checks select
  none. Acceptance is native execution from an authored entry with exact
  symbol/text/continuation replay and mutation failures for redirected or
  duplicated identities.

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

- **UEFI-OS-HANDOFF.** Implement the nonreturning custody transfer from Boot
  Services to the selected OS entry. The bounded memory-map/key retry loop must
  return all custody on stale-key failure and consume boot-scoped services only
  on success. Acceptance includes stale-key, exhaustion, lost-custody,
  post-exit provider-use, and successful handoff canaries.

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
  installation. The detailed vocabulary lives in
  `wiki/architecture/pipeline/terminal_psi.md`; this task records no operation
  ledger. Acceptance: source and producer state can be discarded before an
  independent verifier reconstructs every obligation and executes or lowers
  the same artifact.

  Native/external execution, ABI, fixed native resource, and final-code replay
  claims additionally require exact final-realization evidence. Preserve
  complete standalone products without hidden `CheckedCompilation` state;
  checked API/capability results and opaque executable supply cannot establish
  those claims. Physical optimization replay belongs to
  `TRANSLATION-VALIDATION` in `TASKS_OPTIMIZER.md`.

- **CRASH-CONTRACT.** Complete invocation-specific crash obligations through
  nested structural paths, calls, cycles, and imported effects. Crash is an
  explicit observable outcome with a semantic cause; it is never represented
  as an ordinary return, missing cleanup, or backend trap inferred after the
  fact. Acceptance: safe calls discharge every route and mutations to guards,
  substitutions, or sites reject.

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
  checks the supplied route. Bootstrap discharge remains open under
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

- **OPAQUE-BY-VALUE-BOUNDARY-ABI.** Complete D26 representation agreement at
  independently compiled by-value exchanges. Rejoin consumer demands to exact
  producer opaque/conformance/carrier declarations and immutable source;
  enforce strong selected-application equality at actual exchanges. Finish
  physical movement and lifecycle planning, including D44 transitive
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

  Run literal indexed write-only receiver caller-observation coverage on both
  Linux hosts, retaining original referent identity and exclusive access through
  register- and stack-passed pointers. Extend Terminal receiver
  production to dynamic indexes and retained local aliases; checked admission
  alone does not supply their portable address and lifetime evidence.
  Extend non-observing receiver admission to
  reference-bearing projections only where locating the receiver does not read
  a stored pointer or descriptor. Keep generic, sum, and dynamic dispatch tied
  to their corresponding shape/admission work; do not treat a receiver as
  readable merely to dispatch it.

  Native referent identity follows `STRUCTURAL-BORROW-IDENTITY` below; it is
  not an owner-policy blocker. Preserve write-only non-observation independently
  of the shared physical reference ABI.

- **STRUCTURAL-BORROW-IDENTITY.** Enforce the settled
  [structural borrow identity contract](wiki/design_briefs/core_multiplicity_and_linearity.md#structural-borrow-identity-at-native-calls)
  through structural-signature construction, call argument preparation, and
  native validation/replay. All borrowed access modes use one classifier and
  the existing `BorrowedReference` tag; only owned inputs use value-shape ABI
  selection. Structural producers must require the derived signature result,
  while generic scalar ABI construction stays available. Acceptance: caller-
  visible writes, forwarded references, legal synchronized shared observations,
  write-only non-reading, and register/stack pointer passing work on both Linux
  targets. Independently formed or substituted access/shape/placement pairs
  reject; replace the hand-built mutable/direct-copy fixture with a reference
  case or a rejection control. Do not claim copy equivalence merely because a
  following callee sees the staged write.
  Extend ranked checked-source structural export to receivers containing
  primitive arrays; a fixed `[u64; 3]` field currently hits the attached-Unit-only
  primitive-root fence. Acceptance: the canonical ranked artifact retains the
  array referent and its reference ABI without requiring a record wrapper.

- **BORROW-PROOF-CONVERGENCE.** Make ordinary borrow checking proof-producing
  without allowing propositions to create or amplify authority. Extend symbolic
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
  selected through exact conformances and validated layout paths. Private
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
  and reset only after full return. Use it to discover the real `Vec<T>`
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

- **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW.** Finish consumer permission rows and
  exact target-mechanism classification under the settled
  [filesystem control/lifecycle policy](wiki/design_briefs/effects_authority_and_observation.md#portable-filesystem-control-and-lifecycle-authority).
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
  callees with nested calls or mutable storage and unsupported cast policies
  beyond the selected immutable-local scalar snapshots consumed by
  `typed-trees-to-checked-trees/src/flow/transfers/byte_sequences.rs`. `format_number`'s
  `narrow_i32_to_u8_trapping` result and `print_squares`'s
  `narrow_u32_to_u8_wrapping` result publish no result range, and the checker
  derives none from their `trap_if` calls, so neither byte proves a class.
  `print_squares` and `caesar_cipher` additionally need the class to survive
  loop state edges. An ASCII byte can corrupt an arbitrary Utf8 buffer, so Utf8
  membership alone can never justify the preservation rule. Acceptance:
  `text_samples_compile_from_authored_program_entry_bindings` is green for
  `format_number` and `caesar_cipher`.

- **CML4.** Complete `EdgeCleanupPlan` after outgoing materialization and
  transfer commitment, including structural sums, nested projections, cycles,
  calls, and partial initialization. Cleanup follows reverse establishment and
  exact residual custody; trap/abort edges clean nothing. Acceptance: no affine
  occurrence disappears, duplicates, or is cleaned after transfer.
  Extend checked-source provider discovery and Omega installation-call admission
  to structural-result candidates, retaining exact boundary/candidate result
  signatures and the caller's actual operation-result identity. Implement native
  boundary call-result residual cleanup, including result homes
  and projected copies;
  whole-result disposal does not cover a projected result's residuals.
  Extend source production to anonymous
  projected helper-result operands. Extend the
  type-directed record/array complement in
  [Terminal Psi](wiki/architecture/pipeline/terminal_psi.md) to construction-local
  roots and mixed live-root schedules, preserving maximal untouched subtrees,
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
  Extend result operands to mutable/write-only borrows, anonymous shared borrows,
  projected routes, and self consumers without losing their exact result owner.
  Rejoin exact source/evaluation custody and remove only the cleanup transferred
  by the call; linear structural-result claims need their owning result plan.
  Complete mixed-signature runtime requirement transport for call-bearing
  arguments, remaining computation kinds, and mutable value snapshots.
  Retain evaluated arguments and prove
  computed-argument routes against caller formal ceilings; do not reread caller
  storage or retain callee-local IDs.
  Complete nonliteral contract arithmetic and callee-result bounds requiring
  caller-specific snapshots beyond immutable scalar formal comparisons; carry
  those facts into nested exact-cast obligations without rereading arguments.
  Retire the remaining flat guarded-argument call hoisting once these paths use
  the same evaluation graph. Owning area: argument normalization and checked scalar
  computation lowering. Acceptance: selected arguments
  execute left-to-right once, skipped calls never execute, and dynamic RHS
  calls serialize, independently verify, and execute with their exact guards.
  Extend guarded scalar control to longer dispatches. Complete
  anonymous integer landing, width custody, and warnings for generic/evidence-adapted
  calls, boundary calls, and aggregate-element destinations, plus the remaining
  numeric operator/policy surface, so proof and execution
  consume the same values without rereading changed operands.
  Extend mutable owned parameter execution to the remaining scalar carriers and
  service-reaching Unit bodies, with their current storage represented through
  effects and state transfers. Acceptance: the delivered value is materialized
  once, reassignment changes subsequent reads, and a final guarantee about that
  mutable formal cannot prove equality with its earlier argument value.
  Complete entry-requirement crash implication for structural predicates and
  numeric requirements: admit exact structural entry hypotheses and retain
  numeric evidence through its totality owner. Acceptance: structural entry
  hypotheses cover an unconditional callee under the matching crash guard,
  and numeric coverage retains its checked totality evidence. The strict
  Boolean-formal entry reader and structural crash-predicate owner must retain
  their exact namespace and totality checks; current body observations are not
  entry hypotheses.
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
  for the remaining parameter and aggregate destinations, numeric policies, float
  landing, constant arguments, and their proof consumers. Preserve exact rational
  intermediates until an actual landing boundary. Carry fractional-intermediate
  warnings through ordinary suppression and compiler reports, retaining authored
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
  Builtin proof `Int` must evaluate truncating division and dividend-sign
  remainder without machine-width bounds; preserve those semantics in retained
  proof terms and independent checking. Acceptance: positive/negative dividend and divisor
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
  subset. Acceptance: stack/control custody is never compiler-owned or lost
  across a suspension edge.

- **BLOCKEXEC.** Implement a package-level blocking executor with bounded
  queues, moved custody, linear completion claims, suspension, and provider
  selection. Hung-worker recovery requiring termination must use process
  isolation.

- **SELECTED-WITNESS-EVIDENCE.** Finish executable proof-output calls beyond
  the unconditional Unit/scalar lane, preserving selected proposition,
  producer, optional local term, and runtime-call linkage. Acceptance: omitted,
  reordered, substituted, or unlinked witnesses reject without turning proof
  terms into runtime values.

- **TRAIT-NAMED-WITNESS-CONTRACTS.** Carry named proof inputs/outputs through
  trait requirements, conformances, calls, Terminal Psi, and independent
  verification. Names are public proof API only where declared; satisfier-local
  aliases remain local.

- **QUOTIENT-THEOREM-LIFT.** Admit explicit representative operation,
  congruence theorem, and optional precondition transport for quotient-owned
  operations. No structural or effectful observer crosses the quotient unless
  its law is explicit and checked. Custody-bearing quotients remain fenced.

- **EVALUATED-FOREIGN-BINDINGS.** Replace string-backed import bootstrap with
  typed compile-time locator values for PE, versioned ELF, and Darwin/Mach-O.
  Carry normalized locator, evaluated plan, target applicability, and producer
  custody through provider selection and native emission. Raw foreign bytes are
  data, never Omega symbol names or ambient lookup authority.

  Extend D41 normalized-import evidence from fixed-width scalar calls to a
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
  hermetic const evaluation, and D29 selected realization coverage. Finish
  artifact-qualified symbolic substitution for separately compiled generics;
  recheck the reachable specialization's actual capability reach, proof
  obligations, target facts, and selected realization after closing every
  argument. Boundary-operator empty telescopes remain distinct from
  boundary-trait calls with no telescope. Acceptance: cross-artifact canaries
  preserve actual reach and transitive open obligations, reject stale or
  substituted applications, and grant no coverage to unresolved arguments.
  D32 physical-child binding belongs to `TRANSLATION-VALIDATION` in
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
  installed provider closure. Until that carrier is complete, every
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
