//! Original call observations which survive the selected normal cleanup.
//!
//! Requires was judged before cleanup. Here only its observation correspondence
//! is retained: complete source-frame projection and complete storage effects,
//! never another proof search or operand evaluation. Erased reads obey that
//! same epoch; a descriptor copy alone cannot freeze its referent.
use checked_trees::{
    CallExitPreservation, CheckFacts, CheckedScalarExpression as Scalar, ErasedExitObservation,
    FlowCallFact, FlowClaimJoinExit, FlowClaimJoinExitKind, FlowExitFact, FlowStateFact,
};
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use validation::CanonicalPlace;

pub(super) fn record(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    frames: Option<&validation::CallFrameResolver<'_>>,
) {
    let mut rows = arena::Arena::default();
    let mut erased = arena::Arena::default();
    for (_, source) in facts.flow.control.states.iter() {
        let Some((machine, state)) =
            program.state_flow_site(source.machine_symbol, source.state_symbol)
        else {
            continue;
        };
        for call_handle in source.calls.handles() {
            let call = facts.flow.control.calls.get(call_handle);
            if facts.flow.control.is_retired(source.state_symbol, call) {
                continue;
            }
            let Ok(statement_index) = u32::try_from(call.statement_index) else {
                continue;
            };
            let Some(target) = validation::call_target_state(program, call.target_symbol) else {
                continue;
            };
            for exit_fact in facts.flow.control.exits.span_or_empty(source.exits) {
                if !is_whole_exit_call(program, facts, source, call_handle, call, exit_fact) {
                    continue;
                }
                let Some(exit) = exit_coordinate(program, exit_fact) else {
                    continue;
                };
                let requires_preserved = facts.call_requires_decided(
                    source.state_symbol,
                    statement_index,
                    call.target_symbol,
                ) && requires_reads(program, facts, source, call, frames)
                    .is_some_and(|reads| {
                        super::cleanup_preservation::reads_survive(
                            program,
                            facts,
                            source.machine_symbol,
                            exit,
                            &reads,
                        )
                    });
                let mut observed = Vec::new();
                let Some(arguments) = super::contracts::call_values::captured_argument_expressions(
                    program, facts, source, call,
                ) else {
                    continue;
                };
                if arguments.len() != program.state_parameters(target).len() {
                    continue;
                }
                for (ordinal, (parameter, argument)) in program
                    .state_parameters(target)
                    .iter()
                    .zip(arguments)
                    .enumerate()
                {
                    if !parameter.relevance.is_erased() {
                        continue;
                    }
                    let Ok(ordinal) = u32::try_from(ordinal) else {
                        continue;
                    };
                    let role = checked_trees::CheckedScalarExpressionRole::TransitionArgument {
                        argument_ordinal: ordinal,
                    };
                    let scalar = facts
                        .values
                        .scalar_expressions
                        .bound_expression_at(state.symbol, statement_index, role)
                        .filter(|(custody, _)| {
                            custody.expression == argument
                                && custody.destination == parameter.symbol
                        })
                        .map(|(_, expression)| expression);
                    let proof = facts
                        .values
                        .proof_terms
                        .located_term_at(
                            state.symbol,
                            statement_index,
                            checked_trees::CheckedProofTermRole::TransitionArgument {
                                argument_ordinal: ordinal,
                            },
                        )
                        .filter(|term| term.expression == argument);
                    let reads = if let Some(scalar) = scalar {
                        current_scalar_reads(program, state, call.statement_index, scalar, frames)
                    } else if let Some(proof) = proof {
                        current_proof_reads(
                            program,
                            state,
                            call.statement_index,
                            &proof.term,
                            frames,
                        )
                    } else {
                        None
                    };
                    let preserved = reads.is_some_and(|reads| {
                        super::cleanup_preservation::reads_survive(
                            program,
                            facts,
                            machine.symbol,
                            exit,
                            &reads,
                        )
                    });
                    observed.push(ErasedExitObservation {
                        argument_ordinal: ordinal,
                        expression: argument,
                        preserved,
                    });
                }
                rows.insert(CallExitPreservation {
                    machine: source.machine_symbol,
                    exit,
                    call: call_handle,
                    target_state: target.symbol,
                    requires_preserved,
                    rank_observation: arena::Handle::invalid(),
                    rank_preserved: false,
                    erased_observations: erased.insert_many(observed),
                });
            }
        }
    }
    facts.call_exit_preservations = rows;
    facts.erased_exit_observations = erased;
}

fn exit_coordinate(
    program: &dyn validation::CheckingInput,
    source: &FlowExitFact,
) -> Option<FlowClaimJoinExit> {
    let state = program.state_by_symbol(source.state_symbol)?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(source.statement_index)?;
    let kind = match statement {
        StatementNode::Expression(_) if !source.transition_target.is_valid() => {
            FlowClaimJoinExitKind::FinalExpression
        }
        StatementNode::Transition(transition) if transition.target == source.transition_target => {
            FlowClaimJoinExitKind::TransitionTarget
        }
        StatementNode::Transition(transition)
            if transition.continuation.is_valid()
                && transition.continuation == source.transition_target =>
        {
            FlowClaimJoinExitKind::TransitionContinuation
        }
        _ => return None,
    };
    Some(FlowClaimJoinExit {
        state_symbol: state.symbol,
        statement_index: source.statement_index,
        kind,
    })
}

