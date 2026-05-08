use super::classify::value_kind;
use super::{StateValuePlan, StateValueRole, StateValueUse};
use crate::control_flow::StateKey;
use crate::state_analysis::StateAnalysisContext;
use omega_typed_program::expression::Expression;
use omega_typed_program::machine::Machine;
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::{Statement, TransitionGuard, TransitionTarget};

pub(super) fn build_machine_state_value_plan(
    context: &StateAnalysisContext,
    machine: &Machine,
) -> StateValuePlan {
    let mut plan = StateValuePlan::default();

    for state in &machine.states {
        let required = context.state_is_required(&machine.name, &state.name);
        let source_key = context
            .state_key(&machine.name, &state.name)
            .unwrap_or_default();

        for (statement_index, statement) in state.statements.iter().enumerate() {
            match statement {
                Statement::Assignment(assignment) => {
                    push_value(
                        &mut plan,
                        source_key,
                        &machine.name,
                        &state.name,
                        statement_index,
                        StateValueRole::AssignmentTarget,
                        &assignment.target,
                        required,
                    );
                    push_value(
                        &mut plan,
                        source_key,
                        &machine.name,
                        &state.name,
                        statement_index,
                        StateValueRole::AssignmentValue,
                        &assignment.value,
                        required,
                    );
                }
                Statement::Call(call) => {
                    for argument in &call.arguments {
                        push_value(
                            &mut plan,
                            source_key,
                            &machine.name,
                            &state.name,
                            statement_index,
                            StateValueRole::CallArgument,
                            argument,
                            required,
                        );
                    }
                }
                Statement::Transition(transition) => {
                    if let TransitionGuard::When(expression) = &transition.guard {
                        push_value(
                            &mut plan,
                            source_key,
                            &machine.name,
                            &state.name,
                            statement_index,
                            StateValueRole::TransitionGuard,
                            expression,
                            required,
                        );
                    }

                    collect_transition_arguments(
                        &mut plan,
                        source_key,
                        &machine.name,
                        &state.name,
                        statement_index,
                        &transition.target,
                        required,
                    );

                    if let Some(continuation) = &transition.continuation {
                        collect_transition_arguments(
                            &mut plan,
                            source_key,
                            &machine.name,
                            &state.name,
                            statement_index,
                            continuation,
                            required,
                        );
                    }
                }
                Statement::Expression(expression) => {
                    push_value(
                        &mut plan,
                        source_key,
                        &machine.name,
                        &state.name,
                        statement_index,
                        StateValueRole::AssignmentValue,
                        expression,
                        required,
                    );
                }
                Statement::LocalData(_) => {}
            }
        }
    }

    plan
}

fn collect_transition_arguments(
    plan: &mut StateValuePlan,
    source_key: StateKey,
    machine: &ProgramName,
    state: &ProgramName,
    statement_index: usize,
    target: &TransitionTarget,
    required: bool,
) {
    let TransitionTarget::Named { arguments, .. } = target else {
        return;
    };

    for argument in arguments {
        push_value(
            plan,
            source_key,
            machine,
            state,
            statement_index,
            StateValueRole::TransitionArgument,
            argument,
            required,
        );
    }
}

fn push_value(
    plan: &mut StateValuePlan,
    source_key: StateKey,
    machine: &ProgramName,
    state: &ProgramName,
    statement_index: usize,
    role: StateValueRole,
    expression: &Expression,
    required: bool,
) {
    plan.values.insert(StateValueUse {
        source_key,
        machine: machine.clone(),
        state: state.clone(),
        statement_index,
        role,
        kind: value_kind(expression),
        expression: expression.clone(),
        required,
    });
}
