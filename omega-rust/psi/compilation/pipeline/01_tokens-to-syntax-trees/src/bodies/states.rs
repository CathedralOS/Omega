use crate::bodies::sequence::{BodyKind, parse_statements};
use crate::contracts::state_arrival::parse_state_arrival_contracts;
use crate::input::token_cursor::{Input, ParseResult};
use crate::parameters::parse_parameters::{parse_optional_parameters, parse_optional_return_type};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::State;
use tokens::PunctuationKind;

pub(crate) fn parse_state<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, State> {
    let (name, input) = input.take_identifier()?;

    let (parameters, input) = parse_optional_parameters(syntax_trees, input)?;
    let (return_type, input) = parse_optional_return_type(syntax_trees, input)?;
    let (contracts, input) = parse_state_arrival_contracts(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let (statements, input) = parse_statements(syntax_trees, input, BodyKind::ExplicitState)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((
        State {
            name,
            parameters,
            return_type,
            contracts,
            statements,
        },
        input,
    ))
}
