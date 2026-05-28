use crate::parse_error::ParseError;
use crate::parser::expression::parse_expression_handle_without_struct_literals;
use crate::parser::input::{Input, ParseResult};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use omega_syntax_trees::statement::{
    StatementHandle, StatementNode, TableTransition, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use omega_tokens::PunctuationKind;

mod targets;

use targets::parse_transition_block_target_handle;

pub(super) fn parse_transition_block_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let (subject, mut input) = if input.at_punctuation(PunctuationKind::LeftBrace) {
        (
            Vec::new(),
            input.take_punctuation(PunctuationKind::LeftBrace, "{")?,
        )
    } else {
        let (expressions, rest) = parse_expression_list_until_punctuation(
            syntax_trees,
            input,
            PunctuationKind::LeftBrace,
        )?;
        let input = rest.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        (expressions, input)
    };

    let mut start = Handle::invalid();
    let mut count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (guard, rest) = parse_transition_guard_node(syntax_trees, input, &subject)?;
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
            parse_transition_block_target_handle(syntax_trees, input)?
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

fn parse_expression_list_until_punctuation<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    delimiter: PunctuationKind,
) -> Result<(Vec<ExpressionHandle>, Input<'tokens, 'source>), ParseError> {
    let (expression_input, rest) =
        input.split_at_top_level_punctuation(delimiter, "expected transition block delimiter")?;
    let (expressions, rest_after_expression) =
        parse_transition_expression_list(syntax_trees, expression_input)?;

    if !rest_after_expression.tokens.is_empty() {
        return Err(rest_after_expression.error_here("expected transition subject expression"));
    }

    Ok((expressions, rest))
}

fn parse_transition_guard_node<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    subject: &[ExpressionHandle],
) -> Result<(TransitionGuardNode, Input<'tokens, 'source>), ParseError> {
    let (pattern_input, rest) =
        input.split_at_top_level_punctuation(PunctuationKind::Arrow, "expected `->`")?;
    let (patterns, pattern_rest) = parse_transition_pattern_list(syntax_trees, pattern_input)?;
    if !pattern_rest.tokens.is_empty() {
        return Err(pattern_rest.error_here("expected transition pattern"));
    }

    if subject.is_empty() {
        if patterns.len() == 1 {
            let guard = match patterns.into_iter().next().flatten() {
                Some(expression) => TransitionGuardNode::When(expression),
                None => TransitionGuardNode::Always,
            };
            return Ok((guard, rest));
        }
        return Err(input.error_here("anonymous transition blocks do not support tuple patterns"));
    }

    if patterns.len() == 1 && patterns[0].is_none() {
        return Ok((TransitionGuardNode::Always, rest));
    }

    if subject.len() != patterns.len() {
        return Err(input.error_here(format!(
            "transition pattern arity {} does not match subject arity {}",
            patterns.len(),
            subject.len()
        )));
    }

    let mut combined = ExpressionHandle::invalid();
    for (left, right) in subject.iter().copied().zip(patterns.into_iter()) {
        let Some(right) = right else {
            continue;
        };
        let equality =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: BinaryOperator::Equal,
                    right,
                }));
        combined = if combined.is_valid() {
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: combined,
                    operator: BinaryOperator::And,
                    right: equality,
                }))
        } else {
            equality
        };
    }

    Ok((
        if combined.is_valid() {
            TransitionGuardNode::When(combined)
        } else {
            TransitionGuardNode::Always
        },
        rest,
    ))
}

fn parse_transition_expression_list<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<ExpressionHandle>> {
    parse_transition_match_list(syntax_trees, input, false)
}

fn parse_transition_pattern_list<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Vec<Option<ExpressionHandle>>> {
    parse_transition_match_list(syntax_trees, input, true)
}

fn parse_transition_match_list<'tokens, 'source, T>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    allow_wildcard: bool,
) -> ParseResult<'tokens, 'source, Vec<T>>
where
    T: FromTransitionMatchComponent,
{
    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let (values, input) =
            parse_transition_match_list_after_open_paren(syntax_trees, input, allow_wildcard)?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        Ok((values, input))
    } else {
        let (value, input) = parse_transition_match_component(syntax_trees, input, allow_wildcard)?;
        Ok((vec![value], input))
    }
}

fn parse_transition_match_list_after_open_paren<'tokens, 'source, T>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    allow_wildcard: bool,
) -> ParseResult<'tokens, 'source, Vec<T>>
where
    T: FromTransitionMatchComponent,
{
    let mut values = Vec::new();
    while !input.at_punctuation(PunctuationKind::RightParen) {
        let (value, rest) = parse_transition_match_component(syntax_trees, input, allow_wildcard)?;
        values.push(value);
        input = rest;
        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else {
            break;
        }
    }
    Ok((values, input))
}

fn parse_transition_match_component<'tokens, 'source, T>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    allow_wildcard: bool,
) -> ParseResult<'tokens, 'source, T>
where
    T: FromTransitionMatchComponent,
{
    if allow_wildcard && input.at_contextual("_") {
        let input = input.take_contextual("_")?;
        return Ok((T::from_wildcard(), input));
    }

    let (expression, input) = parse_expression_handle_without_struct_literals(syntax_trees, input)?;
    Ok((T::from_expression(expression), input))
}

trait FromTransitionMatchComponent {
    fn from_expression(expression: ExpressionHandle) -> Self;
    fn from_wildcard() -> Self;
}

impl FromTransitionMatchComponent for ExpressionHandle {
    fn from_expression(expression: ExpressionHandle) -> Self {
        expression
    }

    fn from_wildcard() -> Self {
        ExpressionHandle::invalid()
    }
}

impl FromTransitionMatchComponent for Option<ExpressionHandle> {
    fn from_expression(expression: ExpressionHandle) -> Self {
        Some(expression)
    }

    fn from_wildcard() -> Self {
        None
    }
}
