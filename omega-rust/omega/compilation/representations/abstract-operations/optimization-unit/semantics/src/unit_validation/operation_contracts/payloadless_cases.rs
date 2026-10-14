use crate::unit_validation::operation_contracts::claim_transfers::proposition_structural_roots;
use crate::unit_validation::structural_catalog::resolve_structural_path;
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{StructuralPlaceKind, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_semantics::structural_paths_may_overlap;

pub(crate) fn plain_scalar_sum_call(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        result:
            AbstractStructuralCallResult::Structural {
                result,
                selected_evidence,
            },
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = operation
    else {
        return false;
    };
    let Some(signature) = callee.result.structural() else {
        return false;
    };
    let Some(contract) = &callee.verified_contract else {
        return false;
    };
    matches!(
        signature.multiplicity,
        terminal_psi::StructuralMultiplicity::Affine
            | terminal_psi::StructuralMultiplicity::Unrestricted
    ) && signature.qualifications.is_empty()
        && signature.projected_qualifications.is_empty()
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && *crash_continuations == contract.crash_routes
        && selected_evidence.is_empty()
        && callee.entry_claim_declarations.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.evidence_contract_lanes.is_empty()
        && contract.requires.is_empty()
        && contract.ensures.is_empty()
        && contract.outcome_specific_ensures.is_empty()
        && callee.structural_parameters.iter().all(|parameter| {
            matches!(
                parameter.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            ) && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        })
        && types
            .get(&signature.structural_type)
            .is_some_and(|declaration| match &declaration.shape {
                terminal_psi::StructuralTypeShape::Sum { cases } => cases.iter().all(|case| {
                    case.fields.iter().all(|field| {
                        !field.relevance.is_erased() && field.field_type.scalar_type().is_some()
                    })
                }),
                _ => false,
            })
}

pub(crate) fn case_establishment_matches(
    function: &PsiOptimizationFunction,
    operation: &O,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
) -> bool {
    let O::EstablishCase {
        psi_operation,
        result,
        result_case,
        fields,
    } = operation
    else {
        return false;
    };
    function.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(
                place.kind,
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } if producer == *psi_operation && structural_type == result.structural_type
            )
    }) && matches!(
        result.multiplicity,
        terminal_psi::StructuralMultiplicity::Unrestricted
            | terminal_psi::StructuralMultiplicity::Affine
    ) && super::records::constructor_qualifications_match(
            function, result, fields, types, domains, Some(*result_case),
        )
        && result.claims.is_empty()
        && result.qualification_establishments.is_empty()
        // Construction binds only the selected payload. Shared descriptor
        // fields carry their existing loans; escape is checked independently
        // at each return against the current executable slot graph.
        && terminal_semantics::descriptor_results::owned_shared_byte_descriptor_shape(
            types.values().copied(),
            result.structural_type,
        )
        && types
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                // A mixed case binds its runtime common fields in declaration
                // order, then the selected payload, as Terminal verification did.
                let (common, cases) = match &declaration.shape {
                    terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
                    terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                        (fields.as_slice(), cases)
                    }
                    _ => return false,
                };
                cases.iter().any(|case| {
                    let roster = common
                        .iter()
                        .chain(&case.fields)
                        .filter(|field| !field.relevance.is_erased())
                        .cloned()
                        .collect::<Vec<_>>();
                    case.id == *result_case
                        && super::records::field_initializers_match(
                            function,
                            *psi_operation,
                            result,
                            &roster,
                            fields,
                            types,
                            domains,
                        )
                })
            })
}

pub(crate) fn exact_payloadless_case_return_exits(
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
) -> bool {
    let Some(signature) = callee.result.structural() else {
        return false;
    };
    if !signature.qualifications.is_empty()
        || !signature.projected_qualifications.is_empty()
        || signature.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || callee.block_nodes().any(|node| {
            matches!(
                node.operation,
                abstract_operations::call_or_call_structural_pattern!(..) | O::BoundaryCall { .. }
            )
        })
    {
        return false;
    }
    let mut exits = 0_usize;
    for block in &callee.blocks {
        let Some(node) = block.nodes.last() else {
            return false;
        };
        let O::ReturnStructural {
            source,
            returned_claims,
            ..
        } = &node.operation
        else {
            continue;
        };
        if !returned_claims.is_empty() {
            return false;
        }
        let Some(producer) = callee.structural_places.iter().find_map(|place| {
            (place.id == *source)
                .then_some(place.kind)
                .and_then(|kind| match kind {
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } if structural_type == signature.structural_type => Some(producer),
                    _ => None,
                })
        }) else {
            return false;
        };
        let Some(producer) = callee
            .block_nodes()
            .map(|node| &node.operation)
            .find(|operation| {
                matches!(
                    operation,
                    O::EstablishCase { psi_operation, .. }
                        if *psi_operation == producer
                )
            })
        else {
            return false;
        };
        let O::EstablishCase { result, fields, .. } = producer else {
            return false;
        };
        if result.place != *source
            || !fields.is_empty()
            || result.structural_type != signature.structural_type
            || !case_establishment_matches(callee, producer, types, domains)
        {
            return false;
        }
        exits += 1;
    }
    exits != 0
}

