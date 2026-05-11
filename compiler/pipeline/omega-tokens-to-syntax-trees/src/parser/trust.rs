use crate::parser::input::{Input, ParseResult};
use omega_syntax_trees::item::TrustDefinition;

pub(super) fn parse_trust_definition<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TrustDefinition> {
    let (name, input) = input.take_identifier()?;
    let (token_count, input) = input.skip_braced_block()?;
    Ok((TrustDefinition { name, token_count }, input))
}
