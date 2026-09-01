# Optimizer Tasks

This is the executable work list. Architecture and rationale live in
[`wiki/design_briefs/optimizer_architecture.md`](wiki/design_briefs/optimizer_architecture.md).
Git history is the completed-milestone archive; completed work is summarized
here instead of repeated chronologically.

Selections are exact names. Do not add `O1`/`O2`/`O3`, `debug`/`release`, or
another broad alias.

Status: `[x]` complete, `[>]` active, `[ ]` pending, `[?]` owner language
decision. Only true language-semantic questions belong in
[`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md).

## Now

1. [x] Restore the source-navigation contract across rule owners, lowering,
   compiler hooks, and optimized carriers. The governed boundary satisfies the
   hard entrance/leaf limits with no exceptions.
2. [x] Split the fixed-view-copy artifact architecture from v4 through v6. V5
   introduced `SelectedInstructionPlan::structural_unit_functions`; v6 retains
   structural-call proof and crash rows. V4 remains byte-stable, while v4/v5
   decode absent fields as empty.
3. [x] Add exact wrapping add, subtract, and multiply parameter translation
   families as separate catalog rows and semantic leaves.
4. [x] Repair the four stale optimization-pipeline proof fixtures whose exact
   add/subtract goals are still represented by `KernelDerived(Truth)`. Use
   checked certificate-derived proofs; do not weaken proof admission.
5. [>] Finish workspace validation and rollout canaries before promoting any
   rule beyond explicit opt-in. The authoritative frozen-tree command is
   `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo test --workspace
   --no-fail-fast`; `--all-targets` is not the repository gate because it omits
   doctests. The post-rebase audit completed with 23 failing test targets. Its
   optimizer boundary was already green for the organization guard, all 67
   build-configuration cases (one intentional ignore), descriptor lock, six
   output-tree canaries, eight call-acknowledgement canaries, the 20 exact-name
   opt-in cases, the 163-test optimization pipeline, and Terminal's 128 source
   plus four call canaries. Subsequent focused repairs made layering 80/80,
   run-to-abstract projection 48/48, build-time evaluation 4/4, headless GUI
   1/1, no-selection native publication 5/5, and current Terminal format 43 /
   vocabulary 46 provider, codec-ledger, publication, and image-fingerprint
   canaries green. The reviewed no-selection goldens changed only semantic and
   proof sections; object and image bytes stayed identical on all four targets.
   The native differential admitted-provider test now consumes the current
   tagged boundary-execution carrier and is green 1/1, so it no longer blocks
   compilation of the remaining workspace gate. The runnable Terminal Psi
   differential target now uses the same tagged carrier for all six provider
   settlements and is green 7/7 across O0/O1 realization, deterministic image
   replay, and provider-requirement rejection.
   The latest excluded-target continuation after upstream movement identifies
   14 failing targets. The broad 1,258-case language canary is 114 pass / 1,144
   fail and native-filesystem is 0/89; repeated-runtime, recast-view, sample,
   service-contract, subslice, and wire-plan targets share the dominant
   `attached Unit closure is missing a checked transitive machine plan`
   diagnostic. The producer-to-consumer audit now shows that this is primarily
   an executable-coverage gap: Unit-effect planning admits only one-state
   machines, structural Unit control cannot carry effect operations, and the
   consumer reports the same transitive-plan error when the root itself was
   omitted. The smaller dispatch-order regression is repaired: checked-to-
   Terminal lowering now returns the exact selected plan family as typed route
   data, and static-requirement evidence only influences closure after the
   attached-Unit fallback is genuinely selected. The first composed per-state
   Unit slice is now end-to-end: one Boolean conditional root selects two
   scalar-only boundary-call-and-Unit-return leaves, the checked carrier is
   pruned atomically, the exact boundary/service/contract identities are
   rejoined during lowering, Terminal verification and codec replay pass, and
   deleting the checked boundary fails closed. The new lowering route has a
   23-line coordinating entrance over named `admission`, `catalogs`, and
   `emission` leaves rather than another monolithic producer. The next bounded
   rung is also complete: the typed producer is now a 22-line entrance over
   `topology`, `guards`, `leaves`, and `assembly`; exact closed integer-literal
   guards fold to checked Boolean constants; implicit borrowed `self` remains
   attachment context; and one provider-backed attachment field lowers through
   shared requirement validation into exact `ProviderAttachment` roots. The
   checked producer and Terminal verifier/codec regression are green. The
   composed carrier now also admits exactly one qualification-free owned
   linear whole-root parameter: both Boolean arms transfer their distinct
   checked state-entry aliases, each bodyless attached-Unit boundary leaf
   consumes its alias with a matching completion receipt, and independent
   checked-to-Terminal replay binds all three source claim identities to one
   Terminal claim. Corrupting either edge transfer, a leaf entry claim, the
   completion receipt, the consume event, or the boundary attachment fails
   closed; Terminal verification and codec replay remain green. Producer and
   consumer retain 23- and 24-line entrances over named topology/custody and
   admission/custody/catalog/emission rungs. The real
   `runtime_free_const_exit` compiler canary now crosses this formerly missing
   Terminal route and stops later on the documented host-realization fence:
   only Linux `exit_process(i32)` currently has a closed compiler-intrinsic
   execution identity, while the local macOS plan has none. Parameterless
   internal Unit-call leaves are now complete as the next exact family: both
   branches can call one checked qualification-free empty-body target, the
   producer retains the composed plan only while that ordinary target plan
   survives pruning, and lowering independently rejoins its state, contract,
   and reach before emitting one deduplicated in-module machine. Target-state,
   contract-fingerprint, and missing-target-plan corruption fail closed;
   Terminal verification and codec replay pass. The consumer entrance names
   `internal_calls` as a distinct rung. The first larger acyclic graph is now
   complete: an exact scalar-only entry jump forwards its Boolean argument to
   a distinct dispatch state before the existing conditional effect frontier.
   Production retains the checked scalar edge map; independent lowering
   rebuilds a four-block Terminal graph with distinct entry and dispatch
   values, and scalar-map or target-state corruption fails closed before
   verifier/codec publication. The 27- and 29-line stage entrances explicitly
   name `prefixed_control`; its consumer route descends through a 15-line
   entrance into independent `admission` and `emission` rungs. Internal target
   widening is also underway: a qualification-free, parameterless target may
   now contain exactly one internal Unit call before returning. Admission
   independently rejoins the nested flow coordinate, target state, contract,
   and service reach, retains its transitive empty target, rejects a missing or
   corrupted target plan, and emits one deduplicated three-machine closure that
   passes Terminal verification and codec replay. Recursive admission is now
   proven by a depth-two relay canary: every node has the same exact one-call
   shape, each target is independently rejoined and emitted once, and the
   resulting four-machine module verifies and survives codec replay. Thus the
   admitted target closure is any finite acyclic chain of those exact nodes,
   not a hard-coded depth. `internal_calls` remains a 6-line taxonomy entrance
   exposing separate `admission` and `emission` rungs. Scalar control prefixes
   are no longer depth-coded either: the producer admits a finite ordered chain
   of exact Boolean position-0 pass-through jumps before the conditional
   frontier, and lowering independently reconstructs distinct block parameters
   and dense identities for the whole chain. A two-prefix canary covers both
   boundary leaves and deduplicated internal-call leaves; corrupting the second
   edge fails closed, and verifier/codec replay pass. The producer and consumer
   entrances remain 27 and 29 lines, while their `prefixed_control` leaves are
   248, 131, and 186 lines. Preparation for a second conditional frontier is
   complete at the shared leaf boundary: provider-requirement collection and
   checked-to-Terminal leaf-target admission now consume honest slices rather
   than two-element arrays, while the existing whole-root linear custody proof
   remains deliberately exact to two arms. The second conditional frontier is
   now complete as its own `nested_control` route: an outer guard retains two
   Boolean machine inputs, transfers the second as a distinct inner-dispatch
   block parameter on one arm, and selects an outer leaf on the other; the
   inner guard selects two more leaves. Producer and consumer independently
   rejoin all four edges and three effect leaves. Boundary and deduplicated
   internal-call leaf canaries verify and replay through the codec, while
   outer-handoff and inner-target corruption fail closed. The stage entrances
   remain 29 and 30 lines. The exact two-frontier shape has now become a finite
   general acyclic conditional-graph carrier rather than another collection of
   topology-specific routes. Controls and effect leaves are classified from
   checked state shape; every Boolean guard and full scalar successor map is
   retained by state identity, and a reachability/active-stack walk rejects
   unreachable states and cycles. A depth-three right-deep canary still proves
   two-value, one-value, and empty handoffs, while a balanced three-control
   canary proves argument-bearing edges on both arms and two predecessors
   converging on one leaf emitted exactly once; a four-state canary locks the
   smallest non-prefix graph routed into this family. Independent lowering
   rejoins each guard, transition argument expression, and structural-cleanup
   target against checked facts before dynamically assigning block parameters
   and identities. Scalar reordering, guard or target corruption, and forged
   convergence fail closed; verifier and codec replay pass. An exact
   qualification-free operation prefix is now admitted without adding a route:
   a control may make a finite ordered sequence of parameterless internal Unit
   calls before its two transitions. Checked coordinates shift both guard and
   edge ordinals, target admission rejoins every ordinary target plan and
   transitive closure, and Terminal emission preserves call-before-branch
   source order. One- and two-call canaries pass verifier and codec replay;
   coordinate drift and operation reordering reject. Producer and consumer
   each name a small `operations` rung. Exact parameterless boundary calls may
   now occupy the same prefix: production admits their checked boundary and
   empty signature, lowering independently rejoins the call target and records
   the source-call occurrence, and emission appends the call before the guard
   without manufacturing another block. Verifier/codec replay and coordinate
   corruption canaries pass. The producer descends through an 18-line
   `nested_control` entrance into 330-line `topology`, 60-line `operations`,
   and 91-line `assembly` rungs; the 16-line consumer entrance retains separate
   303-line `admission`, 75-line `operations`, and 238-line `emission` rungs. No
   topology-specific sibling was added. Provider-backed control prefixes now
   cross the same route. Checked scalar-fact construction skips the implicit
   `self` target parameter while retaining each explicit argument's raw target
   position; producer and consumer therefore rejoin an implicit-`self` Boolean
   handoff without confusing source positions with dense scalar indices.
   Provider discovery joins executable prefix operations by their exact call
   coordinates, leaving named-transition call facts in topology custody. A
   provider prefix plus provider-backed leaves verifies and codec-replays;
   deleting either the scalar row or provider requirements rejects. A smaller
   declaration-custody blocker is also closed: checked wire-schema
   `encode`/`decode` statements now finalize as distinct compiler-owned
   intrinsics, while their separately retained schema/type selections own
   nominal authority. The struct-literal String-field canary therefore passes
   checked package admission and now stops honestly at the remaining attached-
   Unit executable-coverage gap. A recurring custody class still leaves
   `CheckedStructLiteralType`, `CheckedOperator`, or compiler-derived member
   access unresolved. Independent failures remain in the legacy `Pair` layout
   fixture, one generic erased-record instance, nominal-affine
   lowering (stack overflow), and the compiler/compilation-report doctests.
   Repair the Unit plan lane first, then authored checked selections, then the
   isolated fixtures and docs. Run the full gate again only after that coherent
   repair batch; no result here permits implicit optimizer enablement.
6. [x] Finish the exact-rule navigation refactor across Psi passes. Copy
   propagation and dead-scalar elimination now use exact named leaves, the
   dead-scalar entrance is a 31-line ordered roster, and the guard rejects new
   production `rule.rs`/`rules.rs` catch-alls. Proof-check elision now has a
   63-line ordered entrance, exact leaves own all six operation classifiers,
   its shared identity-rewrite group is split into model/proposal/typed-literal
   rungs, and a guard forbids restoring either the 552-line flat hub or parent
   glob imports in that family. Its generic node-elision accounting now lives
   under pass support instead of GVN. Control-flow cleanup's former 566-line
   mixed empty-block leaf is now a seven-file semantic ladder: exact linear and
   path-qualified rule leaves share only binding composition and ownership
   identity, while retaining separate accounting leaves. The remaining
   block-merging leaves now own explicit imports as well, and the guard forbids
   parent-glob regression across that subtree. Constant conditionals, shared
   jump fusion, unreachable-machine pruning, and shared merge-ownership
   custody now own explicit dependencies too, shrinking the control-flow
   entrance to its module map and seven-row ordered roster. A family-wide guard
   keeps all control-flow leaves independent of inherited parent globs. The GVN
   expression-key group now owns its closed model and exact total,
   proof-certified, and compatible-policy classifiers without inheriting the
   pass entrance namespace. GVN now has no production parent-glob imports:
   exact-purity admission lives in a named pass-level leaf, join-parameter
   provenance accounting lives with phi translation, and all three traversal
   families name their dependencies directly. The local obligation-free rule
   consumes the shared effect query instead of duplicating it, and a
   family-wide guard rejects regression. SCCP Boolean-result constant
   evaluation now descends through five exact rule entrances over separate
   model, typed-evaluation, and proposal leaves. Exact integer cast, widen, and
   bitwise-not now own three more executable entrances: cast retains an
   adjacent proof-evidence join, while the two unary rules share only a closed
   operation model and proposal traversal under a small group map. The guard
   pins all three entrances and retires the former mixed flat leaves. All 22
   binary integer identities now have exact executable entrances over shared
   closed shape, typed-evaluation, proposal, and witness rungs. A roster canary
   pins their identities and safety classes at positions 0–18 and 22–24, and
   the guard retires the former aggregate definition leaves.

## P0 — Source navigation and taxonomy

- [x] Define the strict entrance/catalog/exact-leaf contract and map it to the
  useful Squalr registry -> plan -> dispatcher pattern.
- [x] Replace the historical 651-line source-organization narrative with a
  concise current contract, honest debt audit, stage inventory, and refactor
  triggers.
- [x] Compact this file from a milestone ledger into an executable checklist.
- [x] Split the 1,254-line source-organization guard into a tiny audit entrance
  over `inventory`, `bounds`, `entrances`, `catalogs`, and `retired_paths`.
  One six-row stage descriptor now carries entrance, catalog, coordination
  marker, and next rungs together while preserving the single aggregated test.
- [x] Make every executable Psi pass `mod.rs` a meaningful pass entrance. The
  stage-wide `rules/catalog.rs` remains the sole exact-selection enable/disable
  table; each pass entrance now owns its visible local rule order instead of
  re-exporting a sibling catalog function.
- [x] Replace copy propagation's generic `rule.rs` and dead-scalar
  elimination's mixed `rules.rs` with exact named rule leaves. Keep shared
  family, proposal, shape, and accounting mechanics at their honest common
  ancestor, and reject generic Psi production rule leaves in the architecture
  guard.
- [x] Split the 967-line optimization manifest into a 37-line wire-family
  entrance over decision-v5, pass-v1, work usage, fact reference, framing, and
  error leaves. Its tests now mirror those record families, and the guard
  prohibits restoring the flat leaf.
- [x] Remove the legalized-call validation forwarding wall. Its small
  `validation/mod.rs` now owns the complete representation-validation join.
- [x] Replace the 943-line abstract-operation identity encoder with a 71-line
  exhaustive family router over structural establishment, calls/effects,
  scalar operations, control exits, and scalar-operation shapes. Shared
  canonical carriers now sit beside the identity entrance; exact tags and
  field order remain in focused family leaves.
- [x] Replace SCCP validation's forwarding wall and 845-line candidate leaf
  with a 98-line rule-first, exhaustive-patch entrance over integer, boolean,
  observation, and range-comparison validation leaves.
- [x] Replace the 881-line live-range replay leaf with a 78-line independent
  reconstruct -> canonicalize -> compare -> receipt entrance over function,
  constraint, fragment, architectural-unit, comparison, and canonical leaves.
- [x] Split the 798-line independent GVN expression-key leaf into an honest
  internal group for total, proof-certified, and directional compatible-policy
  vocabularies while retaining independent validation mechanics.
- [x] Remove GVN's inherited namespace bucket. Put shared exact-effect
  admission at the pass ancestor, phi-only provenance accounting beside phi
  translation, and make every traversal leaf import its exact dependencies.
  Reject both the retired mixed accounting path and family-wide parent globs.
- [x] Audit every governed `lib.rs`/`mod.rs` file and explicitly classify each
  at its source as
  crate map, stage group, or executable entrance. Only executable entrances
  need a real join; no executable entrance may be a forwarding wall. The
  exhaustive architecture guard rejects missing, duplicate, or contradictory
  role declarations and pins every executable entrance to a coordination seam.
- [x] Split the 781-line transformation ledger into a 92-line construction
  entrance over model, construction validation, error, encoding, decoding,
  cursor, and test leaves without changing its stable wire identity.
- [x] Split the 1,399-line register-allocation test matrix by liveness, live
  ranges, legality, homes, fixed-view copies, and selected-input custody; its
  largest focused leaf is now 388 lines.
- [x] Split the 1,388-line selected-lowering test matrix by pressure recovery,
  named-suite behavior, exact immediates, and exit contracts; its largest
  focused leaf is now 599 lines.
- [x] Split conditional-control lowering into Boolean, integer, and shared-edge
  binding leaves below an honest group map; its largest leaf is 410 lines.
- [x] Split provider settlement into a 61-line executable entrance over exact
  plan, normalized foreign-call, per-boundary admission, and mirrored tests.
- [x] Split the pre-allocation machine-effect codec into a versioned V6 group
  over framing, instruction, structural, ownership, value, cursor, and error
  leaves; its largest leaf is 260 lines.
- [x] Split AArch64 MOVN proposal computation into a 95-line meaningful join
  over source admission, bounded selection, recipe, materialization, budget,
  and focused test leaves; its largest leaf is 328 lines.
- [x] Extract spill-choice proposal fixtures from its cohesive 495-line
  computation leaf into an adjacent 280-line focused test leaf without
  changing proposal/replay behavior.
- [x] Extract normalized foreign-scalar boundary-call fixtures from its
  cohesive 514-line lowering leaf into an adjacent 259-line focused test leaf.
- [x] Replace the 783-line selected-block validator with a 39-line executable
  roster/entry/return-routes join over entry control, immediate, parameter,
  exact-binary, active-resident exact-add-chain, and shared instruction replay
  leaves. Its largest leaf is 195 lines and rejection order is unchanged.
- [x] Replace the 779-line legalization source-leaf classifier with a 99-line
  executable admission/dispatch/return join over immediate, entry-parameter,
  direct exact-binary, widened exact-binary, active-resident exact-add-chain,
  return, operation-roster, and fuel leaves. Its largest leaf is 270 lines;
  catalog order, diagnostics, proof custody, and provenance order are unchanged.
- [x] Split the 751-line immutable rewrite model into a 19-line stage-group map
  over source/provenance foundations, scalar evaluation, SCCP snapshots and
  identities, CFG plans, scalar plans, and the candidate contract. Its largest
  leaf is 169 lines; public vocabulary, enum order, and identity bytes are
  unchanged. Neutral canonical writers now sit beside model and candidate
  codecs, so fact identities no longer depend on the candidate codec.
- [x] Replace the 748-line optimization-unit seed constructor with a 67-line
  executable plan/function/identity join over ordered function assembly,
  provenance, scalar dataflow, control flow, facts, and structural custody.
  Its largest leaf is 196 lines; diagnostic, effect, custody, and identity
  order are unchanged.
- [x] Replace the 746-line function-relative realization codec with a 78-line
  executable V9 framing/admission entrance over encoding, decoding,
  post-allocation custody, target layout, rendering, cursor, and error leaves.
  Its largest leaf is 239 lines; exact bytes and rejection order are unchanged.
- [x] Replace the 736-line mixed block-merging leaf with a 16-line
  non-executable family map over separately registered adjacent and
  non-adjacent rules, shared substitution reconstruction, and their distinct
  provenance-accounting leaves. Shared merge-boundary ownership custody now
  sits at the nearest ancestor used by block merging and jump fusion. Its
  largest leaf is 180 lines; the parent
  control-flow-cleanup entrance remains the sole owner of exact rule order.
- [x] Replace the 733-line fixed-view-copy validator with a 95-line executable
  admission/receipt join over root custody, exact copy-constraint admission,
  work usage, policy transformation, leaf destination, shared-entry, and
  selected-plan application leaves. Its largest leaf is 193 lines;
  independent replay and rejection precedence are unchanged, and validation
  does not call the producer.
- [x] Replace the 733-line Terminal-operation match with a 96-line wildcard-free
  executable router over structural establishment, calls, effects, Boolean,
  integer constants/relations, conversions, bitwise operations, shifts, and
  arithmetic leaves. The router performs the sole output append; its largest
  leaf is 322 lines. Exact policies, obligations that the abstract model can
  carry, diagnostics, structural declaration order, and output order are
  unchanged.
- [x] Replace the 726-line post-allocation construction leaf with a 76-line
  plan-assembly coordinator over root custody, ordinary functions, structural-
  Unit functions, physical operand construction, exact alternative selection,
  and focused fixtures. Its largest leaf is 151 lines; the public
  `post_allocation/mod.rs` remains the sole producer-to-validator entrance and
  no rule catalog or execution boundary was duplicated.
- [x] Replace the 722-line recovery-classification computation and 639-line
  mixed semantic/persistence model with an 86-line compute coordinator over
  function/victim classification, immediate eligibility, and exact work
  accounting, plus a separate V3 persistence leaf. Every leaf is below 600
  lines; `recovery_classification/mod.rs` remains the sole compute-to-
  independent-validation entrance.
- [x] Replace the 720-line x86 branch-relaxation computation with a 51-line
  producer/replay-to-artifact coordinator over work admission, branch
  inspection, production, independent replay, reflow, and artifact assembly.
  Every production leaf is below 210 lines; the existing stage entrance and
  sole exact-rule catalog retain execution and enablement ownership.
- [x] Replace the 678-line mixed value-range analysis with a 52-line stage join
  over constant facts and accepted-proof facts, with proof-goal, interval,
  reachability/dominance, and canonical fact-construction leaves below it. The
  join retains exact fact ordering and current-fact validation; its largest
  leaf is 311 lines.
- [x] Replace the 677-line allocation-legality computation with a 108-line
  root/environment admission and plan-assembly entrance over function, live-
  point, early-clobber, fixed-view, and candidate-view leaves. Its largest
  production leaf is 103 lines; rejection and function ordering are unchanged,
  and the public compute-to-independent-validation entrance remains singular.
- [x] Replace the 620-line ranked-u32 countdown contract replay with a 48-line
  ordered admission join over carrier, proof custody, ranked semantics, calling
  convention, and structural-frontier leaves. Machine-code and object replay
  remain visibly parallel; its largest leaf is 200 lines.
- [x] Replace the 1,256-line fixed-point execution test matrix with an 11-line
  map over algebraic rewrites, structural rewrites, proof-check elision, value
  numbering, and cross-pass dispatch/composition. All 34 tests remain; its
  largest leaf is 335 lines.
- [x] Replace seven more 600-749-line production debt leaves with meaningful
  semantic descents: SCCP binary proposal/evaluation, independent SCCP integer
  replay, pressure-rematerialization proposal and validation, fragment text
  placement, fragment emission, and post-allocation plan V3 persistence. Their
  coordinators are 52-83 lines, validation remains independent, exact route or
  wire order stays visible, and their largest production leaf is 339 lines.
- [x] Replace all ten remaining 1,000+ line test matrices with semantic maps for
  pass fixtures, artifact output, physical coordination, selected-machine,
  assignment/legalization, active-resident realization, operation contracts,
  scalar-affine cleanup, target selection, and Terminal-to-native realization.
  Their maps are 5-21 lines, all existing tests remain, and the largest focused
  leaf is 479 lines.
- [x] Ratchet every governed production leaf to at most 600 lines. The final
  debt split covers core identities/contracts, pre-allocation identity,
  post-allocation replay, and Unit assignment; production now has no size
  exceptions.
- [x] Ratchet every governed focused test and fixture to at most 800 lines.
  The final eleven matrices now descend by pass behavior, identity replay,
  assignment family, structural custody, allocation recovery, and provider
  installation.
- [x] Replace parallel rule-stage path arrays in the organization guard with
  typed stage descriptors and generic entrance/catalog/next-rung checks.
  Bespoke checks remain only for genuinely stage-specific invariants.
- [x] Replace the temporary 1,000/1,500 migration ceilings with hard 600/800
  production/test limits. All three entrance exceptions were split away; the
  default 100-line executable-entrance limit now has no exceptions.
- [x] Extract compiler-facing optimization hooks into focused governed
  subtrees: build vocabulary/admission/selection, checked selection custody,
  subtractive rollback, and native realization. Both build-prelude variants
  now consume one exact case -> counter -> transition mapping; the guard pins
  its sole ownership without governing entire compiler/build crates.
- [x] Add optimized program-entry carriers and selected/assigned optimizer
  representations to the governed boundary. The former 771/744-line semantic
  entry/wrapper are 32/52-line validation/construction entrances, assigned
  operations have a 24-line crate map, and selected machine effects have a
  78-line independent admission/identity entrance.
- [x] Remove the narrow `omega-image-emission/ranked_u32_countdown` subtree
  from the optimizer guard. It independently replays one language-level ranked
  carrier but owns no optimization selection, catalog, proposal, or optimized
  stage result; native-image publication belongs to its own coherent
  architecture boundary rather than one special-case optimizer exception.
- [x] Strengthen catalog uniqueness checks against differently named proxy
  optimization arrays across Omega, and replace the duplicated build and
  filesystem prelude switch schemas with one canonical projection whose tests
  pin every exact case -> counter -> transition -> increment mapping.
- [x] Replace SCCP's 491-line nine-rule range-comparison aggregate with a
  23-line family map over range/constant and range/range groups. All nine exact
  identities now own a 40-50-line executable entrance; shared proposal and
  interval evaluation mechanics sit at the nearest evidence-family ancestor,
  the 39-row pass roster is unchanged, and mirrored tests descend by evidence
  family and catalog custody.
- [x] Replace SCCP's mixed 246-line Boolean-result constant-evaluation leaf
  with five exact executable entrances over a stage-group map, closed kind
  model, typed evaluation, and proposal assembly. The 39-row pass roster and
  canonical identities are unchanged; mirrored tests separate Boolean and
  integer results and pin Boolean-rule catalog positions 25-29.

## P1 — Opt-in, catalogs, and compatibility firewall

- [x] Exact named build selections with canonical order and identity.
- [x] Empty selection preserves the ordinary compiler path and constructs no
  optimizer machinery.
- [x] Duplicate, unknown, noncanonical, trailing, and incompatible-version
  selections fail closed.
- [x] Exact per-rule release rollback is subtractive and never enables a rule.
- [x] One owning catalog per Psi, selected-lowering, allocation-recovery,
  post-allocation-machine, and function-relative-layout phase.
- [x] Catalog coverage proves each public exact name is scheduled once or has
  an explicit phase/target rejection.
- [x] No-selection canaries cover source acceptance, diagnostics, interpreter
  output, native bytes, and artifact metadata.

## P2 — Validation, translation, and publication

- [x] Producers create immutable plans; independent validators reconstruct
  representation and rule preconditions before publication.
- [x] Decisions, work usage, manifests, facts, and custody receipts are
  identity-bound.
- [x] Psi candidate declarations retain applied and skipped decisions with
  independently replayed manifest, rule, revision, and policy evidence.
- [>] Complete independent translation validation for every lowering and
  machine-rule family. Twenty-two abstract-to-target families, including
  proof-bearing exact addition and saturating parameterized integer addition
  and subtraction, selected incoming u12 folds, current machine substitutions,
  structural-Unit encoding, resolved layout, and ranked-u32 publication routes
  are covered; the remaining source families and publication routes are not.
- [x] Extend abstract ordinary, Unit, and structural-scalar call operations and
  downstream identities/codecs/lowering to retain Terminal
  `requirement_obligations` and `crash_continuations`. The exact rows now cross
  Terminal projection, optimization-unit V17 identity, target and temporary
  assigned carriers, legalized V10 and selected V12 identity, allocation
  recovery, and fixed-view-copy V6 persistence. Ordinary, Unit, and
  structural-scalar nonempty projection/lowering tests plus identity,
  independent replay, corruption, and V5 compatibility tests pin custody.
- [ ] Apply **Boundary domain requirements consume carried qualifications**.
  Until the next Terminal format/vocabulary revision, keep the legacy boundary
  `requirement_obligations` slot empty and reject every nonempty roster as
  `BoundaryStructuralRequirementsMintObligations`. At that revision remove the
  field from the boundary variant and its wire payload rather than preserving
  an always-empty slot. Add optimizer/publication controls that bind the exact
  boundary, structural argument paths, carried qualification rosters, and
  declaration requirements; joins use at most the common intersection, CSE/GVN
  never equates unequal rosters by computation alone, and no transformation may
  widen a roster or otherwise mint a routed qualification.
- [x] Evolve fixed-view-copy persistence through v6. V5 introduced the
  versioned structural selected subtree; public encoding now emits v6 with
  exact structural-call requirement and crash-continuation rows. Decoding
  accepts v4/v5/v6, v4 bytes and rejection order remain pinned, and v5
  reconstructs the newly absent rows as empty. The authenticated payload also
  closes caller/callee call-plan fields that independent validation checks but
  the selected semantic identity does not fully cover.
- [ ] Add generated differential testing across interpreter/reference native
  execution for exact integer, float, trap, atomic, placed-memory, cleanup, and
  transition cases.
- [ ] Add end-to-end mutation tests for every manifest and custody field.

## P3 — Psi optimizer

- [x] Control-flow cleanup, SCCP, copy propagation, local/dominating/
  phi-translated GVN, dead pure scalar elimination, and proof-check elision.
- [x] Exhaustive exact rule partitions for wrapping neutral/shift/multiply,
  saturating neutral/multiply, and bitwise neutral/absorbing identities.
- [x] Terminal Psi admits finite cyclic control through SCC topology rather
  than a `Loop` terminator; suspension remains an interprocedural call state.
- [ ] Generalize the exact unsigned-countdown carrier into ordinary cyclic
  execution with fixed-point dominance/frontiers, canonical component IDs,
  well-founded ranking certificates, productive unranked components, and
  structured finite-work failures.
- [ ] Retarget LICM and other loop consumers to validated Terminal SCCs.
- [ ] Implement LICM only after transforms can invalidate and reconstruct
  component, loop-carried custody, and ranking evidence.

## P4 — Lowering and instruction selection

- [x] Mandatory legalization has one ordered declarative catalog for current
  scalar, plain-Unit, and structural-Unit forms; omission and ambiguity fail
  closed.
- [x] Selected construction classifies a complete function body through one
  scalar-family catalog and returns registers and blocks together.
- [x] Exact incoming-u12 add/subtract immediate folds.
- [x] Add exact wrapping add/subtract/multiply parameter translation families.
  Each owns a separate catalog row, source grammar, target replay, typed
  error/receipt, corruption, and optimized-custody leaves. Their common
  arithmetic carrier and whole-roster ABI/provenance replay are shared at the
  nearest multi-consumer ancestor.
- [x] Add an independent proof-bearing exact-add parameter translation family.
  Its distinct catalog row reconstructs source and ABI custody without calling
  lowering, retains the exact overflow-obligation identity in its receipt, and
  rejects wrapping/saturating substitution and independent obligation drift.
- [x] Add independent saturating-add and saturating-subtract parameter
  translation families. Each exact catalog row reconstructs ordered source
  operands, whole-roster ABI placement, provenance, and its matching target
  expression; wrapping and proof-bearing exact policy substitution fails
  closed.
- [ ] Add exact address-mode folding, compare/branch selection, extension
  elimination, and constant materialization one named family at a time.
- [ ] Validate ABI operands, calls, clobbers, effects, traps, provenance, and
  logical fuel across every selected rule.

## P5 — Register allocation and frame assignment

- [x] Selected-CFG liveness, live ranges, register views/units/aliases,
  availability, allocation legality, and deterministic transition-free
  interference allocation.
- [x] Fixed-view-copy and active-resident rematerialization recovery with one
  generic allocation-recovery publication carrier.
- [x] x86-64 and AArch64 register-environment ABI/call-clobber corruption
  matrices.
- [ ] Add spill choice, insertion, reload/store validation, and stack-slot
  coloring.
- [ ] Add coalescing, live-range splitting, fixed/precolored intervals, and
  rematerialization cost decisions.
- [ ] Implement frame layout, alignment, red-zone/shadow-space, unwind,
  probing, stable-address loans, and dynamic-allocation constraints.
- [ ] Extend call-clobber validation through general scalar calls and
  live-across-call allocation after calls enter the selected CFG.

## P6 — Machine optimizer

- [x] Target-neutral post-allocation symbolic plan/effects with independent
  validation.
- [x] AArch64 CBNZ fusion and MOVN materialization; x86 XOR-zero and
  MOV-r32-imm32 materialization; x86 rel8 layout relaxation.
- [x] Generic encoding/layout/realization carriers let a new substitution add
  one rule leaf and catalog row rather than a new vertical pipeline.
- [ ] Add declarative peephole matching over symbolic instructions, physical
  register units, effects, traps, memory, stack, and control flow.
- [ ] Add exact copy removal, redundant extension removal, address folding,
  compare/test selection, and scheduling where independently verifiable.
- [ ] Add target cost models as non-authoritative identities. Semantic
  validation must not depend on estimates.

## P7 — Proof-, ownership-, and state-aware optimization

- [x] Accepted-obligation identities authorize exact proof-check and scalar
  rewrites and remain in the publication chain.
- [ ] Alias/borrow-aware load forwarding, dead-store elimination, and mutation
  motion.
- [ ] Field/variant relevance and invariant-window specialization.
- [ ] Cleanup and transition reachability pruning.
- [ ] State-argument/result specialization with edge provenance.
- [ ] Interprocedural service/call summaries and proof-bound inlining.
- [ ] Proof-directed loop bounds, induction simplification, and vectorization
  with exact lane semantics.

Every rule names the exact proof, ownership, effect, and provenance facts it
consumes and retains their identities through publication.

## P8 — Search and ML extensibility

- [x] Versioned, pointer-free input/output schemas expose exact source, target,
  selection, rule, candidate, fact, and cost-model identities.
- [x] Record-only mode cannot change output; deterministic replay rejects stale
  or mismatched contexts and candidates.
- [ ] Add a sandboxed external policy boundary with timeout/resource limits
  and an explicit fallback.
- [ ] Add offline corpus capture, training, evaluation, and regression tools.

ML may rank already-declared equal transformations. It cannot invent an
unchecked rewrite or opt into lossy floating-point semantics.

## P9 — Verification, stabilization, and rollout

- [>] Complete per-rule positive, negative, boundary, disabled, budget,
  determinism, idempotence, and corruption coverage. Every SCCP binary integer
  rule now has a direct positive semantic canary with independently validated
  typed output, exact safety class, and proof/exact witness form. Thirteen
  proof-certified overflow, zero-divisor, signed-quotient, and shift-domain
  cases pin refusal. The three unary integer rules now pin their exact roster
  positions, signed and unsigned endpoint semantics, cross-kind refusal, and
  proof/exact witness forms. Independent validation also binds all 25 integer
  constant-evaluation operation shapes to their exact rule identities and
  rejects a candidate relabelled with another same-safety contract. All five
  Boolean-result literal rules now pin their complete roster contracts, truth
  or signed/unsigned comparison boundaries, every cross-kind refusal, all 20
  cross-contract relabellings, unknown-rule refusal, per-rule result
  corruption, and unary/binary witness and fact corruption. Their independent
  validator now separates literal, range/constant, and range/range evidence
  and binds literal operation shapes to exact rule identities. All nine
  proof-certified integer-range comparison rules now have direct proof-derived
  fixtures covering true, false, and indeterminate results, signed and unsigned
  bounds, both range/literal orientations, all three same-value range/range
  outcomes, every cross-operation producer refusal, every same-family contract
  relabelling, unknown identities, foreign range facts, corrupted results, and
  exact contracts at roster positions 30--38. Validation now rejects analysis
  and invalidation supersets instead of accepting a contract that merely
  contains the expected analyses. All seven control-flow-cleanup validators
  now bind exact rule identity, complete analysis and invalidation sets, safety
  class, and rule-specific cost; a full cross-rule corruption matrix rejects
  every relabelling plus unknown identities, supersets, wrong safety, and wrong
  cost. Mixed empty-threading and block-merge validation files now descend to
  exact rule leaves. Copy propagation now has an exact executable rule
  entrance, complete registry custody, contract/cost/witness/provenance
  corruption coverage, and positive, disabled, budget, determinism, and
  idempotence canaries. The remaining operational axes and other rule families
  are not yet complete.
- [x] Cross-rule phase-composition matrix, including fail-closed unsupported
  combinations.
- [ ] Add randomized valid-Psi and selected-machine differential corpora.
- [ ] Add supported target/OS allocator, encoding, unwind, object, and callable
  matrices.
- [ ] Add versioned compile-time, memory, code-size, and runtime benchmarks.
- [ ] Publish exact-rule release notes and rollback procedures.
- [ ] Require owner-reviewed promotion criteria per exact rule; never promote
  an implicit broad level.
