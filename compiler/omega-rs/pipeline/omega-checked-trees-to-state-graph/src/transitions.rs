mod targets;

use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::ExpressionHandle;
use psi_checked_trees::machine::Machine;
use psi_checked_trees::state::State;
use psi_checked_trees::statement::{StatementNode, TableCall, TableTransition};
use psi_diagnostics::Diagnostic;

use crate::runtime_expressions::copy_runtime_expression;
use crate::segments::{
    SegmentTransition, StateSegment, copy_statement_expression_span,
    table_transition_guard_expression,
};
use crate::transitions::targets::{next_segment_target, plan_call_target, plan_transition_target};
use omega_state_graph::{
    PlannedTransitionTarget, StateGraph, StateKey, TransitionEdge, TransitionExpressionRefs,
};

pub(super) fn plan_transition(
    source_key: StateKey,
    segments: &[StateSegment],
    transition: &SegmentTransition,
    program: &CheckedTrees,
    state_graph: &mut StateGraph,
) -> Result<TransitionEdge, Diagnostic> {
    match transition {
        SegmentTransition::Tree {
            statement_index,
            table,
        } => {
            validate_tree_occurrence(program, source_key, *statement_index, *table)?;
            let target_arguments =
                table_transition_target_arguments(table.target, program, state_graph);
            let target_value = table_transition_target_value(table.target, program, state_graph);
            let continuation_arguments = table
                .continuation
                .is_valid()
                .then(|| {
                    table_transition_target_arguments(table.continuation, program, state_graph)
                })
                .unwrap_or_default();
            let continuation_value = table
                .continuation
                .is_valid()
                .then(|| table_transition_target_value(table.continuation, program, state_graph))
                .unwrap_or_else(ExpressionHandle::invalid);
            let guard_expression = table_transition_guard_expression(*table);
            let guard_expression = guard_expression
                .is_valid()
                .then(|| copy_runtime_expression(state_graph, program, guard_expression))
                .unwrap_or_else(ExpressionHandle::invalid);

            Ok(TransitionEdge {
                statement_index: *statement_index,
                target: plan_transition_target(source_key, segments, table.target, program)?,
                continuation: if table.continuation.is_valid() {
                    plan_transition_target(source_key, segments, table.continuation, program)?
                } else {
                    PlannedTransitionTarget::None
                },
                expressions: TransitionExpressionRefs {
                    target_arguments,
                    target_value,
                    continuation_arguments,
                    continuation_value,
                    guard: guard_expression,
                },
            })
        }
        SegmentTransition::ReturnExpression {
            statement_index,
            expression,
        } => {
            validate_return_occurrence(program, source_key, *statement_index, *expression)?;
            Ok(TransitionEdge {
                statement_index: *statement_index,
                target: PlannedTransitionTarget::Terminal,
                continuation: PlannedTransitionTarget::None,
                expressions: TransitionExpressionRefs {
                    target_arguments: psi_arena::HandleSpan::empty(),
                    target_value: copy_runtime_expression(state_graph, program, *expression),
                    continuation_arguments: psi_arena::HandleSpan::empty(),
                    continuation_value: ExpressionHandle::invalid(),
                    guard: ExpressionHandle::invalid(),
                },
            })
        }
        SegmentTransition::BranchCall {
            statement_index,
            has_continuation_segment,
        } => {
            let table = exact_branch_call_occurrence(
                program,
                source_key,
                *statement_index,
                *has_continuation_segment,
            )?;
            Ok(TransitionEdge {
                statement_index: *statement_index,
                target: plan_call_target(source_key, segments, table, program)?,
                continuation: if *has_continuation_segment {
                    next_segment_target(source_key, segments)?
                } else {
                    PlannedTransitionTarget::None
                },
                expressions: TransitionExpressionRefs {
                    target_arguments: copy_statement_expression_span(
                        state_graph,
                        program,
                        table.arguments,
                    ),
                    target_value: ExpressionHandle::invalid(),
                    continuation_arguments: psi_arena::HandleSpan::empty(),
                    continuation_value: ExpressionHandle::invalid(),
                    guard: ExpressionHandle::invalid(),
                },
            })
        }
    }
}

