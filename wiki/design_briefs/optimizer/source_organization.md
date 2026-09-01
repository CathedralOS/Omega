# Optimizer Source Organization

This is the source-navigation contract for the optimizer. The design entrance
is [optimizer_architecture.md](../optimizer_architecture.md); executable work
belongs in [TASKS_OPTIMIZER.md](../../../TASKS_OPTIMIZER.md), and completed
refactor history belongs in Git.

## Human navigation contract

A human opening an optimization stage must land on one small file that answers
five questions without repository-wide search:

1. What validated value enters?
2. Which exact optimization names are eligible?
3. Where is their canonical order declared?
4. What function coordinates proposal and independent validation?
5. What validated value leaves?

The entrance is normally `mod.rs`. It owns the coordination join, but not the
mechanics. It points directly to an adjacent catalog and the next semantic
rung. A short re-export wall is not an entrance.

Every governed `lib.rs` and `mod.rs` declares exactly one role:

- `crate map`: names the crate's major responsibilities;
- `stage group`: groups neighboring boundaries and performs no execution; or
- `executable entrance`: owns one transformation, replay, or admission join.

The architecture audit rejects missing, duplicate, or false role declarations.
An executable entrance must contain its named coordination seam; a group may
not silently accumulate execution.

## Pattern adopted from Squalr

The useful Squalr scan-rule path is:

```text
registry -> ordered enabled rules -> mapped scan plan -> scanner dispatch
```

The registry is short, adding or removing a built-in is obvious, and each rule
has a named implementation leaf. Omega retains that navigability while using
typed exact names, deterministic order, immutable plans, independent replay,
and identity-bound receipts instead of string keys, unordered maps, global
mutable state, or unchecked in-place mutation.

The Omega rule path is:

```text
stage/mod.rs
  -> catalog.rs
  -> <family>/mod.rs
  -> <exact-rule>/mod.rs
  -> {model, compute, validate, identity, codec, tests}.rs
```

Every rung narrows the question. Do not introduce generic `rule.rs`,
`rules.rs`, `helpers.rs`, or a large mixed match as the next rung. Exact rule
implementations use the optimization name as their file or directory name.

## Compiler and optimizer-execution entrances

The build/compiler hooks and two top-level optimizer-pipeline routes are:

| Responsibility | Entrance | Next rung |
|---|---|---|
| Build-authored exact selection | `omega-build-evaluation/src/optimization/mod.rs` | `vocabulary.rs`, `selection.rs` |
| Injected exact-name vocabulary | `omega-compiler/src/pipeline/optimization/build_vocabulary/mod.rs` | sole `fragments.rs` mapping used by both prelude variants |
| Checked selection custody | `omega-compiler/src/pipeline/optimization/checked_handoff/mod.rs` | retained selection, identity, and report request |
| Native compiler realization | `omega-compiler/src/compiler/optimization/mod.rs` | `admission.rs`, `rollback/`, `native_realization.rs` |
| Verified Psi optimization | `omega-optimization-pipeline/src/coordination/psi_optimization/mod.rs` | `request.rs`, exact Psi catalog, independent abstract projection |
| Native physical continuation | `omega-optimization-pipeline/src/coordination/physical_pipeline/mod.rs` | `routes/composition/`, then one named route |
| Attached Unit abstract-to-target lowering | `omega-abstract-operations-to-target-operations/src/lowering/unit.rs` | `unit/setup.rs`, `unit/body.rs`, then named call, return, scalar, and structural leaves |

The ordinary empty-selection compiler path does not enter the explicit Psi
optimizer. Physical routing consumes the exact selected phase set and one
typed optimized target value; it does not invent optimization profiles.

Attached Unit lowering follows the same readable descent even though it is a
mandatory lowering lane rather than a selectable optimization stage. Its
68-line entrance owns preflight, ABI/parameter preparation, body lowering, and
final target assembly. Stateful operation dispatch lives one rung below it;
boundary, scalar, structural-call, structural-store, and return mechanics stay
in their named sibling leaves instead of inheriting a large entrance namespace.

The guard governs those focused hook subtrees, not entire build/compiler
crates. General source assembly, frontend/trust coordination, subsystem
selection, and public checked-compilation accessors remain with their actual
owners. The optimized semantic program-entry/wrapper subtrees and complete
selected/assigned representation crates are governed alongside the lowering
stages that consume them.

The randomized differential corpus follows the same entrance contract outside
the production crates. `tests/native-differential/tests/optimizer_corpus.rs`
owns admission and replay dispatch in one small file, then points directly to
the adjacent `optimizer_corpus/` leaves: `generator.rs`, `manifest.rs`,
`psi.rs`, `selected_machine.rs`, and the host-only `native.rs`. The current
checked-in V2 corpus identity lives under
`tests/native-differential/corpora/optimizer/v2/`; it is data custody, not
a second schedule or rule registry.

`omega-image-emission/ranked_u32_countdown` is deliberately outside this
guard. It independently replays a language-level ranked execution carrier but
owns no optimization selection, catalog, proposal, or optimized stage result.
Image publication needs one coherent publication architecture boundary; one
special-case lane is not an optimizer root.

The prerequisite composed-Unit carrier follows the same navigational shape.
Its typed-to-checked `composed_control.rs` entrance coordinates `topology`,
`custody`, `guards`, `leaves`, `assembly`, and the exact larger-graph sibling
`prefixed_control`; its checked-to-Terminal entrance coordinates `admission`,
independent `custody` replay, `catalogs`, `emission`, and `prefixed_control`,
with parameterless target closure isolated in `internal_calls`. Both nested
consumer entrances descend through independent `admission` and `emission`
rungs: `internal_calls` owns target-plan and transitive-closure replay while
`prefixed_control` owns finite scalar-prefix chains before one conditional
frontier, and `nested_control` owns general finite acyclic Boolean control
graphs with arbitrary checked state targets, exact scalar handoffs, and shared
effect leaves. Its typed producer has a small coordinator over `topology`,
`operations`, and `assembly`; its consumer retains the parallel `admission`,
`operations`, and `emission` split. The topology/admission rungs own graph
classification and independent reachability/cycle walks; the operation rungs
own the exact finite pre-terminator sequence and preserve effect-before-branch
source order. Internal and boundary calls share this rung; boundary emission
reuses the ancestor call-operation projector so source-call occurrences are
recorded once rather than reconstructed by the graph route. Provider discovery
matches those executable operations to flow calls by exact source coordinate,
so named-transition call facts stay with topology. Implicit `self` likewise
stays attachment context: scalar-edge facts retain the raw target position but
the nested-control plans carry separate dense scalar indices.
Balanced, right-deep, convergent, and call-prefixed shapes do not receive
sibling routes.

