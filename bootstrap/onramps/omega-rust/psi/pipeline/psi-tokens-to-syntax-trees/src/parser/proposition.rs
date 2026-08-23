use crate::parser::data::parse_machine_type_parameters;
use crate::parser::expression::parse_expression_handle_without_struct_literals;
use crate::parser::input::{Input, ParseResult};
use crate::parser::state::parse_optional_state_parameters;
use crate::parser::type_reference::parse_type_reference_handle;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::{PropositionBody, PropositionDefinition};
use psi_tokens::PunctuationKind;

pub(super) fn parse_proposition_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, PropositionDefinition> {
    let (name, input) = input.take_identifier()?;
    let (generic_parameters, input) = parse_machine_type_parameters(syntax_trees, input)?;
    if !generic_parameters.lifetime_parameters.is_empty() {
        return Err(input.error_here(
            "proposition binders are proof-static and cannot declare lifetime parameters",
        ));
    }
    let (parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;

    let (body, input) = if input.at_punctuation(PunctuationKind::Semicolon) {
        (
            PropositionBody::Primitive,
            input.take_punctuation(PunctuationKind::Semicolon, ";")?,
        )
    } else if input.at_contextual("evidence") {
        let input = input.take_contextual("evidence")?;
        let (evidence, input) = parse_type_reference_handle(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        (PropositionBody::Witness { evidence }, input)
    } else if input.at_punctuation(PunctuationKind::LeftBrace) {
        return Err(input.error_here(
            "`{ Evidence; }` proposition evidence is retired; write `evidence Evidence;` after the proposition signature",
        ));
    } else if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (proposition, input) =
            parse_expression_handle_without_struct_literals(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        (PropositionBody::Transparent { proposition }, input)
    } else {
        return Err(input.expected_one_of_here(&[
            "`;` for a primitive proposition",
            "`evidence Interface;` for a witness-bearing proposition",
            "`= fact;` for a transparent proposition",
        ]));
    };

    Ok((
        PropositionDefinition {
            name,
            type_parameters: generic_parameters.type_parameters,
            parameters,
            body,
        },
        input,
    ))
}
