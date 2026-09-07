use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;

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
