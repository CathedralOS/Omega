use super::validate_array_literal_elements_for_shape;
use diagnostics::Diagnostic;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    ExpressionHandle, ExpressionNode,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::state::State;
use symbol_resolved_trees_to_typed_trees::typed_trees::types::{
    FixedArrayLength, TypeReferenceNode,
};

/// The range's value is an array, not one scalar element or the whole backing
/// array. Enforce its element type for ordinary mutable writes as well as
/// write-only writes; permission and selector bounds remain separate checks.
pub(crate) fn validate_array_window_elements<'p>(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    value: ExpressionHandle,
    bound_lookup: &mut (
        &'p TypedTrees,
        Option<
            crate::validation::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'p>,
        >,
    ),
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(target) else {
        return;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return;
    };
    let bound_lookup = bound_lookup.1.get_or_insert_with(|| {
        crate::validation::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup::new(
            bound_lookup.0,
        )
    });
    let Some(collection_type) = crate::validation::value_custody::places::declared_place_type(
        program,
        machine,
        Some(state),
        indexed.collection,
    )
    .or_else(|| {
        crate::validation::value_custody::places::declared_indexed_projection_type(
            program,
            machine,
            Some(state),
            indexed.collection,
        )
    }) else {
        return;
    };
    let (element_type, collection_length) =
        match program.type_reference_table.type_reference(collection_type) {
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => (
                *element_type,
                match length {
                    FixedArrayLength::Literal(length) => Some(*length),
                    _ => None,
                },
            ),
            TypeReferenceNode::Slice { element_type } => (*element_type, None),
            _ => return,
        };
    let start = if range.start.is_valid() {
        crate::validation::normalize_immutable_integer_bound_to_usize(
            program,
            bound_lookup,
            range.start,
        )
    } else {
        Some(0)
    };
    let end = if range.end.is_valid() {
        crate::validation::normalize_immutable_integer_bound_to_usize(
            program,
            bound_lookup,
            range.end,
        )
        .and_then(|end| {
            if range.end_inclusive {
                end.checked_add(1)
            } else {
                Some(end)
            }
        })
    } else {
        collection_length
    };
    let length = start
        .zip(end)
        .and_then(|(start, end)| end.checked_sub(start));
    validate_array_literal_elements_for_shape(
        program,
        machine,
        state,
        value,
        element_type,
        length,
        diagnostics,
    );
}
