/*
Contract clauses, conformance external bindings, proof facts, and callable
signatures (`grammar::contracts`) alongside generic, state, and return
parameter lists (`grammar::parameters`).
*/

pub(crate) mod contracts {
    pub(crate) mod conformance {
        pub(crate) mod external_binding {
            use crate::tokens::token_cursor::{Input, ParseResult};
            use syntax_trees::item::ExternalBinding;
            use tokens::PunctuationKind;

            pub(crate) fn parse_external_provider_binding<'tokens, 'source>(
                input: Input<'tokens, 'source>,
            ) -> ParseResult<'tokens, 'source, ExternalBinding> {
                let start = input;
                let (root, input) = input.take_identifier()?;
                if root.as_str(&input) != "ForeignBinding" {
                    return Err(start.error_here(
            "an external realization must construct the compiler-known ForeignBinding sum; \
             write `via ForeignBinding::Syscall(n)` or another qualified `ForeignBinding::Case`",
        ));
                }
                let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
                parse_provider_binding_case(input)
            }

            fn parse_provider_binding_case<'tokens, 'source>(
                input: Input<'tokens, 'source>,
            ) -> ParseResult<'tokens, 'source, ExternalBinding> {
                let (case, input) = input.take_identifier()?;
                match case.as_str(&input) {
        "Syscall" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (number, input) = input.take_integer()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::Syscall { number }, input))
        }
        "VtableSlot" => Err(input.error_here(
            "`ForeignBinding::VtableSlot` is retired; declare the foreign table layout and use \
                 `ForeignBinding::VtableField(field)`",
        )),
        "DllImport" => Err(input.error_here(
            "`ForeignBinding::DllImport(\"module\", \"symbol\")` is retired; return the \
             compiler-owned `ForeignBinding::DllImport { import: DllImport::Case { .. } }` \
             value from one `via` producer machine instead",
        )),
        "CompilerIntrinsic" => Ok((ExternalBinding::CompilerIntrinsic, input)),
        "TableFunction" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (field, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::TableFunction { field }, input))
        }
        "VtableField" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (field, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::VtableField { field }, input))
        }
        other => Err(input.error_here(format!(
            "unknown ForeignBinding case `{other}`: external leaves require one of \
             `ForeignBinding::Syscall(n)`, \
             `ForeignBinding::CompilerIntrinsic`, \
             `ForeignBinding::VtableField(field)`, or `ForeignBinding::TableFunction(field)`; \
             imports evaluate a `ForeignBinding::DllImport {{ .. }}` producer through `via`"
        ))),
    }
            }
        }
        pub(crate) mod parse_conformance {
            use crate::diagnostics::parse_error::ParseError;
            use crate::grammar::expressions::parse_expression::parse_expression_handle;
            use crate::grammar::type_syntax::parse_type::parse_static_argument_reference;
            use crate::storage::tables::{items, type_refs};
            use crate::tokens::token_cursor::{Input, ParseResult};
            use arena::HandleSpan;
            use syntax_trees::SyntaxTrees;
            use syntax_trees::identifier::Identifier;
            use syntax_trees::item::SatisfiesClause;
            use syntax_trees::types::TypeReferenceHandle;
            use tokens::{KeywordKind, PunctuationKind};

            pub(crate) fn parse_satisfies_traits<'tokens, 'source>(
                syntax_trees: &mut SyntaxTrees,
                mut input: Input<'tokens, 'source>,
            ) -> ParseResult<'tokens, 'source, HandleSpan<SatisfiesClause>> {
                if !input.at_contextual("satisfies") {
                    return Ok((HandleSpan::empty(), input));
                }

                input = input.take_contextual("satisfies")?;
                let mut clauses = crate::storage::handle_spans::HandleSpanBuilder::new();

                loop {
                    let ((trait_name, lifetime_arguments, arguments), mut rest) =
            crate::grammar::contracts::conformance::parse_conformance::parse_conformance_trait_application(
                syntax_trees,
                input,
            )?;

                    if !rest.at_punctuation(PunctuationKind::ColonColon) {
                        return Err(rest.error_here(format!(
                "bare `satisfies {}` on a machine is retired; bind one exact requirement as `satisfies {}::requirement`",
                trait_name.as_str(syntax_trees),
                trait_name.as_str(syntax_trees),
            )));
                    }
                    let next = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
                    let (member, next) = next.take_identifier()?;
                    let requirement = Some(member);
                    rest = next;
                    let mut alias = None;
                    if rest.at_keyword(KeywordKind::As) {
                        let next = rest.take_keyword(KeywordKind::As, "as")?;
                        let (name, next) = next.take_identifier()?;
                        alias = Some(name);
                        rest = next;
                    }
                    let mut via = None;
                    let mut via_expression = syntax_trees::expression::ExpressionHandle::invalid();
                    let mut via_keyword_source_span = None;
                    if rest.at_contextual("via") {
                        via_keyword_source_span = Some(rest.current_source_span());
                        let next = rest.take_contextual("via")?;
                        let is_bootstrap_binding = next
                            .take_identifier()
                            .is_ok_and(|(root, _)| root.as_str(syntax_trees) == "ForeignBinding");
                        if is_bootstrap_binding {
                            let (binding, next) =
                                super::external_binding::parse_external_provider_binding(next)?;
                            via = Some(binding);
                            rest = next;
                        } else {
                            let (expression, next) = parse_expression_handle(syntax_trees, next)?;
                            via_expression = expression;
                            rest = next;
                        }
                    }

                    let handle = items(syntax_trees).append_satisfies_clause(SatisfiesClause {
                        trait_name,
                        lifetime_arguments,
                        arguments,
                        requirement,
                        alias,
                        via,
                        via_expression,
                        via_keyword_source_span,
                    });
                    clauses.push(handle);
                    input = rest;
                    let Some(next) = input.take_comma()? else {
                        break;
                    };
                    input = next;
                }

                Ok((clauses.finish(), input))
            }

            pub(crate) fn parse_optional_satisfies_type_arguments<'tokens, 'source>(
                syntax_trees: &mut SyntaxTrees,
                mut input: Input<'tokens, 'source>,
            ) -> Result<(HandleSpan<TypeReferenceHandle>, Input<'tokens, 'source>), ParseError>
            {
                if !input.at_punctuation(PunctuationKind::Less) {
                    return Ok((HandleSpan::empty(), input));
                }

                input = input.take_punctuation(PunctuationKind::Less, "<")?;
                let mut arguments = Vec::new();
                loop {
                    let (argument, rest) = parse_satisfies_type_argument(syntax_trees, input)?;
                    arguments.push(argument);
                    input = rest;

                    if input.at_punctuation(PunctuationKind::Comma) {
                        input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                        continue;
                    }

                    let input = input.take_punctuation(PunctuationKind::Greater, ">")?;
                    return Ok((
                        type_refs(syntax_trees).insert_type_reference_handles(arguments),
                        input,
                    ));
                }
            }

            pub(crate) fn parse_satisfies_type_argument<'tokens, 'source>(
                syntax_trees: &mut SyntaxTrees,
                input: Input<'tokens, 'source>,
            ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
                parse_static_argument_reference(syntax_trees, input)
            }
            pub(crate) fn parse_conformance_trait_application<'tokens, 'source>(
                syntax_trees: &mut SyntaxTrees,
                input: Input<'tokens, 'source>,
            ) -> ParseResult<
                'tokens,
                'source,
                (Identifier, Vec<Identifier>, HandleSpan<TypeReferenceHandle>),
            > {
                let (trait_name, mut rest) = input.take_identifier()?;
                let mut lifetime_arguments = Vec::new();
                let trait_arguments = if rest.at_punctuation(PunctuationKind::Less) {
                    rest = rest.take_punctuation(PunctuationKind::Less, "<")?;
                    let mut arguments = Vec::new();
                    let mut saw_non_lifetime = false;
                    loop {
                        if rest.at_punctuation(PunctuationKind::Apostrophe) {
                            if saw_non_lifetime {
                                return Err(rest.error_here(
                                    "lifetime arguments precede type, const, and machine arguments",
                                ));
                            }
                            rest = rest.take_punctuation(PunctuationKind::Apostrophe, "'")?;
                            let (lifetime, next) = rest.take_identifier()?;
                            lifetime_arguments.push(lifetime);
                            rest = next;
                        } else {
                            saw_non_lifetime = true;
                            let (argument, next) =
                    crate::grammar::contracts::conformance::parse_conformance::parse_satisfies_type_argument(
                        syntax_trees,
                        rest,
                    )?;
                            arguments.push(argument);
                            rest = next;
                        }
                        if rest.at_punctuation(PunctuationKind::Comma) {
                            rest = rest.take_punctuation(PunctuationKind::Comma, ",")?;
                            continue;
                        }
                        rest = rest.take_punctuation(PunctuationKind::Greater, ">")?;
                        break;
                    }
                    type_refs(syntax_trees).insert_type_reference_handles(arguments)
                } else {
                    HandleSpan::empty()
                };
                Ok(((trait_name, lifetime_arguments, trait_arguments), rest))
            }
        }
    }
    pub(crate) mod facts {
        use crate::diagnostics::parse_error::ParseError;
        use crate::grammar::declarations::parse_declaration::starts_following_item;
        use crate::grammar::expressions::parse_expression::parse_expression_handle_without_struct_literals_or_membership;
        use crate::grammar::expressions::parse_expression::parse_logical_proof_fact_expression_handle;
        use crate::grammar::expressions::parse_expression::parse_proof_fact_expression_handle;
        use crate::storage::tables::{exprs, items};
        use crate::tokens::token_cursor::{Input, parse_path_handle_span};
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
            TableCallExpression, TableMembershipExpression,
        };
        use syntax_trees::identifier::Identifier;
        use syntax_trees::item::{ProofFact, ProofMembershipFact};
        use syntax_trees::types::TypeReferenceHandle;
        use tokens::PunctuationKind;

        fn copy_item_path_to_expression_path(
            syntax_trees: &mut SyntaxTrees,
            span: HandleSpan<Identifier>,
        ) -> HandleSpan<Identifier> {
            let mut copy = crate::storage::handle_spans::HandleSpanBuilder::new();
            let members = syntax_trees
                .tables
                .items
                .identifier_path_members(span)
                .to_vec();

            for member in members {
                copy.push(
                    syntax_trees
                        .tables
                        .expressions
                        .append_identifier_path_member(member),
                );
            }

            copy.finish()
        }

        type MembershipDomain = (HandleSpan<Identifier>, HandleSpan<TypeReferenceHandle>);

        fn expand_carry_portable_item_paths(
            syntax_trees: &mut SyntaxTrees,
            domains: Vec<MembershipDomain>,
        ) -> Vec<MembershipDomain> {
            let mut expanded = Vec::new();
            for (domain, domain_arguments) in domains {
                let name = syntax_trees
                    .tables
                    .items
                    .identifier_path_members(domain)
                    .iter()
                    .map(|member| member.as_str(syntax_trees))
                    .collect::<Vec<_>>()
                    .join("::");
                if name != "Carry::Portable" {
                    expanded.push((domain, domain_arguments));
                    continue;
                }

                for permission in language_core::CarryPermission::ALL {
                    let mut parts = permission.name().split("::");
                    let namespace =
                        items(syntax_trees).append_identifier_path_member(Identifier::generated(
                            parts.next().expect("carry permission has a namespace"),
                        ));
                    let member = items(syntax_trees).append_identifier_path_member(
                        Identifier::generated(parts.next().expect("carry permission has a member")),
                    );
                    debug_assert_eq!(member.arena_index(), namespace.arena_index() + 1);
                    expanded.push((HandleSpan::from_parts(namespace, 2), HandleSpan::empty()));
                }
            }
            expanded
        }

        pub(crate) fn parse_proof_facts_until<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
            is_terminator: impl FnMut(Input<'tokens, 'source>) -> bool,
        ) -> Result<((HandleSpan<ProofFact>, usize), Input<'tokens, 'source>), ParseError> {
            parse_proof_facts_until_with_machine_semicolon(
                syntax_trees,
                input,
                is_terminator,
                false,
            )
        }

        pub(crate) fn parse_proof_facts_until_with_machine_semicolon<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
            is_terminator: impl FnMut(Input<'tokens, 'source>) -> bool,
            machine_final_semicolon: bool,
        ) -> Result<((HandleSpan<ProofFact>, usize), Input<'tokens, 'source>), ParseError> {
            parse_fact_list(
                syntax_trees,
                input,
                is_terminator,
                machine_final_semicolon,
                false,
                false,
            )
        }

        pub(crate) fn parse_required_proof_facts_until<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
            is_terminator: impl FnMut(Input<'tokens, 'source>) -> bool,
            machine_final_semicolon: bool,
        ) -> Result<((HandleSpan<ProofFact>, usize), Input<'tokens, 'source>), ParseError> {
            parse_fact_list(
                syntax_trees,
                input,
                is_terminator,
                machine_final_semicolon,
                false,
                true,
            )
        }

        pub(crate) fn parse_required_logical_proof_facts_until<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
            is_terminator: impl FnMut(Input<'tokens, 'source>) -> bool,
        ) -> Result<((HandleSpan<ProofFact>, usize), Input<'tokens, 'source>), ParseError> {
            parse_fact_list(syntax_trees, input, is_terminator, true, true, true)
        }

        fn parse_fact_list<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
            mut is_terminator: impl FnMut(Input<'tokens, 'source>) -> bool,
            machine_final_semicolon: bool,
            logical_subject: bool,
            requires_first_fact: bool,
        ) -> Result<((HandleSpan<ProofFact>, usize), Input<'tokens, 'source>), ParseError> {
            let mut input = input;
            let body_start_tokens = input.tokens.len();
            let mut facts = crate::storage::handle_spans::HandleSpanBuilder::new();

            while (requires_first_fact && facts.is_empty()) || !is_terminator(input) {
                if input.tokens.is_empty() {
                    return Err(input.error_here("expected proof fact terminator"));
                }

                let fact_input = input;
                let mut authored_fact_handles = Vec::new();
                let (value, rest) = if logical_subject {
                    parse_logical_proof_fact_expression_handle(syntax_trees, input)?
                } else {
                    parse_proof_fact_expression_handle(syntax_trees, input)?
                };
                input = rest;

                if input.at_contextual("in") {
                    input = input.take_contextual("in")?;
                    if input.at_name_like() {
                        let (first_domain, rest) = parse_path_handle_span(input, |member| {
                            items(syntax_trees).append_identifier_path_member(member)
                        })?;
                        let argument_start = rest;
                        let (first_arguments, rest) =
                            parse_proof_fact_domain_arguments(syntax_trees, first_domain, rest)?;
                        let mut argument_site =
                            (!first_arguments.is_empty()).then_some(argument_start);
                        input = rest;

                        let first_expression_domain =
                            copy_item_path_to_expression_path(syntax_trees, first_domain);
                        let first_membership = exprs(syntax_trees).insert(
                            ExpressionNode::Membership(TableMembershipExpression {
                                value,
                                domain: first_expression_domain,
                            }),
                        );

                        let mut chain_expression = first_membership;
                        let mut saw_pipe = false;
                        let mut membership_domains = vec![(first_domain, first_arguments)];

                        while input.at_punctuation(PunctuationKind::Ampersand)
                            || input.at_punctuation(PunctuationKind::Pipe)
                        {
                            let (operator, rest) =
                                if input.at_punctuation(PunctuationKind::Ampersand) {
                                    (
                                        BinaryOperator::And,
                                        input.take_punctuation(PunctuationKind::Ampersand, "&")?,
                                    )
                                } else {
                                    saw_pipe = true;
                                    (
                                        BinaryOperator::Or,
                                        input.take_punctuation(PunctuationKind::Pipe, "|")?,
                                    )
                                };
                            let (domain, rest) = parse_path_handle_span(rest, |member| {
                                items(syntax_trees).append_identifier_path_member(member)
                            })?;
                            let argument_start = rest;
                            let (domain_arguments, rest) =
                                parse_proof_fact_domain_arguments(syntax_trees, domain, rest)?;
                            if !domain_arguments.is_empty() && argument_site.is_none() {
                                argument_site = Some(argument_start);
                            }
                            input = rest;
                            membership_domains.push((domain, domain_arguments));
                            let expression_domain =
                                copy_item_path_to_expression_path(syntax_trees, domain);
                            let membership = exprs(syntax_trees).insert(
                                ExpressionNode::Membership(TableMembershipExpression {
                                    value,
                                    domain: expression_domain,
                                }),
                            );
                            chain_expression = exprs(syntax_trees).insert(ExpressionNode::Binary(
                                TableBinaryExpression {
                                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                    left: chain_expression,
                                    operator,
                                    right: membership,
                                },
                            ));
                        }

                        if saw_pipe {
                            if let Some(site) = argument_site {
                                return Err(site.error_here(
                            "indexed domain applications are not carried by `|` proof-fact alternatives; state the indexed membership as its own fact",
                        ));
                            }
                            let handle = syntax_trees
                                .items
                                .append_proof_fact(ProofFact::Expression(chain_expression));
                            authored_fact_handles.push(handle);
                            facts.push(handle);
                        } else {
                            for (domain, domain_arguments) in
                                expand_carry_portable_item_paths(syntax_trees, membership_domains)
                            {
                                let handle = items(syntax_trees).append_proof_fact(
                                    ProofFact::Membership(ProofMembershipFact {
                                        value,
                                        domain,
                                        domain_arguments,
                                    }),
                                );
                                authored_fact_handles.push(handle);
                                facts.push(handle);
                            }
                        }
                    } else {
                        let (start, rest) =
                            parse_expression_handle_without_struct_literals_or_membership(
                                syntax_trees,
                                input,
                            )?;
                        let (end_inclusive, rest) =
                            if rest.at_punctuation(PunctuationKind::DotDotEqual) {
                                (
                                    true,
                                    rest.take_punctuation(PunctuationKind::DotDotEqual, "..=")?,
                                )
                            } else if rest.at_punctuation(PunctuationKind::DotDot) {
                                (false, rest.take_punctuation(PunctuationKind::DotDot, "..")?)
                            } else {
                                return Err(rest.error_here(
                                    "`in` range proof facts require a range separator",
                                ));
                            };
                        let (end, rest) =
                            parse_expression_handle_without_struct_literals_or_membership(
                                syntax_trees,
                                rest,
                            )?;
                        input = rest;
                        let range_fact = range_membership_expression(
                            syntax_trees,
                            value,
                            start,
                            end,
                            end_inclusive,
                        );
                        let handle = syntax_trees
                            .items
                            .append_proof_fact(ProofFact::Expression(range_fact));
                        authored_fact_handles.push(handle);
                        facts.push(handle);
                    }
                } else {
                    let expression = if input.at_name_like()
                        && !is_terminator(input)
                        && !fact_input.has_newline_before(input)
                    {
                        let (predicate, rest) = input.take_identifier()?;
                        input = rest;
                        exprs(syntax_trees).insert(ExpressionNode::Call(TableCallExpression {
                            target_is_static: false,
                            receiver: value,
                            target: predicate,
                            machine_arguments: Box::default(),
                            arguments: HandleSpan::empty(),
                            evidence_arguments: Box::default(),
                            operational_acknowledgement: Default::default(),
                        }))
                    } else {
                        value
                    };
                    let handle = syntax_trees
                        .items
                        .append_proof_fact(ProofFact::Expression(expression));
                    authored_fact_handles.push(handle);
                    facts.push(handle);
                }

                let authored_source_span = fact_input.source_span_until(input);
                for handle in authored_fact_handles {
                    items(syntax_trees).set_proof_fact_source_span(handle, authored_source_span);
                }

                if is_terminator(input) {
                    continue;
                } else if input.at_punctuation(PunctuationKind::Semicolon) {
                    let after = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
                    if machine_final_semicolon
                        && (starts_following_item(syntax_trees, after) || after.tokens.is_empty())
                    {
                        break;
                    }
                    input = after;
                } else if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                } else if fact_input.has_newline_before(input) {
                    continue;
                } else if !is_terminator(input) {
                    return Err(input.error_here("expected `;`, `,`, or end of proof facts"));
                }
            }

            let token_count = body_start_tokens.saturating_sub(input.tokens.len());

            Ok(((facts.finish(), token_count), input))
        }

        fn range_membership_expression(
            syntax_trees: &mut SyntaxTrees,
            value: ExpressionHandle,
            start: ExpressionHandle,
            end: ExpressionHandle,
            end_inclusive: bool,
        ) -> ExpressionHandle {
            let lower =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: value,
                        operator: BinaryOperator::GreaterOrEqual,
                        right: start,
                    }));
            let upper =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: value,
                        operator: if end_inclusive {
                            BinaryOperator::LessOrEqual
                        } else {
                            BinaryOperator::Less
                        },
                        right: end,
                    }));
            exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                left: lower,
                operator: BinaryOperator::And,
                right: upper,
            }))
        }

        fn parse_proof_fact_domain_arguments<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            domain: HandleSpan<Identifier>,
            input: Input<'tokens, 'source>,
        ) -> Result<(HandleSpan<TypeReferenceHandle>, Input<'tokens, 'source>), ParseError>
        {
            if !input.at_punctuation(PunctuationKind::Less) {
                return Ok((HandleSpan::empty(), input));
            }
            let name = syntax_trees
                .tables
                .items
                .identifier_path_members(domain)
                .iter()
                .map(|member| member.as_str(syntax_trees))
                .collect::<Vec<_>>()
                .join("::");
            if name == "Carry::Portable"
                || language_core::CarryPermission::from_name(&name).is_some()
            {
                return Err(
                    input.error_here("compiler carry permissions do not take index arguments")
                );
            }
            crate::grammar::type_syntax::parse_type::parse_domain_argument_handles(
                syntax_trees,
                input,
            )
        }
    }
    pub(crate) mod parse_contract_clauses {
        use crate::diagnostics::parse_error::ParseError;
        use crate::grammar::expressions::parse_expression::{
            parse_expression_handle_without_struct_literals,
            parse_expression_handle_without_struct_literals_or_membership,
        };
        use crate::grammar::parameters::contracts::parse_machine_parameter_contracts;
        use crate::grammar::parameters::parse_parameters::parse_optional_return_type;
        use crate::storage::tables::{exprs, items};
        use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::expression::ExpressionNode;
        use syntax_trees::expression::TableRangeExpression;
        use syntax_trees::identifier::Identifier;
        use syntax_trees::item::{CapabilityContract, CapabilityContractKind, CrashCause};
        use tokens::{KeywordKind, PunctuationKind};

        pub(crate) struct MachineClauses {
            pub terminates_guarantee: bool,
            pub ranking_subjects: HandleSpan<syntax_trees::expression::ExpressionHandle>,
            pub ranking_view: HandleSpan<Identifier>,
            pub ranking_view_arguments: HandleSpan<syntax_trees::expression::ExpressionHandle>,
            pub ranking_range: syntax_trees::expression::ExpressionHandle,
            pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
            pub service_reach_is_installation_bound: bool,
            pub service_reaches: HandleSpan<Identifier>,
            pub invokes: HandleSpan<Identifier>,
            pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
            pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
            pub suspends: bool,
            pub blocks: bool,
            pub contracts: HandleSpan<CapabilityContract>,
            pub return_type: syntax_trees::types::TypeReferenceHandle,
            pub admitted_same_stack_bytes: u64,
            pub same_stack_clause_span: Option<source::SourceSpan>,
            /// The header wrote `crashes none`. That clause publishes no
            /// contract row, so this is the only record that the empty crash
            /// set was authored rather than omitted.
            pub declares_no_crash_route: bool,
        }

        type RankedSubjects = (
            HandleSpan<syntax_trees::expression::ExpressionHandle>,
            HandleSpan<Identifier>,
            HandleSpan<syntax_trees::expression::ExpressionHandle>,
            syntax_trees::expression::ExpressionHandle,
        );

        pub(crate) fn parse_machine_clauses<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            mut input: Input<'tokens, 'source>,
            type_parameters: HandleSpan<syntax_trees::item::TypeParameter>,
        ) -> ParseResult<'tokens, 'source, MachineClauses> {
            let mut terminates_guarantee = false;
            let mut ranking_subjects = HandleSpan::empty();
            let mut ranking_view = HandleSpan::empty();
            let mut ranking_view_arguments = HandleSpan::empty();
            let mut ranking_range = syntax_trees::expression::ExpressionHandle::invalid();
            let mut service_reach_keyword_source_spans = Vec::new();
            let mut service_reach_is_installation_bound = false;
            let mut service_reaches = Vec::new();
            let mut invokes = Vec::new();
            let mut suspends_keyword_source_spans = Vec::new();
            let mut blocks_keyword_source_spans = Vec::new();
            let mut suspends = false;
            let mut blocks = false;
            let mut contracts = Vec::new();
            let mut crashes_none_declared = false;
            let mut outcome_case_groups = Vec::<String>::new();
            let mut public_selectors = Vec::<String>::new();
            let mut return_type = syntax_trees::types::TypeReferenceHandle::invalid();
            let mut admitted_same_stack_bytes = 0u64;
            let mut same_stack_clause_span = None;

            while !input.at_punctuation(PunctuationKind::LeftBrace)
                && !input.at_punctuation(PunctuationKind::Semicolon)
            {
                if input.at_contextual("requires")
                    && input
                        .take_contextual("requires")?
                        .at_keyword(KeywordKind::Machine)
                {
                    let ((), rest) =
                        parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
                    input = rest;
                    continue;
                }
                if input.at_contextual("terminates") {
                    input = input.take_contextual("terminates")?;
                    if input.at_contextual("by") {
                        let by_input = input.take_contextual("by")?;
                        let (
                            (clause_subjects, clause_view, clause_view_arguments, clause_range),
                            rest,
                        ) = parse_ranked_subjects(syntax_trees, by_input)?;
                        input = rest;
                        ranking_subjects = clause_subjects;
                        ranking_view = clause_view;
                        ranking_view_arguments = clause_view_arguments;
                        ranking_range = clause_range;
                        continue;
                    }
                    if input.at_retired_ranking_block() {
                        return Err(input.error_here(
                            "the `terminates { decreases ...; }` block form is retired (decision 23): \
                 attach the ranking witness to the guarantee as `terminates by \
                 <subjects> [-> View] [in <range>]`",
                        ));
                    }
                    terminates_guarantee = true;
                    continue;
                }

                if input.at_contextual("decreases") {
                    return Err(input.error_here(
                        "a standalone `decreases` clause is retired (decision 23): \
                 attach the ranking witness to the guarantee as `terminates by \
                 <subjects> [-> View] [in <range>]`",
                    ));
                }

                if input.at_contextual("increases") {
                    return Err(input.error_here(
                        "a standalone `increases` clause is retired (decision 23): \
                 select a bounded ranking view with `terminates by \
                 <subject> -> Nat::IncreasingTo(<bound>)`",
                    ));
                }

                if input.at_contextual("effects") {
                    return Err(input.error_here(
                        "the `effects` reach clause is retired; write `reaches <Service> + ...`",
                    ));
                }

                if input.at_contextual("reaches") {
                    input = super::take_operational_reaches_clause(
                        input,
                        &mut service_reach_keyword_source_spans,
                        &mut service_reaches,
                        &mut service_reach_is_installation_bound,
                        machine_reaches_clause_boundary,
                    )?;
                    continue;
                }

                if input.at_contextual("invokes") {
                    input = input.take_contextual("invokes")?;
                    let (binding, after_binding) = input.take_identifier()?;
                    invokes.push(binding);
                    input = after_binding;
                    continue;
                }

                if input.at_contextual("suspends") {
                    suspends_keyword_source_spans.push(input.current_source_span());
                    if suspends {
                        return Err(input.error_here("duplicate `suspends` operational clause"));
                    }
                    suspends = true;
                    input = input.take_contextual("suspends")?;
                    continue;
                }

                if input.at_contextual("blocks") {
                    blocks_keyword_source_spans.push(input.current_source_span());
                    if blocks {
                        return Err(input.error_here("duplicate `blocks` operational clause"));
                    }
                    blocks = true;
                    input = input.take_contextual("blocks")?;
                    continue;
                }

                if input.at_same_stack_demand() {
                    if admitted_same_stack_bytes != 0 {
                        return Err(input.error_here("duplicate `stack N bytes` clause"));
                    }
                    same_stack_clause_span = Some(input.current_source_span());
                    input = input.take_contextual("stack")?;
                    let number = input;
                    let (bytes, after_number) = input.take_integer()?;
                    if bytes <= 0 {
                        return Err(number.error_here(
                    "`stack` demand must be a positive byte count; zero does not admit a foreign leaf",
                ));
                    }
                    if !after_number.at_contextual("bytes") {
                        return Err(
                            after_number.error_here("expected `bytes` after the same-stack demand")
                        );
                    }
                    input = after_number.take_contextual("bytes")?;
                    admitted_same_stack_bytes =
                        u64::try_from(bytes).expect("positive i64 fits in u64");
                    continue;
                }

                if input.at_contextual("crashes") {
                    let keyword_source_span = Some(input.current_source_span());
                    let ((cause, header_token_count), after_header) = parse_crash_header(input)?;
                    let Some(cause) = cause else {
                        let other_route = contracts.iter().any(|contract: &CapabilityContract| {
                            matches!(contract.kind, CapabilityContractKind::Crashes { .. })
                        });
                        input = crate::grammar::contracts::finish_crashes_none(
                            after_header,
                            other_route,
                            machine_reaches_clause_boundary,
                        )?;
                        crashes_none_declared = true;
                        continue;
                    };
                    if crashes_none_declared {
                        return Err(input
                            .error_here("a `crashes Cause` route cannot follow `crashes none`"));
                    }
                    let ((facts, fact_token_count), rest) =
                crate::grammar::contracts::facts::parse_proof_facts_until_with_machine_semicolon(
                    syntax_trees,
                    after_header,
                    |input| {
                        input.at_punctuation(PunctuationKind::LeftBrace)
                            || input.at_punctuation(PunctuationKind::Semicolon)
                            || machine_contract_fact_boundary(input)
                    },
                    true,
                )?;
                    contracts.push(CapabilityContract {
                        kind: CapabilityContractKind::Crashes { cause },
                        keyword_source_span,
                        binding: None,
                        facts,
                        token_count: fact_token_count
                            .checked_add(header_token_count)
                            .expect("crash contract token count overflow"),
                    });
                    input = rest;
                    continue;
                }

                if input.at_contextual("boundary") {
                    return Err(input.error_here(
                "trailing `boundary host` and `boundary Name` contract clauses are retired; use a leading `boundary trait`, `boundary machine`, or `boundary operator` declaration and realize exact requirements with `satisfies Trait::requirement via Binding::...`",
            ));
                }

                if input.at_contextual("requires") || input.at_contextual("ensures") {
                    let keyword_source_span = Some(input.current_source_span());
                    let kind = if input.at_contextual("requires") {
                        input = input.take_contextual("requires")?;
                        CapabilityContractKind::Requires
                    } else {
                        input = input.take_contextual("ensures")?;
                        CapabilityContractKind::Ensures
                    };
                    if matches!(kind, CapabilityContractKind::Ensures)
                        && outcome_case_path_followed_by_arrow(input)?
                    {
                        let ((group_contracts, group_name), rest) =
                            parse_outcome_specific_ensures_group(
                                syntax_trees,
                                input,
                                keyword_source_span,
                            )?;
                        if outcome_case_groups
                            .iter()
                            .any(|existing| existing == &group_name)
                        {
                            return Err(input.error_here(format!(
                                "duplicate outcome-specific ensures group for `{group_name}`"
                            )));
                        }
                        outcome_case_groups.push(group_name);
                        for contract in group_contracts {
                            if let Some(binding) = contract.binding.as_ref() {
                                if public_selectors
                                    .iter()
                                    .any(|existing| existing == binding.as_str(syntax_trees))
                                {
                                    return Err(input.error_here(format!(
                                        "duplicate machine-wide public ensures selector `{}`",
                                        binding.as_str(syntax_trees)
                                    )));
                                }
                                public_selectors.push(binding.as_str(syntax_trees).to_owned());
                            }
                            contracts.push(contract);
                        }
                        input = rest;
                        continue;
                    }
                    if matches!(kind, CapabilityContractKind::Ensures) {
                        reject_ambiguous_outcome_specific_ensures(input)?;
                    }
                    let (binding, fact_input) = input.take_optional_label()?;
                    let ((facts, token_count), rest) =
                        crate::grammar::contracts::facts::parse_required_proof_facts_until(
                            syntax_trees,
                            fact_input,
                            |input| {
                                input.at_punctuation(PunctuationKind::LeftBrace)
                                    || machine_contract_fact_boundary(input)
                            },
                            true,
                        )?;
                    if binding.is_some() && facts.count() != 1 {
                        return Err(fact_input.error_here(
                    "a named requires or ensures clause must contain exactly one proposition",
                ));
                    }
                    if matches!(kind, CapabilityContractKind::Ensures)
                        && let Some(binding) = binding.as_ref()
                    {
                        if public_selectors
                            .iter()
                            .any(|existing| existing == binding.as_str(syntax_trees))
                        {
                            return Err(fact_input.error_here(format!(
                                "duplicate machine-wide public ensures selector `{}`",
                                binding.as_str(syntax_trees)
                            )));
                        }
                        public_selectors.push(binding.as_str(syntax_trees).to_owned());
                    }
                    contracts.push(CapabilityContract {
                        kind,
                        keyword_source_span,
                        binding,
                        facts,
                        token_count,
                    });
                    input = rest;
                    continue;
                }

                if input.at_punctuation(PunctuationKind::Arrow) {
                    let (parsed, rest) = parse_optional_return_type(syntax_trees, input)?;
                    return_type = parsed;
                    input = rest;
                    continue;
                }

                return Err(input.expected_one_of_here(&[
                    "`terminates`",
                    "`decreases`",
                    "`reaches`",
                    "`invokes <binding>`",
                    "`suspends`",
                    "`blocks`",
                    "`crashes <Cause>`",
                    "`boundary`",
                    "`requires`",
                    "`ensures`",
                    "`->`",
                    "`{`",
                ]));
            }

            Ok((
                MachineClauses {
                    terminates_guarantee,
                    ranking_subjects,
                    ranking_view,
                    ranking_view_arguments,
                    ranking_range,
                    service_reach_keyword_source_spans,
                    service_reach_is_installation_bound,
                    service_reaches: syntax_trees
                        .items
                        .insert_identifier_path_members(service_reaches),
                    invokes: items(syntax_trees).insert_identifier_path_members(invokes),
                    suspends_keyword_source_spans,
                    blocks_keyword_source_spans,
                    suspends,
                    blocks,
                    contracts: super::publish_capability_contracts(syntax_trees, contracts),
                    return_type,
                    admitted_same_stack_bytes,
                    same_stack_clause_span,
                    declares_no_crash_route: crashes_none_declared,
                },
                input,
            ))
        }

        fn machine_contract_fact_boundary(input: Input<'_, '_>) -> bool {
            input.at_punctuation(PunctuationKind::Semicolon)
                || input.at_keyword(KeywordKind::Machine)
                || input.at_keyword(KeywordKind::Data)
                || input.at_keyword(KeywordKind::Use)
                || input.at_contextual("requires")
                || input.at_contextual("ensures")
                || input.at_contract_continuation_keyword()
                || input.at_contextual("satisfies")
                || input.at_same_stack_demand()
                || input.tokens.is_empty()
        }

        fn outcome_case_path_followed_by_arrow(input: Input<'_, '_>) -> Result<bool, ParseError> {
            if !input.at_name_like() {
                return Ok(false);
            }
            let (_, mut rest) = input.take_identifier()?;
            let mut member_count = 1usize;
            while rest.at_punctuation(PunctuationKind::ColonColon) {
                rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
                let (_, next) = rest.take_identifier()?;
                rest = next;
                member_count += 1;
            }
            Ok(member_count >= 2 && rest.at_punctuation(PunctuationKind::Arrow))
        }

        fn reject_ambiguous_outcome_specific_ensures(
            input: Input<'_, '_>,
        ) -> Result<(), ParseError> {
            let Ok((guard, arrow)) = input.split_at_top_level_punctuation(
                PunctuationKind::Arrow,
                "outcome-specific ensures requires an arrow",
            ) else {
                return Ok(());
            };
            let after_arrow = arrow.take_punctuation(PunctuationKind::Arrow, "->")?;
            if !after_arrow.at_punctuation(PunctuationKind::LeftBrace) {
                return Ok(());
            }
            if guard.tokens.iter().any(|token| {
                token.punctuation() == Some(PunctuationKind::Semicolon)
                    || matches!(
                        input.text(token).as_str(),
                        "requires"
                            | "ensures"
                            | "crashes"
                            | "terminates"
                            | "decreases"
                            | "reaches"
                            | "effects"
                            | "invokes"
                            | "suspends"
                            | "blocks"
                            | "satisfies"
                            | "stack"
                    )
            }) {
                return Ok(());
            }
            if guard
                .tokens
                .iter()
                .any(|token| token.punctuation() == Some(PunctuationKind::EqualEqual))
            {
                return Err(input.error_here(
            "outcome-specific ensures rejects Boolean guards; write the exact declared result case as `Result::Case -> { guarantees }`",
        ));
            }
            if guard.tokens.iter().any(|token| {
                matches!(
                    token.punctuation(),
                    Some(PunctuationKind::LeftParen | PunctuationKind::LeftBrace)
                )
            }) {
                return Err(input.error_here(
            "outcome-specific ensures rejects case-literal-shaped selectors; write only the exact nominal path `Result::Case -> { guarantees }`",
        ));
            }
            Err(input.error_here(
        "outcome-specific ensures requires the exact nominal result-case path `Result::Case -> { guarantees }`",
    ))
        }

        fn parse_outcome_specific_ensures_group<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
            keyword_source_span: Option<source::SourceSpan>,
        ) -> ParseResult<'tokens, 'source, (Vec<CapabilityContract>, String)> {
            let (result_case, rest) = parse_path_handle_span(input, |member| {
                items(syntax_trees).append_identifier_path_member(member)
            })?;
            let group_name = syntax_trees
                .items
                .identifier_path_members(result_case)
                .iter()
                .map(|member| member.as_str(syntax_trees))
                .collect::<Vec<_>>()
                .join("::");
            let mut input = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
            input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
            let mut contracts = Vec::new();

            while !input.at_punctuation(PunctuationKind::RightBrace) {
                let row_input = input;
                let (binding, fact_input) = input.take_optional_label()?;
                let ((facts, token_count), rest) =
                    crate::grammar::contracts::facts::parse_proof_facts_until(
                        syntax_trees,
                        fact_input,
                        |input| {
                            input.at_punctuation(PunctuationKind::Semicolon)
                                || input.at_punctuation(PunctuationKind::RightBrace)
                                || input.tokens.is_empty()
                        },
                    )?;
                if facts.count() != 1 {
                    return Err(row_input.error_here(
                        "each outcome-specific ensures row must contain exactly one guarantee",
                    ));
                }
                contracts.push(CapabilityContract {
                    kind: CapabilityContractKind::EnsuresForResultCase { result_case },
                    keyword_source_span,
                    binding,
                    facts,
                    token_count,
                });
                input = rest;
                if input.at_punctuation(PunctuationKind::Semicolon) {
                    input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
                } else if !input.at_punctuation(PunctuationKind::RightBrace) {
                    return Err(
                        input.error_here("expected `;` or `}` after outcome-specific ensures row")
                    );
                }
            }
            input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
            if contracts.is_empty() {
                return Err(input.error_here(
                    "an outcome-specific ensures group must contain at least one guarantee",
                ));
            }
            Ok(((contracts, group_name), input))
        }

        fn machine_reaches_clause_boundary(input: Input<'_, '_>) -> bool {
            input.at_punctuation(PunctuationKind::LeftBrace)
                || input.at_punctuation(PunctuationKind::Semicolon)
                || input.at_contextual("requires")
                || input.at_contextual("ensures")
                || input.at_contract_continuation_keyword()
                || input.at_contextual("satisfies")
                || input.at_same_stack_demand()
        }

        fn parse_crash_header<'tokens, 'source>(
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, (Option<CrashCause>, usize)> {
            let input = input.take_contextual("crashes")?;
            // `crashes none` carries no cause; the caller finishes the clause.
            if input.at_contextual("none") {
                return Ok(((None, 2), input.take_contextual("none")?));
            }
            let (cause, after_cause) = input.take_identifier()?;
            let cause = crate::grammar::contracts::parse_crash_cause(&cause, after_cause)?;
            Ok(((Some(cause), 2), after_cause))
        }

        fn parse_ranked_subjects<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, RankedSubjects> {
            let (subjects, mut rest) = if input.at_punctuation(PunctuationKind::LeftParen) {
                let mut tuple_input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
                let mut subjects = Vec::new();
                loop {
                    let (subject, after_subject) =
                        parse_expression_handle_without_struct_literals(syntax_trees, tuple_input)?;
                    subjects.push(subject);
                    if after_subject.at_punctuation(PunctuationKind::Comma) {
                        tuple_input =
                            after_subject.take_punctuation(PunctuationKind::Comma, ",")?;
                        continue;
                    }
                    tuple_input =
                        after_subject.take_punctuation(PunctuationKind::RightParen, ")")?;
                    break;
                }
                (subjects, tuple_input)
            } else {
                let (expression, rest) =
                    parse_expression_handle_without_struct_literals_or_membership(
                        syntax_trees,
                        input,
                    )?;
                (vec![expression], rest)
            };
            let ranking_subjects = exprs(syntax_trees).insert_expression_handles(subjects);
            let mut ranking_view = HandleSpan::empty();
            let mut ranking_view_arguments = HandleSpan::empty();

            if rest.at_punctuation(PunctuationKind::Arrow) {
                rest = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
                let (order, next) = parse_path_handle_span(rest, |member| {
                    items(syntax_trees).append_identifier_path_member(member)
                })?;
                ranking_view = order;
                rest = next;

                if rest.at_punctuation(PunctuationKind::LeftParen) {
                    let mut argument_input =
                        rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
                    let mut arguments = Vec::new();
                    loop {
                        let (argument, after_argument) =
                            parse_expression_handle_without_struct_literals(
                                syntax_trees,
                                argument_input,
                            )?;
                        arguments.push(argument);
                        if after_argument.at_punctuation(PunctuationKind::Comma) {
                            argument_input =
                                after_argument.take_punctuation(PunctuationKind::Comma, ",")?;
                            continue;
                        }
                        argument_input =
                            after_argument.take_punctuation(PunctuationKind::RightParen, ")")?;
                        break;
                    }
                    ranking_view_arguments = syntax_trees
                        .expressions
                        .insert_expression_handles(arguments);
                    rest = argument_input;
                }
            }

            let mut ranking_range = syntax_trees::expression::ExpressionHandle::invalid();
            if rest.at_contextual("in") {
                let range_input = rest.take_contextual("in")?;
                let (start, after_start) =
                    parse_expression_handle_without_struct_literals_or_membership(
                        syntax_trees,
                        range_input,
                    )?;
                let end_inclusive = if after_start.at_punctuation(PunctuationKind::DotDotEqual) {
                    true
                } else if after_start.at_punctuation(PunctuationKind::DotDot) {
                    false
                } else {
                    return Err(after_start.expected_one_of_here(&["`..`", "`..=`"]));
                };
                let after_separator = if end_inclusive {
                    after_start.take_punctuation(PunctuationKind::DotDotEqual, "..=")?
                } else {
                    after_start.take_punctuation(PunctuationKind::DotDot, "..")?
                };
                let (end, next) = parse_expression_handle_without_struct_literals_or_membership(
                    syntax_trees,
                    after_separator,
                )?;
                ranking_range =
                    exprs(syntax_trees).insert(ExpressionNode::Range(TableRangeExpression {
                        start,
                        end,
                        end_inclusive,
                    }));
                rest = next;
            }

            Ok((
                (
                    ranking_subjects,
                    ranking_view,
                    ranking_view_arguments,
                    ranking_range,
                ),
                rest,
            ))
        }
    }
    pub(crate) mod signature {
        use crate::grammar::contracts::facts::parse_proof_facts_until;
        use crate::grammar::parameters::contracts::parse_machine_parameter_contracts;
        use crate::tokens::token_cursor::Input;
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::item::{CapabilityContract, CapabilityContractKind};
        use tokens::{KeywordKind, PunctuationKind};

        fn signature_clause_boundary(input: Input<'_, '_>) -> bool {
            input.at_punctuation(PunctuationKind::Semicolon)
                || input.at_punctuation(PunctuationKind::LeftBrace)
                || input.at_contextual("requires")
                || input.at_contextual("ensures")
                || ((input.at_contextual("terminates")
                    || input.at_contextual("reaches")
                    || input.at_contextual("effects")
                    || input.at_contextual("invokes")
                    || input.at_contextual("suspends")
                    || input.at_contextual("blocks")
                    || input.at_contextual("crashes"))
                    && !input.clause_word_is_an_operand())
                || input.tokens.is_empty()
        }

        pub(crate) struct SignatureClauses {
            pub service_reaches: HandleSpan<syntax_trees::identifier::Identifier>,
            pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
            pub service_reach_is_installation_bound: bool,
            pub invokes: HandleSpan<syntax_trees::identifier::Identifier>,
            pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
            pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
            pub suspends: bool,
            pub blocks: bool,
            pub contracts: HandleSpan<CapabilityContract>,
            pub terminates_guarantee: bool,
            /// The signature wrote `crashes none` (see `MachineClauses`).
            pub declares_no_crash_route: bool,
        }

        pub(crate) fn parse_signature_clauses<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            mut input: Input<'tokens, 'source>,
            type_parameters: HandleSpan<syntax_trees::item::TypeParameter>,
        ) -> Result<
            (SignatureClauses, Input<'tokens, 'source>),
            crate::diagnostics::parse_error::ParseError,
        > {
            let mut service_reaches = Vec::new();
            let mut service_reach_keyword_source_spans = Vec::new();
            let mut service_reach_is_installation_bound = false;
            let mut invokes = Vec::new();
            let mut suspends_keyword_source_spans = Vec::new();
            let mut blocks_keyword_source_spans = Vec::new();
            let mut suspends = false;
            let mut blocks = false;
            let mut contracts = Vec::new();
            let mut crashes_none_declared = false;
            let mut terminates_guarantee = false;

            while !input.at_punctuation(PunctuationKind::Semicolon)
                && !input.at_punctuation(PunctuationKind::LeftBrace)
            {
                if input.at_contextual("requires")
                    && input
                        .take_contextual("requires")?
                        .at_keyword(KeywordKind::Machine)
                {
                    let ((), rest) =
                        parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
                    input = rest;
                    continue;
                }
                if input.at_contextual("requires")
                    && input
                        .restarted(input.tokens.get(1..).unwrap_or_default())
                        .at_contextual("proposition")
                {
                    break;
                }
                if input.at_punctuation(PunctuationKind::RightBrace) {
                    return Err(input.expected_one_of_here(&["`;`", "`{`"]));
                }

                if input.at_contextual("effects") {
                    return Err(input.error_here(
                        "the `effects` reach clause is retired; write `reaches <Service> + ...`",
                    ));
                }

                if input.at_contextual("reaches") {
                    input = super::take_operational_reaches_clause(
                        input,
                        &mut service_reach_keyword_source_spans,
                        &mut service_reaches,
                        &mut service_reach_is_installation_bound,
                        signature_clause_boundary,
                    )?;
                    continue;
                }

                if input.at_contextual("invokes") {
                    input = input.take_contextual("invokes")?;
                    let (binding, after_binding) = input.take_identifier()?;
                    invokes.push(binding);
                    input = after_binding;
                    continue;
                }

                if input.at_contextual("suspends") {
                    suspends_keyword_source_spans.push(input.current_source_span());
                    if suspends {
                        return Err(input.error_here("duplicate `suspends` operational clause"));
                    }
                    suspends = true;
                    input = input.take_contextual("suspends")?;
                    continue;
                }

                if input.at_contextual("blocks") {
                    blocks_keyword_source_spans.push(input.current_source_span());
                    if blocks {
                        return Err(input.error_here("duplicate `blocks` operational clause"));
                    }
                    blocks = true;
                    input = input.take_contextual("blocks")?;
                    continue;
                }

                if input.at_contextual("crashes") {
                    let keyword_source_span = Some(input.current_source_span());
                    let after_keyword = input.take_contextual("crashes")?;
                    if after_keyword.at_contextual("none") {
                        let other_route = contracts.iter().any(|contract: &CapabilityContract| {
                            matches!(contract.kind, CapabilityContractKind::Crashes { .. })
                        });
                        input = crate::grammar::contracts::finish_crashes_none(
                            after_keyword.take_contextual("none")?,
                            other_route,
                            signature_clause_boundary,
                        )?;
                        crashes_none_declared = true;
                        continue;
                    }
                    if crashes_none_declared {
                        return Err(input
                            .error_here("a `crashes Cause` route cannot follow `crashes none`"));
                    }
                    let (cause, after_cause) = after_keyword.take_identifier()?;
                    let cause = crate::grammar::contracts::parse_crash_cause(&cause, after_cause)?;
                    let after_header = after_cause;
                    let header_token_count = 2usize;
                    let ((facts, fact_token_count), rest) = parse_proof_facts_until(
                        syntax_trees,
                        after_header,
                        signature_clause_boundary,
                    )?;
                    contracts.push(CapabilityContract {
                        kind: CapabilityContractKind::Crashes { cause },
                        keyword_source_span,
                        binding: None,
                        facts,
                        token_count: fact_token_count
                            .checked_add(header_token_count)
                            .expect("crash contract token count overflow"),
                    });
                    input = rest;
                    continue;
                }

                if input.at_contextual("requires") || input.at_contextual("ensures") {
                    let keyword_source_span = Some(input.current_source_span());
                    let kind = if input.at_contextual("requires") {
                        input = input.take_contextual("requires")?;
                        CapabilityContractKind::Requires
                    } else {
                        input = input.take_contextual("ensures")?;
                        CapabilityContractKind::Ensures
                    };
                    let (binding, fact_input) = input.take_optional_label()?;
                    let ((facts, token_count), rest) =
                        crate::grammar::contracts::facts::parse_required_proof_facts_until(
                            syntax_trees,
                            fact_input,
                            signature_clause_boundary,
                            false,
                        )?;
                    if binding.is_some() && facts.count() != 1 {
                        return Err(fact_input.error_here(
                    "a named bodyless-signature contract must contain exactly one proposition",
                ));
                    }
                    contracts.push(CapabilityContract {
                        kind,
                        keyword_source_span,
                        binding,
                        facts,
                        token_count,
                    });
                    input = rest;
                    continue;
                }

                if input.at_contextual("terminates") {
                    input = input.take_contextual("terminates")?;
                    if input.at_contextual("by") {
                        return Err(input.error_here(
                            "a ranking witness (`terminates by ...`) does not belong on a \
                     bodyless requirement (decision 23): the requirement authors the \
                     guarantee with bare `terminates`; the implementation supplies \
                     the witness that discharges it",
                        ));
                    }
                    terminates_guarantee = true;
                    continue;
                }

                return Err(input.expected_one_of_here(&[
                    "`requires`",
                    "`ensures`",
                    "`reaches`",
                    "`invokes`",
                    "`suspends`",
                    "`blocks`",
                    "`crashes`",
                    "`terminates`",
                    "`;`",
                    "`{`",
                ]));
            }

            Ok((
                SignatureClauses {
                    service_reaches: syntax_trees
                        .items
                        .insert_identifier_path_members(service_reaches),
                    service_reach_keyword_source_spans,
                    service_reach_is_installation_bound,
                    invokes: syntax_trees.items.insert_identifier_path_members(invokes),
                    suspends_keyword_source_spans,
                    blocks_keyword_source_spans,
                    suspends,
                    blocks,
                    contracts: super::publish_capability_contracts(syntax_trees, contracts),
                    terminates_guarantee,
                    declares_no_crash_route: crashes_none_declared,
                },
                input,
            ))
        }
    }

    use crate::diagnostics::parse_error::ParseError;

    pub(crate) fn parse_crash_cause<'tokens, 'source>(
        cause: &syntax_trees::identifier::Identifier,
        after_cause: crate::tokens::token_cursor::Input<'tokens, 'source>,
    ) -> Result<syntax_trees::item::CrashCause, ParseError> {
        use syntax_trees::item::CrashCause;
        match cause.as_str(&after_cause) {
            "Trap" => Ok(CrashCause::Trap),
            "Abort" => Ok(CrashCause::Abort),
            _ => Err(after_cause.error_here(format!(
                "unknown crash cause `{}`; expected `Trap` or `Abort`",
                cause.as_str(&after_cause)
            ))),
        }
    }

    pub(crate) fn reject_retired_operational_reach(
        service: &syntax_trees::identifier::Identifier,
        input: crate::tokens::token_cursor::Input<'_, '_>,
    ) -> Result<(), ParseError> {
        let replacement = match service.as_str(&input) {
            "Suspend" => "suspends",
            "Block" | "thread_block" => "blocks",
            "sync_wait" => "the appropriate independent `suspends` and/or `blocks` clause",
            _ => return Ok(()),
        };
        Err(input.error_here(format!(
        "`reaches {}` is invalid: `reaches` contains boundary-service identities only; write `{replacement}` as an independent operational clause",
        service.as_str(&input)
    )))
    }
    pub(crate) fn take_service_reach_row_member<'tokens, 'source>(
        input: crate::tokens::token_cursor::Input<'tokens, 'source>,
        service_reaches: &mut Vec<syntax_trees::identifier::Identifier>,
    ) -> Result<crate::tokens::token_cursor::Input<'tokens, 'source>, ParseError> {
        use tokens::PunctuationKind;
        let (service, rest) = input.take_identifier()?;
        reject_retired_operational_reach(&service, rest)?;
        service_reaches.push(service);
        let mut input = rest;
        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else if input.at_punctuation(PunctuationKind::Plus) {
            input = input.take_punctuation(PunctuationKind::Plus, "+")?;
        }
        Ok(input)
    }

    pub(crate) fn take_operational_reaches_clause<'tokens, 'source>(
        input: crate::tokens::token_cursor::Input<'tokens, 'source>,
        service_reach_keyword_source_spans: &mut Vec<source::SourceSpan>,
        service_reaches: &mut Vec<syntax_trees::identifier::Identifier>,
        service_reach_is_installation_bound: &mut bool,
        at_clause_boundary: impl Fn(crate::tokens::token_cursor::Input<'_, '_>) -> bool,
    ) -> Result<crate::tokens::token_cursor::Input<'tokens, 'source>, ParseError> {
        use tokens::PunctuationKind;
        service_reach_keyword_source_spans.push(input.current_source_span());
        let mut input = input.take_contextual("reaches")?;
        let service_count_before_clause = service_reaches.len();
        if input.at_punctuation(PunctuationKind::LessEqual) {
            if *service_reach_is_installation_bound || !service_reaches.is_empty() {
                return Err(input.error_here(
                    "an installation-bound reach row must be declared once as `reaches <= Bound`",
                ));
            }
            *service_reach_is_installation_bound = true;
            input = input.take_punctuation(PunctuationKind::LessEqual, "<=")?;
        } else if *service_reach_is_installation_bound {
            return Err(input.error_here(
            "an installation-bound reach bound cannot be combined with another `reaches` clause",
        ));
        }
        // `reaches none` spells the explicit empty service row. It keeps the
        // keyword span so later stages still distinguish it from omission and
        // contributes no member, exactly like the keyword followed directly by
        // a clause boundary, which stays accepted until the retirement sweep.
        if !*service_reach_is_installation_bound && input.at_contextual("none") {
            let after_none = input.take_contextual("none")?;
            if !at_clause_boundary(after_none) {
                return Err(after_none.error_here(
                    "`reaches none` admits no service members; write `reaches <Service> + ...` to name services",
                ));
            }
            return Ok(after_none);
        }
        while !at_clause_boundary(input) {
            input = take_service_reach_row_member(input, service_reaches)?;
        }
        if *service_reach_is_installation_bound
            && service_reaches.len() == service_count_before_clause
        {
            return Err(input.error_here(
            "an installation-bound reach row requires a nonempty upper bound after `reaches <=`",
        ));
        }
        Ok(input)
    }

    pub(crate) fn publish_capability_contracts(
        syntax_trees: &mut syntax_trees::SyntaxTrees,
        contracts: Vec<syntax_trees::item::CapabilityContract>,
    ) -> arena::HandleSpan<syntax_trees::item::CapabilityContract> {
        let mut span = crate::storage::handle_spans::HandleSpanBuilder::new();
        for contract in contracts {
            span.push(syntax_trees.items.append_capability_contract(contract));
        }
        span.finish()
    }

    /// `crashes none` promises that no crash route exists. It names no cause,
    /// admits no route guard and cannot share a header with a `crashes Cause`
    /// route. The header then publishes no crash contract, which is the empty
    /// cause set an omitted clause denotes on a published surface today; the
    /// explicit spelling exists so that promise is readable without the rule.
    pub(crate) fn finish_crashes_none<'tokens, 'source>(
        after_none: crate::tokens::token_cursor::Input<'tokens, 'source>,
        other_route_present: bool,
        at_clause_boundary: impl Fn(crate::tokens::token_cursor::Input<'_, '_>) -> bool,
    ) -> Result<crate::tokens::token_cursor::Input<'tokens, 'source>, ParseError> {
        if other_route_present {
            return Err(after_none
                .error_here("`crashes none` cannot share a header with a `crashes Cause` route"));
        }
        if !at_clause_boundary(after_none) {
            return Err(after_none.error_here(
                "`crashes none` admits no route guards; name a cause to guard a route",
            ));
        }
        Ok(after_none)
    }
}

