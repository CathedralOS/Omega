# Optimizer Tasks

Last audited: 2026-08-26.

This file is the execution queue for Omega optimization. The durable semantic
model, pass architecture, folder ownership, build hook, verification boundary,
and ML posture live in
[`optimizer_architecture.md`](wiki/design_briefs/optimizer_architecture.md).
The research motivation remains in
[`verified_gated_ml_optimizer.md`](wiki/design_briefs/verified_gated_ml_optimizer.md).

A task belongs here only when it has a concrete output and acceptance gate.
Completed implementation history should be removed rather than accumulated.

## Audited starting point

These facts constrain the work below.

- Psi owns Omega source processing and target-neutral meaning through immutable,
  canonical Terminal Psi. Omega begins at verified Terminal Psi and owns
  provider installation, optimization, ABI/storage realization, and native
  lowering.
- The current development compiler still routes ordinary builds through
  `CheckedTrees -> StateGraph -> ControlFlowPlan`. `StateGraph` and
  `ControlFlowPlan` retain checked-tree expression data and are explicitly
  transitional, so they are not foundations for a new optimizer.
- The clean lane already decodes and verifies Terminal Psi artifact sections
  before constructing `TerminalAbstractOperationPlan`. That representation has
  explicit blocks, typed values, operations, edges, places, claims, cleanup,
  boundary calls, and stable Psi provenance. It is the seed for the optimizer
  unit.
- The legacy state-value planner already contains expression substitution,
  exact integer/float folding, Boolean simplification, and guarded helper
  expansion. This is useful behavior to port and test, not a durable pass
  framework: it consumes `CheckedTrees`, has local fuel/depth caps, and is
  interwoven with lowering.
- `omega-target-operations-to-assigned-target-operations` currently assigns
  computed values by cycling through six x86-64 or nine AArch64 scratch
  registers. It has no general liveness, interference, spilling, splitting,
  coalescing, or frame allocation. The clean Terminal assignment lane handles
  bounded forms but is also not a general allocator.
- `omega-register-model` now owns separate target-neutral declarative physical-
  register and instruction-constraint vocabularies with total structural
  validators. `omega-regalloc` now consumes the opaque validated selected CFG
  for bounded liveness, ranges, candidate legality, the first strict
  transition-free physical-home assignment, and a separately validated local
  pressure-victim decision. It is not yet a general allocator: the latter is
  evidence about which value could leave the current homes, not authority to
  spill, reload, rematerialize, allocate a frame, or emit instructions.
  A separate validated allocator-availability artifact now narrows only
  unconstrained allocator candidates under either the named
  `AllEnvironmentAllocatableViewsV1` baseline or an explicit canonical view
  allowlist. It is compiler-internal policy, not target capability, a hardware
  reservation, a public build optimization selection, or an optimization
  level. Fixed ABI/operand views bypass the flexible allowlist but remain
  subject to reservation and architectural-state conflicts.
  Clean Terminal-ISA-owned x86-64 declarations split every GPR into exact
  byte/word/dword/qword storage lanes, retain non-allocatable high-byte views,
  model 32-bit zero-extension, and cover XMM, RFLAGS, and RIP state. AArch64
  declarations
  keep `SP`/`WSP` distinct from `XZR`/`WZR`, alias `Wn` with `Xn`, split vector
  low/high halves so AAPCS64 can preserve only the required low half of
  `v8`-`v15`, and cover NZCV, FPCR, FPSR, and PC. Both models publish ABI
  argument/result order, complete caller/callee/fixed partitions, stack/red-zone
  facts and selectable frame/dispatch/metering/platform reservations. The
  typed constraint catalog has stable family/variant keys, exact operand
  use/def roles and classes, optional fixed views, canonical ties, early
  clobbers, implicit uses/defs/clobbers, and an exact required-key inventory.
  Target-owned v1 catalogs cover scalar System V/Microsoft and AAPCS64/Darwin
  call/return banks plus Linux syscall and conservative inline-assembly state,
  then add the first ordinary materialize-i64, copy-i64, three-address add-i64,
  two-operand add-i64-immediate, compare-zero, and conditional-branch rows with
  explicit flags and instruction-pointer state. Both add rows are flag-
  transparent; x86-64 can realize them with LEA rather than inventing a two-
  address tie. This
  row describes physical register constraints only: exact, wrapping, or
  trapping arithmetic policy must remain in semantic lowering and may use this
  row only after any required overflow obligation is discharged.
  Generic validation rejects malformed IDs, keys, operands, ties, classes,
  units, and missing/extra inventory; a second ISA-owned comparison rejects
  class-compatible register substitution or missing architectural state. The
  former name-only constraint rows have been removed, so there is one
  constraint authority. The physical model and catalog now carry separate,
  domain-separated content identities; catalog identity is bound to its exact
  physical-model identity. Optimized staging now constructs and retains an opaque
  target-register environment from both validated artifacts plus an explicit
  active reservation profile. The current named conservative baseline activates
  every declared non-inapplicable overlay, excludes the Darwin platform overlay
  on Linux AArch64, and records the exact sorted effective-unit union. A composite
  identity binds the native target, all three component identities, and the six
  selected instruction keys (including the exact target `copy_i64` and
  `add_i64` rows), and is
  copied through selection, liveness,
  live-range, and transitional-assignment custody. The transitional
  scratch assignment and emission paths still do not consume it as allocator
  evidence. `OPT-REGISTER-MODEL` remains open for the complete ordinary
  instruction and feature-variant inventories, parallel ABI banks/aggregate
  forms, and joins to every backend/provider reservation.
- `CompileOptions` contains root, build directory, target, and output policy.
  `BuildConfig` now retains the exact canonical optimization selection set from
  the toolchain build vocabulary. Package-aware admission permits the exact
  root build selection and proves that dependency build companions cannot
  contribute one. The legacy compiler still rejects every nonempty set before
  emission. The clean typed terminal-component staging lane now enters verified
  optimization, target-operation lowering, a bounded typed virtual-register
  instruction CFG, independently replayed liveness/ranges/legality, phase-
  routed selected lowering, strict spill-free homes, and the post-allocation
  machine sidecar. An opaque
  `StagedOptimizedSelectedInstructions` carrier retains the optimizer run,
  final optimization unit, independent abstract projection, target plan, exact
  validated register environment, selected plan, and a content-identity-bound
  validation receipt. That first receipt names the validated pre-physical
  manifest identity in addition to the optimization-bundle and projection
  identities. Liveness, live-range, allocation-legality, fixed-view-copy, and
  register-home receipts propagate it; the parallel transitional-assignment
  receipt binds it directly as well. A nested `StagedOptimizedLiveness` carrier
  additionally retains an independently replayed, content-identified liveness
  plan over the selected CFG. `StagedOptimizedLiveRanges` nests that custody again with
  independently replayed block-local fragments, exact edge connectors, fixed
  sites, architectural state/actions, and canonical virtual interference.
  `StagedOptimizedAllocationLegality` nests that custody once more, joins the
  exact register-environment identity, records phase-specific physical-view
  candidates, and exposes incompatible fixed-view transitions. A bounded
  `StagedOptimizedRegisterHomes` carrier now nests the complete legality stage
  and assigns deterministic views only for transition-free, spill-free plans.
  It intersects candidates across every occupied point, rejects reserved or
  incompatible views through the upstream environment, uses exact virtual
  interference plus storage/write footprints, and independently replays the
  result under a content identity. The constant-leaf fixture reuses one result
  view across mutually exclusive leaf VRegs on both x86-64 and AArch64; the
  forwarded-value fixture fails closed on its two explicit ABI-entry-to-return
  transitions. This carrier grants no split, copy, spill, frame, emission, or
  publication authority. The exact named `LeafLocalBeforeFixedUseV1` policy now
  provides a separate bounded path for that forwarded fixture. It inserts two
  explicit ISA-owned `CopyI64` operations immediately before the leaf returns,
  creates fresh result VRegs, preserves original return provenance and logical
  fuel, assigns zero logical fuel to the native copies, and independently
  reconstructs the complete transformed selected CFG under a hard work budget.
  A sealed validated-analysis boundary then recomputes liveness, ranges,
  interference, architectural state, and allocation legality, requiring zero
  remaining transitions. A separate post-copy home carrier produces
  deterministic `RSI -> RAX` or `X1 -> X0` homes without weakening the direct
  transition rejection. Every new carrier remains custody-only and has no
  emission or publication path. A separate
  `StagedOptimizedAssignedOperations` carrier
  retains the optimizer run, ledger, projection receipt, target plan, assigned
  plan, and independently reconstructed root/function provenance custody. This
  is not allocator validation: the lane still fails closed before machine emission,
  object/image construction, component construction, or installation because
  no independently validated physical realization yet
  authorizes those records. Checked compilation
  retains the domain-separated selection identity, and the core crate defines
  a canonical replay/cache identity bundle over selections, ordered rules,
  target cost model, optional decision and workload inputs, and the
  transformation ledger.
- Terminal Psi semantics, proof evidence, fuel schedules, installation choices,
  and debug maps have separate identities. Optimization must preserve that
  separation and retain source fuel/provenance mappings.
- `omega-optimization-core` now owns frontend- and encoder-independent stable
  identities, rule contracts, ordered analysis/invalidation sets, safety and
  reason vocabularies, hard work budgets, candidate verdicts, and canonical
  decision/pass manifest rows. Decision rows use a self-authenticating v4 codec:
  identity is derived from the input revision, candidate, rule, verdict,
  consumed analyses, a canonical duplicate-free typed fact-reference set, and
  optional validator, then recomputed during decode. Applied decisions cannot
  be represented without an independent validator identity; identity tamper,
  unknown fact kinds, and reordered/duplicate facts reject. The Psi pass
  manager projects unary/binary scalar witnesses into these rows, including
  independently reconstructed propagated block-parameter facts and admitted
  operation-obligation facts consumed by proof-certified folds. The typed fact
  vocabulary can now also name an exact verifier-derived ownership-frontier
  identity. No rule consumes that capability yet; adding the reference does not
  broaden its source-site validity region.
- `omega-optimization-unit` now deterministically reconstructs explicit blocks,
  scalar definitions/uses, conservative effect links, structural roots,
  ownership events, literal facts, source provenance, and separately identified
  logical-fuel settlements from the clean plan. The independent
  `omega-optimization-validation` crate rejects malformed CFG, SSA dominance,
  edge bindings, provenance/fuel, effect chains, place roots, claim frontiers,
  and cleanup metadata. It now also independently re-derives every scalar
  operation contract: literal domains, Boolean and integer operand types,
  cast/widen legality, fixed-carrier requirements, conditional and return
  types, and complete scalar arity/type/result signatures for internal and
  boundary calls. Function and boundary catalogs reject duplicate identities.
  Corruption tests refresh derived node metadata, fact indices, and the unit
  identity before validation, proving that self-consistent but semantically
  ill-typed units still fail closed. The verified builder additionally attaches
  a canonical immutable catalog of verifier-owned block-entry,
  operation-entry/exit, and edge-entry/available-edge-exit ownership snapshots.
  `OPT-UNIT-BUILDER` and `OPT-UNIT-VALIDATOR` remain open until verified
  proof/range evidence and the complete structural place/path,
  structural-call/result, claim-transfer, boundary-completion, and effect
  vocabulary survive the Terminal-Psi lowering boundary.
- Proof-bearing integer casts, shifts, addition, subtraction, multiplication,
  division, and remainder now retain their exact obligation identities through
  Terminal abstract, target, and assigned-target operations. Exact add,
  subtract, and multiply remain distinct from wrapping operations until final
  ISA opcode realization. The optimization unit indexes these operation-to-
  obligation references and its independent validator reconstructs that index.
  The verified builder now joins each reference to the exact reconstructed
  operation owner and admitted proposition, canonically encodes the proposition,
  binds the Terminal-Psi and proof-bundle fingerprints, and attaches a sorted
  accepted-obligation fact index to the unit identity. Bare seeds still contain
  no accepted facts. The unit tasks remain open for the broader proof/range and
  region-indexed fact vocabulary.