Optimizer-only ranked-cycle admission follows the same visible descent.
`omega-optimization-validation/src/unit_validation/context/mod.rs` remains the
small executable context-validation entrance. Its `ranked_cycles` stage group
descends through `graph.rs`, `topology.rs`, and `components.rs` for independent
Terminal/current reconstruction; `model.rs` owns the structural component ID
and opaque validated carrier, `freeze.rs` coordinates component preservation,
and `freeze/normalized_component.rs` independently owns the sole authenticated
zero/one relocation normalization. `replay.rs` owns post-run rederivation. The
33-line stage-group entrance is not a second public validation entrance.
The mirrored `omega-optimization-pipeline/src/tests/cyclic_psi.rs` leaf builds a
real source countdown and pins CFG, dominator, SCC, loop, and liveness analysis
plus topology/frozen-body corruption. General cyclic authority and cyclic
rewrite consumers remain outside this taxonomy.
The first authority-sensitive consumer enters through the small
`countdown_induction/mod.rs` coordination file. Its producer consumes the
ordinary `LoopForest` and requires an exact reducible `LoopRegion` match against
component and ranking custody. The replay side descends through
`replay/region.rs` and independently reconstructs edge rows, boundaries,
reachability, and header dominance without calling CFG, dominator, SCC, or loop
producers. This bridge grants analysis facts only; execution and rewrite
authority remain closed.
Exact loop-invariant constant discovery descends one rung through the 44-line
`countdown_invariant_constants/mod.rs` coordination entrance. `model.rs` owns
the zero/one roles and authenticated snapshot, `compute.rs` consumes counted-
loop custody to locate certificate-owned input-free integer constants, and
`replay.rs` independently reconstructs their component, prospective preheader,
definition, provenance, fuel, and effect bindings without calling the producer.
Both sides search the complete function but accept only the original role
blocks or sole role-ordered canonical preheader suffix. `validate.rs` is the
sole admission seam. The mirrored cyclic-Psi leaf pins the exact pair, empty
acyclic behavior, corruption axes, and post-relocation reauthentication; this
remains analysis authority rather than LICM or rewrite authority.
Exact placement discovery descends one more rung through the 58-line
`countdown_invariant_constant_placement/mod.rs` entrance. `model.rs` owns the
opaque revision/Terminal snapshot and destination/consumer rows; `compute.rs`
joins component, counted-loop, and invariant custody; `replay.rs` independently
rescans definitions, uses, provenance, the preheader jump, and exact consumers;
and `validate.rs` alone seals the result. The mirrored cyclic-Psi leaf covers
the exact zero/one destinations, 31 retained corruption axes, stale revision,
empty acyclic behavior, and independent original-or-preheader location replay.
No rewrite or generic-analysis-manager path is introduced.
The ranking validator's current-IR side descends through the private
`countdown_ranking/current/invariant_constants.rs` resolver. The existing
`current.rs` coordinator still derives the complete certificate, while the new
leaf owns only unique zero/one lookup in the original role block or canonical
preheader suffix. Ranked-cycle validation reconstructs current/Terminal ranking
before the preservation-aware freeze. That freeze admits only the exact
certificate-owned zero/one canonical-suffix relocation while requiring source
provenance and fuel to remain identical; every other component node remains
positionally frozen. The mirrored relocation-shaped cyclic-Psi leaf and
layering guard prove this is independent validation mechanics, not an import
from optimizer placement analyses or a rewrite entrance.
`VerifiedPsiOptimizationSession::from_transformed` then runs full transformed
validation and rebinds the session to the new component/ranking custody before
counted-loop, invariant, and placement analyses are reconstructed. Layering
forbids those analysis leaves from importing the ranking resolver or freeze
normalizer. Ranked rewrites enter through the `ranked_rewrites/mod.rs` stage
group. Its exact `countdown_invariant_constant_relocation/mod.rs` child is the
70-line executable proposal -> independent validation -> atomic application
join over separate `model`, `propose`, `validate`, `apply`, and `apply/realize`
leaves. The application leaf owns transformed-session rebinding and the single
canonical ledger record; the stage group owns no execution and there is no
generic LICM registry. The mirrored cyclic-Psi leaf pins atomic pair motion,
deterministic budget failure, stale revision rejection, partial normalization,
exact provenance/fuel rows, custody reconstruction, and fixed-point behavior.
The nested
consumer `prefixed_control/mod.rs` is itself a small coordinating entrance over
its two rungs. It reuses the ancestor catalogs and internal-call leaf emitters
after independently admitting the scalar prefix rather than copying their
policy into a second route. Prefix depth remains a loop inside the existing
producer leaf and the consumer's `admission`/`emission` pair; it does not create
depth-named modules or routes. Internal-call recursion remains inside its
`admission` leaf; adding tested closure depth does not add a depth-named module
or another orchestration layer. Shared provider discovery and call-target
catalog admission take variable-length state slices, so a control-state call
and a leaf call share one independently rejoined target closure. The exact two-arm linear
custody proof retains its fixed pair because widening catalog arity does not
silently widen ownership semantics.
Shared state-entry claim construction lives at the honest
`attached_unit/claims.rs` ancestor rather than in either control-flow route.
Focused source and replay tests live in the `composed_claims`,
`composed_internal_calls`, `composed_unit_claims`, and
`composed_unit_internal_calls` files; the exact four-state family lives in
`composed_prefixed_control` and `composed_unit_prefixed_control`. The transitive
internal target closure lives in `composed_transitive_internal_calls` and
`composed_unit_transitive_internal_calls`; the two-frontier family lives in
`composed_nested_control` and `composed_unit_nested_control`. Extending the
carrier therefore does not grow the legacy call or structural-control
matrices.

## Rule-owning stage entrances

One stage entrance consumes selections. One adjacent catalog owns exact
enablement and order.

| Phase | Entrance | Sole catalog | Next rung |
|---|---|---|---|
| Mandatory legalization | `omega-target-operations-to-selected-instructions/src/legalization/mod.rs` | `legalization/catalog.rs` | `source/`, `replay/` |
| Psi | `omega-psi-optimizer/src/rules/mod.rs` | `rules/catalog.rs` | `passes/<exact-pass>/` |
| Selected lowering | `omega-regalloc/src/rules/selected_lowering/mod.rs` | adjacent `catalog.rs` | `literal_fold/` |
| Allocation recovery | `omega-regalloc/src/rules/allocation_recovery/mod.rs` | adjacent `catalog.rs` | `fixed_view_copy/`, `pressure_rematerialization/` |
| Post-allocation machine | `omega-machine-optimizer/src/rules/mod.rs` | `rules/catalog.rs` | `rules/peephole_matching/`, then `rules/<isa>/<exact-rule>/` |
| Function-relative layout | `omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/mod.rs` | adjacent `catalog.rs` | compute and independent validation |

