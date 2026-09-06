//! Exact ordinary affine identity calls transfer real ownership without claims.

use super::*;

/// The existing Terminal identity producer has one owned affine input and
/// returns that input unchanged. Empty claim rows describe affine custody;
/// they must not be replaced with invented linear transfers.
pub(crate) fn exact_plain_affine_structural_call(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let O::CallStructural {
        result,
        arguments,
        structural_arguments,
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
    let Some(contract) = callee.verified_contract.as_ref() else {
        return false;
    };
    let [parameter] = callee.structural_parameters.as_slice() else {
        return false;
    };
    let [argument] = structural_arguments.as_slice() else {
        return false;
    };
    let [block] = callee.blocks.as_slice() else {
        return false;
    };
    let [node] = block.nodes.as_slice() else {
        return false;
    };
    let O::ReturnStructural {
        source,
        returned_claims,
        trivial_affine_locals,
        trivial_affine_discards,
        ..
    } = &node.operation
    else {
        return false;
    };

    block.id == callee.entry
        && block.parameters.is_empty()
        && *source == parameter.place
        && returned_claims.is_empty()
        && trivial_affine_locals.is_empty()
        && trivial_affine_discards.is_empty()
        && callee.parameters.is_empty()
        && arguments.is_empty()
        && parameter.position == 0
        && !parameter.is_self
        && parameter.access == terminal_psi::StructuralAccess::Owned
        && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
        && parameter.structural_type == signature.structural_type
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && argument.access == terminal_psi::StructuralAccess::Owned
        && argument.path.is_empty()
        && signature.multiplicity == terminal_psi::StructuralMultiplicity::Affine
        && signature.qualifications.is_empty()
        && signature.projected_qualifications.is_empty()
        && result.structural_type == signature.structural_type
        && result.multiplicity == signature.multiplicity
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && selected_evidence.is_empty()
        && callee.entry_claims.is_empty()
        && callee.entry_claim_declarations.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.evidence_contract_lanes.is_empty()
        && callee.published_service_ceiling.is_empty()
        && contract.requires.is_empty()
        && contract.ensures.is_empty()
        && contract.outcome_specific_ensures.is_empty()
        && contract.crash_routes.is_empty()
        && finite_owned_shape(
            types,
            signature.structural_type,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
        )
}

/// This checkpoint carries only finite records and nonempty fixed arrays.
/// Primitive record fields retain their established no-cleanup meaning.
fn finite_owned_shape(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    active: &mut BTreeSet<StructuralTypeId>,
    complete: &mut BTreeSet<StructuralTypeId>,
) -> bool {
    if complete.contains(&root) {
        return true;
    }
    if !active.insert(root) {
        return false;
    }
    let Some(declaration) = types.get(&root) else {
        return false;
    };
    let valid = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Record { fields } => fields.iter().all(|field| {
            !field.relevance.is_erased()
                && match field.field_type {
                    terminal_psi::StructuralFieldType::Structural(child) => {
                        finite_owned_shape(types, child, active, complete)
                    }
                    terminal_psi::StructuralFieldType::Scalar(_)
                    | terminal_psi::StructuralFieldType::IeeeFloat(_)
                    | terminal_psi::StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ) => true,
                    _ => false,
                }
        }),
        terminal_psi::StructuralTypeShape::FixedArray { element, length } if *length > 0 => {
            finite_owned_shape(types, *element, active, complete)
        }
        _ => false,
    };
    active.remove(&root);
    if valid {
        complete.insert(root);
    }
    valid
}
