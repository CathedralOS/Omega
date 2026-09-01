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
  elimination's mixed `rules.rs` with exact named rule leaves. Literal and
  unconditionally-total admission remains under the dead-pure-scalar pass;
  proof-certified dead-scalar admission lives under the proof-check pass that
  actually enables it. Their neutral `support/dead_scalar_node/` rung owns
  only shared traversal, liveness/effect, accounting, and witness mechanics,
  while each exact leaf owns its pass, contract, and closed classifier. Retire
  the former cross-pass family/shape/proposal paths and reject generic Psi
  production rule leaves in the architecture guard. Independent proof-check
  validation now has one visible twelve-rule routing entrance and adjacent
  identity-to-protocol catalog; SCCP no longer recognizes or dispatches
  proof-check rules.
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
  machine-rule family. Forty-four abstract-to-target families are covered,
  including
  parameterless straight-line Unit return with an independently reconstructed
  empty native call plan, exact return edge/provenance, and plan-global
  structural-type roster custody. The adjacent PortWrite-plus-Unit-return
  family independently retains the exact singleton service ceiling, operation,
  service, port, byte, target provenance, empty native call plan, return edge,
  and cleanup roster across all five native targets. The exact Unit-call family
  independently reconstructs a separate parameterless Unit-return callee and
  retains the caller's `CallUnit; ReturnUnit` roster, callee, arbitrary exact
  requirement/crash rows, empty structural/claim/native-call-plan surfaces,
  provenance, cleanup, and return edge across the same targets. An exact
  parameterless `EstablishTrivialAffineLocal; ReturnUnit` family independently
  reconstructs its empty-record local, declaration ordinal, absent
  construction, exact discard cleanup, provenance, and native Unit call plan
  across all five targets. Its adjacent parameterless
  `EstablishByteSequenceLiteral; ReturnUnit` family retains the exact borrowed-
  view byte-sequence type, canonical literal place, arbitrary byte payload,
  empty cleanup, provenance, and native Unit call plan across all five targets.
  The adjacent parameterless `IntegerConstant; ReturnUnit` family independently
  retains the unused literal's exact operation/result identities, integer
  type/value domain, return edge, empty cleanup/native call plan, provenance,
  and canonical structural roster across all five targets. If Psi dead-scalar
  elimination removes the literal, the transformed plan instead enters the
  distinct return-only family.
  The adjacent parameterless `IeeeFloatConstant; ReturnUnit` family retains
  the literal's exact operation/result identities and raw Binary32 or Binary64
  bits, including signed zero and NaN payloads, plus the return edge,
  provenance, empty cleanup/native call plan, and canonical structural roster
  across all five native targets. Its optimized-target custody canary retains
  the same typed family rather than reclassifying it through host floats; the
  independent optimization-unit validator now also reconstructs IEEE literal
  and fused-multiply-add result definitions instead of rejecting producer-
  correct scalar metadata.
  The adjacent parameterless IEEE-literal-sequence family admits an ordered
  sequence of at least two `IeeeFloatConstant` operations followed by
  `ReturnUnit`. Independent replay preserves every operation/result identity,
  each raw Binary32/Binary64 bit pattern, exact order and provenance, the
  return edge, empty cleanup/native call plan, and canonical structural roster
  across all five native targets. Signed zero and NaN payloads survive without
  host-float conversion, and its optimized-target custody canary retains the
  typed sequence receipt. Its multi-operation grammar remains disjoint from
  both the singleton IEEE-literal and return-only families.
  The x86-only nearest-even IEEE fused-multiply-add sibling admits exactly
  three raw-bit constants, one fused operation consuming them in order, and
  `ReturnUnit`. Independent replay binds every definition/operand identity and
  bit pattern, Binary32/Binary64 format, exact operation/edge provenance,
  empty native Unit call plan, and the complete settlement: canonical provider
  identity and target, scalar FMA slot, selected requirement and compiler-
  intrinsic realization, provider-plan report coordinate and strong digest.
  Missing, duplicate, unknown, substituted, or cross-target settlements fail
  closed. Both formats cross Linux, Windows, and UEFI x86-64 translation plus
  optimized-target custody; Arm targets receive a typed applicability failure.
  Coverage also includes proof-bearing exact
  parameterized fixed-integer addition,
  subtraction, multiplication, division, and remainder, proof-bearing wrapping and
  saturating division/remainder, plus saturating parameterized integer
  addition, subtraction, and multiplication, independently typed wrapping
  shifts and proof-bearing exact shifts, selected incoming u12 folds,
  current machine substitutions, structural-Unit encoding, resolved layout,
  and ranked-u32 publication routes. The x86 zero-extending MOV-r32
  materialization now also crosses
  fragment emission, relocation-free text/object custody, validated object
  publication, and callable-entry replay with exact source-kind, manifest, byte,
  and corruption checks. That rule also composes after active-resident
  rematerialization through the generic post-allocation realization and final
  publication path, retaining both phase identities and rejecting manifest,
  exit-contract, target, and unadmitted-pair corruption. The remaining source
  families and publication routes are not yet complete. The sibling x86
  sign-extending `MOV r64, imm32` family now independently selects the exact
  i32 round-trip bit domain, validates canonical seven-byte ISA forms and
  exact-view writes, and crosses the same generic direct and active-resident
  publication routes with corruption coverage. AArch64 MOVN materialization
  now also composes after active-resident rematerialization on a high-ones
  exact-add fixture, retaining independently decoded replacement bytes and
  both phase roots through fragment, object, and callable publication. The x86
  XOR-zero rule now also composes after active-resident rematerialization of
  an exact zero; its producer and independent replay consume the recovery
  carrier's recomputed liveness so dead-RFLAGS eligibility is checked after
  rewriting, and both phase roots plus canonical three-byte output survive
  fragment, object, and callable publication.