pub(crate) mod parameters {
    pub(crate) mod binding_properties {
        use crate::tokens::token_cursor::{Input, ParseResult};
        use tokens::PunctuationKind;

        pub(crate) fn parse_binding_relevance_brackets<'tokens, 'source>(
            input: Input<'tokens, 'source>,
            site: &str,
        ) -> ParseResult<'tokens, 'source, language_core::BindingRelevance> {
            if !input.at_punctuation(PunctuationKind::LeftBracket) {
                return Ok((language_core::BindingRelevance::Relevant, input));
            }

            let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
            let mut relevance = language_core::BindingRelevance::Relevant;
            let mut declared_erased = false;
            while !input.at_punctuation(PunctuationKind::RightBracket) {
                let (name, next) = input.take_identifier()?;
                match name.as_str(&input) {
                    "erased" => {
                        if declared_erased {
                            return Err(next.error_here("duplicate binding property `erased`"));
                        }
                        declared_erased = true;
                        relevance = language_core::BindingRelevance::Erased;
                        input = next;
                    }
                    other => {
                        return Err(next.error_here(format!(
                    "unknown {site} binding property `{other}`; declared binding properties are `erased`"
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
            Ok((relevance, input))
        }
    }
    pub(crate) mod contracts {
        use crate::grammar::contracts::conformance::parse_conformance::parse_optional_satisfies_type_arguments;
        use crate::grammar::contracts::signature::parse_signature_clauses;
        use crate::grammar::parameters::parse_generic_parameters::GenericParameterSyntax;
        use crate::grammar::parameters::parse_generic_parameters::parse_generic_parameters;
        use crate::grammar::parameters::parse_parameters::{
            parse_optional_parameters, parse_optional_return_type,
        };
        use crate::tokens::token_cursor::{Input, ParseResult};
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::item::MachineParameterContract;
        use tokens::{KeywordKind, PunctuationKind};

        pub(crate) fn parse_machine_parameter_contracts<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            type_parameters: HandleSpan<syntax_trees::item::TypeParameter>,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, ()> {
            let mut input = input;
            loop {
                if !input.at_contextual("requires") {
                    break;
                }
                let after_keyword = input.take_contextual("requires")?;
                if !after_keyword.at_keyword(KeywordKind::Machine) {
                    break;
                }
                let after_machine = after_keyword.take_keyword(KeywordKind::Machine, "machine")?;
                let (name, after_name) = after_machine.take_identifier()?;

                if after_name.at_punctuation(PunctuationKind::ColonColon) {
                    return Err(after_name.error_here(
                "one-off `requires machine T::member(...)` requirements are unsupported; declare a trait and bind an explicit conformance",
            ));
                }

                let Some(parameter_index) = syntax_trees
                    .items
                    .type_parameters(type_parameters)
                    .iter()
                    .position(|parameter| {
                        parameter.name.as_str(syntax_trees) == name.as_str(syntax_trees)
                    })
                else {
                    return Err(after_name.error_here(format!(
                        "`requires machine {}` has no matching generic parameter",
                        name.as_str(syntax_trees)
                    )));
                };

                match &syntax_trees.items.type_parameters(type_parameters)[parameter_index].kind {
                    syntax_trees::item::TypeParameterKind::Machine {
                        contract: None | Some(MachineParameterContract::RequirementIdentity),
                    } => {}
                    syntax_trees::item::TypeParameterKind::Type
                        if syntax_trees.items.type_parameters(type_parameters)[parameter_index]
                            .bounds
                            == syntax_trees::item::DataProperties::default() => {}
                    syntax_trees::item::TypeParameterKind::Machine { contract: Some(_) } => {
                        return Err(after_name.error_here(format!(
                            "machine parameter `{}` already has a `requires machine` contract",
                            name.as_str(syntax_trees)
                        )));
                    }
                    _ => {
                        return Err(after_name.error_here(format!(
                    "`{}` is not a machine parameter; its declared kind or property bounds conflict with `requires machine {}`",
                    name.as_str(syntax_trees),
                    name.as_str(syntax_trees)
                )));
                    }
                }

                if after_name.at_contextual("satisfies") {
                    let after_satisfies = after_name.take_contextual("satisfies")?;
                    let (requirement, trait_arguments, after_requirement) =
                        parse_nominal_requirement_path(syntax_trees, after_satisfies)?;
                    if requirement.len() < 2 {
                        return Err(after_requirement.error_here(format!(
                    "nominal machine parameter `{}` must name an exact `Trait::requirement`; bare `satisfies Trait` is not a callable contract",
                    name.as_str(syntax_trees)
                )));
                    }
                    if after_requirement.at_contextual("as") {
                        return Err(after_requirement.error_here(
                    "nominal machine parameter requirements do not accept `as Name`; `Trait::requirement` must resolve uniquely",
                ));
                    }
                    if after_requirement.at_contextual("via") {
                        return Err(after_requirement.error_here(
                    "a `requires machine` nominal requirement cannot use `via`; native bindings belong to the owning Build",
                ));
                    }
                    let rest = take_machine_contract_terminator(after_requirement)?;
                    let parameter = &mut syntax_trees.items.type_parameters_mut(type_parameters)
                        [parameter_index];
                    parameter.kind = syntax_trees::item::TypeParameterKind::Machine {
                        contract: Some(MachineParameterContract::Nominal {
                            requirement,
                            trait_arguments,
                        }),
                    };
                    input = rest;
                    continue;
                }

                let (nested_generic_parameters, after_type_parameters) = parse_generic_parameters(
                    syntax_trees,
                    after_name,
                    GenericParameterSyntax::StaticBinders,
                )?;
                let nested_type_parameters = nested_generic_parameters.type_parameters;
                let (parameters, after_parameters) =
                    parse_optional_parameters(syntax_trees, after_type_parameters)?;
                let (return_type, after_return) =
                    parse_optional_return_type(syntax_trees, after_parameters)?;
                let ((), after_nested_contracts) = parse_machine_parameter_contracts(
                    syntax_trees,
                    nested_type_parameters,
                    after_return,
                )?;
                let (clauses, rest) = parse_signature_clauses(
                    syntax_trees,
                    after_nested_contracts,
                    nested_type_parameters,
                )?;
                validate_machine_parameter_contracts(syntax_trees, nested_type_parameters, rest)?;
                if clauses.service_reach_is_installation_bound {
                    return Err(rest.error_here(
                "`reaches <= Bound` is installation-selected and cannot appear on a structural machine parameter",
            ));
                }
                let rest = take_machine_contract_terminator(rest)?;

                let contract = syntax_trees::item::StateSignature {
                    name: name.clone(),
                    lifetime_parameters: nested_generic_parameters.lifetime_parameters,
                    type_parameters: nested_type_parameters,
                    parameters,
                    native_callback_parameters: Vec::new(),
                    return_type,
                    service_reach_keyword_source_spans: clauses.service_reach_keyword_source_spans,
                    service_reaches: clauses.service_reaches,
                    invokes: clauses.invokes,
                    suspends_keyword_source_spans: clauses.suspends_keyword_source_spans,
                    blocks_keyword_source_spans: clauses.blocks_keyword_source_spans,
                    suspends: clauses.suspends,
                    blocks: clauses.blocks,
                    contracts: clauses.contracts,
                    terminates_guarantee: clauses.terminates_guarantee,
                    declares_no_crash_route: clauses.declares_no_crash_route,
                    ..crate::grammar::parameters::parse_parameters::blank_state_signature()
                };
                let parameter =
                    &mut syntax_trees.items.type_parameters_mut(type_parameters)[parameter_index];
                parameter.kind = syntax_trees::item::TypeParameterKind::Machine {
                    contract: Some(MachineParameterContract::Structural(contract)),
                };
                input = rest;
            }

            Ok(((), input))
        }

        pub(crate) fn validate_machine_parameter_contracts(
            syntax_trees: &SyntaxTrees,
            type_parameters: HandleSpan<syntax_trees::item::TypeParameter>,
            input: Input<'_, '_>,
        ) -> Result<(), crate::diagnostics::parse_error::ParseError> {
            if let Some(missing) = syntax_trees
                .items
                .type_parameters(type_parameters)
                .iter()
                .find(|parameter| {
                    matches!(
                        parameter.kind,
                        syntax_trees::item::TypeParameterKind::Machine { contract: None }
                    )
                })
            {
                return Err(input.error_here(format!(
            "machine parameter `{}` requires an authored declaration-site contract: write `requires machine {}(...) -> Result;` or `requires machine {} satisfies Trait::requirement;`",
            missing.name.as_str(syntax_trees),
            missing.name.as_str(syntax_trees),
            missing.name.as_str(syntax_trees),
        )));
            }

            Ok(())
        }

        fn take_machine_contract_terminator<'tokens, 'source>(
            input: Input<'tokens, 'source>,
        ) -> Result<Input<'tokens, 'source>, crate::diagnostics::parse_error::ParseError> {
            input.take_punctuation(PunctuationKind::Semicolon, ";")
        }

        fn parse_nominal_requirement_path<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> Result<
            (
                HandleSpan<syntax_trees::identifier::Identifier>,
                HandleSpan<syntax_trees::types::TypeReferenceHandle>,
                Input<'tokens, 'source>,
            ),
            crate::diagnostics::parse_error::ParseError,
        > {
            let (first, mut rest) = input.take_identifier()?;
            let mut members = vec![first];
            let mut trait_arguments = HandleSpan::empty();
            loop {
                if rest.at_punctuation(PunctuationKind::Less) {
                    if !trait_arguments.is_empty() {
                        return Err(rest.error_here(
                            "a nominal machine parameter requirement applies its trait once",
                        ));
                    }
                    let (arguments, next) =
                        parse_optional_satisfies_type_arguments(syntax_trees, rest)?;
                    trait_arguments = arguments;
                    rest = next;
                    if !rest.at_punctuation(PunctuationKind::ColonColon) {
                        return Err(rest.error_here(
                    "a trait application in a nominal machine parameter requirement must be followed by `::requirement`",
                ));
                    }
                }
                if !rest.at_punctuation(PunctuationKind::ColonColon) {
                    break;
                }
                rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
                let (member, next) = rest.take_identifier()?;
                members.push(member);
                rest = next;
                if !trait_arguments.is_empty() {
                    break;
                }
            }
            let mut path = crate::storage::handle_spans::HandleSpanBuilder::new();
            for member in members {
                path.push(syntax_trees.items.append_identifier_path_member(member));
            }
            Ok((path.finish(), trait_arguments, rest))
        }
    }
    #[cfg(test)]
    pub(crate) mod generic_parameters_tests {
        use super::parse_generic_parameters::{
            GenericParameterSyntax, ParsedGenericParameters, parse_generic_parameters,
        };
        use crate::ParseError;
        use crate::tokens::token_cursor::Input;
        use source::SourceId;
        use source_files_to_tokens::Lexer;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::item::{Item, MachineParameterContract, TypeParameterKind};

        const SYNTAX_CHOICES: [GenericParameterSyntax; 6] = [
            GenericParameterSyntax::TypeAndConst,
            GenericParameterSyntax::StaticBinders,
            GenericParameterSyntax::DataDeclaration,
            GenericParameterSyntax::TraitRequirements,
            GenericParameterSyntax::RequirementSignature,
            GenericParameterSyntax::MachineDeclaration,
        ];

        fn parse_parameters(
            source: &str,
            syntax: GenericParameterSyntax,
        ) -> Result<(SyntaxTrees, ParsedGenericParameters), ParseError> {
            let tokens = Lexer::new(source).tokenize().unwrap();
            let source_id = SourceId(7);
            let mut trees = SyntaxTrees::new(source_id);
            let retained_source = Input::retain_source(&tokens);
            let input = Input::new(source_id, &tokens, &retained_source, &mut trees);
            let (parameters, rest) = parse_generic_parameters(&mut trees, input, syntax)?;
            assert!(rest.tokens.is_empty());
            Ok((trees, parameters))
        }

        #[test]
        fn proposition_binders_remain_trait_only() {
            for syntax in SYNTAX_CHOICES {
                let result = parse_parameters("<proposition Relation>", syntax);
                assert_eq!(
                    result.is_ok(),
                    matches!(syntax, GenericParameterSyntax::TraitRequirements)
                );
            }
        }

        #[test]
        fn conformance_and_value_binders_keep_distinct_admission() {
            for syntax in SYNTAX_CHOICES {
                let conformance = parse_parameters("<Evidence satisfies Trait<Value>>", syntax);
                assert_eq!(
                    conformance.is_ok(),
                    matches!(
                        syntax,
                        GenericParameterSyntax::TraitRequirements
                            | GenericParameterSyntax::RequirementSignature
                            | GenericParameterSyntax::MachineDeclaration
                            | GenericParameterSyntax::DataDeclaration
                    )
                );
                if let Ok((trees, parameters)) = conformance {
                    assert_eq!(parameters.conformance_bounds.len(), 1);
                    if matches!(
                        syntax,
                        GenericParameterSyntax::TraitRequirements
                            | GenericParameterSyntax::RequirementSignature
                            | GenericParameterSyntax::DataDeclaration
                    ) {
                        assert!(matches!(
                            trees.items.type_parameters(parameters.type_parameters)[0].kind,
                            TypeParameterKind::Implementation
                        ));
                    } else {
                        assert!(parameters.type_parameters.is_empty());
                    }
                }
                assert!(parse_parameters("<Evidence: Value satisfies Trait>", syntax).is_err());
                let value = parse_parameters("<Count: usize>", syntax);
                assert_eq!(
                    value.is_ok(),
                    matches!(
                        syntax,
                        GenericParameterSyntax::DataDeclaration
                            | GenericParameterSyntax::RequirementSignature
                            | GenericParameterSyntax::MachineDeclaration
                    )
                );
                if let Ok((trees, parameters)) = value {
                    assert!(matches!(
                        trees.items.type_parameters(parameters.type_parameters)[0].kind,
                        TypeParameterKind::Value { .. }
                    ));
                }
            }
        }

        #[test]
        fn callable_binders_distinguish_identity_defaults_from_authored_contracts() {
            for syntax in [
                GenericParameterSyntax::DataDeclaration,
                GenericParameterSyntax::TraitRequirements,
            ] {
                let (trees, parameters) = parse_parameters("<machine Selected>", syntax).unwrap();
                let TypeParameterKind::Machine { contract } =
                    &trees.items.type_parameters(parameters.type_parameters)[0].kind
                else {
                    panic!("machine binder");
                };
                assert_eq!(
                    matches!(
                        contract,
                        Some(MachineParameterContract::RequirementIdentity)
                    ),
                    matches!(syntax, GenericParameterSyntax::TraitRequirements)
                );
                if matches!(syntax, GenericParameterSyntax::DataDeclaration) {
                    assert!(contract.is_none());
                }
            }
            for (declaration, ending) in [
                ("data", "{ value: u64; }"),
                ("data", "= u64;"),
                ("trait", "{}"),
            ] {
                for (clause, nominal) in [
                    ("requires machine Selected(value: u64) -> u64;", false),
                    ("requires machine Selected satisfies Hook::step;", true),
                ] {
                    let source =
                        format!("{declaration} Carrier<machine Selected> {clause} {ending}");
                    let tokens = Lexer::new(&source).tokenize().unwrap();
                    let trees =
                        crate::parse_syntax_trees(&tokens).expect("authored callable contract");
                    let parameters = match trees.root_items().next().unwrap() {
                        Item::Data(data) => data.type_parameters,
                        Item::TypeAlias(alias) => alias.type_parameters,
                        Item::Trait(trait_definition) => trait_definition.type_parameters,
                        _ => panic!("data, transparent alias or trait"),
                    };
                    let TypeParameterKind::Machine {
                        contract: Some(contract),
                    } = &trees.items.type_parameters(parameters)[0].kind
                    else {
                        panic!("retained contract");
                    };
                    assert_eq!(
                        matches!(contract, MachineParameterContract::Nominal { .. }),
                        nominal
                    );
                    assert_eq!(
                        matches!(contract, MachineParameterContract::Structural(_)),
                        !nominal
                    );
                }
                for clauses in [
                    "",
                    "requires machine Selected(value: u64) -> u64; requires machine Selected(value: u64) -> u64;",
                ] {
                    if declaration == "trait" && clauses.is_empty() {
                        continue;
                    }
                    let source =
                        format!("{declaration} Carrier<machine Selected> {clauses} {ending}");
                    let tokens = Lexer::new(&source).tokenize().unwrap();
                    assert!(
                        crate::parse_syntax_trees(&tokens).is_err(),
                        "missing or duplicated contracts must refuse"
                    );
                }
            }
        }

        #[test]
        fn evidence_bounds_retain_explicit_trait_lifetimes_separately_from_static_slots() {
            for syntax in [
                GenericParameterSyntax::MachineDeclaration,
                GenericParameterSyntax::DataDeclaration,
                GenericParameterSyntax::TraitRequirements,
            ] {
                let (trees, parameters) = parse_parameters(
                    "<'held, Value, Selected satisfies Buffered<'held, Value, 3u64>>",
                    syntax,
                )
                .unwrap();
                assert_eq!(parameters.lifetime_parameters.len(), 1);
                let [bound] = parameters.conformance_bounds.as_slice() else {
                    panic!("one bound");
                };
                assert_eq!(bound.trait_name.as_str(&trees), "Buffered");
                assert_eq!(bound.lifetime_arguments.len(), 1);
                assert_eq!(bound.lifetime_arguments[0].as_str(&trees), "held");
                assert_eq!(
                    trees
                        .type_references
                        .type_reference_handles(bound.arguments)
                        .len(),
                    2
                );
                assert!(
                    parse_parameters("<'held, Selected satisfies Buffered<Value, 'held>>", syntax,)
                        .is_err()
                );
            }
        }
    }
    pub(crate) mod parse_generic_parameters {
        use crate::grammar::contracts::conformance::parse_conformance::parse_conformance_trait_application;
        use crate::grammar::type_syntax::parse_type::parse_type_reference_handle;
        use crate::grammar::type_syntax::properties::parse_property_brackets;
        use crate::tokens::token_cursor::{Input, ParseResult};
        use arena::HandleSpan;
        use syntax_trees::SyntaxTrees;
        use syntax_trees::identifier::Identifier;
        use syntax_trees::item::{
            DataProperties, MachineParameterContract, TypeParameter, TypeParameterKind,
        };
        use tokens::PunctuationKind;

        #[derive(Default)]
        pub(crate) struct ParsedGenericParameters {
            pub(crate) lifetime_parameters: Vec<Identifier>,
            pub(crate) type_parameters: HandleSpan<TypeParameter>,
            pub(crate) conformance_bounds: Vec<syntax_trees::item::GenericConformanceBound>,
        }

        #[derive(Clone, Copy)]
        pub(crate) enum GenericParameterSyntax {
            TypeAndConst,
            StaticBinders,
            TraitRequirements,
            RequirementSignature,
            MachineDeclaration,
            DataDeclaration,
            MathematicalDefinition,
        }

        fn close_generic_parameter_list<'tokens, 'source>(
            input: Input<'tokens, 'source>,
            lifetime_parameters: Vec<Identifier>,
            type_parameters: crate::storage::handle_spans::HandleSpanBuilder<TypeParameter>,
            conformance_bounds: Vec<syntax_trees::item::GenericConformanceBound>,
        ) -> ParseResult<'tokens, 'source, ParsedGenericParameters> {
            let input = input.take_punctuation(PunctuationKind::Greater, ">")?;
            Ok((
                ParsedGenericParameters {
                    lifetime_parameters,
                    type_parameters: type_parameters.finish(),
                    conformance_bounds,
                },
                input,
            ))
        }

        pub(crate) fn parse_generic_parameters<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            mut input: Input<'tokens, 'source>,
            syntax: GenericParameterSyntax,
        ) -> ParseResult<'tokens, 'source, ParsedGenericParameters> {
            let allow_machine_parameters = !matches!(
                syntax,
                GenericParameterSyntax::TypeAndConst
                    | GenericParameterSyntax::MathematicalDefinition
            );
            let allow_proposition_parameters =
                matches!(syntax, GenericParameterSyntax::TraitRequirements);
            let allow_conformance_binders = matches!(
                syntax,
                GenericParameterSyntax::TraitRequirements
                    | GenericParameterSyntax::RequirementSignature
                    | GenericParameterSyntax::MachineDeclaration
                    | GenericParameterSyntax::DataDeclaration
            );
            let trait_requirement_parameters = matches!(
                syntax,
                GenericParameterSyntax::TraitRequirements
                    | GenericParameterSyntax::RequirementSignature
                    | GenericParameterSyntax::DataDeclaration
            );
            let allow_value_parameters = matches!(
                syntax,
                GenericParameterSyntax::RequirementSignature
                    | GenericParameterSyntax::MachineDeclaration
                    | GenericParameterSyntax::MathematicalDefinition
                    | GenericParameterSyntax::DataDeclaration
            );
            if !input.at_punctuation(PunctuationKind::Less) {
                return Ok((ParsedGenericParameters::default(), input));
            }

            input = input.take_punctuation(PunctuationKind::Less, "<")?;
            let mut type_parameters = crate::storage::handle_spans::HandleSpanBuilder::new();
            let mut lifetime_parameters = Vec::new();
            let mut conformance_bounds = Vec::new();
            let mut declared_names = Vec::<String>::new();
            let mut saw_runtime_parameter = false;

            loop {
                if input.at_punctuation(PunctuationKind::LeftBracket) {
                    return Err(input.error_here(
                "property brackets attach to the name they follow: write the bounds after the type parameter, like `T [copy]`",
            ));
                }

                if input.at_punctuation(PunctuationKind::Apostrophe) {
                    if saw_runtime_parameter {
                        return Err(input.error_here(
                            "lifetime parameters precede type, const, and machine parameters",
                        ));
                    }
                    let after_tick = input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
                    let (lifetime_name, next) = after_tick.take_identifier()?;
                    if declared_names
                        .iter()
                        .any(|declared| declared == lifetime_name.as_str(syntax_trees))
                    {
                        return Err(next.error_here(format!(
                            "duplicate generic parameter `{}`",
                            lifetime_name.as_str(syntax_trees)
                        )));
                    }
                    declared_names.push(lifetime_name.as_str(syntax_trees).to_owned());
                    lifetime_parameters.push(lifetime_name);
                    input = next;

                    if input.at_punctuation(PunctuationKind::Comma) {
                        input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                        continue;
                    }
                    break;
                }

                let (name, mut kind, next) = if input.at_contextual("const") {
                    let input = input.take_contextual("const")?;
                    let (name, input) = input.take_identifier()?;
                    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
                    let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
                    (name, TypeParameterKind::Const { type_reference }, input)
                } else if input.at_keyword(tokens::KeywordKind::Machine) {
                    if !allow_machine_parameters {
                        return Err(input.error_here(
                    "`<machine M>` is a static machine parameter and is only legal on a machine or conformance declaration",
                ));
                    }
                    let input = input.take_keyword(tokens::KeywordKind::Machine, "machine")?;
                    let (name, input) = input.take_identifier()?;
                    let contract = matches!(syntax, GenericParameterSyntax::TraitRequirements)
                        .then_some(MachineParameterContract::RequirementIdentity);
                    (name, TypeParameterKind::Machine { contract }, input)
                } else if input.at_contextual("proposition") {
                    if !allow_proposition_parameters {
                        return Err(input.error_here(
                    "`<proposition Relation>` is currently legal only on a trait declaration",
                ));
                    }
                    let input = input.take_contextual("proposition")?;
                    let (name, input) = input.take_identifier()?;
                    (
                        name,
                        TypeParameterKind::Proposition { contract: None },
                        input,
                    )
                } else {
                    let (name, input) = input.take_identifier()?;
                    (name, TypeParameterKind::Type, input)
                };
                saw_runtime_parameter = true;
                input = next;
                if declared_names
                    .iter()
                    .any(|declared| declared == name.as_str(syntax_trees))
                {
                    return Err(input.error_here(format!(
                        "duplicate generic parameter `{}`",
                        name.as_str(syntax_trees)
                    )));
                }
                declared_names.push(name.as_str(syntax_trees).to_owned());

                if matches!(kind, TypeParameterKind::Type) && input.at_contextual("satisfies") {
                    if !allow_conformance_binders {
                        return Err(input.error_here(format!(
                    "an evidence binder `{} satisfies Trait<...>` is not admitted in this parameter list",
                    name.as_str(syntax_trees)
                )));
                    }
                    let rest = input.take_contextual("satisfies")?;
                    let ((trait_name, lifetime_arguments, arguments), rest) =
                        parse_conformance_trait_application(syntax_trees, rest)?;
                    conformance_bounds.push(syntax_trees::item::GenericConformanceBound {
                        binder: name.clone(),
                        trait_name,
                        lifetime_arguments,
                        arguments,
                    });
                    if trait_requirement_parameters {
                        let handle = syntax_trees.items.append_type_parameter(TypeParameter {
                            name,
                            kind: TypeParameterKind::Implementation,
                            bounds: DataProperties::default(),
                        });
                        type_parameters.push(handle);
                    }
                    input = rest;

                    if input.at_punctuation(PunctuationKind::Comma) {
                        input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                        continue;
                    }
                    break;
                }
                if matches!(kind, TypeParameterKind::Type)
                    && input.at_punctuation(PunctuationKind::Colon)
                    && input
                        .take_punctuation(PunctuationKind::Colon, ":")
                        .and_then(|rest| rest.take_identifier().map(|(_, rest)| rest))
                        .is_ok_and(|rest| rest.at_contextual("satisfies"))
                {
                    return Err(input.error_here(format!(
                "a conformance has no data subject; write the evidence binder as `{} satisfies Trait<Data>`",
                name.as_str(syntax_trees)
            )));
                }

                if matches!(kind, TypeParameterKind::Type)
                    && allow_value_parameters
                    && input.at_punctuation(PunctuationKind::Colon)
                {
                    let rest = input.take_punctuation(PunctuationKind::Colon, ":")?;
                    let (type_reference, rest) = parse_type_reference_handle(syntax_trees, rest)?;
                    kind = TypeParameterKind::Value { type_reference };
                    input = rest;
                }

                if matches!(kind, TypeParameterKind::Value { .. })
                    && !allow_conformance_binders
                    && input.at_contextual("satisfies")
                {
                    return Err(input.error_here(format!(
                "a `satisfies` conformance binder is not admitted in this parameter list; `{}: <type>` already declares a runtime value binder",
                name.as_str(syntax_trees)
            )));
                }

                if matches!(kind, TypeParameterKind::Type)
                    && input.at_punctuation(PunctuationKind::Colon)
                {
                    let after_colon = input.take_punctuation(PunctuationKind::Colon, ":")?;
                    let parameter = name.as_str(syntax_trees);
                    let bound = after_colon
                        .take_identifier()
                        .map(|(bound, _)| bound.as_str(syntax_trees).to_owned())
                        .unwrap_or_else(|_| "copy".to_owned());
                    return Err(input.error_here(format!(
                "type parameter `{parameter}` takes property bounds in brackets after its name: write `{parameter} [{bound}]`, not `{parameter}: {bound}`"
            )));
                }

                if matches!(kind, TypeParameterKind::Machine { .. })
                    && input.at_punctuation(PunctuationKind::LeftBracket)
                {
                    return Err(input.error_here(
                "a machine parameter takes its callable contract in a mandatory `requires machine M(...) -> Result` clause, not property brackets",
            ));
                }
                if matches!(kind, TypeParameterKind::Proposition { .. })
                    && input.at_punctuation(PunctuationKind::LeftBracket)
                {
                    return Err(input.error_here(
                "a proposition parameter takes its signature in a mandatory `requires proposition Name(...);` clause, not property brackets",
            ));
                }
                let bounds = if input.at_punctuation(PunctuationKind::LeftBracket) {
                    let (bounds, next) = parse_property_brackets(input)?;
                    input = next;
                    bounds
                } else {
                    DataProperties::default()
                };

                let handle =
                    syntax_trees
                        .items
                        .append_type_parameter(TypeParameter { name, kind, bounds });
                type_parameters.push(handle);

                let Some(next) = input.take_comma()? else {
                    break;
                };
                input = next;
            }

            close_generic_parameter_list(
                input,
                lifetime_parameters,
                type_parameters,
                conformance_bounds,
            )
        }
    }
    pub(crate) mod parse_parameters {
        use crate::diagnostics::parse_error::ParseError;
        use crate::grammar::parameters::binding_properties::parse_binding_relevance_brackets;
        use crate::grammar::type_syntax::parse_type::parse_type_reference_handle_allowing_borrow;
        use crate::storage::tables::{items, type_refs_of};
        use crate::tokens::token_cursor::{Input, ParseResult};
        use arena::{Handle, HandleSpan};
        use syntax_trees::SyntaxTrees;
        use syntax_trees::item::{StateParameterHandle, StateSignature};
        use syntax_trees::types::TypeReferenceHandle;
        use tokens::{KeywordKind, PunctuationKind};

        pub(crate) fn parse_optional_parameters<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, HandleSpan<StateParameterHandle>> {
            if !input.at_punctuation(PunctuationKind::LeftParen) {
                return Ok((HandleSpan::empty(), input));
            }

            let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let mut parameters = crate::storage::handle_spans::HandleSpanBuilder::new();

            if !input.at_punctuation(PunctuationKind::RightParen) {
                loop {
                    let (parameter, rest) = parse_parameter(syntax_trees, input)?;
                    let handle = items(syntax_trees).append_state_parameter_handle(parameter);
                    parameters.push(handle);
                    input = rest;
                    let Some(next) = input.take_comma()? else {
                        break;
                    };
                    input = next;
                }
            }

            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            let parameters = parameters.finish();
            Ok((parameters, input))
        }

        pub(crate) fn parse_optional_return_type<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
            if !input.at_punctuation(PunctuationKind::Arrow) {
                return Ok((TypeReferenceHandle::invalid(), input));
            }

            let input = input.take_punctuation(PunctuationKind::Arrow, "->")?;
            parse_type_reference_handle_allowing_borrow(syntax_trees, input)
        }

        pub(crate) fn parse_parameter<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, StateParameterHandle> {
            let (is_const, input) = if input.at_contextual("const") {
                (true, input.take_contextual("const")?)
            } else {
                (false, input)
            };
            let (is_leading_mutable, input) = if input.at_contextual("mut") {
                (true, input.take_contextual("mut")?)
            } else {
                (false, input)
            };

            let (receiver_access, input) = if input.at_punctuation(PunctuationKind::Ampersand) {
                let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
                let (access, input) = input.take_reference_access()?;
                (Some(access), input)
            } else {
                (None, input)
            };

            if input.at_keyword(KeywordKind::SelfValue) {
                return Err(input.error_here(
                    "`self` receivers are retired; declare an explicit named parameter and type",
                ));
            }

            let (name, input) = input.take_identifier()?;
            let (relevance, input) = parse_binding_relevance_brackets(input, "parameter")?;
            let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, borrowed_mutable, input) =
                parse_parameter_type_reference(syntax_trees, input)?;
            Ok((
                items(syntax_trees).insert_state_parameter_node(
                    syntax_trees::item::StateParameterNode {
                        name,
                        type_reference,
                        is_const,
                        is_mutable: receiver_access.is_some_and(|access| access.is_exclusive())
                            || is_leading_mutable
                            || borrowed_mutable,
                        relevance,
                    },
                ),
                input,
            ))
        }

        fn parse_parameter_type_reference<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> Result<(TypeReferenceHandle, bool, Input<'tokens, 'source>), ParseError> {
            if !input.at_punctuation(PunctuationKind::Ampersand) {
                let (type_reference, input) =
                    parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
                return Ok((type_reference, false, input));
            }

            let (type_reference, input) =
                parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
            let borrowed_mutable = matches!(
                type_refs_of(syntax_trees).type_reference(type_reference),
                syntax_trees::types::TypeReferenceNode::Reference {
                    access: language_core::ReferenceAccess::Mutable
                        | language_core::ReferenceAccess::WriteOnly,
                    ..
                }
            );
            Ok((type_reference, borrowed_mutable, input))
        }
        pub(crate) fn parse_callable_signature<'tokens, 'source>(
            syntax_trees: &mut SyntaxTrees,
            input: Input<'tokens, 'source>,
        ) -> ParseResult<'tokens, 'source, StateSignature> {
            let (name, input) = input.take_identifier()?;
            let (parameters, input) = parse_optional_parameters(syntax_trees, input)?;
            let (return_type, input) = parse_optional_return_type(syntax_trees, input)?;

            Ok((
                StateSignature {
                    name,
                    parameters,
                    return_type,
                    ..blank_state_signature()
                },
                input,
            ))
        }

        pub(crate) fn blank_state_signature() -> syntax_trees::item::StateSignature {
            syntax_trees::item::StateSignature {
                name: syntax_trees::identifier::Identifier::generated("*"),
                spelling: None,
                lifetime_parameters: Vec::new(),
                type_parameters: HandleSpan::empty(),
                conformance_bounds: Vec::new(),
                is_default: false,
                parameters: HandleSpan::empty(),
                native_callback_parameters: Vec::new(),
                return_type: Handle::invalid(),
                service_reach_is_installation_bound: false,
                service_reach_keyword_source_spans: Vec::new(),
                service_reaches: HandleSpan::empty(),
                invokes: HandleSpan::empty(),
                suspends_keyword_source_spans: Vec::new(),
                blocks_keyword_source_spans: Vec::new(),
                suspends: false,
                blocks: false,
                contracts: HandleSpan::empty(),
                default_body: HandleSpan::empty(),
                terminates_guarantee: false,
                declares_no_crash_route: false,
            }
        }
    }
}