Fixed/precolored interval analysis is deliberately absent from the selectable
rule table. Its 24-line
`omega-regalloc/src/analyses/fixed_precolored_intervals/mod.rs` entrance joins
`compute.rs` to independently keyed `replay.rs` through the sole `validate.rs`
admission seam; `model.rs` and `identity.rs` own its closed point-interval and
receipt vocabulary. The mirrored cross-target tests live below
`register_allocation/fixed_precolored_intervals/`, and architecture guards name
both production and test ladders while preventing replay from importing the
producer. This is fixed-constraint evidence, not home selection, copy
insertion, range splitting, or a selectable optimization.

Logical spill planning is deliberately absent from this rule table. It is a
non-selectable allocation decision below
`omega-regalloc/src/allocation/logical_spill_operations/mod.rs`. That small
entrance coordinates `compute/` and independent `validate/` rungs; `model.rs`
and `identity.rs` own vocabulary, while `codec/` owns versioned transport. The
test taxonomy mirrors those boundaries, and the cross-target pipeline leaf is
`register_allocation/logical_spill_operations.rs`. No allocation-recovery
catalog or user-visible optimization name is duplicated for this evidence.

Stack-slot coloring follows the same non-selectable allocation taxonomy at
`omega-regalloc/src/allocation/stack_slot_coloring/mod.rs`. Its small entrance
coordinates `compute/` and independently implemented `validate/` rungs;
`compute/intervals.rs` and `compute/first_fit.rs` expose the lifetime and
coloring descent, while `model.rs`, `identity.rs`, and `codec/` own the closed
artifact vocabulary and transport. Mirrored `tests/` cover the exact first-fit
contract, and the cross-target pipeline leaf is
`register_allocation/stack_slot_coloring.rs`. It does not add an optimization
name or enter the allocation-recovery catalog.

Abstract spill insertion continues below
`omega-regalloc/src/allocation/abstract_spill_insertion/mod.rs`. Its entrance
joins the validated logical-operation and stack-slot receipts, while `model.rs`
and `identity.rs` own the schedule vocabulary and `compute.rs` and
`validate.rs` retain separate production and replay custody. The mirrored
pipeline leaf is `register_allocation/abstract_spill_insertion.rs`.

Logical reload home assignment lives beside it at
`omega-regalloc/src/allocation/reload_value_homes/mod.rs`. That executable
entrance joins the source carriers and sends the producer result through
independent replay. `compute.rs` owns the sorted linear proposal, `replay.rs`
owns point-indexed reconstruction, `replay/mechanics.rs` owns only replay-local
interference and accounting mechanics, and `model.rs`, `identity.rs`, and
`validate.rs` own the closed artifact and admission boundary. Its mirrored
pipeline leaf is `register_allocation/reload_value_homes.rs`. Neither stage is
a selectable optimization rule.

Synthetic reload binding continues at
`omega-regalloc/src/allocation/synthetic_reload_values/mod.rs`. Its 27-line
executable entrance joins validated insertion and home custody; `compute.rs`
owns direct canonical traversal, `replay.rs` independently owns keyed
reconstruction and sorting, and `model.rs`, `identity.rs`, and `validate.rs`
own the closed namespace artifact. The pipeline mirror is
`register_allocation/synthetic_reload_values.rs`. The architecture guard names
all three spill-related executable entrances explicitly instead of treating
meaningful joins as generic stage groups.

Epoch-one logical recovery actions continue at
`omega-regalloc/src/allocation/spill_recovery_actions/mod.rs`. Its 39-line
executable entrance joins selected/range/legality, first insertion, worklist,
and second-victim custody, then routes production through `compute.rs` and
independent reconstruction through `replay.rs` and `validate.rs`. `model.rs`
and `identity.rs` own the closed logical storage/store/reload/rewrite
vocabulary. The mirrored pipeline leaf is
`register_allocation/spill_recovery_actions.rs`; no selectable optimization
name or real memory/frame authority is introduced.

Generalized epoch-zero/one insertion lives at
`omega-regalloc/src/allocation/generalized_spill_insertion/mod.rs`. Its 28-line
executable entrance joins the two validated logical-action sources. `compute.rs`
owns direct first-fit coloring and event construction, while `replay.rs`
reconstructs the source rows and occupied offsets independently; `model.rs`,
`identity.rs`, and `validate.rs` own the closed carrier and admission boundary.
The mirrored public fixture is
`register_allocation/generalized_spill_insertion.rs`. The architecture guard
registers both the entrance and semantic ladder explicitly; no selectable rule
or real memory/frame authority is introduced.

Generalized reload-home reanalysis continues at
`omega-regalloc/src/allocation/generalized_reload_value_homes/mod.rs`. Its
65-line executable entrance joins all allocation and generalized-insertion
roots, then validates the producer plan by independent replay. `compute.rs` and
`replay.rs` are small group maps: each descends through named `roots`, `sources`,
home/interference, schedule/timeline, and `work` leaves, with no shared producer
mechanics. `model.rs`, `identity.rs`, and `validate.rs` own the closed outcome
carrier and receipt boundary. The mirrored pipeline leaf is
`register_allocation/generalized_reload_value_homes.rs`; the architecture guard
registers the entrance, full semantic ladder, and independent-replay firewall.
This is compiler-private allocation evidence, not a selectable optimization
rule or real instruction/memory/frame authority.

Epoch-two recovery work begins at
`omega-regalloc/src/allocation/generalized_spill_recovery_worklist/mod.rs`.
Its 25-line executable entrance projects validated generalized pressure through
`compute.rs` and independently keyed `replay.rs`; `model.rs`, `identity.rs`,
and `validate.rs` own the closed work-item vocabulary and admission receipt.
The mirrored pipeline leaf is
`register_allocation/generalized_spill_recovery_worklist.rs`. The architecture
guard registers the entrance and complete ladder and forbids replay from
calling producer mechanics. This remains compiler-private scheduling custody,
not a selectable rule or physical spill realization.

Epoch-two victim choice continues at
`omega-regalloc/src/allocation/generalized_spill_recovery_choice/mod.rs`. Its
56-line executable entrance joins direct blocker traversal to independently
keyed replay. `model.rs`, `identity.rs`, and `validate.rs` retain the closed
resident/contender/choice vocabulary and admission receipt, while `compute.rs`
and `replay.rs` own separate reconstruction mechanics. Their sibling
`original_eligibility.rs` leaves separately prove an original resident's
selected role and post-pressure use suffix for the guarded-original policy;
neither direction calls the other's mechanics. The mirrored pipeline
leaf is `register_allocation/generalized_spill_recovery_choice.rs`; architecture
guards register the full ladder and prohibit validator reentry into proposal.
This is compiler-private choice evidence, not an optimization catalog or
physical spill action.

