//! Exact owned zero formation under an actually omitted constructor field.
//! Type paths select zero storage, never a referent or an unchosen case.
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn selected_type(
    program: &dyn ::validation::CheckingInput,
    mut reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Option<TypeReferenceHandle> {
    for segment in segments {
        reference = validation::peel_declared_constraints(program, reference);
        if matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Reference { .. }
        ) {
            return None;
        }
        reference = match (
            segment,
            program.type_reference_table.type_reference(reference),
        ) {
            (facts::PlaceSegment::Field { symbol }, _) => {
                let definition =
                    validation::field_domains::data_definition_for_field_type(program, reference)?;
                // Without an authored case, payload selection has no witness.
                if facts::payload_variant_for_field(program, *symbol).is_some() {
                    return None;
                }
                program
                    .closed_data_constructor_field(reference, definition.symbol, *symbol)?
                    .type_reference
            }
            (
                facts::PlaceSegment::FixedIndex { index },
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                },
            ) if *index < *length => *element_type,
            _ => return None,
        };
    }
    let carrier = validation::peel_declared_constraints(program, reference);
    if matches!(
        program.type_reference_table.type_reference(carrier),
        TypeReferenceNode::Reference { .. }
    ) {
        return None;
    }
    Some(reference)
}
