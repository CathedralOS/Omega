use crate::parser::expression::{
    parse_expression_handle, parse_expression_handle_without_struct_literals,
};
use crate::parser::input::{Input, ParseResult};
use crate::parse_error::ParseError;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
    TableCallExpression,
};
use omega_syntax_trees::identifier::IdentifierPath;
use omega_syntax_trees::statement::{
    StatementHandle, StatementNode, TableTransition, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use omega_syntax_trees::SyntaxTrees;
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
        (ExpressionHandle::invalid(), input.take_punctuation(PunctuationKind::LeftBrace, "{")?)
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

        let statement = syntax_trees
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
    Ok((classify_transition_target_handle(syntax_trees, expression)?, rest))
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
        ExpressionNode::Name(path) => {
            let members = syntax_trees.expressions.identifier_path_members(path);
            if members.len() == 1 && members[0].as_str() == "self" {
                TransitionTargetNode::SelfTarget
            } else {
                TransitionTargetNode::Value(expression)
            }
        }
        _ => TransitionTargetNode::Value(expression),
    };

    Ok(syntax_trees.statements.insert_transition_target(target))
}

fn classify_call_target_handle(
    syntax_trees: &mut SyntaxTrees,
    call: TableCallExpression,
) -> Result<TransitionTargetNode, ParseError> {
    let receiver_path = if call.receiver.is_valid() {
        expression_handle_to_identifier_path(syntax_trees, call.receiver)
    } else {
        None
    };
    if receiver_path.as_ref().is_some_and(|path| path.len() > 1) {
        return Ok(TransitionTargetNode::Value(
            syntax_trees.expressions.insert(ExpressionNode::Call(call)),
        ));
    }
    let path = match receiver_path {
        None => IdentifierPath::from(vec![call.target.clone()]),
        Some(mut path) => {
            path.push(call.target.clone());
            path
        }
    };

    if path.len() == 1 && path.as_slice()[0].as_str() == "self" {
        Ok(TransitionTargetNode::SelfTarget)
    } else {
        let arguments = copy_expression_handles_to_statement_table(syntax_trees, call.arguments);
        Ok(TransitionTargetNode::Named {
            path: copy_identifier_path_to_statement_table(syntax_trees, &path),
            arguments,
        })
    }
}

fn expression_handle_to_identifier_path(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<IdentifierPath> {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Name(path) => Some(IdentifierPath::from(
            syntax_trees.expressions.identifier_path_members(*path).to_vec(),
        )),
        ExpressionNode::Member(member) => {
            let mut path = expression_handle_to_identifier_path(syntax_trees, member.receiver)?;
            path.push(member.member.clone());
            Some(path)
        }
        _ => None,
    }
}

fn copy_identifier_path_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    path: &IdentifierPath,
) -> HandleSpan<omega_syntax_trees::identifier::Identifier> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for member in path.iter() {
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(member.clone());
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