fn validate_tree_occurrence(
    program: &CheckedTrees,
    source_key: StateKey,
    statement_index: usize,
    table: TableTransition,
) -> Result<(), Diagnostic> {
    let (_, _, statement) = exact_source_statement(program, source_key, statement_index)?;
    match statement {
        StatementNode::Transition(stored) if *stored == table => Ok(()),
        StatementNode::Transition(_) => Err(Diagnostic::error(
            "state-graph transition carrier disagrees with its exact typed statement",
        )),
        _ => Err(Diagnostic::error(
            "state-graph transition carrier did not reference a transition statement",
        )),
    }
}

fn validate_return_occurrence(
    program: &CheckedTrees,
    source_key: StateKey,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Result<(), Diagnostic> {
    let (state, statements, statement) =
        exact_source_statement(program, source_key, statement_index)?;
    if !state.return_type.is_valid() {
        return Err(Diagnostic::error(
            "state-graph return carrier belongs to a state without a return type",
        ));
    }
    if statement_index.checked_add(1) != Some(statements.len()) {
        return Err(Diagnostic::error(
            "state-graph return carrier did not reference the terminal statement",
        ));
    }
    match statement {
        StatementNode::Expression(stored) if *stored == expression => Ok(()),
        StatementNode::Expression(_) => Err(Diagnostic::error(
            "state-graph return carrier disagrees with its exact typed expression",
        )),
        _ => Err(Diagnostic::error(
            "state-graph return carrier did not reference an expression statement",
        )),
    }
}

fn exact_branch_call_occurrence(
    program: &CheckedTrees,
    source_key: StateKey,
    statement_index: usize,
    has_continuation_segment: bool,
) -> Result<&TableCall, Diagnostic> {
    let (_, statements, statement) = exact_source_statement(program, source_key, statement_index)?;
    let StatementNode::Call(call) = statement else {
        return Err(Diagnostic::error(
            "internal branch-call segment did not reference a call statement",
        ));
    };
    let expected_continuation = statement_index
        .checked_add(1)
        .is_some_and(|next| next < statements.len());
    if has_continuation_segment != expected_continuation {
        return Err(Diagnostic::error(
            "state-graph branch-call continuation presence disagrees with its exact typed occurrence",
        ));
    }

    let mut states = program
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| state.state_symbol == source_key.state);
    let flow_state = states.next().ok_or_else(|| {
        Diagnostic::error("state-graph branch-call exact FlowState fact is missing")
    })?;
    if states.next().is_some() {
        return Err(Diagnostic::error(
            "state-graph branch-call exact FlowState fact is duplicated",
        ));
    }
    if flow_state.machine_symbol != source_key.machine {
        return Err(Diagnostic::error(
            "state-graph branch-call exact FlowState fact belongs to another machine",
        ));
    }
    let flow_calls = program
        .facts
        .flow
        .control
        .calls
        .span_or_empty(flow_state.calls);
    if !flow_state.calls.is_empty() && flow_calls.is_empty() {
        return Err(Diagnostic::error(
            "state-graph branch-call FlowCall span is invalid",
        ));
    }
    let mut calls = flow_calls.iter().filter(|flow_call| {
        flow_call.statement_index == statement_index && flow_call.call_ordinal == 0
    });
    let flow_call = calls.next().ok_or_else(|| {
        Diagnostic::error("state-graph branch-call exact FlowCall fact is missing")
    })?;
    if calls.next().is_some() {
        return Err(Diagnostic::error(
            "state-graph branch-call exact FlowCall fact is duplicated",
        ));
    }
    let has_receiver = !program
        .statement_table
        .name_path_members(call.receiver)
        .is_empty();
    if flow_call.receiver_symbol != call.receiver_symbol
        || flow_call.target_symbol != call.target_symbol
        || flow_call.has_receiver != has_receiver
    {
        return Err(Diagnostic::error(
            "state-graph branch-call exact FlowCall coordinates disagree with the typed call",
        ));
    }
    Ok(call)
}