- The clean artifact boundary now exposes a required
  `VerifiedTerminalOptimizationInput` for optimizer consumers. It retains the
  lowered plan beside the complete immutable Terminal module, exact proof
  bundle and fingerprint, verifier-reconstructed obligation set, and accepted
  facts. The ordinary empty-selection path continues to request only the bare
  plan, so it performs no optimizer-context construction. The unit tasks remain
  open until the builder projects the remaining context into complete
  rule-facing region indices and the independent validator checks the broader
  transformation vocabulary against them.
- Psi's structural-frontier verifier now exposes deterministic machine/block,
  operation-entry/exit, and edge-entry/exit snapshots with exact live claims,
  owned-place multiplicities, and projected moved paths. The verified optimizer
  carrier retains those snapshots, and the optimization unit now projects each
  one into a self-authenticating fact row whose complete claim/place/path state
  contributes to content identity. Attachment is one-shot and requires strict
  machine/site and nested snapshot order. The independent validator reprojects
  the complete catalog from the immutable verifier context on initial and
  transformed revisions; removal, duplication, reordering, or a recomputed
  forged snapshot fails closed. Terminal return/crash edges currently expose an
  entry snapshot but no exit snapshot, while control-successor edges expose
  both. The validator also checks proof fingerprints,
  reconstructed-obligation/admitted-fact agreement, operation-obligation
  ownership, and frontier coverage for every retained Psi provenance site.
  The unit tasks remain open for full effect summaries and current-region
  ownership derivation after arbitrary CFG rewrites.
- The clean abstract-operation plan now retains every block's scalar
  declarations in exact Terminal-Psi order instead of attempting to infer them
  from incoming edges. The unit validator independently re-derives operation
  definitions and uses, rejects forged parameter positions, and rechecks the
  current verified CFG contract: parameter-free entry blocks, closed edges,
  total reachability, and acyclicity. Every admitted unit revision now has a
  versioned content identity recomputed from its complete function, CFG,
  operation, fact, effect, ownership, provenance, and fuel state. Construction,
  accepted-fact attachment, rewrite validation, projection replay, and analysis
  admission reject a stale identity. Equal accepted content therefore has one
  unit identity independent of rewrite history; the transformation ledger
  retains that history separately. Exact module structural-type, boundary, and
  provider catalogs now survive into the unit, as do every function's nominal
  attachment, scalar/structural parameters, result shape, full ordered entry
  claims, and published service ceiling. Content identity v4 encodes these
  rows exhaustively; the independent validator compares them with the verified
  source on initial and transformed revisions, and optimized-plan projection
  consumes the unit-held rows before independently checking the round trip.
  Bare validation also rejects a detached entry-claim index or a normal return
  whose kind/identity/type contradicts the retained result signature. The unit
  tasks remain open for region-indexed semantic facts, complete effect/call
  summaries, and the broader path-sensitive place/ownership vocabulary.
- `omega-psi-optimizer` now owns the first deterministic analysis slice:
  predecessor/successor and reachability indices, normal/crash exits,
  dominators, post-dominators, block SCCs, reducible/irreducible loop regions,
  and recursive call-graph SCCs. Its compilation-local analysis manager caches
  by unit identity, resolves declared dependencies in canonical order, expands
  invalidation transitively, supports stable parallel cold computation, and in
  validation mode rejects an undeclared graph change atomically. Every cached,
  cold, and revision-commit entry first checks the recomputed content identity,
  so mutating a unit while retaining its old revision cannot reuse cache rows.
  The manager and CFG tasks remain open until rewrite rules exercise the full
  audit and suspension/richer call exits are present in the optimization
  representation.
- The previously reserved `OwnershipFrontiers` analysis kind is now active. It
  exposes exact source-site fact identities and snapshots, bound to the current
  unit revision, without treating them as timeless function-wide conclusions.
  The analysis manager always invalidates and rebinds this view at a revision
  commit even though the underlying verifier catalog is immutable. No rewrite
  consumes these facts yet; current-region/path applicability remains an open
  semantic-analysis task rather than being inferred from a removed source site.
  The manifest fact-reference codec has a distinct typed
  `OwnershipFrontierFactIdentity` row, so a future borrow-aware rule can record
  the exact capability it used without confusing it with a scalar or admitted
  obligation fact.
- The Psi optimizer now also has immutable compilation-local rule registries.
  They preserve the pass manager's explicit schedule (never hash-sort it),
  reject duplicate rule identities, bind the exact order into a rule-set
  identity, and share no global mutable registry state. Built-in registration
  rows carry explicit contiguous schedule ordinals, so shuffled contribution
  arrival reconstructs the same declared schedule without changing the
  order-preserving public registry contract. Initial semantic
  products cover use/definition rows, scalar constants, exact integer ranges,
  and executable/inexecutable edges. Constants and edge verdicts are now
  projections of one deterministic SCCP fixed point over executable blocks,
  exact `EdgeId`s, and the undefined/constant/overdefined value lattice.
  Block-parameter meets use only feasible incoming edge bindings: a selected
  constant arm excludes the dead arm, equal values on two feasible arms remain
  constant, and differing values become overdefined. Support retains canonical
  operation and transitive edge sets plus the unit-revision/machine/value
  region; scalar constants now depend on CFG invalidation. Literal facts in a
  semantically dead block are not published. A propagated block-parameter fact
  now receives a domain-separated identity over the input revision, typed
  definition, constant, and a canonical machine snapshot containing every
  block reachability state, exact edge verdict, and scalar lattice state. Thus
  omitting or changing even an infeasible competing edge changes the identity.
  The validation crate owns a second coupled fixed-point implementation, built
  without an optimizer dependency, and reconstructs that snapshot before a
  propagated fact can authorize a rewrite. A selected-arm block-parameter
  complement fixture passes this independent gate. `OPT-SCCP` now covers every
  currently foldable closed Terminal-Psi integer and Boolean operation;
  structural-field and call results remain overdefined without source facts.
  The task reopens when the scalar vocabulary grows (including any future
  trapping or exact-float policy). Semantic analyses remain open for the wider
  proof/effect/ownership vocabulary.
- Conservative node-effect summaries now distinguish pure scalar work,
  structural state, internal calls, boundary calls, services, and control.
  Unknown internal-call crash/suspension/observation behavior remains `May`;
  only represented facts can narrow it. Function summaries now compute a
  deterministic call-graph fixed point over reachable blocks, propagating
  observable/structural/crash/suspension knowledge plus exact service and
  boundary identity sets through recursive calls. Unknown callees stay `May`,
  while closed pure recursion does not invent effects; every summary retains
  canonical transitive provenance and its unit revision. Fixed-point scalar
  liveness publishes canonical block and node entry/exit sets and handles
  cyclic synthetic CFGs, preparing dead-scalar rules without treating the
  current acyclic Terminal slice as an architectural limitation.
- The optimization unit now reconstructs an exhaustive observation row for
  every Terminal abstract operation. Each row now retains the full operation
  and CFG successor payload even for pure scalar work, alongside definitions/
  uses, the effect token, ownership and cleanup events, exact Psi provenance
  and logical fuel, conservative crash/suspension knowledge, and classified
  structural, call, service, control, normal-exit, and crash-exit events. The
  operation match is exhaustive so new vocabulary cannot compile without
  classification. Exact integer rewrite validation independently compares all
  closed scalar observation axes before acceptance, reconstructs its node-
  region live-ins and live-outs separately from optimizer analysis, requires
  unchanged live-outs, and forbids new live-in dependencies. The two liveness
  implementations agree on the focused CFG fixture.

  The first multi-block observation slice now guards redundant block-parameter
  copy propagation. The validator independently derives the complete affected-
  block roster, performs its own exact typed scalar/binding-slot normalization,
  and compares canonical whole-block observations across revisions. Those
  observations retain full operations, parameters, successor/boundary edges,
  typed live-ins/outs, effects, crash/suspension state, events, ownership/
  cleanup, provenance/fuel, and explicit present-or-absent verifier-frontier
  identities. Content outside the derived region must remain exactly equal.
  Mutation coverage independently changes arithmetic policy, control/exit
  edges, effects, ownership/cleanup, provenance, fuel, call/crash/suspension
  behavior, typed liveness, frontier identity, and outside-region state. The
  rule remains v1 while the strengthened independent validator is v2.
  `OPT-OBSERVATION-MODEL` remains open for general arbitrary regions, real
  memory traces, explicit suspension edges, and path-derived current ownership
  facts.
- Successful Psi runs now emit a canonical transformation ledger binding the
  exact Terminal-Psi identity, fuel schedule, initial/final unit revisions, and
  every validated rule/candidate/validator revision step. Each step explicitly
  partitions its source operation/edge set into `RealizedAt(output)` or
  `ProvenUnreachableAt(input)` rows and retains the original scheduled fuel for
  both. Only realized rows are runtime charges; unreachable rows retain audit
  custody without inventing a zero-unit settlement or fake output. Broken
  chains, duplicate candidates, empty/duplicate or cross-disposition source
  custody, zero/mismatched fuel, and noncanonical rows reject. The independent
  projection validator requires the initial source/fuel map to equal the exact
  disjoint union of final realized source/fuel and cumulative proven-
  unreachable rows, and rejects resurrection after a tombstone. The pass
  manager ledgers validator-accepted accounting, not the proposal. Empty no-op
  runs receive a valid identity-preserving ledger. `OPT-FUEL-MAP` remains open
  for path-qualified one-to-many rewrites and physical/runtime metering joins,
  and the publication gate must later consume this ledger rather than trusting
  its producer.
- The public Psi run now requires the exact named `OptimizationSelections`,
  reconstructs the built-in registry for that set, and rejects detached rule
  schedules or any named optimization whose implementation is unavailable. A
  successful run emits the canonical composite replay/cache identity over the
  exact selections, ordered rules, explicit baseline structural cost model,
  decision log, and transformation ledger. This is not an `O1`/`O2`/`O3` or
  debug/release mode: the identity names the actual explicit transformation
  selection set and every constituent rule. A convenience spelling may only
  expand to that visible set; it cannot become a distinct opaque level.
- The separately named `CopyPropagation` selection now registers one initial
  redundant-block-parameter rule. It proposes elimination only when every
  exact incoming `EdgeId` binds the same typed value and dominator/use-definition
  products prove that value dominates every rewritten use. The candidate binds
  the complete ordered incoming-edge witness, exact scalar substitution, all
  affected blocks, and multi-node provenance/fuel rows. A patch-specific
  validator independently enumerates the edges, replays the substitution,
  removes only the parameter and corresponding binding positions, reindexes
  later parameters, reconstructs operation metadata and the proof-fact index,
  and runs the total unit validator. Focused tests cover both arms of one
  conditional targeting the same block, differing arguments, incomplete
  witnesses, proof-obligation retention, semantic-accounting retention, the
  empty consumed-fact manifest projection, and a block-parameter-count fixed
  point. `OPT-COPY-PROPAGATION` remains open for explicit scalar-copy forms and
  wider call-result/debug-materialization coverage. SCCP and copy propagation
  can now be selected together: orchestration derives the canonical SCCP then
  copy-propagation schedule and retains one chained manifest per named pass,
  including a manifest for a pass that commits no rewrite.
- The separately named `ControlFlowCleanup` selection now registers an exact
  CFG rule: a Boolean-proven `Conditional` and every block made
  structurally unreachable by selecting its exact edge are rewritten in one
  atomic candidate. The v5 rule and v4 validator independently derive the
  fresh reachability complement, retain shared/reconverged blocks, densely rebase
  later surviving effect links, and rebuild the operation fact and declared-
  place indexes before total validation. Candidate accounting binds the
  decision block, every removed block, and every surviving block whose effects
  shift. It realizes the selected edge and shifted surviving nodes, while the
  rejected edge and every deleted node retain their original provenance/fuel
  as separately located `ProvenUnreachableAt` rows. `CallGraph` is explicitly
  invalidated; verifier-accepted obligation and ownership-frontier catalogs
  remain immutable source custody. Focused tests cover both Boolean arms,
  shared-merge preservation, block/effect/fact reconstruction, incomplete
  region and tombstone corruption, projection/report custody, and a second
  fixed-point sweep.
