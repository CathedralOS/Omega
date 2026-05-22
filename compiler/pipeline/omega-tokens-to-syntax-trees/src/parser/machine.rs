use crate::parser::context::StateKind;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::state::{
    parse_optional_return_type, parse_optional_state_parameters, parse_state,
};
use crate::parser::statement::parse_statement_handle;
use crate::parser::transition::{
    parse_transition_block_handles, parse_transition_statement_handle,
};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, Machine, State, StateHandle, StateParameterHandle,
};
use omega_syntax_trees::statement::StatementHandle;
use omega_syntax_trees::types::TypeReferenceHandle;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_machine<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Machine> {
    let (path, input) = parse_path_handle_span(input, |member| {
        syntax_trees
            .expressions
            .append_identifier_path_member(member)
    })?;
    let (machine_parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let (machine_return_type, mut input) = parse_optional_return_type(syntax_trees, input)?;
    let (satisfies, next) = parse_satisfies_traits(syntax_trees, input)?;
    let ((effects, contracts), next) = parse_machine_clauses(syntax_trees, next)?;
    input = next;
    let MachinePath {
        name,
        attached_data,
        entry_name,
    } = split_machine_path(syntax_trees, path);

    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut state_start = Handle::invalid();
    let mut state_count = 0u32;
    let implicit_entry_name = entry_name
        .clone()
        .unwrap_or_else(|| Identifier::generated("entry"));

    let parse_implicit_entry = machine_parameters.count() > 0
        || machine_return_type.is_valid()
        || starts_implicit_entry_body(input);

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

        append_machine_state(syntax_trees, &mut state_start, &mut state_count, state);
        input = rest;
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let states = if state_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(state_start, state_count)
    };
    Ok((
        Machine {
            name,
            attached_data,
            satisfies,
            effects,
            contracts,
            states,
        },
        input,
    ))
}

fn parse_machine_clauses<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, (HandleSpan<Identifier>, HandleSpan<CapabilityContract>)> {
    let mut effect_start = Handle::invalid();
    let mut effect_count = 0u32;
    let mut contract_start = Handle::invalid();
    let mut contract_count = 0u32;

    while !input.at_punctuation(PunctuationKind::LeftBrace) {
        if input.at_contextual("effects") {
            input = input.take_contextual("effects")?;
            while !input.at_punctuation(PunctuationKind::LeftBrace)
                && !input.at_contextual("requires")
                && !input.at_contextual("ensures")
                && !input.at_contextual("where")
                && !input.at_contextual("satisfies")
            {
                let (effect, rest) = input.take_identifier()?;
                let handle = syntax_trees.items.append_identifier_path_member(effect);
                if effect_count == 0 {
                    effect_start = handle;
                }
                effect_count = effect_count
                    .checked_add(1)
                    .expect("machine effect span count overflow");
                input = rest;

                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                }
            }
            continue;
        }

        if input.at_contextual("requires") || input.at_contextual("ensures") {
            let kind = if input.at_contextual("requires") {
                input = input.take_contextual("requires")?;
                CapabilityContractKind::Requires
            } else {
                input = input.take_contextual("ensures")?;
                CapabilityContractKind::Ensures
            };
            let (token_count, rest) = skip_machine_contract_tokens(input)?;
            let handle = syntax_trees
                .items
                .append_capability_contract(CapabilityContract { kind, token_count });
            if contract_count == 0 {
                contract_start = handle;
            }
            contract_count = contract_count
                .checked_add(1)
                .expect("machine contract span count overflow");
            input = rest;
            continue;
        }

        let (_, rest) = input.expect_token()?;
        input = rest;
    }

    let effects = if effect_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(effect_start, effect_count)
    };
    let contracts = if contract_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(contract_start, contract_count)
    };
    Ok(((effects, contracts), input))
}

fn skip_machine_contract_tokens<'tokens, 'source>(
    mut input: Input<'tokens, 'source>,
) -> Result<(usize, Input<'tokens, 'source>), crate::parse_error::ParseError> {
    let mut count = 0usize;
    while !(input.at_punctuation(PunctuationKind::LeftBrace)
        || input.at_contextual("requires")
        || input.at_contextual("ensures")
        || input.at_contextual("effects")
        || input.at_contextual("where")
        || input.at_contextual("satisfies")
        || input.tokens.is_empty())
    {
        let (_, rest) = input.expect_token()?;
        input = rest;
        count += 1;
    }
    Ok((count, input))
}

