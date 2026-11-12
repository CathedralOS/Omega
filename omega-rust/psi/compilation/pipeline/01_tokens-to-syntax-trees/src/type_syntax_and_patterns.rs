/*
Type reference grammar, type properties, and qualified names
(`grammar::type_syntax`) alongside structural field and variant patterns
(`grammar::patterns`).
*/

pub(crate) mod patterns {

    use crate::tokens::token_cursor::{Input, ParseResult};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::TableMatchField;
    use syntax_trees::statement::TableOutcomeProofSelector;

    pub(crate) mod bindings {

        use crate::storage::tables::{exprs, exprs_of, stmts_of};
        use arena::{Handle, HandleSpan};
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::{
            ExpressionHandle, ExpressionNode, MatchSubjectCapture, TableAtomicExpression,
            TableBinaryExpression, TableBorrowExpression, TableCallExpression, TableCastExpression,
            TableIndexedExpression, TableMemberExpression, TableMembershipExpression,
            TableRangeExpression, TableSavedSubject, TableStructLiteral, TableStructLiteralField,
            TableUnaryExpression,
        };
        use syntax_trees::identifier::Identifier;

        type SubjectCopies = Vec<(Handle<MatchSubjectCapture>, Handle<MatchSubjectCapture>)>;

        #[derive(Clone)]
        pub(crate) struct DestructureBinding {
            pub(crate) component: language_core::PatternComponent,
            pub(crate) binding: Identifier,
            pub(crate) member: Identifier,
            pub(crate) case_variant: Option<Identifier>,
        }

        pub(crate) fn rewrite_destructure_guard_expression(
            syntax_trees: &mut SyntaxTrees,
            expression: ExpressionHandle,
            subject: ExpressionHandle,
            fields: &[DestructureBinding],
        ) -> ExpressionHandle {
            rewrite_expression(syntax_trees, expression, subject, fields, &mut Vec::new())
        }