fn exact_source_statement(
    program: &CheckedTrees,
    source_key: StateKey,
    statement_index: usize,
) -> Result<(&State, &[StatementNode], &StatementNode), Diagnostic> {
    let machine = exact_source_machine(program, source_key)?;
    let state = exact_source_state(program, machine, source_key)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let statement = statements.get(statement_index).ok_or_else(|| {
        Diagnostic::error("state-graph transition statement coordinate is out of range")
    })?;
    Ok((state, statements, statement))
}

fn exact_source_machine(
    program: &CheckedTrees,
    source_key: StateKey,
) -> Result<&Machine, Diagnostic> {
    let mut machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == source_key.machine);
    let machine = machines
        .next()
        .ok_or_else(|| Diagnostic::error("exact transition source machine is missing"))?;
    if machines.next().is_some() {
        return Err(Diagnostic::error(
            "exact transition source machine is duplicated",
        ));
    }
    Ok(machine)
}

fn exact_source_state<'program>(
    program: &'program CheckedTrees,
    machine: &Machine,
    source_key: StateKey,
) -> Result<&'program State, Diagnostic> {
    let mut states = program
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == source_key.state);
    let state = states.next();
    if states.next().is_some() {
        return Err(Diagnostic::error(
            "exact transition source state is duplicated within its machine",
        ));
    }
    if let Some(state) = state {
        if program.machines().iter().any(|candidate| {
            candidate.symbol != machine.symbol
                && program
                    .machine_states(candidate)
                    .iter()
                    .any(|candidate_state| candidate_state.symbol == source_key.state)
        }) {
            return Err(Diagnostic::error(
                "exact transition source state belongs to more than one machine",
            ));
        }
        return Ok(state);
    }
    let cross_owned = program.machines().iter().any(|candidate| {
        candidate.symbol != machine.symbol
            && program
                .machine_states(candidate)
                .iter()
                .any(|state| state.symbol == source_key.state)
    });
    Err(Diagnostic::error(if cross_owned {
        "exact transition source state belongs to another machine"
    } else {
        "exact transition source state is missing"
    }))
}

fn table_transition_target_arguments(
    target: psi_checked_trees::statement::TransitionTargetHandle,
    program: &CheckedTrees,
    state_graph: &mut StateGraph,
) -> psi_arena::HandleSpan<psi_checked_trees::expression::ExpressionHandle> {
    if !target.is_valid() {
        return psi_arena::HandleSpan::empty();
    }

    match program.statement_table.transition_target(target) {
        psi_checked_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            copy_statement_expression_span(state_graph, program, *arguments)
        }
        psi_checked_trees::statement::TransitionTargetNode::SelfTarget
        | psi_checked_trees::statement::TransitionTargetNode::Terminal
        | psi_checked_trees::statement::TransitionTargetNode::Value(_) => {
            psi_arena::HandleSpan::empty()
        }
    }
}

