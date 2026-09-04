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

fn parse_or_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_and_expression_handle,
        &[(PunctuationKind::PipePipe, BinaryOperator::Or)],
    )
}

fn parse_and_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_equality_expression_handle,
        &[(PunctuationKind::AndAnd, BinaryOperator::And)],
    )
}

fn parse_equality_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_comparison_expression_handle,
        &[
            (PunctuationKind::EqualEqual, BinaryOperator::Equal),
            (PunctuationKind::ExclamationEqual, BinaryOperator::NotEqual),
        ],
    )
}

fn parse_comparison_expression_handle<'tokens, 'source>(
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
            (PunctuationKind::LessEqual, BinaryOperator::LessOrEqual),
            (
                PunctuationKind::GreaterEqual,
                BinaryOperator::GreaterOrEqual,
            ),
            (PunctuationKind::Less, BinaryOperator::Less),
            (PunctuationKind::Greater, BinaryOperator::Greater),
        ],
    )
}

// Bitwise operators sit between comparison/membership and the shifts (Rust-style
// precedence: `|` < `^` < `&` < `<<`/`>>`, and all tighter than `==`). Infix `&`
// is disambiguated from a prefix reference by position -- the binary chain only
// consumes `&` after a left operand; prefix `&x` / `&[u8]` are parsed by the unary
// layer below.
pub(super) fn parse_bitwise_or_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_bitwise_xor_expression_handle,
        &[(PunctuationKind::Pipe, BinaryOperator::BitwiseOr)],
    )
}

fn parse_bitwise_xor_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_bitwise_and_expression_handle,
        &[(PunctuationKind::Caret, BinaryOperator::BitwiseXor)],
    )
}

fn parse_bitwise_and_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_shift_expression_handle,
        &[(PunctuationKind::Ampersand, BinaryOperator::BitwiseAnd)],
    )
}

pub(super) fn parse_shift_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_add_expression_handle,
        &[
            (PunctuationKind::LessLess, BinaryOperator::ShiftLeft),
            (PunctuationKind::GreaterGreater, BinaryOperator::ShiftRight),
        ],
    )
}

fn parse_add_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    parse_binary_chain_handle(
        syntax_trees,
        input,
        context,
        parse_multiply_expression_handle,
        &[
            (PunctuationKind::Plus, BinaryOperator::Add),
            (PunctuationKind::Minus, BinaryOperator::Subtract),
        ],
    )
}

fn parse_multiply_expression_handle<'tokens, 'source>(
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
            (PunctuationKind::Asterisk, BinaryOperator::Multiply),
            (PunctuationKind::Slash, BinaryOperator::Divide),
            (PunctuationKind::Percent, BinaryOperator::Modulo),
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
    operators: &[(PunctuationKind, BinaryOperator)],
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (mut expression, mut input) = lower(syntax_trees, input, context)?;

    loop {
        let Some((punctuation, operator)) = operators
            .iter()
            .find(|(punctuation, _)| input.at_punctuation(*punctuation))
            .copied()
        else {
            break;
        };

        let operator_span = input
            .tokens
            .first()
            .map(|token| input.source_span(token))
            .expect("recognized binary punctuation has a source token");
        input = input.take_punctuation(punctuation, punctuation_label(punctuation))?;
        let (right, rest) = lower(syntax_trees, input, context)?;
        input = rest;
        expression =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: expression,
                    operator,
                    right,
                }));
        syntax_trees
            .expressions
            .set_source_span(expression, operator_span);
    }

    Ok((expression, input))
}

