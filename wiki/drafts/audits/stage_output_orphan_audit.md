# Stage-output orphan audit

Two companion sweeps cover the placement rule's halves: whether each rule
stage's produced route still has a named consumer, and whether every designed
accessor channel on a stage product has an external reader. Delete this audit
once the zero-reader accessors are pruned or kept by a representation-contract
ruling.

## Produced-route consumers

Companion sweep to `stage_entrance_orphan_audit.md`. The entrance audit answers
"does a designed stage entrance have a caller"; this audit answers the
placement rule's other half — once a stage produces a route, does a
coordinator or successor actually consume it. A stage whose produced route no
listed consumer names would still pass every entrance check, so the guard
lands beside them in
`tests/architecture/optimizer_source_organization/entrances/rule_stages.rs`.

## Sweep

Enumerated the five rule-owning stage descriptors in
`tests/architecture/optimizer_source_organization/inventory.rs`
(`RULE_STAGES`) and, for each, walked the produced-route consumer list —
files that must still name the stage's `output_marker`:

| Stage entrance | Produced route marker | Consumers checked |
| --- | --- | --- |
| `06_lowered-psi-to-lowered-psi/src/psi_optimization.rs` | `run_psi_optimization` | `terminal-production/src/terminal_production.rs` |
| `01_abstract-operations-to-abstract-operations/src/rules/mod.rs` | `built_in_psi_registries` | `.../pass_manager/entry.rs` |
| `04_selected-instructions-to-selected-instructions/src/rewrites/selected_lowering/mod.rs` | `resolve_selected_lowering_rules` | `.../rewrites/literal_folds/mod.rs` |
| `04_selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/mod.rs` | `selected_allocation_recovery_rule` | `08_selected-instructions-to-register-homes/src/register_allocation.rs` |
| `12_resolved-layout-to-resolved-layout/src/x86_branch_relaxation/mod.rs` | `stage_optimized_x86_branch_relaxation` | `12_resolved-layout-to-resolved-layout/src/phase.rs` |

## Finding

**No orphans on `origin/main`.** Every listed consumer still names its stage's
output marker; all five stages have a live route into a coordinator or
successor file. There was nothing to repair — the outcome is a regression
guard, landed at `fddf82a61dfe`: each `RuleStageDescriptor` now carries
`output_marker` plus its `consumers` roster, and `rule_stages::check` reports
an `orphan stage output` violation when a listed consumer drops the reference
(a consumer file that can't be read is a distinct violation, not a silent
pass).

## Verification

`cargo nextest run -p omega-architecture-test -E
'test(~optimizer_source_organization)'` — `optimizer_source_organization_
preserves_semantic_owners` green at `3f5e37a0b6e1` on linux x86-64.

## Accessor-reader catalog

Mechanical sweep of every `omega-rust/{psi,omega}/pipeline/*` crate's stage
output at `9e1ff76efc` (25 output types): for each stage entrance's return
type, enumerate its `pub` self-receiver accessors and resolve every external
reader — `value.accessor(...)`/`value.field` call and read sites plus
path-qualified `Type::accessor` forms — across `omega-rust`, `tests`, and
`tools`. An orphan output is a designed read/construction channel on a stage
product with no consumer outside its own crate; internally-routed and
test-only channels are cataloged, not flagged.

## Catalog

| Output type | Defining crate | Self-accessors | External readers | Test-only | Zero external |
| --- | --- | --- | --- | --- | --- |
| `TokenStream` | psi/representations/tokens | 2 | 2 | 0 | 0 |
| `SyntaxTrees` | psi/representations/syntax-trees | 19 | 16 | 1 | 2 |
| `SymbolResolvedTrees` | psi/representations/symbol-resolved-trees | 43 | 40 | 2 | 1 |
| `SeededSymbolResolvedTrees` | psi/pipeline/02_syntax-trees-to-symbol-resolved-trees | 4 | 2 | 0 | 2 |
| `ConstInitializerSelection` | psi/pipeline/02_syntax-trees-to-symbol-resolved-trees | 6 | 5 | 0 | 1 |
| `TypedTrees` | psi/representations/typed-trees | 162 | 145 | 8 | 9 |
| `CheckedTrees` | psi/representations/checked-trees | 2 | 1 | 1 | 0 |
| `LoweredPsi` | psi/representations/lowered-psi | 0 | — | — | — |
| `PsiOptimizationStageResult` | psi/pipeline/06_lowered-psi-to-lowered-psi | 4 | 4 | 0 | 0 |
| `CheckedCompilation` | omega/compiler/checked-compilation | 57 | 49 | 8 | 0 |
| `AdmittedArtifactPlan` | omega/pipeline/00_terminal-psi-to-abstract-operations | 4 | 4 | 0 | 0 |
| `AdmittedOptimizationArtifact` | omega/pipeline/00_terminal-psi-to-abstract-operations | 5 | 5 | 0 | 0 |
| `AdmittedNativeArtifact` | omega/pipeline/00_terminal-psi-to-abstract-operations | 6 | 4 | 2 | 0 |
| `AbstractOperationPlan` | omega/representations/abstract-operations | 0 | — | — | — |
| `AbstractOperationPlanWithPlacedViewInputs` | omega/representations/abstract-operations | 0 | — | — | — |
| `ValidatedOptimizedAbstractPlan` | omega/pipeline/01_abstract-operations-to-abstract-operations | 18 | 16 | 2 | 0 |
| `TargetOperationPlan` | omega/representations/target-operations | 0 | — | — | — |
| `StagedOptimizedSelectedInstructions` | omega/pipeline/target-operations-to-selected-instructions | 9 | 8 | 0 | 0 |
| `SelectedInstructionOptimizationOutput` | omega/pipeline/04_selected-instructions-to-selected-instructions | 3 | 2 | 0 | 1 |
| `RetainedAllocation` | omega/pipeline/08_selected-instructions-to-register-homes | 8 | 8 | 0 | 0 |
| `StagedOptimizedPostAllocationMachinePlan` | omega/pipeline/09_register-homes-to-post-allocation-machine | 0 | — | — | — |
| `CanonicalTerminalArtifact` | psi/semantics/terminal-codec | 8 | 8 | 0 | 0 |
| `ProgramEntryTerminalArtifact` | omega/compiler/terminal-artifact | 3 | 3 | 0 | 0 |
| `CompileReport` | omega/compiler/compilation-report | 37 | 30 | 4 | 2 |
| `RetainedGeneratedSyntaxExtension` | omega/compiler/source-assembly | 3 | 3 | 0 | 0 |