- `ControlFlowCleanup` also registers `linear-empty-block-thread.v2`. This
  intentionally handles only a non-entry block
  containing one unconditional jump, with exactly one incoming edge owned by
  another unconditional jump. The rule composes typed block bindings, proves
  verifier-frontier identity across the bypass, and realizes both co-executed
  edge/fuel sources at the retained successor edge rather than falsely declaring
  the removed jump unreachable. The independent validator reconstructs the
  one-predecessor shape, bindings, frontier snapshots, exact affected roster,
  fused provenance/fuel, dense effects, facts, places, and output identity.
  The third exact rule, `path-qualified-empty-block-thread.v1`, atomically
  bypasses every incoming edge of the same empty jump block, including
  conditional and multiple predecessors. Successor edges now own their exact
  provenance/fuel, and every ledger row names both its input occurrence and
  realized output occurrence. The removed outgoing occurrence may fan out only
  over the independently checked incoming-edge antichain; the total unit
  validator rejects duplicate sources on co-executable edges. Projection replay
  applies the occurrence relation record by record, so later threading cannot
  resurrect, lose, or double-charge a source. A verified artifact test carries
  one source through fanout and two later rewrites to the final pre-physical
  projection. Candidate v13, optimization-unit identity v7, ledger v3,
  prephysical manifest v6, the v7 pass, and optimized-plan projection v7 bind
  this admission meaning. General native publication of these broader CFG
  shapes remains unavailable until their physical lowering vocabulary exists.
- The fourth exact `ControlFlowCleanup` rule,
  `adjacent-single-predecessor-block-merge.v3`, removes a genuinely redundant
  jump and block boundary without treating the target as empty. Admission is
  limited to an immediately adjacent target with exactly one incoming edge.
  The target must begin with either a real operation having no successor arms,
  its sole conditional terminator, or an exact return/crash terminal carrying
  edge provenance; non-adjacent block motion remains outside the rule. Typed target parameters are
  replaced by the exact incoming bindings, ownership snapshots must agree
  at edge entry, edge exit, and target entry, and every moved node occurrence
  is replayed. The removed jump-edge source/fuel is fused behind the first
  operation's direct provenance at its new node. For a conditional-first
  target, that source fans out onto exactly its two mutually exclusive
  successor edges. Validator corruption tests reject forged node and fanout
  occurrences, while verified artifact tests reach the exact one-block and
  three-block projections. Candidate v13, optimization-unit identity v7, the
  v9 pass, prephysical manifest v8, and optimized-plan projection v9 bind this
  additional admission meaning; ledger v4 already represents both moves.
- The fifth exact `ControlFlowCleanup` rule,
  `shared-terminal-jump-fusion.v1`, removes one unconditional jump into a
  shared terminal-only block without removing or mutating that target. The
  target must have at least two incoming edges and contain exactly one
  `Return`, `ReturnUnit`, `ReturnStructural`, or `Crash`. Typed parameters are
  substituted only in the cloned terminal, while ownership snapshots must be
  identical at incoming-edge entry, incoming-edge exit, and target entry. The
  incoming edge is realized at the clone and the original terminal occurrence
  fans out to the clone plus its retained source site with identical fuel.
  Total-unit validation admits repeated edge provenance at node sites only for
  exact no-successor terminals in pairwise CFG-antichain blocks; operation
  provenance, mixed node/edge occurrences, and co-executable duplicates remain
  rejected. A full verified artifact test replays the one-to-many terminal
  custody into the pre-physical plan and projection. Candidate v15,
  optimization-unit identity v9, the v10 pass, prephysical manifest v9, and
  optimized-plan projection v10 bind this admission meaning; ledger v4 already
  represents the fanout.
- The sixth exact `ControlFlowCleanup` rule,
  `unreachable-private-machine-pruning.v1`, atomically removes the complete
  active function complement outside the executable root closure. Roots are
  the module entry, provider candidates, conservatively retained attached
  functions, and their transitive internal-call and nominal-cleanup-machine
  references. The candidate names the exact canonical machine set rather than
  inventing a node decision point; the independent validator reconstructs the
  closure and exact complement without trusting `CallGraph`. Active plus pruned
  machines must form an order-preserving partition of the verified source
  roster. Accepted-obligation and ownership-frontier catalogs remain immutable
  historical custody, while every source-bearing removed node and successor
  edge is ledgered as proven unreachable with its original fuel. Candidate
  v14, optimization-unit identity v8, ledger v4, the v9 pass, prephysical
  manifest v8, and optimized-plan projection v9 bind machine-roster replay.
- The first closed rewrite candidate is exact integer constant evaluation for
  proof-bearing add/subtract/multiply. The immutable candidate binds its input
  revision, rule contract, decision point, affected region, required analyses
  and invalidations, substitutions, exact provenance/fuel, literal-fact
  witness, predicted non-authoritative cost, and a typed patch. Only the
  independent validator may derive the accepted output's content identity.
  Witnesses no longer trust raw supporting operation IDs: each operand names a
  domain-separated scalar-fact identity bound to the input revision, machine,
  value, scalar type, exact definition site, constant payload, and literal
  source. Candidate encoding is versioned around those typed identities. The
  independent validation crate reconstructs each identity from its own lookup
  of the immutable unit before re-reading both supported scalar facts,
  evaluates the exact typed operation, constructs the output itself, retains
  Psi provenance/fuel, replaces the consumed obligation reference with the new
  constant fact, and rejects a wrong result without mutating input. Thirty
  built-in rules cover add, subtract, multiply, divide, remainder, and shifts
  plus exact integer casts, widening, and unary/binary bitwise operations across
  their declared exact/wrapping/saturating policies, plus Boolean not/equality
  and integer equality/ordering comparisons, with distinct stable identities
  under one ordered SCCP pass group. Candidate identities
  distinguish unary cast/widen/complement evidence from binary operation
  evidence, and the validator rejects an operand-shape mismatch. Shift
  evaluation retains the operation's distinct count carrier and delegates to
  Psi integer semantics; it never substitutes host-language shifting. Cast,
  widen, width-bounded complement, and bitwise and/or/xor evaluation similarly
  delegate to the source and target Psi integer domains. Proof-bearing
  operations require `ProofCertified`; goal-free wrapping/saturating arithmetic,
  shifts, widen, and bitwise operations require `ExactOperationSemantics`.
  Focused overflow tests produce `4` versus `255` for the same `u8` add
  operands, all four exact/wrapping shift directions and three binary bitwise
  operations pass independent validation, valid cast/widen/complement cases
  fold, and out-of-domain casts, overflowing exact shift-left, and zero-divisor
  division remain inapplicable. The shared independent validator reconstructs
  the fact index rather than trusting rule-authored insertions. Boolean results
  use a distinct canonically encoded patch, Boolean fact reconstruction, and
  independent validator; the pass manager dispatches by typed patch and cannot
  confuse integer and Boolean outputs. A Boolean fixed-point fixture exercises
  that public dispatch. Comparison proposal and validation independently
  reconstruct both operand integer types and delegate ordering to Psi's typed
  comparison semantics; both rule and validator type lookup now covers function
  parameters, block parameters, and node definitions. Propagated constants are
  usable only through the independently reconstructed full fixed-point identity
  described above. Rules can propose candidates only when their explicit parent
  selection is present. No build hook admits that still-incomplete selection.
- A verified-session-only pass manager now performs canonical rule dispatch,
  dependency analysis, proposal enumeration, deterministic negative-cost
  choice with candidate-identity tie breaks, independent validation,
  monotone exact-operation convergence, atomic analysis invalidation, and hard
  accounting for rule evaluations, candidates, validation steps, commits, and
  iterations. Budget exhaustion returns no output, every registered rule is
  covered before successful convergence, and the verifier-owned optimizer
  context remains attached to the resulting unit. A dependent add-then-
  multiply fixture proves the ordered multi-rule group reaches a deterministic
  fixed point across revisions. A canonical revision-history guard now rejects
  a repeated unit identity before analysis invalidation, commit accounting, or
  publication; a deterministic synthetic `A -> B -> A` fixture proves the
  repeated `A` is never committed. A separate `omega-optimization-policy` crate now
  receives only independently validated candidate summaries, chooses improving
  work by exact cost then stable candidate identity, cannot select outside that
  admitted set, and emits a canonical decision log whose codec rejects identity
  tamper and trailing bytes. A real canonically encoded Terminal-Psi artifact
  containing literal exact addition now passes proof admission, enters through
  the public verified optimizer carrier, folds to its exact constant, and
  retains the admitted obligation fact. Each rule registry is one named pass
  group (mixed pass identities reject), and successful runs emit a canonical
  pass-manifest row binding input/output revisions, ordered rules, work usage,
  validator-backed applied decisions, and validator-backed deterministic
  skips. The public pipeline executor now runs the canonical ordered pass
  groups for the exact selected set, applies the explicit work ceiling to each
  group, retains every pass-local manifest, and derives aggregate decision-log,
  ledger, usage, and identity-bundle evidence over the full initial-to-final
  chain. Independent projection rejects reordered or omitted manifests, even
  when the omitted pass committed nothing. Duplicate candidate identities fail
  closed. A new independently validated pre-physical manifest now joins the
  exact selections, pass rows, decision-log and identity-bundle digests,
  transformation ledger, provenance/fuel mapping, per-pass work ceiling and
  aggregate usage, abstract projection receipt, and source/final structural
  statistics under one domain-separated content identity. Its deterministic
  text view is downstream of that structured record. A versioned standalone
  codec now carries the complete record, including the transformation ledger;
  strict nested decoders reject malformed identities, revision chains,
  provenance, truncation, and trailing bytes, and decoding yields only the
  untrusted record that must still pass independent manifest validation. It
  explicitly records that physical realization data is unavailable rather
  than publishing zero or guessed code-size, spill, frame, or allocation
  statistics. A root `build.omg` may now independently request the human
  projection with `builder.optimizations.emit_report()`: report-only builds
  retain an empty transformation set, absence suppresses it, and duplicates
  reject. The pipeline derives a cumulative carrier from validated staged
  custody, joining the pre-physical and post-allocation records plus the
  function-relative record when present; suppression changes only whether text
  is projected. The final publication manifest/report remains open for later
  downstream records and compiler artifact/rebuild-metadata integration.
  The first independently validated post-allocation extension is now retained
  by both strict register-home carriers. It joins the pre-physical manifest to
  the exact target, selected plan, ordered typed selected-transformation ledger,
  liveness/range/legality/environment/home identities, and exact function,
  assignment, distinct-view, interference, and zero-transition counts. It may
  state that spills were not required for that validated home plan, but marks
  frame, emission, and publication unavailable. Its human renderer is again a
  projection of the content-identified record. The transformation ledger
  distinguishes fixed-view-copy and literal-fold identities in application
  order and rejects exact duplicates. A separate optional selected-lowering
  completion identity proves named-suite execution without pretending a
  zero-change result transformed the CFG. The v4 canonical codec round-trips
  direct and transformed forms, reconstructs typed target and stage fields,
  and rejects identity tampering, unknown tags, truncation, and trailing bytes.
  Decoding remains non-authoritative. A new selected-lowering-only
  function-relative realization manifest joins the exact full suite and phase
  subset, mandatory completion, pre-physical/post-allocation manifests, final
  selected CFG, pre-/post-allocation machine roots, pre-layout encoding,
  resolved layout, target, named layout policy, and derived function/block/
  instruction/byte/branch statistics. Its strict v1 codec recomputes identity,
  closed-vocabulary corruption fails, and custody replay reconstructs every
  joined artifact. Its v2 schema now also binds an independently replayed
  frameless whole-function exit contract. The contract selects the exact
  System V AMD64, Microsoft x64, AAPCS64, or Darwin AAPCS64 convention; proves
  canonical RSP-pop or X30 return behavior for every Psi exit; retains RAX/X0
  result custody separately from encoded return reads; and rejects stack/frame
  effects, X30 damage, or any unpreserved callee-saved write. Compiler-selected
  lowering derives a caller-saved-only unconstrained availability set for this
  policy, while an unrestricted x86 plan that chooses RBX fails closed. Frame,
  emission, sections, symbols, relocations, image, installation, and publication
  remain unavailable. Final physical/publication and artifact metadata remain
  open.
