use crate::parser::context::StateKind;
use crate::parser::data::{parse_machine_declaration_parameters, parse_machine_type_parameters};
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::state::{
    parse_optional_return_type, parse_optional_state_parameters, parse_state,
};
use crate::parser::statement::{
    parse_asm_block_statement_handles, parse_statement_handle, reject_retired_proof_output_binding,
    try_parse_atomic_compare_exchange_let, try_parse_atomic_fetch_let, try_parse_atomic_swap_let,
    try_parse_destructure_let, try_parse_proof_output_binding,
};
use crate::parser::transition::parse_transition_block_handles;
use psi_arena::{Handle, HandleSpan};
use psi_source::{SourceSpan, Span};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{Machine, State, StateHandle, StateParameterHandle};
use psi_syntax_trees::statement::StatementHandle;
use psi_syntax_trees::types::TypeReferenceHandle;
use psi_tokens::{KeywordKind, PunctuationKind};

mod clauses;

pub(super) use clauses::parse_generic_conformance_bounds;
pub(in crate::parser) use clauses::parse_optional_satisfies_type_arguments;
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
    let (generic_parameters, input) = parse_machine_declaration_parameters(syntax_trees, input)?;
    let type_parameters = generic_parameters.type_parameters;
    let lifetime_parameters = generic_parameters.lifetime_parameters;
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
            terminates_guarantee,
            ranking_subjects,
            ranking_view,
            ranking_view_arguments,
            ranking_range,
            service_reach_is_installation_bound,
            service_reaches,
            invokes,
            suspends,
            blocks,
            contracts,
            clauses_return_type,
            mut conformance_bounds,
        ),
        next,
    ) = parse_machine_clauses(syntax_trees, next)?;
    let mut header_conformance_bounds = generic_parameters.conformance_bounds;
    header_conformance_bounds.append(&mut conformance_bounds);
    let conformance_bounds = header_conformance_bounds;
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
            contracts: HandleSpan::empty(),
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
                is_public: false,
                target: None,
                boundary: false,
                bodyless: true,
                lifetime_parameters,
                type_parameters,
                satisfies,
                conformance_bounds,
                terminates_guarantee,
                ranking_subjects,
                ranking_view,
                ranking_view_arguments,
                ranking_range,
                service_reach_is_installation_bound,
                service_reaches,
                invokes,
                suspends,
                blocks,
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
        } else if starts_retired_invariant_member(input) {
            return Err(input.error_here(
                "the `invariant` machine member is retired: state arrival facts use \
                 `requires`, result facts use `ensures`, and loop facts are derived from \
                 checked transitions",
            ));
        } else {
            return Err(input.expected_one_of_here(&["`pub entry`", "`entry`", "`state`"]));
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
    if !ranking_subjects.is_empty() {
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
            is_public: false,
            target: None,
            boundary: false,
            bodyless: false,
            lifetime_parameters,
            type_parameters,
            satisfies,
            conformance_bounds,
            terminates_guarantee,
            ranking_subjects,
            ranking_view,
            ranking_view_arguments,
            ranking_range,
            service_reach_is_installation_bound,
            service_reaches,
            invokes,
            suspends,
            blocks,
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
pub(super) fn parse_machine_parameter_contracts<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_parameters: HandleSpan<psi_syntax_trees::item::TypeParameter>,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ()> {
    parse_machine_parameter_contracts_in(syntax_trees, type_parameters, input, true)
}

fn parse_machine_parameter_contracts_in<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_parameters: HandleSpan<psi_syntax_trees::item::TypeParameter>,
    mut input: Input<'tokens, 'source>,
    reject_unknown_parameter: bool,
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

        // A `::` after the name is not a machine-symbol parameter contract.
        // Leave it for the general clause parser, which rejects the retired
        // one-off member requirement rather than silently discarding it.
        if after_name.at_punctuation(PunctuationKind::ColonColon) {
            break;
        }

        let Some(parameter_index) = syntax_trees
            .items
            .type_parameters(type_parameters)
            .iter()
            .position(|parameter| parameter.name == name)
        else {
            if reject_unknown_parameter {
                return Err(after_name.error_here(format!(
                    "`where machine {}` has no matching `<machine {}>` parameter",
                    name.as_str(),
                    name.as_str()
                )));
            }
            // A nested signature's requirements are written in the same
            // clause stream as its owner's remaining requirements. An
            // unknown name at this level therefore belongs to the caller;
            // leave the complete `where machine` clause unconsumed.
            break;
        };

        match &syntax_trees.items.type_parameters(type_parameters)[parameter_index].kind {
            psi_syntax_trees::item::TypeParameterKind::Machine { contract: None } => {}
            psi_syntax_trees::item::TypeParameterKind::Machine { contract: Some(_) } => {
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

        if after_name.at_contextual("satisfies") {
            let after_satisfies = after_name.take_contextual("satisfies")?;
            let (requirement, after_requirement) =
                parse_path_handle_span(after_satisfies, |member| {
                    syntax_trees.items.append_identifier_path_member(member)
                })?;
            if after_requirement.at_punctuation(PunctuationKind::Less) {
                return Err(after_requirement.error_here(
                    "generic trait arguments are not supported in nominal machine parameter requirements; name one exact `Trait::requirement`",
                ));
            }
            if requirement.len() < 2 {
                return Err(after_requirement.error_here(format!(
                    "nominal machine parameter `{}` must name an exact `Trait::requirement`; bare `satisfies Trait` is not a callable contract",
                    name.as_str()
                )));
            }
            if after_requirement.at_contextual("as") {
                return Err(after_requirement.error_here(
                    "nominal machine parameter requirements do not accept `as Name`; `Trait::requirement` must resolve uniquely",
                ));
            }
            if after_requirement.at_contextual("via") {
                return Err(after_requirement.error_here(
                    "a `where machine` nominal requirement cannot use `via`; bindings belong on the satisfying bodyless machine",
                ));
            }
            let after_semicolon =
                after_requirement.take_punctuation(PunctuationKind::Semicolon, ";")?;
            let rest = if continues_after_machine_parameter_contract(after_semicolon) {
                after_semicolon
            } else {
                // A bodyless declaration's final semicolon terminates both the
                // nominal binder and the declaration. Leave it for the outer
                // machine parser, just like operational clauses do.
                after_requirement
            };
            let parameter =
                &mut syntax_trees.items.type_parameters_mut(type_parameters)[parameter_index];
            parameter.kind = psi_syntax_trees::item::TypeParameterKind::Machine {
                contract: Some(psi_syntax_trees::item::MachineParameterContract::Nominal {
                    requirement,
                }),
            };
            input = rest;
            continue;
        }

        let (nested_generic_parameters, after_type_parameters) =
            parse_machine_type_parameters(syntax_trees, after_name)?;
        let nested_type_parameters = nested_generic_parameters.type_parameters;
        let (parameters, after_parameters) =
            parse_optional_state_parameters(syntax_trees, after_type_parameters)?;
        let (return_type, after_return) =
            parse_optional_return_type(syntax_trees, after_parameters)?;
        let ((), after_nested_contracts) = parse_machine_parameter_contracts_in(
            syntax_trees,
            nested_type_parameters,
            after_return,
            false,
        )?;
        let (
            (
                service_reaches,
                service_reach_is_installation_bound,
                invokes,
                suspends,
                blocks,
                contracts,
                terminates_guarantee,
            ),
            mut rest,
        ) = crate::parser::trait_definition::parse_signature_clauses(
            syntax_trees,
            after_nested_contracts,
            false,
        )?;
        if service_reach_is_installation_bound {
            return Err(rest.error_here(
                "`reaches <= Bound` is installation-selected and cannot appear on a structural machine parameter",
            ));
        }
        // Permit a separator after the requirement. The semicolon belongs to
        // this `where machine` signature, never to the generic machine body.
        if rest.at_punctuation(PunctuationKind::Semicolon) {
            rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
        }

        let contract = psi_syntax_trees::item::StateSignature {
            name: name.clone(),
            spelling: None,
            lifetime_parameters: nested_generic_parameters.lifetime_parameters,
            type_parameters: nested_type_parameters,
            is_default: false,
            parameters,
            return_type,
            service_reach_is_installation_bound: false,
            service_reaches,
            invokes,
            suspends,
            blocks,
            contracts,
            default_body: HandleSpan::empty(),
            terminates_guarantee,
        };
        let parameter =
            &mut syntax_trees.items.type_parameters_mut(type_parameters)[parameter_index];
        parameter.kind = psi_syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(psi_syntax_trees::item::MachineParameterContract::Structural(contract)),
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
                psi_syntax_trees::item::TypeParameterKind::Machine { contract: None }
            )
        })
    {
        return Err(input.error_here(format!(
            "machine parameter `{}` requires an authored declaration-site contract: write `where machine {}(...) -> Result` or `where machine {} satisfies Trait::requirement;`",
            missing.name.as_str(),
            missing.name.as_str(),
            missing.name.as_str(),
        )));
    }

    Ok(((), input))
}

