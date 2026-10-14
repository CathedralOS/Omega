//! Storage-plan identity may forget an index only when complete member types
//! do not depend on it. This is not source application equality or admission.

use crate::TypedTrees;
use crate::data::{TypeParameter, TypeParameterKind};
use crate::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

pub(super) fn independent_value_index(
    program: &TypedTrees,
    owner: SymbolHandle,
    parameter: &TypeParameter,
) -> bool {
    if !matches!(parameter.kind, TypeParameterKind::Value { .. }) {
        return false;
    }
    let Some(data) = program.unique_data_definition_by_symbol(owner) else {
        return false;
    };
    if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || data.quotient.is_some()
        || !data.where_facts.is_empty()
        || data.zero_gated
    {
        return false;
    }
    // Inspect all members, including erased and variant payload fields.
    // Provisional identity markers are not evidence of independence.
    program
        .data_member_fields(program.data_members(data))
        .all(|field| independent_type(program, field.type_reference, parameter.symbol, 0))
}

fn independent_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    parameter: SymbolHandle,
    depth: usize,
) -> bool {
    if depth > 128
        || !program
            .type_reference_table
            .contains_type_reference(reference)
    {
        return false;
    }
    let child = |reference| independent_type(program, reference, parameter, depth + 1);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            *symbol != parameter
                && program
                    .data_definition_by_symbol(*symbol)
                    .and_then(|data| data.generic_instance)
                    .is_none_or(child)
        }
        TypeReferenceNode::Reference { referee, .. } => child(*referee),
        TypeReferenceNode::Slice { element_type }
        | TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => child(*element_type),
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .all(|argument| child(*argument)),
        TypeReferenceNode::Unit => true,
        // Richer field contracts need their own complete independence proof.
        // In particular, no symbolic extent, predicate, or open expression
        // obtains a common storage identity through an unsupported marker.
        _ => false,
    }
}