- `omega-lowering-optimizer` now owns a custody-preserving bridge from a
  completed `OptimizationRun` to a clean `TerminalAbstractOperationPlan`.
  Projection replays every retained candidate declaration through the
  independent rewrite validator, checks commit/ledger/manifest/bundle
  agreement, revalidates the transformed unit against the immutable verifier
  context, and independently reconstructs the projected plan shape. The opaque
  result retains the run, exact named selections, decisions, accepted facts,
  ledger, validation receipt, and validated pre-physical manifest; it exposes
  the plan only by borrow and does not claim native publication authority.
  Focused tests cover empty-selection identity, proof-certified constant
  folding, block-parameter copy propagation, deterministic replay, projection
  corruption, commit corruption, manifest field/identity corruption, ordered
  multi-pass evidence corruption, and clean target lowering. The new
  `omega-optimization-pipeline` orchestration crate accepts only an explicit
  nonempty named selection and per-pass budget; it performs artifact admission,
  unit construction, canonical pass execution, and independent projection.
  Clean compiler staging uses this as its only selected route and consumes the
  opaque optimized carrier through a dedicated target-lowering API. The
  empty-selection compatibility route does not call it. All currently
  representable Terminal abstract operations cross this cut by exhaustive
  behavioral observation and lowering; unavailable named families reject at
  registry construction. The compiler-facing physical wrapper derives the
  selected-lowering projection from retained suite custody and covers Psi-only,
  mixed, and lower-only suites on both targets. The current ordinary empty
  program entry is outside the bounded selected-instruction shape and now fails
  at that named boundary rather than falling back to transitional scratch
  assignment. Successfully admitted physical fixtures still stop before frame,
  emission, artifact construction, and publication.
- Omega float semantics forbid ambient fast math. Exact versus wrapping,
  saturating, trapping, fused, and unfused behavior is operation identity, not
  an optimizer preference.

## Squalr pattern audit

`../Squalr` provides the intended small rule-planning pattern:

- rule traits have stable IDs;
- registries own built-in rules;
- rules map an input or mutable execution plan to a more efficient plan;
- dispatch selects a specialized implementation from the planned result; and
- a debug scalar scan can compare the specialized result against a reference.

Omega should retain the separation of rule, registry, plan, dispatcher, and
reference validation. Do not copy Squalr's transitional global singleton,
unsafe initialization, hash-map iteration order, in-place partial mutation, or
absence of declared analysis invalidation. Omega registries must be explicit,
ordered values; rules propose atomic patches; a separate validator accepts or
rejects them. The audit also found registered element-parameter rules without a
dispatcher call site in the scanned crates; Omega therefore needs a registry
coverage test proving that every enabled rule phase is actually scheduled.

## Ownership and placement

Final product source:

```text
source/omega/optimization/
  core/
  psi/{analyses,passes,validation}/
  lowering/
  regalloc/
  machine/
  policy/
  cost/
source/omega/pipeline/
```

Rust migration/reference implementation:

```text
source/on-ramp/rust/omega/
  foundation/omega-optimization-core/
  representations/omega-optimization-unit/
  representations/omega-register-model/
  optimization/
    omega-psi-optimizer/
    omega-lowering-optimizer/
    omega-regalloc/
    omega-machine-optimizer/
    omega-optimization-validation/
    omega-optimization-policy/
  orchestration/omega-optimization-pipeline/
```

Do not create one crate per analysis or pass. Do not place Rust under
`source/omega/`. Existing representation crates continue to own
their data; optimizer/pipeline crates transform them. ISA crates own declarative
target facts and encodings, not cross-target pass policy.

## Global gates

Every milestone below must maintain all of these gates.

1. **Default-off compatibility.** A package that does not opt in takes the
   existing pipeline. It does not initialize optimizer registries, load models,
   emit optimizer-only failures, or change native/interpreter output.
2. **Fail-closed opt-in.** An opted-in build rejects an unsupported slice,
   unavailable pass, invalid candidate, incomplete proof, or failed validator.
   It never silently emits an unoptimized or differently optimized artifact.
3. **Exact semantics.** No ambient fast math, new suspension point, reordered
   effect, weakened cleanup, invented provider, hidden trap, or changed logical
   fuel behavior.
4. **Determinism.** The same canonical inputs, exact optimization selections,
   target model, decision log, and compiler produce the same ordered decisions,
   ledger, and bytes.
5. **Independent validation.** A rule, allocator, scheduler, model, or search
   policy cannot accept its own output merely by returning success.
6. **Provenance.** Every optimized operation and physical interval maps to the
   exact Terminal Psi operations/edges and semantic fuel charges it realizes.
7. **Bounded compilation.** Pass groups have explicit work budgets and
   convergence measures. Exhaustion returns a deterministic diagnostic; it does
   not hang or commit a partial candidate.

## Execution order

```text
P0 explicit build selections + disabled-path firewall
  -> P1 optimization unit, analyses, rule engine
  -> P2 validation and publication gate
  -> P3 exact target-neutral passes
  -> P4 lowering optimization and virtual-register form
  -> P5 register allocation
  -> P6 machine optimization
  -> P7 proof/borrow-aware advanced passes
  -> P8 offline search/ML seams
  -> P9 stabilization and possible promotion
```

Terminal-Psi vertical-slice coverage proceeds in parallel with P0-P2. A pass is
enabled only for the operation vocabulary its validator understands. Do not
route new optimization through legacy `StateGraph` merely to avoid that
dependency.

## P0 — Opt-in and compatibility firewall

- **OPT-MANIFEST-SCHEMA.** Add a structured optimization manifest and a human
  report projection.

  Acceptance: opt-in output records source Terminal Psi identity, optimized
  realization identity, ordered passes/rules, candidate verdicts, consumed
  facts, validator identities, fuel/provenance map, code-size statistics, and
  allocator data when applicable. Suppressing the human report changes no
  decision or executable byte.

  Current closure boundary: pre-physical, post-allocation, and selected-
  lowering function-relative records are structured, content-identified, and
  independently replayable. The function-relative record supplies truthful
  code-size statistics and the v2 record binds a validated frameless leaf exit
  contract while declaring every later authority unavailable. This task
  now also includes an explicit suppressible root-build human-report request
  and a pipeline-owned cumulative carrier over all currently available
  records. This task remains open for general frame/call/save-restore data,
  emission and relocation custody, final artifact/rebuild metadata, and
  materializing the retained request on successful native publication.

## P1 — Optimization representation and rule engine

- **OPT-UNIT-BUILDER.** Build `PsiOptimizationUnit` from a verified
  `TerminalAbstractOperationPlan`.

  Acceptance: functions have explicit blocks and edges, typed SSA scalar
  values, structural places, memory/effect chains, calls, crash/suspension
  exits, ownership/claim/cleanup frontiers, proof/range fact indices, fuel
  sites, and complete Psi provenance. No syntax tree or `ExpressionHandle`
  survives. Rebuilding the same unit is deterministic.

- **OPT-UNIT-VALIDATOR.** Implement a total structural validator independent of
  pass implementations.

  Acceptance: it rejects undefined/nondominating values, block-argument type or
  arity mismatch, malformed CFG, invalid place paths, broken memory/effect
  chains, incomplete provenance, duplicate fuel settlement, invalid
  ownership/claim frontiers, and cleanup-order changes. Corruption tests mutate
  each class independently. Current scalar slice is total and wildcard-free:
  operation/result/operand contracts and cross-function/boundary scalar
  signatures are independently reconstructed even when all cached metadata and
  content identities have been refreshed. Structural call/place/claim
  contracts remain open under this task.

- **OPT-ANALYSIS-MANAGER.** Add deterministic revision-keyed analysis caching,
  dependency declaration, and precise invalidation.

  Acceptance: a test rule that lies about invalidation is detected in the
  pass-validation configuration; cached and cold analysis runs agree; parallel
  analysis scheduling has stable ordered output.

- **OPT-CFG-ANALYSES.** Implement reachability, predecessor/successor indices,
  dominators, post-dominators, loop forest, and SCC/call-graph analysis.

  Acceptance: irreducible loops, recursion, crash-only exits, suspension exits,
  and disconnected private machines have focused tests.

- **OPT-SEMANTIC-ANALYSES.** Implement constant/executable-edge, value-range,
  effect/service, crash, suspension, alias/place, escape/address-stability,
  memory-version, ownership/cleanup, and scalar/place liveness analyses.

  Acceptance: every result names its exact supporting fact identities and
  validity region. Expired, path-mismatched, wrong-version, or incompatible
  access facts produce `Unknown`, not a broadened conclusion.

- **OPT-ORDERED-REGISTRY.** Implement explicit ordered built-in registries with
  duplicate detection and no global singleton.

  Acceptance: insertion/iteration order is canonical, registries are immutable
  during a run, and two concurrently compiled programs cannot affect each
  other's rule sets.

- **OPT-RULE-CONTRACT.** Implement immutable-input rule proposal and atomic
  rewrite patches.

  Acceptance: a candidate declares decision point, affected region, required
  analyses, invalidations, substitutions, provenance changes, witness, and
  predicted non-authoritative cost. Rejected validation leaves the unit and all
  analysis revisions unchanged.

- **OPT-PASS-MANAGER.** Implement analyze/enumerate/choose/validate/commit with
  named versioned pass groups.

  Acceptance: work budgets, deterministic tie breaks, candidate limits, and
  fixed-point convergence metrics are enforced. Oscillating synthetic rules
  terminate with a deterministic diagnostic and no partial commit.

## P2 — Equivalence and publication gate

- **OPT-OBSERVATION-MODEL.** Define the compiler-owned equivalence boundary over
  live-ins/outs, normal/crash/suspension exits, memory/effect traces,
  boundary/provider events, ownership/cleanup frontiers, and fuel attribution.

  Acceptance: the model covers every currently executable Terminal Psi
  operation. Adding a vocabulary operation fails compilation until its
  observation case and tests land in the same vertical slice.

  Current slice: every current operation has an exhaustive node observation,
  scalar evaluation has an independently reconstructed one-node boundary, and
  redundant block-parameter elimination has the first canonical whole-block
  closed-region boundary. The latter compares full normalized operations and
  successor edges, typed live-ins/outs, effects/events/exits, ownership/cleanup,
  provenance/fuel, and retained source-frontier presence. General memory/
  suspension traces and arbitrary-region composition remain open.

- **OPT-REWRITE-WITNESS.** Define a canonical local transformation witness over
  one closed region.

  Acceptance: the verifier reconstructs the pre/post questions from the source
  unit and patch. The candidate cannot omit an observable live-out, effect
  token, crash edge, cleanup action, or fuel site by leaving it out of its
  witness.

  Current slice: the block-parameter candidate identity binds its canonical
  blocks, typed substitution, incoming edges, and changed-node provenance/fuel,
  but the independent validator re-enumerates them before deriving the
  observation region. No producer-authored observation digest is trusted.

- **OPT-LOCAL-VALIDATOR.** Implement bounded validators for CFG identities,
  constant substitution, copy propagation, dead pure values, and algebraic
  operations under exact typed semantics.

  Acceptance: each validator is shared by multiple rules where possible and
  contains no cost/benefit policy. Mutated witnesses and wrong arithmetic
  domains reject.

  Current slice: exact scalar evaluation and redundant block-parameter
  structural identity both have patch-specific independent constructors. The
  latter's v2 validator adds the first multi-block observation comparison and
  exact outside-region equality gate; general CFG identities and later rewrite
  classes remain open.

- **OPT-PROOF-BRIDGE.** Generate proof-kernel-checkable propositions and
  derivations for rewrite classes expressible in the current proof vocabulary;
  identify the minimal new proof rules needed for the rest.

  Acceptance: the optimizer/prover remains untrusted; an independently
  reconstructed obligation set is checked through the existing admission
  profile. Unsupported entailment rejects rather than becoming a compiler
  axiom.

- **OPT-FUEL-MAP.** Preserve exact Terminal Psi logical charges through
  many-to-one and one-to-many rewrites.

  Acceptance: fixed-work certificates continue to refer to original semantics;
  dynamically metered optimized execution charges the same executed semantic
  sites; an optimized native build cannot observe lower fuel merely because it
  uses fewer instructions.

- **OPT-PUBLICATION-GATE.** Require a complete accepted transformation ledger
  before optimized native output can be installed.

  Acceptance: missing source regions, unvalidated candidates, unknown rule
  versions, mismatched selection/decision identity, or incomplete physical
  provenance prevent publication even when byte emission succeeds.