- [x] Extend abstract ordinary, Unit, and structural-scalar call operations and
  downstream identities/codecs/lowering to retain Terminal
  `requirement_obligations` and `crash_continuations`. The exact rows now cross
  Terminal projection, current optimization-unit V18 identity, target and temporary
  assigned carriers, legalized V10 and selected V12 identity, allocation
  recovery, and fixed-view-copy V6 persistence. Ordinary, Unit, and
  structural-scalar nonempty projection/lowering tests plus identity,
  independent replay, corruption, and V5 compatibility tests pin custody.
- [>] Apply **Boundary domain requirements consume carried qualifications**.
  Terminal format 57 and vocabulary 60 retain the earlier removal of
  the boundary `requirement_obligations` field and wire payload rather than
  preserving an always-empty slot. Remaining work is to add
  optimizer/publication controls
  that bind the exact boundary, structural argument paths, carried
  qualification rosters, and declaration requirements; joins use at most the
  common intersection, CSE/GVN never equates unequal rosters by computation
  alone, and no transformation may widen a roster or otherwise mint a routed
  qualification. The first exact whole-root slice now crosses verified
  Terminal production, a no-rewrite optimizer run, independent transformed-
  unit replay, abstract-plan projection, and the pre-physical manifest. It
  proves that the boundary call mints no proof question; binds the exact
  boundary, empty argument path, carried roster, and declaration requirement;
  and rejects reauthenticated missing/widened rosters, boundary substitution,
  path substitution, erased requirements, and detached projection metadata. A
  second whole-root slice now admits a dominating structural operation result
  as a boundary argument, independently rejoins its exact qualification
  roster, and rejects a same-carrier foreign domain. Current-ownership replay
  separately proves that whole-root shared or mutable borrows do not consume a
  linear carrier while owned arguments still do, and rejects stale or
  partially moved borrows. The audited next prerequisite is an explicit,
  canonical path-indexed qualification roster. The first honest bounded slice
  is now complete for parameter roots: Terminal format/verifier, optimizer
  identity and independent replay, no-rewrite abstract projection, and the
  prephysical manifest retain an exact nonempty path and leaf-domain carrier.
  Missing, sibling, duplicate, unsorted, wrong-domain, detached, widened, and
  root-path substitutions fail closed. Target lowering rejects the retained
  projected roster explicitly until downstream carriers support it rather than
  inferring authority from root shape or carrier equality. The paired function-
  result and operation-result slice is now complete. Terminal declarations and
  structural operation results carry separate exact projected rosters; calls
  copy only the callee result roster; and return replay rejoins the declared
  source contract. Format-56/vocabulary-59 compatibility reconstructs absent
  rows as empty. Optimization-unit V18 identity, independent replay, abstract
  projection, prephysical custody, and image signature codecs retain them.
  Target lowering rejects either nonempty result roster until downstream
  carriers support it. Join intersection remains later work because current
  joins and GVN have no structural result carrier or executable consumer. That
  remaining item is not an open language-semantic question.
- [x] Evolve fixed-view-copy persistence through v6. V5 introduced the
  versioned structural selected subtree; public encoding now emits v6 with
  exact structural-call requirement and crash-continuation rows plus
  parameter-rooted projected qualification rows. Decoding accepts v4/v5/v6;
  the legacy v4 payload shape and rejection order remain pinned, while v4/v5
  reconstruct fields absent from those versions as empty. The current-
  vocabulary authenticated digest is re-pinned when Terminal identity changes.
  The authenticated payload also
  closes caller/callee call-plan fields that independent validation checks but
  the selected semantic identity does not fully cover.
