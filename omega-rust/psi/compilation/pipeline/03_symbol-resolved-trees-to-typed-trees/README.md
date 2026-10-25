# Symbol-resolved trees to typed trees: generated-source continuation

The contract is [build execution](../../../../../wiki/spec/build/execution.md).
This note maps its Rust implementation; supported append cohorts are compiler
limits, not a different generated-source language.

## Entry points

- [filesystem_scope.rs](../../../../omega/packages/compilation-inputs/src/build_execution/filesystem_scope.rs) binds request staging and sponsor
  inputs to the package/root filesystem scope. It checks canonical Source metadata
  before reopening review-only replay; it does not admit or execute the build.
- [build_continuation.rs](../../../../omega/compilation/driver/src/checked/checking/build_continuation.rs):
  `evaluate_build_and_continue` schedules Build evaluation, then
  `continue_generated_sources` resolves and types the extension against the
  retained frontend rather than reconstructing it.
- [build-evaluation](../../../../omega/compilation/build-evaluation/src/build_evaluation.rs):
  `evaluate_build` owns declaration admission, filesystem preparation, exact
  machine admission, consent and execution. Its private `AdmittedBuildProgram`
  retains the prepared program, selected machine, scope and sponsor; the
  coordinator receives the completed result, not an executable checkpoint.
- [source assembly](../../../../omega/packages/compilation-inputs/src/source_assembly.rs): generated units and
  dependency bundles retain source bytes, logical paths, and producer custody.
  `GeneratedSourceExtension::parse` joins the exact base and produces separate
  syntax forests through Psi 00/01 before this continuation resolves them.
- [seeded resolution](../02_syntax-trees-to-symbol-resolved-trees/src/resolution.rs)
  appends the later stratum and rebases only extension-owned selections.
- [typed continuation](src/lowerer/seeded_continuation.rs)
  validates append cohorts and preserves the base. Extension-owned nominal
  static-machine binders are pinned by the corpus fixtures under
  `tests/omega/pass/build` and `tests/omega/fail/build`, which bind a program
  entry and run the continuation end to end.

## Bounded append support

| Cohort | Retained constraints |
| --- | --- |
| Data | Plain/lifetime-bearing declarations, local/base applications, bounded constrained arguments, scalar and validated structured const provenance. |
| Monomorphic machines | Ordinary bodies and attached methods; exact `attached_data_symbol` selects the retained or generated owner. |
| Retained constant body uses | Exact selected base declaration, declaration-resolved initializer copied per use, and unchanged visibility/type/custody checking. |
| Generic machines | Authored-order erased lifetimes followed by machine-parented Type or scalar/validated structured const binders. |
| Type binders | Full authored multiplicity and four-axis carry-property bounds. |
| Structured const occurrences | Exact binder symbol/name and template slot with the same checked carrier. |
| Structural static-machine binder | One exact machine-parented binder, flat parameter/result signature over the supported types, and body calls targeting it. |
| Nominal static-machine binder | One ordinary nongeneric trait/requirement pair from the base or extension, with exact trait, requirement, path, and binder symbols. |

An extension-owned nominal trait is non-boundary, nongeneric, nonempty, with
flat bodyless requirements over supported types.
Boundary traits, nested contract telescopes, operational/proof contracts,
default bodies, declaration-identity binders, plural/nested static-machine
binders, generated-machine `satisfies`, broader generic/conformance bounds,
supply forms, and root/data shapes outside the admitted cohorts reject.

Both this continuation and the fresh product route close their run through the
same `close_typed_cohort`: satisfied declarations, progress premises, domain
constraints, semantic qualification casts, fixed-byte literal landing, range
argument validation, provider default finalization, target-machine families
and dynamic call target binding, each over the rows the run produced.
The matching post-typing evaluator consumes the completed candidate; wire-plan
publication starts at the extension wire-schema frontier. The structural prefix
gate returns the owned base unchanged on failure, and orchestration rejects.
Preservation compares retained roots, node identities, and child-table storage
directly; diagnostic snapshots do not establish an unchanged predecessor.
The retained base remains available for the post-evaluation comparison.

Target-scoped units finish their independent pre-resolution evaluation before
the ordinary target filter. The filter consumes the retained base selection
into a combined selection, then settles selected origins and target-owned
`provider_defaults` against the final typed continuation. Only extension
declarations are mutable during this process.
Units share one immutable package-selection authority for this continuation;
sharing it neither makes sibling declarations visible nor admits transitive-only
package selections.

There is no combined-syntax rebuild switch, second frontend reconstruction, or
source-span/name recovery of the admitted entry. New cohorts must extend the
transactional continuation rather than restore one of those routes.

Constant initializers remain detached roots in the resolved base, not runtime
constant storage or decoded review strings. Only newly authored initializers
undergo name resolution; later imports cannot reinterpret retained constructors.
Typing discards the initializer link while keeping declaration/use custody.
