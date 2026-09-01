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

The ordinary empty-selection compiler path does not enter the explicit Psi
optimizer. Physical routing consumes the exact selected phase set and one
typed optimized target value; it does not invent optimization profiles.

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
`psi.rs`, and `selected_machine.rs`. The checked-in V1 corpus identity lives
under `tests/native-differential/corpora/optimizer/v1/`; it is data custody, not
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

The external-policy execution prerequisite has its own dormant compiler
entrance at `omega-compiler/src/compiler/optimization/external_policy/mod.rs`.
That file alone coordinates the opaque sandbox capability, exact transport
limits, canonical response matching, and explicit fallback settlement; named
leaves own each concern. The module is absent from ordinary builds and has no
production capability constructor until a real platform sandbox exists.

Removing a catalog row disables that exact rule. Adding a row must make
omissions, duplicates, unsupported targets, and ambiguous matches fail closed.
A custody crate may consume the catalog owner's typed result; it may not create
a proxy schedule or repeat the rule-name match. Post-allocation machine
composition therefore carries `PostAllocationMachineRuleCatalogEntry`; the
execution rung switches on its closed rule kind and contains no exact
`Optimization` names.

The machine peephole rung is itself navigable: `peephole_matching/mod.rs`
coordinates one immutable terminal-pair input through `instruction.rs`,
`registers.rs`, `relations.rs`, and `liveness.rs`, with vocabulary in
`model.rs`. Exact pattern data stays with each rule, currently the cataloged
`aarch64/compare_zero_branch_nonzero/pattern.rs` and the core-only
`aarch64/elide_same_view_copy_before_return/pattern.rs`. The latter rule's tiny
entrance visibly joins proposal to independent replay, while its `validate/`
group reconstructs footprints and roots without importing the matcher. The
shared rung owns neither enablement nor rewriting, and validators do not
import it. This keeps the path from exact rule entrance to pattern, named
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
