# Optimizer Source Organization

This is the source-navigation contract for the optimizer. The design entrance
is [optimizer_architecture.md](../optimizer_architecture.md); the executable
work list is [TASKS_OPTIMIZER.md](../../../TASKS_OPTIMIZER.md).

## The rule

A human opening an executable optimization stage must land on one small file
that answers five questions without repository-wide search:

1. What validated value enters?
2. Which exact optimization names are eligible?
3. Where is their canonical order declared?
4. What function coordinates proposal and independent validation?
5. What validated value leaves?

The entrance is normally `mod.rs`. It is not a re-export wall and it does not
contain rule mechanics. It owns the small coordination join and points directly
to the next semantic rung.

A directory that only groups neighboring stages may have a map-only `mod.rs`,
but its module documentation must call it a group. It is not counted or
described as a stage entrance.

Every governed `lib.rs` and `mod.rs` begins with exactly one source-local role
declaration: `crate map`, `stage group`, or `executable entrance`. This is not
decorative prose. The architecture audit inventories the complete governed
tree, rejects an unclassified or multiply classified module, and requires each
executable entrance to retain its named coordination seam. A new module cannot
silently become an entrance by accumulating mechanics behind a map.

## The Squalr pattern

The useful reference is Squalr's scan-rule path:

```text
registry -> ordered enabled rules -> mapped scan plan -> scanner dispatch
```

The registry is short, the application loop is obvious, and each rule is easy
to add or remove. Omega keeps that shape while replacing string keys,
unordered maps, global singleton state, and unchecked in-place mutation with
typed exact names, deterministic order, immutable plans, independent replay,
and identity-bound receipts.

The Omega path is therefore:

```text
stage/mod.rs
  -> catalog.rs
  -> <family>/mod.rs
  -> <exact-rule>/mod.rs
  -> {model, compute, validate, identity, codec, tests}.rs
```

Every rung narrows the question. A reader should never have to open a mixed
`rules.rs`, `helpers.rs`, or thousand-line match to discover the next route.

## Entrance and catalog ownership

One stage entrance owns the executable join. One adjacent catalog owns exact
enablement and order.

```rust,ignore
pub fn optimize(input: &ValidatedInput, selections: &OptimizationSelections)
    -> Result<ValidatedOutput, StageError>
{
    let selected = catalog::select(selections)?;
    let proposal = selected.propose(input)?;
    selected.validate(input, proposal)
}
```

The catalog contains descriptors, not a second implementation. Removing a row
disables the rule. Adding a row must make omissions, duplicates, unsupported
targets, and ambiguous matches fail closed.

A cross-stage custody crate consumes the rule owner's typed result. It may not
create a proxy schedule or repeat the rule-name match.

## Folder taxonomy

Use the smallest applicable template.

```text
analyses/<fact>/
  mod.rs          # compute/validate entrance
  model.rs
  compute.rs
  validate.rs
  identity.rs
  tests.rs

rules/<target-or-family>/<exact-rule>/
  mod.rs          # propose/validate entrance
  model.rs
  compute.rs
  validate.rs
  identity.rs
  codec.rs        # only when persisted
  tests.rs

stages/<custody-boundary>/
  mod.rs          # build/replay or project/admit entrance
  model.rs
  construction/
  validation/
  tests/
```

Do not create every leaf pre-emptively. A shared module belongs at the nearest
ancestor where at least two exact leaves consume one semantic contract.

Tests mirror production taxonomy. Large matrices descend by artifact family,
then by behavior (`positive`, `source_corruption`, `target_corruption`,
`compatibility`) rather than accumulating in a stage-wide test file.

## Current rule-stage entrances

| Phase | Entrance | Sole catalog | Next rung |
|---|---|---|---|
| Mandatory legalization | `omega-target-operations-to-selected-instructions/src/legalization/mod.rs` | `legalization/catalog.rs` | `source/`, `replay/` |
| Psi | `omega-psi-optimizer/src/rules/mod.rs` | `rules/catalog.rs` | `passes/<exact-pass>/` |
| Selected lowering | `omega-regalloc/src/rules/selected_lowering/mod.rs` | adjacent `catalog.rs` | `literal_fold/` |
| Allocation recovery | `omega-regalloc/src/rules/allocation_recovery/mod.rs` | adjacent `catalog.rs` | `fixed_view_copy/`, `pressure_rematerialization/` |
| Post-allocation machine | `omega-machine-optimizer/src/rules/mod.rs` | `rules/catalog.rs` | `rules/<isa>/<exact-rule>/` |
| Function-relative layout | `omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/mod.rs` | adjacent `catalog.rs` | compute and independent validation |

This table is an inventory, not prose documentation. Adding a rule-owning
stage requires adding one row here and one architecture-test descriptor.

## Size and cohesion guardrails

Line count is only a smoke alarm. A 70-line re-export wall is invalid and a
focused 300-line decoder may be coherent. The enforceable defaults are:

- executable entrances: prefer at most 100 lines;
- any entrance above 100 lines: exact, non-growing exception and semantic
  reason;
- production leaves: refactor target at 600 lines, hard ceiling at 1,000
  during migration;
- focused tests or fixtures: refactor target at 800 lines, hard ceiling at
  1,500 during migration; and
- no file may mix catalog ownership, producer mechanics, independent
  validation, persistence, and broad fixtures.

The 1,000/1,500 limits are migration ceilings, not a definition of good
organization. Crossing a refactor target creates explicit debt before the file
grows again.

