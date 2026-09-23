//! Optimizer module role: proposal leaf. Proven-case membership specialization plans.
//!
//! A `StructuralCaseMembership` is foldable when the case of the position it
//! observes is proven by the unit itself. At an empty path the observed
//! position is the source place itself, and two proofs qualify: the place is
//! an `OperationResult` whose same-function producer is `EstablishScalarCase`,
//! so the place is assigned once by that producer and cannot be rewritten;
//! or the place's declared structural type is a closed roster of exactly one
//! case — `StructuralTypeShape::Sum` or `Mixed` — so every inhabitant holds
//! that case regardless of producer. At a non-empty path the observed
//! position is nested, so the establishment proof no longer applies: only
//! the roster at the resolved end type can prove the verdict. `Field`
//! segments descend through `Record` and `Mixed` common fields by identity,
//! `FixedIndex` descends a `FixedArray` element, and `Referent` crosses a
//! `Reference` carrier. In each shape the membership verdict is
//! `proven_case == case` at every site. Memberships whose resolved position
//! is not a proven sole-case position — an unestablished multi-case root or
//! a path ending on a multi-case roster — stay unfused; the rule freezes
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
