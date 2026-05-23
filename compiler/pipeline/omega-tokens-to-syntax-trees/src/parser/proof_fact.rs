use crate::parse_error::ParseError;
use crate::parser::expression::parse_expression_handle_without_struct_literals;
use crate::parser::input::{Input, parse_path_handle_span};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{ProofFact, ProofMembershipFact};
use omega_tokens::PunctuationKind;

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

        let (value, rest) = parse_expression_handle_without_struct_literals(syntax_trees, input)?;
        input = rest;

        let fact = if input.at_contextual("in") {
            input = input.take_contextual("in")?;
            let (domain, rest) = parse_path_handle_span(input, |member| {
                syntax_trees.items.append_identifier_path_member(member)
            })?;
            input = rest;
            ProofFact::Membership(ProofMembershipFact { value, domain })
        } else {
            ProofFact::Expression(value)
        };

        let handle = syntax_trees.items.append_proof_fact(fact);
        if fact_count == 0 {
            fact_start = handle;
        }
        fact_count = fact_count
            .checked_add(1)
            .expect("proof fact span count overflow");

        if is_terminator(input) {
            continue;
        } else if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        } else if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
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
