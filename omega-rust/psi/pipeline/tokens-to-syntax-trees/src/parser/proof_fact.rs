use crate::parse_error::ParseError;
use crate::parser::expression::parse_expression_handle_without_struct_literals_or_membership;
use crate::parser::input::{Input, parse_path_handle_span};
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCallExpression,
    TableMembershipExpression,
};
use syntax_trees::item::{ProofFact, ProofMembershipFact};
use tokens::PunctuationKind;

fn copy_item_path_to_expression_path(
    syntax_trees: &mut SyntaxTrees,
    span: HandleSpan<syntax_trees::identifier::Identifier>,
) -> HandleSpan<syntax_trees::identifier::Identifier> {
    let mut start = Handle::invalid();
    let mut count = 0u32;
    let members = syntax_trees
        .tables
        .items
        .identifier_path_members(span)
        .to_vec();

    for member in members {
        let handle = syntax_trees
            .tables
            .expressions
            .append_identifier_path_member(member);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("expression identifier path span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn expand_carry_portable_item_paths(
    syntax_trees: &mut SyntaxTrees,
    domains: Vec<HandleSpan<syntax_trees::identifier::Identifier>>,
) -> Vec<HandleSpan<syntax_trees::identifier::Identifier>> {
    let mut expanded = Vec::new();
    for domain in domains {
        let name = syntax_trees
            .tables
            .items
            .identifier_path_members(domain)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        if name != "Carry::Portable" {
            expanded.push(domain);
            continue;
        }

        for permission in language_core::CarryPermission::ALL {
            let mut parts = permission.name().split("::");
            let namespace = syntax_trees.items.append_identifier_path_member(
                syntax_trees::identifier::Identifier::generated(
                    parts.next().expect("carry permission has a namespace"),
                ),
            );
            let member = syntax_trees.items.append_identifier_path_member(
                syntax_trees::identifier::Identifier::generated(
                    parts.next().expect("carry permission has a member"),
                ),
            );
            debug_assert_eq!(member.arena_index(), namespace.arena_index() + 1);
            expanded.push(HandleSpan::from_parts(namespace, 2));
        }
    }
    expanded
}

pub(super) fn parse_proof_facts_until<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    is_terminator: impl FnMut(Input<'tokens, 'source>) -> bool,
) -> Result<((HandleSpan<ProofFact>, usize), Input<'tokens, 'source>), ParseError> {
    parse_proof_facts_until_with_machine_semicolon(syntax_trees, input, is_terminator, false)
}

/// The machine-contract variant: `machine_final_semicolon` leaves a `;`
/// UNCONSUMED when a non-brace terminator follows it -- the CH10 bodyless
/// `boundary machine .. ensures <fact>;` form, where the semicolon belongs
/// to the MACHINE. Every other fact-list context keeps plain separator
/// semantics.
pub(super) fn parse_proof_facts_until_with_machine_semicolon<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    mut is_terminator: impl FnMut(Input<'tokens, 'source>) -> bool,
    machine_final_semicolon: bool,
) -> Result<((HandleSpan<ProofFact>, usize), Input<'tokens, 'source>), ParseError> {
    let mut input = input;
    let body_start_tokens = input.tokens.len();
    let mut fact_start = Handle::invalid();
    let mut fact_count = 0u32;

    while !is_terminator(input) {
        if input.tokens.is_empty() {
            return Err(input.error_here("expected proof fact terminator"));
        }

        let fact_input = input;
        let mut authored_fact_handles = Vec::new();
        let (value, rest) =
            parse_expression_handle_without_struct_literals_or_membership(syntax_trees, input)?;
        input = rest;

        if input.at_contextual("in") {
            input = input.take_contextual("in")?;
            if input.at_name_like() {
                let (first_domain, rest) = parse_path_handle_span(input, |member| {
                    syntax_trees.items.append_identifier_path_member(member)
                })?;
                input = rest;

                let first_expression_domain =
                    copy_item_path_to_expression_path(syntax_trees, first_domain);
                let first_membership = syntax_trees.expressions.insert(ExpressionNode::Membership(
                    TableMembershipExpression {
                        value,
                        domain: first_expression_domain,
                    },
                ));

                let mut chain_expression = first_membership;
                let mut saw_pipe = false;
                let mut membership_domains = vec![first_domain];

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
                        syntax_trees.items.append_identifier_path_member(member)
                    })?;
                    input = rest;
                    membership_domains.push(domain);
                    let expression_domain = copy_item_path_to_expression_path(syntax_trees, domain);
                    let membership = syntax_trees.expressions.insert(ExpressionNode::Membership(
                        TableMembershipExpression {
                            value,
                            domain: expression_domain,
                        },
                    ));
                    chain_expression = syntax_trees.expressions.insert(ExpressionNode::Binary(
                        TableBinaryExpression {
                            left: chain_expression,
                            operator,
                            right: membership,
                        },
                    ));
                }

                if saw_pipe {
                    let handle = syntax_trees
                        .items
                        .append_proof_fact(ProofFact::Expression(chain_expression));
                    authored_fact_handles.push(handle);
                    if fact_count == 0 {
                        fact_start = handle;
                    }
                    fact_count = fact_count
                        .checked_add(1)
                        .expect("proof fact span count overflow");
                } else {
                    for domain in expand_carry_portable_item_paths(syntax_trees, membership_domains)
                    {
                        let handle = syntax_trees.items.append_proof_fact(ProofFact::Membership(
                            ProofMembershipFact { value, domain },
                        ));
                        authored_fact_handles.push(handle);
                        if fact_count == 0 {
                            fact_start = handle;
                        }
                        fact_count = fact_count
                            .checked_add(1)
                            .expect("proof fact span count overflow");
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
                if fact_count == 0 {
                    fact_start = handle;
                }
                fact_count = fact_count
                    .checked_add(1)
                    .expect("proof fact span count overflow");
            }
        } else {
            let expression = if input.at_name_like() && !fact_input.has_newline_before(input) {
                let (predicate, rest) = input.take_identifier()?;
                input = rest;
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Call(TableCallExpression {
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
            if fact_count == 0 {
                fact_start = handle;
            }
            fact_count = fact_count
                .checked_add(1)
                .expect("proof fact span count overflow");
        }

        let authored_source_span = fact_input.source_span_until(input);
        for handle in authored_fact_handles {
            syntax_trees
                .items
                .set_proof_fact_source_span(handle, authored_source_span);
        }

        if is_terminator(input) {
            continue;
        } else if input.at_punctuation(PunctuationKind::Semicolon) {
            let after = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            // A machine-final `;` (the CH10 bodyless `boundary machine ..
            // ensures <fact>;` form) belongs to the MACHINE, not the fact
            // list: when a HARD ITEM boundary follows -- the next item's
            // keyword or end of input, never a clause keyword (`requires
            // F; ensures ..` continues the signature) and never the body
            // brace (`ensures F; {` keeps its body) -- leave the semicolon
            // unconsumed for parse_machine's bodyless path. Only the
            // machine-contract call site opts in.
            if machine_final_semicolon
                && (starts_machine_contract_following_item(after) || after.tokens.is_empty())
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
    let facts = if fact_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(fact_start, fact_count)
    };

    Ok(((facts, token_count), input))
}

/// A bodyless callable declaration owns its final semicolon even when the
/// following root item carries `pub` and/or `boundary` prefixes. Looking only
/// for a direct `machine`/`data` keyword made consecutive declarations parse as
/// one machine whose clauses continued into the next item.
pub(super) fn starts_machine_contract_following_item(input: Input<'_, '_>) -> bool {
    if input.at_keyword(tokens::KeywordKind::Pub) {
        let after_pub = Input::new(input.source_id, input.tokens.get(1..).unwrap_or_default());
        return starts_machine_contract_following_item(after_pub);
    }

    if input.at_keyword(tokens::KeywordKind::Machine)
        || input.at_keyword(tokens::KeywordKind::Data)
        || input.at_keyword(tokens::KeywordKind::Use)
    {
        return true;
    }

    if !input.at_contextual("boundary") {
        return false;
    }

    let after_boundary = Input::new(input.source_id, input.tokens.get(1..).unwrap_or_default());
    after_boundary.at_keyword(tokens::KeywordKind::Machine)
        || after_boundary.at_keyword(tokens::KeywordKind::Data)
        || after_boundary.at_contextual("requirement")
        || after_boundary.at_contextual("operator")
        || after_boundary.at_contextual("trait")
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
            left: value,
            operator: BinaryOperator::GreaterOrEqual,
            right: start,
        }));
    let upper = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: value,
            operator: if end_inclusive {
                BinaryOperator::LessOrEqual
            } else {
                BinaryOperator::Less
            },
            right: end,
        }));
    syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: lower,
            operator: BinaryOperator::And,
            right: upper,
        }))
}
