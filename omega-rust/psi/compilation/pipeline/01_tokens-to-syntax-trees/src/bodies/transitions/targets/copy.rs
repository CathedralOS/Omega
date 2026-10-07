use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn copy_expression_identifier_path_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<HandleSpan<syntax_trees::identifier::Identifier>> {
    match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => Some(copy_identifier_members_to_statement_table(
            syntax_trees,
            path,
        )),
        ExpressionNode::Member(member) => {
            let receiver =
                copy_expression_identifier_path_to_statement_table(syntax_trees, member.receiver)?;
            Some(append_statement_identifier_path_member(
                syntax_trees,
                receiver,
                member.member,
            ))
        }
        _ => None,
    }
}

fn copy_identifier_members_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<syntax_trees::identifier::Identifier>,
) -> HandleSpan<syntax_trees::identifier::Identifier> {
    crate::bodies::copy_identifier_path_to_statement_table(
        syntax_trees,
        path,
        "transition target path span count overflow",
    )
}

pub(super) fn append_statement_identifier_path_member(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<syntax_trees::identifier::Identifier>,
    member: syntax_trees::identifier::Identifier,
) -> HandleSpan<syntax_trees::identifier::Identifier> {
    let handle = syntax_trees
        .statements
        .append_identifier_path_member(member);

    if path.is_empty() {
        HandleSpan::from_parts(handle, 1)
    } else {
        HandleSpan::from_parts(
            path.start(),
            path.count()
                .checked_add(1)
                .expect("transition target path span count overflow"),
        )
    }
}

pub(super) fn copy_expression_handles_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    arguments: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    let arguments = syntax_trees
        .tables
        .expressions
        .expression_handles(arguments)
        .to_vec();

    for argument in arguments {
        let handle = syntax_trees
            .tables
            .statements
            .append_expression_handle(argument);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("transition target argument span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}
