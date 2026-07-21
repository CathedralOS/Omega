use crate::parser::context::StateKind;
use crate::parser::data::parse_machine_type_parameters;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::state::{
    parse_optional_return_type, parse_optional_state_parameters, parse_state,
};
use crate::parser::statement::{
    parse_asm_block_statement_handles, parse_statement_handle,
    try_parse_atomic_compare_exchange_let, try_parse_atomic_fetch_add_let,
    try_parse_atomic_swap_let, try_parse_destructure_let,
};
use crate::parser::transition::parse_transition_block_handles;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{Machine, State, StateHandle, StateParameterHandle};
use omega_syntax_trees::statement::StatementHandle;
use omega_syntax_trees::types::TypeReferenceHandle;
use omega_tokens::{KeywordKind, PunctuationKind};

mod clauses;

use clauses::{parse_machine_clauses, parse_satisfies_traits};

pub(super) fn parse_machine<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Machine> {
    let (path, input) = parse_path_handle_span(input, |member| {
        syntax_trees
            .expressions
            .append_identifier_path_member(member)
    })?;
    let (type_parameters, input) = parse_machine_type_parameters(syntax_trees, input)?;
    let (machine_parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let (machine_return_type, input) = parse_optional_return_type(syntax_trees, input)?;
    let ((), mut input) = parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
    let (satisfies, next) = parse_satisfies_traits(syntax_trees, input)?;
    if next.at_contextual("via") {
        return Err(next.error_here(
            "`via <Binding>` cannot supply a machine by itself; name the requirement first as \
             `satisfies Trait::machine via Binding::Case(...)`",
        ));
    }
    let (
        (
            terminates,
            terminates_guarantee,
            decreases,
            decrease_order,
            decrease_view_arguments,
            decrease_range,
            effects,
            contracts,
            clauses_return_type,
        ),
        next,
    ) = parse_machine_clauses(syntax_trees, next)?;
    input = next;
    // `-> T` is written either before the clauses or after them
    // (`terminates by ..; -> usize`); both spell the machine's return type.
    let machine_return_type = if machine_return_type.is_valid() {
        machine_return_type
    } else {
        clauses_return_type
    };
    let MachinePath {
        name,
        attached_data,
        entry_name,
    } = split_machine_path(syntax_trees, path);

    // CH10 ACCEPTED FORM (GR6d): `boundary machine f(..) ensures <fact>;`
    // -- a contract with NO body ends at `;` in body position. The implicit
    // ENTRY state still materializes (empty; it carries the signature so
    // citations bind parameters). The item parser refuses the bodyless form
    // without `boundary`.
    if input.at_punctuation(PunctuationKind::Semicolon) {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        let mut state_start = Handle::invalid();
        let mut state_count = 0u32;
        let entry_state = State {
            name: entry_name
                .clone()
                .unwrap_or_else(|| Identifier::generated("entry")),
            parameters: machine_parameters,
            return_type: machine_return_type,
            statements: HandleSpan::empty(),
        };
        append_machine_state(
            syntax_trees,
            &mut state_start,
            &mut state_count,
            entry_state,
        );
        return Ok((
            Machine {
                name,
                attached_data,
                target: None,
                boundary: false,
                bodyless: true,
                type_parameters,
                satisfies,
                terminates,
                terminates_guarantee,
                decreases,
                decrease_order,
                decrease_view_arguments,
                decrease_range,
                effects,
                contracts,
                states: HandleSpan::from_parts(state_start, state_count),
            },
            input,
        ));
    }

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
    // Measured recursion MR2: a MEASURED machine's state whose TERMINAL
    // expression is a self-call to the machine's own entry (`{ self.sum(n -
    // 1, acc + n) }`, or the bare `sum(..)` in a free machine) is TAIL
    // recursion -- rewrite it to the loop-back transition `{ _ ->
    // <entry>(args) }` here, so every downstream pass (termination decrease
    // proof, loop-carried arg staging, both engines) sees the same bare
    // back-edge the arm spelling produces. Unmeasured machines keep the
    // call; validation names the missing measure.
    if terminates && !decreases.is_empty() {
        let entry_callable = entry_name.clone().unwrap_or_else(|| name.clone());
        rewrite_terminal_tail_self_calls(
            syntax_trees,
            states,
            &entry_callable,
            entry_name.is_some(),
        );
    }
    Ok((
        Machine {
            name,
            attached_data,
            target: None,
            boundary: false,
            bodyless: false,
            type_parameters,
            satisfies,
            terminates,
            terminates_guarantee,
            decreases,
            decrease_order,
            decrease_view_arguments,
            decrease_range,
            effects,
            contracts,
            states,
        },
        input,
    ))
}

/// Parse the declaration-site contracts for static machine-symbol parameters.
/// The parameter list only names symbols (`<machine F>`); every such symbol
/// must receive exactly one authored `where machine F(...)` requirement before
/// the executable body's clauses begin. Nothing is inferred from uses.
fn parse_machine_parameter_contracts<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_parameters: HandleSpan<omega_syntax_trees::item::TypeParameter>,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ()> {
    loop {
        if !input.at_contextual("where") {
            break;
        }
        let after_where = input.take_contextual("where")?;
        if !after_where.at_keyword(KeywordKind::Machine) {
            break;
        }
        let after_machine = after_where.take_keyword(KeywordKind::Machine, "machine")?;
        let (name, after_name) = after_machine.take_identifier()?;

        // The ch13 ONE-OFF METHOD REQUIREMENT on a TYPE parameter
        // (`where machine T::increment(&mut self)`) is a different clause:
        // the `::` after the name discriminates it from an MP1 machine-
        // parameter contract (`where machine M(args) -> R`). Leave it for
        // the general where-clause parser.
        if after_name.at_punctuation(PunctuationKind::ColonColon) {
            break;
        }

        let parameter_index = syntax_trees
            .items
            .type_parameters(type_parameters)
            .iter()
            .position(|parameter| parameter.name == name)
            .ok_or_else(|| {
                after_name.error_here(format!(
                    "`where machine {}` has no matching `<machine {}>` parameter",
                    name.as_str(),
                    name.as_str()
                ))
            })?;

        match &syntax_trees.items.type_parameters(type_parameters)[parameter_index].kind {
            omega_syntax_trees::item::TypeParameterKind::Machine { contract: None } => {}
            omega_syntax_trees::item::TypeParameterKind::Machine { contract: Some(_) } => {
                return Err(after_name.error_here(format!(
                    "machine parameter `{}` already has a `where machine` contract",
                    name.as_str()
                )));
            }
            _ => {
                return Err(after_name.error_here(format!(
                    "`{}` is not a machine parameter; declare it as `<machine {}>` before writing a machine contract",
                    name.as_str(),
                    name.as_str()
                )));
            }
        }

        let (parameters, after_parameters) =
            parse_optional_state_parameters(syntax_trees, after_name)?;
        let (return_type, after_return) =
            parse_optional_return_type(syntax_trees, after_parameters)?;
        let ((effects, contracts, terminates_guarantee), mut rest) =
            crate::parser::trait_definition::parse_signature_clauses(syntax_trees, after_return)?;
        // Permit a separator after the requirement. The semicolon belongs to
        // this `where machine` signature, never to the generic machine body.
        if rest.at_punctuation(PunctuationKind::Semicolon) {
            rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
        }

        let contract = omega_syntax_trees::item::StateSignature {
            name: name.clone(),
            is_default: false,
            parameters,
            return_type,
            effects,
            contracts,
            default_body: HandleSpan::empty(),
            terminates_guarantee,
        };
        let parameter =
            &mut syntax_trees.items.type_parameters_mut(type_parameters)[parameter_index];
        parameter.kind = omega_syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(contract),
        };
        input = rest;
    }

    if let Some(missing) = syntax_trees
        .items
        .type_parameters(type_parameters)
        .iter()
        .find(|parameter| {
            matches!(
                parameter.kind,
                omega_syntax_trees::item::TypeParameterKind::Machine { contract: None }
            )
        })
    {
        return Err(input.error_here(format!(
            "machine parameter `{}` requires an authored declaration-site contract: write `where machine {}(...) -> Result`",
            missing.name.as_str(),
            missing.name.as_str()
        )));
    }

    Ok(((), input))
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
            return Err(input.error_here(
                "machine entry bodies must use the `transition` keyword; bare `->` transitions are not supported",
            ));
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
        } else if input.at_contextual("asm") {
            let (new_statements, rest) = parse_asm_block_statement_handles(syntax_trees, input)?;
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        // ATOMICS STAGE 1 (ch17, M3): `let name: T = place.fetch_add(n, ord);`
        // expands to two statements (capture prior + increment).
        // RECORD PATTERNS IN LET POSITION (owner spec 2026-07-18):
        // `let { x, y as h, z as _ } = place;` expands to the marker +
        // per-field lets.
        } else if let Some((new_statements, rest)) = try_parse_destructure_let(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        } else if let Some((new_statements, rest)) =
            try_parse_atomic_fetch_add_let(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        } else if let Some((new_statements, rest)) = try_parse_atomic_swap_let(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        // ATOMICS STAGE 1 (ch17, M4): `let name: T = place.compare_exchange(expected, new_val, succ_ord, fail_ord);`
        // expands to two statements (capture prior + conditional swap).
        } else if let Some((new_statements, rest)) =
            try_parse_atomic_compare_exchange_let(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
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

/// MR2's rewrite walk (see the call site above): for each state, when the
/// LAST statement is a bare terminal expression that IS a self-entry call,
/// replace it with an always-transition to the entry carrying the same
/// argument expressions.
fn rewrite_terminal_tail_self_calls(
    syntax_trees: &mut SyntaxTrees,
    states: HandleSpan<StateHandle>,
    entry_callable: &Identifier,
    receiver_must_be_self: bool,
) {
    use omega_syntax_trees::expression::ExpressionNode;
    use omega_syntax_trees::statement::{
        StatementNode, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };

    // Phase 1 (reads): collect the rewrite sites.
    let mut sites: Vec<(
        StatementHandle,
        Vec<omega_syntax_trees::expression::ExpressionHandle>,
    )> = Vec::new();
    for state_handle in syntax_trees.items.state_handles(states).to_vec() {
        let state = syntax_trees.items.state(state_handle);
        let Some(&last) = syntax_trees.items.statements(state.statements).last() else {
            continue;
        };
        let StatementNode::Expression(expression) = syntax_trees.statements.statement(last) else {
            continue;
        };
        let ExpressionNode::Call(call) = syntax_trees.expressions.expression(*expression) else {
            continue;
        };
        if call.target.as_str() != entry_callable.as_str() {
            continue;
        }
        let receiver_is_self = call.receiver.is_valid()
            && matches!(
                syntax_trees.expressions.expression(call.receiver),
                ExpressionNode::SelfValue
            );
        let shape_matches = if receiver_must_be_self {
            receiver_is_self
        } else {
            !call.receiver.is_valid()
        };
        if !shape_matches {
            continue;
        }
        let arguments = syntax_trees
            .expressions
            .expression_handles(call.arguments)
            .to_vec();
        sites.push((last, arguments));
    }

    // Phase 2 (writes): mint the bare Named loop-back per site.
    for (statement_handle, arguments) in sites {
        let path_start = syntax_trees
            .statements
            .append_identifier_path_member(entry_callable.clone());
        let path = HandleSpan::from_parts(path_start, 1);
        let mut argument_span = HandleSpan::empty();
        for (index, argument) in arguments.iter().enumerate() {
            let handle = syntax_trees.statements.append_expression_handle(*argument);
            if index == 0 {
                argument_span = HandleSpan::from_parts(handle, arguments.len() as u32);
            }
        }
        let target =
            syntax_trees
                .statements
                .insert_transition_target(TransitionTargetNode::Named {
                    path,
                    path_starts_at_self: false,
                    arguments: argument_span,
                });
        syntax_trees.statements.replace_statement(
            statement_handle,
            StatementNode::Transition(TableTransition {
                target,
                continuation: omega_syntax_trees::statement::TransitionTargetHandle::invalid(),
                guard: TransitionGuardNode::Always,
            }),
        );
    }
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

    let name = join_path(members);
    let attached_data = join_path(&members[..members.len() - 1]);

    MachinePath {
        name: Identifier::generated(name),
        attached_data: Some(Identifier::generated(attached_data)),
        entry_name: members.last().cloned(),
    }
}

fn join_path(members: &[Identifier]) -> String {
    let mut name = String::new();

    for (index, member) in members.iter().enumerate() {
        if index > 0 {
            name.push_str("::");
        }

        name.push_str(member.as_str());
    }

    name
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
