use crate::parse_error::ParseError;
use crate::parser::expression::parse_expression_handle_without_struct_literals_or_membership;
use crate::parser::input::{Input, parse_path_handle_span};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    BinaryOperator, ExpressionNode, TableBinaryExpression, TableMembershipExpression,
};
use omega_syntax_trees::item::{ProofFact, ProofMembershipFact};
use omega_tokens::PunctuationKind;

fn copy_item_path_to_expression_path(
    syntax_trees: &mut SyntaxTrees,
    span: HandleSpan<omega_syntax_trees::identifier::Identifier>,
) -> HandleSpan<omega_syntax_trees::identifier::Identifier> {
    let mut start = Handle::invalid();
    let mut count = 0u32;
    let members = syntax_trees
        .tables
        .items
        .identifier_path_members(span)
        .iter()
        .cloned()
        .collect::<Vec<_>>();

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

pub(super) fn parse_proof_facts_until<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    mut is_terminator: impl FnMut(Input<'tokens, 'source>) -> bool,
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
        let (value, rest) =
            parse_expression_handle_without_struct_literals_or_membership(syntax_trees, input)?;
        input = rest;

        if input.at_contextual("in") {
            input = input.take_contextual("in")?;
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
                if fact_count == 0 {
                    fact_start = handle;
                }
                fact_count = fact_count
                    .checked_add(1)
                    .expect("proof fact span count overflow");
            } else {
                for domain in membership_domains {
                    let handle = syntax_trees.items.append_proof_fact(ProofFact::Membership(
                        ProofMembershipFact { value, domain },
                    ));
                    if fact_count == 0 {
                        fact_start = handle;
                    }
                    fact_count = fact_count
                        .checked_add(1)
                        .expect("proof fact span count overflow");
                }
            }
        } else {
            let handle = syntax_trees
                .items
                .append_proof_fact(ProofFact::Expression(value));
            if fact_count == 0 {
                fact_start = handle;
            }
            fact_count = fact_count
                .checked_add(1)
                .expect("proof fact span count overflow");
        }

        if is_terminator(input) {
            continue;
        } else if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
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
