use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_reference_handle;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::DomainDefinition;

pub(super) fn parse_domain_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DomainDefinition> {
    let (name, input) = input.take_identifier()?;
    let input = input.take_contextual("for")?;
    let (target_type, input) = parse_type_reference_handle(syntax_trees, input)?;
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
