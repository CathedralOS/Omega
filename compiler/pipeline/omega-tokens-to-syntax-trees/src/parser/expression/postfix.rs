use crate::parse_error::ParseError;
use crate::parser::context::ExpressionContext;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use omega_core::arena::HandleSpan;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableCastExpression,
    TableIndexedExpression, TableMemberExpression, TableRangeExpression,
};
use omega_tokens::{KeywordKind, PunctuationKind};

use super::primary::parse_primary_expression_handle;
use super::{parse_expression_handle, parse_expression_handle_in};

pub(in crate::parser) fn parse_argument_list_after_open_paren_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<ExpressionHandle>> {
    let mut arguments = Vec::new();

    if !input.at_punctuation(PunctuationKind::RightParen) {
        loop {
            let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
            arguments.push(expression);
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                if input.at_punctuation(PunctuationKind::RightParen) {
                    break;
                }
                continue;
            }

            break;
        }
    }

    input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let arguments = syntax_trees
        .expressions
        .insert_expression_handles(arguments);
    Ok((arguments, input))
}

pub(super) fn parse_postfix_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (mut expression, mut input) =
        parse_primary_expression_handle(syntax_trees, input, context)?;

    loop {
        if input.at_punctuation(PunctuationKind::LeftParen) {
            input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (arguments, rest) =
                parse_argument_list_after_open_paren_handle(syntax_trees, input)?;
            input = rest;
            expression = build_call_expression_handle(syntax_trees, expression, arguments)?;
            continue;
        }

        if input.at_punctuation(PunctuationKind::LeftBracket) {
            input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
            let (index, rest) = parse_index_or_range_expression_handle(syntax_trees, input)?;
            input = rest.take_punctuation(PunctuationKind::RightBracket, "]")?;
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Indexed(TableIndexedExpression {
                        collection: expression,
                        index,
                    }));
            continue;
        }

        if input.at_punctuation(PunctuationKind::Dot) {
            let after_dot = input.take_punctuation(PunctuationKind::Dot, ".")?;
            let (member, rest) = after_dot.take_identifier()?;

            // CONCURRENCY STAGE 1: `handle.join()` is the IDENTITY on the
            // handle. Under the synchronous-spawn desugar (see
            // `expression/spawn.rs`) the spawned call already ran to
            // completion at the spawn site and `Join<T>` erased to `T`, so
            // the handle IS the result. `join` is a reserved machine/state
            // name (rejected at definition sites), so this rewrite can never
            // capture a user method. Only the exact zero-argument call form
            // rewrites; `x.join` stays an ordinary member read.
            if member.as_str() == "join" && rest.at_punctuation(PunctuationKind::LeftParen) {
                let after_open = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
                if after_open.at_punctuation(PunctuationKind::RightParen) {
                    input = after_open.take_punctuation(PunctuationKind::RightParen, ")")?;
                    continue;
                }
                return Err(after_open.error_here(
                    "`join` takes no arguments: it returns the spawned call's completed result",
                ));
            }

            input = rest;
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Member(TableMemberExpression {
                        receiver: expression,
                        member,
                    }));
            continue;
        }

        if input.at_keyword(KeywordKind::As) {
            input = input.take_keyword(KeywordKind::As, "as")?;
            let (target_type, rest) = parse_path_handle_span(input, |member| {
                syntax_trees
                    .expressions
                    .append_identifier_path_member(member)
            })?;
            input = rest;
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Cast(TableCastExpression {
                        value: expression,
                        target_type,
                    }));
            continue;
        }

        break;
    }

    Ok((expression, input))
}

fn parse_index_or_range_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    // Leading `..` / `..=` branch: open-start range (`..b`, `..`, `..=b`).
    if let Some(end_inclusive) = range_separator(&input) {
        let input = take_range_separator(input, end_inclusive)?;
        let (end, input) = if input.at_punctuation(PunctuationKind::RightBracket) {
            // A trailing inclusive separator with no end expression is invalid:
            // `..=` does not name a normalizable end.
            if end_inclusive {
                return Err(ParseError::new(
                    "inclusive range `..=` requires an end expression",
                ));
            }
            (ExpressionHandle::invalid(), input)
        } else {
            let (end, input) =
                parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
            (end, input)
        };
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Range(TableRangeExpression {
                    start: ExpressionHandle::invalid(),
                    end,
                    end_inclusive,
                })),
            input,
        ));
    }

    let (start, input) =
        parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
    let Some(end_inclusive) = range_separator(&input) else {
        return Ok((start, input));
    };

    let input = take_range_separator(input, end_inclusive)?;
    let (end, input) = if input.at_punctuation(PunctuationKind::RightBracket) {
        // `start..=` with no end has no normalizable end bound: reject it.
        if end_inclusive {
            return Err(ParseError::new(
                "inclusive range `..=` requires an end expression",
            ));
        }
        (ExpressionHandle::invalid(), input)
    } else {
        let (end, input) =
            parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
        (end, input)
    };
    Ok((
        syntax_trees
            .expressions
            .insert(ExpressionNode::Range(TableRangeExpression {
                start,
                end,
                end_inclusive,
            })),
        input,
    ))
}

/// Returns `Some(end_inclusive)` if the input is positioned at a range separator
/// (`..` -> `Some(false)`, `..=` -> `Some(true)`), otherwise `None`.
fn range_separator(input: &Input<'_, '_>) -> Option<bool> {
    if input.at_punctuation(PunctuationKind::DotDotEqual) {
        Some(true)
    } else if input.at_punctuation(PunctuationKind::DotDot) {
        Some(false)
    } else {
        None
    }
}

fn take_range_separator<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    end_inclusive: bool,
) -> Result<Input<'tokens, 'source>, ParseError> {
    if end_inclusive {
        input.take_punctuation(PunctuationKind::DotDotEqual, "..=")
    } else {
        input.take_punctuation(PunctuationKind::DotDot, "..")
    }
}

fn build_call_expression_handle(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    arguments: HandleSpan<ExpressionHandle>,
) -> Result<ExpressionHandle, ParseError> {
    let expression = syntax_trees.expressions.expression(expression).clone();
    match expression {
        ExpressionNode::Name(path) => {
            let members = syntax_trees
                .tables
                .expressions
                .identifier_path_members(path)
                .to_vec();
            let target = members
                .last()
                .cloned()
                .ok_or_else(|| ParseError::new("missing call target"))?;
            let receiver = if members.len() <= 1 {
                ExpressionHandle::invalid()
            } else {
                let receiver_path = syntax_trees
                    .tables
                    .expressions
                    .copy_identifier_path_prefix(path, members.len() - 1);
                syntax_trees
                    .tables
                    .expressions
                    .insert(ExpressionNode::Name(receiver_path))
            };

            Ok(syntax_trees
                .tables
                .expressions
                .insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target,
                    arguments,
                })))
        }
        ExpressionNode::Member(member) => {
            Ok(syntax_trees
                .expressions
                .insert(ExpressionNode::Call(TableCallExpression {
                    receiver: member.receiver,
                    target: member.member,
                    arguments,
                })))
        }
        _ => Err(ParseError::new(
            "call target must be a path or member access",
        )),
    }
}
