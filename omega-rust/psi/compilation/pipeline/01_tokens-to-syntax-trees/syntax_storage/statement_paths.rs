use crate::syntax_storage::tables::stmts;
pub(crate) fn copy_identifier_path_to_statement_table(
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
) -> arena::HandleSpan<syntax_trees::identifier::Identifier> {
    let mut copy = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();

    let member_count = syntax_trees.expressions.identifier_path_members(path).len();

    for index in 0..member_count {
        let member = syntax_trees.expressions.identifier_path_members(path)[index].clone();
        copy.push(stmts(syntax_trees).append_identifier_path_member(member));
    }

    copy.finish()
}

pub(crate) fn append_identifier_path_member_to_statement_table(
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
    member: syntax_trees::identifier::Identifier,
) -> arena::HandleSpan<syntax_trees::identifier::Identifier> {
    let handle = syntax_trees
        .statements
        .append_identifier_path_member(member);

    if path.is_empty() {
        arena::HandleSpan::from_parts(handle, 1)
    } else {
        arena::HandleSpan::from_parts(
            path.start(),
            path.count()
                .checked_add(1)
                .expect("handle span count overflow"),
        )
    }
}

pub(crate) fn copy_expression_handles_to_statement_table(
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    arguments: arena::HandleSpan<syntax_trees::expression::ExpressionHandle>,
) -> arena::HandleSpan<syntax_trees::expression::ExpressionHandle> {
    let mut copy = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();

    let arguments = syntax_trees
        .tables
        .expressions
        .expression_handles(arguments)
        .to_vec();

    for argument in arguments {
        copy.push(
            syntax_trees
                .tables
                .statements
                .append_expression_handle(argument),
        );
    }

    copy.finish()
}

pub(crate) fn copy_expression_to_statement_identifier_path(
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    expression: syntax_trees::expression::ExpressionHandle,
) -> Option<arena::HandleSpan<syntax_trees::identifier::Identifier>> {
    use syntax_trees::expression::ExpressionNode;
    match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => Some(copy_identifier_path_to_statement_table(
            syntax_trees,
            path.to_owned(),
        )),
        ExpressionNode::Member(member) => {
            let receiver =
                copy_expression_to_statement_identifier_path(syntax_trees, member.receiver)?;
            Some(append_identifier_path_member_to_statement_table(
                syntax_trees,
                receiver,
                member.member,
            ))
        }
        _ => None,
    }
}
