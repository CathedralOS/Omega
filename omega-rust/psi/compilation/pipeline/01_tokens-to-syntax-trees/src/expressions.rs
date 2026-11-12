/*
Precedence expression grammar (`grammar::expressions`): primary operands,
postfix calls and projections, match dispatch, domain membership, static
argument values, and proof-fact expressions.
*/

pub(crate) mod context {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ExpressionContext {
        Default,
        NoStructLiteral,
        TypeExpressionNoStructLiteral,
        NoStructLiteralOrMembership,
        ProofFact,
        GroupedProofFact,
        LogicalProofFact,
        LogicalTypeExpressionNoStructLiteral,
        LogicalValue,
        GroupedLogicalProofFact,
    }

    impl ExpressionContext {
        pub(crate) fn allows_struct_literal(self) -> bool {
            matches!(
                self,
                Self::Default
                    | Self::GroupedProofFact
                    | Self::GroupedLogicalProofFact
                    | Self::LogicalValue
            )
        }

        pub(crate) fn allows_membership(self) -> bool {
            !matches!(
                self,
                Self::NoStructLiteralOrMembership | Self::ProofFact | Self::LogicalProofFact
            )
        }

        pub(crate) fn allows_type_expression(self) -> bool {
            matches!(
                self,
                Self::ProofFact
                    | Self::GroupedProofFact
                    | Self::LogicalProofFact
                    | Self::GroupedLogicalProofFact
                    | Self::TypeExpressionNoStructLiteral
                    | Self::LogicalTypeExpressionNoStructLiteral
            )
        }

        pub(crate) fn boolean(self) -> Self {
            match self {
                Self::Default => Self::GroupedProofFact,
                Self::NoStructLiteral => Self::TypeExpressionNoStructLiteral,
                Self::NoStructLiteralOrMembership => Self::ProofFact,
                Self::LogicalValue => Self::GroupedLogicalProofFact,
                _ => self,
            }
        }

        pub(crate) fn allows_logical_subject(self) -> bool {
            matches!(
                self,
                Self::LogicalProofFact
                    | Self::GroupedLogicalProofFact
                    | Self::LogicalTypeExpressionNoStructLiteral
                    | Self::LogicalValue
            )
        }

        pub(crate) fn value(self) -> Self {
            if self.allows_logical_subject() {
                Self::LogicalValue
            } else {
                Self::Default
            }
        }

