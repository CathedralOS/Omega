/*
Statement grammar (`grammar::bodies::statements`): local data and atomic
bindings, destructuring and proof outputs, statement tables, and inline
assembly blocks and scratch specifications.
*/

pub(crate) mod statements {
    pub(crate) mod atomic_lets {
        use crate::grammar::bodies::statements::statement_tables::copy_expression_as_place;
        use crate::grammar::expressions::parse_postfix::memory_ordering_from_expression;
        use crate::storage::tables::{exprs, exprs_of, items};
        use arena::HandleSpan;
        use language_core::atomic::AtomicOrderingPlan;
        use numerics::literals::IntegerLiteral;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, TableAtomicExpression,
            TableBinaryExpression,
        };
        use syntax_trees::identifier::Identifier;
        use syntax_trees::statement::{
            StatementHandle, StatementNode, TableAssignment, TableLocalData,
        };

        enum AtomicUpdate {
            Fetch {
                operand: ExpressionHandle,
                operator: BinaryOperator,
            },
            Swap {
                replacement: ExpressionHandle,
            },
            CompareExchange {
                expected: ExpressionHandle,
                replacement: ExpressionHandle,
            },
        }

        pub(crate) fn try_desugar_atomic_let(
            syntax_trees: &mut SyntaxTrees,
            binding: &TableLocalData,
        ) -> Option<HandleSpan<StatementHandle>> {
            let ExpressionNode::Call(call) = syntax_trees
                .expressions
                .expression(binding.initial_value)
                .clone()
            else {
                return None;
            };
            if call.target_is_static || !call.receiver.is_valid() {
                return None;
            }
            let arguments = exprs_of(syntax_trees).expression_handles(call.arguments);
            if let Some(observation) =
                language_core::atomic::AtomicCompareExchangeObservationAxis::single_attempt_member(
                    call.target.as_str(syntax_trees),
                )
            {
                let [expected, replacement, success, failure] = arguments else {
                    return None;
                };
                let ordering = AtomicOrderingPlan::CompareExchangeOnce {
                    success: memory_ordering_from_expression(syntax_trees, *success).ok()?,
                    failure: memory_ordering_from_expression(syntax_trees, *failure).ok()?,
                };
                let operands = [*expected, *replacement];
                let mut carrier = call;
                carrier.arguments = exprs(syntax_trees).insert_expression_handles(operands);
                let value = syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(carrier));
                let mut local = binding.clone();
                local.initial_value = syntax_trees
            .expressions
            .insert(ExpressionNode::Atomic(TableAtomicExpression {
            value,
            result: ExpressionHandle::invalid(),
            ordering,
            result_custody:
                language_core::atomic::AtomicExpressionResultCustody::CompareExchangeOnce(
                    language_core::atomic::AtomicCompareExchangeOnceResultCustody::for_observation(
                        observation,
                    ),
                ),
        }));
                let statement = syntax_trees
                    .statements
                    .insert(StatementNode::LocalData(local));
                let first = items(syntax_trees).append_statement_handle(statement);
                return Some(HandleSpan::from_parts(first, 1));
            }
            let (update, ordering) = match (call.target.as_str(syntax_trees), arguments) {
                (
                    "fetch_add" | "fetch_sub" | "fetch_xor" | "fetch_or" | "fetch_and",
                    [operand, ordering],
                ) => {
                    let operator = match call.target.as_str(syntax_trees) {
                        "fetch_add" => BinaryOperator::Add,
                        "fetch_sub" => BinaryOperator::Subtract,
                        "fetch_xor" => BinaryOperator::BitwiseXor,
                        "fetch_or" => BinaryOperator::BitwiseOr,
                        "fetch_and" => BinaryOperator::BitwiseAnd,
                        _ => return None,
                    };
                    (
                        AtomicUpdate::Fetch {
                            operand: *operand,
                            operator,
                        },
                        AtomicOrderingPlan::ReadModifyWrite(
                            memory_ordering_from_expression(syntax_trees, *ordering).ok()?,
                        ),
                    )
                }
                ("swap", [replacement, ordering]) => (
                    AtomicUpdate::Swap {
                        replacement: *replacement,
                    },
                    AtomicOrderingPlan::Swap(
                        memory_ordering_from_expression(syntax_trees, *ordering).ok()?,
                    ),
                ),
                ("compare_exchange", [expected, replacement, success, failure]) => (
                    AtomicUpdate::CompareExchange {
                        expected: *expected,
                        replacement: *replacement,
                    },
                    AtomicOrderingPlan::CompareExchange {
                        success: memory_ordering_from_expression(syntax_trees, *success).ok()?,
                        failure: memory_ordering_from_expression(syntax_trees, *failure).ok()?,
                    },
                ),
                _ => return None,
            };
            let target = copy_expression_as_place(syntax_trees, call.receiver)?;

