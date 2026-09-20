//! Optimizer module role: proposal leaf. Established-case-derived exact specialization candidates.
//!
//! A `StructuralCaseMembership` is foldable when its observed place is an
//! `OperationResult` whose same-function producer is `EstablishScalarCase`:
//! the place is assigned once by that producer and cannot be rewritten, so
//! the membership verdict is `result_case == case` at every site. Machines
//! holding an authenticated cyclic component are frozen byte-exact for this
//! family and never yield rows. Memberships with a non-empty path observe a
//! nested position the root case does not fix and stay unfused.

use super::{
    CaseMembershipSpecializationCandidate, CaseMembershipSpecializationError, EstablishedCasePlan,
    NodeLocation, O, OperationId, PlaceId, PsiOptimizationFunction, PsiOptimizationUnit,
    ResolvedCaseMembership, ScalarType, StructuralPlaceKind, VerifiedPsiOptimizationSession, apply,
    candidate_identity,
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
            let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
                continue;
            };
            let Some(plan) = plan(function, declaration.id, producer) else {
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
/// the place is not established by an `EstablishScalarCase` in this function.
/// An admissible plan carries every empty-path membership observing the
/// place, in node order.
pub(super) fn plan(
    function: &PsiOptimizationFunction,
    place: PlaceId,
    producer: OperationId,
) -> Option<EstablishedCasePlan> {
    let established_case = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::EstablishScalarCase {
                psi_operation,
                result,
                result_case,
                ..
            } if *psi_operation == producer && result.place == place => Some(*result_case),
            _ => None,
        })?;
    let mut memberships = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let O::StructuralCaseMembership {
                psi_operation,
                result,
                source,
                path,
                case,
            } = &node.operation
            else {
                continue;
            };
            if *source != place || result.scalar_type != ScalarType::Boolean || !path.is_empty() {
                continue;
            }
            memberships.push(ResolvedCaseMembership {
                site: NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                },
                psi_operation: *psi_operation,
                result: result.value,
                source: place,
                producer,
                observed_case: *case,
                established_case,
                outcome: established_case == *case,
            });
        }
    }
    memberships.sort_by_key(|row| (row.site.block, row.site.node));
    Some(EstablishedCasePlan {
        machine: function.machine,
        place,
        producer,
        established_case,
        memberships,
    })
}

pub(super) fn from_plan(
    unit: &PsiOptimizationUnit,
    plan: EstablishedCasePlan,
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
