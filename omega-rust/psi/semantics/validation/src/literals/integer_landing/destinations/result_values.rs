use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

/// A selected range requests its collection's element type independently of
/// runtime length. Reject a statically impossible footprint here, but leave
/// dynamic footprint and write-permission checks to their assignment owners.
pub(super) fn admit_assignment_value(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    value: ExpressionHandle,
    admitted: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle) -> bool,
    other_elements: &mut Vec<ExpressionHandle>,
) -> bool {
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

    if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(target)
        && let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index)
    {
        let Some(collection_type) =
            crate::places::declared_place_type(program, machine, Some(state), indexed.collection)
        else {
            return false;
        };
        let (element_type, length) =
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
                _ => return false,
            };
        // Constructors retain their own field destinations. Owning a record
        // subtree here would let a rejected element exclude those valid grants.
        if !has_scalar_array_element_shape(program, element_type) {
            return false;
        }
        let start = if range.start.is_valid() {
            static_window_bound(program, range.start)
        } else {
            WindowBound::Known(0)
        };
        let end = if range.end.is_valid() {
            match static_window_bound(program, range.end) {
                WindowBound::Known(end) if range.end_inclusive => end
                    .checked_add(1)
                    .map_or(WindowBound::Invalid, WindowBound::Known),
                bound => bound,
            }
        } else {
            length.map_or(WindowBound::Unknown, WindowBound::Known)
        };
        let (start, end) = match (start, end) {
            (WindowBound::Invalid, _) | (_, WindowBound::Invalid) => return false,
            (start, end) => (start.value(), end.value()),
        };
        if start.zip(end).is_some_and(|(start, end)| start > end)
            || end.zip(length).is_some_and(|(end, length)| end > length)
            || start
                .zip(length)
                .is_some_and(|(start, length)| start > length)
        {
            return false;
        }
        let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(value)
        else {
            return false;
        };
        let elements = program.expression_table.expression_handles(*elements);
        if start
            .zip(end)
            .is_some_and(|(start, end)| elements.len() != end - start)
        {
            return false;
        }
        for element in elements {
            if !admitted(element_type, *element) {
                other_elements.push(*element);
            }
        }
        return true;
    }

    let destination = crate::places::declared_place_type_raw(program, machine, Some(state), target)
        .or_else(|| {
            crate::places::declared_indexed_projection_type_raw(
                program,
                machine,
                Some(state),
                target,
            )
        });
    destination.is_some_and(|destination| {
        admitted(
            crate::places::assignment_value_type(program, destination),
            value,
        )
    })
}

enum WindowBound {
    Known(usize),
    Unknown,
    Invalid,
}

impl WindowBound {
    fn value(self) -> Option<usize> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unknown | Self::Invalid => None,
        }
    }
}

fn static_window_bound(program: &TypedTrees, expression: ExpressionHandle) -> WindowBound {
    let Some(expression) = crate::normalize_immutable_integer_bound_expression(program, expression)
    else {
        return WindowBound::Unknown;
    };
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Integer(_)
    ) {
        return WindowBound::Unknown;
    }
    crate::normalize_immutable_integer_bound_to_usize(program, expression)
        .map_or(WindowBound::Invalid, WindowBound::Known)
}

fn has_scalar_array_element_shape(
    program: &TypedTrees,
    mut element_type: TypeReferenceHandle,
) -> bool {
    use typed_trees::types::TypeReferenceNode;

    let mut visited = Vec::new();
    loop {
        if program.primitive_type_reference(element_type).is_some() {
            return true;
        }
        if visited.contains(&element_type) {
            return false;
        }
        visited.push(element_type);
        let TypeReferenceNode::FixedArray {
            element_type: nested_element,
            ..
        } = program.type_reference_table.type_reference(element_type)
        else {
            return false;
        };
        element_type = *nested_element;
    }
}

/// Results inherit their consumer's destination, not their dispatch inputs.
/// Each array element and match arm retains its own landing obligation. Failed
/// children remain exclusion roots even when their enclosing value is admitted.
pub(super) fn admit_result_values(
    program: &TypedTrees,
    destination: TypeReferenceHandle,
    expression: ExpressionHandle,
    admitted: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle) -> bool,
    other_elements: &mut Vec<ExpressionHandle>,
) -> bool {
    admit_result_values_inner(
        program,
        destination,
        expression,
        admitted,
        other_elements,
        &mut Vec::new(),
    )
}

fn admit_result_values_inner(
    program: &TypedTrees,
    destination: TypeReferenceHandle,
    expression: ExpressionHandle,
    admitted: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle) -> bool,
    other_elements: &mut Vec<ExpressionHandle>,
    active: &mut Vec<ExpressionHandle>,
) -> bool {
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    if !program.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return false;
    }
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) {
        let arms = program.expression_table.match_arms(dispatch.arms);
        if arms.is_empty() {
            return false;
        }
        active.push(expression);
        for arm in arms {
            if !admit_result_values_inner(
                program,
                destination,
                arm.value,
                admitted,
                other_elements,
                active,
            ) {
                other_elements.push(arm.value);
            }
        }
        active.pop();
        return true;
    }
    if let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(expression)
        && let TypeReferenceNode::FixedArray {
            element_type,
            length,
        } = program.type_reference_table.type_reference(destination)
        && has_scalar_array_element_shape(program, *element_type)
    {
        let elements = program.expression_table.expression_handles(*elements);
        if let FixedArrayLength::Literal(length) = length
            && elements.len() != *length
        {
            return false;
        }
        active.push(expression);
        for element in elements {
            if !admit_result_values_inner(
                program,
                *element_type,
                *element,
                admitted,
                other_elements,
                active,
            ) {
                other_elements.push(*element);
            }
        }
        active.pop();
        true
    } else {
        admitted(destination, expression)
    }
}