            let mut result_binding = binding.clone();
            result_binding.role = language_core::LocalBindingRole::AtomicObservedResult;
            result_binding.initial_value = syntax_trees
                .expressions
                .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
            let result = result_reference(syntax_trees, &binding.name);
            let value = match update {
                AtomicUpdate::Fetch { operand, operator } => {
                    exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: result,
                        operator,
                        right: operand,
                    }))
                }
                AtomicUpdate::Swap { replacement } => replacement,
                AtomicUpdate::CompareExchange {
                    expected,
                    replacement,
                } => {
                    let prior_for_subtract = result_reference(syntax_trees, &binding.name);
                    let difference =
                        exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: replacement,
                            operator: BinaryOperator::Subtract,
                            right: prior_for_subtract,
                        }));
                    let prior_for_compare = result_reference(syntax_trees, &binding.name);
                    let comparison =
                        exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: prior_for_compare,
                            operator: BinaryOperator::Equal,
                            right: expected,
                        }));
                    let replacement_delta =
                        exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: comparison,
                            operator: BinaryOperator::Multiply,
                            right: difference,
                        }));
                    exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: result,
                        operator: BinaryOperator::Add,
                        right: replacement_delta,
                    }))
                }
            };
            let value =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Atomic(TableAtomicExpression {
                        value,
                        result,
                        ordering,
                        result_custody:
                            language_core::atomic::AtomicExpressionResultCustody::Scalar,
                    }));
            let local = syntax_trees
                .statements
                .insert(StatementNode::LocalData(result_binding));
            let first = items(syntax_trees).append_statement_handle(local);
            let assignment = syntax_trees
                .statements
                .insert(StatementNode::Assignment(TableAssignment { target, value }));
            items(syntax_trees).append_statement_handle(assignment);
            Some(HandleSpan::from_parts(first, 2))
        }

        fn result_reference(syntax_trees: &mut SyntaxTrees, name: &Identifier) -> ExpressionHandle {
            let name = Identifier::generated(name.as_str(syntax_trees));
            let member = exprs(syntax_trees).append_identifier_path_member(name);
            let path = HandleSpan::from_parts(member, 1);
            exprs(syntax_trees).insert(ExpressionNode::Name(path.into()))
        }

        pub(crate) fn try_desugar_atomic_store(
            syntax_trees: &mut SyntaxTrees,
            expression: ExpressionHandle,
        ) -> Option<TableAssignment> {
            let ExpressionNode::Call(call) = exprs_of(syntax_trees).expression(expression).clone()
            else {
                return None;
            };
            if call.target_is_static
                || call.target.as_str(syntax_trees) != "store"
                || !call.receiver.is_valid()
            {
                return None;
            }
            let argument_count = syntax_trees
                .tables
                .expressions
                .expression_handles(call.arguments)
                .len();
            if argument_count != 2 {
                return None;
            }
            let arguments = syntax_trees
                .tables
                .expressions
                .expression_handles(call.arguments);
            let value = arguments[0];
            let ordering = memory_ordering_from_expression(syntax_trees, arguments[1]).ok()?;
            let value =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Atomic(TableAtomicExpression {
                        value,
                        result: ExpressionHandle::invalid(),
                        ordering: language_core::atomic::AtomicOrderingPlan::Store(ordering),
                        result_custody:
                            language_core::atomic::AtomicExpressionResultCustody::Scalar,
                    }));
            let receiver = call.receiver;
            Some(TableAssignment {
                target: receiver,
                value,
            })
        }
    }
    pub(crate) mod destructure_and_proof_output {
        use crate::diagnostics::parse_error::ParseError;
        use crate::grammar::expressions::parse_expression::parse_expression_handle;
        use crate::storage::tables::{exprs, exprs_of, items, stmts};
        use crate::tokens::token_cursor::Input;
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::ExpressionHandle;
        use syntax_trees::identifier::Identifier;
        use syntax_trees::statement::{StatementHandle, StatementNode, TableLocalData};
        use tokens::{KeywordKind, PunctuationKind};

        pub(crate) fn try_parse_destructure_let<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> Option<(HandleSpan<StatementHandle>, Input<'tokens, 'source>)> {
            use syntax_trees::expression::{
                ExpressionNode, MatchFieldAction, TableMatchField, TableMemberExpression,
            };

            if !input.at_keyword(KeywordKind::Let) {
                return None;
            }
            let after_let = input.take_keyword(KeywordKind::Let, "let").ok()?;
            if !after_let.at_punctuation(PunctuationKind::LeftBrace) {
                return None;
            }
            let mut rest = after_let
                .take_punctuation(PunctuationKind::LeftBrace, "{")
                .ok()?;
            let mut fields: Vec<(Identifier, Option<Identifier>)> = Vec::new();
            loop {
                if rest.at_punctuation(PunctuationKind::RightBrace) {
                    rest = rest
                        .take_punctuation(PunctuationKind::RightBrace, "}")
                        .ok()?;
                    break;
                }
                let (field, after_field) = rest.take_identifier().ok()?;
                let mut binding = Some(field.clone());
                let mut after_binding = after_field;
                if after_binding.at_keyword(KeywordKind::As) {
                    let after_as = after_binding.take_keyword(KeywordKind::As, "as").ok()?;
                    if after_as.at_contextual("_") {
                        binding = None;
                        after_binding = after_as.take_contextual("_").ok()?;
                    } else {
                        let (renamed, after_renamed) = after_as.take_identifier().ok()?;
                        if renamed.as_str(syntax_trees) == "_" {
                            binding = None;
                        } else {
                            binding = Some(renamed);
                        }
                        after_binding = after_renamed;
                    }
                }
                fields.push((field, binding));
                if after_binding.at_punctuation(PunctuationKind::Comma) {
                    rest = after_binding
                        .take_punctuation(PunctuationKind::Comma, ",")
                        .ok()?;
                } else {
                    rest = after_binding;
                }
            }
            if fields.is_empty() {
                return None;
            }
            let rest = rest.take_punctuation(PunctuationKind::Equal, "=").ok()?;
            let (value, rest) = parse_expression_handle(syntax_trees, rest).ok()?;
            let rest = rest
                .take_punctuation(PunctuationKind::Semicolon, ";")
                .ok()?;
            fn is_place(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> bool {
                match exprs_of(syntax_trees).expression(expression) {
                    ExpressionNode::Name(_) => true,
                    ExpressionNode::Member(member) => is_place(syntax_trees, member.receiver),
                    _ => false,
                }
            }
            if !is_place(syntax_trees, value) {
                return None;
            }

            /*
                The marker local carries the pattern itself: every field the
                author named, bound or waived, in authored order. Its name
                says nothing, so coverage is decided on this roster once the
                subject's type has given each entry its field.
            */
            let roster = fields
                .iter()
                .map(|(field, binding)| TableMatchField {
                    component: language_core::PatternComponent::Fields,
                    member: field.clone(),
                    action: match binding {
                        Some(binding) => MatchFieldAction::Bind(binding.clone()),
                        None => MatchFieldAction::Waive,
                    },
                })
                .collect::<Vec<_>>();
            let pattern_fields = exprs(syntax_trees).insert_match_fields(roster);
            let marker = syntax_trees
                .statements
                .insert(StatementNode::LocalData(TableLocalData {
                    role: language_core::LocalBindingRole::RecordPattern,
                    name: Identifier::generated("__destructure"),
                    type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                    initial_value: value,
                    is_mutable: false,
                    relevance: language_core::BindingRelevance::Relevant,
                    pattern_fields,
                }));
            let marker = items(syntax_trees).append_statement_handle(marker);
            let mut statements = crate::storage::handle_spans::HandleSpanBuilder::new();
            statements.push(marker);

            for (field, binding) in fields {
                let Some(binding) = binding else {
                    continue;
                };
                let member =
                    exprs(syntax_trees).insert(ExpressionNode::Member(TableMemberExpression {
                        receiver: value,
                        access_source_span: Default::default(),
                        member: field,
                        case_variant: None,
                    }));
                let statement =
                    syntax_trees
                        .statements
                        .insert(StatementNode::LocalData(TableLocalData {
                            role: language_core::LocalBindingRole::Value,
                            name: binding,
                            type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                            initial_value: member,
                            is_mutable: false,
                            relevance: language_core::BindingRelevance::Relevant,
                            pattern_fields: Default::default(),
                        }));
                statements.push(items(syntax_trees).append_statement_handle(statement));
            }

            Some((statements.finish(), rest))
        }

        pub(crate) fn try_parse_proof_output_binding<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> Option<(HandleSpan<StatementHandle>, Input<'tokens, 'source>)> {
            use syntax_trees::expression::ExpressionNode;
            use syntax_trees::statement::{
                TableProofOutputBindingStatement, TableProofOutputSelector,
            };

            let mut rest = input.take_keyword(KeywordKind::Let, "let").ok()?;
            rest = rest
                .take_punctuation(PunctuationKind::LeftParen, "(")
                .ok()?;
            let mut bindings = Vec::new();

            if !rest.at_punctuation(PunctuationKind::Semicolon) {
                let (binding, next) = rest.take_identifier().ok()?;
                bindings.push(TableProofOutputSelector {
                    output_field: Identifier::generated("value"),
                    binding,
                });
                rest = next;
            }
            rest = rest
                .take_punctuation(PunctuationKind::Semicolon, ";")
                .ok()?;

            while !rest.at_punctuation(PunctuationKind::RightParen) {
                let (output_field, next) = rest.take_identifier().ok()?;
                rest = next.take_punctuation(PunctuationKind::Colon, ":").ok()?;
                let (binding, next) = rest.take_identifier().ok()?;
                bindings.push(TableProofOutputSelector {
                    output_field,
                    binding,
                });
                if next.at_punctuation(PunctuationKind::Comma) {
                    rest = next.take_punctuation(PunctuationKind::Comma, ",").ok()?;
                } else if next.at_punctuation(PunctuationKind::RightParen) {
                    rest = next;
                } else {
                    return None;
                }
            }
            rest = rest
                .take_punctuation(PunctuationKind::RightParen, ")")
                .ok()?;
            if bindings.is_empty() {
                return None;
            }
            rest = rest.take_punctuation(PunctuationKind::Equal, "=").ok()?;
            let (call, next) = parse_expression_handle(syntax_trees, rest).ok()?;
            if !matches!(
                exprs_of(syntax_trees).expression(call),
                ExpressionNode::Call(_)
            ) {
                return None;
            }
            rest = next
                .take_punctuation(PunctuationKind::Semicolon, ";")
                .ok()?;
            let statement = stmts(syntax_trees).insert(StatementNode::ProofOutputBindingStatement(
                TableProofOutputBindingStatement {
                    bindings: bindings.into_boxed_slice(),
                    call,
                },
            ));
            let statement = items(syntax_trees).append_statement_handle(statement);
            Some((HandleSpan::from_parts(statement, 1), rest))
        }

        pub(crate) fn reject_retired_proof_output_binding(
            input: Input<'_, '_>,
        ) -> Result<(), ParseError> {
            let Ok(rest) = input.take_keyword(KeywordKind::Let, "let") else {
                return Ok(());
            };
            let Ok(rest) = rest.take_punctuation(PunctuationKind::LeftBrace, "{") else {
                return Ok(());
            };
            let Ok((_, rest)) = rest.take_identifier() else {
                return Ok(());
            };
            if rest.at_punctuation(PunctuationKind::Colon) {
                return Err(input.error_here(
            "generated proof-output packages are retired; bind the ordinary result and selected proofs as `let (value; public_output: local_term) = call();`, or use `let (; public_output: local_term) = call();` for an evidence-only result",
        ));
            }
            Ok(())
        }
    }
    pub(crate) mod discard_and_local_data {
        use crate::grammar::bodies::statements::statement_tables::expression_handle_to_statement_call;
        use crate::grammar::expressions::parse_expression::parse_expression_handle;
        use crate::grammar::type_syntax::parse_type::parse_type_reference_handle_allowing_borrow;
        use crate::storage::tables::{exprs, exprs_of, stmts};
        use crate::tokens::token_cursor::{Input, ParseResult};
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::{
            ExpressionHandle, ExpressionNode, TableIndexedExpression, TableMemberExpression,
        };
        use syntax_trees::statement::{StatementHandle, StatementNode, TableLocalData};
        use tokens::PunctuationKind;

        pub(crate) fn parse_discard_statement_handle<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, StatementHandle> {
            let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
            let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
            let rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;

            let Some(mut call) = expression_handle_to_statement_call(syntax_trees, expression)
            else {
                return Err(
                    input.error_here("`_ =` discards a call result; only a call can follow `_ =`")
                );
            };
            call.discards_result = true;

            Ok((stmts(syntax_trees).insert(StatementNode::Call(call)), rest))
        }

        pub(crate) fn parse_if_transition_statement_handle<'tokens, 'source>(
            _syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, StatementHandle> {
            Err(input.error_here(
        "the `if` statement is retired; dispatch is `transition <guard> { true -> ... _ -> ... }` \
         (every arm set must provably cover all cases)",
    ))
        }

        pub(crate) fn copy_compound_assignment_target(
            syntax_trees: &mut SyntaxTrees,
            expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            let copy = match exprs_of(syntax_trees).expression(expression).clone() {
                node @ (ExpressionNode::Name(_) | ExpressionNode::Member(_)) => {
                    copied_name_or_member(syntax_trees, node)?
                }
                ExpressionNode::Indexed(indexed) => {
                    let collection =
                        copy_compound_assignment_target(syntax_trees, indexed.collection)?;
                    let index = copy_stable_compound_assignment_index(syntax_trees, indexed.index)?;
                    ExpressionNode::Indexed(TableIndexedExpression { collection, index })
                }
                _ => return None,
            };

            Some(exprs(syntax_trees).insert(copy))
        }

        fn copied_name_or_member(
            syntax_trees: &mut SyntaxTrees,
            node: ExpressionNode,
        ) -> Option<ExpressionNode> {
            Some(match node {
                ExpressionNode::Name(path) if path.application.is_none() => ExpressionNode::Name(
                    exprs(syntax_trees)
                        .copy_identifier_path_prefix(path.members, path.members.len())
                        .into(),
                ),
                ExpressionNode::Member(member) => ExpressionNode::Member(TableMemberExpression {
                    receiver: copy_compound_assignment_target(syntax_trees, member.receiver)?,
                    access_source_span: member.access_source_span,
                    member: member.member,
                    case_variant: member.case_variant.clone(),
                }),
                _ => return None,
            })
        }

        fn copy_stable_compound_assignment_index(
            syntax_trees: &mut SyntaxTrees,
            expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            let copy = match exprs_of(syntax_trees).expression(expression).clone() {
                ExpressionNode::Boolean(value) => ExpressionNode::Boolean(value),
                ExpressionNode::Integer(value) => ExpressionNode::Integer(value),
                node @ (ExpressionNode::Name(_) | ExpressionNode::Member(_)) => {
                    copied_name_or_member(syntax_trees, node)?
                }
                _ => return None,
            };

            Some(exprs(syntax_trees).insert(copy))
        }

        pub(crate) fn parse_local_data<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, TableLocalData> {
            let (is_mutable, input) = if input.at_contextual("mut")
                && input
                    .take_contextual("mut")
                    .is_ok_and(|rest| rest.at_name_like())
            {
                (true, input.take_contextual("mut")?)
            } else {
                (false, input)
            };
            let (name, input) = input.take_identifier()?;
            let (relevance, input) =
                crate::grammar::parameters::binding_properties::parse_binding_relevance_brackets(
                    input, "local",
                )?;
            let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, input) =
                parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
            let (initial_value, input) = if input.at_punctuation(PunctuationKind::Equal) {
                let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
                let (expression, input) = parse_expression_handle(syntax_trees, input)?;
                (expression, input)
            } else {
                (ExpressionHandle::invalid(), input)
            };
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;

            Ok((
                TableLocalData {
                    role: language_core::LocalBindingRole::Value,
                    name,
                    type_reference,
                    initial_value,
                    is_mutable,
                    relevance,
                    pattern_fields: Default::default(),
                },
                input,
            ))
        }
    }
    pub(crate) mod inline_assembly {
        pub(crate) mod scratch {
            use super::*;
            use numerics::{arithmetic::ArithmeticDomain, literals::IntegerLiteral};
            use syntax_trees::expression::{
                BinaryOperator, TableBinaryExpression, TableCastExpression,
            };
            use syntax_trees::statement::TableLocalData;
            use syntax_trees::types::TypeReferenceNode;

            pub(crate) use language_core::inline_assembly::ASM_SCRATCH_NAMES as NAMES;
            use language_core::inline_assembly::{ASM_SCRATCH_SLOT_COUNT, ASM_ZERO_FLAG_SLOT};
            use numerics::literals::IntegerRadix;

            pub(crate) fn name(trees: &mut SyntaxTrees, value: &str) -> ExpressionHandle {
                let member = trees
                    .expressions
                    .append_identifier_path_member(Identifier::generated(value));
                trees.expressions.insert(ExpressionNode::Name(
                    (HandleSpan::from_parts(member, 1)).into(),
                ))
            }

            pub(crate) fn unsigned_type(
                trees: &mut SyntaxTrees,
            ) -> syntax_trees::types::TypeReferenceHandle {
                trees
                    .type_references
                    .insert(TypeReferenceNode::Named(Identifier::generated("u32")))
            }

            fn cast_to(
                trees: &mut SyntaxTrees,
                value: ExpressionHandle,
                target_type: syntax_trees::types::TypeReferenceHandle,
                domain: ArithmeticDomain,
            ) -> ExpressionHandle {
                trees
                    .expressions
                    .insert(ExpressionNode::Cast(TableCastExpression {
                        value,
                        target_type,
                        target_label: HandleSpan::empty(),
                        domain,
                        semantic_domain: HandleSpan::empty(),
                        semantic_domain_arguments: HandleSpan::empty(),
                        form: Default::default(),
                    }))
            }

            fn cast(
                trees: &mut SyntaxTrees,
                value: ExpressionHandle,
                domain: ArithmeticDomain,
            ) -> ExpressionHandle {
                let target_type = unsigned_type(trees);
                cast_to(trees, value, target_type, domain)
            }

            pub(crate) fn subtraction(
                trees: &mut SyntaxTrees,
                destination: ExpressionHandle,
                source: ExpressionHandle,
            ) -> ExpressionHandle {
                modular_binary(trees, destination, source, BinaryOperator::Subtract)
            }

            pub(crate) fn addition(
                trees: &mut SyntaxTrees,
                destination: ExpressionHandle,
                source: ExpressionHandle,
            ) -> ExpressionHandle {
                modular_binary(trees, destination, source, BinaryOperator::Add)
            }

            #[derive(Clone, Copy, PartialEq, Eq)]
            pub(crate) enum ArithmeticFlagProducer {
                Subtract,
                Add,
            }

            fn modular_binary(
                trees: &mut SyntaxTrees,
                destination: ExpressionHandle,
                source: ExpressionHandle,
                operator: BinaryOperator,
            ) -> ExpressionHandle {
                let left = cast(trees, destination, ArithmeticDomain::Wrapping);
                let right = cast(trees, source, ArithmeticDomain::Wrapping);
                let result =
                    trees
                        .expressions
                        .insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left,
                            operator,
                            right,
                        }));
                cast(trees, result, ArithmeticDomain::Exact)
            }

            pub(crate) fn zero_flag(
                trees: &mut SyntaxTrees,
                destination: ExpressionHandle,
            ) -> StatementHandle {
                let target = name(trees, "zf");
                let zero = trees
                    .expressions
                    .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
                let value =
                    trees
                        .expressions
                        .insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: destination,
                            operator: BinaryOperator::Equal,
                            right: zero,
                        }));
                trees
                    .statements
                    .insert(StatementNode::Assignment(TableAssignment { target, value }))
            }

            pub(crate) fn bind(
                trees: &mut SyntaxTrees,
                first: usize,
            ) -> (
                [ExpressionHandle; ASM_SCRATCH_SLOT_COUNT],
                Vec<StatementHandle>,
            ) {
                let mut registers = [ExpressionHandle::invalid(); ASM_SCRATCH_SLOT_COUNT];
                let mut bindings = Vec::new();
                let names: Vec<_> = trees
                    .expressions
                    .iter_expressions()
                    .skip(first)
                    .filter_map(|(handle, node)| {
                        let ExpressionNode::Name(path) = node else {
                            return None;
                        };
                        let Some(members) = path.application.is_none().then_some(path.members)
                        else {
                            return None;
                        };
                        let [identifier] = trees.expressions.identifier_path_members(members)
                        else {
                            return None;
                        };
                        NAMES
                            .iter()
                            .position(|name| !name.is_empty() && *name == identifier.as_str(trees))
                            .map(|index| (handle, index))
                    })
                    .collect();
                for (handle, index) in names {
                    let hidden = format!("asm#scratch#{first}#{}", NAMES[index]);
                    if !registers[index].is_valid() {
                        registers[index] = name(trees, &hidden);
                        let type_reference = if [15, 16, 19, 20].contains(&index) {
                            wide_type(trees)
                        } else if index == ASM_ZERO_FLAG_SLOT {
                            trees
                                .type_references
                                .insert(TypeReferenceNode::Named(Identifier::generated("bool")))
                        } else {
                            unsigned_type(trees)
                        };
                        let initial_value =
                            trees.expressions.insert(if index == ASM_ZERO_FLAG_SLOT {
                                ExpressionNode::Boolean(false)
                            } else {
                                ExpressionNode::Integer(IntegerLiteral::zero())
                            });
                        bindings.push(trees.statements.insert(StatementNode::LocalData(
                            TableLocalData {
                                role: Default::default(),
                                name: Identifier::generated(&hidden),
                                type_reference,
                                initial_value,
                                is_mutable: true,
                                relevance: Default::default(),
                                pattern_fields: Default::default(),
                            },
                        )));
                    }
                    let ExpressionNode::Name(path) =
                        trees.expressions.expression(registers[index]).clone()
                    else {
                        unreachable!()
                    };
                    trees
                        .expressions
                        .replace_expression(handle, ExpressionNode::Name(path.into()));
                }
                (registers, bindings)
            }

            pub(crate) fn wide_type(
                trees: &mut SyntaxTrees,
            ) -> syntax_trees::types::TypeReferenceHandle {
                trees
                    .type_references
                    .insert(TypeReferenceNode::Named(Identifier::generated("u64")))
            }
            fn literal(trees: &mut SyntaxTrees, value: i64) -> ExpressionHandle {
                trees
                    .expressions
                    .insert(ExpressionNode::Integer(IntegerLiteral::from_value(value)))
            }
            fn wide_literal(trees: &mut SyntaxTrees, value: i64) -> ExpressionHandle {
                let value = literal(trees, value);
                wide_cast(trees, value)
            }
            fn binary(
                trees: &mut SyntaxTrees,
                left: ExpressionHandle,
                operator: BinaryOperator,
                right: ExpressionHandle,
            ) -> ExpressionHandle {
                trees
                    .expressions
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left,
                        operator,
                        right,
                    }))
            }
            fn write(
                trees: &mut SyntaxTrees,
                register: &str,
                value: ExpressionHandle,
            ) -> StatementHandle {
                let target = name(trees, register);
                trees
                    .statements
                    .insert(StatementNode::Assignment(TableAssignment { target, value }))
            }
            fn reg_binary(
                trees: &mut SyntaxTrees,
                left: &str,
                operator: BinaryOperator,
                right: &str,
            ) -> ExpressionHandle {
                let left = name(trees, left);
                let right = name(trees, right);
                binary(trees, left, operator, right)
            }
            pub(crate) fn preserve_sub_operands(
                trees: &mut SyntaxTrees,
                destination: ExpressionHandle,
                source: ExpressionHandle,
            ) -> Vec<StatementHandle> {
                let destination = trees.expressions.expression(destination).clone();
                let destination = trees.expressions.insert(destination);
                let source = trees.expressions.expression(source).clone();
                let source = trees.expressions.insert(source);
                vec![
                    write(trees, "asm#sub_left", destination),
                    write(trees, "asm#sub_right", source),
                ]
            }
            fn wide_cast(trees: &mut SyntaxTrees, value: ExpressionHandle) -> ExpressionHandle {
                let target_type = wide_type(trees);
                cast_to(trees, value, target_type, ArithmeticDomain::Exact)
            }
            fn masked_shift(
                trees: &mut SyntaxTrees,
                value: ExpressionHandle,
                mask: i64,
                shift: i64,
            ) -> ExpressionHandle {
                let mask = literal(trees, mask);
                let value = binary(trees, value, BinaryOperator::BitwiseAnd, mask);
                let shift = literal(trees, shift);
                binary(trees, value, BinaryOperator::ShiftRight, shift)
            }
            pub(crate) fn arithmetic_flags(
                trees: &mut SyntaxTrees,
                destination: ExpressionHandle,
                producer: ArithmeticFlagProducer,
            ) -> StatementHandle {
                use BinaryOperator::*;
                let cf = match producer {
                    ArithmeticFlagProducer::Subtract => {
                        let left = name(trees, "asm#sub_left");
                        let left = wide_cast(trees, left);
                        let right = name(trees, "asm#sub_right");
                        let right = wide_cast(trees, right);
                        let base = wide_literal(trees, 4294967296);
                        let sum = binary(trees, left, Add, base);
                        let difference = binary(trees, sum, Subtract, right);
                        let count = wide_literal(trees, 32);
                        let high = binary(trees, difference, ShiftRight, count);
                        let one = wide_literal(trees, 1);
                        binary(trees, high, BitwiseXor, one)
                    }
                    ArithmeticFlagProducer::Add => {
                        let left = name(trees, "asm#sub_left");
                        let left = wide_cast(trees, left);
                        let right = name(trees, "asm#sub_right");
                        let right = wide_cast(trees, right);
                        let sum = binary(trees, left, Add, right);
                        let count = wide_literal(trees, 32);
                        binary(trees, sum, ShiftRight, count)
                    }
                };
                let of = match producer {
                    ArithmeticFlagProducer::Subtract => {
                        let left = name(trees, "asm#sub_left");
                        let difference = binary(trees, left, BitwiseXor, destination);
                        let operands =
                            reg_binary(trees, "asm#sub_left", BitwiseXor, "asm#sub_right");
                        let overflow = binary(trees, operands, BitwiseAnd, difference);
                        masked_shift(trees, overflow, 0x80000000, 20)
                    }
                    ArithmeticFlagProducer::Add => {
                        let left = name(trees, "asm#sub_left");
                        let left_difference = binary(trees, left, BitwiseXor, destination);
                        let right = name(trees, "asm#sub_right");
                        let right_difference = binary(trees, right, BitwiseXor, destination);
                        let overflow = binary(trees, left_difference, BitwiseAnd, right_difference);
                        masked_shift(trees, overflow, 0x80000000, 20)
                    }
                };
                let operands = reg_binary(trees, "asm#sub_left", BitwiseXor, "asm#sub_right");
                let af = binary(trees, operands, BitwiseXor, destination);
                let af = masked_shift(trees, af, 16, 0);
                let sf = masked_shift(trees, destination, 0x80000000, 24);
                let zero = literal(trees, 0);
                let negated = subtraction(trees, zero, destination);
                let nonzero = binary(trees, destination, BitwiseOr, negated);
                let sign = literal(trees, 0x80000000);
                let inverted = binary(trees, nonzero, BitwiseXor, sign);
                let zf = masked_shift(trees, inverted, 0x80000000, 25);
                let four = literal(trees, 4);
                let upper = binary(trees, destination, ShiftRight, four);
                let folded = binary(trees, destination, BitwiseXor, upper);
                let fifteen = literal(trees, 15);
                let nibble = binary(trees, folded, BitwiseAnd, fifteen);
                let table = literal(trees, 0x9669);
                let table = cast(trees, table, ArithmeticDomain::Exact);
                let parity = binary(trees, table, ShiftRight, nibble);
                let mask = literal(trees, 1);
                let parity = binary(trees, parity, BitwiseAnd, mask);
                let count = literal(trees, 2);
                let pf = binary(trees, parity, ShiftLeft, count);
                let mut value = cf;
                for bits in [pf, af, zf, sf, of] {
                    let bits = wide_cast(trees, bits);
                    value = binary(trees, value, BitwiseOr, bits);
                }
                write(trees, "asm#flags", value)
            }
            pub(crate) fn initialize_flags(trees: &mut SyntaxTrees) -> Vec<StatementHandle> {
                let zero = wide_literal(trees, 0);
                let flags = write(trees, "asm#flags", zero);
                let zero = wide_literal(trees, 0);
                vec![flags, write(trees, "asm#flags_mask", zero)]
            }
            pub(crate) fn replace_arithmetic_flags(trees: &mut SyntaxTrees) -> StatementHandle {
                let mask = wide_literal(trees, 0x8d5);
                write(trees, "asm#flags_mask", mask)
            }
            pub(crate) fn observe_environment(
                trees: &mut SyntaxTrees,
                raw: ExpressionHandle,
            ) -> StatementHandle {
                write(trees, "asm#flags_environment", raw)
            }
            pub(crate) fn snapshot(trees: &mut SyntaxTrees) -> Vec<StatementHandle> {
                let all = trees.expressions.insert(ExpressionNode::Integer(
                    IntegerLiteral::from_parts(
                        false,
                        IntegerRadix::Hexadecimal,
                        "ffffffffffffffff",
                    )
                    .expect("compiler-owned flag mask"),
                ));
                let target_type = wide_type(trees);
                let all = trees
                    .expressions
                    .insert(ExpressionNode::Cast(TableCastExpression {
                        value: all,
                        target_type,
                        target_label: HandleSpan::empty(),
                        domain: ArithmeticDomain::Exact,
                        semantic_domain: HandleSpan::empty(),
                        semantic_domain_arguments: HandleSpan::empty(),
                        form: Default::default(),
                    }));
                let replacement = name(trees, "asm#flags_mask");
                let mask = binary(trees, all, BinaryOperator::BitwiseXor, replacement);
                let environment = name(trees, "asm#flags_environment");
                let persistent = binary(trees, environment, BinaryOperator::BitwiseAnd, mask);
                let flags = name(trees, "asm#flags");
                let image = binary(trees, persistent, BinaryOperator::BitwiseOr, flags);
                let image = write(trees, "asm#flags_snapshot", image);
                let snapshot = name(trees, "asm#flags_snapshot");
                let mask = wide_literal(trees, 0x8d5);
                let flags = binary(trees, snapshot, BinaryOperator::BitwiseAnd, mask);
                let flags = write(trees, "asm#flags", flags);
                let mask = replace_arithmetic_flags(trees);
                vec![image, flags, mask]
            }
            pub(crate) fn observed_zero_flag(trees: &mut SyntaxTrees) -> StatementHandle {
                let flags = name(trees, "asm#flags");
                let mask = wide_literal(trees, 64);
                let value = binary(trees, flags, BinaryOperator::BitwiseAnd, mask);
                let zero = wide_literal(trees, 0);
                let value = binary(trees, value, BinaryOperator::NotEqual, zero);
                write(trees, "zf", value)
            }
        }

        use crate::diagnostics::parse_error::ParseError;
        use crate::grammar::bodies::transitions::targets::parse_target::parse_transition_block_target_handle;
        use crate::grammar::expressions::parse_expression::parse_expression_handle;
        use crate::storage::tables::{exprs_of, items, stmts, stmts_of};
        use crate::tokens::token_cursor::{Input, ParseResult};
        use arena::HandleSpan;
        use language_core::inline_assembly::{
            AsmCatalogEntry, AsmFenceKind, AsmInstructionAvailability, AsmInstructionRefusal,
            AsmInstructionShape, AsmMemoryTransferKind, AsmSchedulingHintKind,
            AsmTargetApplicability, asm_catalog_entry, is_unmodeled_aarch64_scratch_view,
            scratch_register,
        };
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
        use syntax_trees::identifier::Identifier;
        use syntax_trees::statement::{
            AssemblyExit, AssemblyFactKind, StatementHandle, StatementNode, TableAssignment,
            TableCall, TableTransition, TransitionExit, TransitionGuardNode,
            TransitionTargetHandle,
        };
        use tokens::PunctuationKind;

        pub(crate) fn parse_asm_block_statement_handles<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
            let expression_start = exprs_of(syntax_trees).expression_count();
            let input = input.take_contextual("asm")?;
            let (contract, input) = parse_asm_contract(syntax_trees, input)?;
            let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
            let mut target_requirement = AsmTargetApplicability::Any;
            let mut blocks = Vec::new();
            let mut primitives = Vec::new();
            let body_requires = contract.requires.clone();
            let mut arithmetic_updates = Vec::new();
            let mut block = ParsedAssemblyBlock {
                label: Identifier::generated("asm#entry"),
                requires: contract.requires,
                ..Default::default()
            };
            let mut observes_hardware_flags = false;
            let mut restores_hardware_flags = false;
            let mut flag_snapshots = Vec::new();
            let mut realized_clobbers = std::collections::BTreeSet::new();

            while !input.at_punctuation(PunctuationKind::RightBrace) {
                if assembly_label_starts(input) {
                    if block.instruction_count != 0 {
                        blocks.push(block);
                        block = ParsedAssemblyBlock::default();
                    } else if !blocks.is_empty() || block.label.as_str(syntax_trees) != "asm#entry"
                    {
                        return Err(input.error_here(
                            "an asm block must contain at least one known instruction",
                        ));
                    }
                    let (label, rest) = input.take_identifier()?;
                    block.label = label;
                    input = rest;
                    while input.at_contextual("requires") || input.at_contextual("ensures") {
                        let requires = input.at_contextual("requires");
                        input =
                            input.take_contextual(if requires { "requires" } else { "ensures" })?;
                        let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
                        if requires {
                            block.requires.push(expression);
                        } else {
                            block.ensures.push(expression);
                        }
                        input = rest;
                    }
                    input = input.take_punctuation(PunctuationKind::Colon, ":")?;
                    continue;
                }
                if !matches!(block.exit, AssemblyExit::Fallthrough) {
                    return Err(input.error_here("an asm control transfer must be the final instruction in its block (including a labeled block)"));
                }
                if let Ok((mnemonic, after_jump)) = parse_asm_mnemonic(syntax_trees, input)
                    && let Some(AsmCatalogEntry::Contract(contract)) =
                        asm_catalog_entry(mnemonic.as_str(syntax_trees))
                    && matches!(
                        contract.shape,
                        AsmInstructionShape::JumpState | AsmInstructionShape::JumpZeroFlag { .. }
                    )
                    && let Ok((label, rest)) = after_jump.take_identifier()
                    && (rest.at_punctuation(PunctuationKind::Semicolon)
                        || rest.at_punctuation(PunctuationKind::RightBrace))
                {
                    target_requirement =
                        target_requirement
                            .intersect(contract.target)
                            .ok_or_else(|| {
                                input.error_here("assembly body mixes incompatible catalog targets")
                            })?;
                    block.exit =
                        if let AsmInstructionShape::JumpZeroFlag { when_set } = contract.shape {
                            scratch::name(syntax_trees, "zf");
                            AssemblyExit::ZeroFlag(label, when_set)
                        } else {
                            AssemblyExit::Jump(label)
                        };
                    block.instruction_count += 1;
                    input = rest;
                    if input.at_punctuation(PunctuationKind::Semicolon) {
                        input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
                    }
                    continue;
                }
                let (instruction, rest) =
                    parse_asm_instruction_statement_handle(syntax_trees, input)?;
                target_requirement = target_requirement
                    .intersect(instruction.contract.target)
                    .ok_or_else(|| {
                        input.error_here("assembly body mixes incompatible catalog targets")
                    })?;
                realized_clobbers.extend(instruction.contract.clobbers.iter().copied());
                observes_hardware_flags |= matches!(
                    instruction.contract.shape,
                    AsmInstructionShape::FlagsSnapshot | AsmInstructionShape::FlagsRestore
                );
                if let StatementNode::Assignment(assignment) =
                    stmts_of(syntax_trees).statement(instruction.statement)
                    && let ExpressionNode::Name(path) =
                        exprs_of(syntax_trees).expression(assignment.target)
                    && path.application.is_none()
                    && let [name] = exprs_of(syntax_trees).identifier_path_members(path.members)
                    && let Some((_, _, clobber)) = scratch_register(name.as_str(syntax_trees))
                {
                    realized_clobbers.insert(clobber);
                }
                restores_hardware_flags |=
                    instruction.contract.shape == AsmInstructionShape::FlagsRestore;
                if instruction.contract.shape == AsmInstructionShape::FlagsSnapshot {
                    flag_snapshots.push(instruction.statement);
                }
                if let Some(arithmetic) = instruction.arithmetic {
                    arithmetic_updates.push((instruction.statement, arithmetic));
                }
                block.has_no_local_post_state |= matches!(
                    instruction.contract.shape,
                    AsmInstructionShape::Halt | AsmInstructionShape::DerivedExit
                );
                if matches!(
                    stmts_of(syntax_trees).statement(instruction.statement),
                    StatementNode::Transition(_)
                ) {
                    block.exit = AssemblyExit::EnclosingTransition;
                }
                if instruction.contract.shape
                    != AsmInstructionShape::SchedulingHint(AsmSchedulingHintKind::Nop)
                {
                    block.instructions.push(instruction.statement);
                }
                if let Some(arithmetic) = instruction.arithmetic {
                    primitives.push(arithmetic.primitive);
                    let flag = scratch::zero_flag(syntax_trees, arithmetic.result);
                    let StatementNode::Assignment(flag_write) =
                        stmts_of(syntax_trees).statement(flag)
                    else {
                        unreachable!()
                    };
                    primitives.push(flag_write.value);
                    block.instructions.push(flag);
                }
                block.instruction_count += 1;
                input = rest;
                if input.at_punctuation(PunctuationKind::Semicolon) {
                    input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
                } else if !input.at_punctuation(PunctuationKind::RightBrace) {
                    return Err(
                        input.error_here("multiple asm instructions must be separated by `;`")
                    );
                }
            }
            if block.instruction_count == 0 {
                return Err(
                    input.error_here("an asm block must contain at least one known instruction")
                );
            }
            blocks.push(block);
            let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
            for (index, block) in blocks.iter().enumerate() {
                if block.has_no_local_post_state && !block.ensures.is_empty() {
                    return Err(input.error_here("asm `ensures` requires a falling-through block; this instruction has no local post-state"));
                }
                if blocks[..index].iter().any(|prior| {
                    prior.label.as_str(syntax_trees) == block.label.as_str(syntax_trees)
                }) {
                    return Err(input.error_here(format!(
                        "assembly label `{}` is declared more than once",
                        block.label.as_str(syntax_trees)
                    )));
                }
                if let AssemblyExit::Jump(label) | AssemblyExit::ZeroFlag(label, _) = &block.exit
                    && !blocks.iter().any(|target| {
                        target.label.as_str(syntax_trees) == label.as_str(syntax_trees)
                    })
                {
                    return Err(input.error_here(format!(
                        "assembly jump names undeclared label `{}`",
                        label.as_str(syntax_trees)
                    )));
                }
            }
            for (_, node) in syntax_trees
                .expressions
                .iter_expressions()
                .skip(expression_start)
            {
                let ExpressionNode::Name(path) = node else {
                    continue;
                };
                if path.application.is_some() {
                    continue;
                }
                let [name] = exprs_of(syntax_trees).identifier_path_members(path.members) else {
                    continue;
                };
                if is_unmodeled_aarch64_scratch_view(name.as_str(syntax_trees)) {
                    return Err(input.error_here(format!("unsupported AArch64 scratch register view `{}`: register aliases require shared-bit state modeling", name.as_str(syntax_trees))));
                }
                if language_core::inline_assembly::is_unmodeled_x86_scratch_view(
                    name.as_str(syntax_trees),
                ) {
                    let name = name.as_str(syntax_trees);
                    return Err(input.error_here(format!("unsupported x86 scratch register view `{name}`: register aliases require shared-bit state modeling")));
                }
            }
            let uses_scratch = exprs_of(syntax_trees).iter_expressions().skip(expression_start).any(|(_, node)| {
        matches!(node, ExpressionNode::Name(path) if path.application.is_none() && matches!(exprs_of(syntax_trees).identifier_path_members(path.members), [name] if scratch::NAMES.iter().any(|register| !register.is_empty() && *register == name.as_str(syntax_trees))))
    });
            if target_requirement == AsmTargetApplicability::Aarch64 && observes_hardware_flags {
                return Err(input.error_here(
                    "x86 flags snapshots/restoration cannot observe AArch64 scratch state",
                ));
            }
            if uses_scratch && restores_hardware_flags {
                return Err(input.error_here(
            "scratch-register CFG flags cannot yet be combined with hardware popfq restoration",
        ));
            }
            let last = blocks.last_mut().expect("assembly has a nonempty block");
            if !contract.ensures.is_empty()
                && (!matches!(last.exit, AssemblyExit::Fallthrough) || last.has_no_local_post_state)
            {
                return Err(input.error_here(
                    "asm `ensures` requires a falling-through block at the body exit",
                ));
            }
            last.ensures.extend(contract.ensures);
            if let Some(declared_clobbers) = contract.clobbers {
                validate_asm_clobber_contract(&declared_clobbers, &realized_clobbers, input)?;
            }
            if observes_hardware_flags && !restores_hardware_flags {
                for (statement, arithmetic) in arithmetic_updates {
                    let generated_start = exprs_of(syntax_trees).expression_count();
                    let saved = scratch::preserve_sub_operands(
                        syntax_trees,
                        arithmetic.left,
                        arithmetic.right,
                    );
                    let flags = scratch::arithmetic_flags(
                        syntax_trees,
                        arithmetic.result,
                        arithmetic.producer,
                    );
                    let mask = scratch::replace_arithmetic_flags(syntax_trees);
                    collect_generated_primitives(syntax_trees, generated_start, &mut primitives);
                    for block in &mut blocks {
                        if let Some(index) = block
                            .instructions
                            .iter()
                            .position(|item| *item == statement)
                        {
                            let count = saved.len();
                            block.instructions.splice(index..index, saved);
                            block
                                .instructions
                                .splice(index + count + 2..index + count + 2, [flags, mask]);
                            break;
                        }
                    }
                }
                let prefix = scratch::initialize_flags(syntax_trees);
                blocks[0].requires.drain(..body_requires.len());
                blocks.insert(
                    0,
                    ParsedAssemblyBlock {
                        label: Identifier::generated("asm#flags_entry"),
                        requires: body_requires,
                        instructions: prefix,
                        ..Default::default()
                    },
                );
                for snapshot in flag_snapshots {
                    let StatementNode::Assignment(mut assignment) =
                        stmts_of(syntax_trees).statement(snapshot).clone()
                    else {
                        unreachable!()
                    };
                    let generated_start = exprs_of(syntax_trees).expression_count();
                    let raw = scratch::observe_environment(syntax_trees, assignment.value);
                    let mut observation = vec![raw];
                    observation.extend(scratch::snapshot(syntax_trees));
                    assignment.value = scratch::name(syntax_trees, "asm#flags_snapshot");
                    stmts(syntax_trees)
                        .replace_statement(snapshot, StatementNode::Assignment(assignment));
                    let flag = scratch::observed_zero_flag(syntax_trees);
                    collect_generated_primitives(syntax_trees, generated_start, &mut primitives);
                    for block in &mut blocks {
                        if let Some(index) = block
                            .instructions
                            .iter()
                            .position(|candidate| *candidate == snapshot)
                        {
                            observation.push(flag);
                            block.instructions.splice(index..index, observation);

                            break;
                        }
                    }
                }
            }
            let (scratch, bindings) = scratch::bind(syntax_trees, expression_start);
            let blocks: Vec<_> = blocks
                .into_iter()
                .map(|block| syntax_trees::statement::AssemblyBlock {
                    label: block.label,
                    requires: syntax_trees
                        .statements
                        .insert_assembly_expressions(block.requires),
                    ensures: syntax_trees
                        .statements
                        .insert_assembly_expressions(block.ensures),
                    instructions: syntax_trees
                        .statements
                        .insert_assembly_instructions(block.instructions),
                    exit: block.exit,
                })
                .collect();
            let blocks = stmts(syntax_trees).insert_assembly_blocks(blocks);
            let primitives = syntax_trees
                .statements
                .insert_assembly_expressions(primitives);
            let graph =
                stmts(syntax_trees).insert_assembly(syntax_trees::statement::AssemblyControlFlow {
                    target_requirement,
                    blocks,
                    scratch,
                    primitives,
                });
            let statement = syntax_trees
                .statements
                .insert(StatementNode::Assembly(graph));
            let mut statements = crate::storage::handle_spans::HandleSpanBuilder::new();
            for statement in bindings.into_iter().chain(std::iter::once(statement)) {
                statements.push(items(syntax_trees).append_statement_handle(statement));
            }
            Ok((statements.finish(), input))
        }

        #[derive(Default)]
        struct ParsedAssemblyBlock {
            label: Identifier,
            requires: Vec<ExpressionHandle>,
            ensures: Vec<ExpressionHandle>,
            instructions: Vec<StatementHandle>,
            instruction_count: u32,
            has_no_local_post_state: bool,
            exit: AssemblyExit,
        }

        fn assembly_label_starts(input: Input<'_, '_>) -> bool {
            input.take_identifier().is_ok_and(|(_, rest)| {
                rest.at_punctuation(PunctuationKind::Colon)
                    || rest.at_contextual("requires")
                    || rest.at_contextual("ensures")
            })
        }

        #[derive(Default)]
        struct ParsedAsmContract {
            requires: Vec<ExpressionHandle>,
            ensures: Vec<ExpressionHandle>,
            clobbers: Option<std::collections::BTreeSet<String>>,
        }

        fn parse_asm_contract<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, ParsedAsmContract> {
            let mut input = input;
            let mut contract = ParsedAsmContract::default();

            while !input.at_punctuation(PunctuationKind::LeftBrace) {
                if input.at_contextual("requires") || input.at_contextual("ensures") {
                    let kind = if input.at_contextual("requires") {
                        AssemblyFactKind::Requires
                    } else {
                        AssemblyFactKind::Ensures
                    };
                    input = input.take_contextual(match kind {
                        AssemblyFactKind::Requires => "requires",
                        AssemblyFactKind::Ensures => "ensures",
                    })?;
                    let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
                    match kind {
                        AssemblyFactKind::Requires => contract.requires.push(expression),
                        AssemblyFactKind::Ensures => contract.ensures.push(expression),
                    }
                    input = rest;
                    if input.at_punctuation(PunctuationKind::Semicolon) {
                        input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
                    }
                    continue;
                }
                if !input.at_contextual("clobbers") {
                    return Err(input.expected_one_of_here(&[
                        "`clobbers <registers>`",
                        "`requires`",
                        "`ensures`",
                        "`{`",
                    ]));
                }
                if contract.clobbers.is_some() {
                    return Err(input.error_here("an asm block may declare `clobbers` only once"));
                }
                input = input.take_contextual("clobbers")?;
                let clobber_list_site = input;
                let mut declared = std::collections::BTreeSet::new();
                if input.at_contextual("none") {
                    input = input.take_contextual("none")?;
                } else {
                    while !input.at_punctuation(PunctuationKind::LeftBrace)
                        && !input.at_punctuation(PunctuationKind::Semicolon)
                        && !input.at_contextual("requires")
                        && !input.at_contextual("ensures")
                        && !input.at_contextual("clobbers")
                    {
                        let (register, rest) = input.take_identifier()?;
                        declared.insert(register.as_str(syntax_trees).to_owned());
                        input = rest;
                        if input.at_punctuation(PunctuationKind::Comma) {
                            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                        }
                    }
                    if declared.is_empty() {
                        return Err(clobber_list_site.error_here(
                            "an empty asm clobber contract must be explicit: spell `clobbers none`",
                        ));
                    }
                }
                contract.clobbers = Some(declared);
                if input.at_punctuation(PunctuationKind::Semicolon) {
                    input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
                }
            }

            Ok((contract, input))
        }

        fn validate_asm_clobber_contract<'tokens, 'source>(
            declared: &std::collections::BTreeSet<String>,
            realized: &std::collections::BTreeSet<&'static str>,
            input: Input<'tokens, 'source>,
        ) -> Result<(), ParseError> {
            let missing = realized
                .iter()
                .filter(|register| !declared.contains(**register))
                .copied()
                .collect::<Vec<_>>();
            let extra = declared
                .iter()
                .filter(|register| !realized.contains(register.as_str()))
                .map(String::as_str)
                .collect::<Vec<_>>();
            if missing.is_empty() && extra.is_empty() {
                return Ok(());
            }

            let mut details = Vec::new();
            if !missing.is_empty() {
                details.push(format!("missing {}", format_asm_registers(&missing)));
            }
            if !extra.is_empty() {
                details.push(format!("not clobbered {}", format_asm_registers(&extra)));
            }
            Err(input.error_here(format!(
                "asm `clobbers` must exactly match the realized instruction contract: {}",
                details.join("; ")
            )))
        }

        fn format_asm_registers(registers: &[&str]) -> String {
            registers
                .iter()
                .map(|register| format!("`{register}`"))
                .collect::<Vec<_>>()
                .join(", ")
        }

        #[derive(Clone, Copy)]
        struct ParsedArithmetic {
            left: ExpressionHandle,
            right: ExpressionHandle,
            result: ExpressionHandle,
            primitive: ExpressionHandle,
            producer: scratch::ArithmeticFlagProducer,
        }

        struct ParsedAsmInstruction {
            statement: StatementHandle,
            contract: language_core::inline_assembly::AsmInstructionContract,
            arithmetic: Option<ParsedArithmetic>,
        }

        impl ParsedAsmInstruction {
            fn ok<'tokens, 'source>(
                statement: StatementHandle,
                contract: language_core::inline_assembly::AsmInstructionContract,
                input: Input<'tokens, 'source>,
            ) -> ParseResult<'tokens, 'source, Self> {
                Ok((
                    Self {
                        statement,
                        contract,
                        arithmetic: None,
                    },
                    input,
                ))
            }
        }

        fn reject_bracketed_asm_operand(
            syntax_trees: &SyntaxTrees,
            mnemonic_site: Input<'_, '_>,
            mnemonic: &str,
            operand: ExpressionHandle,
        ) -> Result<(), ParseError> {
            if matches!(
                exprs_of(syntax_trees).expression(operand),
                ExpressionNode::ArrayLiteral(_)
            ) {
                return Err(mnemonic_site.error_here(format!(
                    "asm instruction `{mnemonic}` operand uses bracketed `[...]` memory \
             addressing: spell authorized access as a typed Omega place such as \
             `self.buffer[index]` — the place carries the provenance/permission \
             contract raw addresses cannot express"
                )));
            }
            Ok(())
        }

        fn parsed_intrinsic_instruction<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            mnemonic: &Identifier,
            intrinsic_name: &'static str,
            contract: language_core::inline_assembly::AsmInstructionContract,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, ParsedAsmInstruction> {
            ParsedAsmInstruction::ok(
                asm_intrinsic_call(syntax_trees, mnemonic, intrinsic_name, HandleSpan::empty()),
                contract,
                input,
            )
        }

        fn asm_intrinsic_call(
            syntax_trees: &mut SyntaxTrees,
            mnemonic: &Identifier,
            intrinsic_name: &'static str,
            arguments: HandleSpan<ExpressionHandle>,
        ) -> StatementHandle {
            stmts(syntax_trees).insert(StatementNode::Call(TableCall {
                target_is_static: false,
                receiver: HandleSpan::empty(),
                target: Identifier::new(intrinsic_name, mnemonic.source_span()),
                machine_arguments: Box::default(),
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: Default::default(),
                discards_result: false,
            }))
        }

        fn asm_intrinsic_result_assignment(
            syntax_trees: &mut SyntaxTrees,
            mnemonic: &Identifier,
            intrinsic_name: &'static str,
            arguments: HandleSpan<ExpressionHandle>,
            destination: ExpressionHandle,
        ) -> StatementHandle {
            let value =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
                        target_is_static: false,
                        receiver: ExpressionHandle::invalid(),
                        target: Identifier::new(intrinsic_name, mnemonic.source_span()),
                        machine_arguments: Box::default(),
                        arguments,
                        evidence_arguments: Box::default(),
                        operational_acknowledgement: Default::default(),
                    }));
            stmts(syntax_trees).insert(StatementNode::Assignment(TableAssignment {
                target: destination,
                value,
            }))
        }

        fn parse_asm_mnemonic<'tokens, 'source>(
            trees: &SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, Identifier> {
            let (mnemonic, rest) = input.take_identifier()?;
            if mnemonic.as_str(trees) == "b" && rest.at_punctuation(PunctuationKind::Dot) {
                let rest = rest.take_punctuation(PunctuationKind::Dot, ".")?;
                let (condition, rest) = rest.take_identifier()?;
                return Ok((
                    Identifier::generated(format!("b.{}", condition.as_str(trees))),
                    rest,
                ));
            }
            Ok((mnemonic, rest))
        }

        fn arithmetic_flag_producer(
            shape: AsmInstructionShape,
        ) -> Option<scratch::ArithmeticFlagProducer> {
            match shape {
                AsmInstructionShape::Subtract
                | AsmInstructionShape::SubtractThreeOperand
                | AsmInstructionShape::Compare => Some(scratch::ArithmeticFlagProducer::Subtract),
                AsmInstructionShape::Add => Some(scratch::ArithmeticFlagProducer::Add),
                _ => None,
            }
        }

        fn parse_asm_instruction_statement_handle<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, ParsedAsmInstruction> {
            let mnemonic_site = input;
            let (mnemonic, input) = parse_asm_mnemonic(syntax_trees, input)?;

            let Some(entry) = asm_catalog_entry(mnemonic.as_str(syntax_trees)) else {
                return Err(mnemonic_site.error_here(format!(
            "unknown asm instruction `{}`: only known-contract instructions compile \
             (`hlt`, `in`, `out`, `jmp`, `mov`/`movq`, `lfence`, `sfence`, `mfence`, `dmb`, `cli`, `sti`, \
             `serialize`, `isb`, `pause`, `yield`, `wfe`, `wfi`, `sev`, `sevl`, `nop`, `pushfq`, `popfq`, \
             `rdmsr`, `wrmsr`, `wbinvd`, `invd`, `wbnoinvd`, \
             `ldr`, `str`, structured `read_crN`/`write_crN`/`read_<sysreg>`/`write_<sysreg>`); opaque \
             forms (`db`, raw bytes) are rejected",
            mnemonic.as_str(syntax_trees)
        )));
            };
            let mut contract = match entry {
                AsmCatalogEntry::Contract(contract) => contract,
                AsmCatalogEntry::Refused(AsmInstructionRefusal::HiddenControlExit) => {
                    return Err(mnemonic_site.error_here(format!(
                        "asm instruction `{}` creates a hidden control exit: user assembly may not \
                 return, call, or branch indirectly; spell control flow as `jmp state(...)`",
                        mnemonic.as_str(syntax_trees)
                    )));
                }
                AsmCatalogEntry::Refused(AsmInstructionRefusal::UnmodeledMemoryAccess) => {
                    return Err(mnemonic_site.error_here(format!(
                        "asm instruction `{}` may access memory, but no structured operand \
                 provenance/permission contract is modeled for it yet; use typed Omega \
                 place/view operations until that instruction contract lands",
                        mnemonic.as_str(syntax_trees)
                    )));
                }
            };
            if contract.availability == AsmInstructionAvailability::DeriverOnly {
                return Err(mnemonic_site.error_here(format!(
                    "asm instruction `{}` is deriver-only: user-authored assembly may not spell \
             entry/exit protocol operations or manufacture an unmodeled control exit",
                    mnemonic.as_str(syntax_trees)
                )));
            }

            match contract.shape {
                AsmInstructionShape::JumpZeroFlag { .. } => Err(mnemonic_site.error_here(format!(
                    "{} requires a declared local assembly label",
                    mnemonic.as_str(syntax_trees)
                ))),
                AsmInstructionShape::Subtract
                | AsmInstructionShape::SubtractThreeOperand
                | AsmInstructionShape::Add
                | AsmInstructionShape::Compare
                | AsmInstructionShape::RegisterMove => {
                    let (destination, input) = parse_expression_handle(syntax_trees, input)?;
                    let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    let (first_source, input) = parse_expression_handle(syntax_trees, input)?;
                    let (left, source, input) =
                        if contract.shape == AsmInstructionShape::SubtractThreeOperand {
                            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                            let (right, input) = parse_expression_handle(syntax_trees, input)?;
                            (first_source, right, input)
                        } else {
                            (destination, first_source, input)
                        };
                    for operand in [destination, left, source] {
                        reject_bracketed_asm_operand(
                            syntax_trees,
                            mnemonic_site,
                            mnemonic.as_str(syntax_trees),
                            operand,
                        )?;
                    }
                    if matches!(exprs_of(syntax_trees).expression(destination), ExpressionNode::Name(path) if path.application.is_none() && matches!(exprs_of(syntax_trees).identifier_path_members(path.members), [name] if name.as_str(syntax_trees) == "zf"))
                    {
                        return Err(mnemonic_site.error_here("mov/sub/subs/add/cmp cannot directly write ZF; its version is established by the catalog arithmetic result"));
                    }
                    for operand in [destination, left, source] {
                        if let ExpressionNode::Name(path) =
                            exprs_of(syntax_trees).expression(operand)
                            && path.application.is_none()
                            && let [name] =
                                exprs_of(syntax_trees).identifier_path_members(path.members)
                            && let Some((_, target, _)) =
                                scratch_register(name.as_str(syntax_trees))
                        {
                            contract.target = contract.target.intersect(target).ok_or_else(|| mnemonic_site.error_here("assembly instruction and register operands require incompatible targets"))?;
                        }
                    }
                    let producer = arithmetic_flag_producer(contract.shape);
                    let value = if let Some(producer) = producer {
                        let spelling = mnemonic.as_str(syntax_trees);
                        let ExpressionNode::Name(path) =
                            exprs_of(syntax_trees).expression(destination)
                        else {
                            return Err(mnemonic_site.error_here(format!(
                                "{spelling} requires a modeled 32-bit register destination"
                            )));
                        };
                        if path.application.is_some() {
                            return Err(mnemonic_site.error_here(format!(
                                "{spelling} requires an unapplied modeled register destination"
                            )));
                        }
                        let [name] = exprs_of(syntax_trees).identifier_path_members(path.members)
                        else {
                            return Err(mnemonic_site.error_here(format!(
                                "{spelling} requires a modeled 32-bit register destination"
                            )));
                        };
                        let Some((_, register_target, _)) =
                            scratch_register(name.as_str(syntax_trees))
                        else {
                            return Err(mnemonic_site.error_here(format!(
                                "{spelling} requires a modeled 32-bit register destination"
                            )));
                        };
                        contract.target =
                            contract.target.intersect(register_target).ok_or_else(|| {
                                mnemonic_site.error_here(
                        "assembly instruction and register operands require incompatible targets",
                    )
                            })?;
                        if contract.shape == AsmInstructionShape::SubtractThreeOperand {
                            let modeled_left = match exprs_of(syntax_trees).expression(left) {
                                ExpressionNode::Name(path) if path.application.is_none() => {
                                    matches!(exprs_of(syntax_trees).identifier_path_members(path.members), [name] if scratch_register(name.as_str(syntax_trees)).is_some_and(|(_, target, _)| target == register_target))
                                }
                                _ => false,
                            };
                            if !modeled_left {
                                return Err(mnemonic_site.error_here(format!(
                                    "{spelling} requires a modeled 32-bit register left input"
                                )));
                            }
                        }
                        if contract.shape == AsmInstructionShape::Compare {
                            contract.clobbers = match register_target {
                                AsmTargetApplicability::Aarch64 => &["nzcv"],
                                _ => &["rflags"],
                            };
                        }
                        let admitted_source = match exprs_of(syntax_trees).expression(source) {
                            ExpressionNode::Integer(literal) => {
                                literal.value_u64().is_some_and(|value| {
                                    value <= u32::MAX as u64
                                        && (register_target != AsmTargetApplicability::Aarch64
                                            || value <= 4095
                                            || (value & 4095 == 0 && value >> 12 <= 4095))
                                })
                            }
                            ExpressionNode::Name(path) if path.application.is_none() => {
                                matches!(exprs_of(syntax_trees).identifier_path_members(path.members), [name] if scratch_register(name.as_str(syntax_trees)).is_some_and(|(_, target, _)| target == register_target))
                            }
                            _ => false,
                        };
                        if !admitted_source {
                            return Err(mnemonic_site.error_here(format!(
                        "{spelling} requires a modeled 32-bit register or a fitting unsigned 32-bit immediate source"
                    )));
                        }
                        match producer {
                            scratch::ArithmeticFlagProducer::Subtract => {
                                scratch::subtraction(syntax_trees, left, source)
                            }
                            scratch::ArithmeticFlagProducer::Add => {
                                scratch::addition(syntax_trees, left, source)
                            }
                        }
                    } else {
                        source
                    };
                    let result = if contract.shape == AsmInstructionShape::Compare {
                        scratch::name(syntax_trees, "asm#comparison_result")
                    } else {
                        destination
                    };
                    let statement =
                        stmts(syntax_trees).insert(StatementNode::Assignment(TableAssignment {
                            target: result,
                            value,
                        }));
                    let arithmetic = producer.map(|producer| {
                        let ExpressionNode::Cast(cast) = exprs_of(syntax_trees).expression(value)
                        else {
                            unreachable!("arithmetic construction returns a policy erasure")
                        };
                        ParsedArithmetic {
                            left,
                            right: source,
                            result,
                            primitive: cast.value,
                            producer,
                        }
                    });
                    Ok((
                        ParsedAsmInstruction {
                            statement,
                            contract,
                            arithmetic,
                        },
                        input,
                    ))
                }
                AsmInstructionShape::MemoryTransfer(kind) => {
                    let (first, input) = parse_expression_handle(syntax_trees, input)?;
                    let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    let (second, input) = parse_expression_handle(syntax_trees, input)?;
                    reject_bracketed_asm_operand(
                        syntax_trees,
                        mnemonic_site,
                        mnemonic.as_str(syntax_trees),
                        first,
                    )?;
                    reject_bracketed_asm_operand(
                        syntax_trees,
                        mnemonic_site,
                        mnemonic.as_str(syntax_trees),
                        second,
                    )?;
                    let (target, value) = match kind {
                        AsmMemoryTransferKind::Load => (first, second),
                        AsmMemoryTransferKind::Store => (second, first),
                    };
                    ParsedAsmInstruction::ok(
                        stmts(syntax_trees)
                            .insert(StatementNode::Assignment(TableAssignment { target, value })),
                        contract,
                        input,
                    )
                }
                AsmInstructionShape::JumpState => {
                    let (target, input) =
                        parse_transition_block_target_handle(syntax_trees, input)?;
                    ParsedAsmInstruction::ok(
                        stmts(syntax_trees).insert(StatementNode::Transition(TableTransition {
                            patterns: HandleSpan::empty(),
                            target,
                            continuation: TransitionTargetHandle::invalid(),
                            guard: TransitionGuardNode::Always,
                            proof_selectors: HandleSpan::empty(),
                            exit: TransitionExit::Ordinary,
                            ends_dispatch: true,
                            source_span: Default::default(),
                        })),
                        contract,
                        input,
                    )
                }
                AsmInstructionShape::Halt => ParsedAsmInstruction::ok(
                    asm_intrinsic_call(syntax_trees, &mnemonic, "asm#hlt", HandleSpan::empty()),
                    contract,
                    input,
                ),
                AsmInstructionShape::PortOut => {
                    let (port, input) = parse_expression_handle(syntax_trees, input)?;
                    let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    let (value, input) = parse_expression_handle(syntax_trees, input)?;
                    let arguments = syntax_trees
                        .statements
                        .insert_expression_handles(vec![port, value]);
                    ParsedAsmInstruction::ok(
                        asm_intrinsic_call(syntax_trees, &mnemonic, "asm#port_out", arguments),
                        contract,
                        input,
                    )
                }
                AsmInstructionShape::PortIn => {
                    let (destination, input) = parse_expression_handle(syntax_trees, input)?;
                    let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    let (port, input) = parse_expression_handle(syntax_trees, input)?;
                    let arguments = syntax_trees
                        .expressions
                        .insert_expression_handles(vec![port]);
                    let statement = asm_intrinsic_result_assignment(
                        syntax_trees,
                        &mnemonic,
                        "asm#port_in",
                        arguments,
                        destination,
                    );
                    ParsedAsmInstruction::ok(statement, contract, input)
                }
                AsmInstructionShape::MemoryFence(kind) => {
                    let (kind, input) = if kind.target() == AsmTargetApplicability::Aarch64 {
                        let (option, rest) = input.take_identifier()?;
                        let kind = AsmFenceKind::from_aarch64_option(option.as_str(syntax_trees))
                            .ok_or_else(|| {
                            input.error_here(
                            "dmb requires an explicit supported full-system option: ld, st, or sy",
                        )
                        })?;
                        (kind, rest)
                    } else {
                        (kind, input)
                    };
                    parsed_intrinsic_instruction(
                        syntax_trees,
                        &mnemonic,
                        kind.intrinsic_name(),
                        language_core::inline_assembly::AsmInstructionContract::memory_fence(kind),
                        input,
                    )
                }
                AsmInstructionShape::InterruptControl(kind) => parsed_intrinsic_instruction(
                    syntax_trees,
                    &mnemonic,
                    kind.intrinsic_name(),
                    contract,
                    input,
                ),
                AsmInstructionShape::InstructionSerialization(kind) => {
                    parsed_intrinsic_instruction(
                        syntax_trees,
                        &mnemonic,
                        kind.intrinsic_name(),
                        contract,
                        input,
                    )
                }
                AsmInstructionShape::SchedulingHint(kind) => parsed_intrinsic_instruction(
                    syntax_trees,
                    &mnemonic,
                    kind.intrinsic_name(),
                    contract,
                    input,
                ),
                AsmInstructionShape::CacheOperation(kind) => ParsedAsmInstruction::ok(
                    asm_intrinsic_call(
                        syntax_trees,
                        &mnemonic,
                        kind.intrinsic_name(),
                        HandleSpan::empty(),
                    ),
                    contract,
                    input,
                ),
                AsmInstructionShape::FlagsSnapshot => {
                    let (destination, input) = parse_expression_handle(syntax_trees, input)?;
                    let statement = asm_intrinsic_result_assignment(
                        syntax_trees,
                        &mnemonic,
                        "asm#pushfq",
                        HandleSpan::empty(),
                        destination,
                    );
                    ParsedAsmInstruction::ok(statement, contract, input)
                }
                AsmInstructionShape::FlagsRestore => {
                    let (source, input) = parse_expression_handle(syntax_trees, input)?;
                    let arguments = syntax_trees
                        .statements
                        .insert_expression_handles(vec![source]);
                    ParsedAsmInstruction::ok(
                        asm_intrinsic_call(syntax_trees, &mnemonic, "asm#popfq", arguments),
                        contract,
                        input,
                    )
                }
                AsmInstructionShape::MsrRead => {
                    let (destination, input) = parse_expression_handle(syntax_trees, input)?;
                    let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    let (index, input) = parse_expression_handle(syntax_trees, input)?;
                    let arguments = syntax_trees
                        .expressions
                        .insert_expression_handles(vec![index]);
                    let statement = asm_intrinsic_result_assignment(
                        syntax_trees,
                        &mnemonic,
                        "asm#rdmsr",
                        arguments,
                        destination,
                    );
                    ParsedAsmInstruction::ok(statement, contract, input)
                }
                AsmInstructionShape::MsrWrite => {
                    let (index, input) = parse_expression_handle(syntax_trees, input)?;
                    let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    let (value, input) = parse_expression_handle(syntax_trees, input)?;
                    let arguments = syntax_trees
                        .statements
                        .insert_expression_handles(vec![index, value]);
                    ParsedAsmInstruction::ok(
                        asm_intrinsic_call(syntax_trees, &mnemonic, "asm#wrmsr", arguments),
                        contract,
                        input,
                    )
                }
                AsmInstructionShape::ControlRegisterRead(register) => {
                    let (destination, input) = parse_expression_handle(syntax_trees, input)?;
                    let statement = asm_intrinsic_result_assignment(
                        syntax_trees,
                        &mnemonic,
                        register.read_intrinsic_name(),
                        HandleSpan::empty(),
                        destination,
                    );
                    ParsedAsmInstruction::ok(statement, contract, input)
                }
                AsmInstructionShape::ControlRegisterWrite(register) => {
                    let (source, input) = parse_expression_handle(syntax_trees, input)?;
                    let arguments = syntax_trees
                        .statements
                        .insert_expression_handles(vec![source]);
                    ParsedAsmInstruction::ok(
                        asm_intrinsic_call(
                            syntax_trees,
                            &mnemonic,
                            register
                                .write_intrinsic_name()
                                .expect("writable control-register shape"),
                            arguments,
                        ),
                        contract,
                        input,
                    )
                }
                AsmInstructionShape::SystemRegisterRead(register) => {
                    let (destination, input) = parse_expression_handle(syntax_trees, input)?;
                    let statement = asm_intrinsic_result_assignment(
                        syntax_trees,
                        &mnemonic,
                        register.read_intrinsic_name(),
                        HandleSpan::empty(),
                        destination,
                    );
                    ParsedAsmInstruction::ok(statement, contract, input)
                }
                AsmInstructionShape::SystemRegisterWrite(register) => {
                    let (source, input) = parse_expression_handle(syntax_trees, input)?;
                    let arguments = syntax_trees
                        .statements
                        .insert_expression_handles(vec![source]);
                    ParsedAsmInstruction::ok(
                        asm_intrinsic_call(
                            syntax_trees,
                            &mnemonic,
                            register
                                .write_intrinsic_name()
                                .expect("writable system-register shape"),
                            arguments,
                        ),
                        contract,
                        input,
                    )
                }
                AsmInstructionShape::GlobalDescriptorTable(operation) => {
                    let (operand, input) = parse_expression_handle(syntax_trees, input)?;
                    let arguments = syntax_trees
                        .statements
                        .insert_expression_handles(vec![operand]);
                    ParsedAsmInstruction::ok(
                        asm_intrinsic_call(
                            syntax_trees,
                            &mnemonic,
                            operation.intrinsic_name(),
                            arguments,
                        ),
                        contract,
                        input,
                    )
                }
                AsmInstructionShape::DerivedExit | AsmInstructionShape::DescriptorTableLoad => {
                    unreachable!("deriver-only instructions refuse before source lowering")
                }
            }
        }

        fn collect_generated_primitives(
            trees: &SyntaxTrees,
            first: usize,
            primitives: &mut Vec<ExpressionHandle>,
        ) {
            primitives.extend(trees.expressions.iter_expressions().skip(first).filter_map(
                |(handle, node)| matches!(node, ExpressionNode::Binary(_)).then_some(handle),
            ));
        }
    }
    pub(crate) mod parse_statement {
        use super::atomic_lets::try_desugar_atomic_let;
        use super::destructure_and_proof_output::{
            reject_retired_proof_output_binding, try_parse_destructure_let,
            try_parse_proof_output_binding,
        };
        use super::inline_assembly::parse_asm_block_statement_handles;
        use crate::storage::tables::{exprs, exprs_of, items, stmts};

        use crate::grammar::bodies::statements::atomic_lets::try_desugar_atomic_store;
        use crate::grammar::bodies::statements::discard_and_local_data::{
            copy_compound_assignment_target, parse_discard_statement_handle,
            parse_if_transition_statement_handle, parse_local_data,
        };
        use crate::grammar::bodies::statements::statement_tables::{
            expression_handle_to_statement_call, root_binding_declaration,
        };
        use crate::grammar::bodies::transitions::parse_transition::parse_transition_block_handles;
        use crate::grammar::declarations::parse_declaration::starts_machine_declaration;
        use crate::grammar::expressions::parse_expression::parse_expression_handle;
        use crate::tokens::token_cursor::{Input, ParseResult};
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
        use syntax_trees::statement::{
            StatementHandle, StatementNode, TableAssignment, TableTransition, TransitionExit,
            TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
        };
        use tokens::{KeywordKind, PunctuationKind};

        pub(crate) fn parse_statement_handles<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
            reject_retired_proof_output_binding(input)?;
            if input.at_keyword(KeywordKind::Transition) {
                let next = input.take_keyword(KeywordKind::Transition, "transition")?;
                parse_transition_block_handles(syntax_trees, next)
            } else if input.at_contextual("asm") {
                parse_asm_block_statement_handles(syntax_trees, input)
            } else if let Some(parsed) = try_parse_proof_output_binding(syntax_trees, input) {
                Ok(parsed)
            } else if let Some(parsed) = try_parse_destructure_let(syntax_trees, input) {
                Ok(parsed)
            } else if input.at_keyword(KeywordKind::Let) {
                let input = input.take_keyword(KeywordKind::Let, "let")?;
                let (binding, rest) = parse_local_data(syntax_trees, input)?;
                let statements =
                    if let Some(statements) = try_desugar_atomic_let(syntax_trees, &binding) {
                        statements
                    } else {
                        let statement = syntax_trees
                            .statements
                            .insert(StatementNode::LocalData(binding));
                        let handle = items(syntax_trees).append_statement_handle(statement);
                        HandleSpan::from_parts(handle, 1)
                    };
                Ok((statements, rest))
            } else {
                let (statement, rest) = parse_statement_handle(syntax_trees, input)?;
                let handle = items(syntax_trees).append_statement_handle(statement);
                Ok((HandleSpan::from_parts(handle, 1), rest))
            }
        }

        fn finish_semicolon_statement<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            node: StatementNode,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, StatementHandle> {
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            Ok((stmts(syntax_trees).insert(node), input))
        }

        fn parse_statement_handle<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, StatementHandle> {
            if input.at_keyword(KeywordKind::If) {
                return parse_if_transition_statement_handle(syntax_trees, input);
            }

            if input.at_contextual("relax") {
                return Err(input.error_here(
                    "`relax` is retired: invariant windows (ch11) supersede it -- write \
             plainly; a momentary violation opens a window that must close before \
             any read, call, or terminal exit",
                ));
            }

            if input.at_contextual("spawn") {
                let after_spawn = input.take_contextual("spawn")?;
                if after_spawn.at_punctuation(PunctuationKind::LeftBrace) {
                    return Err(input.error_here(
                        "statement `spawn { ... }` is retired: task activation requires an \
                 explicit runtime capability and returns a linear `Task<T>` that must \
                 be settled or transferred; implicit detach is not supported",
                    ));
                }
            }

            if input.at_contextual("_") {
                let after_underscore = input.take_contextual("_")?;
                if after_underscore.at_punctuation(PunctuationKind::Equal) {
                    return parse_discard_statement_handle(syntax_trees, after_underscore);
                }
            }

            if input.at_contextual("trap") {
                return Err(input.error_here(
            "statement `trap` is retired; write `crash Trap;` and publish a covering `crashes Trap` route",
        ));
            }

            if input.at_contextual("crash") {
                let source_span = input
                    .tokens
                    .first()
                    .map(|token| input.source_span(token))
                    .unwrap_or_default();
                let input = input.take_contextual("crash")?;
                let (cause, input) = input.take_identifier()?;
                let cause = match cause.as_str(syntax_trees) {
                    "Trap" => syntax_trees::item::CrashCause::Trap,
                    "Abort" => syntax_trees::item::CrashCause::Abort,
                    _ => {
                        return Err(input.error_here(format!(
                            "unknown crash cause `{}`; expected `Trap` or `Abort`",
                            cause.as_str(syntax_trees)
                        )));
                    }
                };
                let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
                let target = syntax_trees
                    .statements
                    .insert_transition_target(TransitionTargetNode::Terminal);
                return Ok((
                    stmts(syntax_trees).insert(StatementNode::Transition(TableTransition {
                        patterns: HandleSpan::empty(),
                        target,
                        continuation: TransitionTargetHandle::invalid(),
                        guard: TransitionGuardNode::Always,
                        proof_selectors: HandleSpan::empty(),
                        exit: TransitionExit::Crash(cause),
                        ends_dispatch: true,
                        source_span,
                    })),
                    input,
                ));
            }

            let (expression, input) = parse_expression_handle(syntax_trees, input)?;

            if let Some(binding) = root_binding_declaration(syntax_trees, expression)? {
                return finish_semicolon_statement(
                    syntax_trees,
                    StatementNode::RootBinding(binding),
                    input,
                );
            }

            if let Some(assignment) = try_desugar_atomic_store(syntax_trees, expression) {
                return finish_semicolon_statement(
                    syntax_trees,
                    StatementNode::Assignment(assignment),
                    input,
                );
            }

            if input.at_punctuation(PunctuationKind::Equal) {
                let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
                let (value, input) = parse_expression_handle(syntax_trees, input)?;
                return finish_semicolon_statement(
                    syntax_trees,
                    StatementNode::Assignment(TableAssignment {
                        target: expression,
                        value,
                    }),
                    input,
                );
            }

            for (punctuation, label, operator) in [
                (PunctuationKind::PlusEqual, "+=", BinaryOperator::Add),
                (PunctuationKind::MinusEqual, "-=", BinaryOperator::Subtract),
                (
                    PunctuationKind::AsteriskEqual,
                    "*=",
                    BinaryOperator::Multiply,
                ),
                (PunctuationKind::SlashEqual, "/=", BinaryOperator::Divide),
                (PunctuationKind::PercentEqual, "%=", BinaryOperator::Modulo),
            ] {
                if !input.at_punctuation(punctuation) {
                    continue;
                }
                let read_target = copy_compound_assignment_target(syntax_trees, expression)
                    .ok_or_else(|| {
                        input.error_here("compound assignment target must be a place")
                    })?;
                let input = input.take_punctuation(punctuation, label)?;
                let (right, input) = parse_expression_handle(syntax_trees, input)?;
                let value =
                    exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: read_target,
                        operator,
                        right,
                    }));
                return finish_semicolon_statement(
                    syntax_trees,
                    StatementNode::Assignment(TableAssignment {
                        target: expression,
                        value,
                    }),
                    input,
                );
            }

            if input.at_punctuation(PunctuationKind::RightBrace)
                || starts_machine_declaration(input)
            {
                let target = syntax_trees
                    .statements
                    .insert_transition_target(TransitionTargetNode::Value(expression));
                let source_span = exprs_of(syntax_trees).source_span(expression);
                return Ok((
                    stmts(syntax_trees).insert(StatementNode::Transition(TableTransition {
                        patterns: HandleSpan::empty(),
                        target,
                        continuation: TransitionTargetHandle::invalid(),
                        guard: TransitionGuardNode::Always,
                        proof_selectors: HandleSpan::empty(),
                        exit: TransitionExit::Ordinary,
                        ends_dispatch: true,
                        source_span,
                    })),
                    input,
                ));
            }

            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            if let Some(call) = expression_handle_to_statement_call(syntax_trees, expression) {
                Ok((stmts(syntax_trees).insert(StatementNode::Call(call)), input))
            } else {
                Ok((
                    stmts(syntax_trees).insert(StatementNode::Expression(expression)),
                    input,
                ))
            }
        }
    }
    pub(crate) mod statement_tables {
        use crate::diagnostics::parse_error::ParseError;
        use crate::grammar::bodies::copy_expression_handles_to_statement_table;
        use crate::storage::tables::{exprs, exprs_of};
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::{
            ExpressionHandle, ExpressionNode, TableCallExpression, TableIndexedExpression,
            TableMemberExpression,
        };
        use syntax_trees::identifier::Identifier;
        use syntax_trees::statement::TableCall;

        pub(crate) fn copy_expression_as_place(
            syntax_trees: &mut SyntaxTrees,
            expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            let node = exprs_of(syntax_trees).expression(expression).clone();
            let copied = match node {
                ExpressionNode::Member(m) => {
                    let recv = copy_expression_as_place(syntax_trees, m.receiver)?;
                    ExpressionNode::Member(TableMemberExpression {
                        receiver: recv,
                        access_source_span: m.access_source_span,
                        member: m.member,
                        case_variant: m.case_variant.clone(),
                    })
                }
                ExpressionNode::Name(path) if path.application.is_none() => {
                    let new_path = syntax_trees
                        .expressions
                        .copy_identifier_path_prefix(path.members, path.members.len());
                    ExpressionNode::Name(new_path.into())
                }
                ExpressionNode::Indexed(idx) => {
                    let coll = copy_expression_as_place(syntax_trees, idx.collection)?;
                    ExpressionNode::Indexed(TableIndexedExpression {
                        collection: coll,
                        index: idx.index,
                    })
                }
                ExpressionNode::Borrow(inner) => {
                    let inner_copy = copy_expression_as_place(syntax_trees, inner.target)?;
                    ExpressionNode::Borrow(syntax_trees::expression::TableBorrowExpression {
                        target: inner_copy,
                        access: inner.access,
                    })
                }
                _ => return None,
            };
            Some(exprs(syntax_trees).insert(copied))
        }

        pub(crate) fn root_binding_declaration(
            syntax_trees: &SyntaxTrees,
            expression: ExpressionHandle,
        ) -> Result<Option<syntax_trees::statement::RootBinding>, ParseError> {
            let ExpressionNode::Call(call) = exprs_of(syntax_trees).expression(expression) else {
                return Ok(None);
            };
            if call.target.as_str(syntax_trees) == "bind"
                && !call.target_is_static
                && call.receiver.is_valid()
                && matches!(exprs_of(syntax_trees).expression(call.receiver),
            ExpressionNode::Member(member) if member.member.as_str(syntax_trees) == "roots")
            {
                return Err(ParseError::new(
                    "receiver-style root binding is retired; use Build::bind(builder, slot, entry)",
                ));
            }
            if call.target.as_str(syntax_trees) != "bind"
                || !call.target_is_static
                || !call.receiver.is_valid()
            {
                return Ok(None);
            }
            let ExpressionNode::Name(owner) = exprs_of(syntax_trees).expression(call.receiver)
            else {
                return Ok(None);
            };
            let owner = exprs_of(syntax_trees).identifier_path_members(*owner);
            if !matches!(owner, [name] if name.as_str(syntax_trees) == "Build") {
                return Ok(None);
            }
            if !call.machine_arguments.is_empty() {
                return Ok(None);
            }
            let invalid = || {
                ParseError::new(
                    "root-slot binding requires an explicit Build argument, one slot path and one implementation path or description expression",
                )
            };
            let [builder, slot, implementation] =
                exprs_of(syntax_trees).expression_handles(call.arguments)
            else {
                return Err(invalid());
            };
            if !call.machine_arguments.is_empty() || !call.evidence_arguments.is_empty() {
                return Err(invalid());
            }
            let operand = |handle| {
                let ExpressionNode::Name(path) = exprs_of(syntax_trees).expression(handle) else {
                    return Err(invalid());
                };
                if path.application.is_some() {
                    return Err(invalid());
                }
                Ok(syntax_trees
                    .expressions
                    .identifier_path_members(path.members)
                    .to_vec()
                    .into_boxed_slice())
            };
            let described_operand = |handle: ExpressionHandle| {
                let ExpressionNode::Name(path) = exprs_of(syntax_trees).expression(handle) else {
                    return handle;
                };
                let members = exprs_of(syntax_trees).identifier_path_members(path.members);
                if path.application.is_none()
                    && members.len() == 1
                    && !members[0].is_self_receiver(syntax_trees)
                {
                    handle
                } else {
                    ExpressionHandle::invalid()
                }
            };
            Ok(Some(syntax_trees::statement::RootBinding {
                receiver: *builder,
                slot: operand(*slot)?,
                implementation: if matches!(
                    exprs_of(syntax_trees).expression(*implementation),
                    ExpressionNode::Name(_)
                ) {
                    operand(*implementation)?
                } else {
                    Box::default()
                },
                implementation_operand: described_operand(*implementation),
                source_span: call.target.source_span(),
            }))
        }

        pub(crate) fn expression_handle_to_statement_call(
            syntax_trees: &mut SyntaxTrees,
            expression: ExpressionHandle,
        ) -> Option<TableCall> {
            let ExpressionNode::Call(call) = exprs_of(syntax_trees).expression(expression).clone()
            else {
                return None;
            };

            let (receiver, target) = split_expression_call_handle(syntax_trees, &call)?;
            Some(TableCall {
                target_is_static: call.target_is_static,
                receiver,
                target,
                machine_arguments: call.machine_arguments,
                arguments: copy_expression_handles_to_statement_table(syntax_trees, call.arguments),
                evidence_arguments: call.evidence_arguments,
                operational_acknowledgement: call.operational_acknowledgement,
                discards_result: false,
            })
        }

        fn split_expression_call_handle(
            syntax_trees: &mut SyntaxTrees,
            call: &TableCallExpression,
        ) -> Option<(HandleSpan<Identifier>, Identifier)> {
            let receiver = if call.receiver.is_valid() {
                crate::grammar::bodies::copy_expression_to_statement_identifier_path(
                    syntax_trees,
                    call.receiver,
                )?
            } else {
                HandleSpan::empty()
            };

            Some((receiver, call.target.clone()))
        }
    }
}
