use crate::parser::context::StateKind;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::state::{parse_optional_return_type, parse_state};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::Machine;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_machine<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Machine> {
    let (path, input) = parse_path_handle_span(input, |member| {
        syntax_trees.expressions.append_identifier_path_member(member)
    })?;
    let (machine_return_type, mut input) = parse_optional_return_type(syntax_trees, input)?;
    let (name, entry_name) = split_machine_path(syntax_trees, path);

    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut state_start = Handle::invalid();
    let mut state_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (mut state, rest) = if input.at_keyword(KeywordKind::Pub) {
            let input2 = input.take_keyword(KeywordKind::Pub, "pub")?;
            let input2 = input2.take_keyword(KeywordKind::Entry, "entry")?;
            parse_state(syntax_trees, input2, StateKind::Entry)?
        } else if input.at_keyword(KeywordKind::Entry) {
            let input2 = input.take_keyword(KeywordKind::Entry, "entry")?;
            parse_state(syntax_trees, input2, StateKind::Entry)?
        } else if input.at_keyword(KeywordKind::State) {
            let input2 = input.take_keyword(KeywordKind::State, "state")?;
            parse_state(syntax_trees, input2, StateKind::State)?
        } else if input.at_keyword(KeywordKind::Fn) {
            let input2 = input.take_keyword(KeywordKind::Fn, "fn")?;
            parse_state(syntax_trees, input2, StateKind::Function)?
        } else if input.at_keyword(KeywordKind::Invariant) {
            let input2 = input.take_keyword(KeywordKind::Invariant, "invariant")?;
            let (_, rest) = skip_machine_invariant(input2)?;
            input = rest;
            continue;
        } else {
            return Err(input.expected_one_of_here(&[
                "`pub entry`",
                "`entry`",
                "`state`",
                "`fn`",
                "`invariant`",
            ]));
        };

        if let Some(entry_name) = &entry_name {
            if state.name == "entry" {
                state.name = entry_name.clone();
            }
        }

        if machine_return_type.is_valid() && !state.return_type.is_valid() {
            state.return_type = machine_return_type;
        }

        let handle = syntax_trees.items.insert_state(&state);
        let handle = syntax_trees.items.append_state_handle(handle);
        if state_count == 0 {
            state_start = handle;
        }
        state_count = state_count.checked_add(1).expect("machine state span count overflow");
        input = rest;
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let states = if state_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(state_start, state_count)
    };
    Ok((Machine { name, states }, input))
}

fn split_machine_path(
    syntax_trees: &SyntaxTrees,
    path: HandleSpan<Identifier>,
) -> (Identifier, Option<Identifier>) {
    let members = syntax_trees.expressions.identifier_path_members(path);

    if members.len() <= 1 {
        return (
            members
                .first()
                .cloned()
                .expect("machine path should contain a name"),
            None,
        );
    }

    let mut name = String::new();

    for (index, member) in members[..members.len() - 1].iter().enumerate() {
        if index > 0 {
            name.push_str("::");
        }

        name.push_str(member.as_str());
    }

    (Identifier::generated(name), members.last().cloned())
}

fn skip_machine_invariant<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ()> {
    let (_, input) = input.take_identifier()?;

    if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (_, input) = input.skip_bracketed_block()?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        Ok(((), input))
    } else {
        let (_, input) = input.skip_braced_block()?;
        Ok(((), input))
    }
}
