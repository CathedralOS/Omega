use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;

pub(super) fn copy_expression_identifier_path_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<HandleSpan<syntax_trees::identifier::Identifier>> {
    crate::bodies::copy_expression_to_statement_identifier_path(
        syntax_trees,
        expression,
        "transition target path span count overflow",
    )
}

pub(super) fn append_statement_identifier_path_member(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<syntax_trees::identifier::Identifier>,
    member: syntax_trees::identifier::Identifier,
) -> HandleSpan<syntax_trees::identifier::Identifier> {
    crate::bodies::append_identifier_path_member_to_statement_table(
        syntax_trees,
        path,
        member,
        "transition target path span count overflow",
    )
}

pub(super) fn copy_expression_handles_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    arguments: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    crate::bodies::copy_expression_handles_to_statement_table(
        syntax_trees,
        arguments,
        "transition target argument span count overflow",
    )
}
