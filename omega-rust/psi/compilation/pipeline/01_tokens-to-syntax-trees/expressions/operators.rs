use crate::expressions::context::ExpressionContext;
use crate::syntax_storage::tables::{exprs, exprs_of};
use crate::token_input::cursor::Input;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::IntegerLiteral;
use syntax_trees::FloatSpelling;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableUnaryExpression,
    UnaryOperator,
};
use tokens::{KeywordKind, PunctuationKind};

use language_core::ReferenceAccess;

/// Logical chain operators over membership operands, with their
/// precedence (higher binds tighter).
pub(crate) const LOGICAL_OPERATORS: &[(PunctuationKind, BinaryOperator, u8)] = &[
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
];

/// Arithmetic chain operators over unary operands, with their
/// precedence (higher binds tighter).
pub(crate) const ARITHMETIC_OPERATORS: &[(PunctuationKind, BinaryOperator, u8)] = &[
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
    (PunctuationKind::PlusWrapping, BinaryOperator::Add, 4),
    (PunctuationKind::PlusSaturating, BinaryOperator::Add, 4),
    (PunctuationKind::PlusTrapping, BinaryOperator::Add, 4),
    (PunctuationKind::MinusWrapping, BinaryOperator::Subtract, 4),
    (
        PunctuationKind::MinusSaturating,
        BinaryOperator::Subtract,
        4,
    ),
    (PunctuationKind::MinusTrapping, BinaryOperator::Subtract, 4),
    (
        PunctuationKind::AsteriskWrapping,
        BinaryOperator::Multiply,
        5,
    ),
    (
        PunctuationKind::AsteriskSaturating,
        BinaryOperator::Multiply,
        5,
    ),
    (
        PunctuationKind::AsteriskTrapping,
        BinaryOperator::Multiply,
        5,
    ),
    (PunctuationKind::SlashWrapping, BinaryOperator::Divide, 5),
    (PunctuationKind::SlashSaturating, BinaryOperator::Divide, 5),
    (PunctuationKind::SlashTrapping, BinaryOperator::Divide, 5),
    (PunctuationKind::PercentWrapping, BinaryOperator::Modulo, 5),
    (
        PunctuationKind::PercentSaturating,
        BinaryOperator::Modulo,
        5,
    ),
    (PunctuationKind::PercentTrapping, BinaryOperator::Modulo, 5),
    (
        PunctuationKind::LessLessWrapping,
        BinaryOperator::ShiftLeft,
        3,
    ),
    (
        PunctuationKind::LessLessSaturating,
        BinaryOperator::ShiftLeft,
        3,
    ),
    (
        PunctuationKind::LessLessTrapping,
        BinaryOperator::ShiftLeft,
        3,
    ),
];

/// The policy a suffixed operator token selects. A suffixed form keeps
/// the precedence and associativity of its base operator, so the chain
/// table lists it beside that operator and this reads the suffix back.
pub(crate) fn punctuation_policy(punctuation: PunctuationKind) -> ArithmeticDomain {
    match punctuation {
        PunctuationKind::PlusWrapping
        | PunctuationKind::MinusWrapping
        | PunctuationKind::AsteriskWrapping
        | PunctuationKind::SlashWrapping
        | PunctuationKind::PercentWrapping
        | PunctuationKind::LessLessWrapping => ArithmeticDomain::Wrapping,
        PunctuationKind::PlusSaturating
        | PunctuationKind::MinusSaturating
        | PunctuationKind::AsteriskSaturating
        | PunctuationKind::SlashSaturating
        | PunctuationKind::PercentSaturating
        | PunctuationKind::LessLessSaturating => ArithmeticDomain::Saturating,
        PunctuationKind::PlusTrapping
        | PunctuationKind::MinusTrapping
        | PunctuationKind::AsteriskTrapping
        | PunctuationKind::SlashTrapping
        | PunctuationKind::PercentTrapping
        | PunctuationKind::LessLessTrapping => ArithmeticDomain::Trapping,
        _ => ArithmeticDomain::Exact,
    }
}

/// Right shift has no suffixed token: `>>` stays one token so a nested
/// static argument list can close one `>` at a time. Its policy suffix is
/// the `%`, `|` or `!` written immediately after it, with no space, so
/// `a >>! b` traps while `a >> !b` shifts by a negation.
pub(crate) fn take_right_shift_policy<'tokens, 'source>(
    shift: source::SourceSpan,
    input: Input<'tokens, 'source>,
) -> Result<(ArithmeticDomain, Input<'tokens, 'source>), crate::ParseError> {
    for (punctuation, label, policy) in [
        (PunctuationKind::Percent, "%", ArithmeticDomain::Wrapping),
        (PunctuationKind::Pipe, "|", ArithmeticDomain::Saturating),
        (
            PunctuationKind::Exclamation,
            "!",
            ArithmeticDomain::Trapping,
        ),
    ] {
        if !input.at_punctuation(punctuation) {
            continue;
        }
        let suffix = input.current_source_span();
        if suffix.source_id == shift.source_id && suffix.span.start == shift.span.end {
            return Ok((policy, input.take_punctuation(punctuation, label)?));
        }
    }
    Ok((ArithmeticDomain::Exact, input))
}