- [>] Add generated differential testing across interpreter/reference native
  execution for exact integer, float, trap, atomic, placed-memory, cleanup, and
  transition cases. V2 adds the first same-artifact host-native exact-integer
  lane: both Boolean paths of one Terminal immediate-return artifact are
  interpreted, the exact host post-allocation materialization rule is applied,
  and the resulting linked function is called on both paths and compared with
  the interpreter result. Float, trap, atomic, placed-memory, cleanup, and
  transition lanes remain open.
- [>] Add end-to-end mutation tests for every manifest and custody field. The
  ordinary-callable entry slice now reauthenticates and rejects every mutable
  manifest field, rejects every closed singleton wire tag and unavailable-data
  position, and mutates each receipt custody root independently. Other artifact
  and manifest families remain open. The function-fragment-emission V9 slice
  now separately mutates all 30 representable record/subrecord fields, every
  codec envelope and closed tag, all six unavailable-data positions, and all
  three custody roots; independent reconstruction rejects every mutation. The
  following relocation-free function-fragment text-section V9 slice now does
  the same for all 36 representable record/subrecord fields, every applicable
  closed wire and envelope axis, all six unavailable-data positions, and all
  four custody roots. Each record mutation is reauthenticated so independent
  replay, rather than a stale outer digest, rejects it. The adjacent
  relocation-free object-container V1 slice now reauthenticates all 21
  representable manifest/subrecord fields, rejects every applicable closed
  wire and envelope axis plus all four unavailable-data positions, and mutates
  all five receipt custody roots independently. The adjacent optimized-object-
  artifact V1 slice covers both publication records: all 23 representable
  artifact fields and all 16 representable manifest fields are reauthenticated
  independently; their envelope, vocabulary, optional-debug, target, machine,
  stage, unavailable-data, trailing, and truncation wire axes fail closed; and
  all six receipt custody roots are mutated separately.

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
- [x] Add independent proof-bearing exact-add, exact-subtract, exact-multiply,
  exact-divide, and exact-remainder parameter translation families. Their
  distinct catalog rows reconstruct fixed-integer source and ABI custody without
  calling lowering, retain the exact operation obligation, reject address
  carriers, and fail closed on arithmetic-policy substitution, ordered-operand
  drift, and independent obligation drift. Exact-subtract, exact-multiply,
  exact-divide, and exact-remainder also cross optimized-target custody with real
  Terminal certificates; quotient and remainder use Terminal's canonical
  nonzero-divisor and representable-result goal.
- [x] Add independent proof-bearing wrapping-divide and wrapping-remainder
  parameter translation. Their exact catalog rows retain ordered fixed-integer
  operands and the source obligation, reject address carriers and every
  arithmetic-policy substitution, and cross optimized-target custody with
  Terminal's canonical nonzero-divisor certificate. Signed `MIN / -1` wraps to
  `MIN`; signed `MIN % -1` is zero. Neither acquires exact division's
  representable-result precondition.
- [x] Add independent proof-bearing saturating-divide parameter translation.
  Its exact catalog row retains ordered fixed-integer operands and the source
  obligation, rejects address carriers and every arithmetic-policy
  substitution, and crosses optimized-target custody with Terminal's canonical
  nonzero-divisor certificate. Signed `MIN / -1` clamps to `MAX`; no separate
  representable-result precondition is required.
- [x] Add independent proof-bearing saturating-remainder parameter translation.
  Its exact catalog row retains ordered fixed-integer operands and the source
  obligation, rejects address carriers and every arithmetic-policy
  substitution, and crosses optimized-target custody with Terminal's canonical
  nonzero-divisor certificate. It follows truncating remainder's dividend sign
  and defines signed `MIN % -1` as zero without a representable-result
  precondition.
- [x] Add independent saturating-add and saturating-subtract parameter
  translation families. Each exact catalog row reconstructs ordered source
  operands, whole-roster ABI placement, provenance, and its matching target
  expression; wrapping and proof-bearing exact policy substitution fails
  closed.
- [x] Add independent saturating-multiply parameter translation. Its exact
  catalog row, typed source/target replay, receipt, and optimized-target custody
  retain ordered operands across every integer width, address-U64, all five
  native target profiles, and register/stack placement; wrapping, proof-bearing
  exact, add, and subtract policy substitution fails closed.
- [x] Add independent wrapping shift parameter translation. The left and right
  catalog rows descend through one dedicated shift taxonomy while retaining
  independently typed value and count operands, ordered ABI placement, and
  exact source/target provenance. Fixed and address64 carriers are admitted
  independently and signed negative counts retain Euclidean modulo-width
  reduction. Right shift separately retains unsigned fixed/address zero-fill
  and signed fixed sign-fill semantics; direction swaps, exact shifts, bitwise,
  and arithmetic expression substitutions fail closed through optimized-target
  custody.