fn parse_unary_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    // CALLACK: `suspend` and `block` remain contextual identifiers except
    // when followed by another expression rather than `(` (so declarations
    // may still name an ordinary `suspend()` or `block()` operation).
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
        let (acknowledges_block, input) = if input.at_contextual("block") {
            (true, input.take_contextual("block")?)
        } else {
            (false, input)
        };
        let (expression, rest) = parse_unary_expression_handle(syntax_trees, input, context)?;
        let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression).clone()
        else {
            return Err(input.error_here(
                "`suspend` is a call acknowledgement and must appear immediately before a call",
            ));
        };
        syntax_trees.expressions.replace_expression(
            expression,
            ExpressionNode::Call(psi_syntax_trees::expression::TableCallExpression {
                operational_acknowledgement: psi_language_core::CallOperationalAcknowledgement {
                    acknowledges_suspend: true,
                    acknowledges_block,
                    ..Default::default()
                },
                ..call
            }),
        );
        return Ok((expression, rest));
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
        let (expression, rest) = parse_unary_expression_handle(syntax_trees, input, context)?;
        let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression).clone()
        else {
            return Err(input.error_here(
                "`block` is a call acknowledgement and must appear immediately before a call",
            ));
        };
        syntax_trees.expressions.replace_expression(
            expression,
            ExpressionNode::Call(psi_syntax_trees::expression::TableCallExpression {
                operational_acknowledgement: psi_language_core::CallOperationalAcknowledgement {
                    acknowledges_block: true,
                    ..Default::default()
                },
                ..call
            }),
        );
        return Ok((expression, rest));
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
        let (expression, rest) = parse_unary_expression_handle(syntax_trees, input, context)?;
        return Ok((
            syntax_trees.expressions.insert(ExpressionNode::Borrow(
                psi_syntax_trees::expression::TableBorrowExpression {
                    target: expression,
                    access,
                },
            )),
            rest,
        ));
    }

    if input.at_punctuation(PunctuationKind::Exclamation) {
        let operator_span = input
            .tokens
            .first()
            .map(|token| input.source_span(token))
            .expect("recognized unary punctuation has a source token");
        let input = input.take_punctuation(PunctuationKind::Exclamation, "!")?;
        let (operand, rest) = parse_unary_expression_handle(syntax_trees, input, context)?;
        let expression =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: UnaryOperator::LogicalNot,
                    operand,
                }));
        syntax_trees
            .expressions
            .set_source_span(expression, operator_span);
        return Ok((expression, rest));
    }

    if input.at_punctuation(PunctuationKind::Tilde) {
        let operator_span = input
            .tokens
            .first()
            .map(|token| input.source_span(token))
            .expect("recognized unary punctuation has a source token");
        let input = input.take_punctuation(PunctuationKind::Tilde, "~")?;
        let (operand, rest) = parse_unary_expression_handle(syntax_trees, input, context)?;
        let expression =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: UnaryOperator::BitwiseNot,
                    operand,
                }));
        syntax_trees
            .expressions
            .set_source_span(expression, operator_span);
        return Ok((expression, rest));
    }

    if input.at_punctuation(PunctuationKind::Minus) {
        let operator_span = input
            .tokens
            .first()
            .map(|token| input.source_span(token))
            .expect("recognized unary punctuation has a source token");
        let input = input.take_punctuation(PunctuationKind::Minus, "-")?;
        let (operand, rest) = parse_unary_expression_handle(syntax_trees, input, context)?;
        // Fold numeric literals into their negative value so a negative literal
        // stays a constant (usable in guards/static contexts). Negating any other
        // expression lowers to `0 - operand`, reusing the existing subtraction
        // lane rather than introducing a dedicated negate operator + codegen.
        let negated = match syntax_trees.expressions.expression(operand).clone() {
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
                    right: operand,
                })
            }
        };
        let negated = syntax_trees.expressions.insert(negated);
        syntax_trees
            .expressions
            .set_source_span(negated, operator_span);
        return Ok((negated, rest));
    }

    if input.at_contextual("move") {
        let input = input.take_contextual("move")?;
        // Ownership is currently inferred from the value flow itself. `move`
        // is accepted as explicit spelling for that move, then lowered to the
        // moved expression so the existing ownership lane remains the source of
        // truth.
        return parse_unary_expression_handle(syntax_trees, input, context);
    }

    parse_postfix_expression_handle(syntax_trees, input, context)
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
