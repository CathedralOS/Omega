use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_constraint_handles;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::InvariantDefinition;
use psi_tokens::PunctuationKind;

pub(super) fn parse_invariant_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, InvariantDefinition> {
    let (name, input) = input.take_identifier()?;

    if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (constraints, input) = parse_type_constraint_handles(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((InvariantDefinition { name, constraints }, input));
    }

    let (_, input) = input.skip_braced_block()?;
    Ok((
        InvariantDefinition {
            name,
            constraints: psi_arena::HandleSpan::empty(),
        },
        input,
    ))
}