pub(crate) fn exact_payloadless_structural_call(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
) -> bool {
    let O::CallStructural {
        result: AbstractStructuralCallResult::Structural { result, .. },
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = operation
    else {
        return false;
    };
    let Some(callee_result) = callee.result.structural() else {
        return false;
    };
    let Some(contract) = callee.verified_contract.as_ref() else {
        return false;
    };
    callee.parameters.is_empty()
        && callee.structural_parameters.is_empty()
        && callee.entry_claim_declarations.is_empty()
        && callee.content_entry_claims.is_empty()
        && contract.requires.is_empty()
        && contract.ensures.is_empty()
        && callee.evidence_contract_lanes.is_empty()
        && structural_arguments.is_empty()
        && claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && *crash_continuations == contract.crash_routes
        && result.structural_type == callee_result.structural_type
        && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && result.multiplicity == callee_result.multiplicity
        && result.qualifications.is_empty()
        && result.qualifications == callee_result.qualifications
        && result.projected_qualifications.is_empty()
        && result.projected_qualifications == callee_result.projected_qualifications
        && result.claims.is_empty()
        && contract.outcome_specific_ensures.iter().all(|row| {
            proposition_structural_roots(&row.proposition)
                .into_iter()
                .all(|root| root == callee_result.place)
        })
        && exact_payloadless_case_return_exits(callee, types, domains)
}

pub(crate) fn payloadless_selected_evidence_surface_matches(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<
        semantic_vocabulary::StructuralDomainId,
        &terminal_psi::StructuralDomainDeclaration,
    >,
) -> bool {
    let O::CallStructural {
        result:
            AbstractStructuralCallResult::Structural {
                selected_evidence, ..
            },
        ..
    } = operation
    else {
        return true;
    };
    selected_evidence.is_empty()
        || (exact_payloadless_structural_call(operation, callee, types, domains)
            && callee
                .verified_contract
                .as_ref()
                .is_some_and(|contract| !contract.outcome_specific_ensures.is_empty()))
}

/// Result claims bind by path: each continues the claim the callee's
/// returning edge holds at that path beneath its returned value, so the
/// caller names no callee-local identity. The callee's own ownership replay
/// proves that every return transfers exactly the claims its value holds;
/// this join checks the shape both sides share.
pub(crate) fn validate_structural_call_result(
    result: &terminal_psi::StructuralOperationResult,
    callee: &PsiOptimizationFunction,
    exact_claim_free: bool,
    claim_transfers: &[terminal_psi::ClaimTransfer],
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let Some(signature) = callee.result.structural() else {
        return false;
    };
    if result.structural_type != signature.structural_type
        || result.multiplicity != signature.multiplicity
        || result.qualifications != signature.qualifications
        || result.projected_qualifications != signature.projected_qualifications
        || result
            .qualifications
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || result.claims.windows(2).any(|pair| pair[0] >= pair[1])
        || result.claims.iter().any(|claim| {
            resolve_structural_path(types, result.structural_type, &claim.path).is_none()
        })
        || result.claims.iter().enumerate().any(|(index, claim)| {
            result.claims[index + 1..]
                .iter()
                .any(|other| structural_paths_may_overlap(&claim.path, &other.path))
        })
    {
        return false;
    }
    if exact_claim_free {
        return true;
    }
    // A callee with no claim declarations receives no claim.
    if !claim_transfers.is_empty()
        && callee.entry_claim_declarations.is_empty()
        && callee.content_entry_claims.is_empty()
    {
        return false;
    }
    let claim_shape = match result.multiplicity {
        terminal_psi::StructuralMultiplicity::Linear => {
            matches!(result.claims.as_slice(), [claim] if claim.path.is_empty())
        }
        terminal_psi::StructuralMultiplicity::Affine => {
            result.claims.iter().all(|claim| !claim.path.is_empty())
        }
        terminal_psi::StructuralMultiplicity::Unrestricted => result.claims.is_empty(),
    };
    claim_shape
        && terminal_semantics::result_claim_catalog_matches_returns(
            &result.claims,
            callee
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .filter_map(|node| match &node.operation {
                    abstract_operations::AbstractOperation::ReturnStructural {
                        returned_claims,
                        ..
                    } => Some(returned_claims.as_slice()),
                    _ => None,
                }),
        )
}
