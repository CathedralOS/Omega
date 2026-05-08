use crate::plan::NativePlan;
use crate::state_analysis::StateAnalysisContext;
use omega_core::arena::Arena;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::Program;
use omega_typed_program::expression::Expression;
use omega_typed_program::machine::Machine;
use omega_typed_program::statement::{Statement, TransitionGuard};
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateValuePlan {
    pub values: Arena<StateValueUse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateValueUse {
    pub machine: String,
    pub state: String,
    pub statement_index: usize,
    pub role: StateValueRole,
    pub kind: StateValueKind,
    pub expression: Expression,
    pub required: bool,
}

impl Default for StateValueUse {
    fn default() -> Self {
        Self {
            machine: String::new(),
            state: String::new(),
            statement_index: 0,
            role: StateValueRole::AssignmentValue,
            kind: StateValueKind::Literal,
            expression: Expression::Integer(0),
            required: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateValueRole {
    AssignmentTarget,
    #[default]
    AssignmentValue,
    CallArgument,
    TransitionArgument,
    TransitionGuard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateValueKind {
    Array,
    Binary,
    Literal,
    MutablePlace,
    Place,
    Struct,
    #[default]
    Unknown,
}

pub fn build_state_value_plan(program: &Program, native_plan: &NativePlan) -> StateValuePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_value_plan_with_workers(
        Arc::new(program.clone()),
        Arc::new(StateAnalysisContext::from_native_plan(native_plan)),
        workers.handle(),
    )
}

pub fn build_state_value_plan_with_workers(
    program: Arc<Program>,
    context: Arc<StateAnalysisContext>,
    workers: WorkerPoolHandle,
) -> StateValuePlan {
    if program.machines.is_empty() {
        return StateValuePlan::default();
    }

    let machine_count = program.machines.len();
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines
            .get(index)
            .expect("state-value worker index should be in range");

        build_machine_state_value_plan(&context, machine)
    });

    let mut plan = StateValuePlan::default();

    for machine_plan in machine_plans {
        plan.values
            .insert_many(machine_plan.values.iter().map(|(_, value)| value.clone()));
    }

    plan
}

fn build_machine_state_value_plan(
    context: &StateAnalysisContext,
    machine: &Machine,
) -> StateValuePlan {
    let mut plan = StateValuePlan::default();

    for state in &machine.states {
        let required = context.state_is_required(&machine.name, &state.name);

        for (statement_index, statement) in state.statements.iter().enumerate() {
            match statement {
                Statement::Assignment(assignment) => {
                    push_value(
                        &mut plan,
                        &machine.name,
                        &state.name,
                        statement_index,
                        StateValueRole::AssignmentTarget,
                        &assignment.target,
                        required,
                    );
                    push_value(
                        &mut plan,
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
                        &machine.name,
                        &state.name,
                        statement_index,
                        &transition.target,
                        required,
                    );

                    if let Some(continuation) = &transition.continuation {
                        collect_transition_arguments(
                            &mut plan,
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
    machine: &str,
    state: &str,
    statement_index: usize,
    target: &omega_typed_program::statement::TransitionTarget,
    required: bool,
) {
    let omega_typed_program::statement::TransitionTarget::Named { arguments, .. } = target else {
        return;
    };

    for argument in arguments {
        push_value(
            plan,
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
    machine: &str,
    state: &str,
    statement_index: usize,
    role: StateValueRole,
    expression: &Expression,
    required: bool,
) {
    plan.values.insert(StateValueUse {
        machine: machine.to_owned(),
        state: state.to_owned(),
        statement_index,
        role,
        kind: value_kind(expression),
        expression: expression.clone(),
        required,
    });
}

fn value_kind(expression: &Expression) -> StateValueKind {
    match expression {
        Expression::ArrayLiteral(_) => StateValueKind::Array,
        Expression::Binary(_) => StateValueKind::Binary,
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => StateValueKind::Literal,
        Expression::Indexed(_) | Expression::Name(_) => StateValueKind::Place,
        Expression::Mutable(_) => StateValueKind::MutablePlace,
        Expression::StructLiteral(_) => StateValueKind::Struct,
    }
}
