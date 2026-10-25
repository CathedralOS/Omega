use super::sequence::{BodyKind, parse_statements};
use crate::bodies::states::parse_state;
use crate::input::token_cursor::{Input, ParseResult};
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{State, StateHandle, StateParameterHandle};
use syntax_trees::types::TypeReferenceHandle;
use tokens::{KeywordKind, PunctuationKind};

pub(crate) fn parse_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    entry_name: Option<Identifier>,
    machine_parameters: HandleSpan<StateParameterHandle>,
    machine_return_type: TypeReferenceHandle,
) -> ParseResult<'tokens, 'source, (HandleSpan<StateHandle>, bool)> {
    if input.at_punctuation(PunctuationKind::Semicolon) {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        let entry_state = State {
            name: entry_name.unwrap_or_else(|| Identifier::generated("entry")),
            parameters: machine_parameters,
            return_type: machine_return_type,
            contracts: HandleSpan::empty(),
            statements: HandleSpan::empty(),
        };
        let state = syntax_trees.items.insert_state(&entry_state);
        let state = syntax_trees.items.append_state_handle(state);
        return Ok(((HandleSpan::from_parts(state, 1), true), input));
    }

    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut state_start = Handle::invalid();
    let mut state_count = 0u32;
    let implicit_entry_name = entry_name
        .clone()
        .unwrap_or_else(|| Identifier::generated("entry"));

    let parse_implicit_entry = machine_parameters.count() > 0
        || machine_return_type.is_valid()
        || starts_implicit_entry_body(input)
        // An otherwise empty body is still the checked implementation of a
        // zero-argument, Unit-returning callable. Preserve its implicit entry
        // so overload identity and contract ownership survive downstream.
        || input.at_punctuation(PunctuationKind::RightBrace);

    if parse_implicit_entry {
        let (state, rest) = parse_implicit_entry_state(
            syntax_trees,
            input,
            implicit_entry_name,
            machine_parameters,
            machine_return_type,
        )?;
        let handle = append_machine_state(syntax_trees, &mut state_start, &mut state_count, state);
        debug_assert!(handle.is_valid());
        input = rest;
    }

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if starts_retired_entry_member(input)
            || (input.at_keyword(KeywordKind::Public)
                && input
                    .take_keyword(KeywordKind::Public, "public")?
                    .at_contextual("entry"))
        {
            return Err(input.error_here(
                "explicit nested `entry` / `public entry` machine members are retired; put the callable signature on the `machine` head, put its entry statements directly in the machine body, and put visibility on the declaration as `public machine`",
            ));
        }

        let (mut state, rest) = if input.at_keyword(KeywordKind::State) {
            let input2 = input.take_keyword(KeywordKind::State, "state")?;
            parse_state(syntax_trees, input2)?
        } else if starts_retired_invariant_member(input) {
            return Err(input.error_here(
                "the `invariant` machine member is retired: state arrival facts use \
                 `requires`, result facts use `ensures`, and loop facts are derived from \
                 checked transitions",
            ));
        } else {
            return Err(input.expected_one_of_here(&["`state`"]));
        };

        if machine_return_type.is_valid() && !state.return_type.is_valid() {
            state.return_type = machine_return_type;
        }

        append_machine_state(syntax_trees, &mut state_start, &mut state_count, state);
        input = rest;
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let states = HandleSpan::from_parts_or_empty(state_start, state_count);

    Ok(((states, false), input))
}

fn starts_implicit_entry_body(input: Input<'_, '_>) -> bool {
    !input.at_punctuation(PunctuationKind::RightBrace)
        && !input.at_keyword(KeywordKind::Public)
        && !starts_retired_entry_member(input)
        && !starts_state_member(input)
        && !starts_retired_invariant_member(input)
}

pub(super) fn starts_machine_member(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::RightBrace)
        || input.at_keyword(KeywordKind::Public)
        || starts_retired_entry_member(input)
        || starts_state_member(input)
        || starts_retired_invariant_member(input)
}

/// `entry` remains an ordinary contextual name. Only declaration-shaped
/// heads end the implicit statement body; `entry(value);` is a normal call.
fn starts_retired_entry_member(input: Input<'_, '_>) -> bool {
    if !input.at_contextual("entry") {
        return false;
    }
    let after_name = input.restarted(input.tokens.get(1..).unwrap_or_default());
    if after_name
        .tokens
        .first()
        .is_some_and(crate::input::token_cursor::is_identifier_token_for_parser)
    {
        return true;
    }
    if !after_name.at_punctuation(PunctuationKind::LeftParen) {
        return false;
    }
    let mut nesting = 0u32;
    for (position, token) in after_name.tokens.iter().enumerate() {
        match token.kind {
            tokens::TokenKind::Punctuation(PunctuationKind::LeftParen) => nesting += 1,
            tokens::TokenKind::Punctuation(PunctuationKind::RightParen) => {
                nesting = nesting.saturating_sub(1);
                if nesting == 0 {
                    let tail =
                        input.restarted(after_name.tokens.get(position + 1..).unwrap_or_default());
                    return tail.at_punctuation(PunctuationKind::LeftBrace)
                        || tail.at_punctuation(PunctuationKind::Arrow)
                        || [
                            "requires", "ensures", "crashes", "reaches", "blocks", "suspends",
                        ]
                        .iter()
                        .any(|keyword| tail.at_contextual(keyword));
                }
            }
            _ => {}
        }
    }
    false
}

fn starts_retired_invariant_member(input: Input<'_, '_>) -> bool {
    if !input.at_contextual("invariant") {
        return false;
    }

    let after_keyword = input.restarted(input.tokens.get(1..).unwrap_or_default());
    if !after_keyword
        .tokens
        .first()
        .is_some_and(crate::input::token_cursor::is_identifier_token_for_parser)
    {
        return false;
    }

    input
        .restarted(after_keyword.tokens.get(1..).unwrap_or_default())
        .at_punctuation(PunctuationKind::LeftBrace)
}

fn starts_state_member(input: Input<'_, '_>) -> bool {
    if !input.at_keyword(KeywordKind::State) {
        return false;
    }

    input
        .restarted(input.tokens.get(1..).unwrap_or_default())
        .tokens
        .first()
        .is_some_and(crate::input::token_cursor::is_identifier_token_for_parser)
}

fn parse_implicit_entry_state<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    name: Identifier,
    parameters: HandleSpan<StateParameterHandle>,
    return_type: TypeReferenceHandle,
) -> ParseResult<'tokens, 'source, State> {
    let (statements, input) = parse_statements(syntax_trees, input, BodyKind::MachineEntry)?;
    Ok((
        State {
            name,
            parameters,
            return_type,
            contracts: HandleSpan::empty(),
            statements,
        },
        input,
    ))
}

fn append_machine_state(
    syntax_trees: &mut SyntaxTrees,
    state_start: &mut Handle<StateHandle>,
    state_count: &mut u32,
    state: State,
) -> Handle<StateHandle> {
    let handle = syntax_trees.items.insert_state(&state);
    let handle = syntax_trees.items.append_state_handle(handle);
    if *state_count == 0 {
        *state_start = handle;
    }
    *state_count = state_count
        .checked_add(1)
        .expect("machine state span count overflow");
    handle
}
