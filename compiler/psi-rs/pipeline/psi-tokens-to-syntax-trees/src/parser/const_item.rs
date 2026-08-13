//! `const` item parsing.
//!
//! `const Type::NAME: TypeReference = <expression>;` — the initializer parses
//! as an ordinary expression; the LITERAL-ONLY restriction and the scoped-form
//! requirement are enforced with full context at symbol-resolution lowering
//! in Psi's symbol-resolution stage, not here.

use crate::parser::expression::parse_expression_handle;
use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_type_reference_handle;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::ConstDefinition;
use psi_tokens::PunctuationKind;

pub(super) fn parse_const_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ConstDefinition> {
    let (first, input) = input.take_identifier()?;
    let (scope, name, input) = if input.at_punctuation(PunctuationKind::ColonColon) {
        let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (name, input) = input.take_identifier()?;
        (first, name, input)
    } else {
        // Free-floating form: parsed, rejected later with the real reason.
        (Identifier::generated(""), first, input)
    };
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
    let (value, input) = parse_expression_handle(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((
        ConstDefinition {
            scope,
            name,
            type_reference,
            value,
        },
        input,
    ))
}