- [x] Add independent proof-bearing exact shift-right parameter translation.
  Its shift leaf retains independently typed fixed-integer value/count
  operands, their exact ABI placement and provenance, and the source obligation
  through target replay and optimized custody. The canonical `ExactShiftCount`
  goal admits only the unmodified mathematical range `0 <= count < value_width`;
  it does not require shifted-out bits to be zero. Address carriers, obligation
  drift, direction/policy substitution, and ordered-operand drift fail closed.
- [x] Add independent proof-bearing exact shift-left parameter translation.
  Its sibling leaf retains independently typed fixed-integer value/count
  operands, exact ABI placement and provenance, and the source obligation
  through target replay and optimized custody. The canonical
  `ExactShiftLeftRepresentable` goal proves both the unmodified count range and
  that the mathematical left-shift result fits the value carrier. Address
  carriers, obligation drift, direction/policy substitution, and ordered-operand
  drift fail closed.
- [ ] Add exact address-mode folding, compare/branch selection, extension
  elimination, and constant materialization one named family at a time.
  - [x] Add x86-64 sign-extended imm32 i64 materialization as one exact named
    family over the full i32 round-trip bit domain. Its independently replayed
    symbolic plan, canonical `REX.W + C7 /0 r64, imm32` encoder/decoder,
    seven-byte layout, direct and active-resident custody, object/fragment, and
    callable publication all preserve exact-view writes and RFLAGS.
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
- [>] Add spill choice, insertion, reload/store validation, and stack-slot
  coloring. Deterministic bounded spill choice now feeds a V1 independently
  replayed logical spill-operation plan for active-resident non-address
  unsigned-U64 instruction results. It binds selected/range/legality/choice
  custody, records logical storage plus store-before-pressure,
  reload-before-first-future-use, and the complete later-use rewrite suffix,
  and has versioned transport, exact budgets, typed refusal, corruption, and
  x86-64/AArch64 fixture coverage. It intentionally grants no physical slot,
  offset, instruction, frame, unwind, trap, or publication authority. A second
  V1 independently replayed artifact now colors its closed block-local storage
  lifetimes by deterministic lowest-offset first fit. It grants only canonical
  8-byte-aligned offsets relative to an abstract spill-area origin, with
  identity-bound roots, exact work budgets, strict versioned transport,
  endpoint-conflict/reuse tests, empty-pressure behavior, and cross-target
  corruption coverage. Current public logical plans contain at most one spill
  per function, so compiler fixtures can exercise only offset zero; internal
  interval tests pin overlapping, disjoint, touching, and cross-block coloring.
  A third independently replayed V1 artifact now joins those two validated
  sources into one deterministic per-function abstract store/reload/rewrite
  insertion schedule. It binds the logical plan, slot coloring, register
  environment, allocator availability, optimization unit, fuel, budget,
  physical views, reload class, and spill-area-relative geometry, with exact
  ordering, empty-pressure, x86-64/AArch64, budget, and corruption coverage.
  It creates no selected or machine instruction, allocates no reload home,
  chooses no opcode or SP/FP address, and grants no frame, trap, unwind,
  probing, encoding, emission, or publication authority. A fourth
  independently replayed V1 artifact now reanalyzes the logical reload value
  introduced by that schedule. It reconstructs the original linear-scan
  prefix, applies the validated single block-local spill, derives the reload's
  exact lifetime and legal-view intersection, and assigns the lowest
  compatible physical view. Its independent point-indexed event replay caught
  the intermediate-point lifetime case and rejects root, assignment, complete
  candidate-domain, coexisting-home, usage, and budget corruption on x86-64
  and AArch64. It creates no real virtual register, selected or machine
  instruction, memory effect, frame address, trap claim, encoding, emission,
  or publication authority. Reload or subsequent pressure fails with typed
  evidence. A fifth independently replayed V1 artifact now binds each admitted
  logical reload and home to a compiler-private `{epoch, ordinal}` synthetic
  namespace in canonical function/logical-value order. Epoch zero retains the
  insertion and reload-home roots, lifetime, class, and assigned view, with
  deterministic x86-64/AArch64, cross-target corruption, exact-budget, and
  empty-pressure coverage. It still creates no real `VirtualRegisterId` or
  instruction and grants no downstream authority. Its honest recursive-
  recovery fixture prerequisite is now public and independently replayed.
  Mandatory legalization admits the
  versioned exact graph
  `r + (b + (r + (a + b)))`, retaining `b` across the first use of `r`, on
  x86-64 and AArch64. It appends a distinct recipe/carrier identity without
  changing the existing V10 recipe tags. A distinct eighth scalar-selection
  family now constructs and independently replays the exact nine-register,
  twelve-instruction selected shape. With only two admitted physical views,
  the public liveness, ranges, availability, legality, spill choice, logical
  spill, slot-coloring, abstract-insertion, and reload-home chain reaches the
  exact typed `ReloadPressure { function: 0, result: 0 }` branch on x86-64 and
  AArch64. The fixture retains the pressure point, incoming/victim values,
  store, reload, and complete rewrite suffix; it fabricates no `Validated*`
  receipt. A sixth independently replayed V1 artifact now begins exactly at
  that failure and creates one bounded epoch-one work item. It retains the
  source reload, machine, block, half-open lifetime, class, complete canonical
  candidate domain, and separate reload-trigger/worklist budgets. It grants no
  victim choice, assigned view, selected virtual register, rewrite, memory,
  frame, trap, unwind, encoding, emission, or publication authority. A seventh
  independently replayed V1 artifact now consumes that item and reconstructs
  the post-first-spill allocation twice: production uses a sorted schedule and
  validation uses a point-indexed event timeline. It retains the complete
  active-resident and recoverable-contender rosters and deterministically
  chooses the farthest-ending, then highest-VReg second victim. The public
  x86-64/AArch64 fixture selects `v3 [9,15)` over `v4 [11,13)` at reload point
  12 with exact usage `{5, 2, 10, 1, 1}`. It grants no eviction, logical spill,
  rewrite, storage, selected identity, memory, frame, trap, unwind, encoding,
  emission, or publication authority. An eighth independently replayed V1
  artifact now consumes that choice and emits epoch-one target-neutral logical
  storage, store-before-source-reload, reload-before-first-later-flexible-use,
  and the complete later-use rewrite suffix. Its public x86-64/AArch64 fixture
  retains victim `v3`, store anchor 6, reload/rewrite anchor 7 at point 14,
  `{epoch: 1, ordinal: 0}` namespaces, current/reclaimed views, and exact usage
  `{1, 1, 5, 1, 1}`. It creates no real virtual register, instruction,
  slot/offset, memory effect, frame, trap, unwind, encoding, emission, or
  publication authority. A ninth independently replayed V1 artifact now performs
  generalized abstract insertion and recoloring. It binds the validated first
  insertion and epoch-one action identities plus environment, availability,
  optimization-unit, fuel, budget, and usage roots; colors closed lifetimes by
  deterministic 8-byte lowest-offset first fit; and emits one canonical
  `Store < Reload < Rewrite` event stream. The public dual-target fixture gives
  epoch zero `[9,12]` at offset 0 and epoch one `[12,14]` at offset 8 because
  their shared closed endpoint conflicts, for 16 abstract bytes, seven events,
  and usage `{1, 2, 10, 2, 3}`. The epoch-one store names the triggering epoch-
  zero reload it precedes. It still creates no real register, instruction,
  memory effect, frame, trap, unwind, encoding, emission, or publication
  authority. A tenth independently replayed V1 artifact now reanalyzes both
  generalized reload actions without collapsing the first result into the
  second failure. Its producer uses a sorted allocation schedule; replay owns
  a separate point-indexed timeline. On both x86-64 and AArch64, epoch zero
  receives the lowest compatible home for `[12,17)`, while epoch one retains
  an exact `Pressure` outcome at point 14 for `[14,15)`, its two-view candidate
  domain, and the complete two-home blocker roster containing the epoch-zero
  reload and original `v5`. Roots, outcomes, domains, rosters, ordering, usage,
  and all representable budget axes fail closed; exact usage is
  `{3, 4, 18, 1, 3}`. The 65-line entrance descends through distinct
  `compute/` and `replay/` semantic ladders and grants no selected VReg,
  instruction, memory, frame, trap, or publication authority. An eleventh
  independently replayed V1 artifact now turns that retained epoch-one
  pressure into exactly one compiler-private epoch-two work item. It preserves
  the source pressure action and lineage, machine, block, `[14,15)` lifetime,
  class, complete two-view domain, and both canonical blockers on x86-64 and
  AArch64 under exact usage `{2, 2, 13, 1, 1}`. Its 25-line entrance joins a
  direct producer to separately keyed replay; root, item, domain, blocker,
  ordering, usage, representable budget, and cross-target corruption fail
  closed. The item is not a spill action or selected VReg and chooses no victim
  or home. A twelfth independently replayed V1 artifact now consumes that work
  item and its exact blocker roster. It reconstructs the original `v5` and
  epoch-zero reload lifetimes/views, proves both are individually recoverable
  contenders, and selects the farthest-ending then highest canonical value:
  the epoch-zero reload `[12,17)` on both native architecture fixtures. Direct
  traversal and separately keyed replay retain every root, work item,
  pressure, candidate, resident, contender, selected view, and reclaimed view
  under exact usage `{2, 2, 13, 1, 1}`; all representable budget axes and
  cross-target custody fail closed. The 50-line entrance grants no eviction,
  logical action, selected VReg, instruction, memory, frame, trap, or
  publication authority. Remaining work converts that choice into a bounded
  epoch-two logical recovery action.
  Lower spill-pseudo representation,
  abstract spill memory effects and ISA lowering, final frame offsets,
  unwind/probing, and downstream realization remain engineering work. Real
  memory insertion is owner-blocked only on the spill-access fault semantics
  recorded in `OWNER_QUESTIONS.md`; the abstract schedule, reload-home
  analysis, and synthetic namespace are not blocked.
