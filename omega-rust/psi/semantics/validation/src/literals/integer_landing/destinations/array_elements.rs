use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

/// A selected range requests its collection's element type at the exact window
/// width. This discovers landing destinations only; ordinary write permissions
/// and bounds validation remain obligations of the assignment validators.
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
        let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(collection_type)
        else {
            return false;
        };
        // Constructors retain their own field destinations. Owning a record
        // subtree here would let a rejected element exclude those valid grants.
        if !has_scalar_array_element_shape(program, *element_type) {
            return false;
        }
        let start = if range.start.is_valid() {
            crate::normalize_immutable_integer_bound_to_usize(program, range.start)
        } else {
            Some(0)
        };
        let end =
            crate::normalize_immutable_integer_bound_to_usize(program, range.end).and_then(|end| {
                if range.end_inclusive {
                    end.checked_add(1)
                } else {
                    Some(end)
                }
            });
        let (Some(start), Some(end)) = (start, end) else {
            return false;
        };
        if start > end || end > *length {
            return false;
        }
        let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(value)
        else {
            return false;
        };
        let elements = program.expression_table.expression_handles(*elements);
        if elements.len() != end - start {
            return false;
        }
        for element in elements {
            if !admitted(*element_type, *element) {
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

fn has_scalar_array_element_shape(
    program: &TypedTrees,
    mut element_type: TypeReferenceHandle,
) -> bool {
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

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
            length: FixedArrayLength::Literal(_),
        } = program.type_reference_table.type_reference(element_type)
        else {
            return false;
        };
        element_type = *nested_element;
    }
}

/// A fixed scalar-array destination owns its literal tree, but every element
/// still has an independent landing obligation. Rejected elements remain roots
/// for the shared-node exclusion pass; admitting an array cannot bless a typed
/// or differently used large literal inside it.
pub(super) fn admit_array_elements(
    program: &TypedTrees,
    destination: TypeReferenceHandle,
    expression: ExpressionHandle,
    admitted: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle) -> bool,
    other_elements: &mut Vec<ExpressionHandle>,
) -> bool {
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    if let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(expression)
        && let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(destination)
        && (program.primitive_type_reference(*element_type).is_some()
            || matches!(
                program.type_reference_table.type_reference(*element_type),
                TypeReferenceNode::FixedArray { .. }
            ))
    {
        let elements = program.expression_table.expression_handles(*elements);
        if elements.len() != *length {
            return false;
        }
        for element in elements {
            if !admit_array_elements(program, *element_type, *element, admitted, other_elements) {
                other_elements.push(*element);
            }
        }
        true
    } else {
        admitted(destination, expression)
    }
}
