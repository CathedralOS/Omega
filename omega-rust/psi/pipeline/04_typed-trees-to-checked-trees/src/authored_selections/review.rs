use checked_trees::CheckFacts;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
use typed_trees::{TypedTrees, expression::ExpressionNode};

/// Rederive one exact compiler-owned collection view from the typed call and
/// the checked environments that own it.
pub(crate) fn derive_checked_collection_view_intrinsic(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    super::contexts::checked_collection_view_intrinsic_from_exact_owner(
        program, facts, expression, call,
    )
}