- **OPT-DIFFERENTIAL-HARNESS.** Generalize the native differential harness to
  compare interpreter, optimizer-disabled native, and explicitly selected
  optimized native observations.

  Acceptance: generated inputs and curated cases compare result, output bytes,
  crash route, boundary trace, and other exposed observations. A differential
  pass is evidence and regression coverage, never the publication authority.

## P3 — Initial exact Psi optimizer

- **OPT-CFG-CLEANUP.** Add unreachable-block elimination, empty-block
  threading, constant conditional folding, redundant jump elimination, and
  unreachable private-machine pruning.

  Acceptance: operation/edge provenance and fuel mapping remain complete;
  crash, cleanup, suspension, and boundary-only blocks are never classified as
  empty.

  Current slice: exact constant-conditional folding is available under only
  the explicit `ControlFlowCleanup` selection. Its v5 rule and v4 validator
  atomically replaces the conditional and removes exactly the blocks made
  unreachable by that selected edge, while retaining shared successors. It
  rebuilds dense effects, current operation facts, and declared places;
  invalidates the call graph; realizes all surviving source sites whose output
  location changes; and durably records the rejected edge plus every deleted
  node and its original scheduled fuel as independently proven unreachable and
  uncharged. Successor-edge custody and ledger v3 now distinguish the two
  conditional arms directly. The v10 pass also includes exact linear empty-jump
  threading plus `path-qualified-empty-block-thread.v1`: typed bindings are
  composed, ownership frontiers must be identical across each bypass, and the
  removed outgoing source is realized on every and only mutually exclusive
  incoming edge. Its fourth rule removes an adjacent jump into a unique-
  predecessor block whose first node is either a real non-branching operation,
  the block's sole conditional, or its exact return/crash terminal. It substitutes typed block parameters and
  realizes the removed edge at the first operation or across exactly the two
  mutually exclusive successor edges, without authorizing non-adjacent code
  motion. Its fifth rule fuses one selected unconditional path into a shared,
  terminal-only return/crash block while retaining that block for its other
  incoming paths. Terminal provenance and fuel fan out only across the exact
  no-successor CFG antichain, and typed substitutions affect only the clone.
  Its sixth rule prunes the exact unreachable private-machine
  complement, rooting entry, providers, attached functions, internal calls,
  and nominal cleanup-machine references. Candidate v15, optimization-unit
  identity v9, ledger v4, prephysical manifest v9, and projection v10 bind both
  occurrence and function-roster replay. General non-adjacent redundant jumps and
  unreachable cleanup not caused by the conditional fold
  remain open.

- **OPT-SCCP.** Implement sparse conditional constant propagation over the
  closed integer and Boolean Terminal Psi operations.

  Acceptance: folds honor width, signedness, `Exact`/`Wrapping`/`Saturating`/
  `Trapping` policy, exact casts, shifts, division/remainder obligations, and
  block parameters. Float support waits for complete per-operation exact
  semantics and must never use host arithmetic as a shortcut.

- **OPT-COPY-PROPAGATION.** Remove redundant scalar copies and block parameters.

  Acceptance: dominance, call-result materialization, effect chains, proof-term
  identity, debug provenance, and fuel attribution remain valid.

- **OPT-GVN-CSE.** Add local CSE followed by dominator-based global value
  numbering for pure, total operations.

  Acceptance: the expression key includes complete type/domain/provider
  semantics; potentially trapping, effectful, placed, atomic, or observation-
  dependent work is excluded unless its exact contract proves equivalence.

  Current slice: the exact named `GlobalValueNumbering` selection owns local
  `same-block-obligation-free-total-scalar-cse.v1` followed by cross-block
  `dominator-obligation-free-total-scalar-gvn.v1`. The first replaces a later result
  with the earliest equivalent same-block leader over the complete
  obligation-free total scalar vocabulary: literals, Boolean operations,
  integer comparisons/bitwise/widening, wrapping shifts, and wrapping or
  saturating add/subtract/multiply. Keys bind the exact policy, integer domains,
  literal payload, and operands. Only equality, bitwise and/or/xor, and
  wrapping or saturating add/multiply canonicalize swapped operands. The
  second selects the earliest outer expression in a strictly dominating block,
  independently of canonical block-roster order, and proves that leader
  dominates every rewritten use. The independent validators rebuild the key,
  reachable CFG/dominators, definitions, uses, dense effects,
  fact/place indexes, substitution, and custody accounting. Redundant
  provenance/fuel moves forward to the next co-executed node, never backward
  to the leader. Candidate v19, optimization-unit identity v10, the named v2
  pass, prephysical manifest v13, and projection v14 bind this meaning; ledger
  v4 already represents the relocation and substitution. Phi translation,
  partial redundancy elimination, and cyclic-CFG GVN remain outside this rule;
  current admitted optimization units reject control cycles.

- **OPT-DEAD-SCALAR-WORK.** Remove unused pure and total scalar operations.

  Acceptance: an unused result is insufficient when the operation may trap,
  charge a distinct semantic site, produce proof/runtime evidence, or carry an
  effect/cleanup/boundary event.

  Current slice: the exact named `DeadPureScalarElimination` selection expands
  to `dead-unused-scalar-literal-elimination.v1` and
  `dead-unused-unconditionally-total-scalar-elimination.v1`. The first removes
  only unused `BooleanConstant` and `IntegerConstant` nodes. The second has a
  closed whitelist: Boolean not/equality; integer equality/order comparisons,
  bitwise operations, and widening; wrapping shifts; and wrapping or saturating
  add/subtract/multiply. Each admitted nonliteral operation is pure,
  unconditionally total for verified typed operands, and obligation-free.
  Exact casts/arithmetic/shifts, all divide/remainder policies, calls,
  structural work, and boundary/service/control operations remain excluded.
  The independent validator binds each exact rule identity to its own closed
  shape vocabulary and reconstructs liveness, effect shape, exact
  definition/type, absence of operation-obligation references, every use, and
  relocation accounting.
  Removed work is never called unreachable: its operation provenance and fuel
  move to the next co-executed node, and every shifted later node is ledgered.
  Dense effects, definition/use sites, literal facts, places, and identity are
  rebuilt. A verified wrapping-add artifact removes the unused arithmetic and
  then its two newly dead literals at the suite fixed point, replaying every
  source/fuel site into the return. Candidate v19, optimization-unit identity
  v10, the v2 pass, prephysical manifest v13,
  and projection v14 bind this meaning; ledger v4 already represents the
  many-to-one moves. Other scalar operations remain open until their exact
  semantic and custody contracts are admitted individually.

- **OPT-PROOF-CHECK-ELISION.** Omit redundant physical checks whose exact
  obligations were already verified and whose operation semantics permit
  no-code realization.

  Acceptance: the semantic obligation and source provenance remain in the
  ledger/report. Elision never converts a `Trapping` operation into an `Exact`
  one or removes a required runtime policy event.

  Current slice: the exact named `ProofCheckElision` selection owns
  `dead-unused-proof-certified-scalar-elimination.v1`. It removes an unused
  proof-bearing scalar node only from this closed vocabulary: exact integer
  cast; exact left/right shift; exact add/subtract/multiply/divide/remainder;
  and wrapping or saturating divide/remainder. Each candidate carries the
  exact `AcceptedObligationFactIdentity` as a dedicated proof witness and the
  pass manifest records that accepted obligation as its consumed fact. The
  independent validator reconstructs the operation's obligation, result type,
  operation-reference fact, and accepted row by identity, machine, operation,
  and obligation before applying the shared dead-node custody rewrite. The
  active operation-reference index disappears with the operation, while the
  verifier-owned accepted-obligation catalog remains byte-for-byte intact as
  historical proof custody. A verified Terminal artifact projects the removed
  exact add, its manifest evidence, and its source/fuel relocation through the
  lowering boundary. Candidate v19, optimization-unit identity v10, the named
  v1 pass, prephysical manifest v13, and projection v14 bind this meaning;
  ledger v4 already represents the relocation. General check elision whose
  result remains live, runtime policy events, and physical check recognition
  remain open.

- **OPT-INITIAL-PIPELINE.** Define the canonical target-neutral schedule for
  each subset of the initial named optimizations and its bounded repetition
  rules.

  Acceptance: it reaches a deterministic fixed point, running it again changes
  neither unit nor ledger, and randomized rule-registration order cannot change
  output because registry order is canonical.

  Current slice: the supported six-family subset has the canonical
  `SparseConditionalConstantPropagation -> ControlFlowCleanup ->
  CopyPropagation -> GlobalValueNumbering -> ProofCheckElision ->
  DeadPureScalarElimination` schedule,
  with proof-enabled deletion before dead-scalar cleanup so newly dead
  obligation-free operands can disappear in the same pipeline sweep. It has
  distinct ordered pass manifests, aggregate replay evidence, per-pass budgets,
  and deterministic artifact tests. Thirty-two shuffled built-in registration
  orders produce identical full SCCP runs, and a direct second
  SCCP/CFG/copy/GVN/proof/dead-scalar sweep changes neither final unit nor the
  composed transformation ledger. Remaining to close: add canonical schedules
  and the same fixed-point evidence for each newly implemented initial family.

- Named selections now declare a closed execution phase rather than being
  grouped into an optimization level. The full root-build suite remains the
  build/cache identity, while Psi scheduling and its pre-physical receipt retain
  the exact Psi subset separately. `SelectedIncomingU12ExactAddImmediate` is
  the first selected-lowering family exposed by the canonical build vocabulary;
  it is never treated as Psi work, and native publication remains fail-closed
  until its selected-lowering execution receipt joins the full suite identity.

  Current selected-lowering slice: the compiler-owned
  `SelectedIncomingU12ExactAddImmediateToNoChangeV1` schedule derives the full
  suite and budget from upstream custody, executes only the exact
  selected-lowering projection, and retains zero or more strictly decreasing
  fold/reanalysis steps plus a mandatory validated zero-action termination
  attempt. Aggregate choice/classification/fold usage must fit the one retained
  suite budget. The one-step staging APIs keep their explicit apply-or-reject
  behavior. The named completion now has a domain-separated identity and joins
  strict homes, pre-allocation effects, the post-allocation manifest, and the
  independently replayed post-allocation machine sidecar. Its manifest field is
  separate from the ordered literal-fold ledger, so a verified zero-change run
  truthfully records completion with no invented transformation. The typed
  terminal-component compiler route now dispatches that phase from the retained
  full suite. Psi-only requests take direct strict homes and stop after
  independently validated post-allocation machine custody. Selected-lowering
  routes continue into a validated function-relative realization manifest
  binding the machine, encoding, and layout roots plus exact code-size
  statistics for both changed and verified no-change suites. The legacy native
  compiler firewall remains closed rather than feeding selected builds into its
  old backend. A validated frameless whole-function exit contract now joins
  every selected-lowering realization and the v2 manifest; compiler staging
  derives caller-saved-only unconstrained homes for that exact policy.
  Remaining to close: add general frame/call/save-restore and entry-bridge/
  hardening custody, broaden selected instruction shapes, and complete section/
  relocation, emission, final-manifest, and publication custody.

## P4 — Lowering optimizer and virtual-register form

- **OPT-ABSTRACT-LOWERING-CUT.** Make the clean Terminal-derived optimized plan
  the true input to abstract-operation lowering.

  Acceptance: no checked-tree expression table, legacy state-value simplifier,
  or source binding substitution is needed for supported Terminal slices.
  Unsupported shapes fail at a named boundary.

  Current slice: the independently validated `omega-lowering-optimizer`
  projection and opaque custody carriers are landed. The opt-in-only
  `omega-optimization-pipeline` is the clean compiler lane's sole selected
  entry, and SCCP/copy-propagation outputs lower through a dedicated optimized
  target-operation API without legacy state. The opaque staged-assignment
  carrier now also retains that complete custody through the existing bounded
  scratch-register assignment and independently checks Terminal-Psi identity,
  the domain-separated independent projection-receipt identity, native target,
  entry, exact function order, attachments, and operation provenance. It also
  retains the clean ISA-owned independently validated physical-register model
  and target-semantic constraint catalog and rejects a target/environment
  mismatch. Its `Staged` name is load-bearing: it proves cross-stage custody,
  not physical-home legality, liveness, interference, or publication fitness.
  Unsupported named families fail at registry construction; the current
  operation vocabulary is admitted by exhaustive behavioral observation,
  projection, and target lowering. The empty-selection compatibility route
  remains untouched. The typed compiler-facing physical route now replaces
  its former scratch-assignment probe with selection, liveness, ranges,
  legality, exact phase dispatch, strict homes, and post-allocation machine
  replay. The parallel staged-assignment carrier remains transitional evidence,
  not the compiler route's allocation authority. The validated pre-physical
  manifest identity survives
  every implemented selected/physical staging receipt, so a later realization
  manifest cannot silently join only the older optimizer bundle. Remaining to
  close: broaden the bounded selected/physical vocabulary and retain optimizer
  custody through frame/exit validation, machine emission, and component
  construction; then
  bind optimizer and physical identities into an explicit final realization/
  publication manifest before optimized output can be installed.