        fn rewrite_expression(
            syntax_trees: &mut SyntaxTrees,
            expression: ExpressionHandle,
            subject: ExpressionHandle,
            fields: &[DestructureBinding],
            captures: &mut SubjectCopies,
        ) -> ExpressionHandle {
            let rewritten = match exprs_of(syntax_trees).expression(expression).clone() {
                ExpressionNode::Match(dispatch) => {
                    let saved_subject = rewrite_expression(
                        syntax_trees,
                        dispatch.subject,
                        subject,
                        fields,
                        captures,
                    );
                    let outer_captures = captures.len();
                    let capture = if dispatch.capture.is_valid() {
                        let copied = exprs(syntax_trees).insert_match_subject(saved_subject);
                        captures.push((dispatch.capture, copied));
                        copied
                    } else {
                        Handle::invalid()
                    };
                    let mut arms = exprs_of(syntax_trees).match_arms(dispatch.arms).to_vec();
                    for arm in &mut arms {
                        use syntax_trees::expression::{MatchFieldAction, MatchPattern};
                        let visible_fields = match &mut arm.pattern {
                            MatchPattern::Value(pattern) => {
                                *pattern = rewrite_expression(
                                    syntax_trees,
                                    *pattern,
                                    subject,
                                    fields,
                                    captures,
                                );
                                fields.to_vec()
                            }
                            MatchPattern::Destructure(pattern) => {
                                let mut pattern_fields = syntax_trees
                                    .expressions
                                    .match_fields(pattern.fields)
                                    .to_vec();
                                let visible = fields.iter().filter(|outer| {
                            !pattern_fields.iter().any(|inner| matches!(&inner.action, MatchFieldAction::Bind(binding) if binding.as_str(syntax_trees) == outer.binding.as_str(syntax_trees)))
                                && !stmts_of(syntax_trees).outcome_proof_selectors(pattern.proof_selectors).iter().any(|selector| selector.binding.as_str(syntax_trees) == outer.binding.as_str(syntax_trees))
                        }).cloned().collect::<Vec<_>>();
                                for field in &mut pattern_fields {
                                    if let MatchFieldAction::Equal(value) = &mut field.action {
                                        *value = rewrite_expression(
                                            syntax_trees,
                                            *value,
                                            subject,
                                            fields,
                                            captures,
                                        );
                                    }
                                }
                                pattern.fields =
                                    exprs(syntax_trees).insert_match_fields(pattern_fields);
                                pattern.guard = rewrite_optional_expression(
                                    syntax_trees,
                                    pattern.guard,
                                    subject,
                                    &visible,
                                    captures,
                                );
                                visible
                            }
                            MatchPattern::Wildcard => fields.to_vec(),
                        };
                        arm.value = rewrite_expression(
                            syntax_trees,
                            arm.value,
                            subject,
                            &visible_fields,
                            captures,
                        );
                    }
                    captures.truncate(outer_captures);
                    let arms = exprs(syntax_trees).insert_match_arms(arms);
                    ExpressionNode::Match(syntax_trees::expression::TableMatchExpression {
                        subject: saved_subject,
                        arms,
                        capture,
                    })
                }
                ExpressionNode::SavedSubject(saved) => {
                    ExpressionNode::SavedSubject(TableSavedSubject {
                        component: saved.component,
                        capture: captures
                            .iter()
                            .rev()
                            .find_map(|(source, copied)| {
                                (*source == saved.capture).then_some(*copied)
                            })
                            .unwrap_or(saved.capture),
                        pattern_head: saved.pattern_head,
                    })
                }
                ExpressionNode::Atomic(atomic) => ExpressionNode::Atomic(TableAtomicExpression {
                    value: rewrite_expression(
                        syntax_trees,
                        atomic.value,
                        subject,
                        fields,
                        captures,
                    ),
                    result: if atomic.result.is_valid() {
                        rewrite_expression(syntax_trees, atomic.result, subject, fields, captures)
                    } else {
                        ExpressionHandle::invalid()
                    },
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                }),
                ExpressionNode::ArrayLiteral(values) => {
                    let source_values = exprs_of(syntax_trees).expression_handles(values).to_vec();
                    let values = source_values
                        .iter()
                        .map(|value| {
                            rewrite_expression(syntax_trees, *value, subject, fields, captures)
                        })
                        .collect::<Vec<_>>();
                    ExpressionNode::ArrayLiteral(
                        exprs(syntax_trees).insert_expression_handles(values),
                    )
                }
                ExpressionNode::Binary(binary) => ExpressionNode::Binary(TableBinaryExpression {
                    policy: binary.policy,
                    left: rewrite_expression(syntax_trees, binary.left, subject, fields, captures),
                    operator: binary.operator,
                    right: rewrite_expression(
                        syntax_trees,
                        binary.right,
                        subject,
                        fields,
                        captures,
                    ),
                }),
                ExpressionNode::Call(call) => ExpressionNode::Call(TableCallExpression {
                    callee: rewrite_optional_expression(
                        syntax_trees,
                        call.callee,
                        subject,
                        fields,
                        captures,
                    ),
                    target_is_static: call.target_is_static,
                    receiver: if call.target_is_static {
                        call.receiver
                    } else {
                        rewrite_optional_expression(
                            syntax_trees,
                            call.receiver,
                            subject,
                            fields,
                            captures,
                        )
                    },
                    target: call.target,
                    machine_arguments: call.machine_arguments,
                    arguments: rewrite_expression_span(
                        syntax_trees,
                        call.arguments,
                        subject,
                        fields,
                        captures,
                    ),
                    evidence_arguments: call.evidence_arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                }),
                ExpressionNode::Cast(cast) => ExpressionNode::Cast(TableCastExpression {
                    value: rewrite_expression(syntax_trees, cast.value, subject, fields, captures),
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    form: cast.form,
                }),
                ExpressionNode::Indexed(indexed) => {
                    ExpressionNode::Indexed(TableIndexedExpression {
                        collection: rewrite_expression(
                            syntax_trees,
                            indexed.collection,
                            subject,
                            fields,
                            captures,
                        ),
                        index: rewrite_expression(
                            syntax_trees,
                            indexed.index,
                            subject,
                            fields,
                            captures,
                        ),
                    })
                }
                ExpressionNode::Membership(membership) => {
                    ExpressionNode::Membership(TableMembershipExpression {
                        value: rewrite_expression(
                            syntax_trees,
                            membership.value,
                            subject,
                            fields,
                            captures,
                        ),
                        domain: membership.domain,
                    })
                }
                ExpressionNode::Member(member) => ExpressionNode::Member(TableMemberExpression {
                    receiver: rewrite_expression(
                        syntax_trees,
                        member.receiver,
                        subject,
                        fields,
                        captures,
                    ),
                    access_source_span: member.access_source_span,
                    member: member.member,
                    case_variant: member.case_variant,
                }),
                ExpressionNode::Borrow(inner) => ExpressionNode::Borrow(TableBorrowExpression {
                    target: rewrite_expression(
                        syntax_trees,
                        inner.target,
                        subject,
                        fields,
                        captures,
                    ),
                    access: inner.access,
                }),
                ExpressionNode::Name(path) => {
                    if let Some((field, case_variant, component)) =
                        single_destructured_field_name(syntax_trees, path, fields)
                    {
                        if component == language_core::PatternComponent::Case {
                            // This is another view of the dispatch's saved value,
                            // never a field read or another evaluation of its source.
                            let ExpressionNode::SavedSubject(mut saved) =
                                *exprs_of(syntax_trees).expression(subject)
                            else {
                                unreachable!(
                                    "destructure aliases are produced from a saved subject"
                                )
                            };
                            saved.component = component;
                            ExpressionNode::SavedSubject(saved)
                        } else {
                            ExpressionNode::Member(TableMemberExpression {
                                receiver: subject,
                                access_source_span: Default::default(),
                                member: field,
                                case_variant,
                            })
                        }
                    } else {
                        ExpressionNode::Name(path)
                    }
                }
                ExpressionNode::Range(range) => ExpressionNode::Range(TableRangeExpression {
                    start: rewrite_optional_expression(
                        syntax_trees,
                        range.start,
                        subject,
                        fields,
                        captures,
                    ),
                    end: rewrite_optional_expression(
                        syntax_trees,
                        range.end,
                        subject,
                        fields,
                        captures,
                    ),
                    end_inclusive: range.end_inclusive,
                }),
                ExpressionNode::StructLiteral(struct_literal) => {
                    let source_fields = syntax_trees
                        .expressions
                        .struct_fields(struct_literal.fields)
                        .to_vec();
                    let struct_fields = source_fields
                        .iter()
                        .map(|field| TableStructLiteralField {
                            name: field.name.clone(),
                            value: rewrite_expression(
                                syntax_trees,
                                field.value,
                                subject,
                                fields,
                                captures,
                            ),
                        })
                        .collect::<Vec<_>>();
                    ExpressionNode::StructLiteral(TableStructLiteral {
                        application_type: struct_literal.application_type,
                        constructor_name: struct_literal.constructor_name,
                        fields: exprs(syntax_trees).insert_struct_fields(struct_fields),
                    })
                }
                ExpressionNode::Unary(unary) => ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand: rewrite_expression(
                        syntax_trees,
                        unary.operand,
                        subject,
                        fields,
                        captures,
                    ),
                }),
                ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::String(_)
                | ExpressionNode::TypeExpression(_)
                | ExpressionNode::ZeroValue(_) => {
                    exprs_of(syntax_trees).expression(expression).clone()
                }
            };

