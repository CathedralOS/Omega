//! Admitted target-dependent scalar observations shared by flow and value capture.
//!
//! These are producer identities, never permission to replay a runtime call or
//! mutable place. Selection, invocation, result admission and child formation
//! remain obligations of the original occurrence.

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;

pub(crate) fn deferred_integer_observation(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    expression: ExpressionHandle,
    primitive: Option<PrimitiveType>,
) -> Option<checked_trees::IntegerRangeRecipe> {
    let admitted = match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            call.layout_query.is_some()
                || (primitive.is_some_and(|primitive| primitive.integer_bounds().is_some())
                    && program.constant_evaluation_entry(owner).is_some())
        }
        ExpressionNode::Name(path) => program
            .const_declaration_by_symbol(path.symbol)
            .is_some_and(|declaration| {
                declaration.evaluated_value.is_none()
                    && declaration.materialized_initializer.is_valid()
            }),
        _ => false,
    };
    admitted.then(|| checked_trees::IntegerRangeRecipe::observation(expression, owner))
}