- **OPT-VIRTUAL-REGISTERS.** Change instruction selection to produce typed
  virtual registers/classes and explicit register/machine-state uses and defs.

  Acceptance: fixed ABI operands are constraints, not wholesale preassignment;
  temporary scratch needs are visible to liveness; instruction encoders receive
  only assigned physical operands later.

  Current slice: `omega-terminal-selected-instructions` owns a data-only,
  target-neutral selected CFG with typed virtual registers, exact definition
  sites, explicit operand access/class/fixed-view constraints, machine-state
  uses/defs/clobbers, source block/edge/value/operation provenance, and
  path-specific logical-fuel settlements. The separate
  `omega-terminal-target-operations-to-selected-instructions` stage produces
  and independently validates four deliberately bounded production shapes
  over one runtime Boolean parameter and a three-block conditional: leaf-local
  unsigned-i64 constants, one shared returned entry parameter, or two
  leaf-local constants followed by one proof-bearing exact add or exact
  subtract and a cleanup-free return. Both exact selected kinds retain their
  obligation and verifier-owned accepted-fact identity. Addition consumes the
  target-owned flag-transparent `add_i64` row. AArch64 subtraction consumes a
  flag-transparent three-address `SUB` row; x86-64 subtraction consumes an
  alias-safe three-address pseudo row and honestly clobbers RFLAGS rather than
  pretending general subtraction is an LEA. Both x86-64 and
  AArch64 use ISA-owned constraint rows and fixed-register view resolvers;
  compare/branch explicitly cross RFLAGS/RIP or NZCV/PC, and fixed ABI operands
  remain constraints rather than assigned homes. The opt-in orchestration
  carrier owns the complete optimized-lowering and register-environment custody
  and grants no liveness, allocation, emission, or publication authority.
  Unsupported source shapes fail closed. Remaining to close: generalize the
  selected CFG across the complete legalized operation vocabulary and retain
  cleanup, call, suspension, memory, proof, and effect frontiers before all
  selected programs can enter liveness.

- **OPT-TARGET-LEGALIZATION.** Separate target legalization from physical home
  assignment.

  Acceptance: illegal widths/shapes decompose into target-legal operations with
  complete provenance; legalization does not allocate registers or select
  stack offsets.

- **OPT-TARGET-COMBINES.** Add exact immediate folding, addressing-mode folding,
  compare/branch formation, strength reduction, and target instruction
  combines.

  Acceptance: rules declare target model dependencies and preserve trap,
  overflow, flag, memory, and effect behavior. Cross-target tests show that an
  ISA-specific rule cannot run on another target.

- **OPT-CALL-LOWERING.** Normalize internal, boundary, callback, and tail-call
  sequences before allocation.

  Acceptance: calling-plan identity, argument/result placement constraints,
  clobbers, outgoing frame requirements, provider bindings, and cleanup
  frontiers are explicit allocator inputs.

- **OPT-AGGREGATE-COPY-PLANNING.** Select scalar, vector, loop, or checked
  provider copy plans from exact size/alignment/overlap/access facts.

  Acceptance: write-only access cannot be observed; overlapping copies choose
  an overlap-safe plan; affine/linear ownership moves are not modeled as
  unrestricted byte copies.

## P5 — Register allocation and frame assignment

- **OPT-REGISTER-MODEL.** Publish declarative AArch64 and x86-64 register-unit,
  class, alias, preservation, reservation, and instruction-constraint models.

  Acceptance: models cover general-purpose, float/vector, flags/predicate, ABI,
  dispatch, metering, syscall, inline-assembly, and backend-reserved state.
  Target tests detect overlapping or omitted units.

  Current slice: independently validated physical models and the closed
  Register Constraint Catalog v1 substrate are landed. Required target-owned
  keys currently cover scalar call/return, Linux syscall, conservative inline
  assembly, and the first ordinary materialize/copy/three-address-add/
  three-address-subtract/compare/branch forms for
  the existing System V, Microsoft, AAPCS64, and Darwin conventions. The
  declarations live in clean Terminal ISA crates and are joined into a
  validated target-register environment retained by optimized staging. Both
  generic structural and ISA-semantic corruption suites are required. The ISA
  validators now reject a structurally valid but semantically altered
  same-architecture physical model before constructing canonical constraint
  rows, and fixed-register resolvers refuse noncanonical models; selected
  custody then revalidates against the exact retained environment. Physical,
  catalog, reservation-profile, and joined-environment identities cover every
  retained field with fixed-width canonical encodings; exhaustive mutation and
  both-target tests guard determinism and separation. The active baseline is
  intentionally conservative policy, not a claim that every declared overlay
  is universally active.
  Remaining to close: the rest of the ordinary instruction keys and fixed/tied
  constraints, complete integer/vector/aggregate ABI banks, feature-profile
  variants (including extended vector/floating control state), and dynamic
  backend/provider reservation closure.

- **OPT-LIVENESS.** Compute block and instruction liveness, live intervals, use
  positions, loop weights, call crossings, and fixed constraints.

  Acceptance: conditionals, loops, crash exits, calls, cleanup blocks,
  suspension frontiers, and disconnected functions have focused tests.

  Current slice: `omega-regalloc` computes deterministic reverse-fixed-point
  block and instruction liveness over the opaque validated three-block
  conditional-return carrier. It keeps typed virtual-register liveness separate
  from architectural register-unit liveness, records dense instruction/use
  positions, fixed entry/return views, exact instruction uses/defs/clobbers,
  and branch-polarity-preserving successor facts. The exact admitted selector
  now also supports a Boolean condition plus one shared unsigned-i64 entry
  parameter returned from either leaf. That shape proves a value live across
  both branch edges and a real condition/result interference pair while
  retaining distinct entry and return fixed-view sites. A separately implemented
  validator reconstructs CFG order, effects, transfers, canonical sets, and a
  domain-separated content identity. Opt-in orchestration retains this result
  only in a nested custody carrier that grants no interval, allocation,
  emission, or publication authority. The v1 analysis rejects use-def, tied,
  and early-clobber operands rather than pretending their later interference
  semantics are complete. A second independently validated artifact converts
  the facts into maximal half-open fragments within each block, exact
  polarity/edge connectors across blocks, ordered occurrence and fixed-view
  sites, separate architectural-unit fragments/actions, and canonical unordered
  virtual-register interference pairs. It never convexifies ranges across CFG
  blocks or treats layout adjacency as semantic reachability.
  A subsequent independently replayed legality artifact computes canonical
  physical-view candidates at every occupied VReg point. Candidate storage and
  write footprints cannot overlap the active reservation union, architectural
  semantic liveness, or same-phase architectural use/def/clobber actions.
  Fixed views are checked at their exact phases. The forwarded fixture exposes
  two explicit entry-to-return transition requirements (`RSI -> RAX` or
  `X1 -> X0`), while the constant fixture exposes none. This artifact grants no
  split, copy insertion, home, spill, frame, emission, or publication authority.
  Legality now directly binds an independently validated allocator-availability
  identity. The baseline admits every structurally allocatable view whose full
  storage/write footprint avoids active reservations; an explicit allowlist may
  only remove from that set and fails closed if an unconstrained point becomes
  empty. The artifact has a strict v1 codec, while legality and every dependent
  fixed-copy/home/pressure/recovery/post-allocation identities and applicable
  codecs moved to a new schema domain/version so cached custody cannot silently
  cross policies.
  A subsequent bounded home artifact accepts only transition-free plans,
  chooses the lowest stable shared legal view in first-live-point/VReg order,
  checks exact interference and complete write footprints, and is independently
  replayed. It rejects unresolved transitions, empty intersections, and pressure
  requiring a spill; it does not authorize physical emission.
  The register-home plan now also has a versioned self-authenticating codec
  binding its legality, live-range, register-environment, machine, VReg, class,
  and physical-view fields. Its strict decoder rejects malformed framing,
  invalid machine IDs, identity tampering, truncation, and trailing bytes, and
  returns only the plain plan; independent home validation remains required
  before custody can accept artifact or cache bytes.
  Both direct and post-copy home carriers now retain an independently
  reconstructed post-allocation manifest identity. The record binds the final
  selected-plan identity plus an ordered typed transformation ledger; fixed-
  view-copy and literal-fold identities cannot masquerade as each other or as
  an untransformed home plan.
  The record exposes exact assignment/view/interference statistics and marks
  frame, emission, and publication unavailable rather than inventing them.
  The exact named `LeafLocalBeforeFixedUseV1` artifact now closes the admitted
  forwarded-value transition: it binds the selected `copy_i64` key in the target
  environment identity, inserts one copy and fresh VReg per leaf fixed Use,
  preserves source IDs and semantic fuel, independently reconstructs the whole
  transformed plan, and records exact bounded work usage. A sealed interface
  accepts only the original opaque selection or this opaque validated
  transformation for complete analysis replay. Fresh liveness, ranges, and
  legality eliminate both transition rows before the unchanged strict home
  assigner runs. The unmaterialized path still rejects.

  Remaining to close: live-interval construction, loop weights, calls and call
  crossings, crashes, cleanup and suspension frontiers, disconnected
  functions, and dedicated tied/use-def/early-clobber handling. General
  liveness remains dependent on completing
  `OPT-VIRTUAL-REGISTERS` for calls, cleanup, suspension, memory, loops, and the
  rest of the legalized instruction vocabulary.

