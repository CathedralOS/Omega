use crate::parse_error::ParseError;
use crate::parser::expression::{
    parse_argument_list_after_open_paren_handle, parse_expression_handle,
};
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableMemberExpression,
};
use omega_syntax_trees::statement::{TransitionTargetHandle, TransitionTargetNode};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_transition_target_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TransitionTargetHandle> {
    let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
    Ok((
        classify_transition_target_handle(syntax_trees, expression)?,
        rest,
    ))
}

pub(super) fn parse_transition_block_target_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TransitionTargetHandle> {
    if input.at_keyword(KeywordKind::SelfValue) || input.at_name_like() {
        let (expression, rest) = parse_transition_target_expression_handle(syntax_trees, input)?;
        return Ok((
            classify_transition_target_handle(syntax_trees, expression)?,
            rest,
        ));
    }

    parse_transition_target_handle(syntax_trees, input)
}

fn classify_transition_target_handle(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Result<TransitionTargetHandle, ParseError> {
    let node = syntax_trees.expressions.expression(expression).clone();
    let target = match node {
        ExpressionNode::Call(call) => classify_call_target_handle(syntax_trees, call)?,
        ExpressionNode::SelfValue => TransitionTargetNode::SelfTarget,
        ExpressionNode::Name(_) => TransitionTargetNode::Value(expression),
        _ => TransitionTargetNode::Value(expression),
    };

    Ok(syntax_trees.statements.insert_transition_target(target))
}

fn parse_transition_target_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (mut expression, mut input) = if input.at_keyword(KeywordKind::SelfValue) {
        (
            syntax_trees.expressions.insert(ExpressionNode::SelfValue),
            input.take_keyword(KeywordKind::SelfValue, "self")?,
        )
    } else {
        let (path, rest) = parse_path_handle_span(input, |member| {
            syntax_trees
                .expressions
                .append_identifier_path_member(member)
        })?;
        (
            syntax_trees.expressions.insert(ExpressionNode::Name(path)),
            rest,
        )
    };

    while input.at_punctuation(PunctuationKind::Dot) {
        input = input.take_punctuation(PunctuationKind::Dot, ".")?;
        let (member, rest) = input.take_identifier()?;
        input = rest;
        expression =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: expression,
                    member,
                }));
    }

    if input.at_punctuation(PunctuationKind::LeftParen) {
        input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let (arguments, rest) = parse_argument_list_after_open_paren_handle(syntax_trees, input)?;
        input = rest;
        expression = match syntax_trees.expressions.expression(expression).clone() {
            ExpressionNode::Name(path) => {
                let members = syntax_trees.expressions.identifier_path_members(path);
                let target = members
                    .last()
                    .cloned()
                    .expect("call path should have at least one member");
                let receiver = if members.len() <= 1 {
                    ExpressionHandle::invalid()
                } else {
                    let receiver_path = syntax_trees
                        .expressions
                        .copy_identifier_path_prefix(path, members.len() - 1);
                    syntax_trees
                        .expressions
                        .insert(ExpressionNode::Name(receiver_path))
                };

                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
                        receiver,
                        target,
                        arguments,
                    }))
            }
            ExpressionNode::Member(member) => {
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
                        receiver: member.receiver,
                        target: member.member,
                        arguments,
                    }))
            }
            _ => unreachable!("transition target call base should be a path or member access"),
        };
    }

    Ok((expression, input))
}

fn classify_call_target_handle(
    syntax_trees: &mut SyntaxTrees,
    call: TableCallExpression,
) -> Result<TransitionTargetNode, ParseError> {
    let receiver_depth = if call.receiver.is_valid() {
        expression_handle_identifier_depth(syntax_trees, call.receiver)
    } else {
        Some(0)
    };
    let Some(receiver_depth) = receiver_depth else {
        return Ok(TransitionTargetNode::Value(
            syntax_trees.expressions.insert(ExpressionNode::Call(call)),
        ));
    };
    if receiver_depth > 1 {
        return Ok(TransitionTargetNode::Value(
            syntax_trees.expressions.insert(ExpressionNode::Call(call)),
        ));
    }
    let (path, path_starts_at_self) = if call.receiver.is_valid() {
        let receiver =
            copy_expression_identifier_path_to_statement_table(syntax_trees, call.receiver)
                .ok_or_else(|| ParseError::new("call target must be a path or member access"))?;
        (
            append_statement_identifier_path_member(
                syntax_trees,
                receiver.members,
                call.target.clone(),
            ),
            receiver.starts_at_self,
        )
    } else {
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(call.target.clone());
        (HandleSpan::from_parts(handle, 1), false)
    };

    if path_starts_at_self && path.count() == 1 {
        Ok(TransitionTargetNode::SelfTarget)
    } else {
        let arguments = copy_expression_handles_to_statement_table(syntax_trees, call.arguments);
        Ok(TransitionTargetNode::Named {
            path,
            path_starts_at_self,
            arguments,
        })
    }
}

fn expression_handle_identifier_depth(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<usize> {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Name(path) => Some(
            syntax_trees
                .expressions
                .identifier_path_members(*path)
                .len(),
        ),
        ExpressionNode::Member(member) => {
            let depth = expression_handle_identifier_depth(syntax_trees, member.receiver)?;
            Some(depth + 1)
        }
        ExpressionNode::SelfValue => Some(1),
        _ => None,
    }
}

struct StatementIdentifierPath {
    members: HandleSpan<omega_syntax_trees::identifier::Identifier>,
    starts_at_self: bool,
}

fn copy_expression_identifier_path_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<StatementIdentifierPath> {
    match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => Some(StatementIdentifierPath {
            members: copy_identifier_members_to_statement_table(syntax_trees, path),
            starts_at_self: false,
        }),
        ExpressionNode::SelfValue => {
            let self_member = syntax_trees.statements.append_identifier_path_member(
                omega_syntax_trees::identifier::Identifier::generated("self"),
            );
            Some(StatementIdentifierPath {
                members: HandleSpan::from_parts(self_member, 1),
                starts_at_self: true,
            })
        }
        ExpressionNode::Member(member) => {
            let mut receiver =
                copy_expression_identifier_path_to_statement_table(syntax_trees, member.receiver)?;
            receiver.members = append_statement_identifier_path_member(
                syntax_trees,
                receiver.members,
                member.member,
            );
            Some(receiver)
        }
        _ => None,
    }
}

fn copy_identifier_members_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<omega_syntax_trees::identifier::Identifier>,
) -> HandleSpan<omega_syntax_trees::identifier::Identifier> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    let member_count = syntax_trees.expressions.identifier_path_members(path).len();

    for index in 0..member_count {
        let member = syntax_trees.expressions.identifier_path_members(path)[index].clone();
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(member);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("transition target path span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn append_statement_identifier_path_member(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<omega_syntax_trees::identifier::Identifier>,
    member: omega_syntax_trees::identifier::Identifier,
) -> HandleSpan<omega_syntax_trees::identifier::Identifier> {
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

fn copy_expression_handles_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    arguments: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for argument in syntax_trees.expressions.expression_handles(arguments) {
        let handle = syntax_trees.statements.append_expression_handle(*argument);
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