The exact guarded-original prerequisite does not add another regalloc
coordinator. Its distinct legalized source/replay leaves retain
`r + ((r + (a + b)) + (b + r))`; named selected construction and block-replay
leaves carry the same fork/join into the existing choice entrance. The mirrored
`register_allocation/guarded_original_spill_recovery_choice.rs` leaf pins the
exact original/reload contenders, choice, budgets, and corruption fences. The
separate original-action leaf owns the following V2 handoff; older recipe tags
and the choice taxonomy stay unchanged.

Epoch-two logical action planning continues at
`omega-regalloc/src/allocation/generalized_spill_recovery_actions/mod.rs`.
Its 43-line executable entrance exposes the preserved V1 reload-victim entry
and a separate V2 guarded-original entry while joining direct traversal to
independently keyed replay without hiding either lifecycle. `model.rs` owns the
closed victim and logical store/reload/rewrite vocabulary, `identity.rs` owns
versioned custody, and `validate.rs` compares replay before sealing a receipt.
The `compute/` and `replay/` subtrees make reload and original reconstruction
separate named leaves; V2 additionally binds selected-plan and live-range
roots, while V1 retains its original signature and exact identity encoding. The
mirrored pipeline leaf is
`register_allocation/generalized_spill_recovery_actions.rs`;
architecture guards register the complete ladder and forbid validation from
calling producer mechanics. This stage remains target-neutral logical custody,
not physical spill insertion or publication authority.

Recursive logical insertion continues at
`omega-regalloc/src/allocation/recursive_spill_insertion/mod.rs`. Its 26-line
executable entrance visibly joins direct projection to independent keyed
replay. `model.rs` owns the typed prior/reload/original epoch-two action sources,
the distinct original-VReg/reload-action stored-value vocabulary, complete
logical slots, and canonical events; `identity.rs` and `validate.rs` own custody
and receipt admission while `compute.rs` and `replay.rs` retain separate
mechanics. V1 remains a reload-only, byte-stable identity domain. A separately
named V2 policy accepts only matching original victim/store rows and retains the
selected VReg through the schedule and existing spill-pseudo boundary. The
mirrored pipeline leaves are `register_allocation/recursive_spill_insertion.rs`
and `register_allocation/original_recursive_spill_insertion.rs`; architecture
guards register the complete ladder and forbid validation from calling producer
mechanics. This is still an abstract spill-area schedule, not physical
memory/frame or publication authority.

Final recursive home closure enters through the 60-line
`recursive_reload_value_homes/mod.rs` coordination file. `model.rs`,
`identity.rs`, and `validate.rs` own its closed carrier and receipt boundary.
Production descends through `compute/{roots,sources,schedule,homes,work}`;
independent validation mirrors the questions through
`replay/{roots,sources,timeline,homes,work}` without calling compute. No leaf
exceeds 200 lines. The mirrored `register_allocation/recursive_reload_value_homes.rs`
test owns both victim paths, exact rosters, corruption, budget, and cross-target
custody; architecture guards register the entrance and full ladder.

Homed pseudo closure enters through the 26-line
`spill_pseudo_instructions/homed/mod.rs` coordination file beside the unchanged
V1 entrance. `model.rs`, `identity.rs`, and `validate.rs` own the distinct V2
policy, carrier, identity, and receipt; `compute.rs` owns direct traversal and
`replay.rs` independently reconstructs keyed storage/instruction/rewrite rows
with destination homes. The mirrored
`register_allocation/homed_spill_pseudo_instructions.rs` leaf owns both victim
paths, V1 byte stability, corruption, budgets, and cross-target custody.
Architecture guards register the entrance and ladder and prohibit replay from
calling producer mechanics.

Abstract spill-effect projection enters through the 24-line
`abstract_spill_memory_effects/mod.rs` coordination file. `model.rs`,
`identity.rs`, and `validate.rs` own its closed V1 policy, target-neutral rows,
identity, and receipt. `compute.rs` descends to separate storage/work leaves;
`replay.rs` independently reconstructs keyed storage, effects, order, and work
without importing producer mechanics. The mirrored
`register_allocation/abstract_spill_memory_effects/mod.rs` test entrance
descends through fixture, positive/effect, corruption, and budget leaves.
Architecture guards register the full ladder and enforce the absence of real
memory/frame/fault/encoding authority.

Abstract spill-access constraint planning enters through the adjacent 24-line
`abstract_spill_access_constraints/mod.rs` coordination file. `model.rs`,
`identity.rs`, and `validate.rs` own the closed V1 carrier and receipt;
production descends through `compute/{accesses,dependencies,work}` while keyed
replay independently answers the same questions through
`replay/{accesses,dependencies,work}`. Every leaf remains below 200 lines. The
mirrored pipeline test owns both victim lineages, targets, corruption, and all
five budget axes; architecture guards register the complete ladder and prohibit
executable memory, frame, fault, alias, encoding, or publication claims.

Function-relative V9 mutation coverage enters through the five-line
`function_relative_manifest_mutation_matrix/mod.rs` stage group. It descends
into separate `fixture`, `manifest_fields`, `manifest_wire`, `wire_offsets`,
and `custody` leaves; no leaf mixes public-route construction, logical field
mutation, byte geometry, wire rejection, and receipt-root mutation. The source-
organization ladder registers the complete test taxonomy beside the owning V9
codec taxonomy.

ProgramStorage semantic-wrapper manifest coverage likewise enters through the
five-line `manifest_mutation_matrix/mod.rs` stage group. Its `fixture`,
`fields`, `wire`, and `wire_offsets` leaves separate canonical local
object/manifest construction, reauthenticated logical mutation, and byte-axis
rejection. The matrix deliberately stops at the existing canonical manifest
replay; it does not claim the unavailable physical adapter or a full staged
publication route.

Receiving terminal-authority classification enters through the 90-line
`realization/terminal_authority_policy/mod.rs` coordinator. It visibly owns
explicit foreign-row admission and current-policy construction, while named
`model`, `normalized_foreign`, `classification`, `inventory`, and `commitment`
leaves own the corresponding vocabulary and mechanics. Inventory and foreign-
row tests mirror that split. The entrance remains meaningful without restoring
the former 810-line mixed policy, codec, inventory, and test file.

