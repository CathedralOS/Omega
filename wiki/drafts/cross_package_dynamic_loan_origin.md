# CROSS-PACKAGE-DYNAMIC-LOAN-ORIGIN — scope verification record

Board row: TASKS.md `CROSS-PACKAGE-DYNAMIC-LOAN-ORIGIN`, mined candidate.

## Resolution: alias of the cross-package-visibility loan-origin cluster

The name re-mines the same surface as the resolved umbrella row
`CROSS-PACKAGE-DYNAMIC-EVIDENCE-LOAN-ORIGIN`: the recorded failure
`cross_package_visibility::public_dynamic_return_may_carry_private_producer_selected_evidence`
("state `code` requires an exact retained loan origin for its shared
receiver") stopped emitting after SHARED-RECEIVER-LOAN-ORIGIN landed the
retained-lineage/borrow-evidence family at `e76d715c8e`.

Verification surface: `tests/package_compilation_inputs.rs` module
`cross_package_visibility`
(`omega-rust/omega/compiler/compiler/tests/package_compilation_inputs/cross_package_visibility.rs`,
21 tests covering public dynamic returns carrying private
producer-selected evidence, cross-package borrow/lifetime joins, and
package-boundary receiver custody).

## Re-verification attempt

Prior recorded green: 21/21 `cross_package_visibility` at `dcfb595098`
(linux x86-64), zero loan-origin diagnostics — per the umbrella row and
PACKAGE-CROSS-VISIBILITY-LOAN-ORIGIN sibling annotation.

This host attempted re-verification at `d21620fa27` via
`cargo nextest run -p compiler --test package_compilation_inputs -E
'test(cross_package_visibility)'`; the build is blocked by unrelated
in-flight breakage at main tip: `external-roots` commit `2d8c5136cc`
imports `effects::ComponentEraJournalRoster`, which `effects` does not
export at this revision (producer half of the epoch-cohort journal work
has not landed). Not attributable to the loan-origin surface.

## Slice status

No independent slice remains: the named semantic is pinned by the landed
cross_package_visibility battery; the row resolves as a re-mine pointer
to CROSS-PACKAGE-DYNAMIC-EVIDENCE-LOAN-ORIGIN / SHARED-RECEIVER-LOAN-ORIGIN.
