//! Optimizer module role: proposal leaf. Proven-case membership specialization candidates.
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
//! `proven_case == case` at every site. Machines holding an authenticated
//! cyclic component are frozen byte-exact for this family and never yield
//! rows. Memberships whose resolved position is not a proven sole-case
//! position — an unestablished multi-case root or a path ending on a
//! multi-case roster — stay unfused.

use super::{
    CaseMembershipPlan, CaseMembershipSpecializationCandidate, CaseMembershipSpecializationError,
    PlaceId, PsiOptimizationFunction, PsiOptimizationUnit, VerifiedPsiOptimizationSession,
    admission, apply, candidate_identity,
};
use std::collections::BTreeSet;

pub(super) fn all(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<CaseMembershipSpecializationCandidate>, CaseMembershipSpecializationError> {
    let unit = session.unit();
    // Machines holding an authenticated cyclic component are frozen
    // byte-exact for this family; their nodes cannot be rewritten.
    let frozen = session
        .cycle_components()
        .components()
        .iter()
        .map(|component| component.id.machine)
        .collect::<BTreeSet<_>>();
    let mut candidates = Vec::new();
    for function in &unit.functions {
        if frozen.contains(&function.machine) {
            continue;
        }
        for declaration in &function.structural_places {
            let Some(plan) = plan(unit, function, declaration.id) else {
                continue;
            };
            if plan.memberships.is_empty() {
                continue;
            }
            candidates.push(from_plan(unit, plan)?);
        }
    }
    let required = u64::try_from(candidates.len())
        .map_err(|_| CaseMembershipSpecializationError::CoordinateOverflow)?;
    if required > candidate_limit {
        return Err(
            CaseMembershipSpecializationError::CandidateBudgetExhausted {
                required,
                limit: candidate_limit,
            },
        );
    }
    Ok(candidates)
}

/// Independently derived specialization plan for one place, or `None` when
/// no membership observing it is proven by the unit. An admissible plan
/// carries every proven membership observing the place, in node order: each
/// row's basis is the place's root proof at an empty path or the sole-case
/// roster at the resolved nested position.
pub(super) fn plan(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<CaseMembershipPlan> {
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
    memberships.sort_by_key(|row| (row.site().block, row.site().node));
    Some(CaseMembershipPlan {
        machine: function.machine,
        place,
        producer: evidence.root_basis.and_then(|(_, producer)| producer),
        memberships,
    })
}

pub(super) fn from_plan(
    unit: &PsiOptimizationUnit,
    plan: CaseMembershipPlan,
) -> Result<CaseMembershipSpecializationCandidate, CaseMembershipSpecializationError> {
    let output = apply::realize(unit, &plan)?;
    let identity = candidate_identity(
        unit.identity,
        output.identity,
        plan.machine,
        plan.place,
        &plan.memberships,
    );
    Ok(CaseMembershipSpecializationCandidate {
        identity,
        input: unit.identity,
        output: output.identity,
        machine: plan.machine,
        place: plan.place,
        producer: plan.producer,
        memberships: plan.memberships,
    })
}
