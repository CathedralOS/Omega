use crate::parser::input::{Input, ParseResult};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::DomainDefinition;
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
    let (body_token_count, input) = input.skip_braced_block()?;

    Ok((
        DomainDefinition {
            name,
            target_type,
            body_token_count,
        },
        input,
    ))
}
