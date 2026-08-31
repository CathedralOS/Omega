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

`omega-image-emission/ranked_u32_countdown` is deliberately outside this
guard. It independently replays a language-level ranked execution carrier but
owns no optimization selection, catalog, proposal, or optimized stage result.
Image publication needs one coherent publication architecture boundary; one
special-case lane is not an optimizer root.

## Rule-owning stage entrances

One stage entrance consumes selections. One adjacent catalog owns exact
enablement and order.

| Phase | Entrance | Sole catalog | Next rung |
|---|---|---|---|
| Mandatory legalization | `omega-target-operations-to-selected-instructions/src/legalization/mod.rs` | `legalization/catalog.rs` | `source/`, `replay/` |
| Psi | `omega-psi-optimizer/src/rules/mod.rs` | `rules/catalog.rs` | `passes/<exact-pass>/` |
| Selected lowering | `omega-regalloc/src/rules/selected_lowering/mod.rs` | adjacent `catalog.rs` | `literal_fold/` |
| Allocation recovery | `omega-regalloc/src/rules/allocation_recovery/mod.rs` | adjacent `catalog.rs` | `fixed_view_copy/`, `pressure_rematerialization/` |
| Post-allocation machine | `omega-machine-optimizer/src/rules/mod.rs` | `rules/catalog.rs` | `rules/<isa>/<exact-rule>/` |
| Function-relative layout | `omega-optimization-pipeline/src/stages/layout/x86_branch_relaxation/mod.rs` | adjacent `catalog.rs` | compute and independent validation |

Removing a catalog row disables that exact rule. Adding a row must make
omissions, duplicates, unsupported targets, and ambiguous matches fail closed.
A custody crate may consume the catalog owner's typed result; it may not create
a proxy schedule or repeat the rule-name match.

Psi has one additional local rung. `rules/catalog.rs` orders the selected
passes, while `passes/<exact-pass>/mod.rs` visibly orders that pass's local
rules. A family folder below a pass is a group, not another enablement table.

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
