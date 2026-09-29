//! Optimizer module role: proposal leaf. Proven-case membership specialization plans.
//!
//! A `StructuralCaseMembership` is foldable when the case of the position it
//! observes is proven by the unit itself. Three proofs qualify. An
//! `EstablishCase` establishment — the place's own producer, or the same
//! producer reached through `Field` descents across owned, complete
//! structural children stored by an `EstablishRecord`, through `Case` then
//! `Field` descents into the payload child an `EstablishCase` stores whole
//! for exactly the case it establishes, through the uniform block-parameter
//! binding every incoming edge spells whole, or through a case-payload
//! `StructuralLeafCopy` whose canonical projection prefixes the path still
//! to resolve beneath its source — proves the selected case at its
//! position permanently. An
//! `EstablishZeroedStructural` establishment selects each admissible sum's
//! first declared case at every position of its storage: the verifier
//! admits it only on zero-constructible shapes, so the remaining `Field`,
//! `Case`, and index path descends the declared type alone. A single-case
//! roster —
//! `StructuralTypeShape::Sum` or `Mixed` — proves the case at any
//! resolvable path without custody evidence: `Field` segments descend
//! through `Record` and `Mixed` common fields by identity, `FixedIndex`
//! and `RuntimeIndex` descend a `FixedArray` element, and `Referent`
//! crosses a `Reference` carrier. In each shape the membership verdict is
//! `proven_case == case` at every site. Memberships whose resolved
//! position is not proven — an unestablished multi-case root, a path
//! ending on a multi-case roster — stay unfused; the rule freezes
//! machines holding an authenticated cyclic component before asking for a
//! plan.

use super::{
    CaseMembershipSpecializationRewrite, PlaceId, PsiOptimizationFunction, PsiOptimizationUnit,
    admission,
};

/// Independently derived specialization plan for one place, or `None` when
/// the place is not rostered in `function`. An admissible plan carries every
/// proven membership observing the place, in node order: each row's basis is
/// the place's root proof at an empty path or the sole-case roster at the
/// resolved nested position. A plan with no memberships means no
/// observation of the place is currently proven.
pub(crate) fn plan(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<CaseMembershipSpecializationRewrite> {
    let evidence = admission::membership_evidence(unit, function, place)?;
    let mut memberships = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some(row) = admission::admit_membership_node(
                unit,
                &evidence,
                function.machine,
                block.id,
                node_index,
                node,
            ) else {
                continue;
            };
            memberships.push(row);
        }
    }
    memberships.sort_by_key(|row| (row.site.block, row.site.node));
    Some(CaseMembershipSpecializationRewrite {
        machine: function.machine,
        place,
        producer: evidence.root_basis.and_then(|(_, producer)| producer),
        memberships,
    })
}
