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
    NodeLocation, O, OperationId, PlaceId, PsiOptimizationFunction, PsiOptimizationUnit,
    ResolvedCaseMembership, ScalarType, StructuralPlaceKind, VerifiedPsiOptimizationSession, apply,
    candidate_identity,
};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeShape,
};

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
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?;
    let root_type = declared_structural_type(function, declaration);
    let root_basis = proof_basis(unit, function, declaration);
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
            if *source != place || result.scalar_type != ScalarType::Boolean {
                continue;
            }
            // An empty path observes the place's root case and uses its
            // establishment-or-roster basis; a non-empty path observes a
            // nested position only the resolved roster can prove.
            let Some((proven_case, producer)) = (if path.is_empty() {
                root_basis
            } else {
                root_type
                    .and_then(|root| resolved_sole_case(unit, root, path))
                    .map(|case| (case, None))
            }) else {
                continue;
            };
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
        producer: root_basis.and_then(|(_, producer)| producer),
        memberships,
    })
}

/// The case `place`'s root is proven to hold, plus its establishment
/// producer when the proof is one `EstablishScalarCase` operation result.
fn proof_basis(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<(semantic_vocabulary::StructuralCaseId, Option<OperationId>)> {
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
                } if *psi_operation == producer && result.place == declaration.id => {
                    Some(*result_case)
                }
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

/// The one case of the closed roster the `path` resolves to under the
/// place's declared structural type, or `None` when the path fails to
/// descend — a `Field` name absent from a `Record`/`Mixed` common-field
/// roster, a `FixedIndex` on a non-array, a `Referent` crossing on a
/// non-reference — or when the resolved end type is not a `Sum`/`Mixed`
/// roster of exactly one case.
pub(super) fn sole_case_at_path(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
    path: &[StructuralPathSegment],
) -> Option<semantic_vocabulary::StructuralCaseId> {
    resolved_sole_case(unit, declared_structural_type(function, declaration)?, path)
}

/// The one case of a declared closed roster, or `None` when the place's
/// declared type is not a sum shape or names more than one case.
pub(super) fn sole_case(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<semantic_vocabulary::StructuralCaseId> {
    sole_case_at_path(unit, function, declaration, &[])
}

/// The sole case of the closed roster at `path`'s end under `root`, or
/// `None` when the path fails to descend or the end type is not a sole-case
/// `Sum`/`Mixed` roster.
fn resolved_sole_case(
    unit: &PsiOptimizationUnit,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<semantic_vocabulary::StructuralCaseId> {
    let mut current = root;
    for segment in path {
        current = descended_type(unit, current, segment)?;
    }
    roster_sole_case(unit, current)
}

/// The structural type one path segment descends to under `current`, or
/// `None` when the segment does not apply to `current`'s shape or names a
/// field whose declared type is not structural.
fn descended_type(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    segment: &StructuralPathSegment,
) -> Option<StructuralTypeId> {
    let shape = &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == current)?
        .shape;
    match segment {
        StructuralPathSegment::Field(identity) => {
            let fields = match shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => return None,
            };
            match fields
                .iter()
                .find(|field| field.identity == *identity)?
                .field_type
            {
                StructuralFieldType::Structural(next) => Some(next),
                _ => None,
            }
        }
        StructuralPathSegment::FixedIndex(_) => match shape {
            StructuralTypeShape::FixedArray { element, .. } => Some(*element),
            _ => None,
        },
        StructuralPathSegment::Referent => match shape {
            StructuralTypeShape::Reference { referent, .. } => Some(*referent),
            _ => None,
        },
    }
}

/// The one case of `type_id`'s closed roster, or `None` when its shape is
/// not a `Sum`/`Mixed` or names more than one case.
fn roster_sole_case(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<semantic_vocabulary::StructuralCaseId> {
    let cases = match &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)?
        .shape
    {
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
