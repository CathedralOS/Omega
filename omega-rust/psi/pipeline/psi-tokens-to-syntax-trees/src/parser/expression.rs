use crate::parser::context::ExpressionContext;
use crate::parser::input::{Input, ParseResult};
use psi_numerics::literals::IntegerLiteral;
use psi_source::SourceText;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableUnaryExpression,
    UnaryOperator,
};
use psi_tokens::{KeywordKind, PunctuationKind};

mod membership;
mod postfix;
mod primary;

use membership::parse_membership_expression_handle;
pub(in crate::parser) use postfix::memory_ordering_from_expression;
pub(super) use postfix::parse_argument_list_after_open_paren_handle;
use postfix::parse_postfix_expression_handle;
pub(in crate::parser) use postfix::try_parse_static_symbol_application;

pub(super) fn parse_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)
}

pub(super) fn parse_expression_handle_without_struct_literals<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_expression_handle_in(syntax_trees, input, ExpressionContext::NoStructLiteral)
}

pub(super) fn parse_expression_handle_without_struct_literals_or_membership<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_expression_handle_in(
        syntax_trees,
        input,
        ExpressionContext::NoStructLiteralOrMembership,
    )
}

/// Parse the operator subset that can appear in a const generic argument
/// without consuming the closing `>` as a comparison. This intentionally
/// stops above the comparison/equality/logical layers while retaining ordinary
/// integer precedence, shifts, bitwise operators, unary syntax, and grouped
/// subexpressions.
pub(super) fn parse_const_integer_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let outer_depth = input.depth();
    let input = input.deepen()?;
    let (handle, rest) = parse_bitwise_or_expression_handle(
        syntax_trees,
        input,
        ExpressionContext::NoStructLiteral,
    )?;
    Ok((handle, rest.with_depth(outer_depth)))
}

fn parse_expression_handle_in<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    // Bound recursion depth here: every nested expression (parenthesized group,
    // call argument, index) re-enters through this single choke point. Deepen on
    // entry to catch pathological nesting before it overflows the stack, then
    // restore the caller's depth on exit so a flat run of siblings (args, binary
    // operands) does not accumulate toward the limit.
    let outer_depth = input.depth();
    let input = input.deepen()?;
    let (handle, rest) = parse_or_expression_handle(syntax_trees, input, context)?;
    Ok((handle, rest.with_depth(outer_depth)))
}

// Membership separates ordinary logical/comparison operators from integer
// operators. Keep those grammar entrypoints distinct: an operator after a domain
// path must not silently become a new integer operand of that membership.
fn parse_or_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_membership_expression_handle,
        &[
            (PunctuationKind::PipePipe, BinaryOperator::Or, 0),
            (PunctuationKind::AndAnd, BinaryOperator::And, 1),
            (PunctuationKind::EqualEqual, BinaryOperator::Equal, 2),
            (
                PunctuationKind::ExclamationEqual,
                BinaryOperator::NotEqual,
                2,
            ),
            (PunctuationKind::LessEqual, BinaryOperator::LessOrEqual, 3),
            (
                PunctuationKind::GreaterEqual,
                BinaryOperator::GreaterOrEqual,
                3,
            ),
            (PunctuationKind::Less, BinaryOperator::Less, 3),
            (PunctuationKind::Greater, BinaryOperator::Greater, 3),
        ],
    )
}

pub(super) fn parse_bitwise_or_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_unary_expression_handle,
        &[
            (PunctuationKind::Pipe, BinaryOperator::BitwiseOr, 0),
            (PunctuationKind::Caret, BinaryOperator::BitwiseXor, 1),
            (PunctuationKind::Ampersand, BinaryOperator::BitwiseAnd, 2),
            (PunctuationKind::LessLess, BinaryOperator::ShiftLeft, 3),
            (
                PunctuationKind::GreaterGreater,
                BinaryOperator::ShiftRight,
                3,
            ),
            (PunctuationKind::Plus, BinaryOperator::Add, 4),
            (PunctuationKind::Minus, BinaryOperator::Subtract, 4),
            (PunctuationKind::Asterisk, BinaryOperator::Multiply, 5),
            (PunctuationKind::Slash, BinaryOperator::Divide, 5),
            (PunctuationKind::Percent, BinaryOperator::Modulo, 5),
        ],
    )
}