- [ ] Add coalescing, live-range splitting, fixed/precolored intervals, and
  rematerialization cost decisions.
- [ ] Implement frame layout, alignment, red-zone/shadow-space, unwind,
  probing, stable-address loans, and dynamic-allocation constraints.
- [ ] Extend call-clobber validation through general scalar calls and
  live-across-call allocation after calls enter the selected CFG.

## P6 — Machine optimizer

- [x] Target-neutral post-allocation symbolic plan/effects with independent
  validation.
- [x] AArch64 CBNZ fusion and MOVN materialization; x86 XOR-zero,
  MOV-r32-imm32, and sign-extending MOV-r64-imm32 materialization; x86 rel8
  layout relaxation.
- [x] Generic encoding/layout/realization carriers let a new substitution add
  one rule leaf and catalog row rather than a new vertical pipeline.
- [x] Retain the canonical post-allocation catalog entry through physical
  composition and dispatch both source lineages on its closed typed rule kind;
  the pipeline contains no duplicate exact-name schedule.
- [x] Admit the first allocation-recovery plus post-allocation-machine pairs:
  active-resident immediate-U64 multi-use rematerialization followed by
  AArch64 MOVN, x86 XOR-zero, x86 MOV-r32-imm32, or x86 sign-extending
  MOV-r64-imm32 selection. One generic
  realization retains both phase roots, exact baseline/final bytes,
  whole-function exit custody, and final fragment/object/callable publication;
  every other recovery-machine pair remains a typed rejection.
