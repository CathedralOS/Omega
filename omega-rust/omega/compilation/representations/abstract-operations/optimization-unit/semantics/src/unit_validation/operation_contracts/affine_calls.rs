//! Exact ordinary affine identity calls transfer real ownership without claims.
use crate::unit_validation::operation_contracts::plain_calls::plain_structural_call;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::StructuralTypeId;
use std::collections::{BTreeMap, BTreeSet};

/// The existing Terminal identity producer has one owned affine input and
/// returns that input unchanged. Empty claim rows describe affine custody;
/// they must not be replaced with invented linear transfers.
pub(crate) fn exact_plain_affine_structural_call(
    operation: &O,
    callee: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    let Some(call) = plain_structural_call(operation, callee) else {
        return false;
    };
    let [parameter] = callee.structural_parameters.as_slice() else {
        return false;
    };
    let [argument] = call.structural_arguments else {
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
        cleanup_actions,
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
        && cleanup_actions.is_empty()
        && callee.parameters.is_empty()
        && call.arguments.is_empty()
        && parameter.position == 0
        && parameter.access == terminal_psi::StructuralAccess::Owned
        && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
        && parameter.structural_type == call.signature.structural_type
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && argument.access == terminal_psi::StructuralAccess::Owned
        && argument.path.is_empty()
        && call.signature.multiplicity == terminal_psi::StructuralMultiplicity::Affine
        && call.signature.qualifications.is_empty()
        && call.signature.projected_qualifications.is_empty()
        && call.result.structural_type == call.signature.structural_type
        && call.result.multiplicity == call.signature.multiplicity
        && call.selected_evidence.is_empty()
        && callee.entry_claims.is_empty()
        && call.contract.outcome_specific_ensures.is_empty()
        && callee.published_service_ceiling.is_empty()
        && finite_owned_shape(
            types,
            call.signature.structural_type,
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
        terminal_psi::StructuralTypeShape::Record { fields, .. } => fields.iter().all(|field| {
            !field.relevance.is_erased()
                && match field.field_type {
                    terminal_psi::StructuralFieldType::Structural(child) => {
                        finite_owned_shape(types, child, active, complete)
                    }
                    terminal_psi::StructuralFieldType::Scalar(_)
                    | terminal_psi::StructuralFieldType::Atomic(_)
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
        terminal_psi::StructuralTypeShape::Mixed { fields, cases } => fields
            .iter()
            .chain(cases.iter().flat_map(|case| case.fields.iter()))
            .all(|field| {
                !field.relevance.is_erased()
                    && match field.field_type {
                        terminal_psi::StructuralFieldType::Structural(child) => {
                            finite_owned_shape(types, child, active, complete)
                        }
                        terminal_psi::StructuralFieldType::Scalar(_)
                        | terminal_psi::StructuralFieldType::Atomic(_)
                        | terminal_psi::StructuralFieldType::IeeeFloat(_)
                        | terminal_psi::StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                        ) => true,
                        _ => false,
                    }
            }),
        _ => false,
    };
    active.remove(&root);
    if valid {
        complete.insert(root);
    }
    valid
}
