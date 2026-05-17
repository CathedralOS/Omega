use crate::parse_error::ParseError;
use crate::parser::expression::{
    parse_expression_handle, parse_expression_handle_without_struct_literals,
};
use crate::parser::input::{Input, ParseResult};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCallExpression,
};
use omega_syntax_trees::statement::{
    StatementHandle, StatementNode, TableTransition, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_transition_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let (target, mut input) = parse_transition_target_handle(syntax_trees, input)?;
    let continuation;
    if input.at_punctuation(PunctuationKind::Arrow) {
        input = input.take_punctuation(PunctuationKind::Arrow, "->")?;
        let (next_target, rest) = parse_transition_target_handle(syntax_trees, input)?;
        continuation = next_target;
        input = rest;
    } else {
        continuation = TransitionTargetHandle::invalid();
    }

    let guard;
    if input.at_keyword(KeywordKind::When) || input.at_contextual("when") {
        let input = if input.at_keyword(KeywordKind::When) {
            input.take_keyword(KeywordKind::When, "when")?
        } else {
            input.take_contextual("when")?
        };
        let (expression, rest) =
            parse_expression_handle_without_struct_literals(syntax_trees, input)?;
        let input = if rest.at_punctuation(PunctuationKind::Semicolon) {
            rest.take_punctuation(PunctuationKind::Semicolon, ";")?
        } else {
            rest
        };
        guard = TransitionGuardNode::When(expression);
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation,
                    guard,
                })),
            input,
        ));
    }

    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if continuation == TransitionTargetHandle::invalid() {
        if let TransitionTargetNode::Value(expression) =
            syntax_trees.statements.transition_target(target).clone()
        {
            return Ok((
                syntax_trees
                    .statements
                    .insert(StatementNode::Expression(expression)),
                input,
            ));
        }
    }

    Ok((
        syntax_trees
            .statements
            .insert(StatementNode::Transition(TableTransition {
                target,
                continuation,
                guard: TransitionGuardNode::Always,
            })),
        input,
    ))
}

pub(super) fn parse_transition_block_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let (subject, mut input) = if input.at_punctuation(PunctuationKind::LeftBrace) {
        (
            ExpressionHandle::invalid(),
            input.take_punctuation(PunctuationKind::LeftBrace, "{")?,
        )
    } else {
        let (expression, rest) = parse_expression_handle_until_punctuation(
            syntax_trees,
            input,
            PunctuationKind::LeftBrace,
        )?;
        let input = rest.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        (expression, input)
    };

    let mut start = Handle::invalid();
    let mut count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (guard, rest) = if input.at_contextual("_") {
            (TransitionGuardNode::Always, input.take_contextual("_")?)
        } else {
            let (pattern, rest) =
                parse_expression_handle_without_struct_literals(syntax_trees, input)?;
            if subject.is_valid() {
                (
                    TransitionGuardNode::When(syntax_trees.expressions.insert(
                        ExpressionNode::Binary(TableBinaryExpression {
                            left: subject,
                            operator: BinaryOperator::Equal,
                            right: pattern,
                        }),
                    )),
                    rest,
                )
            } else {
                (TransitionGuardNode::When(pattern), rest)
            }
        };
        input = rest.take_punctuation(PunctuationKind::Arrow, "->")?;

        let (target, rest) = if input.at_punctuation(PunctuationKind::LeftBrace) {
            let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
            let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
            (
                syntax_trees
                    .statements
                    .insert_transition_target(TransitionTargetNode::Terminal),
                input,
            )
        } else {
            parse_transition_target_handle(syntax_trees, input)?
        };
        input = rest;

        let statement =
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: TransitionTargetHandle::invalid(),
                    guard,
                }));
        let handle = syntax_trees.items.append_statement_handle(statement);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("transition block statement span count overflow");
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let statements = if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    };
    Ok((statements, input))
}

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

fn parse_expression_handle_until_punctuation<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    delimiter: PunctuationKind,
) -> Result<(ExpressionHandle, Input<'tokens, 'source>), ParseError> {
    let (expression_input, rest) =
        input.split_at_top_level_punctuation(delimiter, "expected transition block delimiter")?;
    let (expression, rest_after_expression) =
        parse_expression_handle_without_struct_literals(syntax_trees, expression_input)?;

    if !rest_after_expression.tokens.is_empty() {
        return Err(rest_after_expression.error_here("expected transition subject expression"));
    }

    Ok((expression, rest))
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