        pub(crate) fn grouped(self) -> Self {
            if self == Self::LogicalValue {
                Self::LogicalValue
            } else if self.allows_logical_subject() {
                Self::GroupedLogicalProofFact
            } else if self.allows_type_expression() {
                Self::GroupedProofFact
            } else {
                Self::Default
            }
        }
    }
}
pub(crate) mod frames;
pub(crate) mod membership {
    use crate::grammar::expressions::context::ExpressionContext;
    use crate::storage::tables::exprs;
    use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
        TableMembershipExpression,
    };
    use tokens::PunctuationKind;

    /// Domain membership tests (`value in Domain & Other`) after a finished
    /// arithmetic operand.
    pub(crate) fn extend_membership<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        mut expression: ExpressionHandle,
        mut input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        while context.allows_membership() && input.at_contextual("in") {
            input = input.take_contextual("in")?;
            let subject = expression;
            let (domain, rest) = parse_path_handle_span(input, |member| {
                exprs(syntax_trees).append_identifier_path_member(member)
            })?;
            input = rest;
            expression =
                exprs(syntax_trees).insert(ExpressionNode::Membership(TableMembershipExpression {
                    value: expression,
                    domain,
                }));

            while input.at_punctuation(PunctuationKind::Ampersand)
                || input.at_punctuation(PunctuationKind::Pipe)
            {
                let (operator, after_operator) = if input.at_punctuation(PunctuationKind::Ampersand)
                {
                    (
                        BinaryOperator::And,
                        input.take_punctuation(PunctuationKind::Ampersand, "&")?,
                    )
                } else {
                    (
                        BinaryOperator::Or,
                        input.take_punctuation(PunctuationKind::Pipe, "|")?,
                    )
                };
                let (domain, rest) = parse_path_handle_span(after_operator, |member| {
                    exprs(syntax_trees).append_identifier_path_member(member)
                })?;
                input = rest;
                let right = exprs(syntax_trees).insert(ExpressionNode::Membership(
                    TableMembershipExpression {
                        value: subject,
                        domain,
                    },
                ));
                expression =
                    exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: expression,
                        operator,
                        right,
                    }));
            }
        }

        Ok((expression, input))
    }
}
pub(crate) mod parse_expression {
    use crate::grammar::expressions::context::ExpressionContext;
    use crate::storage::tables::{exprs, exprs_of};
    use crate::tokens::token_cursor::{Input, ParseResult};
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::literals::IntegerLiteral;
    use syntax_trees::FloatSpelling;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
        TableUnaryExpression, UnaryOperator,
    };
    use tokens::{KeywordKind, PunctuationKind};

    use super::frames::{Goal, run_expression};
    use language_core::ReferenceAccess;

    pub(crate) fn parse_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)
    }

    pub(crate) fn parse_expression_handle_without_struct_literals<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        parse_expression_handle_in(syntax_trees, input, ExpressionContext::NoStructLiteral)
    }

    pub(crate) fn parse_expression_handle_without_struct_literals_or_membership<
        'tokens,
        'source,
    >(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        parse_expression_handle_in(
            syntax_trees,
            input,
            ExpressionContext::NoStructLiteralOrMembership,
        )
    }

    pub(crate) fn parse_proof_fact_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        super::proof_fact_expressions::parse(syntax_trees, input, ExpressionContext::ProofFact)
    }

    pub(crate) fn parse_logical_proof_fact_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        super::proof_fact_expressions::parse(
            syntax_trees,
            input,
            ExpressionContext::LogicalProofFact,
        )
    }

    pub(crate) fn parse_const_integer_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        run_expression(syntax_trees, input, Goal::ConstInteger)
    }

    pub(crate) fn parse_expression_handle_in<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        run_expression(syntax_trees, input, Goal::Expression(context))
    }

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
        let expression =
            exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                policy,
                left,
                operator,
                right,
            }));
        exprs(syntax_trees).set_source_span(expression, span);
        expression
    }

    pub(crate) enum UnaryPrefix<'tokens, 'source> {
        Borrow(ReferenceAccess),
        Operator(UnaryOperator, source::SourceSpan),
        Negate(source::SourceSpan),
        Acknowledge {
            input: Input<'tokens, 'source>,
            suspend: bool,
            block: bool,
        },
        Move,
    }

    pub(crate) fn has_unary_expression_prefix(
        input: Input<'_, '_>,
    ) -> Result<bool, crate::ParseError> {
        take_unary_prefix(input, ExpressionContext::Default).map(|prefix| prefix.is_some())
    }

    pub(crate) fn take_unary_prefix<'tokens, 'source>(
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> Result<Option<(UnaryPrefix<'tokens, 'source>, Input<'tokens, 'source>)>, crate::ParseError>
    {
        if context.allows_type_expression() && super::primary::starts_lifetime_reference_type(input)
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
                return Err(input.error_here(
                    "duplicate `suspend` call acknowledgement; write it exactly once",
                ));
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
                return Err(input
                    .error_here("duplicate `block` call acknowledgement; write it exactly once"));
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
            UnaryPrefix::Borrow(access) => Ok(exprs(syntax_trees).insert(ExpressionNode::Borrow(
                syntax_trees::expression::TableBorrowExpression {
                    target: expression,
                    access,
                },
            ))),
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
}
pub(crate) mod parse_postfix {
    use crate::diagnostics::parse_error::ParseError;
    use crate::grammar::expressions::context::ExpressionContext;
    use crate::grammar::type_syntax::parse_type::parse_cast_target_type_reference_handle;
    use crate::storage::tables::{exprs, exprs_of};
    use crate::tokens::token_cursor::{Input, ParseResult};
    use arena::HandleSpan;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{
        ExpressionHandle, ExpressionNode, StaticMachineArgument, StaticSymbolApplication,
        TableCallExpression, TableCastExpression, TableMemberExpression,
    };
    use tokens::{KeywordKind, PunctuationKind};

    use crate::grammar::type_syntax::parse_type::parse_domain_argument_handles;
    use numerics::arithmetic::ArithmeticDomain;
    use syntax_trees::identifier::Identifier;

    /// A primary operand and its postfix suffixes, for statement grammar
    /// that reads a call or place target without operators around it.
    pub(crate) fn parse_postfix_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        super::frames::run_expression(syntax_trees, input, super::frames::Goal::Postfix(context))
    }

    /// The end of a call's argument list once its value arguments are
    /// parsed: the optional `;` evidence lane, then the closing `)`.
    pub(crate) fn finish_argument_lanes<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        arguments: Vec<ExpressionHandle>,
        mut input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, (HandleSpan<ExpressionHandle>, Box<[Identifier]>)> {
        let mut evidence_arguments = Vec::new();
        if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            if input.at_punctuation(PunctuationKind::RightParen) {
                return Err(input.error_here(
                    "the `;` call-lane separator must be followed by an evidence term",
                ));
            }
            loop {
                let (argument, rest) = input.take_identifier()?;
                evidence_arguments.push(argument);
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
        Ok(((arguments, evidence_arguments.into_boxed_slice()), input))
    }

    /// A `.member` projection after the operand parsed so far, or the
    /// start of an atomic `.load(ordering)` whose ordering the caller parses.
    pub(crate) enum MemberSuffix<'tokens, 'source> {
        Member(ExpressionHandle, Input<'tokens, 'source>),
        Load { after_open: Input<'tokens, 'source> },
    }

    pub(crate) fn parse_member_suffix<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        mut expression: ExpressionHandle,
        input: Input<'tokens, 'source>,
    ) -> Result<MemberSuffix<'tokens, 'source>, ParseError> {
        let mut access_source_span = input.source_span(
            input
                .tokens
                .first()
                .expect("recognized member punctuation has a source token"),
        );
        let after_dot = input.take_punctuation(PunctuationKind::Dot, ".")?;
        let (member, rest) = after_dot.take_identifier()?;
        access_source_span.span.end = member.source_span().span.end;

        if rest.at_punctuation(PunctuationKind::Less) {
            let retired = match member.as_str(syntax_trees) {
                "accept_boundary" => Some(
                    "receiver-style boundary acceptance is retired; use Build::accept_boundary<Boundary>(builder)",
                ),
                "select_provider" => Some(
                    "receiver-style provider selection is retired; use Build::select_provider<Slot, Provider>(builder)",
                ),
                "exclude_service" => Some(
                    "receiver-style service exclusion is retired; use Build::exclude_service<BoundaryTrait>(builder)",
                ),
                "select_representation" => Some(
                    "receiver-style representation selection is retired; use Build::select_representation<Opaque, Conformance>(builder)",
                ),
                "require_wire_compatibility" => Some(
                    "receiver-style wire compatibility is retired; use Build::require_wire_compatibility<Edge, Lineage, Local, Peer, Fact>(builder)",
                ),
                "assume_binding_contract" => Some(
                    "receiver-style assumed binding is retired; use Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)",
                ),
                _ => None,
            };
            if let Some(message) = retired {
                return Err(rest.error_here(message));
            }
        }

        if member.as_str(syntax_trees) == "load" && rest.at_punctuation(PunctuationKind::LeftParen)
        {
            let after_open = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
            return Ok(MemberSuffix::Load { after_open });
        }

        let member_span = member.source_span();
        expression = exprs(syntax_trees).insert(ExpressionNode::Member(TableMemberExpression {
            receiver: expression,
            access_source_span,
            member,
            case_variant: None,
        }));
        exprs(syntax_trees).set_source_span(expression, member_span);
        Ok(MemberSuffix::Member(expression, rest))
    }

    /// A cast written after the operand parsed so far, starting at `as`.
    pub(crate) fn parse_cast_suffix<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        mut expression: ExpressionHandle,
        mut input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        let cast_source = input.current_source_span();
        input = input.take_keyword(KeywordKind::As, "as")?;
        if at_cast_entry(input) {
            return parse_cast_entries(syntax_trees, expression, input, false);
        }
        let mut form = language_core::cast_form::CastForm::Value;
        if input.at_punctuation(PunctuationKind::Ampersand) {
            input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
            let access = if input.at_contextual("mut") {
                input = input.take_contextual("mut")?;
                language_core::ReferenceAccess::Mutable
            } else if input.at_contextual("write") {
                input = input.take_contextual("write")?;
                language_core::ReferenceAccess::WriteOnly
            } else {
                language_core::ReferenceAccess::Shared
            };
            form = language_core::cast_form::CastForm::Recast(access);
        }
        let target_start = input;
        let (target_type, rest) = parse_cast_target_type_reference_handle(syntax_trees, input)?;
        let consumed = target_start.tokens.len().saturating_sub(rest.tokens.len());
        let target_label_text = target_start.tokens[..consumed]
            .iter()
            .map(|token| target_start.text(token).as_str())
            .collect::<String>();
        let mut target_label = HandleSpan::empty();
        exprs(syntax_trees).append_identifier_path_member_to_span(
            &mut target_label,
            Identifier::generated(target_label_text),
        );
        input = rest;
        let mut domain = ArithmeticDomain::Exact;
        let mut semantic_domain = arena::HandleSpan::empty();
        let mut semantic_domain_arguments = arena::HandleSpan::empty();
        if input.at_contextual("in") {
            let after_in = input.take_contextual("in")?;
            let (domain_name, rest) =
                crate::grammar::type_syntax::parse_type::parse_static_name(after_in)?;
            match ArithmeticDomain::from_name(domain_name.as_str(syntax_trees)) {
                Some(_) if form.is_recast() => {
                    return Err(input.error_here(
                        "a recast re-views a place's bytes and takes no arithmetic domain; domains retag VALUE casts (`x as u8 in Wrapping`)",
                    ));
                }
                Some(parsed) => domain = parsed,
                None => {
                    exprs(syntax_trees).append_identifier_path_member_to_span(
                        &mut semantic_domain,
                        domain_name.clone(),
                    );
                    let (arguments, rest) = parse_domain_argument_handles(syntax_trees, rest)?;
                    semantic_domain_arguments = arguments;
                    input = rest;
                }
            }
            if semantic_domain.is_empty() {
                if rest.at_punctuation(PunctuationKind::Less) {
                    return Err(
                        rest.error_here("compiler arithmetic domains do not take index arguments")
                    );
                }
                input = rest;
            }
        }
        expression = exprs(syntax_trees).insert(ExpressionNode::Cast(TableCastExpression {
            value: expression,
            target_type,
            target_label,
            domain,
            semantic_domain,
            semantic_domain_arguments,
            form,
        }));
        exprs(syntax_trees).set_source_span(expression, cast_source);
        if at_cast_entry(input) {
            if form.is_recast() {
                return Err(input
                    .error_here("a recast re-views a place's bytes and takes no domain entries"));
            }
            if domain != ArithmeticDomain::Exact || !semantic_domain.is_empty() {
                return Err(input.error_here(
                        "a cast names its domains either with entries (`as T +D`) or with the older `as T in D`, not both",
                    ));
            }
            (expression, input) = parse_cast_entries(syntax_trees, expression, input, true)?;
        }
        Ok((expression, input))
    }

    fn at_cast_entry(input: Input<'_, '_>) -> bool {
        input.at_punctuation(PunctuationKind::Plus)
            || input.at_punctuation(PunctuationKind::Minus)
            || input.at_punctuation(PunctuationKind::Equal)
    }

    /*
        The run of `=D`, `-D` and `+D` entries after `as` (domains.md,
        "Grouping"). A `+` or `-` here always begins an entry, so no name lookup
        is needed and arithmetic on a cast result takes parentheses. Each entry
        becomes its own cast node over the previous one, because a cast node
        names one domain; typing accounts for the run as one simultaneous cast.
    */
    fn parse_cast_entries<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        mut expression: ExpressionHandle,
        mut input: Input<'tokens, 'source>,
        after_carrier: bool,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        use language_core::cast_form::{CastEntry, CastEntryKind, CastForm};
        let mut first = true;
        while at_cast_entry(input) {
            let kind = if input.at_punctuation(PunctuationKind::Plus) {
                CastEntryKind::Add
            } else if input.at_punctuation(PunctuationKind::Minus) {
                CastEntryKind::Remove
            } else {
                CastEntryKind::Keep
            };
            let sign = kind.sign();
            if after_carrier && kind != CastEntryKind::Add {
                return Err(input.error_here(format!(
                    "a carrier cast `as T` may only add domains; `{sign}` entries belong to a domain-only cast written first, as in `value as -D as T +D`. For arithmetic on the cast result, parenthesise the cast: `(value as T) {sign} ...`",
                )));
            }
            let entry_start = input;
            let entry_source = input.current_source_span();
            let after_sign = match kind {
                CastEntryKind::Add => input.take_punctuation(PunctuationKind::Plus, "+")?,
                CastEntryKind::Remove => input.take_punctuation(PunctuationKind::Minus, "-")?,
                CastEntryKind::Keep => input.take_punctuation(PunctuationKind::Equal, "=")?,
            };
            if !after_sign.at_name_like() {
                return Err(after_sign.error_here(format!(
                    "`{sign}` after a cast begins a domain entry such as `{sign}Km` and must be followed by a domain path; for arithmetic on the cast result, parenthesise the cast: `(value as T) {sign} ...`",
                )));
            }
            let (domain_name, rest) =
                crate::grammar::type_syntax::parse_type::parse_static_name(after_sign)?;
            let mut domain = ArithmeticDomain::Exact;
            let mut semantic_domain = arena::HandleSpan::empty();
            let mut semantic_domain_arguments = arena::HandleSpan::empty();
            match ArithmeticDomain::from_name(domain_name.as_str(syntax_trees)) {
                Some(parsed) => {
                    if rest.at_punctuation(PunctuationKind::Less) {
                        return Err(rest.error_here(
                            "compiler arithmetic domains do not take index arguments",
                        ));
                    }
                    domain = parsed;
                    input = rest;
                }
                None => {
                    exprs(syntax_trees)
                        .append_identifier_path_member_to_span(&mut semantic_domain, domain_name);
                    let (arguments, rest) = parse_domain_argument_handles(syntax_trees, rest)?;
                    semantic_domain_arguments = arguments;
                    input = rest;
                }
            }
            let consumed = entry_start.tokens.len().saturating_sub(input.tokens.len());
            let label_text = entry_start.tokens[..consumed]
                .iter()
                .map(|token| entry_start.text(token).as_str())
                .collect::<String>();
            let mut target_label = HandleSpan::empty();
            exprs(syntax_trees).append_identifier_path_member_to_span(
                &mut target_label,
                Identifier::generated(label_text),
            );
            // Typing replaces this placeholder with the operand's carrier.
            let target_type = crate::storage::tables::type_refs(syntax_trees).insert_unit();
            expression = exprs(syntax_trees).insert(ExpressionNode::Cast(TableCastExpression {
                value: expression,
                target_type,
                target_label,
                domain,
                semantic_domain,
                semantic_domain_arguments,
                form: CastForm::Entry(CastEntry {
                    kind,
                    first,
                    after_carrier,
                }),
            }));
            // Each entry reports at its own sign, not at the shared `as`.
            exprs(syntax_trees).set_source_span(expression, entry_source);
            first = false;
        }
        Ok((expression, input))
    }

    pub(crate) fn range_separator(input: &Input<'_, '_>) -> Option<bool> {
        if input.at_punctuation(PunctuationKind::DotDotEqual) {
            Some(true)
        } else if input.at_punctuation(PunctuationKind::DotDot) {
            Some(false)
        } else {
            None
        }
    }

    pub(crate) fn take_range_separator<'tokens, 'source>(
        input: Input<'tokens, 'source>,
        end_inclusive: bool,
    ) -> Result<Input<'tokens, 'source>, ParseError> {
        if end_inclusive {
            input.take_punctuation(PunctuationKind::DotDotEqual, "..=")
        } else {
            input.take_punctuation(PunctuationKind::DotDot, "..")
        }
    }

    pub(crate) fn build_call_expression_handle(
        syntax_trees: &mut SyntaxTrees,
        expression: ExpressionHandle,
        machine_arguments: Box<[StaticMachineArgument]>,
        arguments: HandleSpan<ExpressionHandle>,
        evidence_arguments: Box<[Identifier]>,
    ) -> Result<ExpressionHandle, ParseError> {
        let callee = expression;
        let expression = exprs_of(syntax_trees).expression(expression);
        let call = match expression {
            ExpressionNode::Name(path) => {
                let path = *path;
                let (target, receiver) = call_path_target_and_receiver(syntax_trees, path);
                let target = target.ok_or_else(|| ParseError::new("missing call target"))?;
                Ok(TableCallExpression {
                    callee,
                    target_is_static: true,
                    receiver,
                    target,
                    machine_arguments,
                    arguments,
                    evidence_arguments,
                    operational_acknowledgement: Default::default(),
                })
            }
            ExpressionNode::Member(member) => {
                validate_atomic_call_orderings(
                    syntax_trees,
                    member.member.as_str(syntax_trees),
                    member.receiver,
                    arguments,
                )?;
                Ok(TableCallExpression {
                    callee,
                    target_is_static: false,
                    receiver: member.receiver,
                    target: member.member.clone(),
                    machine_arguments,
                    arguments,
                    evidence_arguments,
                    operational_acknowledgement: Default::default(),
                })
            }
            _ => Ok(TableCallExpression {
                callee,
                receiver: ExpressionHandle::invalid(),
                target: Identifier::new("$callable", exprs_of(syntax_trees).source_span(callee)),
                target_is_static: false,
                machine_arguments,
                arguments,
                evidence_arguments,
                operational_acknowledgement: Default::default(),
            }),
        }?;
        let call = spell_test_group_facet(syntax_trees, call);
        let source = call.target.source_span();
        let expression = exprs(syntax_trees).insert(ExpressionNode::Call(call));
        exprs(syntax_trees).set_source_span(expression, source);
        Ok(expression)
    }

    pub(crate) fn memory_ordering_from_expression(
        syntax_trees: &SyntaxTrees,
        expression: ExpressionHandle,
    ) -> Result<language_core::atomic::MemoryOrdering, String> {
        let ExpressionNode::Name(path) = exprs_of(syntax_trees).expression(expression) else {
            return Err(
            "atomic ordering must be one of `NoOrdering`, `Receive`, `Publish`, `ReceivePublish`, or `GlobalOrder`"
                .to_owned(),
        );
        };
        let members = exprs_of(syntax_trees).identifier_path_members(*path);
        if members.len() != 1 {
            return Err(
            "atomic ordering must be an unqualified built-in name: `NoOrdering`, `Receive`, `Publish`, `ReceivePublish`, or `GlobalOrder`"
                .to_owned(),
        );
        }
        language_core::atomic::MemoryOrdering::from_name(members[0].as_str(syntax_trees)).ok_or_else(|| {
        format!(
            "unknown atomic ordering `{}`; expected `NoOrdering`, `Receive`, `Publish`, `ReceivePublish`, or `GlobalOrder`",
            members[0].as_str(syntax_trees)
        )
    })
    }

    fn validate_atomic_call_orderings(
        syntax_trees: &SyntaxTrees,
        target: &str,
        receiver: ExpressionHandle,
        arguments: HandleSpan<ExpressionHandle>,
    ) -> Result<(), ParseError> {
        if !receiver.is_valid() {
            return Ok(());
        }
        let arguments = exprs_of(syntax_trees).expression_handles(arguments);
        match (target, arguments) {
        ("store", [_, ordering]) => {
            let ordering = memory_ordering_from_expression(syntax_trees, *ordering)
                .map_err(ParseError::new)?;
            if !ordering.valid_for_store() {
                return Err(ParseError::new(format!(
                    "atomic store cannot use `{}` ordering; use `NoOrdering`, `Publish`, or `GlobalOrder`",
                    ordering.name()
                )));
            }
        }
        (
            "fetch_add" | "fetch_sub" | "fetch_xor" | "fetch_or" | "fetch_and" | "swap",
            [_, ordering],
        ) => {
            let _ = memory_ordering_from_expression(syntax_trees, *ordering)
                .map_err(ParseError::new)?;
        }
        (member, [_, _, success, failure])
            if member == "compare_exchange"
                || language_core::atomic::AtomicCompareExchangeObservationAxis::single_attempt_member(
                    member,
                )
                .is_some() =>
        {
            let success =
                memory_ordering_from_expression(syntax_trees, *success).map_err(ParseError::new)?;
            let failure =
                memory_ordering_from_expression(syntax_trees, *failure).map_err(ParseError::new)?;
            if !failure.valid_compare_exchange_failure(success) {
                return Err(ParseError::new(format!(
                    "atomic compare_exchange failure ordering `{}` is invalid for success ordering `{}`; failure cannot publish or be stronger than success",
                    failure.name(),
                    success.name()
                )));
            }
        }
        _ => {}
    }
        Ok(())
    }

    pub(crate) fn try_parse_static_machine_arguments<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Result<Option<(Box<[StaticMachineArgument]>, Input<'tokens, 'source>)>, ParseError> {
        if !input.at_punctuation(PunctuationKind::Less) {
            return Ok(None);
        }

        let mut group_depth = 0usize;
        let mut previous = None;
        for (offset, token) in input.tokens.iter().enumerate() {
            if token.is_non_semantic() {
                continue;
            }
            let Some(punctuation) = token.punctuation() else {
                previous = None;
                continue;
            };
            if group_depth == 0
                && punctuation == PunctuationKind::LeftParen
                && matches!(
                    previous,
                    Some(PunctuationKind::Greater | PunctuationKind::GreaterGreater)
                )
            {
                let mut prefix = input;
                prefix.tokens = &input.tokens[..offset];
                let checkpoint = syntax_trees.append_checkpoint();
                if let Ok(Some((arguments, end))) =
                    parse_static_call_arguments(syntax_trees, prefix)
                    && end.tokens.is_empty()
                    && !end.at_punctuation(PunctuationKind::Greater)
                {
                    let mut rest = input;
                    rest.tokens = &input.tokens[offset..];
                    return Ok(Some((arguments, rest)));
                }
                syntax_trees.rewind_appended(checkpoint);
            }
            match punctuation {
                PunctuationKind::LeftParen
                | PunctuationKind::LeftBracket
                | PunctuationKind::LeftBrace => group_depth += 1,
                PunctuationKind::RightParen
                | PunctuationKind::RightBracket
                | PunctuationKind::RightBrace => {
                    let Some(depth) = group_depth.checked_sub(1) else {
                        break;
                    };
                    group_depth = depth;
                }
                PunctuationKind::Semicolon
                | PunctuationKind::Colon
                | PunctuationKind::Equal
                | PunctuationKind::Arrow
                    if group_depth == 0 =>
                {
                    break;
                }
                _ => {}
            }
            previous = Some(punctuation);
        }
        Ok(None)
    }

    fn parse_static_call_arguments<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Result<Option<(Box<[StaticMachineArgument]>, Input<'tokens, 'source>)>, ParseError> {
        let mut cursor = input.take_punctuation(PunctuationKind::Less, "<")?;
        let mut arguments = Vec::new();
        loop {
            let Some((argument, rest)) = try_parse_static_argument(syntax_trees, cursor)? else {
                return Ok(None);
            };
            arguments.push(argument);
            cursor = rest;

            if cursor.at_punctuation(PunctuationKind::Comma) {
                cursor = cursor.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }
            if !cursor.at_punctuation(PunctuationKind::Greater) {
                return Ok(None);
            }
            cursor = cursor.take_punctuation(PunctuationKind::Greater, ">")?;
            return Ok(Some((arguments.into_boxed_slice(), cursor)));
        }
    }

    /*
        Names and applied members have the same scoped syntax in static operands
        and type positions. Keep each application's arguments on its own path
        component; namespace resolution later decides whether it names a type,
        requirement, or executable selection. Operand delimiters belong to the caller.
    */
    pub(crate) fn parse_static_name_selection<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
        let Ok((first, mut input)) = input.take_identifier() else {
            return Ok(None);
        };
        if input.at_punctuation(PunctuationKind::Dot) {
            input = input.take_punctuation(PunctuationKind::Dot, ".")?;
            let (member, rest) = input.take_identifier()?;
            return Ok(Some((
                StaticMachineArgument {
                    type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                    path: Box::default(),
                    application: None,
                    const_literal: None,
                    evidence_projection: Some(syntax_trees::expression::EvidenceProjection {
                        term: first,
                        member,
                    }),
                },
                rest,
            )));
        }
        let mut path = vec![first];
        while input.at_punctuation(PunctuationKind::ColonColon) {
            let after_separator = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let Ok((member, rest)) = after_separator.take_identifier() else {
                return Ok(None);
            };
            path.push(member);
            input = rest;
        }
        let application = if let Some((mut application, rest)) =
            try_parse_static_symbol_application(syntax_trees, input)?
        {
            input = rest;
            if input.at_punctuation(PunctuationKind::ColonColon) {
                let next = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
                let Some((member, rest)) = parse_static_name_selection(syntax_trees, next)? else {
                    return Ok(None);
                };
                application.member = exprs(syntax_trees).insert_static_member(member);
                input = rest;
            }
            Some(application)
        } else {
            None
        };
        Ok(Some((
            StaticMachineArgument {
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                path: path.into_boxed_slice(),
                application,
                const_literal: None,
                evidence_projection: None,
            },
            input,
        )))
    }

    fn try_parse_static_argument<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        mut input: Input<'tokens, 'source>,
    ) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
        let authored = input;
        let argument_checkpoint = syntax_trees.append_checkpoint();
        if input.at_punctuation(PunctuationKind::LeftBracket)
            || input.at_punctuation(PunctuationKind::Ampersand)
        {
            return parse_static_type_argument(syntax_trees, input).map(Some);
        }
        let Some((argument, rest)) = parse_static_name_selection(syntax_trees, input)? else {
            syntax_trees.rewind_appended(argument_checkpoint);
            return try_parse_static_value_argument(syntax_trees, input);
        };
        input = rest;
        if input.at_punctuation(PunctuationKind::LeftBracket) || input.at_contextual("in") {
            syntax_trees.rewind_appended(argument_checkpoint);
            return parse_static_type_argument(syntax_trees, authored).map(Some);
        }

        if input.at_punctuation(PunctuationKind::GreaterGreater)
            && let Some(value) = try_parse_static_value_argument(syntax_trees, authored)?
        {
            if argument.application.is_some() {
                syntax_trees.rewind_appended(argument_checkpoint);
                return try_parse_static_value_argument(syntax_trees, authored);
            }
            return Ok(Some(value));
        }
        if !(input.at_punctuation(PunctuationKind::Comma)
            || input.at_punctuation(PunctuationKind::Greater))
        {
            syntax_trees.rewind_appended(argument_checkpoint);
            return try_parse_static_value_argument(syntax_trees, authored);
        }

        Ok(Some((argument, input)))
    }

    fn try_parse_static_value_argument<'tokens, 'source>(
        syntax: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
        let checkpoint = syntax.append_checkpoint();
        let result = parse_static_value_alternatives(syntax, input);
        if !matches!(&result, Ok(Some((argument, _))) if argument.const_literal.is_none()) {
            syntax.rewind_appended(checkpoint);
        }
        result
    }

    fn parse_static_value_alternatives<'tokens, 'source>(
        scratch: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
        let Some((expression, rest)) =
            super::static_argument_values::parse_static_argument_expression(scratch, input)?
        else {
            return Ok(None);
        };
        if !(rest.at_punctuation(PunctuationKind::Comma)
            || rest.at_punctuation(PunctuationKind::Greater))
            || matches!(
                scratch.expressions.expression(expression),
                ExpressionNode::Name(_)
            )
        {
            return Ok(None);
        }
        let (type_reference, const_literal) = match scratch.expressions.expression(expression) {
            ExpressionNode::Integer(literal) => (Default::default(), Some(literal.clone())),
            _ => (
                scratch.type_references.insert(
                    syntax_trees::types::TypeReferenceNode::ConstExpression(expression),
                ),
                None,
            ),
        };
        Ok(Some((
            StaticMachineArgument {
                type_reference,
                path: Box::default(),
                application: None,
                const_literal,
                evidence_projection: None,
            },
            rest,
        )))
    }

    pub(crate) fn try_parse_static_symbol_application<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Result<Option<(Box<StaticSymbolApplication>, Input<'tokens, 'source>)>, ParseError> {
        let checkpoint = syntax_trees.append_checkpoint();
        let result = parse_static_symbol_application(syntax_trees, input);
        if !matches!(&result, Ok(Some(_))) {
            syntax_trees.rewind_appended(checkpoint);
        }
        result
    }

    fn parse_static_symbol_application<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Result<Option<(Box<StaticSymbolApplication>, Input<'tokens, 'source>)>, ParseError> {
        if input.at_punctuation(PunctuationKind::Less) {
            let mut cursor = input.take_punctuation(PunctuationKind::Less, "<")?;
            let mut lifetime_arguments = Vec::new();
            let mut arguments = Vec::new();
            let mut saw_non_lifetime = false;
            loop {
                if cursor.at_punctuation(PunctuationKind::Apostrophe) {
                    if saw_non_lifetime {
                        return Err(cursor.error_here(
                            "lifetime arguments precede type, const, and static-machine arguments",
                        ));
                    }
                    cursor = cursor.take_punctuation(PunctuationKind::Apostrophe, "'")?;
                    let (lifetime, rest) = cursor.take_identifier()?;
                    lifetime_arguments.push(lifetime);
                    cursor = rest;
                } else {
                    saw_non_lifetime = true;
                    let Some((argument, rest)) = try_parse_static_argument(syntax_trees, cursor)?
                    else {
                        return Ok(None);
                    };
                    arguments.push(argument);
                    cursor = rest;
                }

                if cursor.at_punctuation(PunctuationKind::Comma) {
                    cursor = cursor.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                if !cursor.at_punctuation(PunctuationKind::Greater) {
                    return Ok(None);
                }
                cursor = cursor.take_punctuation(PunctuationKind::Greater, ">")?;
                break Ok(Some((
                    Box::new(StaticSymbolApplication {
                        member: arena::Handle::invalid(),
                        lifetime_arguments: lifetime_arguments.into_boxed_slice(),
                        arguments: arguments.into_boxed_slice(),
                    }),
                    cursor,
                )));
            }
        } else {
            Ok(None)
        }
    }

    fn parse_static_type_argument<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Result<(StaticMachineArgument, Input<'tokens, 'source>), ParseError> {
        let (type_reference, rest) =
            crate::grammar::type_syntax::parse_type::parse_type_reference_handle(
                syntax_trees,
                input,
            )?;
        Ok((
            StaticMachineArgument {
                type_reference,
                path: Box::default(),
                application: None,
                const_literal: None,
                evidence_projection: None,
            },
            rest,
        ))
    }

    /*
        `owner.tests.group<Requirement>()` registers a test group, and
        `owner.tests.service<Group, Slot>()` selects its runner-supplied service,
        through the Build's `tests` facet (wiki/spec/build/testing.md). The facet is a value
        operand, so its receiver takes the same member projection an ordinary
        `owner.tests` value does rather than the static path a dotted call
        prefix otherwise becomes. Every other call keeps its parsed receiver.
    */
    fn spell_test_group_facet(
        syntax_trees: &mut SyntaxTrees,
        mut call: TableCallExpression,
    ) -> TableCallExpression {
        if !matches!(call.target.as_str(syntax_trees), "group" | "service")
            || call.machine_arguments.is_empty()
            || !call.receiver.is_valid()
        {
            return call;
        }
        let ExpressionNode::Name(path) = exprs_of(syntax_trees).expression(call.receiver) else {
            return call;
        };
        let path = *path;
        let members = exprs_of(syntax_trees)
            .identifier_path_members(path)
            .to_vec();
        let [.., facet] = members.as_slice() else {
            return call;
        };
        if members.len() < 2 || facet.as_str(syntax_trees) != "tests" {
            return call;
        }
        let facet = facet.clone();
        let owner_path = syntax_trees
            .expressions
            .copy_identifier_path_prefix(path, members.len() - 1);
        let owner = exprs(syntax_trees).insert(ExpressionNode::Name(owner_path));
        let receiver = exprs(syntax_trees).insert(ExpressionNode::Member(TableMemberExpression {
            receiver: owner,
            access_source_span: facet.source_span(),
            member: facet.clone(),
            case_variant: None,
        }));
        exprs(syntax_trees).set_source_span(receiver, facet.source_span());
        call.receiver = receiver;
        call.target_is_static = false;
        call
    }

    pub(crate) fn call_path_target_and_receiver(
        syntax_trees: &mut SyntaxTrees,
        path: HandleSpan<Identifier>,
    ) -> (Option<Identifier>, ExpressionHandle) {
        let members = exprs_of(syntax_trees).identifier_path_members(path);
        let count = members.len();
        let target = members.last().cloned();
        let receiver = if count <= 1 {
            ExpressionHandle::invalid()
        } else {
            let receiver_path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path, count - 1);
            exprs(syntax_trees).insert(ExpressionNode::Name(receiver_path))
        };
        (target, receiver)
    }

    #[cfg(test)]
    mod transaction_tests {
        use super::*;

        #[test]
        fn rejected_nested_static_attempts_leave_the_published_prefix_intact() {
            for spelling in [
                "<Outer<[u8; Count]>, value + >()",
                "<Outer<Inner<3>>, value + >()",
            ] {
                let tokens = source_files_to_tokens::Lexer::new(spelling)
                    .tokenize()
                    .unwrap();
                let retained = Input::retain_source(&tokens);
                let mut syntax = SyntaxTrees::new(source::SourceId(7));
                let prefix = syntax.expressions.insert(ExpressionNode::Boolean(true));
                let input = Input::new(source::SourceId(7), &tokens, &retained, &mut syntax);
                let types = syntax.type_references.type_reference_count();
                assert!(
                    try_parse_static_machine_arguments(&mut syntax, input)
                        .unwrap()
                        .is_none()
                );
                assert_eq!(syntax.expressions.iter_expressions().count(), 1);
                assert_eq!(syntax.type_references.type_reference_count(), types);
                assert!(matches!(
                    syntax.expressions.expression(prefix),
                    ExpressionNode::Boolean(true)
                ));
            }
        }
    }
}
pub(crate) mod primary {
    use crate::grammar::type_syntax::parse_type::parse_type_reference_handle;
    use crate::storage::tables::exprs;
    use crate::tokens::token_cursor::Input;
    use source::SourceSpan;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
    use syntax_trees::types::TypeReferenceHandle;
    use tokens::{PunctuationKind, TokenKind};

