# Stage-output orphan audit

This audit asks whether every designed accessor channel on a stage product has
an external reader. Delete this audit once the zero-reader accessors are pruned
or kept by a representation-contract ruling.

## Accessor-reader catalog

Mechanical sweep of every `omega-rust/{psi,omega}/pipeline/*` crate's stage
output at `9e1ff76efc`: for each stage entrance's return type, enumerate its
`public` self-receiver accessors and resolve every external reader —
`value.accessor(...)`/`value.field` call and read sites plus path-qualified
`Type::accessor` forms — across `omega-rust`, `tests`, and `tools`. An orphan
output is a designed read/construction channel on a stage product with no
consumer outside its own crate; internally-routed and test-only channels are
cataloged, not flagged. The counts are as measured at `9e1ff76efc`; the table
lists only output types that still exist, at their current crates.

## Catalog

| Output type | Defining crate | Self-accessors | External readers | Test-only | Zero external |
| --- | --- | --- | --- | --- | --- |
| `TokenStream` | psi/compilation/representations/tokens | 2 | 2 | 0 | 0 |
| `SyntaxTrees` | psi/compilation/representations/syntax-trees | 19 | 16 | 1 | 2 |
| `SymbolResolvedTrees` | psi/compilation/representations/symbol-resolved-trees | 43 | 40 | 2 | 1 |
| `SeededSymbolResolvedTrees` | psi/compilation/representations/symbol-resolved-trees | 4 | 2 | 0 | 2 |
| `ConstInitializerSelection` | psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees | 6 | 5 | 0 | 1 |
| `TypedTrees` | psi/compilation/representations/typed-trees | 162 | 145 | 8 | 9 |
| `CheckedCompilation` | omega/compilation/products | 57 | 49 | 8 | 0 |
| `AdmittedOptimizationArtifact` | omega/compilation/pipeline/00_terminal-psi-to-abstract-operations | 5 | 5 | 0 | 0 |
| `AdmittedNativeArtifact` | omega/compilation/pipeline/00_terminal-psi-to-abstract-operations | 6 | 4 | 2 | 0 |
| `AbstractOperationPlan` | omega/compilation/representations/abstract-operations | 0 | — | — | — |
| `ValidatedOptimizedAbstractPlan` | omega/compilation/pipeline/01_abstract-operations-to-abstract-operations | 18 | 16 | 2 | 0 |
| `TargetOperationPlan` | omega/compilation/representations/target-operations | 0 | — | — | — |
| `StagedOptimizedSelectedInstructions` | omega/compilation/pipeline/03_target-operations-to-selected-instructions | 9 | 8 | 0 | 0 |
| `SelectedInstructionOptimizationOutput` | omega/compilation/pipeline/04_selected-instructions-to-selected-instructions | 3 | 2 | 0 | 1 |
| `RetainedAllocation` | omega/compilation/pipeline/05_selected-instructions-to-register-homes | 8 | 8 | 0 | 0 |
| `StagedOptimizedPostAllocationMachinePlan` | omega/compilation/pipeline/06_register-homes-to-post-allocation-machine | 0 | — | — | — |
| `CanonicalTerminalArtifact` | psi/compilation/representations/terminal-psi/codec | 8 | 8 | 0 | 0 |
| `CompileReport` | omega/compilation/products | 37 | 30 | 4 | 2 |

Two outputs (`AbstractOperationPlan`, `TargetOperationPlan`) are consumed
whole — passed by value into the next stage or destructured into owned parts —
and expose no accessor channel to audit.

## True orphan outputs (zero readers anywhere)

- `CompileReport::package_publication` — the accessor in
  `products/src/compilation_products.rs` has no caller; the package
  publication channel is assembled but never read.
- `SyntaxTrees::snapshot_json_pretty` — the pretty-printed variant of the
  test-consumed `snapshot_json` inspection channel.
- `SymbolResolvedTrees::machine_ranking_view` — one same-crate reader feeds
  the test-only `snapshot_json` surface; nothing external ranks machines.
- `ConstInitializerSelection::initializer_dependencies` — read only inside
  its defining crate.

## Internally-routed channels (not orphans)

- `TypedTrees::{wire_field_fixed_array, wire_field_slice_element}` — private
  plumbing inside `wire_schema_queries`; the consuming query is
  `wire_members`-adjacent, in-crate.
- `TypedTrees::closed_integer_expression_value_in` — exactly one in-crate
  caller.
- `SelectedInstructionOptimizationOutput` — one accessor internally routed;
  the two consumed channels carry the product forward.

## Deliberate test-only surface (not orphans)

`{SyntaxTrees, SymbolResolvedTrees, TypedTrees}::snapshot_json`,
`*_for_test` constructors/mutators, `TypedTrees::machine_states_mut`, and
`AdmittedNativeArtifact::{into_optimization_artifact,
try_into_native_input}` are inspection/custody adapters maintained for
compiler tests, not orphaned product.

## Disposition

The zero-reader set is dominated by `public` representation API
(`typed-trees`, `syntax-trees`, the compile report) whose pruning is a
representation-contract call owned by the REPRESENTATION-OWNERSHIP /
PIPELINE-OWNER-CONSOLIDATION cluster, not an audit-side repair.

## Residual risk

- `name(...)` greps do not observe destructuring (`let Foo { field } = out`)
  or reads routed through a re-exporting wrapper type; flagged names were
  cross-checked with `Type::name` UFCS patterns before listing.
- Accessor names shared across impls in different crates attribute by
  receiver type only in the per-impl sweep; a same-named method on another
  type can mask a dead channel (spot-checked on the flagged set — no mask
  found).
- The sweep covers accessor *methods* and `public` fields on the return type; it
  does not measure whether a consumed field's *contents* are themselves read
  downstream (data-depth orphans), which needs per-field provenance tracing
  beyond this audit's call-site method.