Four outputs (`LoweredPsi`, `AbstractOperationPlan`,
`AbstractOperationPlanWithPlacedViewInputs`, `TargetOperationPlan`) are
consumed whole — passed by value into the next stage or destructured into
owned parts — and expose no accessor channel to audit.

## True orphan outputs (zero readers anywhere)

- `CompileReport::package_publication` — the name appears nowhere outside
  `compilation-report/src`; the package publication channel is assembled but
  never read.
- `CompileReport::terminal_callback_placements` — the placement data itself
  is live (it flows from `CheckedCompilation::callback_placements` into the
  retained terminal artifact's custody at `terminal_artifact.rs:94-137`), but
  this report-level view into it has no reader.
- `SyntaxTrees::root_mathematical_definitions` — convenience iterator over
  root definitions; every consumer takes the handle-keyed
  `root_mathematical_definition(handle)` route instead.
- `SyntaxTrees::snapshot_json_pretty` — the pretty-printed variant of the
  test-consumed `snapshot_json` inspection channel.
- `SymbolResolvedTrees::machine_ranking_view` — one same-crate reader feeds
  the test-only `snapshot_json` surface; nothing external ranks machines.
- `TypedTrees::normalized_machine_parameter_overload_identity` — the
  requirement-side overload-identity query; its machine-side sibling is also
  test-only (`machine_by_normalized_overload_identity`).
- `TypedTrees::push_conformance_type_parameter` — a table-builder push with
  no call anywhere; sibling `push_*_type_parameter` methods are pushed by
  construction paths or tests.
- `TypedTrees::wire_schema_version_era` — the versioned wire-schema era
  discriminator query; `wire_field_fixed_array`/`wire_field_slice_element`
  are internally routed inside `wire_schema_queries`, but this one has no
  call site at all.
- `SeededSymbolResolvedTrees::{into_unrebased_trees,
  rebase_authored_selections}` — the pre-typed-continuation accessors,
  superseded by the consumed `into_typing_continuation_parts` /
  `rebase_authored_selections_for_typed_continuation` pair; only the crate's
  own tests still name them.
- `ConstInitializerSelection::initializer_dependencies` — exercised only by
  the defining crate's own `constant/` tests.

## Internally-routed channels (not orphans)

- `TypedTrees::{wire_field_fixed_array, wire_field_slice_element}` — private
  plumbing inside `wire_schema_queries`; the consuming query is
  `wire_members`-adjacent, in-crate.
- `TypedTrees::{boundary_calling_plan_report_fingerprint_for_arguments,
  closed_integer_expression_value_in, push_domain_type_parameter}` — each has
  exactly one in-crate caller (a defaulted-argument wrapper, the
  invalid-machine variant, or the domain declaration constructor).
- `SelectedInstructionOptimizationOutput` — one accessor internally routed;
  the two consumed channels carry the product forward.

## Deliberate test-only surface (not orphans)

`{SyntaxTrees, SymbolResolvedTrees, TypedTrees}::snapshot_json`,
`*_for_test` constructors/mutators,
`CheckedTrees::state_acceptance`, `TypedTrees::machine_states_mut`,
`AdmittedNativeArtifact::{into_optimization_artifact,
try_into_native_input}`, and the `CompileReport` custody views consumed only
by compiler test suites (`executable_publication`,
`into_retained_native_artifact`, `publish_retained_terminal_artifact`,
`require_package_native_physical_evidence`) are inspection/custody adapters
maintained for the test corpus, not orphaned product.

## Disposition

No accessor was deleted in this change: the zero-reader set is dominated by
`pub` representation API (`typed-trees`, `syntax-trees`, `compilation-report`)
whose pruning is a representation-contract call owned by the
REPRESENTATION-OWNERSHIP / PIPELINE-OWNER-CONSOLIDATION cluster, not an
audit-side repair. The `SeededSymbolResolvedTrees` pair is the closest
repair-adjacent case — the superseded variants remain only because their own
crate's tests still exercise them; deleting them means retiring those test
channels, a sequencing call for the continuation owner.

## Residual risk

- `name(...)` greps do not observe destructuring (`let Foo { field } = out`)
  or reads routed through a re-exporting wrapper type; flagged names were
  cross-checked with `Type::name` UFCS patterns before listing.
- Accessor names shared across impls in different crates attribute by
  receiver type only in the per-impl sweep; a same-named method on another
  type can mask a dead channel (spot-checked on the flagged set — no mask
  found).
- The sweep covers accessor *methods* and `pub` fields on the return type; it
  does not measure whether a consumed field's *contents* are themselves read
  downstream (data-depth orphans), which needs per-field provenance tracing
  beyond this audit's call-site method.
