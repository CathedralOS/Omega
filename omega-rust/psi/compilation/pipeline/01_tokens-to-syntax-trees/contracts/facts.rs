use crate::declarations::items::starts_following_item;
use crate::errors::parse_error::ParseError;
use crate::expressions::parse::parse_expression_handle_without_struct_literals_or_membership;
use crate::expressions::parse::parse_logical_proof_fact_expression_handle;
use crate::expressions::parse::parse_proof_fact_expression_handle;
use crate::syntax_storage::tables::{exprs, items};
use crate::token_input::cursor::{Input, parse_path_handle_span};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCallExpression,
    TableMembershipExpression,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{ProofFact, ProofMembershipFact};
use syntax_trees::types::TypeReferenceHandle;
use tokens::PunctuationKind;

fn copy_item_path_to_expression_path(
    syntax_trees: &mut SyntaxTrees,
    span: HandleSpan<Identifier>,
) -> HandleSpan<Identifier> {
    let mut copy = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
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
            let namespace = items(syntax_trees).append_identifier_path_member(
                Identifier::generated(parts.next().expect("carry permission has a namespace")),
            );
            let member = items(syntax_trees).append_identifier_path_member(Identifier::generated(
                parts.next().expect("carry permission has a member"),
            ));
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
    parse_proof_facts_until_with_machine_semicolon(syntax_trees, input, is_terminator, false)
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
    let mut facts = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();

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
                let mut argument_site = (!first_arguments.is_empty()).then_some(argument_start);
                input = rest;

                let first_expression_domain =
                    copy_item_path_to_expression_path(syntax_trees, first_domain);
                let first_membership = exprs(syntax_trees).insert(ExpressionNode::Membership(
                    TableMembershipExpression {
                        value,
                        domain: first_expression_domain,
                    },
                ));

                let mut chain_expression = first_membership;
                let mut saw_pipe = false;
                let mut membership_domains = vec![(first_domain, first_arguments)];

                while input.at_punctuation(PunctuationKind::Ampersand)
                    || input.at_punctuation(PunctuationKind::Pipe)
                {
                    let (operator, rest) = if input.at_punctuation(PunctuationKind::Ampersand) {
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
                    let expression_domain = copy_item_path_to_expression_path(syntax_trees, domain);
                    let membership = exprs(syntax_trees).insert(ExpressionNode::Membership(
                        TableMembershipExpression {
                            value,
                            domain: expression_domain,
                        },
                    ));
                    chain_expression =
                        exprs(syntax_trees).insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: chain_expression,
                            operator,
                            right: membership,
                        }));
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
                        let handle = items(syntax_trees).append_proof_fact(ProofFact::Membership(
                            ProofMembershipFact {
                                value,
                                domain,
                                domain_arguments,
                            },
                        ));
                        authored_fact_handles.push(handle);
                        facts.push(handle);
                    }
                }
            } else {
                let (start, rest) = parse_expression_handle_without_struct_literals_or_membership(
                    syntax_trees,
                    input,
                )?;
                let (end_inclusive, rest) = if rest.at_punctuation(PunctuationKind::DotDotEqual) {
                    (
                        true,
                        rest.take_punctuation(PunctuationKind::DotDotEqual, "..=")?,
                    )
                } else if rest.at_punctuation(PunctuationKind::DotDot) {
                    (false, rest.take_punctuation(PunctuationKind::DotDot, "..")?)
                } else {
                    return Err(rest.error_here("`in` range proof facts require a range separator"));
                };
                let (end, rest) = parse_expression_handle_without_struct_literals_or_membership(
                    syntax_trees,
                    rest,
                )?;
                input = rest;
                let range_fact =
                    range_membership_expression(syntax_trees, value, start, end, end_inclusive);
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
                    callee: ExpressionHandle::invalid(),
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
    let lower = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            policy: numerics::arithmetic::ArithmeticDomain::Exact,
            left: value,
            operator: BinaryOperator::GreaterOrEqual,
            right: start,
        }));
    let upper = syntax_trees
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
) -> Result<(HandleSpan<TypeReferenceHandle>, Input<'tokens, 'source>), ParseError> {
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
    if name == "Carry::Portable" || language_core::CarryPermission::from_name(&name).is_some() {
        return Err(input.error_here("compiler carry permissions do not take index arguments"));
    }
    crate::types::references::parse_domain_argument_handles(syntax_trees, input)
}
