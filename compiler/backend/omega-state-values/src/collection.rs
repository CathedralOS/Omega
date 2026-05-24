use super::classify::value_kind;
use super::simplify::simplify_state_expression_for_role;
use super::{StateValuePlan, StateValueRole, StateValueUse};
use crate::StateValuePlanningContext;
use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTableCapacity};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};
use omega_control_flow::StateKey;

pub(super) fn build_machine_state_value_plan(
    program: &CheckedTrees,
    context: &StateValuePlanningContext,
    machine: &Machine,
) -> StateValuePlan {
    let value_capacity = estimated_machine_value_capacity(program, machine);
    let mut plan = StateValuePlan::with_capacities(
        value_capacity,
        ExpressionTableCapacity {
            expressions: value_capacity,
            ..ExpressionTableCapacity::default()
        },
    );

    for state in program.machine_states(machine) {
        let source_key = StateKey {
            machine: machine.symbol,
            state: state.symbol,
            segment_index: 0,
        };
        let required = context.state_is_required_by_key(source_key);
        let statements = program.statement_table.statements(state.statement_nodes);

        for (statement_index, statement) in statements.iter().enumerate() {
            match statement {
                StatementNode::Assignment(assignment) => {
                    push_value(
                        &mut plan,
                        program,
                        machine,
                        state,
                        source_key,
                        statement_index,
                        StateValueRole::AssignmentTarget,
                        assignment.target,
                        required,
                    );
                    push_value(
                        &mut plan,
                        program,
                        machine,
                        state,
                        source_key,
                        statement_index,
                        StateValueRole::AssignmentValue,
                        assignment.value,
                        required,
                    );
                }
                StatementNode::Call(call) => {
                    for argument in program.statement_table.expression_handles(call.arguments) {
                        push_value(
                            &mut plan,
                            program,
                            machine,
                            state,
                            source_key,
                            statement_index,
                            StateValueRole::CallArgument,
                            *argument,
                            required,
                        );
                    }
                }
                StatementNode::Transition(transition) => {
                    collect_transition_arguments(
                        &mut plan,
                        program,
                        machine,
                        state,
                        source_key,
                        statement_index,
                        transition.target,
                        required,
                    );

                    if transition.continuation.is_valid() {
                        collect_transition_arguments(
                            &mut plan,
                            program,
                            machine,
                            state,
                            source_key,
                            statement_index,
                            transition.continuation,
                            required,
                        );
                    }
                }
                StatementNode::Expression(expression) => {
                    push_value(
                        &mut plan,
                        program,
                        machine,
                        state,
                        source_key,
                        statement_index,
                        StateValueRole::AssignmentValue,
                        *expression,
                        required,
                    );
                }
                StatementNode::LocalData(_) => {}
            }
        }
    }

    plan
}

fn estimated_machine_value_capacity(program: &CheckedTrees, machine: &Machine) -> usize {
    program
        .machine_states(machine)
        .iter()
        .map(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .map(|statement| estimated_statement_value_capacity(program, statement))
                .sum::<usize>()
        })
        .sum()
}

fn estimated_statement_value_capacity(program: &CheckedTrees, statement: &StatementNode) -> usize {
    match statement {
        StatementNode::Assignment(_) => 2,
        StatementNode::Call(call) => program
            .statement_table
            .expression_handles(call.arguments)
            .len(),
        StatementNode::Transition(transition) => {
            estimated_transition_target_value_capacity(program, transition.target)
                + transition
                    .continuation
                    .is_valid()
                    .then(|| {
                        estimated_transition_target_value_capacity(program, transition.continuation)
                    })
                    .unwrap_or(0)
        }
        StatementNode::Expression(_) => 1,
        StatementNode::LocalData(_) => 0,
    }
}

fn estimated_transition_target_value_capacity(
    program: &CheckedTrees,
    target: TransitionTargetHandle,
) -> usize {
    let TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(target)
    else {
        return 0;
    };

    program.statement_table.expression_handles(*arguments).len()
}

fn collect_transition_arguments(
    plan: &mut StateValuePlan,
    program: &CheckedTrees,
    machine: &Machine,
    state: &omega_checked_trees::state::State,
    source_key: StateKey,
    statement_index: usize,
    target: TransitionTargetHandle,
    required: bool,
) {
    let TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(target)
    else {
        return;
    };

    for argument in program.statement_table.expression_handles(*arguments) {
        push_value(
            plan,
            program,
            machine,
            state,
            source_key,
            statement_index,
            StateValueRole::TransitionArgument,
            *argument,
            required,
        );
    }
}

fn push_value(
    plan: &mut StateValuePlan,
    program: &CheckedTrees,
    machine: &Machine,
    state: &omega_checked_trees::state::State,
    source_key: StateKey,
    statement_index: usize,
    role: StateValueRole,
    expression: ExpressionHandle,
    required: bool,
) {
    let expression = if role == StateValueRole::AssignmentTarget
        || program.expression_table.expression_is_literal(expression)
        || (program
            .expression_table
            .expression_is_direct_place_path(expression)
            && !state_has_initialized_locals_before(program, state, statement_index))
    {
        plan.expressions
            .copy_from(&program.expression_table, expression)
    } else {
        let simplified_expression = simplify_state_expression_for_role(
            program,
            machine,
            state,
            statement_index,
            role,
            &program.expression_table.to_tree(expression),
        );
        plan.expressions.insert_tree(&simplified_expression)
    };
    plan.values.insert(StateValueUse {
        source_key,
        statement_index,
        role,
        kind: value_kind(&plan.expressions, expression),
        expression,
        required,
    });
}

fn state_has_initialized_locals_before(
    program: &CheckedTrees,
    state: &omega_checked_trees::state::State,
    statement_index: usize,
) -> bool {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .any(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(local_data) if local_data.initial_value.is_valid()
            )
        })
}
