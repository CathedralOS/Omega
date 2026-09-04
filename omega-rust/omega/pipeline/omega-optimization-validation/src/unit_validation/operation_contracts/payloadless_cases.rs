use super::*;

pub(crate) fn affine_scalar_record_establishment_matches(
    function: &PsiOptimizationFunction,
    operation: &O,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let O::EstablishAffineScalarRecord {
        psi_operation,
        result,
        field,
        value,
    } = operation
    else {
        return false;
    };
    let place_matches = function.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(
                place.kind,
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } if producer == *psi_operation && structural_type == result.structural_type
            )
    });
    let exact_i64_field = types.get(&result.structural_type).is_some_and(|declaration| {
        matches!(
            &declaration.shape,
            psi_terminal::StructuralTypeShape::Record { fields }
                if matches!(fields.as_slice(), [candidate]
                    if candidate.id == *field
                        && candidate.relevance == psi_terminal::BindingRelevance::Relevant
                        && matches!(candidate.field_type,
                            psi_terminal::StructuralFieldType::Scalar(ScalarType::Integer(integer))
                                if integer.carrier() == psi_core::IntegerCarrier::Fixed
                                    && integer.sign() == psi_core::IntegerSign::Signed
                                    && integer.bits() == 64))
        )
    });
    let i64_type =
        psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 64).expect("signed i64 is valid");
    place_matches
        && result.multiplicity == psi_terminal::StructuralMultiplicity::Affine
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && exact_i64_field
        && i64_type.admits(*value)
}

pub(crate) fn payloadless_establishment_matches(
    function: &PsiOptimizationFunction,
    operation: &O,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let O::EstablishPayloadlessCase {
        psi_operation,
        result,
        result_case,
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
    }) && result.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && types.get(&result.structural_type).is_some_and(|declaration| {
            matches!(
                &declaration.shape,
                psi_terminal::StructuralTypeShape::Sum { cases }
                    if cases.iter().any(|case| case.id == *result_case && case.fields.is_empty())
            )
        })
}

pub(crate) fn exact_payloadless_case_return_exits(
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let Some(signature) = callee.result.structural() else {
        return false;
    };
    if !signature.qualifications.is_empty()
        || !signature.projected_qualifications.is_empty()
        || signature.multiplicity != psi_terminal::StructuralMultiplicity::Unrestricted
        || callee
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(
                    node.operation,
                    O::Call { .. }
                        | O::CallUnit { .. }
                        | O::CallUnitWithDynamicArguments { .. }
                        | O::CallStructuralScalar { .. }
                        | O::CallStructuralScalarWithDynamicArguments { .. }
                        | O::CallDynamicScalar { .. }
                        | O::CallDynamicParameterScalar { .. }
                        | O::CallDynamicUnit { .. }
                        | O::CallDynamicParameterUnit { .. }
                        | O::CallStructural { .. }
                        | O::BoundaryCall { .. }
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
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .map(|node| &node.operation)
            .find(|operation| {
                matches!(
                    operation,
                    O::EstablishPayloadlessCase { psi_operation, .. }
                        if *psi_operation == producer
                )
            })
        else {
            return false;
        };
        let O::EstablishPayloadlessCase { result, .. } = producer else {
            return false;
        };
        if result.place != *source
            || result.structural_type != signature.structural_type
            || !payloadless_establishment_matches(callee, producer, types)
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
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        result,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence: _,
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
        && contract.crash_routes.is_empty()
        && callee.evidence_contract_lanes.is_empty()
        && structural_arguments.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && result.structural_type == callee_result.structural_type
        && result.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
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
        && exact_payloadless_case_return_exits(callee, types)
}

pub(crate) fn payloadless_selected_evidence_surface_matches(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        selected_evidence, ..
    } = operation
    else {
        return true;
    };
    selected_evidence.is_empty()
        || (exact_payloadless_structural_call(operation, callee, types)
            && callee
                .verified_contract
                .as_ref()
                .is_some_and(|contract| !contract.outcome_specific_ensures.is_empty()))
}

pub(crate) fn validate_structural_call_result(
    result: &psi_terminal::StructuralOperationResult,
    callee: &PsiOptimizationFunction,
    exact_payloadless: bool,
    claim_transfers: &[psi_terminal::ClaimTransfer],
    returned: &[psi_terminal::StructuralResultClaimTransfer],
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
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
    if exact_payloadless {
        return true;
    }
    if callee.entry_claim_declarations.is_empty()
        || result.claims.is_empty()
        || returned.is_empty()
        || returned.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return false;
    }
    let callee_claims = callee
        .entry_claim_declarations
        .iter()
        .map(|claim| (claim.claim, claim.path.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let result_claims = result
        .claims
        .iter()
        .map(|claim| (claim.claim, claim.path.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let transferred = claim_transfers
        .iter()
        .map(|transfer| transfer.claim)
        .collect::<BTreeSet<_>>();
    let returned_callee = returned
        .iter()
        .map(|transfer| transfer.callee_claim)
        .collect::<BTreeSet<_>>();
    let returned_caller = returned
        .iter()
        .map(|transfer| transfer.caller_claim)
        .collect::<BTreeSet<_>>();
    callee_claims.len() == callee.entry_claim_declarations.len()
        && result_claims.len() == result.claims.len()
        && returned_callee.len() == returned.len()
        && returned_caller.len() == returned.len()
        && returned_callee == callee_claims.keys().copied().collect()
        && returned_caller == result_claims.keys().copied().collect()
        && transferred == result_claims.keys().copied().collect()
        && returned.iter().all(|transfer| {
            callee_claims.get(&transfer.callee_claim) == result_claims.get(&transfer.caller_claim)
        })
}
