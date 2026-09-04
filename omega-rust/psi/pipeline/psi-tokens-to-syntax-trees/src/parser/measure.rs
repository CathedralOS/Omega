use crate::parser::expression::parse_expression_handle;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::state::{parse_optional_return_type, parse_optional_state_parameters};
use psi_arena::HandleSpan;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::ExpressionHandle;
use psi_syntax_trees::item::{MeasureDefinition, StateParameterHandle};
use psi_tokens::PunctuationKind;

/// Parses a `measure` item, in either the simple body form
///   `measure Card::PowerOrder(card: Card) -> usize { card.power }`
/// or the lexicographic tuple form
///   `measure Quest::Difficulty lexicographic { tier, remaining_steps }`.
pub(super) fn parse_measure_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, MeasureDefinition> {
    let body_start_tokens = input.tokens.len();
    let (name, input) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;

    if input.at_contextual("lexicographic") {
        let input = input.take_contextual("lexicographic")?;
        let (components, mut input) = parse_lexicographic_components(syntax_trees, input)?;
        let token_count = body_start_tokens.saturating_sub(input.tokens.len());
        // Trailing semicolons are not required for measures; nothing to consume.
        let _ = &mut input;
        return Ok((
            MeasureDefinition {
                name,
                parameter: StateParameterHandle::invalid(),
                return_type: psi_syntax_trees::types::TypeReferenceHandle::invalid(),
                lexicographic: true,
                body: components,
                token_count,
            },
            input,
        ));
    }

    let (parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let parameter_handles = syntax_trees.items.state_parameters(parameters);
    // A measure is a function of exactly one decreasing value. Any other arity is
    // ill-formed; we leave the parameter unset so the termination checker rejects
    // the order rather than silently using the first parameter.
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

/// Parses `{ expr }` and returns a single-element body span.
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

/// Parses `{ e1, e2, ... }` and returns the ordered component span.
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
