//! Storage contained in a by-value call result. This does not classify the
//! origins of references stored in that result or discharge its permissions.

use validation::CanonicalPlace;
use validation::project_type_reference_from_segments;

use typed_trees::data::DataMember;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn is_private_result_place(
    program: &dyn ::validation::CheckingInput,
    place: &CanonicalPlace,
) -> bool {
    let Some(expression) = place.root.as_expression() else {
        return false;
    };
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    let Some(result) = validation::call_target_return_type(program, call.target_symbol) else {
        return false;
    };
    let Some(root_type) = unconstrained_type(program, result) else {
        return false;
    };
    match program.type_reference_table.type_reference(root_type) {
        TypeReferenceNode::Named { symbol, .. }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            ..
        } => {
            if !symbol.is_valid()
                || program
                    .data_definitions()
                    .iter()
                    .filter(|definition| definition.symbol == *symbol)
                    .count()
                    != 1
            {
                return false;
            }
        }
        TypeReferenceNode::FixedArray { element_type, .. } if element_type.is_valid() => {}
        _ => return false,
    }
    let mut current = result;
    let mut selected_variant = None;
    for (index, segment) in place.segments.iter().enumerate() {
        let Some(reference) = unconstrained_type(program, current) else {
            return false;
        };
        match (
            program.type_reference_table.type_reference(reference),
            segment,
        ) {
            (
                TypeReferenceNode::Named { symbol, .. }
                | TypeReferenceNode::Generic {
                    base_symbol: symbol,
                    ..
                },
                facts::PlaceSegment::Case { .. } | facts::PlaceSegment::Field { .. },
            ) => {
                let mut definitions = program
                    .data_definitions()
                    .iter()
                    .filter(|definition| symbol.is_valid() && definition.symbol == *symbol);
                let Some(definition) = definitions.next() else {
                    return false;
                };
                if definitions.next().is_some() {
                    return false;
                }
                let members = program.data_members(definition);
                match segment {
                    facts::PlaceSegment::Case { variant } => {
                        if selected_variant.is_some() || !members.iter().any(|member| {
                            matches!(member, DataMember::Variant(candidate) if candidate.symbol == *variant)
                        }) {
                            return false;
                        }
                        selected_variant = Some(*variant);
                        continue;
                    }
                    facts::PlaceSegment::Field { symbol } => {
                        if !symbol.is_valid()
                            || !members.iter().any(|member| match member {
                                DataMember::Field(field) => {
                                    selected_variant.is_none() && field.symbol == *symbol
                                }
                                DataMember::Variant(variant) => {
                                    selected_variant == Some(variant.symbol)
                                        && program
                                            .data_payload_fields(variant)
                                            .iter()
                                            .any(|field| field.symbol == *symbol)
                                }
                            })
                        {
                            return false;
                        }
                        selected_variant = None;
                    }
                    _ => unreachable!(),
                }
            }
            (
                TypeReferenceNode::FixedArray { .. },
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. },
            ) if selected_variant.is_none() => {}
            // A reference/slice traversal leaves the result's own storage.
            // Unknown types and selectors are not proof of a private place.
            _ => return false,
        }
        let Some(projected) =
            project_type_reference_from_segments(program, result, &place.segments[..=index])
        else {
            return false;
        };
        current = projected;
    }
    selected_variant.is_none() && current.is_valid()
}

fn unconstrained_type(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let reference = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    reference.is_valid().then_some(reference)
}