- [ ] Add declarative peephole matching over symbolic instructions, physical
  register units, effects, traps, memory, stack, and control flow.
  - [x] Establish the first bounded terminal-pair matcher and move the AArch64
    compare-zero/branch-nonzero producer onto an exact declarative descriptor
    covering selected kinds, alternatives, operands, named physical units,
    register views, encoded effects, liveness continuity, and dead flags. Keep
    the existing independent validator as a separate replay implementation.
  - [x] Prove the next descriptor vocabulary with an exact AArch64
    same-view `CopyI64; ReturnI64` elision. The shared matcher now admits exact
    per-operand read/write contracts and named cross-instruction relations for
    equal virtual registers and equal physical view/storage, while the rule's
    independent replay, canonical codec, attempt history, and disposition
    roster remain rule-local. The exact rule now has a build selection and sole
    machine-catalog row; a rule-neutral carrier retains its disposition through
    encoding, zero-byte layout, exit custody, realization, fragment/object, and
    callable publication. No ordinary lowering emits terminal
    `CopyI64; ReturnI64`; fixed-view recovery emits a different shared-entry
    shape and composition is a typed refusal. Applied positive coverage remains
    at the machine-rule boundary, while compiler-facing coverage proves honest
    deterministic zero-action selection and publication.
  - [ ] Generalize beyond the body-tail/terminator pair only when a second
    exact rule proves a non-terminal-pair topology; the copy-elision rule
    deliberately retains the existing bounded topology.
- [ ] Add exact copy removal, redundant extension removal, address folding,
  compare/test selection, and scheduling where independently verifiable. The
  admitted same-view return-copy case is evidence for the first family, not
  completion of general copy removal or a claim that current lowering produces
  its exact candidate.