fn continues_after_machine_parameter_contract(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::LeftBrace)
        || input.at_punctuation(PunctuationKind::Arrow)
        || input.at_contextual("terminates")
        || input.at_contextual("decreases")
        || input.at_contextual("reaches")
        || input.at_contextual("effects")
        || input.at_contextual("invokes")
        || input.at_contextual("suspends")
        || input.at_contextual("blocks")
        || input.at_contextual("crashes")
        || input.at_contextual("boundary")
        || input.at_contextual("requires")
        || input.at_contextual("ensures")
        || input.at_contextual("where")
        || input.at_contextual("satisfies")
}

fn starts_implicit_entry_body(input: Input<'_, '_>) -> bool {
    !input.at_punctuation(PunctuationKind::RightBrace)
        && !input.at_keyword(KeywordKind::Pub)
        && !input.at_keyword(KeywordKind::Entry)
        && !starts_state_member(input)
        && !starts_retired_invariant_member(input)
}

fn starts_machine_member(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::RightBrace)
        || input.at_keyword(KeywordKind::Pub)
        || input.at_keyword(KeywordKind::Entry)
        || starts_state_member(input)
        || starts_retired_invariant_member(input)
}

fn starts_retired_invariant_member(input: Input<'_, '_>) -> bool {
    if !input.at_contextual("invariant") {
        return false;
    }

    let after_keyword = Input::new(input.source_id, input.tokens.get(1..).unwrap_or_default());
    if !after_keyword
        .tokens
        .first()
        .is_some_and(crate::parser::input::is_identifier_token_for_parser)
    {
        return false;
    }

    Input::new(
        input.source_id,
        after_keyword.tokens.get(1..).unwrap_or_default(),
    )
    .at_punctuation(PunctuationKind::LeftBrace)
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
            contracts: HandleSpan::empty(),
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
        reject_retired_proof_output_binding(input)?;
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
        } else if let Some((new_statements, rest)) =
            try_parse_proof_output_binding(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("machine statement span count overflow");
            input = rest;
        } else if let Some((new_statements, rest)) = try_parse_destructure_let(syntax_trees, input)
        {
            if statement_count == 0 {
                statement_start = new_statements.start();
            }
            statement_count = statement_count
                .checked_add(new_statements.count())
                .expect("state statement span count overflow");
            input = rest;
        } else if let Some((new_statements, rest)) = try_parse_atomic_fetch_let(syntax_trees, input)
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
    use psi_syntax_trees::expression::ExpressionNode;
    use psi_syntax_trees::statement::{
        StatementNode, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };

    // Phase 1 (reads): collect the rewrite sites.
    let mut sites: Vec<(
        StatementHandle,
        Vec<psi_syntax_trees::expression::ExpressionHandle>,
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
                    evidence_arguments: Box::default(),
                });
        syntax_trees.statements.replace_statement(
            statement_handle,
            StatementNode::Transition(TableTransition {
                target,
                continuation: psi_syntax_trees::statement::TransitionTargetHandle::invalid(),
                guard: TransitionGuardNode::Always,
                exit: Default::default(),
                source_span: Default::default(),
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

    let name = join_path_identifier(members);
    let attached_data = join_path_identifier(&members[..members.len() - 1]);

    MachinePath {
        name,
        attached_data: Some(attached_data),
        entry_name: members.last().cloned(),
    }
}

fn join_path_identifier(members: &[Identifier]) -> Identifier {
    let mut name = String::new();

    for (index, member) in members.iter().enumerate() {
        if index > 0 {
            name.push_str("::");
        }

        name.push_str(member.as_str());
    }

    let first = members
        .first()
        .expect("joined machine path should contain a name")
        .source_span();
    let last = members
        .last()
        .expect("joined machine path should contain a name")
        .source_span();
    debug_assert_eq!(first.source_id, last.source_id);
    Identifier::new(
        name,
        SourceSpan::new(first.source_id, Span::new(first.span.start, last.span.end)),
    )
}

#[cfg(test)]
mod tests {
    use super::join_path_identifier;
    use psi_source::{SourceId, SourceSpan, Span};
    use psi_syntax_trees::identifier::Identifier;

    #[test]
    fn joined_machine_path_retains_authored_source_span() {
        let source = SourceId(7);
        let members = [
            Identifier::new("Provider", SourceSpan::new(source, Span::new(11, 19))),
            Identifier::new("first", SourceSpan::new(source, Span::new(21, 26))),
        ];

        let joined = join_path_identifier(&members);

        assert_eq!(joined.as_str(), "Provider::first");
        assert_eq!(
            joined.source_span(),
            SourceSpan::new(source, Span::new(11, 26))
        );
    }
}
