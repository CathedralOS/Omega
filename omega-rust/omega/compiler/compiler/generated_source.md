# Admitted build and generated-source continuation

The contract is [build execution](../../../../wiki/spec/build/execution.md).
This note maps its Rust implementation; supported append cohorts are compiler
limits, not a different generated-source language.

## Entry points

- [build_scope.rs](src/pipeline/build_scope.rs) binds request staging and sponsor
  inputs to the package/root filesystem scope. It checks canonical Source metadata
  before reopening review-only replay; it does not admit or execute the build.
- [checked_entry.rs](src/pipeline/checked_entry.rs): `AdmittedBuildCheckpoint`
  couples the coherent frontend, admitted build, package verdict, and base source
  map. Execution verifies the returned build symbol. `try_seeded_extension`
  continues the retained frontend rather than reconstructing it.
- [build-evaluation](../../build/build-evaluation/src/lib.rs):
  `AdmittedBuildProgram` retains the prepared program and program-bound entry
  token with reach/admission, initial Build value, target, scope, and sponsor.
  Evaluation and replay consume the admitted route.
- [source assembly](src/pipeline/source_assembly.rs): generated units and
  dependency bundles retain source bytes, logical paths, and producer custody.
- [seeded resolution](../../../psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/lowerer.rs)
  appends the later stratum and rebases only extension-owned selections.
- [typed continuation](../../../psi/pipeline/symbol-resolved-trees-to-typed-trees/src/lowerer.rs)
  validates append cohorts and preserves the base. The
  [continuation tests](../../../psi/pipeline/symbol-resolved-trees-to-typed-trees/src/lowerer/tests.rs)
  include extension-owned nominal static-machine binders.

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

Suffix finalization handles satisfied declarations, progress premises, domain
constraints, semantic qualification casts, and fixed-byte literal landing.
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
The exercising build continuation and independent Terminal execution command is
`cargo nextest run -p compiler --test build_config_granted generated_bodies_use_retained_module_constants_after_build_execution --no-fail-fast`.

Integration controls live in [generated invocations](tests/build_config_granted/generated_invocations.rs)
and [package inputs](tests/package_compilation_inputs.rs). They are the places
to check no-rerun handoff, exact custody, one-way visibility, and final package
authority when extending this path.