fn is_whole_exit_call(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    source: &FlowStateFact,
    handle: arena::Handle<FlowCallFact>,
    call: &FlowCallFact,
    exit: &FlowExitFact,
) -> bool {
    if call.statement_index != exit.statement_index
        || exit.state_symbol != source.state_symbol
        || exit.machine_symbol != source.machine_symbol
    {
        return false;
    }
    let Some((machine, state)) =
        program.state_flow_site(source.machine_symbol, source.state_symbol)
    else {
        return false;
    };
    if exit.transition_target.is_valid() {
        if validation::transition_call_target(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        ) != Some(exit.transition_target)
        {
            return false;
        }
        match program
            .statement_table
            .transition_target(exit.transition_target)
        {
            TransitionTargetNode::Named { path, .. } => {
                !call.authored_expression.is_valid()
                    && validation::call_target_state(program, path.symbol).map(|state| state.symbol)
                        == validation::call_target_state(program, call.target_symbol)
                            .map(|state| state.symbol)
                    && validation::call_target_state(program, path.symbol).is_some()
            }
            TransitionTargetNode::Value(expression) => {
                *expression == call.authored_expression
                    && facts
                        .flow
                        .control
                        .tail_transfers
                        .iter()
                        .any(|(_, transfer)| transfer.call == handle)
            }
            TransitionTargetNode::Terminal => false,
        }
    } else {
        matches!(program.statement_table.statements(state.statement_nodes).get(call.statement_index),
            Some(StatementNode::Expression(expression)) if *expression == call.authored_expression)
            && facts
                .flow
                .control
                .tail_transfers
                .iter()
                .any(|(_, transfer)| transfer.call == handle)
    }
}

fn requires_reads(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    source: &FlowStateFact,
    call: &FlowCallFact,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let target = validation::call_target_state(program, call.target_symbol)?;
    let (owner, _) = validation::find_state(program, target.symbol)?;
    let parameters = program.state_parameters(target);
    let predicates = crate::execution::terminal_unit::state_contracts::arrival_predicates(
        program, facts, owner, target,
    )?;
    let mut reads = Vec::new();
    for predicate in predicates {
        let bound = crate::facts::boolean_predicates::bind_parameters(
            program,
            owner.symbol,
            target.symbol,
            parameters,
            predicate,
        )?;
        for observation in &bound.bindings {
            let actual = super::contracts::call_values::observation_actual(
                program,
                source,
                call,
                observation,
            )?;
            // Primitive value formals receive the completed scalar, including
            // a mutable field read earlier in the actual list. Their source
            // storage is not the receiving value's later observation.
            if matches!(observation.operand, Scalar::Parameter { .. })
                && validation::reference_shell(program, actual.parameter.type_reference).is_none()
                && program
                    .primitive_type_reference(actual.parameter.type_reference)
                    .is_some()
            {
                continue;
            }
            let place = crate::semantic::facts::projected_argument_place(
                program,
                source.state_symbol,
                call.statement_index,
                actual.argument?,
                actual.parameter.type_reference,
                &actual.path,
            )?;
            let place = crate::flow::rebase_exact_local_place(
                program,
                source.state_symbol,
                call.statement_index,
                place,
                frames,
            )?;
            if !reads.contains(&place) {
                reads.push(place);
            }
        }
    }
    Some(reads)
}

fn current_scalar_reads(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    statement: usize,
    expression: &Scalar,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let mut reads = Vec::new();
    let mut expression = expression.clone();
    crate::facts::boolean_predicates::map_scalar(&mut expression, &mut |operand| {
        let place = match operand {
            Scalar::Parameter { .. } | Scalar::Local { .. } => return Some(()),
            Scalar::StorageRead { symbol, .. } => {
                crate::flow::canonical_place_from_symbol(*symbol)?
            }
            Scalar::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => {
                let (root, segments, _, _) = crate::values::resolve_structural_parameter_path(
                    program,
                    program.state_parameters(state),
                    *parameter_position,
                    path,
                )?;
                CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments,
                }
            }
            _ => return None,
        };
        let place =
            crate::flow::rebase_exact_local_place(program, state.symbol, statement, place, frames)?;
        if !reads.contains(&place) {
            reads.push(place);
        }
        Some(())
    })?;
    Some(reads)
}

fn current_proof_reads(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    statement: usize,
    term: &checked_trees::CheckedProofTerm,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let mut pending = vec![term];
    let mut reads = Vec::new();
    while let Some(term) = pending.pop() {
        match term {
            checked_trees::CheckedProofTerm::Construction { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.term))
            }
            checked_trees::CheckedProofTerm::Scalar(expression) => {
                for read in current_scalar_reads(program, state, statement, expression, frames)? {
                    if !reads.contains(&read) {
                        reads.push(read);
                    }
                }
            }
            checked_trees::CheckedProofTerm::Formal { parameter_symbol } => {
                let parameter = program.state_parameters(state).iter().find(|parameter| {
                    parameter.symbol == *parameter_symbol && parameter.relevance.is_erased()
                })?;
                // Erasure alone is not immutability. The existing complete
                // BorrowFree carrier judgment includes erased fields and
                // rejects references, open substitutions and unknown shape.
                if !validation::has_contents(
                    program,
                    parameter.type_reference,
                    &[],
                    validation::ContentsRequirement::BorrowFree,
                ) {
                    return None;
                }
            }
        }
    }
    Some(reads)
}
