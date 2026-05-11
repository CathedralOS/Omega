use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_constraints;
use omega_syntax_trees::item::InvariantDefinition;
use omega_tokens::PunctuationKind;

pub(super) fn parse_invariant_definition<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, InvariantDefinition> {
    let (name, input) = input.take_identifier()?;

    if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (constraints, input) = parse_type_constraints(input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((InvariantDefinition { name, constraints }, input));
    }

    let (_, input) = input.skip_braced_block()?;
    Ok((
        InvariantDefinition {
            name,
            constraints: Vec::new(),
        },
        input,
    ))
}
