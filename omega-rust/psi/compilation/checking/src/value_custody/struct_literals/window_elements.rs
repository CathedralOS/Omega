use super::validate_array_literal_elements_for_shape;
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Receiving geometry for a collection window. This describes type vocabulary,
/// not bounds evidence, storage access, or the result of a range-view expression.
pub struct ArrayWindowShape {
    /// Zero when the collection's semantic element has not been prepared.
    pub element_type: typed_trees::types::TypeReferenceHandle,
    pub start: Option<usize>,
    pub end: Option<usize>,
}

impl ArrayWindowShape {
    pub fn length(&self) -> Option<usize> {
        self.start
            .zip(self.end)
            .and_then(|(start, end)| end.checked_sub(start))
    }
}

pub fn array_window_shape(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    bound_lookup: &mut crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'_>,
) -> Option<ArrayWindowShape> {
    let indexed = program.expression_table.indexed_node(target)?;
    let range = program.expression_table.range_node(indexed.index)?;
    let collection_type = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        indexed.collection,
    )
    .or_else(|| {
        crate::value_custody::places::declared_indexed_projection_type_raw(
            program,
            machine,
            Some(state),
            indexed.collection,
        )
    })?;
    let shape = crate::value_custody::places::unwrapped_type_reference(program, collection_type)?;
    let (_, collection_length) =
        crate::value_custody::type_shapes::collection_element_and_length(program, shape)?;
    // The erased shape supplies extent only. The semantic projection preserves
    // qualifications and arithmetic policy inherited from the collection.
    let element_type =
        crate::value_custody::places::collection_element_type_reference(program, collection_type)
            .unwrap_or_default();
    let start = if range.start.is_valid() {
        crate::normalize_immutable_integer_bound_to_usize(program, bound_lookup, range.start)
    } else {
        Some(0)
    };
    let end = if range.end.is_valid() {
        crate::normalize_immutable_integer_bound_to_usize(program, bound_lookup, range.end)
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
    Some(ArrayWindowShape {
        element_type,
        start,
        end,
    })
}

/// The range's value is an array, not one scalar element or the whole backing
/// array. Enforce its element type for ordinary mutable writes as well as
/// write-only writes; permission and selector bounds remain separate checks.
pub(crate) fn validate_array_window_elements<'p>(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    value: ExpressionHandle,
    bound_lookup: &mut (
        &'p dyn crate::CheckingInput,
        Option<crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'p>>,
    ),
    diagnostics: &mut Vec<Diagnostic>,
) {
    if program
        .expression_table
        .indexed_node(target)
        .and_then(|indexed| program.expression_table.range_node(indexed.index))
        .is_none()
    {
        return;
    }
    let bound_lookup = bound_lookup.1.get_or_insert_with(|| {
        crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup::new(bound_lookup.0)
    });
    let Some(shape) = array_window_shape(program, machine, state, target, bound_lookup) else {
        return;
    };
    let element_type = shape.element_type;
    if !element_type.is_valid() {
        diagnostics.push(Diagnostic::error(
            "array window replacement has no prepared destination element type",
        ));
        return;
    }
    let length = shape.length();
    // Formation proves each new element; copying relies on the established
    // source element type. Use the same complete element identity as ordinary
    // fixed-array reception, never a primitive-only or constraint-erasing match.
    // The window's width, rather than the backing array's capacity, is owed.
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        if let Some(dispatch) = program.expression_table.match_node(value) {
            pending.extend(
                program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .map(|arm| arm.value),
            );
            continue;
        }
        if let Some(borrow) = program.expression_table.borrow_expression(value) {
            pending.push(borrow.target);
            continue;
        }
        if program.expression_table.array_literal_node(value).is_some() {
            validate_array_literal_elements_for_shape(
                program,
                machine,
                state,
                value,
                element_type,
                length,
                diagnostics,
            );
            continue;
        }
        let source_shape = crate::expression_result_type_reference(program, machine, state, value)
            .and_then(|reference| {
                let element = crate::value_custody::places::collection_element_type_reference(
                    program, reference,
                )?;
                let shape =
                    crate::value_custody::places::unwrapped_type_reference(program, reference)?;
                let (_, length) = crate::value_custody::type_shapes::collection_element_and_length(
                    program, shape,
                )?;
                Some((element, length))
            });
        let Some((source_element, source_length)) = source_shape else {
            diagnostics.push(Diagnostic::error(
                "array window replacement requires a source with an established collection shape",
            ));
            continue;
        };
        if !crate::value_custody::type_references::type_references_match(
            program,
            source_element,
            element_type,
        ) {
            diagnostics.push(Diagnostic::error(
                "array window replacement source must preserve the complete destination element type, including its qualifications",
            ));
        }
        if length.is_none() || source_length != length {
            diagnostics.push(Diagnostic::error(
                "array window replacement must prove the source element count equals the selected window width",
            ));
        }
    }
}