- **OPT-LINEAR-SCAN.** Implement deterministic linear-scan allocation with
  class constraints and stable tie breaks.

  Acceptance: simple functions use registers without the current modulo scratch
  cycling; allocation never assigns reserved or incompatible units; repeated
  builds are identical.

  Current slice: canonical CFG-aware range fragments, exact VReg interference,
  identity-bound register environments, and independently replayed phase-
  specific candidate legality now feed a bounded deterministic home assigner.
  Production now orders canonical half-open interval envelopes by start/VReg,
  expires the active set by exclusive end, and chooses the lowest legal view
  against only still-active exact CFG interferences and complete write/storage
  footprints. The independent validator reconstructs the same allocation with
  separate interval and active-set logic. It assigns the admitted constant
  conditional on both ISAs without modulo scratch cycling, permits exact
  mutually-exclusive reuse, and fails closed if any VReg lacks one shared legal
  view, needs a spill, or retains a fixed-view transition. The post-copy fixture
  now proves one real interfering entry pair receives distinct homes while its
  expired mutually exclusive split results reuse the same return view on both
  ISAs. This closes deterministic active expiration for the transition-free/
  spill-free base case, not general linear scan.

  A validated miniature two-register model now exercises flexible, non-fixed
  candidates directly in both production allocation and independent replay.
  Two overlapping intervals deterministically receive views 0 and 1, an
  expired third interval reuses view 0, and three pairwise-interfering
  intervals fail identically at VReg 2 with `NoCompatibleHome`. This identifies
  the exact future spill-choice boundary without pretending a spill exists.
  The production exact-add conditional now carries the ordinary three-address
  `add_i64` row from verified Terminal operations through selection, liveness,
  ranges, legality, and deterministic homes on both ISAs. Each leaf contributes
  one flexible interference pair; the allocator assigns distinct homes and
  reuses the same pair after the mutually exclusive leaf interval expires.
  Exact arithmetic policy remains explicit through the obligation and accepted
  verifier-fact identity rather than being collapsed into the physical row.
  The x86-64 stable-order fixture currently chooses `rbx` as its second home;
  this is deterministic legality evidence, not yet a callee-save/frame cost
  claim.

  The sibling production exact-subtract conditional now carries proof-bearing
  `ExactSubtractI64` through the same source-to-home vertical on both targets.
  The selected-plan identity has a distinct subtract kind and roots its
  obligation and accepted fact. The target-register environment has a named
  `subtract_i64` key in both its selected and allocation rosters. AArch64
  records no NZCV effect for ordinary `SUB`; x86-64 records the exact RFLAGS
  clobber required by its alias-safe `SUB` / `NEG; ADD` / `MOV; SUB` pseudo
  realizations. Liveness, ranges, legality, recovery replay, and deterministic
  homes therefore consume the target fact without weakening proof custody or
  silently extending the add-immediate fold to subtraction.

  Sequencing: the exact named forwarded-value copy/split base case, copy-key
  identity, fresh split-result VRegs, provenance/fuel custody, independent
  reconstruction, complete reanalysis, post-copy homes, active expiration, and
  the first real competing pair now exist. Flexible candidate ranking and the
  deterministic pressure failure are covered at allocator-core level, and the
  two-way flexible case now has a production source-to-home vertical. The named
  `SingleBlockFarthestEndThenHighestVregV1` policy now records the first
  supported local pressure point, active residents, all victims whose removal
  really recovers a legal incoming view, and the deterministic selected victim
  under an explicit work budget. Its separate replay implementation and
  versioned identity/codec fail closed on cross-block or connected pressure and
  do not weaken the existing home assigner's `NoCompatibleHome` result. Next
  the named `SelectedVictimImmediateU64EligibilityV1` policy joins that victim
  back to the sealed validated selected CFG, exact range/legality roots,
  scalar/origin/definition data, instruction provenance and logical-fuel
  anchors, and canonical future flexible uses. It positively classifies only
  the current cleanup-free non-address u64 literal source; every unsupported
  victim gets an exact no-admitted-recovery reason, never an inferred spill.
  The classification identity and replay grant no strategy, code mutation,
  fuel movement, storage, frame, emission, or publication authority. The
  separately validated allocator-availability boundary now
  supplies that production pressure vertical without misusing reservation
  overlays: retaining only `rdi` on x86-64 or `x0` on AArch64 makes the second
  exact-add literal the deterministic incoming victim on both targets, while
  fixed views outside the flexible allowlist remain exact and legal. The victim
  is positively classified as the immediate-u64 literal `8`. The separate
  `SelectedIncomingU12ExactAddImmediateV1` policy now consumes only that exact
  incoming, single-use, immediately adjacent unsigned-u12 case. It replaces
  `MaterializeI64 + ExactAddI64` with proof-bearing
  `ExactAddI64Immediate`, preserves both operations and both logical-fuel
  settlements exactly once, retains the add obligation and accepted fact,
  removes the victim, and globally redensifies later instruction/VReg IDs.
  Its strict recipe codec roots the complete input custody and transformed
  selected identity; replay reconstructs the full CFG before granting a sealed
  reanalysis carrier. Orchestration now owns an append-only chain: the first
  and every subsequent fold are different explicit calls, each step retains
  choice/classification/fold plus wholly fresh liveness/range/legality, and the
  entire prefix is replayed before append. No-action requests reject. With a
  sole flexible `rax`/`x0` view, two separately requested folds close both leaf
  pressure points; the final home carrier derives both ordered `LiteralFold`
  manifest entries from custody and reaches homes on both targets. These manual
  APIs contain no implicit loop. The separately named build-selectable suite
  owns its explicit fixed-point schedule and verified no-change termination;
  neither route grants general rematerialization, spill, frame, or emission
  authority.
  Default staging retains every environment-allocatable view and preserves the
  former spill-free homes. Selected-value ownership/proof custody and target
  frame policy must still join before any victim can become a typed spill or
  reload.
  Provider/runtime reservation requirements must either join the active profile
  or fail closed.

- **OPT-INTERVAL-SPLITTING.** Split live ranges around fixed uses, calls, high-
  pressure regions, and profitable rematerialization points.

  Acceptance: split copies have complete provenance and are visible to later
  coalescing/peephole passes. Address-stable values are not illegally split
  across changing homes.

  Current boundary: `LeafLocalBeforeFixedUseV1` is only the exact scalar-u64
  entry-to-leaf-return base case. It does not close general fixed-use, call,
  pressure, rematerialization, or address-stability splitting.

- **OPT-SPILLS-RELOADS.** Insert typed spills/reloads and rematerialize cheap
  constants/addresses.

  Acceptance: spill code obeys effect and trap ordering, retains source value
  identity, and cannot use placed/volatile memory as a private spill location.

  Current boundary: `SingleBlockFarthestEndThenHighestVregV1` is an explicit
  structural recovery-victim policy only. It considers the incoming VReg and
  exactly those active residents whose hypothetical removal exposes a legal
  incoming view, ranks farthest exclusive end then highest VReg ID, and records
  no memory/rematerialization choice or placement. This closes deterministic
  victim selection, not spill materialization.

  `SelectedVictimImmediateU64EligibilityV1` now closes only the next analysis
  question: whether that exact victim is a validated flexible-use u64 literal
  rematerialization candidate. It records the incoming-versus-active role,
  original instruction/value/provenance/fuel anchors, and future use demands.
  Eligibility does not select, move, duplicate, or charge an instruction.

  `SelectedIncomingU12ExactAddImmediateV1` closes one physical-form alternative
  only: an incoming unsigned-u12 literal, uniquely materialized immediately
  before its sole flexible right use by a proof-bearing exact add. It folds the
  literal into the target-owned immediate row and preserves semantic/fuel
  custody under complete replay and reanalysis. It is not a general spill or
  rematerialization mechanism and never runs unless explicitly invoked.

- **OPT-STACK-SLOTS.** Assign aligned frame slots with lifetime-based reuse,
  outgoing-call areas, ABI shadow space/red-zone policy, dynamic restrictions,
  and deterministic layout.

  Acceptance: overlapping live values never share a slot; stable-address loans
  keep a stable slot; frame size/alignment and unwind/entry requirements are
  validated for every target.

- **OPT-COALESCING.** Add conservative copy and phi/block-parameter coalescing.

  Acceptance: coalescing never violates register constraints, merges distinct
  address identities, or obscures cleanup/debug recovery requirements.

- **OPT-REGALLOC-VERIFIER.** Independently replay liveness, register units,
  clobbers, spills, stack slots, frame layout, and state footprints.

  Acceptance: targeted corruption tests change one assignment, clobber, spill,
  slot, or frame field at a time and are rejected before machine emission.

- **OPT-ALLOCATOR-ADAPTERS.** Feed the same allocator core from clean Terminal
  target operations and, where it avoids duplicated disposable work, the legacy
  target-operation representation.

  Acceptance: adapters preserve each lane's semantic/provenance roots and
  contain no allocation policy. Durable Terminal functionality does not depend
  on the legacy adapter.

## P6 — Machine optimizer

- **OPT-MACHINE-EFFECTS.** Make symbolic machine instructions declare all
  physical register units, flags, memory effects, traps, barriers, calls,
  cleanup/fuel provenance, and latency/size alternatives.

  Acceptance: the declaration is sufficient for independent scheduling,
  peephole, and state-footprint validation. Encoders reject undeclared implicit
  state.

  Current slice: `omega-terminal-selected-instructions` now owns a closed v1
  target machine-effect catalog vocabulary, and the two clean Terminal ISA
  owners publish and semantically validate catalogs for all eight currently
  admitted selected kinds. Each declaration binds its exact register-
  constraint key, explicit no-memory/no-trap/no-call/no-cleanup status, control
  barrier status, stable target-alternative keys, and honest size/latency
  knowledge. AArch64 `MaterializeI64` is encoder-resolved rather than assumed
  to be one instruction. x86 branch displacement and register-dependent forms
  remain encoder-resolved. x86 exact subtraction publishes four ordered,
  applicability-qualified pseudo alternatives and retains the constraint
  row's RFLAGS clobber; AArch64 publishes one flag-transparent SUB alternative.
  Each alternative now also declares its exact encoded realization separately
  from the selected instruction's semantic/ABI custody: external operand
  dependencies, implicit unit uses/defs/clobbers, memory, stack adjustment,
  architectural-fault possibility, and control behavior. This makes the x86
  XOR-zero subtraction alternative honestly independent of its semantic input
  operands and makes return instructions honest about hardware state without
  pretending the returned value is encoded in `RET`.

  The new `omega-machine-optimizer` independently computes and replays an
  immutable pre-allocation sidecar from the sealed validated selected-analysis
  boundary. Every row retains selected instruction kind and payload, constraint
  key, exact architectural unit uses/defs/clobbers, proof/fuel provenance,
  effect declarations, and all legal target alternatives. Its identity binds
  the selected plan, optimization unit, fuel schedule, native target,
  register-environment/catalog roots, complete ordered rows, and counts. Opt-in
  orchestration can stage the sidecar by borrow over original selection,
  fixed-view-copy output, the final output of an explicit literal-fold
  sequence, or a completed named selected-lowering suite. Each route
  independently revalidates its exact source custody and binds the transformed
  selected identity; the named-suite path retains positive completion evidence
  even when its ordered change ledger is empty. No pre-transformation analysis
  fact crosses that boundary.

  The sidecar also has a strict, versioned, self-authenticating binary codec
  (currently v3 after adding encoded-realization content).
  Decoding rejects wrong framing or version, truncated or trailing data,
  unknown closed-vocabulary tags, and any identity/content mismatch. It grants
  no rewrite, home, emission, or publication authority.

  A second immutable post-allocation sidecar now joins the exact selected form,
  pre-allocation effects, ranges, legality, physical homes, validated post-
  allocation manifest, register environment, physical model, constraint
  catalog, and machine-effect catalog. Each operand retains its VReg, class,
  physical view, access, storage units, exact read/write units, and write
  semantics. Each instruction retains separate implicit effects plus canonical
  complete use/def/clobber sets. The resolver requires exactly one applicable
  catalog alternative; zero or multiple matches fail closed rather than
  introducing a hidden cost policy. Independent reconstruction rejects root,
  chosen-alternative, physical-view, and unit-footprint corruption. Borrowed
  orchestration covers ordinary homes, fixed-view-copy homes, literal-fold
  homes, and named selected-lowering completion homes without transferring
  emission authority.

  x86 flag-transparent three-address addition uses one always-applicable LEA
  alternative for allocator-produced GPR64 homes. R12 is a valid SIB index
  when `REX.X=1`; only reserved RSP has the no-index encoding, so R12+R12 must
  remain legal rather than falling back to an undeclared flag-writing ADD.

  Each clean Terminal ISA owner now also has a `selected_form_encoding` module
  for layout-independent scalar forms. The modules resolve canonical physical
  views through target-owned architectural-name tables, reject foreign,
  reserved, or non-GPR64 views, emit only one versioned canonical byte form,
  and decode those bytes independently before returning a validated fragment.
  AArch64 variant 0 materialization remains the explicit zero-seeded `MOVZ`
  plus ascending nonzero `MOVK` sequence; a shortest-`MOVN` policy must receive
  a new named/versioned rule rather than silently changing this alternative.
  x86 retains exact 10-byte materialization, deterministic LEA base/index
  orientation, all four subtraction alias forms, and the admitted U12-only
  immediate boundary.

  Opt-in orchestration joins those fragments to the exact selected plan and
  post-allocation sidecar. Its immutable v2 identity binds both roots, every
  instruction ID and chosen alternative, the canonical bytes, the decoded
  physical footprint, and the complete encoded-realization effects. It compares
  byte counts with each target size declaration and requires the target decoder
  to reproduce the catalog alternative's realization exactly. x86-64 near
  return is verified as canonical `C3`, with RSP use/def, RIP def, an eight-byte
  activation-stack read/pop, possible architectural fault, and return control.
  AArch64 `RET X30` is verified as canonical `C0 03 5F D6`, with X30 use, PC
  def, possible architectural fault, unchanged stack, and no encoded X0 read.
  The selected return value remains separate ABI custody in RAX/X0. Only
  conditional branches remain explicit deferred rows in this pre-layout
  artifact because they require resolved layout.

  A sibling immutable v1 function-relative layout artifact now resolves that
  deferral under the exact named required-stage policy
  `EntryThenZeroFallthroughThenNonzeroV1`. It independently reorders the
  admitted three-block diamond as entry, zero successor, nonzero successor;
  proves the zero edge is the immediate fallthrough; retains both exact Psi
  edge/block identities and offsets; and emits a direct nonzero branch to the
  remaining successor. x86-64 uses a deterministic six-byte `JNE rel32` whose
  signed displacement is measured from instruction end. AArch64 uses fixed
  four-byte `B.NE imm19` whose aligned signed displacement is measured from the
  branch word. Both ISA owners independently decode opcode, predicate, and
  displacement before returning effects, and orchestration requires those
  effects and sizes to match the chosen machine alternative. The layout
  identity binds selected, post-allocation, and pre-layout roots plus the target,
  named policy, complete function/block/instruction spans, exact bytes,
  successor custody, displacement, and decoded effects. It retains fragments;
  it creates no code section, symbol, object relocation, executable span, or
  publication authority.

  A selected-lowering-only function-relative realization carrier now owns that
  validated layout together with its strict homes, post-allocation machine, and
  pre-layout encoding. Its immutable v2 manifest binds the complete named suite,
  exact selected-lowering subset and completion, pre-physical and post-
  allocation manifests, final selected CFG, pre-/post-allocation machine roots,
  both encoding/layout roots, target, and named layout policy. Statistics are
  derived from the retained layout rather than supplied by callers. Its strict
  self-authenticating codec and independent custody replay reject detached or
  corrupted roots. A target-neutral whole-function exit artifact now joins the
  same roots and the target owner's exact System V AMD64, Microsoft x64,
  AAPCS64, or Darwin AAPCS64 preservation convention. Its named frameless-leaf
  policy requires caller-saved unconstrained homes, no non-return stack/memory
  effects, no callee-saved definitions or clobbers, preserved X30 on AArch64,
  exact RAX/X0 ABI result custody, and exact canonical return bytes/effects for
  every Psi return edge. It retains the external aligned-stack/return-state
  assumption and possible architectural return faults rather than inventing
  stronger facts. The v2 manifest binds this contract identity. Frame,
  emission, sections, symbols, relocations, executable image, installation, and
  publication remain explicitly unavailable.

  x86 short-branch selection is deliberately not hidden inside the baseline.
  A future `X86RelaxConditionalBranchesToRel8V1` suite may monotonically shrink
  validated near branches to `rel8` under its own fixed-point work accounting
  and replay receipt.

  Remaining to close: complete memory/trap/call/cleanup vocabularies as
  selected IR admits them, general CFG layout and non-fallthrough terminator
  bundles, general framed/calling exit policies with save/restore and unwind
  evidence, authoritative entry-bridge and enabled-hardening identities,
  whole-program span/relocation validation, and publication-side enforcement of
  the independent encoding receipt.

