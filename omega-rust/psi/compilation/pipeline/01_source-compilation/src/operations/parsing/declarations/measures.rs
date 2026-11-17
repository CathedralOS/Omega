use crate::expressions::parse::parse_expression_handle;
use crate::parameters::callable::{parse_optional_parameters, parse_optional_return_type};
use crate::token_input::cursor::{Input, ParseResult, parse_path_handle_span};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::item::{MeasureDefinition, StateParameterHandle};
use tokens::PunctuationKind;

pub(crate) fn parse_measure_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, MeasureDefinition> {
    let body_start_tokens = input.tokens.len();
    let (name, input) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;

    if input.at_contextual("lexicographic") {
        let input = input.take_contextual("lexicographic")?;
        let (components, input) = parse_lexicographic_components(syntax_trees, input)?;
        let token_count = body_start_tokens.saturating_sub(input.tokens.len());
        return Ok((
            MeasureDefinition {
                name,
                parameter: StateParameterHandle::invalid(),
                return_type: syntax_trees::types::TypeReferenceHandle::invalid(),
                lexicographic: true,
                body: components,
                token_count,
            },
            input,
        ));
    }

    let (parameters, input) = parse_optional_parameters(syntax_trees, input)?;
    let parameter_handles = syntax_trees.items.state_parameters(parameters);
    let parameter = if parameter_handles.len() == 1 {
        parameter_handles[0]
    } else {
        StateParameterHandle::invalid()
    };
    let (return_type, input) = parse_optional_return_type(syntax_trees, input)?;
    let (body, input) = parse_body_block(syntax_trees, input)?;
    let token_count = body_start_tokens.saturating_sub(input.tokens.len());

    Ok((
        MeasureDefinition {
            name,
            parameter,
            return_type,
            lexicographic: false,
            body,
            token_count,
        },
        input,
    ))
}

fn parse_body_block<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<ExpressionHandle>> {
    let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let (expression, input) = parse_expression_handle(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let body = syntax_trees
        .expressions
        .insert_expression_handles([expression]);
    Ok((body, input))
}

fn parse_lexicographic_components<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<ExpressionHandle>> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut components = Vec::new();

    if !input.at_punctuation(PunctuationKind::RightBrace) {
        loop {
            let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
            components.push(expression);
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                if input.at_punctuation(PunctuationKind::RightBrace) {
                    break;
                }
                continue;
            }

            break;
        }
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let body = syntax_trees
        .expressions
        .insert_expression_handles(components);
    Ok((body, input))
}