Terminal native-artifact realization now enters through the 65-line
`realization/mod.rs` coordinator. Its two public routes visibly distinguish
ordinary realization from checked boundary-scope custody, then join at the
64-line `native_artifact.rs` lifecycle before descending through named input,
provider, machine-code, and output stages. Installed selected-provider closure
review independently enters through the 52-line
`realization/terminal_authority_review.rs` coordinator. Its `context`,
`reviewer`, and `operations` leaves separate exact lookup and reachability,
recursive provider-closure expansion, and exhaustive authority-edge
classification; its test matrix lives in a parallel `tests` leaf. These
entrances preserve exact checked-scope validation and recursive closure review
without returning to a mixed 920-line implementation-and-test file.

The external-policy execution prerequisite has its own dormant compiler
entrance at `omega-compiler/src/compiler/optimization/external_policy/mod.rs`.
That file alone coordinates the opaque sandbox capability, exact transport
limits, canonical response matching, and explicit fallback settlement; named
leaves own each concern. The module is absent from ordinary builds and has no
production capability constructor until a real platform sandbox exists.

Offline policy data enters separately through the 41-line
`omega-optimization-policy-offline/src/corpus/mod.rs` coordinator. It joins
canonical V2-log capture to independent corpus validation, then descends
through named `model`, `identity`, `split`, and `codec` leaves. Mirrored
admission, codec, and splitting tests sit below `corpus/tests/`; the architecture
inventory registers the complete ladder and forbids compiler, build, pipeline,
Psi-optimizer, and process-tooling dependencies. This is corpus custody, not a
second policy catalog or compiler activation path.

The 44-line `src/bin/omega-optimization-policy-offline/main.rs` command
entrance coordinates the closed `capture`, `train`, `evaluate`, and
`regression` vocabulary. Its adjacent `arguments`, `capture`, `inputs`,
`training`, `evaluation`, `publication`, and `error` leaves make positional
admission, strict artifact custody, fixed report-split selection, create-once
file output, and exit classification separately visible. The architecture
inventory names that full descent, including the mirrored command tests.

The adjacent `reference_policy/mod.rs` entrance exposes only deterministic
training, evaluation, and strict decode calls. Its `training/mod.rs` and
`evaluation/mod.rs` entrances each join separate compute and independent replay
leaves; `identity`, `inference`, `model`, and `codec/` remain named sibling
rungs. Tests mirror codec, training, evaluation, and refusal behavior, and the
tooling architecture ladder registers all three library entrances. The command
routes call these public validated operations; no second trainer, evaluator,
process, or compiler route is hidden below them.

Removing a catalog row disables that exact rule. Adding a row must make
omissions, duplicates, unsupported targets, and ambiguous matches fail closed.
A custody crate may consume the catalog owner's typed result; it may not create
a proxy schedule or repeat the rule-name match. Post-allocation machine
composition therefore carries `PostAllocationMachineRuleCatalogEntry`; the
execution rung switches on its closed rule kind and contains no exact
`Optimization` names.

The machine peephole rung is itself navigable: `peephole_matching/mod.rs`
coordinates one immutable instruction-pair input and closed topology through
`instruction.rs`,
`registers.rs`, `relations.rs`, and `liveness.rs`, with vocabulary in
`model.rs`. Exact pattern data stays with each rule, currently the cataloged
`aarch64/compare_zero_branch_nonzero/pattern.rs` and
`aarch64/elide_same_view_copy_before_return/pattern.rs`, plus the adjacent-body
`aarch64/elide_same_view_copy_before_compare_zero/pattern.rs`. Each copy rule's
tiny entrance visibly joins proposal to independent replay; its validation
rung reconstructs footprints and roots without importing the matcher. The
shared rung owns neither enablement nor rewriting, and validators do not import
it. The only topologies are body-tail/terminator and adjacent ordinary-body
instructions; there is no generic pattern AST. This keeps the path from exact
rule entrance to pattern, named
matcher mechanics, and independent replay visible without creating a proxy
rule schedule. Only a catalog row may grant compiler enablement.

Descriptive machine costs are a sibling rung, not a rule catalog.
`omega-machine-optimizer/src/costs/mod.rs` selects the current model and binds
it to the exact `NativeTarget`; `model.rs` exposes exact-or-bounded size and
explicitly unavailable latency, while `identity.rs` owns the domain-separated
target/model identity. The model neither selects nor validates rules. An
architecture dependency guard rejects imports from this rung in production
machine-rule validators, so estimates cannot become semantic admission.

Psi has one additional local rung. `rules/catalog.rs` orders the selected
passes, while `passes/<exact-pass>/mod.rs` visibly orders that pass's local
rules. A family folder below a pass is a group, not another enablement table.

Proof-check elision demonstrates the intended split. Its pass entrance is only
the ordered roster and module map. Position zero's proof-certified dead-scalar
rule lives at `proof_check_elision/dead_scalar/mod.rs`; no pass imports an
enabled rule from a sibling pass. Each exact identity leaf owns the operation
classifier bearing that identity's name. The adjacent `identity_rewrite/`
group contains only the common candidate model, proposal construction, and
typed zero/one vocabulary. Generic provenance accounting for deleting a node
lives in `passes/support/node_elision_accounting.rs`, where GVN and proof-check
elision consume it as peers; neither pass reaches through the other's module.
The narrower `passes/support/dead_scalar_node/` protocol owns only liveness,
effect, accounting, and proof-witness coordination shared by the literal,
unconditionally-total, and proof-certified exact leaves. It receives each
leaf's closed classifier and contract; it owns neither rule identity nor pass
selection.

Control-flow cleanup follows the same rule when two transformations share a
concept but not an accounting contract. `empty_block_threading/` exposes
separate `linear.rs` and `path_qualified.rs` rule leaves over shared binding
composition and ownership-identity checks; their distinct provenance/effect
accounting remains in separately named leaves.

Independent control-flow validation mirrors that taxonomy instead of restoring
a mixed replay file. Its small pass-family entrance maps constant folding,
empty-block threading, block merging, shared-jump fusion, and unreachable
machine pruning. The threading and merge groups descend immediately to one
named validator folder per exact rule. Each small `mod.rs` owns exact admission
and points directly to `replay.rs`, which reconstructs only that rule's graph
transformation. A shared `contract.rs` admits only the rule identity, complete
required-analysis set, complete invalidation set, and safety class declared by
that entrance; replay separately binds the rule's exact cost and transformation
evidence. Cross-rule, unknown-rule, superset, wrong-safety, and wrong-cost
relabellings are rejected for all seven rules.

Copy propagation uses the same direct route. Its pass entrance contains the
one-row roster, `redundant_block_parameter/mod.rs` owns the exact contract and
proposal join, and `proposal.rs` owns traversal mechanics. Independent
validation admits that one exact contract before descending into observation
and operation rewriting. The former flat producer path is retired.

