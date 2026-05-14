use crate::parser::expression::parse_expression_handle;
use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_reference_allowing_borrow;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableMemberExpression,
};
use omega_syntax_trees::statement::{StatementHandle, StatementNode, TableAssignment, TableCall, TableLocalData};
use omega_syntax_trees::SyntaxTrees;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    if input.at_keyword(KeywordKind::Let) {
        let input = input.take_keyword(KeywordKind::Let, "let")?;
        return parse_local_data_statement_handle(syntax_trees, input);
    }

    let (expression, input) = parse_expression_handle(syntax_trees, input)?;

    if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (value, input) = parse_expression_handle(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(TableAssignment {
                    target: expression,
                    value,
                })),
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::RightBrace) {
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Expression(expression)),
            input,
        ));
    }

    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if let Some(call) = expression_handle_to_statement_call(syntax_trees, expression) {
        Ok((
            syntax_trees.statements.insert(StatementNode::Call(call)),
            input,
        ))
    } else {
        Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Expression(expression)),
            input,
        ))
    }
}

fn parse_local_data_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let (name, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, input) = parse_type_reference_allowing_borrow(input)?;
    let (initial_value, input) = if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (expression, input) = parse_expression_handle(syntax_trees, input)?;
        (expression, input)
    } else {
        (ExpressionHandle::invalid(), input)
    };
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;

    let type_reference = syntax_trees
        .type_references
        .insert_tree(&type_reference, &mut syntax_trees.expressions);
    Ok((
        syntax_trees
            .statements
            .insert(StatementNode::LocalData(TableLocalData {
                name,
                type_reference,
                initial_value,
            })),
        input,
    ))
}

fn expression_handle_to_statement_call(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<TableCall> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression).clone() else {
        return None;
    };

    let (receiver, target) = split_expression_call_handle(syntax_trees, &call)?;
    Some(TableCall {
        receiver,
        target,
        arguments: call.arguments,
    })
}

fn split_expression_call_handle(
    syntax_trees: &mut SyntaxTrees,
    call: &TableCallExpression,
) -> Option<(HandleSpan<omega_syntax_trees::identifier::Identifier>, omega_syntax_trees::identifier::Identifier)> {
    let receiver = if call.receiver.is_valid() {
        expression_handle_to_identifier_path_span(syntax_trees, call.receiver)?
    } else {
        HandleSpan::empty()
    };

    Some((receiver, call.target.clone()))
}

fn expression_handle_to_identifier_path_span(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<HandleSpan<omega_syntax_trees::identifier::Identifier>> {
    match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => {
            let members = syntax_trees
                .expressions
                .identifier_path_members(path)
                .to_vec();
            Some(copy_expression_identifier_path_to_statement_table(
                syntax_trees,
                &members,
            ))
        }
        ExpressionNode::Member(member) => {
            let mut members = expression_handle_to_identifier_vec(syntax_trees, member.receiver)?;
            members.push(member.member);
            Some(copy_expression_identifier_path_to_statement_table(
                syntax_trees,
                &members,
            ))
        }
        _ => None,
    }
}

fn expression_handle_to_identifier_vec(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<Vec<omega_syntax_trees::identifier::Identifier>> {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Name(path) => Some(
            syntax_trees
                .expressions
                .identifier_path_members(*path)
                .to_vec(),
        ),
        ExpressionNode::Member(TableMemberExpression { receiver, member }) => {
            let mut members = expression_handle_to_identifier_vec(syntax_trees, *receiver)?;
            members.push(member.clone());
            Some(members)
        }
        _ => None,
    }
}

fn copy_expression_identifier_path_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    members: &[omega_syntax_trees::identifier::Identifier],
) -> HandleSpan<omega_syntax_trees::identifier::Identifier> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for member in members {
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(member.clone());
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("statement identifier path span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}