    pub(crate) fn spanned(
        syntax_trees: &mut SyntaxTrees,
        node: ExpressionNode,
        source_span: SourceSpan,
    ) -> ExpressionHandle {
        let expression = exprs(syntax_trees).insert(node);
        exprs(syntax_trees).set_source_span(expression, source_span);
        expression
    }

    pub(crate) fn try_applied_requirement_head<'tokens, 'source>(
        syntax: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Option<(TypeReferenceHandle, Input<'tokens, 'source>)> {
        let (_, after_name) = input.expect_token().ok()?;
        if !after_name.at_punctuation(PunctuationKind::Less)
            && !after_name.at_punctuation(PunctuationKind::ColonColon)
        {
            return None;
        }
        probe_applied_requirement_head(syntax, input)
    }

    #[inline(never)]
    fn probe_applied_requirement_head<'tokens, 'source>(
        syntax: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Option<(TypeReferenceHandle, Input<'tokens, 'source>)> {
        let checkpoint = syntax.append_checkpoint();
        let accepted = (|| {
            // This expression owns the applied receiver head; the call parser
            // consumes its member. A full type can itself include that member.
            let (name, rest) =
                crate::grammar::type_syntax::parse_type::parse_static_name(input).ok()?;
            let (reference, rest) =
                crate::grammar::type_syntax::parse_type::parse_named_type_reference(
                    syntax, name, rest, false,
                )
                .ok()?;
            if !matches!(
                syntax.type_references.type_reference(reference),
                syntax_trees::types::TypeReferenceNode::Generic { .. }
            ) {
                return None;
            }
            let after_separator = rest
                .take_punctuation(PunctuationKind::ColonColon, "::")
                .ok()?;
            let (_, after_member) = after_separator.take_identifier().ok()?;
            (after_member.at_punctuation(PunctuationKind::LeftParen)
                || after_member.at_punctuation(PunctuationKind::Less))
            .then_some((reference, rest))
        })();
        if accepted.is_none() {
            syntax.rewind_appended(checkpoint);
        }
        accepted
    }

    pub(crate) fn try_generic_type_operand<'tokens, 'source>(
        syntax: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        allow_body_delimiter: bool,
    ) -> Option<(TypeReferenceHandle, Input<'tokens, 'source>)> {
        let checkpoint = syntax.append_checkpoint();
        let accepted = (|| {
            let (reference, rest) = parse_type_reference_handle(syntax, input).ok()?;
            if !matches!(
                syntax.type_references.type_reference(reference),
                syntax_trees::types::TypeReferenceNode::Generic { .. }
            ) {
                return None;
            }
            let accepted = rest.tokens.is_empty()
                || [
                    PunctuationKind::Arrow,
                    PunctuationKind::EqualEqual,
                    PunctuationKind::ExclamationEqual,
                    PunctuationKind::PipePipe,
                    PunctuationKind::AndAnd,
                    PunctuationKind::Comma,
                    PunctuationKind::Semicolon,
                    PunctuationKind::LeftBrace,
                    PunctuationKind::RightBrace,
                    PunctuationKind::RightParen,
                ]
                .into_iter()
                .any(|punctuation| rest.at_punctuation(punctuation))
                || [
                    "requires",
                    "ensures",
                    "terminates",
                    "decreases",
                    "reaches",
                    "effects",
                    "invokes",
                    "suspends",
                    "blocks",
                    "crashes",
                    "boundary",
                    "satisfies",
                ]
                .into_iter()
                .any(|keyword| rest.at_contextual(keyword));
            (accepted && (allow_body_delimiter || !rest.at_punctuation(PunctuationKind::LeftBrace)))
                .then_some((reference, rest))
        })();
        if accepted.is_none() {
            syntax.rewind_appended(checkpoint);
        }
        accepted
    }

    pub(crate) fn starts_fixed_array_type(input: Input<'_, '_>) -> bool {
        let mut bracket_depth = 0usize;
        let mut parenthesis_depth = 0usize;
        let mut brace_depth = 0usize;
        for token in input.tokens {
            match token.kind {
                TokenKind::Punctuation(PunctuationKind::LeftBracket) => bracket_depth += 1,
                TokenKind::Punctuation(PunctuationKind::LeftParen) => parenthesis_depth += 1,
                TokenKind::Punctuation(PunctuationKind::RightParen) => {
                    let Some(depth) = parenthesis_depth.checked_sub(1) else {
                        return false;
                    };
                    parenthesis_depth = depth;
                }
                TokenKind::Punctuation(PunctuationKind::LeftBrace) => brace_depth += 1,
                TokenKind::Punctuation(PunctuationKind::RightBrace) => {
                    let Some(depth) = brace_depth.checked_sub(1) else {
                        return false;
                    };
                    brace_depth = depth;
                }
                TokenKind::Punctuation(PunctuationKind::RightBracket) => {
                    if bracket_depth <= 1 {
                        return false;
                    }
                    bracket_depth -= 1;
                }
                TokenKind::Punctuation(PunctuationKind::Semicolon)
                    if bracket_depth == 1 && parenthesis_depth == 0 && brace_depth == 0 =>
                {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    pub(crate) fn starts_lifetime_reference_type(input: Input<'_, '_>) -> bool {
        input.at_punctuation(PunctuationKind::Ampersand)
            && input
                .take_punctuation(PunctuationKind::Ampersand, "&")
                .is_ok_and(|rest| rest.at_punctuation(PunctuationKind::Apostrophe))
    }

    #[cfg(test)]
    mod generic_type_operand_tests {}
}
pub(crate) mod static_argument_values {
    use crate::diagnostics::parse_error::ParseError;
    use crate::grammar::expressions::parse_expression::parse_const_integer_expression_handle;
    use crate::tokens::token_cursor::Input;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::ExpressionHandle;
    use tokens::PunctuationKind;

    pub(crate) fn parse_static_argument_expression<'tokens, 'source>(
        scratch: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> Result<Option<(ExpressionHandle, Input<'tokens, 'source>)>, ParseError> {
        let whole_attempt = scratch.append_checkpoint();
        let parsed = parse_const_integer_expression_handle(scratch, input);
        let (expression, rest) = match parsed {
            Ok(parsed)
                if parsed.1.at_punctuation(PunctuationKind::Comma)
                    || parsed.1.at_punctuation(PunctuationKind::Greater) =>
            {
                parsed
            }
            _ => {
                scratch.rewind_appended(whole_attempt);
                let mut boundaries = Vec::new();
                let mut remaining = input;
                while let Ok((_, boundary)) = remaining.split_at_top_level_punctuation(
                    PunctuationKind::GreaterGreater,
                    "static argument delimiter",
                ) {
                    boundaries.push(boundary);
                    remaining = boundary.take_punctuation(PunctuationKind::GreaterGreater, ">>")?;
                }
                let mut selected = None;
                for rest in boundaries.into_iter().rev() {
                    let mut prefix = input;
                    prefix.tokens = &input.tokens[..input.tokens.len() - rest.tokens.len()];
                    let attempt = scratch.append_checkpoint();
                    if let Ok((expression, end)) =
                        parse_const_integer_expression_handle(scratch, prefix)
                        && end.tokens.is_empty()
                        && !end.at_punctuation(PunctuationKind::Greater)
                    {
                        selected = Some((expression, rest));
                        break;
                    }
                    scratch.rewind_appended(attempt);
                }
                let Some(selected) = selected else {
                    return Ok(None);
                };
                selected
            }
        };
        Ok(Some((expression, rest)))
    }
}
#[cfg(test)]
pub(crate) mod static_targets_tests {
    use source_files_to_tokens::Lexer;
    use syntax_trees::{SyntaxTrees, expression::ExpressionNode, statement::StatementNode};

    #[test]
    fn call_roots_retain_distinct_target_occurrences_through_syntax_copy() {
        let text = "const FIRST: u64 = outer(inner()); const SECOND: u64 = library::read(); const THIRD: u64 = receiver.read();";
        let tokens = Lexer::new(text).tokenize().expect("tokens");
        let syntax = crate::parse_syntax_trees(&tokens).expect("syntax");
        let mut copied = SyntaxTrees::new(source::SourceId::default());
        copied.extend_from(&syntax);
        for program in [&syntax, &copied] {
            let mut pending = program
                .root_items()
                .filter_map(|item| match item {
                    syntax_trees::item::Item::Const(definition) => Some(definition.value),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut spans = Vec::new();
            while let Some(expression) = pending.pop() {
                let ExpressionNode::Call(call) = program.expressions.expression(expression) else {
                    panic!("call");
                };
                let span = program.expressions.source_span(expression);
                assert_eq!(span, call.target.source_span());
                assert_eq!(
                    &text[span.span.start..span.span.end],
                    call.target.as_str(program)
                );
                assert!(
                    !spans.contains(&span),
                    "nested and sibling calls have distinct occurrences"
                );
                spans.push(span);
                pending.extend(
                    program
                        .expressions
                        .expression_handles(call.arguments)
                        .iter()
                        .copied(),
                );
            }
            assert_eq!(spans.len(), 4);
        }
    }

    #[test]
    fn static_and_value_call_targets_remain_distinct_after_syntax_copy() {
        let text = r#"
        machine example() {
            library::emit();
            receiver.emit();
            let first: u64 = library::read();
            let second: u64 = receiver.read();
        }
    "#;
        let tokens = Lexer::new(text).tokenize().expect("tokenize calls");
        let syntax = crate::parse_syntax_trees(&tokens).expect("parse calls");
        let mut copied = SyntaxTrees::new(source::SourceId::default());
        copied.extend_from(&syntax);
        for program in [&syntax, &copied] {
            let mut statements = Vec::new();
            let mut expressions = Vec::new();
            for item in program.root_items() {
                let syntax_trees::item::Item::Machine(machine) = item else {
                    continue;
                };
                for state in program.items.state_handles(machine.states) {
                    let state = program.items.state(*state);
                    for statement in program.items.statements(state.statements) {
                        if let StatementNode::Call(call) = program.statements.statement(*statement)
                        {
                            statements.push(call.target_is_static);
                        }
                        if let StatementNode::LocalData(local) =
                            program.statements.statement(*statement)
                        {
                            let ExpressionNode::Call(call) =
                                program.expressions.expression(local.initial_value)
                            else {
                                panic!("local initializer remains a call");
                            };
                            expressions.push(call.target_is_static);
                        }
                    }
                }
            }
            assert_eq!(statements, [true, false]);
            assert_eq!(expressions, [true, false]);
        }
    }

    #[test]
    fn structural_static_argument_probe_preserves_indexed_comparisons() {
        for comparison in ["left < values[0]", "left < values[0] > (right)"] {
            let source = format!(
                "machine compare(left: u64, right: u64, values: [u64; 1]) -> bool {{ {comparison} }}"
            );
            let tokens = Lexer::new(&source).tokenize().expect("tokens");
            let syntax = crate::parse_syntax_trees(&tokens).expect("ordinary comparison syntax");
            assert!(
                syntax.type_references.generic_nodes().is_empty(),
                "abandoned static argument probes publish no generic types"
            );
            assert!(!syntax.expressions.iter_expressions().any(|(_, expression)| matches!(expression, ExpressionNode::Call(call) if !call.machine_arguments.is_empty())));
        }
    }

    #[test]
    fn applied_requirement_call_keeps_its_trait_application_as_static_receiver() {
        let text = "const READ: i32 = Parent<u64>::read(); const NOTED: i32 = Sink<u8, bool>::note<Handler>(value); const COMPARED: bool = left < right;";
        let tokens = Lexer::new(text).tokenize().expect("tokens");
        let syntax = crate::parse_syntax_trees(&tokens).expect("syntax");
        let values = syntax
            .root_items()
            .filter_map(|item| match item {
                syntax_trees::item::Item::Const(definition) => Some(definition.value),
                _ => None,
            })
            .collect::<Vec<_>>();
        let applied = |expression, trait_name: &str, arity: u32, member: &str, static_arguments| {
            let ExpressionNode::Call(call) = syntax.expressions.expression(expression) else {
                panic!("applied requirement call");
            };
            assert!(call.target_is_static);
            assert_eq!(call.target.as_str(&syntax), member);
            assert_eq!(call.machine_arguments.len(), static_arguments);
            let ExpressionNode::TypeExpression(application) =
                syntax.expressions.expression(call.receiver)
            else {
                panic!("the applied trait is the static receiver");
            };
            let syntax_trees::types::TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } = syntax.type_references.type_reference(*application)
            else {
                panic!("generic trait application");
            };
            assert_eq!(base_name.as_str(&syntax), trait_name);
            assert_eq!(arguments.count(), arity);
        };
        applied(values[0], "Parent", 1, "read", 0);
        applied(values[1], "Sink", 2, "note", 1);
        assert!(matches!(
            syntax.expressions.expression(values[2]),
            ExpressionNode::Binary(_)
        ));
    }

    #[test]
    fn boolean_type_operands_preserve_constructor_and_value_contexts() {
        let cases = [
            (
                "local",
                "machine choose<T>() -> bool { let selected: bool = T == Box<u64>; selected }",
            ),
            (
                "guard",
                "machine choose<T>() -> bool { transition { T == Box<u64> -> true _ -> false } }",
            ),
            (
                "constructor",
                "data Box<T> { value: T; } machine build() -> Box<u64> { (Box { value: 1u64 }) }",
            ),
            (
                "call",
                "machine choose(value: u64) -> u64 { read<u64>(value) }",
            ),
            (
                "comparison",
                "machine compare(left: u64, right: u64, values: [u64; 1]) -> bool { left < values[0] > (right) }",
            ),
        ];
        let mut equalities = 0;
        for (role, text) in cases {
            let tokens = Lexer::new(text).tokenize().expect("tokens");
            let syntax = crate::parse_syntax_trees(&tokens)
                .unwrap_or_else(|error| panic!("{role}: {error:?}"));
            match role {
            "local" | "guard" => equalities += syntax.expressions.iter_expressions().filter(|(_, node)| matches!(node, ExpressionNode::Binary(binary)
                if binary.operator == syntax_trees::expression::BinaryOperator::Equal
                    && matches!(syntax.expressions.expression(binary.right), ExpressionNode::TypeExpression(_)))).count(),
            "constructor" => assert!(syntax.expressions.iter_expressions().any(|(_, node)| matches!(node, ExpressionNode::StructLiteral(_)))),
            "call" => assert!(syntax.expressions.iter_expressions().any(|(_, node)| matches!(node, ExpressionNode::Call(call) if call.machine_arguments.len() == 1))),
            "comparison" => assert!(syntax.type_references.generic_nodes().is_empty()),
            _ => unreachable!(),
        }
        }
        assert_eq!(
            equalities, 2,
            "local values and guards share static type syntax"
        );
    }

    #[test]
    fn boolean_type_operands_do_not_turn_qualified_type_arguments_into_indices() {
        let text = "machine initialize<P, T>() -> AccessOutcome<Extent in Granted & Resident<P, T>, AccessReturn<Extent in Granted & Vacant, T>, Failure> {}";
        let tokens = Lexer::new(text).tokenize().expect("tokens");
        let syntax = crate::parse_syntax_trees(&tokens).expect("qualified type arguments");
        let machine = syntax
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .expect("machine");
        let state = syntax
            .items
            .state(syntax.items.state_handles(machine.states)[0]);
        let syntax_trees::types::TypeReferenceNode::Generic { arguments, .. } =
            syntax.type_references.type_reference(state.return_type)
        else {
            panic!("applied result");
        };
        let arguments = syntax.type_references.type_reference_handles(*arguments);
        assert!(
            matches!(
                syntax.type_references.type_reference(arguments[0]),
                syntax_trees::types::TypeReferenceNode::Constrained { .. }
            ),
            "a constrained type retains its type role"
        );
        assert!(
            !syntax.type_references.const_expression_nodes().iter().any(
                |(_, expression)| matches!(
                    syntax.expressions.expression(*expression),
                    ExpressionNode::TypeExpression(_)
                )
            ),
            "a type description supplies no integer/index value"
        );
    }
}

pub(crate) mod proof_fact_expressions {
    use super::context::ExpressionContext;
    use super::parse_expression::parse_expression_handle_in;
    use crate::storage::tables::exprs_of;
    use crate::tokens::token_cursor::{Input, ParseResult};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

    pub(crate) fn parse<'tokens, 'source>(
        syntax: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        fact_context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        let checkpoint = syntax.append_checkpoint();
        let expression_context = if fact_context.allows_logical_subject() {
            ExpressionContext::LogicalTypeExpressionNoStructLiteral
        } else {
            ExpressionContext::TypeExpressionNoStructLiteral
        };
        if let Ok((expression, rest)) =
            parse_expression_handle_in(syntax, input, expression_context)
        {
            let expressions = exprs_of(syntax);
            let span = expressions.source_span(expression).span;
            if matches!(
                expressions.expression(expression),
                ExpressionNode::Binary(binary)
                    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
            ) && span.start < span.end
            {
                return Ok((expression, rest));
            }
        }
        syntax.rewind_appended(checkpoint);
        parse_expression_handle_in(syntax, input, fact_context)
    }
}
