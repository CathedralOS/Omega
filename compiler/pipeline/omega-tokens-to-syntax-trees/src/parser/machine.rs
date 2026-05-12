use crate::parser::context::StateKind;
use crate::parser::input::{parse_path, Input, ParseResult};
use crate::parser::state::{parse_optional_return_type, parse_state};
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::Machine;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_machine<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Machine> {
    let (path, input) = parse_path(input)?;
    let (machine_return_type, mut input) = parse_optional_return_type(input)?;
    let (name, entry_name) = split_machine_path(path);

    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut states = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_keyword(KeywordKind::Pub) {
            let input2 = input.take_keyword(KeywordKind::Pub, "pub")?;
            let input2 = input2.take_keyword(KeywordKind::Entry, "entry")?;
            let (state, rest) = parse_state(input2, StateKind::Entry)?;
            states.push(state);
            input = rest;
        } else if input.at_keyword(KeywordKind::Entry) {
            let input2 = input.take_keyword(KeywordKind::Entry, "entry")?;
            let (state, rest) = parse_state(input2, StateKind::Entry)?;
            states.push(state);
            input = rest;
        } else if input.at_keyword(KeywordKind::State) {
            let input2 = input.take_keyword(KeywordKind::State, "state")?;
            let (state, rest) = parse_state(input2, StateKind::State)?;
            states.push(state);
            input = rest;
        } else if input.at_keyword(KeywordKind::Fn) {
            let input2 = input.take_keyword(KeywordKind::Fn, "fn")?;
            let (state, rest) = parse_state(input2, StateKind::Function)?;
            states.push(state);
            input = rest;
        } else if input.at_keyword(KeywordKind::Invariant) {
            let input2 = input.take_keyword(KeywordKind::Invariant, "invariant")?;
            let (_, rest) = skip_machine_invariant(input2)?;
            input = rest;
        } else {
            return Err(input.error_here("expected machine item"));
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    if let Some(entry_name) = entry_name {
        if let Some(entry_state) = states.iter_mut().find(|state| state.name == "entry") {
            entry_state.name = entry_name;
        }
    }

    if let Some(return_type) = &machine_return_type {
        for state in &mut states {
            if state.return_type.is_none() {
                state.return_type = Some(return_type.clone());
            }
        }
    }

    Ok((Machine { name, states }, input))
}

fn split_machine_path(path: omega_syntax_trees::identifier::IdentifierPath) -> (Identifier, Option<Identifier>) {
    if path.len() <= 1 {
        return (
            path.as_slice()
                .first()
                .cloned()
                .expect("machine path should contain a name"),
            None,
        );
    }

    let members = path.as_slice();
    let name = Identifier::generated(
        members[..members.len() - 1]
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::"),
    );

    (name, members.last().cloned())
}

fn skip_machine_invariant<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ()> {
    let (_, input) = input.take_identifier()?;

    if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (_, input) = crate::parser::type_reference::parse_type_constraints(input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        Ok(((), input))
    } else {
        let (_, input) = input.skip_braced_block()?;
        Ok(((), input))
    }
}