fn table_transition_target_value(
    target: psi_checked_trees::statement::TransitionTargetHandle,
    program: &CheckedTrees,
    state_graph: &mut StateGraph,
) -> psi_checked_trees::expression::ExpressionHandle {
    if !target.is_valid() {
        return psi_checked_trees::expression::ExpressionHandle::invalid();
    }

    match program.statement_table.transition_target(target) {
        psi_checked_trees::statement::TransitionTargetNode::Value(expression) => {
            copy_runtime_expression(state_graph, program, *expression)
        }
        _ => psi_checked_trees::expression::ExpressionHandle::invalid(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_arena::{Handle, HandleSpan};
    use psi_checked_trees::expression::ExpressionNode;
    use psi_checked_trees::name::Identifier;
    use psi_checked_trees::statement::TransitionGuardNode;
    use psi_checked_trees::types::TypeReferenceNode;
    use psi_checked_trees::{FlowCallFact, FlowStateFact};
    use psi_symbols::SymbolHandle;

    const MACHINE: u32 = 1;
    const STATE: u32 = 2;
    const TARGET: u32 = 3;

    struct OccurrenceFixture {
        program: CheckedTrees,
        source_key: StateKey,
        tree_index: usize,
        call_index: usize,
        return_index: usize,
        tree: TableTransition,
        expression: ExpressionHandle,
    }

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn occurrence_fixture() -> OccurrenceFixture {
        let mut program = CheckedTrees::default();
        let expression = program
            .typed
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let return_type = program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Unit);
        let tree = TableTransition::default();
        let call = TableCall {
            receiver_symbol: symbol(MACHINE),
            target_symbol: symbol(TARGET),
            target: Identifier::generated("next"),
            ..Default::default()
        };
        let mut statements = HandleSpan::empty();
        program
            .typed
            .statement_table
            .push_statement(&mut statements, StatementNode::Transition(tree));
        program
            .typed
            .statement_table
            .push_statement(&mut statements, StatementNode::Call(call.clone()));
        program
            .typed
            .statement_table
            .push_statement(&mut statements, StatementNode::Expression(expression));

        let mut machine = Machine {
            symbol: symbol(MACHINE),
            name: Identifier::generated("Root::run"),
            attached_data: Some(Identifier::generated("Root")),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: symbol(STATE),
                name: Identifier::generated("run"),
                return_type,
                statement_nodes: statements,
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);

        let mut calls = HandleSpan::empty();
        program.facts.flow.control.calls.append_to_span(
            &mut calls,
            FlowCallFact {
                statement_index: 1,
                call_ordinal: 0,
                receiver_symbol: call.receiver_symbol,
                target_symbol: call.target_symbol,
                has_receiver: false,
                ..Default::default()
            },
        );
        program.facts.flow.control.states.append(FlowStateFact {
            machine_symbol: symbol(MACHINE),
            state_symbol: symbol(STATE),
            calls,
            ..Default::default()
        });

        OccurrenceFixture {
            program,
            source_key: StateKey {
                machine: symbol(MACHINE),
                state: symbol(STATE),
                segment_index: 0,
            },
            tree_index: 0,
            call_index: 1,
            return_index: 2,
            tree,
            expression,
        }
    }

    fn error_message<T: std::fmt::Debug>(result: Result<T, Diagnostic>) -> String {
        result
            .expect_err("invalid transition occurrence must fail closed")
            .message
    }

    fn flow_state_mut(program: &mut CheckedTrees) -> &mut FlowStateFact {
        let handle = program
            .facts
            .flow
            .control
            .states
            .iter()
            .next()
            .expect("flow state")
            .0;
        program.facts.flow.control.states.get_mut(handle)
    }

    fn flow_call_mut(program: &mut CheckedTrees) -> &mut FlowCallFact {
        let handle = program
            .facts
            .flow
            .control
            .calls
            .iter()
            .next()
            .expect("flow call")
            .0;
        program.facts.flow.control.calls.get_mut(handle)
    }

    #[test]
    fn exact_tree_return_and_branch_call_occurrences_are_accepted() {
        let fixture = occurrence_fixture();
        validate_tree_occurrence(
            &fixture.program,
            fixture.source_key,
            fixture.tree_index,
            fixture.tree,
        )
        .expect("exact transition occurrence");
        validate_return_occurrence(
            &fixture.program,
            fixture.source_key,
            fixture.return_index,
            fixture.expression,
        )
        .expect("exact return occurrence");
        let call = exact_branch_call_occurrence(
            &fixture.program,
            fixture.source_key,
            fixture.call_index,
            true,
        )
        .expect("exact branch-call occurrence");
        assert_eq!(call.target_symbol, symbol(TARGET));
    }

    #[test]
    fn tree_occurrence_rejects_range_kind_and_copied_payload_drift() {
        let fixture = occurrence_fixture();
        assert!(
            error_message(validate_tree_occurrence(
                &fixture.program,
                fixture.source_key,
                99,
                fixture.tree,
            ))
            .contains("out of range")
        );
        assert!(
            error_message(validate_tree_occurrence(
                &fixture.program,
                fixture.source_key,
                fixture.call_index,
                fixture.tree,
            ))
            .contains("did not reference a transition")
        );
        let drifted = TableTransition {
            guard: TransitionGuardNode::When(fixture.expression),
            ..fixture.tree
        };
        assert!(
            error_message(validate_tree_occurrence(
                &fixture.program,
                fixture.source_key,
                fixture.tree_index,
                drifted,
            ))
            .contains("disagrees with its exact typed statement")
        );
    }

    #[test]
    fn return_occurrence_rejects_type_terminal_kind_and_expression_drift() {
        let fixture = occurrence_fixture();
        assert!(
            error_message(validate_return_occurrence(
                &fixture.program,
                fixture.source_key,
                fixture.tree_index,
                fixture.expression,
            ))
            .contains("terminal statement")
        );
        assert!(
            error_message(validate_return_occurrence(
                &fixture.program,
                fixture.source_key,
                fixture.return_index,
                ExpressionHandle::invalid(),
            ))
            .contains("exact typed expression")
        );

        let mut no_return_type = occurrence_fixture();
        let machine = no_return_type.program.machines()[0].clone();
        no_return_type.program.typed.machine_states_mut(&machine)[0].return_type =
            Default::default();
        assert!(
            error_message(validate_return_occurrence(
                &no_return_type.program,
                no_return_type.source_key,
                no_return_type.return_index,
                no_return_type.expression,
            ))
            .contains("without a return type")
        );

        let mut wrong_kind = occurrence_fixture();
        let machine = wrong_kind.program.machines()[0].clone();
        let statement_nodes = wrong_kind.program.machine_states(&machine)[0].statement_nodes;
        let statements = wrong_kind
            .program
            .typed
            .statement_table
            .statements_mut(statement_nodes);
        statements[wrong_kind.return_index] = StatementNode::Call(TableCall::default());
        assert!(
            error_message(validate_return_occurrence(
                &wrong_kind.program,
                wrong_kind.source_key,
                wrong_kind.return_index,
                wrong_kind.expression,
            ))
            .contains("expression statement")
        );
    }

    #[test]
    fn source_machine_and_state_coordinates_must_be_exact() {
        let mut duplicate_machine = occurrence_fixture();
        let machine = duplicate_machine.program.machines()[0].clone();
        duplicate_machine.program.typed.push_machine(machine);
        assert!(
            error_message(validate_tree_occurrence(
                &duplicate_machine.program,
                duplicate_machine.source_key,
                duplicate_machine.tree_index,
                duplicate_machine.tree,
            ))
            .contains("source machine is duplicated")
        );

        let mut missing_state = occurrence_fixture();
        missing_state.source_key.state = symbol(99);
        assert!(
            error_message(validate_tree_occurrence(
                &missing_state.program,
                missing_state.source_key,
                missing_state.tree_index,
                missing_state.tree,
            ))
            .contains("source state is missing")
        );

        let mut cross_owned = occurrence_fixture();
        let mut foreign = Machine {
            symbol: symbol(77),
            name: Identifier::generated("Other::run"),
            ..Default::default()
        };
        cross_owned.program.typed.push_machine_state(
            &mut foreign,
            State {
                symbol: cross_owned.source_key.state,
                name: Identifier::generated("run"),
                ..Default::default()
            },
        );
        cross_owned.program.typed.push_machine(foreign);
        assert!(
            error_message(validate_tree_occurrence(
                &cross_owned.program,
                cross_owned.source_key,
                cross_owned.tree_index,
                cross_owned.tree,
            ))
            .contains("more than one machine")
        );
    }

    #[test]
    fn branch_call_rejects_continuation_and_flow_state_drift() {
        let fixture = occurrence_fixture();
        assert!(
            error_message(exact_branch_call_occurrence(
                &fixture.program,
                fixture.source_key,
                fixture.call_index,
                false,
            ))
            .contains("continuation presence")
        );

        let mut missing = occurrence_fixture();
        missing.program.facts.flow.control.states = Default::default();
        assert!(
            error_message(exact_branch_call_occurrence(
                &missing.program,
                missing.source_key,
                missing.call_index,
                true,
            ))
            .contains("FlowState fact is missing")
        );

        let mut duplicate = occurrence_fixture();
        let state = duplicate
            .program
            .facts
            .flow
            .control
            .states
            .iter()
            .next()
            .expect("flow state")
            .1
            .clone();
        duplicate.program.facts.flow.control.states.append(state);
        assert!(
            error_message(exact_branch_call_occurrence(
                &duplicate.program,
                duplicate.source_key,
                duplicate.call_index,
                true,
            ))
            .contains("FlowState fact is duplicated")
        );

        let mut cross_machine = occurrence_fixture();
        flow_state_mut(&mut cross_machine.program).machine_symbol = symbol(88);
        assert!(
            error_message(exact_branch_call_occurrence(
                &cross_machine.program,
                cross_machine.source_key,
                cross_machine.call_index,
                true,
            ))
            .contains("belongs to another machine")
        );
    }

    #[test]
    fn branch_call_rejects_invalid_missing_and_duplicate_flow_calls() {
        let mut invalid = occurrence_fixture();
        flow_state_mut(&mut invalid.program).calls =
            HandleSpan::from_parts(Handle::<FlowCallFact>::from_arena_index(999), 1);
        assert!(
            error_message(exact_branch_call_occurrence(
                &invalid.program,
                invalid.source_key,
                invalid.call_index,
                true,
            ))
            .contains("FlowCall span is invalid")
        );

        let mut missing = occurrence_fixture();
        flow_call_mut(&mut missing.program).call_ordinal = 1;
        assert!(
            error_message(exact_branch_call_occurrence(
                &missing.program,
                missing.source_key,
                missing.call_index,
                true,
            ))
            .contains("FlowCall fact is missing")
        );

        let mut duplicate = occurrence_fixture();
        let flow_call = duplicate
            .program
            .facts
            .flow
            .control
            .calls
            .iter()
            .next()
            .expect("flow call")
            .1
            .clone();
        duplicate.program.facts.flow.control.calls = Default::default();
        let calls = duplicate
            .program
            .facts
            .flow
            .control
            .calls
            .insert_many([flow_call.clone(), flow_call]);
        flow_state_mut(&mut duplicate.program).calls = calls;
        assert!(
            error_message(exact_branch_call_occurrence(
                &duplicate.program,
                duplicate.source_key,
                duplicate.call_index,
                true,
            ))
            .contains("FlowCall fact is duplicated")
        );
    }

    #[test]
    fn branch_call_rejects_receiver_target_and_presence_drift_independently() {
        let mutations: [fn(&mut FlowCallFact); 3] = [
            |call: &mut FlowCallFact| call.receiver_symbol = symbol(81),
            |call: &mut FlowCallFact| call.target_symbol = symbol(82),
            |call: &mut FlowCallFact| call.has_receiver = true,
        ];
        for mutate in mutations {
            let mut fixture = occurrence_fixture();
            mutate(flow_call_mut(&mut fixture.program));
            assert!(
                error_message(exact_branch_call_occurrence(
                    &fixture.program,
                    fixture.source_key,
                    fixture.call_index,
                    true,
                ))
                .contains("coordinates disagree")
            );
        }
    }
}