- [x] Add target cost models as non-authoritative identities. The V1 entrance
  binds exact native-target identity to retained exact-or-bounded size
  knowledge while keeping latency explicitly unavailable. Machine-rule
  semantic validators are architecture-guarded from importing the model.

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
- [>] Add a sandboxed external policy boundary with timeout/resource limits
  and an explicit fallback. The neutral `omega-bounded-process` prerequisite
  now owns structured command preparation, process-container cleanup, concrete
  resource limits, bounded duplex capture, wall-clock deadlines, and typed
  failures on behalf of both Unix process groups and Windows Jobs. Resolver
  execution and Git acquisition consume it without retaining duplicate
  lifecycle or capture engines. These are resource and cleanup controls, not a
  filesystem, executable, credential, or network sandbox. A dormant
  compiler-private adapter now exchanges canonical V2 decision logs through
  exact request, response, stderr, aggregate-output, deadline, cleanup, and
  process limits. It independently requires the context, point order, input,
  rule, and complete candidate surfaces to remain identical, permits only a
  schema-legal action change, and makes `FailClosed` versus
  `UseRecordedBaseline` explicit. Ordinary builds exclude it; even the
  experimental feature cannot construct its opaque verified-sandbox
  invocation because there is intentionally no production constructor. A real
  platform sandbox backend, capability construction, and build-level
  activation remain engineering work.
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
  idempotence canaries. GVN roster positions 9--15 now have exact complete
  contract custody across all seven total scalar identity families and all 26
  laws. Positive replay binds each family's validator identity and `-1` cost;
  the corruption matrix rejects all 42 directed cross-family relabellings plus
  unknown identities, analysis/invalidation supersets and subsets, wrong
  safety, and wrong cost. The seven producers now live in a mirrored test
  taxonomy with isolated fixed-point coverage for shift-zero and wrapping
  multiply-zero, plus runtime-disabled, deterministic, and budgeted pass
  canaries. GVN roster positions 0--8 now also bind all nine scalar
  common-subexpression rules to exact scope-specific analysis and invalidation
  sets, safety, `-1` cost, and validator identity. Their corruption matrix
  rejects all 72 directed cross-rule relabellings plus unknown identities,
  analysis/invalidation supersets and subsets, wrong safety, and wrong cost.
  One nine-fixture pass matrix proves default-disabled execution, exact
  manifest/fact and ledger/commit custody, repeated-run determinism,
  deterministic budget exhaustion, and idempotence for every row; direct
  missing-evidence and corruption refusals now cover the previously weaker
  proof-certified and compatible-policy dominating rows. The three
  dead-scalar semantic families now bind exact complete contracts, `-1` cost,
  and distinct validator identities across the dead-pure-scalar and
  proof-check suites. Their six directed relabellings plus unknown identities,
  analysis/invalidation supersets and subsets, wrong safety, and wrong cost
  fail closed; a three-fixture pass matrix pins default-disabled execution,
  repeated-run determinism, deterministic budget exhaustion, manifest/ledger
  custody, and output idempotence. All twelve proof-check-elision rows now pin
  complete exact contracts, proof safety, `-1` cost, and validator identity;
  their 132 directed relabellings plus every contract-axis corruption fail
  closed. A twelve-fixture engine matrix pins default-disabled behavior, exact
  roster evaluation counts, repeated-run and budget determinism, exact
  manifest/fact and ledger/commit custody, accepted-proof retention,
  source-obligation pruning, and fixed-point idempotence for every row. A
  three-leaf SCCP engine matrix now gives all 39 exact roster rows isolated
  fixtures and pins default-disabled behavior, exact roster evaluation counts,
  repeated-run and iteration-budget determinism, typed fact custody through
  declarations and manifests, exact validator/commit/ledger joins, and
  fixed-point idempotence. A graph-shape fixture matrix now gives all seven
  ControlFlowCleanup roster rows isolated whole-engine coverage for
  default-disabled behavior, exact evaluation and commit counts, repeated-run
  and iteration-budget determinism, declaration/manifest/fact/ledger custody,
  and fixed-point idempotence. The x86 sign-extending MOV-r64-imm32 rule now pins
  exact successful usage and first-over-boundary failure across all five work
  axes. Its direct publication path independently sums baseline and selected
  bytes, binds the complete and phase-local selection identities, and rejects
  authenticated one-field substitutions of all eight generic post-allocation
  custody fields after the enclosing V9 manifest identity is recomputed. The
  x86 XOR-zero rule now has the same exact five-axis success/boundary matrix,
  independently rejects action corruption, and runs all eight authenticated
  custody mutations through the shared post-allocation corruption harness. The
  x86 zero-extending MOV-r32-imm32 rule now pins the same five exact usage and
  first-over-boundary axes, retains its independent action-corruption refusal,
  and crosses that shared eight-field authenticated-custody harness as well.
  AArch64 MOVN now pins exact successful usage and the first-over-boundary
  failure on all five work axes, independently rejects authenticated action
  corruption, and crosses the same shared eight-field custody harness. A
  two-fusion AArch64 CBNZ fixture now pins a nonzero exact-success and
  first-over-boundary matrix for all five work axes; reauthenticated action
  corruption is independently refused in its named operational leaf, and its
  direct publication crosses all eight shared custody mutations. True
  same-view copy elision now adds a two-pair independently replayed fixture with
  exact usage `{5, 2, 2, 2, 3}` and representable first-over failures on all
  five work axes; its existing applied/negative/equality, disabled,
  deterministic, action/codec corruption, eight-field publication custody,
  target, and composition evidence remains intact. True second-application
  idempotence remains open for all six rules: each
  publishes an immutable encoding-choice artifact, not a rewritten
  `PostAllocationMachinePlan` that it can honestly consume again.
  Repeated reconstruction remains determinism evidence and is not relabelled as
  idempotence. The exact x86 rel32-to-rel8 layout rule now pins exact successful
  usage and first-over-boundary failure on all five work axes with a
  two-relaxation fixture. Reauthenticated action-byte corruption is
  independently rejected at the public realization boundary; direct
  publication rejects substitutions of all five function-relative layout
  custody fields plus authenticated phase-local selection, baseline/final
  layout, relaxation-identity, and rel8 exit-custody mutations. Its immutable
  replacement layout is likewise not reconsumable as the baseline input. The
  active-resident immediate-U64 multi-use rematerialization rule now has an
  operational matrix covering explicit disablement, typed no-pressure
  refusal, applied x86-64/AArch64 cases, repeated reconstruction across every
  recovery carrier, exact component work usages and first-over budgets, direct
  action and enclosing-custody corruption, and a genuine second application
  over rebuilt post-transform analyses that deterministically reaches
  `NoAction`. The shared-entry fixed-view-copy recovery rule now also pins
  explicit disablement without hidden fallback, exact five-field work usage on
  x86-64 and AArch64, exact-envelope success, deterministic refusal at every
  representable first-over budget, and the nonzero budget-domain floor for
  unit-valued axes. Its immutable transformed carrier is not accepted as a
  second input, so no false idempotence claim is made. The exact incoming-u12
  add lowering rule now has its own x86-64/AArch64 matrix: true default-disabled
  projection, exact-subtract cross-rule refusal without fallback, applied and
  repeated deterministic execution, exact aggregate work
  `{19, 8, 123, 6, 9}`, exact-envelope success, first-over failure on all five
  axes, action-corruption refusal, and an honest validated terminal no-change
  attempt. Its transformed selected carrier is not a fresh staging input, so
  that within-run fixed point is not relabelled as second-invocation
  idempotence. The sibling incoming-u12 subtract rule now mirrors that named
  matrix on x86-64 and AArch64: true default-disabled projection, exact-add
  cross-rule refusal, deterministic applied execution, exact aggregate work
  `{19, 8, 123, 6, 9}`, all five first-over budget failures, typed action
  corruption, and an independently validated terminal no-change attempt. Its
  semantic boundary admits the exact U12 maximum 4095 and deterministically
  refuses 4096. Repeated reconstruction remains determinism evidence and the
  terminal attempt remains within-run fixed-point evidence; neither is called
  public second-invocation idempotence. The remaining operational axes and
  other rule families are not yet complete.
