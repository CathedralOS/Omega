use crate::parser::expression::parse_expression_handle_without_struct_literals;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{DomainDefinition, DomainFact, DomainMembershipFact};
use omega_syntax_trees::types::TypeReferenceNode;
use omega_tokens::PunctuationKind;

pub(super) fn parse_domain_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DomainDefinition> {
    let (target_name, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
    let (domain_name, input) = input.take_identifier()?;
    let target_type = syntax_trees
        .type_references
        .insert(TypeReferenceNode::Named(target_name.clone()));
    let name = Identifier::generated(format!("{target_name}::{domain_name}"));
    let ((facts, body_token_count), input) = parse_domain_body(syntax_trees, input)?;

    Ok((
        DomainDefinition {
            name,
            target_type,
            facts,
            body_token_count,
        },
        input,
    ))
}

fn parse_domain_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, (HandleSpan<DomainFact>, usize)> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let body_start_tokens = input.tokens.len();
    let mut fact_start = Handle::invalid();
    let mut fact_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (value, rest) = parse_expression_handle_without_struct_literals(syntax_trees, input)?;
        input = rest;

        let fact = if input.at_contextual("in") {
            input = input.take_contextual("in")?;
            let (domain, rest) = parse_path_handle_span(input, |member| {
                syntax_trees.items.append_identifier_path_member(member)
            })?;
            input = rest;
            DomainFact::Membership(DomainMembershipFact { value, domain })
        } else {
            DomainFact::Expression(value)
        };

        let handle = syntax_trees.items.append_domain_fact(fact);
        if fact_count == 0 {
            fact_start = handle;
        }
        fact_count = fact_count
            .checked_add(1)
            .expect("domain fact span count overflow");

        if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        } else if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        } else if !input.at_punctuation(PunctuationKind::RightBrace) {
            return Err(input.error_here("expected `;`, `,`, or `}` after domain fact"));
        }
    }

    let body_token_count = body_start_tokens.saturating_sub(input.tokens.len());
    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let facts = if fact_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(fact_start, fact_count)
    };

    Ok(((facts, body_token_count), input))
}
