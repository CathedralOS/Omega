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

## Re-verification

Prior recorded green: 21/21 `cross_package_visibility` at `dcfb595098`
(linux x86-64), zero loan-origin diagnostics — per the umbrella row and
PACKAGE-CROSS-VISIBILITY-LOAN-ORIGIN sibling annotation.

An earlier attempt at `d21620fa27` was blocked by unrelated in-flight
breakage (`external-roots` commit `2d8c5136cc` imported
`effects::ComponentEraJournalRoster`, which `effects` did not export at
that revision). That breakage is gone: neither side of the import
remains on current main.

Re-verified green at `8de5f83c09` (macOS arm64, mbx/nextest):
`mbx nextest run -p compiler --test package_compilation_inputs -E
'test(cross_package_visibility)'` — 21/21 pass in 1.9 s, including
`public_dynamic_return_may_carry_private_producer_selected_evidence`
(the public bare-dynamic return carrying private producer-selected
evidence accepts, while naming the producer's private conformance still
rejects), with zero loan-origin diagnostics.

Re-verified again at `d6a0625f6b` (macOS arm64, mbx/nextest, same command):
21/21 pass in 2.0 s with zero loan-origin diagnostics, while resolving the
remaining sibling stubs on the same surface.

## Slice status

No independent slice remains: the named semantic is pinned by the landed
cross_package_visibility battery; the row resolves as a re-mine pointer
to CROSS-PACKAGE-DYNAMIC-EVIDENCE-LOAN-ORIGIN / SHARED-RECEIVER-LOAN-ORIGIN.

The sibling stubs on the same surface resolve under the same battery at
`d6a0625f6b`, each pinning the named leg it re-mines — no independent
slice under any of them:

- `DYNAMIC-RETURN-LOAN-ORIGIN` — a dynamic return carrying
  producer-selected evidence across the package boundary, pinned by
  `public_dynamic_return_may_carry_private_producer_selected_evidence`.
- `PACKAGE-DYNAMIC-RETURN-LOAN-ORIGIN` — the package boundary on the same
  dynamic-return surface, same pin.
- `PRIVATE-PRODUCER-EVIDENCE-LOAN-ORIGIN` — private producer-selected
  evidence retained across the package boundary, pinned by
  `public_dynamic_return_may_carry_private_producer_selected_evidence`
  and `quotient_formation_retains_selected_evidence_as_private_package_custody`.