The control-flow pass entrance consequently owns no descendant dependency
bucket: it is the module map plus the exact seven-row local roster. Every rule,
accounting leaf, and shared custody leaf imports the vocabulary it consumes.

The adjacent executable lowering boundaries retain the same visible descent.
The 100-line Terminal-operation router maps structural scalar operations to its
named `structural_scalar_fields` leaf. Straight-line abstract-to-target lowering
then routes direct Boolean and integer field projection through
`straight_line/structural_scalar_field.rs` rather than growing the exhaustive
operation leaf. The mirrored
`tests/structural_and_cleanup/structural_scalar.rs` leaf owns structural scalar
projection and call fixtures independently from its bounded-cleanup sibling.

Abstract-to-target validation follows the same entrance rule. Its 51-line
`validation/mod.rs` exposes the module map and public validation calls;
`whole_plan.rs` binds whole-plan roots, exact external settlement rosters,
function order, and structural declarations before the catalog selects one
named family leaf. Settlement-aware and plain validators are closed descriptor
variants, not an ambient mode or a second family schedule.
Constant bitwise-not immediate translation has its own small
`straight_line_integer_bitwise_not_immediate/mod.rs` executable entrance. It
descends directly to `grammar.rs` for the exact three-operation source shape and
`replay.rs` for independent target/provenance reconstruction. Its tests mirror
that descent through separate fixture, positive, source-corruption, and target-
corruption leaves; the catalog canary and public optimized-custody leaf remain
visible in the registered semantic ladder.
Constant Boolean-not immediate translation is its exact sibling under
`straight_line_boolean_not_immediate/`. Its 27-line `mod.rs` entrance joins
only `grammar.rs` and `replay.rs`; mirrored fixture, positive, source-
corruption, and target-corruption leaves sit below their own test group. The
catalog and architecture ladders register it independently from plain Boolean
immediate and parameter Boolean-not families.
Constant Boolean-equality immediate translation follows the same taxonomy under
`straight_line_boolean_equal_immediate/`. Its 27-line entrance joins an exact
four-operation source grammar to independent target replay; fixture, positive,
source-corruption, and target-corruption leaves mirror that production split.
Its own catalog adapter, catalog canary, and optimized-custody leaf keep the
family visibly separate from plain Boolean immediate, constant Boolean-not,
and parameter Boolean equality.
Constant integer-equality immediate translation is independently rooted under
`straight_line_integer_equal_immediate/`. Its 27-line entrance joins only the
same-type four-operation grammar and independent Boolean-immediate replay.
Mirrored fixture, positive, source-corruption, and target-corruption leaves,
plus a dedicated catalog adapter/canary and optimized-custody leaf, keep it
separate from parameter integer equality and every plain immediate family.
Constant integer-less-than immediate translation mirrors that taxonomy under
`straight_line_integer_less_than_immediate/`. The 27-line entrance exposes an
exact ordered source grammar and independent Boolean-immediate replay; its own
fixture/corruption group, catalog adapter/canary, and optimized-custody leaf
keep constant ordering separate from equality and parameter comparison.
Constant integer-less-than-or-equal immediate translation is the independent
inclusive sibling under `straight_line_integer_less_or_equal_immediate/`. Its
27-line entrance joins the exact same-type four-operation grammar to replay of
one Boolean immediate, with fixed signed/unsigned and address-width comparison
semantics. Its dedicated fixture/corruption group, catalog adapter/canary, and
optimized-custody leaf keep inclusive ordering explicitly selectable rather
than hiding it behind the strict family.
Constant integer bitwise-AND immediate translation is separately rooted under
`straight_line_integer_bitwise_and_immediate/`. Its 27-line entrance joins the
exact two-constant four-operation grammar to independent integer-immediate
replay. Dedicated fixture, positive, source-corruption, target-corruption,
catalog, and optimized-custody leaves cover signed/unsigned fixed 8/16/32/64
and address64 across all five native targets. The catalog row remains visibly
disjoint from plain immediate, bitwise-not, bitwise-OR/XOR, and parameter AND.
Constant integer bitwise-OR immediate translation owns the parallel
`straight_line_integer_bitwise_or_immediate/` taxonomy. Its 27-line entrance
joins the exact ordered four-operation grammar to independent
`ReturnIntegerImmediate` replay using `IntegerType::bitwise_or`. Dedicated
fixture, corruption, catalog, and optimized-custody leaves cover 180 direct and
180 optimized fixed/address cases across all five targets while remaining
disjoint from plain immediate, bitwise-not, AND/XOR, and parameter OR.
Constant integer bitwise-XOR immediate translation owns the matching
`straight_line_integer_bitwise_xor_immediate/` taxonomy. Its 27-line entrance
joins the exact ordered four-operation grammar to independent
`ReturnIntegerImmediate` replay using `IntegerType::bitwise_xor`. Dedicated
fixture, corruption, catalog, and optimized-custody leaves cover 180 direct and
180 optimized fixed/address cases across all five targets while remaining
disjoint from plain immediate, bitwise-not, AND/OR, and parameter XOR.
Constant wrapping integer-add immediate translation is its exact arithmetic
sibling under `straight_line_wrapping_integer_add_immediate/`. Its 27-line
entrance joins the ordered two-constant grammar to independent
`ReturnIntegerImmediate` replay using `IntegerType::wrapping_add`. Dedicated
fixture, corruption, catalog, and optimized-custody leaves cover signed/
unsigned fixed 8/16/32/64 and address64 on all five targets while remaining
disjoint from exact/saturating add, subtract/multiply, plain immediate, and the
parameter wrapping-add family.
Constant wrapping integer-subtract immediate translation is the adjacent exact
ordered sibling under `straight_line_wrapping_integer_subtract_immediate/`.
Its 27-line entrance joins the two-constant four-operation grammar to
independent `ReturnIntegerImmediate` replay using
`IntegerType::wrapping_sub`. Dedicated fixture, corruption, catalog, and
optimized-custody leaves cover signed/unsigned fixed 8/16/32/64 and address64
on all five targets with 180 direct and 180 optimized cases. The family remains
disjoint from exact/saturating subtract, wrapping add/multiply, plain immediate,
and parameter wrapping-subtract.
Constant wrapping integer-multiply immediate translation is the next exact
arithmetic sibling under `straight_line_wrapping_integer_multiply_immediate/`.
Its 27-line entrance joins the two-constant four-operation grammar to
independent `ReturnIntegerImmediate` replay using `IntegerType::wrapping_mul`.
Dedicated fixture, corruption, catalog, and optimized-custody leaves cover
signed/unsigned fixed 8/16/32/64 and address64 on all five targets with 180
direct and 180 optimized cases. The family remains disjoint from
exact/saturating multiply, wrapping add/subtract, plain immediate, and
parameter wrapping-multiply.

