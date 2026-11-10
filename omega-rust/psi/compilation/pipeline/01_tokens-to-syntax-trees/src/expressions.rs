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
pub(crate) mod match_dispatch {

    use super::parse_expression::{
        parse_expression_handle, parse_expression_handle_without_struct_literals,
    };
    use crate::grammar::patterns::bindings::{
        DestructureBinding, rewrite_destructure_guard_expression,
    };
    use crate::storage::tables::{exprs, exprs_of};
    use crate::tokens::token_cursor::{Input, ParseResult};
    use arena::Handle;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{
        ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern, MatchSubjectCapture,
        TableDestructurePattern, TableMatchArm, TableMatchExpression, TableSavedSubject,
    };
    use tokens::{KeywordKind, PunctuationKind};

    pub(crate) fn parse_match_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        let start = input;
        let input = input.take_keyword(KeywordKind::Match, "match")?;
        let (subject, input) =
            parse_expression_handle_without_struct_literals(syntax_trees, input)?;
        let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        let mut capture = Handle::<MatchSubjectCapture>::invalid();
        let mut arms = Vec::new();
        while !input.at_punctuation(PunctuationKind::RightBrace) {
            let arm_start = input;
            let (mut pattern, rest) = parse_pattern(syntax_trees, input)?;
            let rest = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
            let (mut value, rest) = parse_expression_handle(syntax_trees, rest)?;
            if let MatchPattern::Destructure(destructure) = &mut pattern {
                if !capture.is_valid() {
                    capture = exprs(syntax_trees).insert_match_subject(subject);
                }
                let bindings = syntax_trees
                    .expressions
                    .match_fields(destructure.fields)
                    .iter()
                    .filter_map(|field| {
                        let MatchFieldAction::Bind(binding) = &field.action else {
                            return None;
                        };
                        Some(DestructureBinding {
                            component: field.component,
                            binding: binding.clone(),
                            member: field.member.clone(),
                            case_variant: None,
                        })
                    })
                    .collect::<Vec<_>>();
                for (position, binding) in bindings.iter().enumerate() {
                    if bindings[..position].iter().any(|prior| {
                        prior.binding.as_str(syntax_trees) == binding.binding.as_str(syntax_trees)
                    }) || syntax_trees
                        .statements
                        .outcome_proof_selectors(destructure.proof_selectors)
                        .iter()
                        .any(|selector| {
                            selector.binding.as_str(syntax_trees)
                                == binding.binding.as_str(syntax_trees)
                        })
                    {
                        return Err(arm_start.error_here(format!(
                            "pattern binding `{}` is introduced more than once in this arm",
                            binding.binding.as_str(syntax_trees)
                        )));
                    }
                }
                let saved = syntax_trees
                    .expressions
                    .insert(ExpressionNode::SavedSubject(TableSavedSubject {
                        component: Default::default(),
                        capture,
                        pattern_head: destructure.head,
                    }));
                let source_span = exprs_of(syntax_trees).source_span(subject);
                exprs(syntax_trees).set_source_span(saved, source_span);
                value = rewrite_destructure_guard_expression(syntax_trees, value, saved, &bindings);
                if destructure.guard.is_valid() {
                    destructure.guard = rewrite_destructure_guard_expression(
                        syntax_trees,
                        destructure.guard,
                        saved,
                        &bindings,
                    );
                }
            }
            arms.push(TableMatchArm {
                pattern,
                value,
                source_span: arm_start.source_span_until(rest),
            });
            input = if rest.at_punctuation(PunctuationKind::Comma) {
                rest.take_punctuation(PunctuationKind::Comma, ",")?
            } else {
                rest
            };
        }
        let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        if arms.is_empty() {
            return Err(input.error_here("match expression must have at least one arm"));
        }
        let arms = exprs(syntax_trees).insert_match_arms(arms);
        let expression =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Match(TableMatchExpression {
                    subject,
                    capture,
                    arms,
                }));
        exprs(syntax_trees).set_source_span(expression, start.source_span_until(input));
        Ok((expression, input))
    }

    fn parse_pattern<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, MatchPattern> {
        if input.at_contextual("_") {
            return Ok((MatchPattern::Wildcard, input.take_contextual("_")?));
        }
        let (head, rest) = parse_expression_handle_without_struct_literals(syntax_trees, input)?;
        if !rest.at_punctuation(PunctuationKind::LeftBrace)
            || !matches!(
                exprs_of(syntax_trees).expression(head),
                ExpressionNode::Name(_)
            )
        {
            return Ok((MatchPattern::Value(head), rest));
        }
        let (fields, rest) =
            crate::grammar::patterns::parse_destructure_fields(syntax_trees, rest)?;
        let fields_span = exprs(syntax_trees).insert_match_fields(fields.fields);
        let proof_selectors = syntax_trees
            .statements
            .insert_outcome_proof_selectors(fields.proof_selectors);
        let (guard, rest) = if rest.at_keyword(KeywordKind::If) {
            let rest = rest.take_keyword(KeywordKind::If, "if")?;
            parse_expression_handle_without_struct_literals(syntax_trees, rest)?
        } else {
            (ExpressionHandle::invalid(), rest)
        };
        Ok((
            MatchPattern::Destructure(TableDestructurePattern {
                application_type: Default::default(),
                head,
                fields: fields_span,
                has_rest: fields.has_rest,
                guard,
                proof_selectors,
            }),
            rest,
        ))
    }
}
pub(crate) mod membership {
    use crate::grammar::expressions::context::ExpressionContext;
    use crate::grammar::expressions::parse_expression::parse_bitwise_or_expression_handle;
    use crate::storage::tables::exprs;
    use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
        TableMembershipExpression,
    };
    use tokens::PunctuationKind;

    pub(crate) fn parse_membership_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        let (mut expression, mut input) =
            parse_bitwise_or_expression_handle(syntax_trees, input, context)?;

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

    use super::membership::parse_membership_expression_handle;
    use super::parse_postfix::parse_postfix_expression_handle;
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
        let outer_depth = input.depth();
        let outer_constant = input.constant_position();
        let input = input.with_constant_position(true).deepen()?;
        let (handle, rest) = parse_bitwise_or_expression_handle(
            syntax_trees,
            input,
            ExpressionContext::NoStructLiteral,
        )?;
        Ok((
            handle,
            rest.with_depth(outer_depth)
                .with_constant_position(outer_constant),
        ))
    }

    pub(crate) fn parse_expression_handle_in<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
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
        let context = context.boolean();
        parse_binary_chain_handle::<4>(
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

    pub(crate) fn parse_bitwise_or_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        parse_binary_chain_handle::<6>(
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
            ],
        )
    }

    /// The policy a suffixed operator token selects. A suffixed form keeps
    /// the precedence and associativity of its base operator, so the chain
    /// table lists it beside that operator and this reads the suffix back.
    fn punctuation_policy(punctuation: PunctuationKind) -> ArithmeticDomain {
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
    fn take_right_shift_policy<'tokens, 'source>(
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

    fn parse_binary_chain_handle<'tokens, 'source, const PRECEDENCE_LEVELS: usize>(
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
        let mut pending = [None; PRECEDENCE_LEVELS];
        let mut pending_count = 0;

        while let Some((punctuation, operator, precedence)) = operators
            .iter()
            .find(|(punctuation, _, _)| input.at_punctuation(*punctuation))
            .copied()
        {
            while pending_count != 0 {
                let (_, _, pending_precedence, _, _) = pending[pending_count - 1]
                    .as_ref()
                    .expect("pending operator");
                if *pending_precedence < precedence {
                    break;
                }
                pending_count -= 1;
                let (left, operator, _, span, policy) =
                    pending[pending_count].take().expect("pending operator");
                expression = insert_binary_expression(
                    syntax_trees,
                    left,
                    operator,
                    policy,
                    expression,
                    span,
                );
            }
            let operator_span = input.current_source_span();
            input = input.take_punctuation(punctuation, punctuation_label(punctuation))?;
            let mut policy = punctuation_policy(punctuation);
            if punctuation == PunctuationKind::GreaterGreater {
                (policy, input) = take_right_shift_policy(operator_span, input)?;
            }
            // A constant position is folded before any operand is typed, and
            // folding a suffixed operator as the bare Exact one would drop
            // its policy without a word. Reject it at the operator, however
            // deeply the constant nests it.
            if policy != ArithmeticDomain::Exact && input.constant_position() {
                return Err(crate::ParseError::at_source_span(
                    format!(
                        "the policy operator `{}{}` is not available in a constant expression yet; use it in executable machine code",
                        operator.display_name(),
                        policy.operator_suffix(),
                    ),
                    operator_span,
                ));
            }
            pending[pending_count] =
                Some((expression, operator, precedence, operator_span, policy));
            pending_count += 1;
            (expression, input) = lower(syntax_trees, input, context)?;
        }

        while pending_count != 0 {
            pending_count -= 1;
            let (left, operator, _, span, policy) =
                pending[pending_count].take().expect("pending operator");
            expression =
                insert_binary_expression(syntax_trees, left, operator, policy, expression, span);
        }
        Ok((expression, input))
    }

    fn insert_binary_expression(
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

    enum UnaryPrefix<'tokens, 'source> {
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

    fn parse_unary_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        let Some((first_prefix, mut input)) = take_unary_prefix(input, context)? else {
            return parse_postfix_expression_handle(syntax_trees, input, context);
        };
        let mut prefixes = Vec::new();
        while let Some((prefix, rest)) = take_unary_prefix(input, context)? {
            prefixes.push(prefix);
            input = rest;
        }
        let (mut expression, rest) = parse_postfix_expression_handle(syntax_trees, input, context)?;
        while let Some(prefix) = prefixes.pop() {
            expression = apply_unary_prefix(syntax_trees, expression, prefix)?;
        }
        expression = apply_unary_prefix(syntax_trees, expression, first_prefix)?;
        Ok((expression, rest))
    }

    pub(crate) fn has_unary_expression_prefix(
        input: Input<'_, '_>,
    ) -> Result<bool, crate::ParseError> {
        take_unary_prefix(input, ExpressionContext::Default).map(|prefix| prefix.is_some())
    }

    fn take_unary_prefix<'tokens, 'source>(
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

    fn apply_unary_prefix(
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
        TableCallExpression, TableCastExpression, TableIndexedExpression, TableMemberExpression,
        TableRangeExpression,
    };
    use tokens::{KeywordKind, PunctuationKind};

    use super::parse_expression::{parse_expression_handle, parse_expression_handle_in};
    use super::primary::parse_primary_expression_handle;
    use crate::grammar::type_syntax::parse_type::parse_domain_argument_handles;
    use numerics::arithmetic::ArithmeticDomain;
    use syntax_trees::identifier::Identifier;

    pub(crate) fn parse_argument_list_after_open_paren_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, (HandleSpan<ExpressionHandle>, Box<[Identifier]>)> {
        parse_arguments_in(syntax_trees, input, ExpressionContext::Default)
    }

    fn parse_arguments_in<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        mut input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, (HandleSpan<ExpressionHandle>, Box<[Identifier]>)> {
        let mut arguments = Vec::new();

        if !input.at_punctuation(PunctuationKind::RightParen)
            && !input.at_punctuation(PunctuationKind::Semicolon)
        {
            loop {
                let (expression, rest) =
                    parse_expression_handle_in(syntax_trees, input, context.value())?;
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

    pub(crate) fn parse_postfix_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        let (expression, input) = parse_primary_expression_handle(syntax_trees, input, context)?;
        parse_postfix_suffixes_handle(syntax_trees, expression, input, context)
    }

    fn parse_postfix_suffixes_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        mut expression: ExpressionHandle,
        mut input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        loop {
            let can_call = matches!(
                exprs_of(syntax_trees).expression(expression),
                ExpressionNode::Name(_) | ExpressionNode::Member(_)
            );
            if can_call
                && input.at_punctuation(PunctuationKind::Less)
                && let Some((machine_arguments, rest)) =
                    try_parse_static_machine_arguments(syntax_trees, input)?
            {
                let after_open = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
                let ((arguments, evidence_arguments), rest) =
                    parse_arguments_in(syntax_trees, after_open, context)?;
                input = rest;
                expression = build_call_expression_handle(
                    syntax_trees,
                    expression,
                    machine_arguments,
                    arguments,
                    evidence_arguments,
                )?;
                continue;
            }

            if can_call && input.at_punctuation(PunctuationKind::LeftParen) {
                input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
                let ((arguments, evidence_arguments), rest) =
                    parse_arguments_in(syntax_trees, input, context)?;
                input = rest;
                expression = build_call_expression_handle(
                    syntax_trees,
                    expression,
                    Box::default(),
                    arguments,
                    evidence_arguments,
                )?;
                continue;
            }

            if input.at_punctuation(PunctuationKind::LeftBracket) {
                let operator_span = input
                    .tokens
                    .first()
                    .map(|token| input.source_span(token))
                    .expect("recognized index punctuation has a source token");
                input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
                let (index, rest) =
                    parse_index_or_range_expression_handle(syntax_trees, input, context.value())?;
                input = rest.take_punctuation(PunctuationKind::RightBracket, "]")?;
                expression =
                    exprs(syntax_trees).insert(ExpressionNode::Indexed(TableIndexedExpression {
                        collection: expression,
                        index,
                    }));
                exprs(syntax_trees).set_source_span(expression, operator_span);
                continue;
            }

            if input.at_punctuation(PunctuationKind::Dot) {
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

                if member.as_str(syntax_trees) == "load"
                    && rest.at_punctuation(PunctuationKind::LeftParen)
                {
                    let after_open = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
                    if !after_open.at_punctuation(PunctuationKind::RightParen) {
                        let (ordering_expression, after_ord) =
                            parse_expression_handle(syntax_trees, after_open)?;
                        if after_ord.at_punctuation(PunctuationKind::RightParen) {
                            let ordering =
                                memory_ordering_from_expression(syntax_trees, ordering_expression)
                                    .map_err(|reason| after_open.error_here(reason))?;
                            if !ordering.valid_for_load() {
                                return Err(after_open.error_here(format!(
                                "atomic load cannot use `{}` ordering; use `NoOrdering`, `Receive`, or `GlobalOrder`",
                                ordering.name()
                            )));
                            }
                            input = after_ord.take_punctuation(PunctuationKind::RightParen, ")")?;
                            expression = exprs(syntax_trees).insert(ExpressionNode::Atomic(
                            syntax_trees::expression::TableAtomicExpression {
                                value: expression,
                                result: ExpressionHandle::invalid(),
                                ordering: language_core::atomic::AtomicOrderingPlan::Load(ordering),
                                result_custody:
                                    language_core::atomic::AtomicExpressionResultCustody::Scalar,
                            },
                        ));
                            continue;
                        }
                    }
                    return Err(after_open.error_here(
                    "`load` takes exactly one ordering argument: e.g. `self.counter.load(NoOrdering)`",
                ));
                }

                input = rest;
                let member_span = member.source_span();
                expression =
                    exprs(syntax_trees).insert(ExpressionNode::Member(TableMemberExpression {
                        receiver: expression,
                        access_source_span,
                        member,
                        case_variant: None,
                    }));
                exprs(syntax_trees).set_source_span(expression, member_span);
                continue;
            }

            if input.at_keyword(KeywordKind::As) {
                let cast_source = input.current_source_span();
                input = input.take_keyword(KeywordKind::As, "as")?;
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
                let (target_type, rest) =
                    parse_cast_target_type_reference_handle(syntax_trees, input)?;
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
                            let (arguments, rest) =
                                parse_domain_argument_handles(syntax_trees, rest)?;
                            semantic_domain_arguments = arguments;
                            input = rest;
                        }
                    }
                    if semantic_domain.is_empty() {
                        if rest.at_punctuation(PunctuationKind::Less) {
                            return Err(rest.error_here(
                                "compiler arithmetic domains do not take index arguments",
                            ));
                        }
                        input = rest;
                    }
                }
                expression =
                    exprs(syntax_trees).insert(ExpressionNode::Cast(TableCastExpression {
                        value: expression,
                        target_type,
                        target_label,
                        domain,
                        semantic_domain,
                        semantic_domain_arguments,
                        form,
                    }));
                exprs(syntax_trees).set_source_span(expression, cast_source);
                continue;
            }

            break;
        }

        Ok((expression, input))
    }

    fn parse_index_or_range_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        let (start, input, end_inclusive) = if let Some(end_inclusive) = range_separator(&input) {
            (ExpressionHandle::invalid(), input, end_inclusive)
        } else {
            let (start, input) = parse_expression_handle_in(syntax_trees, input, context)?;
            let Some(end_inclusive) = range_separator(&input) else {
                return Ok((start, input));
            };
            (start, input, end_inclusive)
        };

        let input = take_range_separator(input, end_inclusive)?;
        let (end, input) = if input.at_punctuation(PunctuationKind::RightBracket) {
            if end_inclusive {
                return Err(ParseError::new(
                    "inclusive range `..=` requires an end expression",
                ));
            }
            (ExpressionHandle::invalid(), input)
        } else {
            let (end, input) = parse_expression_handle_in(syntax_trees, input, context)?;
            (end, input)
        };
        Ok((
            exprs(syntax_trees).insert(ExpressionNode::Range(TableRangeExpression {
                start,
                end,
                end_inclusive,
            })),
            input,
        ))
    }

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
        machine_arguments: Box<[StaticMachineArgument]>,
        arguments: HandleSpan<ExpressionHandle>,
        evidence_arguments: Box<[Identifier]>,
    ) -> Result<ExpressionHandle, ParseError> {
        let expression = exprs_of(syntax_trees).expression(expression);
        let call = match expression {
            ExpressionNode::Name(path) => {
                let path = *path;
                let (target, receiver) = call_path_target_and_receiver(syntax_trees, path);
                let target = target.ok_or_else(|| ParseError::new("missing call target"))?;

                Ok(TableCallExpression {
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
                    target_is_static: false,
                    receiver: member.receiver,
                    target: member.member.clone(),
                    machine_arguments,
                    arguments,
                    evidence_arguments,
                    operational_acknowledgement: Default::default(),
                })
            }
            _ => Err(ParseError::new(
                "call target must be a path or member access",
            )),
        }?;
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
        let Ok((first, rest)) = input.take_identifier() else {
            return try_parse_static_value_argument(syntax_trees, input);
        };
        input = rest;
        let argument = if input.at_punctuation(PunctuationKind::Dot) {
            input = input.take_punctuation(PunctuationKind::Dot, ".")?;
            let (member, rest) = input.take_identifier()?;
            input = rest;
            StaticMachineArgument {
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                path: Box::default(),
                application: None,
                const_literal: None,
                evidence_projection: Some(syntax_trees::expression::EvidenceProjection {
                    term: first,
                    member,
                }),
            }
        } else {
            let mut path = vec![first];
            while input.at_punctuation(PunctuationKind::ColonColon) {
                let after_separator = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
                let Ok((member, rest)) = after_separator.take_identifier() else {
                    return Ok(None);
                };
                path.push(member);
                input = rest;
            }

            if input.at_punctuation(PunctuationKind::LeftBracket) || input.at_contextual("in") {
                return parse_static_type_argument(syntax_trees, authored).map(Some);
            }
            let application = if let Some((application, rest)) =
                try_parse_static_symbol_application(syntax_trees, input)?
            {
                input = rest;
                let mut application = application;
                if input.at_punctuation(PunctuationKind::ColonColon) {
                    let next = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
                    let Some((member, rest)) = try_parse_static_argument(syntax_trees, next)?
                    else {
                        return Ok(None);
                    };
                    application.member = exprs(syntax_trees).insert_static_member(member);
                    input = rest;
                }
                Some(application)
            } else {
                None
            };
            StaticMachineArgument {
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                path: path.into_boxed_slice(),
                application,
                const_literal: None,
                evidence_projection: None,
            }
        };

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
    use crate::grammar::expressions::context::ExpressionContext;
    use crate::grammar::expressions::match_dispatch::parse_match_expression_handle;
    use crate::grammar::expressions::parse_expression::parse_expression_handle_in;
    use crate::grammar::expressions::parse_postfix::parse_argument_list_after_open_paren_handle;
    use crate::grammar::expressions::parse_postfix::try_parse_static_machine_arguments;
    use crate::grammar::type_syntax::parse_type::parse_type_reference_handle;
    use crate::storage::tables::{exprs, exprs_of};
    use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
    use numerics::literals::IntegerLiteral;
    use source::{SourceSpan, Span};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{
        ExpressionHandle, ExpressionNode, TableCallExpression, TableStructLiteral,
        TableStructLiteralField,
    };
    use syntax_trees::identifier::Identifier;
    use syntax_trees::types::TypeReferenceHandle;
    use tokens::{KeywordKind, NumericLiteralKind, PunctuationKind, TokenKind};

    fn spanned(
        syntax_trees: &mut SyntaxTrees,
        node: ExpressionNode,
        source_span: SourceSpan,
    ) -> ExpressionHandle {
        let expression = exprs(syntax_trees).insert(node);
        exprs(syntax_trees).set_source_span(expression, source_span);
        expression
    }

    pub(crate) fn parse_primary_expression_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        if context.allows_type_expression() && starts_lifetime_reference_type(input) {
            let start = input;
            let (reference, rest) = parse_type_reference_handle(syntax_trees, input)?;
            let expression = spanned(
                syntax_trees,
                ExpressionNode::TypeExpression(reference),
                start.source_span_until(rest),
            );
            return Ok((expression, rest));
        }
        if input.at_keyword(KeywordKind::True) {
            let source_span = input
                .tokens
                .first()
                .map(|token| input.source_span(token))
                .expect("recognized boolean literal has a source token");
            let input = input.take_keyword(KeywordKind::True, "true")?;
            let expression = spanned(syntax_trees, ExpressionNode::Boolean(true), source_span);
            return Ok((expression, input));
        }

        if input.at_keyword(KeywordKind::False) {
            let source_span = input
                .tokens
                .first()
                .map(|token| input.source_span(token))
                .expect("recognized boolean literal has a source token");
            let input = input.take_keyword(KeywordKind::False, "false")?;
            let expression = spanned(syntax_trees, ExpressionNode::Boolean(false), source_span);
            return Ok((expression, input));
        }

        if input.at_keyword(KeywordKind::SelfValue) {
            if context.allows_logical_subject() {
                let source_span =
                    input.source_span(input.tokens.first().expect("logical subject token"));
                let input = input.take_keyword(KeywordKind::SelfValue, "self")?;
                let member = syntax_trees
                    .expressions
                    .append_identifier_path_member(Identifier::new("self", source_span));
                let expression = spanned(
                    syntax_trees,
                    ExpressionNode::Name(arena::HandleSpan::from_parts(member, 1)),
                    source_span,
                );
                return Ok((expression, input));
            }
            return Err(input.error_here(
                "implicit `self` is retired; name the explicit parameter or domain subject",
            ));
        }

        if input.at_keyword(KeywordKind::Match) {
            return parse_match_expression_handle(syntax_trees, input);
        }

        if input.at_contextual("zero_value") {
            let after_name = input.take_contextual("zero_value")?;
            if after_name.at_punctuation(PunctuationKind::Less) {
                let after_less = after_name.take_punctuation(PunctuationKind::Less, "<")?;
                let (type_reference, rest) = parse_type_reference_handle(syntax_trees, after_less)?;
                let rest = rest.take_punctuation(PunctuationKind::Greater, ">")?;
                let rest = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
                let rest = rest.take_punctuation(PunctuationKind::RightParen, ")")?;
                return Ok((
                    exprs(syntax_trees).insert(ExpressionNode::ZeroValue(type_reference)),
                    rest,
                ));
            }
        }

        if input.tokens.first().is_some_and(|token| {
            matches!(
                token.kind,
                TokenKind::NumericLiteral(NumericLiteralKind::Integer(_))
            )
        }) {
            let source_span = input
                .tokens
                .first()
                .map(|token| input.source_span(token))
                .expect("recognized integer literal has a source token");
            let (literal, input) = input.take_integer_literal()?;
            let expression = spanned(syntax_trees, ExpressionNode::Integer(literal), source_span);
            return Ok((expression, input));
        }

        if input.tokens.first().is_some_and(|token| {
            matches!(
                token.kind,
                TokenKind::NumericLiteral(NumericLiteralKind::Float(_))
            )
        }) {
            let source_span = input.current_source_span();
            let (value, input) = input.take_float_text()?;
            let expression = spanned(syntax_trees, ExpressionNode::Float(value), source_span);
            return Ok((expression, input));
        }

        if input.at_contextual("utf16") && input.at_contextual_then_string("utf16") {
            let start = input;
            let input = input.take_contextual("utf16")?;
            let (value, input) = input.take_string()?;
            let units: Vec<_> = value
                .encode_utf16()
                .map(|unit| {
                    exprs(syntax_trees).insert(ExpressionNode::Integer(IntegerLiteral::from_value(
                        i64::from(unit),
                    )))
                })
                .collect();
            let units = exprs(syntax_trees).insert_expression_handles(units);
            let expression = syntax_trees
                .expressions
                .insert(ExpressionNode::ArrayLiteral(units));
            exprs(syntax_trees).set_source_span(expression, start.source_span_until(input));
            return Ok((expression, input));
        }

        if input
            .tokens
            .first()
            .is_some_and(|token| token.kind == TokenKind::StringLiteral)
        {
            let (value, input) = input.take_string_bytes()?;
            return Ok((
                exprs(syntax_trees).insert(ExpressionNode::String(value)),
                input,
            ));
        }

        if input.at_punctuation(PunctuationKind::LeftParen) {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (expression, input) =
                parse_expression_handle_in(syntax_trees, input, context.grouped())?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            return Ok((expression, input));
        }

        if input.at_punctuation(PunctuationKind::LeftBracket) {
            if context.allows_type_expression() && starts_fixed_array_type(input) {
                let start = input;
                let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
                let expression = syntax_trees
                    .expressions
                    .insert(ExpressionNode::TypeExpression(type_reference));
                exprs(syntax_trees).set_source_span(expression, start.source_span_until(input));
                return Ok((expression, input));
            }
            let start = input;
            let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
            let mut values = Vec::new();

            if !input.at_punctuation(PunctuationKind::RightBracket) {
                loop {
                    let (value, rest) = parse_expression_handle_in(
                        syntax_trees,
                        input,
                        ExpressionContext::Default,
                    )?;
                    values.push(value);
                    input = rest;
                    let Some(next) = input.take_comma()? else {
                        break;
                    };
                    input = next;
                }
            }

            let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
            let values = exprs(syntax_trees).insert_expression_handles(values);
            let expression = syntax_trees
                .expressions
                .insert(ExpressionNode::ArrayLiteral(values));
            exprs(syntax_trees).set_source_span(expression, start.source_span_until(input));
            return Ok((expression, input));
        }

        if context.allows_struct_literal() && input.at_contextual("spawn") {
            let after_spawn = input.take_contextual("spawn")?;
            if after_spawn.at_punctuation(PunctuationKind::LeftBrace) {
                return Err(input.error_here(
                    "`spawn { ... }` is retired: start an ordinary named machine through an \
                 admitted task-runtime capability (`runtime.start<Worker::run>(...)`) and \
                 keep the returned linear `Task<T>`",
                ));
            }
        }

        if input.at_name_like() {
            let start = input;
            if let Some((application, rest)) = try_applied_requirement_head(syntax_trees, start) {
                return parse_applied_requirement_call(syntax_trees, start, application, rest);
            }
            let (path, input) = parse_path_handle_span(input, |member| {
                exprs(syntax_trees).append_identifier_path_member(member)
            })?;

            if context.allows_type_expression()
                && input.at_punctuation(PunctuationKind::Less)
                && let Some((type_reference, rest)) =
                    try_generic_type_operand(syntax_trees, start, !context.allows_struct_literal())
            {
                let expression = syntax_trees
                    .expressions
                    .insert(ExpressionNode::TypeExpression(type_reference));
                exprs(syntax_trees).set_source_span(expression, start.source_span_until(rest));
                return Ok((expression, rest));
            }

            if context.allows_struct_literal() && input.at_punctuation(PunctuationKind::LeftBrace) {
                let members = exprs_of(syntax_trees).identifier_path_members(path);
                let first = members.first().expect("constructor path has a head");
                let last = members.last().expect("constructor path has a tail");
                let mut source_span = first.source_span();
                source_span.span.end = last.source_span().span.end;
                let mut name = first.as_str(syntax_trees).to_owned();
                for member in &members[1..] {
                    name.push_str("::");
                    name.push_str(member.as_str(syntax_trees));
                }
                let constructor_name = Identifier::new(name, source_span);
                return parse_struct_literal_handle(syntax_trees, constructor_name, input, context);
            }

            let expression = exprs(syntax_trees).insert(ExpressionNode::Name(path));
            exprs(syntax_trees).set_source_span(expression, start.source_span_until(input));
            return Ok((expression, input));
        }

        Err(input.error_here("expected expression"))
    }

    fn try_applied_requirement_head<'tokens, 'source>(
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
            let (reference, rest) = parse_type_reference_handle(syntax, input).ok()?;
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

    fn parse_applied_requirement_call<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        application: TypeReferenceHandle,
        rest: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        let start = input;
        let receiver = syntax_trees
            .expressions
            .insert(ExpressionNode::TypeExpression(application));
        exprs(syntax_trees).set_source_span(receiver, start.source_span_until(rest));
        let rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (target, rest) = rest.take_identifier()?;
        let (machine_arguments, rest) =
            match try_parse_static_machine_arguments(syntax_trees, rest)? {
                Some((machine_arguments, rest)) => (machine_arguments, rest),
                None => (Box::default(), rest),
            };
        let rest = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let ((arguments, evidence_arguments), rest) =
            parse_argument_list_after_open_paren_handle(syntax_trees, rest)?;
        let source = target.source_span();
        let call = exprs(syntax_trees).insert(ExpressionNode::Call(TableCallExpression {
            receiver,
            target,
            target_is_static: true,
            machine_arguments,
            arguments,
            evidence_arguments,
            operational_acknowledgement: Default::default(),
        }));
        exprs(syntax_trees).set_source_span(call, source);
        Ok((call, rest))
    }

    fn try_generic_type_operand<'tokens, 'source>(
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

    fn starts_fixed_array_type(input: Input<'_, '_>) -> bool {
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

    fn parse_struct_literal_handle<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        constructor_name: Identifier,
        mut input: Input<'tokens, 'source>,
        context: ExpressionContext,
    ) -> ParseResult<'tokens, 'source, ExpressionHandle> {
        let start = input;
        let type_name_span = constructor_name.source_span();
        input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        let mut fields = Vec::new();

        while !input.at_punctuation(PunctuationKind::RightBrace) {
            let (name, rest) = input.take_identifier()?;
            input = rest.take_punctuation(PunctuationKind::Colon, ":")?;
            let (value, rest) = parse_expression_handle_in(syntax_trees, input, context.value())?;
            input = rest;
            fields.push(TableStructLiteralField { name, value });

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }
        }

        let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        let fields = exprs(syntax_trees).insert_struct_fields(fields);
        let expression = syntax_trees
            .expressions
            .insert(ExpressionNode::StructLiteral(TableStructLiteral {
                application_type: Default::default(),
                constructor_name,
                fields,
            }));
        let literal_tail_span = start.source_span_until(input);
        debug_assert_eq!(type_name_span.source_id, literal_tail_span.source_id);
        exprs(syntax_trees).set_source_span(
            expression,
            SourceSpan::new(
                type_name_span.source_id,
                Span::new(type_name_span.span.start, literal_tail_span.span.end),
            ),
        );
        Ok((expression, input))
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