fn parse_binary_chain_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
    lower: fn(
        &mut SyntaxTrees,
        Input<'tokens, 'source>,
        ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle>,
    operators: &[(PunctuationKind, BinaryOperator, u8)],
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (mut expression, mut input) = lower(syntax_trees, input, context)?;
    let mut pending = Vec::new();

    while let Some((punctuation, operator, precedence)) = operators
        .iter()
        .find(|(punctuation, _, _)| input.at_punctuation(*punctuation))
        .copied()
    {
        while pending
            .last()
            .is_some_and(|(_, _, pending_precedence, _)| *pending_precedence >= precedence)
        {
            let (left, operator, _, span) = pending.pop().expect("pending operator");
            expression = insert_binary_expression(syntax_trees, left, operator, expression, span);
        }
        let operator_span = input.current_source_span();
        input = input.take_punctuation(punctuation, punctuation_label(punctuation))?;
        pending.push((expression, operator, precedence, operator_span));
        (expression, input) = lower(syntax_trees, input, context)?;
    }

    while let Some((left, operator, _, span)) = pending.pop() {
        expression = insert_binary_expression(syntax_trees, left, operator, expression, span);
    }
    Ok((expression, input))
}

fn insert_binary_expression(
    syntax_trees: &mut SyntaxTrees,
    left: ExpressionHandle,
    operator: BinaryOperator,
    right: ExpressionHandle,
    span: psi_source::SourceSpan,
) -> ExpressionHandle {
    let expression =
        syntax_trees
            .expressions
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator,
                right,
            }));
    syntax_trees.expressions.set_source_span(expression, span);
    expression
}

enum UnaryPrefix<'tokens, 'source> {
    Borrow(psi_language_core::ReferenceAccess),
    Operator(UnaryOperator, psi_source::SourceSpan),
    Negate(psi_source::SourceSpan),
    Acknowledge {
        input: Input<'tokens, 'source>,
        suspend: bool,
        block: bool,
    },
    Move,
}

fn parse_unary_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let mut input = input;
    let mut prefixes = Vec::new();
    while let Some((prefix, rest)) = take_unary_prefix(input)? {
        prefixes.push(prefix);
        input = rest;
    }
    let (mut expression, rest) = parse_postfix_expression_handle(syntax_trees, input, context)?;
    while let Some(prefix) = prefixes.pop() {
        expression = apply_unary_prefix(syntax_trees, expression, prefix)?;
    }
    Ok((expression, rest))
}

fn take_unary_prefix<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> Result<Option<(UnaryPrefix<'tokens, 'source>, Input<'tokens, 'source>)>, crate::ParseError> {
    // Acknowledgements remain ordinary identifiers when followed by `(`.
    if input.at_contextual("suspend")
        && input
            .take_contextual("suspend")
            .is_ok_and(|rest| !rest.at_punctuation(PunctuationKind::LeftParen))
    {
        let input = input.take_contextual("suspend")?;
        if input.at_contextual("suspend") {
            return Err(
                input.error_here("duplicate `suspend` call acknowledgement; write it exactly once")
            );
        }
        let (block, input) = if input.at_contextual("block") {
            (true, input.take_contextual("block")?)
        } else {
            (false, input)
        };
        return Ok(Some((
            UnaryPrefix::Acknowledge {
                input,
                suspend: true,
                block,
            },
            input,
        )));
    }
    if input.at_contextual("block")
        && input
            .take_contextual("block")
            .is_ok_and(|rest| !rest.at_punctuation(PunctuationKind::LeftParen))
    {
        let input = input.take_contextual("block")?;
        if input.at_contextual("suspend") {
            return Err(input.error_here(
                "call acknowledgements use canonical order `suspend block`, never `block suspend`",
            ));
        }
        if input.at_contextual("block") {
            return Err(
                input.error_here("duplicate `block` call acknowledgement; write it exactly once")
            );
        }
        return Ok(Some((
            UnaryPrefix::Acknowledge {
                input,
                suspend: false,
                block: true,
            },
            input,
        )));
    }
    if input.at_punctuation(PunctuationKind::Ampersand) {
        let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
        let (access, input) = if input.at_contextual("mut") {
            (
                psi_language_core::ReferenceAccess::Mutable,
                input.take_contextual("mut")?,
            )
        } else if input.at_contextual("write") {
            (
                psi_language_core::ReferenceAccess::WriteOnly,
                input.take_contextual("write")?,
            )
        } else if input.at_keyword(KeywordKind::State) {
            (
                psi_language_core::ReferenceAccess::Mutable,
                input.take_keyword(KeywordKind::State, "state")?,
            )
        } else {
            (psi_language_core::ReferenceAccess::Shared, input)
        };
        return Ok(Some((UnaryPrefix::Borrow(access), input)));
    }
    for (punctuation, operator, label) in [
        (PunctuationKind::Exclamation, UnaryOperator::LogicalNot, "!"),
        (PunctuationKind::Tilde, UnaryOperator::BitwiseNot, "~"),
    ] {
        if input.at_punctuation(punctuation) {
            return Ok(Some((
                UnaryPrefix::Operator(operator, input.current_source_span()),
                input.take_punctuation(punctuation, label)?,
            )));
        }
    }
    if input.at_punctuation(PunctuationKind::Minus) {
        return Ok(Some((
            UnaryPrefix::Negate(input.current_source_span()),
            input.take_punctuation(PunctuationKind::Minus, "-")?,
        )));
    }
    if input.at_contextual("move") {
        return Ok(Some((UnaryPrefix::Move, input.take_contextual("move")?)));
    }
    Ok(None)
}

