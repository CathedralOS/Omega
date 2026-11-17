use crate::expressions::parse::parse_expression_handle;
use crate::token_input::cursor::{Input, ParseResult};
use crate::types::references::parse_type_reference_handle;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::ConstDefinition;
use tokens::PunctuationKind;

pub(crate) fn parse_const_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ConstDefinition> {
    let (first, input) = input.take_identifier()?;
    let (scope, name, input) = if input.at_punctuation(PunctuationKind::ColonColon) {
        let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (name, input) = input.take_identifier()?;
        (first, name, input)
    } else {
        (Identifier::generated(""), first, input)
    };
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
    let (value, input) = parse_expression_handle(syntax_trees, input.with_constant_position(true))?;
    let input = input.with_constant_position(false);
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((
        ConstDefinition {
            scope,
            name,
            is_public: false,
            type_reference,
            value,
            pending_target_observation: false,
            normalization: None,
            qualification: None,
            pending_qualification: None,
        },
        input,
    ))
}