Projected structural call/return custody follows a plan taxonomy because no
single function can validate the closure. The 54-line
`lowering/coordination/projected_qualifications/mod.rs` entrance owns only the
global rejection fence and points to `structural_call_return.rs` for its one
complete producer grammar. Independent validation enters through the 68-line
`validation/structural_call_return/mod.rs`, then descends through named
source, layout, target, and local caller/callee replay leaves. A separate plan
catalog and structural function adapters make enablement explicit. Mirrored
fixture, positive, source-corruption, target-corruption, and fence leaves sit
under one registered semantic ladder, with the public optimizer-custody canary
at its final rung.

Legalization continues the same exact closure through
`legalization/projected_structural_call_return/`. Its seven-line stage-group
entrance names only `source/` and `replay/`; their 51- and 34-line entrances
descend through explicit candidate, grammar/contract, and custody leaves.
Ordinary legalization rosters remain in adjacent `ordinary_roster.rs` leaves,
keeping the main source and replay entrances at 52 and 43 lines. Mirrored
positive, corruption, and fence tests cover all five targets and keep the
selection boundary visible. Layering guards forbid replay from importing the
producer or target lowering, and the source-organization registry names both
semantic ladders.

Selection continues through
`selection/construction/projected_structural_call_return/`. Its 70-line
executable entrance coordinates only named `projection`, `constraints`, and
`transfer` leaves; the selected representation has its own adjacent
`projected_structural_call_return/` stage group. Independent validation enters
through a 28-line coordinator and descends through separate `source` and
`target` reconstruction leaves. Identity-specific encoding lives under
`selection/identity/projected_structural.rs`, with shared register primitives
extracted from the main identity file. The registered ladder terminates at
public all-five custody and explicit liveness/pre-allocation refusal tests:
allocation and later physical stages do not inherit authority merely because
the atomic selected carrier exists.

The checked-Psi exact-add proof producer uses the same navigability rule
without pretending to be an optimizer catalog. Its 94-line `direct_add/mod.rs`
entrance visibly owns strategy precedence and names `correlated`, `targeted`,
`flat`, `relation`, and `conjunction` leaves. The 76-line conjunction entrance
then names only `compute`, `definitions`, and `model`; its tests descend into
fixture, positive, refusal, budget, and corruption leaves. An architecture
guard fixes both entrances at 100 lines and requires every named rung, while a
source integration canary lives outside the producer taxonomy.

GVN's `expression_keys/` group owns a closed key model and three explicit
classifiers: total, proof-certified, and directional compatible-policy. Those
vocabulary leaves import their own operation and scalar types rather than
inheriting the broader traversal namespace from the GVN entrance.
`effect_admission.rs` owns the shared exact-pure query, while provenance
accounting that only applies to join-parameter translation lives beside that
family in `phi_translated/accounting.rs`. The pass entrance is therefore only
the module map, test-only classifier visibility, and exact sixteen-row roster;
all local, dominating, phi-translated, and identity leaves name their own
dependencies.

The first nine GVN rows are the three-by-three scalar common-subexpression
matrix: same-block, dominating, and phi-translated traversal scopes crossed
with obligation-free, proof-certified, and compatible-policy evidence. Their
producer tests enter through
`tests/global_value_numbering/scalar_common_subexpression/mod.rs`, then descend
to scope behavior or the exact contract-custody matrix. Independent admission
requires the complete scope-specific analysis set, complete invalidation set,
exact safety class, `-1` cost, and exact named rule before semantic replay. A
nine-fixture operational matrix exercises each row through the selected pass
and the empty default registry, binds manifest/validator/fact and ledger/commit
custody, and proves determinism, budget failure, and output idempotence.

GVN total scalar identity tests mirror the seven exact roster rows beneath
`tests/global_value_numbering/total_scalar_identity/`. The group entrance is a
map to one leaf each for wrapping neutral arithmetic, shift-by-zero,
multiply-by-zero, saturating neutral arithmetic, saturating multiply-by-zero,
bitwise neutral literals, and bitwise absorbing literals; `catalog.rs` alone
pins their opt-in registry placement. Independent validation admits the exact
required-analysis set, invalidation set, safety class, cost, and matching rule
identity, then publishes the matching validator identity. Its 26-law positive
matrix and seven-family corruption matrix prevent a semantically valid patch
from being relabelled across same-shaped contracts.

Dead-scalar tests enter through
`tests/dead_scalar_elimination/mod.rs` and descend immediately to literal,
unconditionally-total, and exact contract-custody leaves. The custody matrix
spans the two-rule dead-pure-scalar roster and the proof-check roster's
proof-certified dead-node row because the independent dead-node validator is
shared by those three exact semantic families. It binds each rule to complete
required and invalidated analysis sets, safety, `-1` cost, and a distinct
validator identity, then rejects every directed cross-rule relabelling and
each contract-axis corruption. A pass-manager matrix separately proves that
both opt-in suites are disabled by default, deterministic under repeated
execution and budget exhaustion, custody-preserving in their manifests and
ledgers, and idempotent at the resulting fixed point.

The complete proof-check roster has one companion contract matrix and one
whole-engine operational matrix. The contract matrix pins all twelve ordered
rows, their complete analysis contracts, proof-certified safety, `-1` cost,
and exact validator identities. It rejects all 132 directed cross-rule
relabellings plus unknown identities, analysis and invalidation subsets and
supersets, wrong cost, and wrong safety at either candidate construction or
independent validation. The operational matrix uses one fixture per roster row
to prove default-disabled execution, exact evaluation order, deterministic
repetition and budget failure, manifest/fact and ledger/commit custody,
accepted-proof retention, source-obligation pruning, and fixed-point
idempotence.

Independent proof-check validation has the same single-entry route.
`candidates/proof_check_elision/mod.rs` maps all twelve exact rule identities
through its adjacent `rule_catalog.rs` to dead-node, operand-substitution,
same-operand constant, or unit-divisor replay. The generic candidate dispatcher
enters that route before patch-family dispatch; the SCCP entrance recognizes
only SCCP rules and no longer contains a hidden proof-check identity table.

SCCP range comparisons follow the same descent. The pass entrance retains the
sole 39-row local order, while `range_comparisons/` first separates
range-against-constant from range-against-range evidence. Each of the nine
canonical rule identities then owns an exact executable `mod.rs`; only contract
construction, proposal traversal, and interval evaluation are shared within
the matching evidence family. The architecture guard pins every exact proposal
join and rejects restoration of the former mixed producer or flat test path.