            let source_span = exprs_of(syntax_trees).source_span(expression);
            let rewritten = exprs(syntax_trees).insert(rewritten);
            exprs(syntax_trees).set_source_span(rewritten, source_span);
            rewritten
        }

        fn rewrite_optional_expression(
            syntax_trees: &mut SyntaxTrees,
            expression: ExpressionHandle,
            subject: ExpressionHandle,
            fields: &[DestructureBinding],
            captures: &mut SubjectCopies,
        ) -> ExpressionHandle {
            if expression.is_valid() {
                rewrite_expression(syntax_trees, expression, subject, fields, captures)
            } else {
                expression
            }
        }

        fn rewrite_expression_span(
            syntax_trees: &mut SyntaxTrees,
            expressions: HandleSpan<ExpressionHandle>,
            subject: ExpressionHandle,
            fields: &[DestructureBinding],
            captures: &mut SubjectCopies,
        ) -> HandleSpan<ExpressionHandle> {
            let source_expressions = syntax_trees
                .expressions
                .expression_handles(expressions)
                .to_vec();
            let expressions = source_expressions
                .iter()
                .map(|expression| {
                    rewrite_expression(syntax_trees, *expression, subject, fields, captures)
                })
                .collect::<Vec<_>>();
            exprs(syntax_trees).insert_expression_handles(expressions)
        }

        fn single_destructured_field_name(
            syntax_trees: &SyntaxTrees,
            path: HandleSpan<Identifier>,
            fields: &[DestructureBinding],
        ) -> Option<(
            Identifier,
            Option<Identifier>,
            language_core::PatternComponent,
        )> {
            let members = exprs_of(syntax_trees).identifier_path_members(path);
            let [member] = members else {
                return None;
            };
            fields
                .iter()
                .find(|field| field.binding.as_str(syntax_trees) == member.as_str(syntax_trees))
                .map(|field| {
                    (
                        field.member.clone(),
                        field.case_variant.clone(),
                        field.component,
                    )
                })
        }
    }

    pub(crate) struct DestructureFields {
        pub(crate) fields: Vec<TableMatchField>,
        pub(crate) has_rest: bool,
        pub(crate) proof_selectors: Vec<TableOutcomeProofSelector>,
    }

    /// A destructure pattern's `{ fields; proof selectors }`. The field
    /// values are expressions, so the expression frame machine parses it.
    pub(crate) fn parse_destructure_fields<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, DestructureFields> {
        use crate::grammar::expressions::frames::{Goal, Parsed, run};
        match run(syntax_trees, input, Goal::DestructureFields)? {
            (Parsed::DestructureFields(fields), rest) => Ok((fields, rest)),
            (Parsed::Expression(_), _) => {
                unreachable!("a destructure-fields goal finishes with fields")
            }
        }
    }
}

pub(crate) mod type_syntax {
    pub(crate) mod parse_type {
        use crate::grammar::contracts::conformance::parse_conformance::parse_conformance_trait_application;
        use crate::grammar::expressions::static_argument_values::parse_static_argument_expression;
        use crate::storage::tables::type_refs;
        use crate::tokens::token_cursor::{Input, ParseResult};
        use arena::HandleSpan;
        use numerics::arithmetic::ArithmeticDomain;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::ExpressionNode;
        use syntax_trees::identifier::Identifier;
        use syntax_trees::types::{
            DomainConstraint, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle,
            TypeReferenceNode,
        };
        use tokens::PunctuationKind;

        pub(crate) fn parse_static_name<'tokens, 'source>(
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, Identifier> {
            let (first, mut input) = input.take_identifier()?;
            if !input.at_punctuation(PunctuationKind::ColonColon) {
                return Ok((first, input));
            }
            let mut name = first.as_str(&input).to_owned();
            let mut source_span = first.source_span();
            while input.at_punctuation(PunctuationKind::ColonColon) {
                input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
                let (member, rest) = input.take_identifier()?;
                name.push_str("::");
                name.push_str(member.as_str(&input));
                source_span.span.end = member.source_span().span.end;
                input = rest;
            }
            Ok((Identifier::new(name, source_span), input))
        }

        pub(crate) fn parse_type_reference_handle<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
            parse_type_reference_handle_with_trailing_domain(syntax_trees, input, true)
        }

        pub(crate) fn parse_cast_target_type_reference_handle<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
            parse_type_reference_handle_with_trailing_domain(syntax_trees, input, false)
        }

