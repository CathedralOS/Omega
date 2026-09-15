use crate::expressions::parse_expression::parse_expression_handle_without_struct_literals;
use crate::input::token_cursor::{Input, ParseResult};
use crate::parameters::parse_generic_parameters::GenericParameterSyntax;
use crate::parameters::parse_generic_parameters::parse_generic_parameters;
use crate::parameters::parse_parameters::parse_optional_parameters;
use crate::type_syntax::parse_type::parse_type_reference_handle;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{PropositionBody, PropositionDefinition};
use tokens::PunctuationKind;

pub(super) fn parse_proposition_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, PropositionDefinition> {
    let (name, input) = input.take_identifier()?;
    let (generic_parameters, input) =
        parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::StaticBinders)?;
    if !generic_parameters.lifetime_parameters.is_empty() {
        return Err(input.error_here(
            "proposition binders are proof-static and cannot declare lifetime parameters",
        ));
    }
    let (parameters, input) = parse_optional_parameters(syntax_trees, input)?;

    let (body, transparent_formula_source_span, input) = if input
        .at_punctuation(PunctuationKind::Semicolon)
    {
        (
            PropositionBody::Primitive,
            None,
            input.take_punctuation(PunctuationKind::Semicolon, ";")?,
        )
    } else if input.at_contextual("evidence") {
        let input = input.take_contextual("evidence")?;
        let (evidence, input) = parse_type_reference_handle(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        (PropositionBody::Witness { evidence }, None, input)
    } else if input.at_punctuation(PunctuationKind::LeftBrace) {
        return Err(input.error_here(
            "`{ Evidence; }` proposition evidence is retired; write `evidence Evidence;` after the proposition signature",
        ));
    } else if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let formula_start = input;
        let (proposition, formula_end) =
            parse_expression_handle_without_struct_literals(syntax_trees, input)?;
        let formula_source_span = formula_start.source_span_until(formula_end);
        let input = formula_end.take_punctuation(PunctuationKind::Semicolon, ";")?;
        (
            PropositionBody::Transparent { proposition },
            Some(formula_source_span),
            input,
        )
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
            is_public: false,
            type_parameters: generic_parameters.type_parameters,
            parameters,
            transparent_formula_source_span,
            body,
        },
        input,
    ))
}