Boolean-result SCCP constant evaluation uses the same rule-first route.
`constant_evaluation/boolean/` is a group map over five exact executable rule
entrances; each entrance owns its canonical contract and chooses one member of
the closed Boolean-evaluation kind model. Typed operation/fact evaluation and
candidate assembly are distinct shared leaves. Tests mirror Boolean versus
integer result families and separately pin the Boolean rules' positions in the
sole 39-row pass roster. The guard rejects restoration of the mixed producer,
the flat test leaf, or inherited parent-glob dependencies anywhere below
constant evaluation. Independent Boolean-result validation now descends from
a small evidence router through exact literal-Boolean and integer-comparison
leaves. The comparison entrance separates constant, range/constant, and
range/range reconstruction, while literal constant evaluation first replays
the exact operation-to-rule identity. A same-safety contract therefore cannot
be relabelled across the five literal rules; the architecture guard pins the
entire validation descent and retires the former mixed comparison leaf.
Independent interval classification and evaluation descend one rung further
under the matching `range_against_constant/` or `range_against_range/`
evidence family. The former flat validation-side `range_comparisons.rs` path is
retired, and candidate admission requires the exact analysis and invalidation
sets carried by that evidence shape rather than accepting contract supersets.

Integer-result SCCP constant evaluation now gives exact cast, widen, and
bitwise-not their own executable entrances. The proof-certified cast keeps its
obligation join beside its proposal traversal. Widen and bitwise-not descend
through a small `unary/` group map to a closed operation model and shared
single-operand proposal builder. The guard pins all three proposal joins and
the supporting ladders, retires the former mixed `cast.rs` and `unary.rs`
leaves, and continues to forbid inherited parent-glob dependencies. All 22
binary integer identities likewise own exact executable entrances. Their
group retains only the closed operation-shape model, typed evaluator,
constant-fact proposal traversal, and proof/exact witness builder. The sole
39-row pass roster remains authoritative; a separate canary pins every binary
identity and safety class to positions 0–18 and 22–24. The guard pins each
proposal join and retires the former aggregate arithmetic, quotient, shift,
and bitwise definition leaves.

The SCCP tests mirror that descent. Integer-result cases enter a small group
map over binary, unary, and propagation behavior. Binary fixtures live below
the SCCP fixture family and construct fresh typed units instead of mutating an
unrelated rule fixture. The 22-row positive matrix distinguishes wrapping and
saturating overflow behavior, signed quotient policies, distinct shift-count
types, and bitwise results; a separate proof-certified refusal matrix covers
overflow and undefined domains. Unary fixtures use the same fresh-unit
discipline to pin signed and unsigned endpoints and cross-kind refusal. On the
independent-validation side, the integer-evaluation entrance descends first
through an exhaustive operation-to-rule identity replay, then through unary or
binary operation reconstruction. Consequently a semantically valid candidate
cannot be relabelled as another same-safety integer rule, and the architecture
guard makes that validation ladder directly navigable. Boolean-result fixtures
likewise live in their own SCCP fixture leaf. Their matrix covers both truth
boundaries, signed and unsigned comparison endpoints, every wrong rule/shape
pair, all cross-contract relabellings, unknown identities, wrong result values,
and representative unary/binary witness and fact corruption. Positions 25–29
pin the complete contracts rather than identity alone.

## Semantic folder templates

Use the smallest applicable template; do not create empty leaves in advance.

```text
analyses/<fact>/
  mod.rs          # compute -> independent validation entrance
  model.rs
  compute/
  validate/
  identity.rs
  tests/

rules/<target-or-family>/<exact-rule>/
  mod.rs          # proposal -> independent validation entrance
  model.rs
  compute/
  validate/
  identity.rs
  codec/          # only when persisted
  tests/

stages/<custody-boundary>/
  mod.rs          # build/replay or project/admit entrance
  model.rs
  construction/
  validation/
  tests/
```

Shared mechanics belong at the nearest ancestor where at least two exact
leaves consume the same semantic contract. Producer and validator mechanics
remain separate even when they share neutral canonical vocabulary.

Tests mirror production taxonomy. Large matrices descend first by artifact or
rule family, then by behavior such as `positive`, `source_corruption`,
`target_corruption`, and `compatibility`.

Parent module glob imports are also a navigation smell: a tiny entrance must
not become a hidden namespace bucket for all descendants. Newly migrated exact
rule leaves import their own dependencies explicitly; existing wildcard debt
is removed family by family before those families grow.

## Size and cohesion ratchets

Line count is a smoke alarm, not a substitute for review. The enforceable
healthy limits are:

- executable entrances: at most 100 lines by default;
- an entrance exception: exact path, semantic reason, and non-growing ceiling,
  never above 200 lines;
- production files: at most 600 lines;
- focused tests and fixtures: at most 800 lines; and
- no file may mix catalog ownership, proposal mechanics, independent
  validation, persistence, and broad fixtures.

The architecture test now enforces 600/800 directly; the former 1,000/1,500
migration ceilings and all entrance exceptions have been removed.

## Architecture guard shape

The guard mirrors what it checks:

```text
tests/architecture/optimizer_source_organization/
  mod.rs                 # run each audit and aggregate violations
  inventory.rs           # governed roots and six rule-stage descriptors
  bounds.rs              # production, test, and entrance ceilings
  module_roles.rs        # exhaustive source-local role classification
  entrances/
    mod.rs               # coordinate entrance and ladder checks
    requirements/
      mod.rs             # typed domain inventory entrance
      executable/        # Psi, translation, selection, physical, native
      ladders/           # named semantic descents
  catalogs.rs            # sole order and exact-rule checks
  retired_paths.rs       # prohibited legacy shapes
```

The guard's inventory is architecture too. Domain files must be short enough
to read as maps, and its entrance must iterate typed domain groups rather than
accumulate one repository-wide path array.

## Enforced state

The live tree and architecture guard establish:

- the governed compiler-hook, optimizer-execution, and rule-owning entrances
  above are small and meaningful;
- post-allocation construction and replay meet only at
  `omega-machine-optimizer/src/planning/post_allocation/mod.rs`, with separate
  semantic subtrees;
- no governed production file exceeds 600 lines;
- no governed test or fixture exceeds 800 lines;
- no executable entrance exceeds 100 lines; and
- the guard itself has a 19-line coordinator over typed domain inventories,
  with no monolithic entrance or requirement array.

These are organization defects, not language-design questions.

## Review checklist

Start from the phase entrance and answer:

- Is there one catalog row to enable or disable the exact optimization?
- Does that row descend to one exact named leaf?
- Does the leaf entrance join proposal to independent validation?
- Are shared mechanics below the nearest honest family ancestor?
- Do tests follow the same taxonomy?
- Can the architecture guard discover the route through a named domain group?

If any answer is no, refactor the route before extending it.
