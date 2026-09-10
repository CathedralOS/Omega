//! Primitive arrays retain ordered scalar leaves and ordinary owned storage.
//! Shape reconstruction uses the Terminal type contract, including empty inner
//! dimensions; current value availability and ownership are checked separately.

use super::*;

pub(crate) fn scalar_array_establishment_matches(
    function: &PsiOptimizationFunction,
    operation: &O,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let O::EstablishScalarArray {
        psi_operation,
        result,
        elements,
    } = operation
    else {
        return false;
    };
    let Some((scalar_type, count)) = terminal_semantics::scalar_array_leaf_shape(
        types.values().copied(),
        result.structural_type,
    ) else {
        return false;
    };
    result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && u64::try_from(elements.len()).ok() == Some(count)
        && function.structural_places.iter().any(|place| {
            place.id == result.place
                && matches!(place.kind,
                StructuralPlaceKind::OperationResult { producer, structural_type }
                    if producer == *psi_operation && structural_type == result.structural_type)
        })
        && elements.iter().all(|element| {
            function
                .parameters
                .iter()
                .chain(function.blocks.iter().flat_map(|block| {
                    block
                        .parameters
                        .iter()
                        .chain(block.nodes.iter().flat_map(|node| &node.definitions))
                }))
                .any(|definition| {
                    definition.value == *element && definition.scalar_type == scalar_type
                })
        })
}

pub(crate) fn plain_scalar_array_call(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        result,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence,
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
    signature.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && signature.qualifications.is_empty()
        && signature.projected_qualifications.is_empty()
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && selected_evidence.is_empty()
        && callee.entry_claim_declarations.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.evidence_contract_lanes.is_empty()
        && contract.requires.is_empty()
        && contract.ensures.is_empty()
        && contract.crash_routes.is_empty()
        && contract.outcome_specific_ensures.is_empty()
        // Argument types/access are independently checked by the ordinary call
        // contract. An array result must not exclude a borrowed primitive input.
        && callee.structural_parameters.iter().all(|parameter| {
            parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        })
        && terminal_semantics::scalar_array_leaf_shape(
            types.values().copied(),
            signature.structural_type,
        )
        .is_some()
}
