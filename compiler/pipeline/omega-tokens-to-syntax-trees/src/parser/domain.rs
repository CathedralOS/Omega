use crate::parser::input::{Input, ParseResult};
use crate::parser::proof_fact::parse_proof_facts_until;
use omega_core::arena::HandleSpan;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{DomainDefinition, DomainFact};
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
    let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let ((facts, body_token_count), mut input) =
        parse_proof_facts_until(syntax_trees, input, |input| {
            input.at_punctuation(PunctuationKind::RightBrace)
        })?;
    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok(((facts, body_token_count), input))
}