        fn parse_type_reference_handle_with_trailing_domain<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
            allow_trailing_domain: bool,
        ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
            let outer_depth = input.depth();
            // Every expression a type contains (an array extent, a constant
            // argument, a range bound) is a constant position.
            let outer_constant = input.constant_position();
            let input = input.with_constant_position(true).deepen()?;
            let (type_reference, rest) =
                parse_type_reference_handle_inner(syntax_trees, input, allow_trailing_domain)?;
            Ok((
                type_reference,
                rest.with_depth(outer_depth)
                    .with_constant_position(outer_constant),
            ))
        }

        fn parse_type_reference_handle_inner<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
            allow_trailing_domain: bool,
        ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
            if input.at_punctuation(PunctuationKind::Ampersand) {
                return parse_borrow_type_reference(syntax_trees, input, allow_trailing_domain);
            }
            if input.at_punctuation(PunctuationKind::LeftParen) {
                let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
                let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
                return Ok((type_refs(syntax_trees).insert_unit(), input));
            }

            if input.at_punctuation(PunctuationKind::LeftBracket) {
                let input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
                let (element_type, input) = parse_type_reference_handle(syntax_trees, input)?;
                if input.at_punctuation(PunctuationKind::Semicolon) {
                    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
                    let (expression, input) =
                        crate::grammar::expressions::parse_expression::parse_expression_handle(
                            syntax_trees,
                            input,
                        )?;
                    let length = match syntax_trees.expressions.expression(expression) {
                        ExpressionNode::Integer(value) => {
                            let text = value.to_string();
                            match text.parse::<usize>() {
                                Ok(value) => FixedArrayLength::Literal(value),
                                Err(_) => {
                                    let use_type = syntax_trees
                                        .type_references
                                        .insert(TypeReferenceNode::ConstExpression(expression));
                                    FixedArrayLength::Expression(use_type)
                                }
                            }
                        }
                        ExpressionNode::Name(path) => {
                            let members = syntax_trees.expressions.identifier_path_members(*path);
                            let name = Identifier::new(
                                members
                                    .iter()
                                    .map(|name| name.as_str(syntax_trees))
                                    .collect::<Vec<_>>()
                                    .join("::"),
                                syntax_trees.expressions.source_span(expression),
                            );
                            FixedArrayLength::ConstParameter(name)
                        }
                        _ => {
                            let use_type = syntax_trees
                                .type_references
                                .insert(TypeReferenceNode::ConstExpression(expression));
                            FixedArrayLength::Expression(use_type)
                        }
                    };
                    let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
                    let array =
                        syntax_trees
                            .type_references
                            .insert(TypeReferenceNode::FixedArray {
                                element_type,
                                length,
                            });
                    return if allow_trailing_domain {
                        apply_in_domain_suffix(syntax_trees, array, input)
                    } else {
                        Ok((array, input))
                    };
                }

                let type_reference = syntax_trees
                    .type_references
                    .insert(TypeReferenceNode::Slice { element_type });

                let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
                return if allow_trailing_domain {
                    apply_in_domain_suffix(syntax_trees, type_reference, input)
                } else {
                    Ok((type_reference, input))
                };
            }

            if input.at_keyword(tokens::KeywordKind::Machine) {
                let source_span = input.current_source_span();
                let input = input.take_keyword(tokens::KeywordKind::Machine, "machine")?;
                let (signature, input) =
                    crate::grammar::parameters::contracts::parse_runtime_machine_contract(
                        syntax_trees,
                        Identifier::new("$callable", source_span),
                        input,
                    )?;
                let signature = syntax_trees.items.insert_state_signature(&signature);
                return Ok((
                    type_refs(syntax_trees).insert(TypeReferenceNode::Callable { signature }),
                    input,
                ));
            }

            if input.at_contextual("dyn") {
                let input = input.take_contextual("dyn")?;
                let ((trait_name, lifetime_arguments, trait_arguments), input) =
                    parse_conformance_trait_application(syntax_trees, input)?;
                if !lifetime_arguments.is_empty() {
                    return Err(input.error_here(
                        "a dynamic selector takes no lifetime arguments; it borrows nothing",
                    ));
                }
                if input.at_punctuation(PunctuationKind::ColonColon) {
                    return Err(input.error_here(
                "`dyn Data::Implementation` is retired; name the trait application (`dyn Trait<Data>`) and select an implementation with `Implementation as dyn Trait<Data>`",
            ));
                }
                return Ok((
                    type_refs(syntax_trees).insert(TypeReferenceNode::DynamicTrait {
                        trait_name,
                        trait_arguments,
                    }),
                    input,
                ));
            }

            let authored = input;
            let checkpoint = syntax_trees.append_checkpoint();
            let (base_name, input) = parse_static_name(input)?;
            let (reference, rest) =
                parse_named_type_reference(syntax_trees, base_name, input, allow_trailing_domain)?;
            if !rest.at_punctuation(PunctuationKind::ColonColon) {
                return Ok((reference, rest));
            }
            // An application followed by a member cannot be flattened into the
            // ordinary Generic node: Trait<T>::member and Trait::member<T>
            // apply different declarations. Reuse the scoped selection syntax.
            syntax_trees.rewind_appended(checkpoint);
            let Some((selection, rest)) =
                crate::grammar::expressions::parse_postfix::parse_static_name_selection(
                    syntax_trees,
                    authored,
                )?
            else {
                return Err(authored.error_here("expected a scoped type selection"));
            };
            let reference =
                type_refs(syntax_trees).insert(TypeReferenceNode::StaticSelection(selection));
            if allow_trailing_domain {
                apply_in_domain_suffix(syntax_trees, reference, rest)
            } else {
                Ok((reference, rest))
            }
        }

        pub(crate) fn parse_named_type_reference<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            base_name: Identifier,
            mut input: Input<'tokens, 'source>,
            allow_trailing_domain: bool,
        ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
            let type_reference = if input.at_punctuation(PunctuationKind::Less) {
                input = input.take_punctuation(PunctuationKind::Less, "<")?;
                let mut lifetime_arguments = Vec::new();
                let mut arguments: Vec<TypeReferenceHandle> = Vec::new();

                loop {
                    if input.at_punctuation(PunctuationKind::Apostrophe) {
                        if !arguments.is_empty() {
                            return Err(input.error_here(
                                "lifetime arguments precede type, const, and machine arguments",
                            ));
                        }
                        let input_after_tick =
                            input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
                        let (lifetime, rest) = input_after_tick.take_identifier()?;
                        lifetime_arguments.push(lifetime);
                        input = rest;
                        let Some(next) = input.take_comma()? else {
                            break;
                        };
                        input = next;
                        continue;
                    }

                    let (argument, rest) = parse_static_argument_reference(syntax_trees, input)?;
                    arguments.push(argument);
                    input = rest;
                    let Some(next) = input.take_comma()? else {
                        break;
                    };
                    input = next;
                }

                input = input.take_punctuation(PunctuationKind::Greater, ">")?;
                let argument_count = arguments.len();
                let first_argument = arguments
                    .first()
                    .copied()
                    .unwrap_or_else(TypeReferenceHandle::invalid);
                let arguments = syntax_trees
                    .type_references
                    .insert_type_reference_handles(arguments);
                if base_name.as_str(syntax_trees) == "Join" {
                    return Err(input.error_here(
                        "`Join<T>` is retired: task activation returns a linear `Task<T>`; \
                 settle it with `finish()` or transfer it to another owner",
                    ));
                } else if base_name.as_str(syntax_trees) == "Slice"
                    && lifetime_arguments.is_empty()
                    && argument_count == 1
                {
                    type_refs(syntax_trees).insert(TypeReferenceNode::Slice {
                        element_type: first_argument,
                    })
                } else {
                    type_refs(syntax_trees).insert(TypeReferenceNode::Generic {
                        base_name,
                        lifetime_arguments,
                        arguments,
                    })
                }
            } else {
                type_refs(syntax_trees).insert_named(base_name)
            };

            if input.at_punctuation(PunctuationKind::LeftBracket) {
                return Err(input.error_here(
            "type brackets are retired; express scalar bounds in the owning data or machine `requires` clause, result `ensures` clause, or the value's established facts",
        ));
            }

            let (type_reference, input) = if allow_trailing_domain {
                apply_in_domain_suffix(syntax_trees, type_reference, input)?
            } else {
                (type_reference, input)
            };
            Ok((type_reference, input))
        }

        pub(crate) fn parse_static_argument_reference<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
            if input.at_integer()
                || input
                    .tokens
                    .first()
                    .is_some_and(|token| token.float_literal_kind().is_some())
                || input.at_punctuation(PunctuationKind::Minus)
                || input.at_punctuation(PunctuationKind::Exclamation)
                || input.at_punctuation(PunctuationKind::Tilde)
                || (input.at_punctuation(PunctuationKind::LeftParen)
                    && input
                        .take_punctuation(PunctuationKind::LeftParen, "(")
                        .is_ok_and(|rest| !rest.at_punctuation(PunctuationKind::RightParen)))
                || input.at_contextual("true")
                || input.at_contextual("false")
            {
                let (expression, rest) = parse_static_argument_expression(syntax_trees, input)?
                    .ok_or_else(|| input.error_here("expected static argument expression"))?;
                if let ExpressionNode::Name(_) = syntax_trees.expressions.expression(expression) {
                    let name = Identifier::new(
                        syntax_trees.display_expression_name(expression),
                        syntax_trees.expressions.source_span(expression),
                    );
                    return Ok((type_refs(syntax_trees).insert_named(name), rest));
                }
                Ok((
                    type_refs(syntax_trees).insert(TypeReferenceNode::ConstExpression(expression)),
                    rest,
                ))
            } else if input
                .tokens
                .first()
                .is_some_and(crate::tokens::token_cursor::is_identifier_token_for_parser)
            {
                let checkpoint = syntax_trees.append_checkpoint();
                let const_expression = parse_static_argument_expression(syntax_trees, input)?;
                if let Some((expression, expression_rest)) = const_expression
                    && !matches!(
                        syntax_trees.expressions.expression(expression),
                        ExpressionNode::Name(_)
                    )
                    && (expression_rest.at_punctuation(PunctuationKind::Comma)
                        || expression_rest.at_punctuation(PunctuationKind::Greater))
                {
                    Ok((
                        type_refs(syntax_trees)
                            .insert(TypeReferenceNode::ConstExpression(expression)),
                        expression_rest,
                    ))
                } else {
                    syntax_trees.rewind_appended(checkpoint);
                    parse_type_reference_handle(syntax_trees, input)
                }
            } else {
                parse_type_reference_handle(syntax_trees, input)
            }
        }

        fn apply_in_domain_suffix<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            type_reference: TypeReferenceHandle,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
            if !input.at_contextual("in") {
                return Ok((type_reference, input));
            }
            let mut cursor = input.take_contextual("in")?;
            let mut constraints = Vec::new();
            loop {
                let (domain_name, rest) = parse_static_name(cursor)?;
                let (arguments, rest) = parse_domain_argument_handles(syntax_trees, rest)?;
                if domain_name.as_str(syntax_trees) == "Carry::Portable" {
                    if !arguments.is_empty() {
                        return Err(
                            rest.error_here("compiler carry domains do not take index arguments")
                        );
                    }
                    constraints.extend(language_core::CarryPermission::ALL.map(|permission| {
                        TypeConstraintNode::Domain(DomainConstraint {
                            name: Identifier::generated(permission.name()),
                            arguments: HandleSpan::empty(),
                        })
                    }));
                } else {
                    constraints.push(
                        match ArithmeticDomain::from_name(domain_name.as_str(syntax_trees)) {
                            Some(domain) if arguments.is_empty() => {
                                TypeConstraintNode::ArithmeticDomain(domain)
                            }
                            Some(_) => {
                                return Err(rest.error_here(
                                    "compiler arithmetic domains do not take index arguments",
                                ));
                            }
                            None => TypeConstraintNode::Domain(DomainConstraint {
                                name: domain_name,
                                arguments,
                            }),
                        },
                    );
                }

                if !rest.at_punctuation(PunctuationKind::Ampersand) {
                    cursor = rest;
                    break;
                }
                cursor = rest.take_punctuation(PunctuationKind::Ampersand, "&")?;
            }
            let constraints = type_refs(syntax_trees).insert_constraints(constraints);
            let type_reference =
                syntax_trees
                    .type_references
                    .insert(TypeReferenceNode::Constrained {
                        base_type: type_reference,
                        constraints,
                    });
            Ok((type_reference, cursor))
        }

        pub(crate) fn parse_domain_argument_handles<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, HandleSpan<TypeReferenceHandle>> {
            if !input.at_punctuation(PunctuationKind::Less) {
                return Ok((HandleSpan::empty(), input));
            }
            let mut input = input.take_punctuation(PunctuationKind::Less, "<")?;
            let mut arguments = Vec::new();
            loop {
                let (argument, rest) = parse_static_argument_reference(syntax_trees, input)?;
                input = rest;
                arguments.push(argument);
                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                input = input.take_punctuation(PunctuationKind::Greater, ">")?;
                break;
            }
            Ok((
                type_refs(syntax_trees).insert_type_reference_handles(arguments),
                input,
            ))
        }

        pub(crate) fn parse_type_reference_handle_allowing_borrow<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
            parse_borrow_type_reference(syntax_trees, input, true)
        }

        fn parse_borrow_type_reference<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
            allow_trailing_domain: bool,
        ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
            let (is_reference, lifetime, access, input) =
                if input.at_punctuation(PunctuationKind::Ampersand) {
                    let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
                    let (lifetime, input) = parse_optional_lifetime(input)?;
                    let (access, input) = input.take_reference_access()?;
                    if input.at_contextual("relaxed") {
                        return Err(input.error_here(
                            "`&relaxed` is retired: invariant windows (ch11) supersede the \
                     relax surface -- take the ordinary reference",
                        ));
                    }
                    (true, lifetime, access, input)
                } else {
                    (false, None, language_core::ReferenceAccess::Shared, input)
                };

            let (type_reference, input) = parse_type_reference_handle_with_trailing_domain(
                syntax_trees,
                input,
                allow_trailing_domain,
            )?;
            let type_reference = if is_reference {
                type_refs(syntax_trees).insert(TypeReferenceNode::Reference {
                    referee: type_reference,
                    access,
                    lifetime,
                })
            } else {
                type_reference
            };

            Ok((type_reference, input))
        }

        pub(crate) fn parse_optional_lifetime<'tokens, 'source>(
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, Option<syntax_trees::identifier::Identifier>> {
            if !input.at_punctuation(PunctuationKind::Apostrophe) {
                return Ok((None, input));
            }
            let input = input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
            let (name, input) = input.take_identifier()?;
            Ok((Some(name), input))
        }
    }
    pub(crate) mod properties {
        use crate::tokens::token_cursor::{Input, ParseResult};
        use syntax_trees::item::DataProperties;
        use tokens::PunctuationKind;

        pub(crate) fn parse_property_brackets<'tokens, 'source>(
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, DataProperties> {
            let mut properties = DataProperties::default();
            let mut declared_multiplicity: Option<&'static str> = None;
            if !input.at_punctuation(PunctuationKind::LeftBracket) {
                return Ok((properties, input));
            }

            let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
            while !input.at_punctuation(PunctuationKind::RightBracket) {
                let (name, next) = input.take_identifier()?;
                match name.as_str(&input) {
                    "copy" | "linear" => {
                        let spelling = if name.as_str(&input) == "copy" {
                            "copy"
                        } else {
                            "linear"
                        };
                        if let Some(previous) = declared_multiplicity {
                            let message = if previous == spelling {
                                format!("duplicate type property `{spelling}`")
                            } else {
                                format!(
                                    "type properties `[copy]` and `[linear]` are mutually exclusive; \
                             `{previous}` was already declared"
                                )
                            };
                            return Err(next.error_here(message));
                        }
                        declared_multiplicity = Some(spelling);
                        properties.multiplicity = if spelling == "copy" {
                            language_core::Multiplicity::Unrestricted
                        } else {
                            language_core::Multiplicity::Linear
                        };
                        input = next;
                    }
                    "zero_init" => {
                        return Err(next.error_here(
                    "type property `[zero_init]` is retired; whether zeroed storage establishes this type is derived from its default domain and zero-case payload",
                ));
                    }
                    "carry" => {
                        if properties.carry.is_some() {
                            return Err(next.error_here("duplicate type property `carry`"));
                        }
                        let (policy, rest) = parse_carry_policy(next)?;
                        properties.carry = Some(policy);
                        input = rest;
                    }
                    "send" => {
                        return Err(next.error_here(
                    "type property `[send]` is retired; declare the required four-axis `[carry(...)]` policy",
                ));
                    }
                    "sized" => {
                        return Err(next.error_here(
                    "type property `sized` is computed from the data shape and cannot be declared",
                ));
                    }
                    other => {
                        return Err(next.error_here(format!(
                    "unknown type property `{other}`; declared properties are `copy`, `linear`, `carry(...)`"
                )));
                    }
                }

                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                break;
            }

            let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
            Ok((properties, input))
        }

        fn parse_carry_policy<'tokens, 'source>(
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, language_core::CarryPolicy> {
            use language_core::{
                CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension,
            };

            let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let mut suspension = None;
            let mut cpu = None;
            let mut host_thread = None;
            let mut address = None;

            while !input.at_punctuation(PunctuationKind::RightParen) {
                let (axis, next) = input.take_identifier()?;
                let next = next.take_punctuation(PunctuationKind::Colon, ":")?;
                let (value, rest) = next.take_identifier()?;
                match axis.as_str(&input) {
                    "suspension" => {
                        if suspension.is_some() {
                            return Err(rest.error_here("duplicate carry axis `suspension`"));
                        }
                        suspension = Some(match value.as_str(&input) {
                            "forbidden" => CarrySuspension::Forbidden,
                            "allowed" => CarrySuspension::Allowed,
                            other => {
                                return Err(rest.error_here(format!(
                            "unknown carry suspension `{other}`; expected `forbidden` or `allowed`"
                        )));
                            }
                        });
                    }
                    "cpu" => {
                        if cpu.is_some() {
                            return Err(rest.error_here("duplicate carry axis `cpu`"));
                        }
                        cpu = Some(match value.as_str(&input) {
                            "same" => CarryCpu::Origin,
                            "any" => CarryCpu::Any,
                            other => {
                                return Err(rest.error_here(format!(
                                    "unknown carry CPU affinity `{other}`; expected `same` or `any`"
                                )));
                            }
                        });
                    }
                    "thread" => {
                        if host_thread.is_some() {
                            return Err(rest.error_here("duplicate carry axis `thread`"));
                        }
                        host_thread = Some(match value.as_str(&input) {
                            "same" => CarryHostThread::Origin,
                            "any" => CarryHostThread::Any,
                            other => {
                                return Err(rest.error_here(format!(
                            "unknown carry host-thread affinity `{other}`; expected `same` or `any`"
                        )));
                            }
                        });
                    }
                    "address" => {
                        if address.is_some() {
                            return Err(rest.error_here("duplicate carry axis `address`"));
                        }
                        address = Some(match value.as_str(&input) {
                            "stable" => CarryAddress::Stable,
                            "movable" => CarryAddress::Movable,
                            other => {
                                return Err(rest.error_here(format!(
                            "unknown carry address policy `{other}`; expected `stable` or `movable`"
                        )));
                            }
                        });
                    }
                    other => {
                        return Err(rest.error_here(format!(
                    "unknown carry axis `{other}`; expected `suspension`, `cpu`, `thread`, or `address`"
                )));
                    }
                }
                input = rest;
                let Some(next) = input.take_comma()? else {
                    break;
                };
                input = next;
            }

            let missing = [
                ("suspension", suspension.is_none()),
                ("cpu", cpu.is_none()),
                ("thread", host_thread.is_none()),
                ("address", address.is_none()),
            ]
            .into_iter()
            .filter_map(|(axis, absent)| absent.then_some(axis))
            .collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(input.error_here(format!(
                    "carry policy must state all four axes; missing {}",
                    missing.join(", ")
                )));
            }

            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((
                CarryPolicy {
                    suspension: suspension.expect("checked above"),
                    cpu: cpu.expect("checked above"),
                    host_thread: host_thread.expect("checked above"),
                    address: address.expect("checked above"),
                },
                input,
            ))
        }
    }
    #[cfg(test)]
    pub(crate) mod qualified_names {
        use source::SourceId;
        use source_files_to_tokens::Lexer;
        use syntax_trees::SyntaxTrees;
        #[test]
        fn boolean_domain_indices_retain_literal_identity_and_span_in_qualifications_and_casts() {
            for spelling in ["true", "false"] {
                for cast in [false, true] {
                    let text = if cast {
                        format!(
                            "machine convert(value: u64) {{ let tagged: u64 = value as u64 in Tagged<{spelling}>; }}"
                        )
                    } else {
                        format!("data Packet {{ value: u64 in policy::Tagged<{spelling}>; }}")
                    };
                    let source_id = SourceId(17);
                    let tokens = Lexer::new(&text)
                        .tokenize()
                        .expect("tokenize boolean domain index");
                    let mut syntax = SyntaxTrees::new(SourceId::default());
                    crate::parse(&mut syntax, source_id, &tokens)
                        .expect("parse boolean domain index");
                    let arguments = if cast {
                        syntax
                            .expressions
                            .iter_expressions()
                            .find_map(|(_, expression)| {
                                let syntax_trees::expression::ExpressionNode::Cast(cast) =
                                    expression
                                else {
                                    return None;
                                };
                                Some(cast.semantic_domain_arguments)
                            })
                            .expect("indexed cast")
                    } else {
                        syntax.type_references.domain_constraints()[0].arguments
                    };
                    let [argument] = syntax.type_references.type_reference_handles(arguments)
                    else {
                        panic!("one boolean index");
                    };
                    let syntax_trees::types::TypeReferenceNode::ConstExpression(expression) =
                        syntax.type_references.type_reference(*argument)
                    else {
                        panic!("boolean literal retains its authored value expression");
                    };
                    let syntax_trees::expression::ExpressionNode::Boolean(value) =
                        syntax.expressions.expression(*expression)
                    else {
                        panic!("boolean index remains a Boolean literal");
                    };
                    assert_eq!(*value, spelling == "true");
                    let expected_start = text.find(spelling).expect("authored boolean literal");
                    let source_span = syntax.expressions.source_span(*expression);
                    assert_eq!(source_span.source_id, source_id);
                    assert_eq!(source_span.span.start, expected_start);
                    assert_eq!(source_span.span.end, expected_start + spelling.len());
                }
            }
        }

        #[test]
        fn qualified_type_names_reject_missing_path_members() {
            for name in ["dungeon::", "dungeon::::Damage", "dungeon::<u64>"] {
                let text = format!("data Packet {{ value: {name}; }}");
                let tokens = Lexer::new(&text)
                    .tokenize()
                    .expect("tokenize invalid qualified type");
                crate::parse_syntax_trees(&tokens).expect_err("every separator needs a member");
            }
        }
    }
    #[cfg(test)]
    pub(crate) mod remainder_tests {
        use source_files_to_tokens::Lexer;
        use syntax_trees::item::{DataMember, Item};
        use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

        #[test]
        fn division_and_remainder_const_arguments_retain_operations_before_semantic_admission() {
            for operand in [
                "7 % 2",
                "8 % 2",
                "7u64 % 2",
                "7 + 8 % 2",
                "7 / 2",
                "7 / 2 * 2",
                "7u64 / 2 * 2",
                "7 + 8 / 2",
                "(7 / 2) * 2",
                "(7u64 / 2) * 2",
                "7.0",
                "7.0f64",
                "0.1 + 0.9",
                "7 / 2.0 * 2",
                "7.0 / 2 * 2",
                "(7.0 / 2) * 2",
            ] {
                for domain in [false, true] {
                    let field_type = if domain {
                        format!("u64 in Quantity<{operand}>")
                    } else {
                        format!("Buffer<{operand}>")
                    };
                    let source = format!("data Main {{ value: {field_type}; }}");
                    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
                    let syntax =
                        crate::parse_syntax_trees(&tokens).expect("parse const arithmetic syntax");
                    let data = syntax
                        .root_items()
                        .find_map(|item| match item {
                            Item::Data(data) => Some(data),
                            _ => None,
                        })
                        .expect("data");
                    let DataMember::Field(field) = &syntax.items.data_members(data.members)[0]
                    else {
                        panic!("field");
                    };
                    let arguments =
                        match syntax.type_references.type_reference(field.type_reference) {
                            TypeReferenceNode::Generic { arguments, .. } => *arguments,
                            TypeReferenceNode::Constrained { constraints, .. } => {
                                let [TypeConstraintNode::Domain(domain)] =
                                    syntax.type_references.constraints(*constraints)
                                else {
                                    panic!("domain");
                                };
                                domain.arguments
                            }
                            _ => panic!("argument owner"),
                        };
                    let [argument] = syntax.type_references.type_reference_handles(arguments)
                    else {
                        panic!("one argument");
                    };
                    assert!(
                        matches!(
                            syntax.type_references.type_reference(*argument),
                            TypeReferenceNode::ConstExpression(_)
                        ),
                        "{source}"
                    );
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) mod static_argument_delimiters_tests {
        use source_files_to_tokens::Lexer;
        use syntax_trees::expression::{BinaryOperator, ExpressionNode};
        use syntax_trees::item::{DataMember, Item};
        use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

        #[test]
        fn nested_static_values_keep_closers_shifts_and_named_argument_transactions() {
            for operand in [
                "1u64",
                "N + 1u64",
                "8u64 >> 1u64",
                "(N + 8u64) >> 1u64",
                "N",
            ] {
                for owner in ["type", "domain", "implementation"] {
                    let field_type = if owner == "domain" {
                        format!("u64 in Quantity<Inner<{operand}>>")
                    } else {
                        format!("Outer<Inner<{operand}>>")
                    };
                    let text = if owner == "implementation" {
                        format!("Selected satisfies Outer<{operand}> {{}}")
                    } else {
                        format!("data Packet {{ value: {field_type}; }}")
                    };
                    let tokens = Lexer::new(&text).tokenize().unwrap();
                    let syntax =
                        crate::parse_syntax_trees(&tokens).expect("nested static argument");
                    let argument = if owner == "implementation" {
                        let implementation = syntax
                            .root_items()
                            .find_map(|item| match item {
                                Item::Conformance(implementation) => Some(implementation),
                                _ => None,
                            })
                            .unwrap();
                        let [argument] = syntax
                            .type_references
                            .type_reference_handles(implementation.trait_arguments)
                        else {
                            panic!("one implementation argument");
                        };
                        *argument
                    } else {
                        let data = syntax
                            .root_items()
                            .find_map(|item| match item {
                                Item::Data(data) => Some(data),
                                _ => None,
                            })
                            .unwrap();
                        let DataMember::Field(field) = &syntax.items.data_members(data.members)[0]
                        else {
                            panic!("field");
                        };
                        let outer_arguments =
                            match syntax.type_references.type_reference(field.type_reference) {
                                TypeReferenceNode::Generic { arguments, .. } => *arguments,
                                TypeReferenceNode::Constrained { constraints, .. } => {
                                    let [TypeConstraintNode::Domain(domain)] =
                                        syntax.type_references.constraints(*constraints)
                                    else {
                                        panic!("domain");
                                    };
                                    domain.arguments
                                }
                                _ => panic!("outer application"),
                            };
                        let [inner] = syntax
                            .type_references
                            .type_reference_handles(outer_arguments)
                        else {
                            panic!("one inner application");
                        };
                        let TypeReferenceNode::Generic { arguments, .. } =
                            syntax.type_references.type_reference(*inner)
                        else {
                            panic!("inner application");
                        };
                        let [argument] = syntax.type_references.type_reference_handles(*arguments)
                        else {
                            panic!("one static value");
                        };
                        *argument
                    };
                    if operand == "N" {
                        let TypeReferenceNode::Named(name) =
                            syntax.type_references.type_reference(argument)
                        else {
                            panic!("named argument");
                        };
                        assert_eq!(name.as_str(&syntax), "N");
                        assert_eq!(
                            syntax.expressions.iter_expressions().count(),
                            0,
                            "rejected expression alternatives must not publish detached rows"
                        );
                        continue;
                    }
                    let TypeReferenceNode::ConstExpression(expression) =
                        syntax.type_references.type_reference(argument)
                    else {
                        panic!("retained static expression");
                    };
                    match syntax.expressions.expression(*expression) {
                        ExpressionNode::Integer(_) => assert_eq!(operand, "1u64"),
                        ExpressionNode::Binary(binary) => {
                            assert_eq!(
                                binary.operator,
                                if operand.contains(">>") {
                                    BinaryOperator::ShiftRight
                                } else {
                                    BinaryOperator::Add
                                }
                            );
                            assert!(matches!(
                                syntax.expressions.expression(binary.right),
                                ExpressionNode::Integer(_)
                            ));
                        }
                        _ => panic!("authored literal or computation"),
                    }
                }
            }
        }
    }
}