- **OPT-PRE-RA-MACHINE.** Add machine copy propagation, cheap rematerialization
  hints, and instruction-alternative selection before allocation.

  Acceptance: these rules operate on virtual registers and produce allocator-
  visible constraints; they do not assign physical scratch registers.

- **OPT-SCHEDULER.** Implement deterministic dependency- and pressure-aware
  local scheduling.

  Acceptance: data, flags, memory, effects, traps, fuel, placed/atomic order,
  and explicit semantic safe-point barriers are preserved. A target without a
  latency model uses a stable baseline schedule.

- **OPT-POST-RA-PEEPHOLES.** Remove redundant moves/spills/reloads and add small
  target peephole combines after allocation.

  Acceptance: each rule has a concrete physical-state validator; no rule relies
  on undefined flags, partial-register folklore, or final branch displacement.

- **OPT-BLOCK-LAYOUT.** Select deterministic function/block layout and
  fallthrough edges from static or admitted profile weights.

  Acceptance: layout changes no semantic edge identity; profile absence has a
  deterministic baseline; final short/long encoding remains the encoder's job.

- **OPT-MACHINE-VALIDATOR.** Replay symbolic instruction legality, control-flow
  targets, state footprints, frame accesses, provenance, and allocator output
  before encoding.

  Acceptance: no optimized byte sequence is published solely because an ISA
  encoder accepted it.

## P7 — Proof-, ownership-, and state-aware optimizations

- **OPT-PATH-ALIAS.** Derive exact disjointness and non-interference from loan
  occurrences, structural paths, proven dynamic-index relations, access modes,
  and physical footprints.

  Acceptance: authority alone never proves disjointness; proof compatibility
  never invents missing resource rows; collection-wide mutation remains
  conservative where the checked summary is collection-wide.

- **OPT-MEMORY-SSA.** Promote ordinary local memory to SSA and model remaining
  memory with region/path version tokens.

  Acceptance: address escape/stability, placed/external observation, atomics,
  boundaries, calls, and cleanup partition memory into honest barriers.

- **OPT-LOAD-STORE.** Add load forwarding, redundant-load elimination, dead-
  store elimination, and store sinking for ordinary memory.

  Acceptance: every rewrite cites exact alias/effect facts; write-only borrows
  cannot authorize a read; calls with unknown reach block the rewrite.

- **OPT-SROA-MOVE-ELISION.** Add scalar replacement, aggregate promotion,
  return-place forwarding, copy elision, and in-place transfer.

  Acceptance: field relevance, invariant windows, content conservation,
  multiplicity, cleanup ownership, and address stability remain correct.

- **OPT-CLEANUP-OPTIMIZATION.** Share identical cleanup suffixes, eliminate
  checked no-code affine discards from physical code, and sink cleanup only
  across legal operations.

  Acceptance: semantic cleanup order and exact claim frontier remain in the
  ledger; a linear value never becomes discardable through optimization.

- **OPT-STATE-MACHINES.** Add state fusion, transition threading, dispatch
  bypass, acyclic-region direct lowering, and proven unreachable-state removal.

  Acceptance: state/edge provenance, transition arguments, progress facts,
  safe points, and task activation semantics remain reconstructible.

- **OPT-INLINING-SPECIALIZATION.** Add whole-program inlining, provider
  devirtualization, constant/type/domain specialization, and tail calls.

  Acceptance: recursion/termination checks, service reach, crash routes,
  provider installation identity, code-growth budget, and proof dependencies
  are recomputed. Package/component boundaries honor the separate-compilation
  contract.

- **OPT-LOOPS.** Add induction analysis, loop-invariant code motion, strength
  reduction, unrolling, and check hoisting/elision.

  Acceptance: exact arithmetic policy, loop-carried ownership, termination
  measure, effects, safe points, and code-growth limits are preserved.

- **OPT-VECTORIZATION.** Add proof/alias-aware straight-line and loop
  vectorization.

  Acceptance: vector lanes reproduce scalar operation semantics including
  integer policy and exact float rounding; tails are complete; alignment and
  non-alias facts are explicit; no vectorization crosses an observable ordering
  or suspension boundary.

## P8 — Search and ML extensibility

- **OPT-DECISION-SCHEMA.** Externalize meaningful heuristic choices as
  versioned decision points with canonical features and legal action sets.

  Acceptance: baseline output is unchanged when decisions are merely recorded;
  raw paths, authored names, pointer addresses, arena order, and debug strings
  are absent from features.

- **OPT-DECISION-REPLAY.** Replay a canonical external decision log.

  Acceptance: mismatched source/selection/target/rule/cost-model identity,
  missing decisions, duplicate decisions, or illegal actions reject. Replayed
  candidates still pass every normal validator.

- **OPT-COST-MODEL-INTERFACE.** Define non-authoritative size, latency,
  throughput, pressure, and target-resource estimates.

  Acceptance: a missing or deliberately wrong cost model can make code slower
  but cannot make invalid code publish. Model identity and version are retained.

- **OPT-WORKLOAD-PROFILES.** Define bounded, content-addressed workload/profile
  inputs with explicit build custody.

  Acceptance: profile capture is outside ordinary compilation; replay is
  deterministic; profile absence is supported; dependency packages cannot
  smuggle root optimization selections.

- **OPT-TRAINING-RECORDS.** Export decision input, legal candidates, selected
  action, validator result, realization identity, and measured outcomes.

  Acceptance: the schema is versioned and reproducible, distinguishes rejected
  candidates from slow valid candidates, and contains no correctness verdict
  supplied by a model.

- **OPT-OFFLINE-SEARCH.** Build an offline search/autotuning driver that invokes
  the compiler as a deterministic `decisions -> validated artifact` function.

  Acceptance: search cannot bypass compilation/publication validation; only a
  fixed accepted decision log enters a build; no search engine or model is a
  runtime dependency.

- **OPT-LEARNED-POLICY-EXPERIMENT.** Permit a learned policy to rank already
  legal candidates offline.

  Acceptance: the baseline model-free compiler remains complete and supported;
  the experiment enables no optimization implicitly and has no TCB role. Arbitrary model-
  generated rewrites remain fenced until their general equivalence certificates
  are independently checkable.

## P9 — Test matrix, stabilization, and rollout

- **OPT-DISABLED-CORPUS.** Run every existing pass/fail/run canary and sample
  without opt-in through the default-off firewall.

  Acceptance: source acceptance, diagnostics, interpreter behavior, native
  observations, and requested output kind match the pre-optimizer baseline.

- **OPT-CANARY-CORPUS.** Add focused `canaries/pass/optimizer`,
  `canaries/fail/optimizer`, and executable differential cases. Every opt-in
  canary owns a `build.omg` enabling the exact optimization(s) it exercises.

  Acceptance: the corpus covers each rule family plus float, trap, atomic,
  placed-memory, boundary, provider, cleanup, linear/affine, suspension,
  termination, fuel, and allocator barriers.

- **OPT-METAMORPHIC-CORPUS.** Add deterministic rebuild, idempotence,
  fixed-point, equivalent-source-shape, pass-order perturbation, and corrupted-
  ledger tests.

  Acceptance: changes in parallel worker count and auxiliary-report policy do
  not change decisions or bytes.

- **OPT-TARGET-CORPUS.** Exercise x86-64 and AArch64 ABI, register pressure,
  calls, syscalls, host boundaries, inline assembly, object/image, and direct
  execution paths.

  Acceptance: each supported OS/architecture combination has allocator and
  machine-validation coverage, not merely successful encoding.

- **OPT-BENCHMARKS.** Establish compile-time, peak-memory, code-size, runtime,
  and spill/frame benchmarks over small kernels and representative applications.

  Acceptance: results are versioned non-authoritative evidence; compile work
  budgets prevent pathological inputs from producing unbounded optimization
  time.

- **OPT-EXPERIMENTAL-RELEASE-GATE.** Publish each initially experimental named
  optimization only when its transformation class has an independent
  validator, full provenance, deterministic identity, target coverage, and
  differential corpus.

  Acceptance: experimental release notes state the exact supported Terminal
  Psi vocabulary and targets. Unsupported inputs fail clearly and install no
  output.

- **OPT-PROMOTION-DECISION.** Decide separately whether any named optimization
  should become implicit by default, and only after sustained optimized-path
  assurance and empty-set compatibility evidence.

  Acceptance: each promotion is an explicit owner decision with that
  optimization's stable contract, cache/artifact migration plan, rollback
  strategy, and consumer impact audit. Until a specific promotion closes, no
  consumer receives that optimization unless its root `build.omg` explicitly
  enables it. No broad level is promoted.

## Deferred questions that do not block P0-P3

- Whether the first general equivalence backend uses only proof-kernel
  derivations, a small separately assured translation validator, or both.
- Whether compile budgets are per named optimization, one orthogonal build
  resource limit, or compiler-owned fixed bounds.
- Whether profile-guided specialization is itself a named optimization plus an
  explicit workload-profile input, or stays an offline fixed decision-log
  input.
- When the clean Terminal and legacy backend representations can converge
  without creating a second portable IR.
- Which learned policy family, if any, demonstrates enough value to retain
  after the deterministic decision/replay seam exists.

None of these questions permits ambient fast math, optimizer-selected language
semantics, a mutable canonical Terminal Psi artifact, or default enablement
during the experimental phase.
