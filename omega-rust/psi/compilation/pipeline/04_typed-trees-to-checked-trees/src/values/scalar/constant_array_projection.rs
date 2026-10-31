//! A fixed projection of a closed constant array produces its existing scalar
//! leaf. The original typed indexing and declaration selections remain intact
//! for checking and source custody; this creates neither a place nor a borrow.
//! Every leaf must be a literal, since selecting one element cannot erase the
//! evaluation of an effectful or failing sibling constructor expression.

use super::structural_fields;
use crate::flow::expressions;
use checked_trees::CheckedOperatorFacts;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn selected_leaf(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut projections = Vec::new();
    let mut root = expression;
    while let Some(indexed) = program.expression_table.indexed_node(root) {
        projections.push((root, indexed.index));
        root = indexed.collection;
    }
    if projections.is_empty() {
        return None;
    }
    let mut reference = validation::declared_constant_array_type(program, root)?;
    if !closed_literal_array(program, root, reference) {
        return None;
    }
    let mut selected = root;
    for (projection, index) in projections.into_iter().rev() {
        let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let elements = program.expression_table.array_literal_node(selected)?;
        let literal = program.expression_table.integer_node(index)?;
        if literal.landing().is_some_and(|landing| {
            landing.landed_type == numerics::literals::LandedIntegerType::Addr
                || landing.domain != ArithmeticDomain::Exact
        }) || !structural_fields::indexed_read_is_builtin(
            program, operators, parameters, projection, reference, index,
        ) {
            return None;
        }
        let element_index = usize::try_from(literal.value_u64()?).ok()?;
        if element_index >= *length {
            return None;
        }
        selected = *expressions(program)
            .expression_handles(*elements)
            .get(element_index)?;
        reference = *element_type;
    }
    matches!(
        program.expression_table.expression(selected),
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_)
    )
    .then_some(selected)
}

fn closed_literal_array(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> bool {
    validation::closed_literal_array_elements(program, expression, reference).is_some()
}