fn parse_satisfies_traits<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<Identifier>> {
    if !input.at_contextual("satisfies") {
        return Ok((HandleSpan::empty(), input));
    }

    input = input.take_contextual("satisfies")?;
    let mut trait_start = Handle::invalid();
    let mut trait_count = 0u32;

    loop {
        let (trait_name, rest) = input.take_identifier()?;
        let handle = syntax_trees.items.append_identifier_path_member(trait_name);
        if trait_count == 0 {
            trait_start = handle;
        }
        trait_count = trait_count
            .checked_add(1)
            .expect("machine satisfies span count overflow");
        input = rest;

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }

        break;
    }

    let satisfies = if trait_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(trait_start, trait_count)
    };
    Ok((satisfies, input))
}

fn starts_implicit_entry_body(input: Input<'_, '_>) -> bool {
    !input.at_punctuation(PunctuationKind::RightBrace)
        && !input.at_keyword(KeywordKind::Pub)
        && !input.at_keyword(KeywordKind::Entry)
        && !starts_state_member(input)
        && !input.at_keyword(KeywordKind::Invariant)
}

fn starts_machine_member(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::RightBrace)
        || input.at_keyword(KeywordKind::Pub)
        || input.at_keyword(KeywordKind::Entry)
        || starts_state_member(input)
        || input.at_keyword(KeywordKind::Invariant)
}

fn starts_state_member(input: Input<'_, '_>) -> bool {
    if !input.at_keyword(KeywordKind::State) {
        return false;
    }

    Input::new(input.source_id, input.tokens.get(1..).unwrap_or_default())
        .tokens
        .first()
        .is_some_and(crate::parser::input::is_identifier_token_for_parser)
}

fn parse_implicit_entry_state<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    name: Identifier,
    parameters: HandleSpan<StateParameterHandle>,
    return_type: TypeReferenceHandle,
) -> ParseResult<'tokens, 'source, State> {
    let (statements, input) = parse_implicit_entry_statements(syntax_trees, input)?;
    Ok((
        State {
            name,
            parameters,
            return_type,
            statements,
        },
        input,
    ))
}

fn parse_implicit_entry_statements<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let mut statement_start = Handle::invalid();
    let mut statement_count = 0u32;

    while !starts_machine_member(input) {
        if input.at_punctuation(PunctuationKind::Arrow) {
            let next = input.take_punctuation(PunctuationKind::Arrow, "->")?;
            let (statement, rest) = parse_transition_statement_handle(syntax_trees, next)?;
            append_statement_handle(
                syntax_trees,
                &mut statement_start,
                &mut statement_count,
                statement,
            );
            input = rest;
        } else if input.at_keyword(KeywordKind::Transition) || input.at_keyword(KeywordKind::Match)
        {
            let next = if input.at_keyword(KeywordKind::Transition) {
                input.take_keyword(KeywordKind::Transition, "transition")?
            } else {
                input.take_keyword(KeywordKind::Match, "match")?
            };
            let (new_statements, rest) = parse_transition_block_handles(syntax_trees, next)?;
            if !new_statements.is_empty() {
                if statement_count == 0 {
                    statement_start = new_statements.start();
                }
                statement_count = statement_count
                    .checked_add(new_statements.count())
                    .expect("state statement span count overflow");
            }
            input = rest;
        } else {
            let (statement, rest) = parse_statement_handle(syntax_trees, input)?;
            append_statement_handle(
                syntax_trees,
                &mut statement_start,
                &mut statement_count,
                statement,
            );
            input = rest;
        }
    }

    let statements = if statement_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(statement_start, statement_count)
    };
    Ok((statements, input))
}

fn append_statement_handle(
    syntax_trees: &mut SyntaxTrees,
    statement_start: &mut Handle<StatementHandle>,
    statement_count: &mut u32,
    statement: StatementHandle,
) {
    let handle = syntax_trees.items.append_statement_handle(statement);
    if *statement_count == 0 {
        *statement_start = handle;
    }
    *statement_count = statement_count
        .checked_add(1)
        .expect("state statement span count overflow");
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

struct MachinePath {
    name: Identifier,
    attached_data: Option<Identifier>,
    entry_name: Option<Identifier>,
}

fn split_machine_path(syntax_trees: &SyntaxTrees, path: HandleSpan<Identifier>) -> MachinePath {
    let members = syntax_trees.expressions.identifier_path_members(path);

    if members.len() <= 1 {
        return MachinePath {
            name: members
                .first()
                .cloned()
                .expect("machine path should contain a name"),
            attached_data: None,
            entry_name: None,
        };
    }

    let mut name = String::new();
    let mut attached_data = String::new();

    for (index, member) in members.iter().enumerate() {
        if index > 0 {
            name.push_str("::");
        }

        name.push_str(member.as_str());
    }

    for (index, member) in members[..members.len() - 1].iter().enumerate() {
        if index > 0 {
            attached_data.push_str("::");
        }

        attached_data.push_str(member.as_str());
    }

    MachinePath {
        name: Identifier::generated(name),
        attached_data: Some(Identifier::generated(attached_data)),
        entry_name: members.last().cloned(),
    }
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