pub(crate) fn insert_binary_expression(
    syntax_trees: &mut SyntaxTrees,
    left: ExpressionHandle,
    operator: BinaryOperator,
    policy: ArithmeticDomain,
    right: ExpressionHandle,
    span: source::SourceSpan,
) -> ExpressionHandle {
    let expression = exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
        policy,
        left,
        operator,
        right,
    }));
    exprs(syntax_trees).set_source_span(expression, span);
    expression
}

pub(crate) enum UnaryPrefix<'tokens, 'source> {
    Borrow(ReferenceAccess, source::SourceSpan),
    Operator(UnaryOperator, source::SourceSpan),
    Negate(source::SourceSpan),
    Acknowledge {
        input: Input<'tokens, 'source>,
        suspend: bool,
        block: bool,
    },
    Move,
}

pub(crate) fn has_unary_expression_prefix(input: Input<'_, '_>) -> Result<bool, crate::ParseError> {
    take_unary_prefix(input, ExpressionContext::Default).map(|prefix| prefix.is_some())
}

pub(crate) fn take_unary_prefix<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> Result<Option<(UnaryPrefix<'tokens, 'source>, Input<'tokens, 'source>)>, crate::ParseError> {
    if context.allows_type_expression()
        && crate::expressions::primary::starts_lifetime_reference_type(input)
    {
        return Ok(None);
    }
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
        let span = input.current_source_span();
        let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
        let (access, input) = if input.at_contextual("mut") {
            (ReferenceAccess::Mutable, input.take_contextual("mut")?)
        } else if input.at_contextual("write") {
            (ReferenceAccess::WriteOnly, input.take_contextual("write")?)
        } else if input.at_keyword(KeywordKind::State) {
            (
                ReferenceAccess::Mutable,
                input.take_keyword(KeywordKind::State, "state")?,
            )
        } else {
            (ReferenceAccess::Shared, input)
        };
        return Ok(Some((UnaryPrefix::Borrow(access, span), input)));
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
    if input.at_contextual_then_name("move") {
        return Ok(Some((UnaryPrefix::Move, input.take_contextual("move")?)));
    }
    Ok(None)
}

pub(crate) fn apply_unary_prefix(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    prefix: UnaryPrefix<'_, '_>,
) -> Result<ExpressionHandle, crate::ParseError> {
    match prefix {
        UnaryPrefix::Borrow(access, span) => {
            let borrow = exprs(syntax_trees).insert(ExpressionNode::Borrow(
                syntax_trees::expression::TableBorrowExpression {
                    target: expression,
                    access,
                },
            ));
            // A loan diagnosis can point at the operator that forms it.
            exprs(syntax_trees).set_source_span(borrow, span);
            Ok(borrow)
        }
        UnaryPrefix::Operator(operator, span) => {
            let expression =
                exprs(syntax_trees).insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator,
                    operand: expression,
                }));
            exprs(syntax_trees).set_source_span(expression, span);
            Ok(expression)
        }
        UnaryPrefix::Negate(span) => {
            let negated = match exprs_of(syntax_trees).expression(expression) {
                ExpressionNode::Integer(literal) => ExpressionNode::Integer(literal.negated()),
                ExpressionNode::Float(text) => ExpressionNode::Float(FloatSpelling::generated(
                    format!("-{}", text.as_str(syntax_trees)),
                )),
                _ => {
                    let zero = syntax_trees
                        .expressions
                        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
                    ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: zero,
                        operator: BinaryOperator::Subtract,
                        right: expression,
                    })
                }
            };
            let expression = exprs(syntax_trees).insert(negated);
            exprs(syntax_trees).set_source_span(expression, span);
            Ok(expression)
        }
        UnaryPrefix::Acknowledge {
            input,
            suspend,
            block,
        } => {
            if !exprs(syntax_trees).set_call_acknowledgement(
                expression,
                language_core::CallOperationalAcknowledgement {
                    acknowledges_suspend: suspend,
                    acknowledges_block: block,
                    ..Default::default()
                },
            ) {
                return Err(input.error_here(if suspend {
                    "`suspend` is a call acknowledgement and must appear immediately before a call"
                } else {
                    "`block` is a call acknowledgement and must appear immediately before a call"
                }));
            }
            Ok(expression)
        }
        UnaryPrefix::Move => Ok(expression),
    }
}

pub(crate) fn punctuation_label(punctuation: PunctuationKind) -> &'static str {
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
        PunctuationKind::PlusWrapping => "+%",
        PunctuationKind::PlusSaturating => "+|",
        PunctuationKind::PlusTrapping => "+!",
        PunctuationKind::MinusWrapping => "-%",
        PunctuationKind::MinusSaturating => "-|",
        PunctuationKind::MinusTrapping => "-!",
        PunctuationKind::AsteriskWrapping => "*%",
        PunctuationKind::AsteriskSaturating => "*|",
        PunctuationKind::AsteriskTrapping => "*!",
        PunctuationKind::SlashWrapping => "/%",
        PunctuationKind::SlashSaturating => "/|",
        PunctuationKind::SlashTrapping => "/!",
        PunctuationKind::PercentWrapping => "%%",
        PunctuationKind::PercentSaturating => "%|",
        PunctuationKind::PercentTrapping => "%!",
        PunctuationKind::LessLessWrapping => "<<%",
        PunctuationKind::LessLessSaturating => "<<|",
        PunctuationKind::LessLessTrapping => "<<!",
        _ => "?",
    }
}