fn apply_unary_prefix(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    prefix: UnaryPrefix<'_, '_>,
) -> Result<ExpressionHandle, crate::ParseError> {
    match prefix {
        UnaryPrefix::Borrow(access) => Ok(syntax_trees.expressions.insert(ExpressionNode::Borrow(
            psi_syntax_trees::expression::TableBorrowExpression {
                target: expression,
                access,
            },
        ))),
        UnaryPrefix::Operator(operator, span) => {
            let expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Unary(TableUnaryExpression {
                        operator,
                        operand: expression,
                    }));
            syntax_trees.expressions.set_source_span(expression, span);
            Ok(expression)
        }
        UnaryPrefix::Negate(span) => {
            // Preserve literal folding and the existing 0 - value representation.
            let negated = match syntax_trees.expressions.expression(expression).clone() {
                ExpressionNode::Integer(literal) => ExpressionNode::Integer(literal.negated()),
                ExpressionNode::Float(text) => {
                    ExpressionNode::Float(SourceText::generated(format!("-{}", text.as_str())))
                }
                _ => {
                    let zero = syntax_trees
                        .expressions
                        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
                    ExpressionNode::Binary(TableBinaryExpression {
                        left: zero,
                        operator: BinaryOperator::Subtract,
                        right: expression,
                    })
                }
            };
            let expression = syntax_trees.expressions.insert(negated);
            syntax_trees.expressions.set_source_span(expression, span);
            Ok(expression)
        }
        UnaryPrefix::Acknowledge {
            input,
            suspend,
            block,
        } => {
            let ExpressionNode::Call(call) =
                syntax_trees.expressions.expression(expression).clone()
            else {
                return Err(input.error_here(if suspend {
                    "`suspend` is a call acknowledgement and must appear immediately before a call"
                } else {
                    "`block` is a call acknowledgement and must appear immediately before a call"
                }));
            };
            syntax_trees.expressions.replace_expression(
                expression,
                ExpressionNode::Call(psi_syntax_trees::expression::TableCallExpression {
                    operational_acknowledgement:
                        psi_language_core::CallOperationalAcknowledgement {
                            acknowledges_suspend: suspend,
                            acknowledges_block: block,
                            ..Default::default()
                        },
                    ..call
                }),
            );
            Ok(expression)
        }
        // Ownership is inferred from value flow; explicit `move` has no extra node.
        UnaryPrefix::Move => Ok(expression),
    }
}

fn punctuation_label(punctuation: PunctuationKind) -> &'static str {
    match punctuation {
        PunctuationKind::PipePipe => "||",
        PunctuationKind::AndAnd => "&&",
        PunctuationKind::EqualEqual => "==",
        PunctuationKind::ExclamationEqual => "!=",
        PunctuationKind::LessEqual => "<=",
        PunctuationKind::GreaterEqual => ">=",
        PunctuationKind::Less => "<",
        PunctuationKind::Greater => ">",
        PunctuationKind::LessLess => "<<",
        PunctuationKind::GreaterGreater => ">>",
        PunctuationKind::Plus => "+",
        PunctuationKind::Minus => "-",
        PunctuationKind::Asterisk => "*",
        PunctuationKind::Slash => "/",
        PunctuationKind::Percent => "%",
        _ => "?",
    }
}