- [x] Cross-rule phase-composition matrix, including fail-closed unsupported
  combinations.
- [x] Add randomized valid-Psi and selected-machine differential corpora. The
  versioned V2 corpus deterministically generates 64 target-paired records from
  one fixed seed, verifies its checked-in shape and record digest, independently
  interprets both Boolean paths of every valid Terminal-Psi artifact, and runs
  each lane twice while comparing optimizer identities, pass manifests,
  commits, ledgers, pre/post-physical manifests, custody receipts, selected
  bytes, and resolved layouts. The Psi lane exercises exact wrapping-add SCCP;
  the separately interpreted selected-machine lane exercises x86-64
  zero-extended `MOV r32, imm32`, sign-extended `MOV r64, imm32`, and AArch64
  shortest `MOVN` materialization with independent ISA decoders/validators.
  V2 additionally interprets both paths of a third immediate-leaf artifact,
  applies the exact host post-allocation materialization rule twice, links the
  optimized bytes, executes both Boolean paths, and requires their U64 result
  to equal the interpreter result. `OMEGA_OPTIMIZER_CORPUS_CASE=<n>` replays one
  fully printed record. The SCCP and selected-machine fixtures remain separate
  because folded SCCP with retained dead source literals is not yet an admitted
  selected-lowering shape; the corpus does not claim that unsupported composed
  carrier.
- [ ] Add supported target/OS allocator, encoding, unwind, object, and callable
  matrices. The first applied selected-lowering publication matrix now covers
  exact incoming-u12 add and subtract on Linux x64, Windows x64, Linux Arm64,
  and macOS Arm64. Every row forces two literal-fold commits and runs twice
  while pinning target and phase selections, encoding, ELF/COFF/Mach-O object
  form and text bytes, callable ABI, frameless exit policy, codecs, manifests,
  and deterministic container bytes. This does not claim unwind coverage:
  physical spill insertion, final frame layout, and unwind authority remain
  compiler prerequisites under P5 rather than an owner language decision.
- [ ] Add versioned compile-time, memory, code-size, and runtime benchmarks.
- [x] Publish exact-rule release notes and rollback procedures. The versioned
  V1 inventory names all 17 canonical exact rules, phases, target
  applicability, experimental status, exact rollback spelling, supported
  compositions, and fail-closed carrier limits; its native-only runbook owns
  receipt capture, verification, and restoration.
- [x] Require owner-reviewed promotion criteria per exact rule; never promote
  an implicit broad level. The standalone rollout architecture gate rejects
  canonical inventory drift and any `Recommended` or `Default` row without a
  matching completed exact-name evidence record. Broad optimization levels
  remain absent.