## Current audit

The principal rule-stage entrances are small and catalog-backed, but the tree
does not yet fully satisfy this contract.

- The former 1,254-line source-organization architecture test now has one tiny
  coordination entrance over inventory, bounds, executable entrances,
  catalogs, and retired paths. Its six rule stages share one typed descriptor
  for entrance, catalog, markers, and next rungs.
- Psi pass entrances now own their visible local rule order rather than
  re-exporting a registration function from a hidden sibling catalog.
- The optimization manifest now has a 37-line entrance that owns its stable
  decision-v5/pass-v1 format registry and descends into decision, pass, work,
  fact, framing, error, and matching test leaves. Legalized-call validation
  likewise owns its join directly instead of hiding it behind one child.
- Abstract-operation identity encoding now has a 71-line exhaustive family
  router over structural establishment, calls/effects, scalar operations,
  control exits, and scalar-operation shapes, with identity-wide carriers at
  the nearest shared ancestor. SCCP candidate validation now has a 98-line
  rule-first, exhaustive-patch join over focused validation leaves.
- Independent live-range replay now has a 78-line reconstruct/canonicalize/
  compare/receipt join; independent GVN keys descend by total,
  proof-certified, and compatible-policy vocabulary. The former 1,457-line
  structural-catalog test matrix now mirrors six production validation families.
- All 297 governed module maps are source-locally classified: 162 executable
  entrances, 14 crate maps, and 121 stage groups. The guard exhaustively checks
  those roles and the real coordination marker of every executable entrance.
- The transformation ledger now descends from a 92-line custody-validation
  entrance into model, error, validation, encoding, decoding, cursor, and test
  leaves. Register-allocation and selected-lowering test matrices now mirror
  their retained artifact and exact-rule families; their largest leaves are
  388 and 599 lines respectively.
- Conditional-control lowering now descends by Boolean result, integer result,
  and shared edge binding. Provider settlement has one 61-line executable join
  over exact-plan, normalized-call, and per-boundary leaves. Pre-allocation
  machine-effect persistence descends through an explicit V6 vocabulary into
  framing, instruction, structural, ownership, and value leaves.
- MOVN proposal computation now has a 95-line root-admission, bounded-selection,
  and plan-finalization join over source, recipe, materialization, budget, and
  focused test leaves. Fixed-view-copy persistence now has an 82-line V4/V5
  selected-plan entrance over a historical scalar leaf and a 71-line structural
  entrance; structural ABI, call, declaration, settlement, and signature fields
  descend into named leaves below it.
- Spill-choice computation and normalized foreign-scalar boundary-call lowering
  now retain cohesive 495- and 514-line production leaves while their focused
  fixtures live in adjacent 280- and 259-line test leaves.
- Exact wrapping add, subtract, and multiply translation descend through one
  75-line arithmetic catalog and sub-70-line source/replay coordinators into
  separate grammar, target, error, receipt, corruption, and custody leaves.
  The adjacent family-error entrance keeps its exact typed sum below 100 lines
  by separating whole-translation validation failures from family failures.
- Selected-block validation now has a 39-line roster/entry/return-routes join
  over exact block-family replay leaves and one shared instruction comparator;
  its largest leaf is 195 lines and it never calls construction helpers.
- Scalar legalization source projection now has a 99-line common-admission,
  exact-family-dispatch, and return join over named family, operation-roster,
  return, and fuel leaves. Its largest leaf is 270 lines and it preserves
  catalog order, diagnostics, proof custody, and provenance order.
- The immutable rewrite vocabulary now has a 19-line stage-group map over
  source/provenance foundations, scalar evaluation, SCCP, CFG plans, scalar
  plans, and the candidate contract. Its largest leaf is 169 lines; candidate
  construction remains at its separate executable entrance, and neutral
  canonical writers prevent the model from depending on its candidate codec.
- No production-classified leaf remains at 750+ lines; 23 governed production
  leaves remain at 600-749 lines.
- Eleven broad test and fixture leaves remain above 1,000 lines even though
  their production stages already have named taxonomies.
- The old task ledger and this brief accumulated milestone history instead of
  remaining entrances. Git history is the milestone archive.

These are organization defects, not language-design questions. They are
tracked at the top of `TASKS_OPTIMIZER.md` and block declaring the navigation
migration complete.

## Architecture test shape

The guard must mirror the architecture it checks:

```text
tests/architecture/optimizer_source_organization/
  mod.rs             # run the audit and report violations
  inventory.rs       # governed roots and stage descriptors
  bounds.rs          # line and entrance ceilings
  module_roles.rs    # exhaustive source-local module classification
  entrances/         # meaningful joins and required semantic ladders
  catalogs.rs        # sole-order and exact-leaf checks
  retired_paths.rs   # prohibited legacy shapes
```

Stage descriptors carry the entrance, catalog, coordination marker, catalog
marker, and next rungs together. The current six-row inventory uses generic
entrance/catalog checks; bespoke checks are reserved for invariants such as the
sole legalization catalog and fixed-view-copy protocol ownership.

## Review test

Before adding an optimization, start from the phase entrance and answer:

- Is there one catalog row to enable or disable it?
- Does that row descend to one exact named leaf?
- Does the leaf entrance join proposal to independent validation?
- Are shared mechanics below the nearest honest family ancestor?
- Do tests follow the same taxonomy?
- Can the architecture guard discover the route without another bespoke
  thousand-line list?

If any answer is no, refactor the route before extending it.
