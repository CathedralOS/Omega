//! Optimizer module role: proposal leaf. Proven-case membership specialization candidates.
//!
//! A `StructuralCaseMembership` is foldable when its observed place's case is
//! proven by the unit itself. Two proofs qualify: the place is an
//! `OperationResult` whose same-function producer is `EstablishScalarCase`,
//! so the place is assigned once by that producer and cannot be rewritten;
//! or the place's declared structural type is a closed roster of exactly one
//! case — `StructuralTypeShape::Sum` or `Mixed` — so every inhabitant holds
//! that case regardless of producer. In both shapes the membership verdict is
//! `proven_case == case` at every site. Machines holding an authenticated
//! cyclic component are frozen byte-exact for this family and never yield
//! rows. Memberships with a non-empty path observe a nested position the
//! root case does not fix and stay unfused.

use super::{
    CaseMembershipPlan, CaseMembershipSpecializationCandidate, CaseMembershipSpecializationError,
    NodeLocation, O, OperationId, PlaceId, PsiOptimizationFunction, PsiOptimizationUnit,
    ResolvedCaseMembership, ScalarType, StructuralPlaceKind, VerifiedPsiOptimizationSession, apply,
    candidate_identity,
};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{StructuralPlaceDeclaration, StructuralTypeShape};

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
/// the place's case is not proven by the unit: neither established by an
/// `EstablishScalarCase` in this function nor declared under a sole-case
/// roster. An admissible plan carries every empty-path membership observing
/// the place, in node order.
pub(super) fn plan(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<CaseMembershipPlan> {
    let (proven_case, producer) = proof_basis(unit, function, place)?;
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
                proven_case,
                outcome: proven_case == *case,
            });
        }
    }
    memberships.sort_by_key(|row| (row.site.block, row.site.node));
    Some(CaseMembershipPlan {
        machine: function.machine,
        place,
        producer,
        proven_case,
        memberships,
    })
}

/// The case `place` is proven to hold, plus its establishment producer when
/// the proof is one `EstablishScalarCase` operation result.
fn proof_basis(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<(semantic_vocabulary::StructuralCaseId, Option<OperationId>)> {
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?;
    if let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind {
        let established = function
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
            });
        if let Some(case) = established {
            return Some((case, Some(producer)));
        }
    }
    sole_case(unit, function, declaration).map(|case| (case, None))
}

/// The declared structural type of one rostered place, or `None` when the
/// place kind carries no resolvable type in this function.
pub(super) fn declared_structural_type(
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<StructuralTypeId> {
    match declaration.kind {
        StructuralPlaceKind::OperationResult {
            structural_type, ..
        }
        | StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        }
        | StructuralPlaceKind::TrivialAffineLocal {
            structural_type, ..
        } => Some(structural_type),
        StructuralPlaceKind::ProviderAttachment { attachment, .. } => Some(attachment),
        StructuralPlaceKind::Parameter { .. } => function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::BlockParameter { .. } => function
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .find(|parameter| parameter.place == declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::Result => function
            .result
            .structural()
            .and_then(|result| (result.place == declaration.id).then_some(result.structural_type)),
    }
}

/// The one case of a declared closed roster, or `None` when the place's
/// declared type is not a sum shape or names more than one case.
pub(super) fn sole_case(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<semantic_vocabulary::StructuralCaseId> {
    let structural_type = declared_structural_type(function, declaration)?;
    let declaration_type = unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == structural_type)?;
    let cases = match &declaration_type.shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return None,
    };
    let [sole] = cases.as_slice() else {
        return None;
    };
    Some(sole.id)
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
