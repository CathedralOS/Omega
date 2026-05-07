use crate::control_flow::OperationKind;
use crate::plan::NativePlan;
use omega_core::arena::Arena;
use omega_typed_program::Program;
use omega_typed_program::expression::Expression;
use omega_typed_program::statement::Statement;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateStoragePlan {
    pub locals: Arena<StateLocalStorage>,
    pub mutations: Arena<StateMutation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateLocalStorage {
    pub machine: String,
    pub state: String,
    pub statement_index: usize,
    pub name: String,
    pub type_name: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMutation {
    pub machine: String,
    pub state: String,
    pub statement_index: usize,
    pub target: Expression,
    pub value: Expression,
    pub mutation_kind: StateMutationKind,
    pub lowering: StateMutationLowering,
    pub required: bool,
}

impl Default for StateMutation {
    fn default() -> Self {
        Self {
            machine: String::new(),
            state: String::new(),
            statement_index: 0,
            target: Expression::Integer(0),
            value: Expression::Integer(0),
            mutation_kind: StateMutationKind::Unknown,
            lowering: StateMutationLowering::Unknown,
            required: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateMutationKind {
    Local,
    MachineOwned,
    ParameterOrAlias,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateMutationLowering {
    AlreadyLowered,
    NeedsLocalWrite,
    NeedsMachineOwnedWrite,
    NeedsAliasWrite,
    #[default]
    Unknown,
}

pub fn build_state_storage_plan(program: &Program, native_plan: &NativePlan) -> StateStoragePlan {
    let mut plan = StateStoragePlan::default();

    for machine in &program.machines {
        for state in &machine.states {
            let required = state_is_required(native_plan, &machine.name, &state.name);

            for (statement_index, statement) in state.statements.iter().enumerate() {
                match statement {
                    Statement::LocalData(local_data) => {
                        plan.locals.insert(StateLocalStorage {
                            machine: machine.name.clone(),
                            state: state.name.clone(),
                            statement_index,
                            name: local_data.name.clone(),
                            type_name: local_data.type_reference.display_name(),
                            required,
                        });
                    }
                    Statement::Assignment(assignment) => {
                        let mutation_kind =
                            mutation_kind(program, &machine.name, state, &assignment.target);
                        plan.mutations.insert(StateMutation {
                            machine: machine.name.clone(),
                            state: state.name.clone(),
                            statement_index,
                            target: assignment.target.clone(),
                            value: assignment.value.clone(),
                            mutation_kind,
                            lowering: mutation_lowering(
                                native_plan,
                                &machine.name,
                                &state.name,
                                statement_index,
                                mutation_kind,
                            ),
                            required,
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    plan
}

fn mutation_lowering(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
    mutation_kind: StateMutationKind,
) -> StateMutationLowering {
    if state_mutation_is_already_lowered(native_plan, machine_name, state_name, statement_index) {
        return StateMutationLowering::AlreadyLowered;
    }

    match mutation_kind {
        StateMutationKind::Local => StateMutationLowering::NeedsLocalWrite,
        StateMutationKind::MachineOwned => StateMutationLowering::NeedsMachineOwnedWrite,
        StateMutationKind::ParameterOrAlias => StateMutationLowering::NeedsAliasWrite,
        StateMutationKind::Unknown => StateMutationLowering::Unknown,
    }
}

fn state_mutation_is_already_lowered(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    let Some(machine) = native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .map(|(_, machine)| machine)
    else {
        return false;
    };
    let Some(state) = native_plan
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.name == state_name))
    else {
        return false;
    };
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return false;
    };

    operations.iter().any(|operation| {
        operation.statement_index == statement_index
            && matches!(
                operation.kind,
                OperationKind::ConstantIntegerAssignment | OperationKind::StaticAssignment { .. }
            )
    })
}

fn mutation_kind(
    program: &Program,
    machine_name: &str,
    state: &omega_typed_program::state::State,
    target: &Expression,
) -> StateMutationKind {
    let Some(root_name) = root_place_name(target) else {
        return StateMutationKind::Unknown;
    };
    let Some(machine) = program
        .machines
        .iter()
        .find(|machine| machine.name == machine_name)
    else {
        return StateMutationKind::Unknown;
    };

    if machine
        .owned_data
        .iter()
        .any(|owned_data| owned_data.name == root_name)
    {
        return StateMutationKind::MachineOwned;
    }

    if state
        .parameters
        .iter()
        .any(|parameter| parameter.name == root_name)
    {
        return StateMutationKind::ParameterOrAlias;
    }

    if state.statements.iter().any(|statement| {
        matches!(statement, Statement::LocalData(local_data) if local_data.name == root_name)
    }) {
        return StateMutationKind::Local;
    }

    StateMutationKind::Unknown
}

fn root_place_name(expression: &Expression) -> Option<&str> {
    match expression {
        Expression::Name(path) => path.first().map(String::as_str),
        Expression::Indexed(indexed) => root_place_name(&indexed.collection),
        Expression::Mutable(expression) => root_place_name(expression),
        _ => None,
    }
}

fn state_is_required(native_plan: &NativePlan, machine_name: &str, state_name: &str) -> bool {
    native_plan
        .runtime_flow
        .states
        .iter()
        .any(|(_, state)| state.machine == machine_name && state.state == state_name)
        || native_plan.state_calls.calls.iter().any(|(_, state_call)| {
            state_call.required
                && ((state_call.source_machine == machine_name
                    && state_call.source_state == state_name)
                    || (state_call.target_machine == machine_name
                        && state_call.target_state == state_name))
        })
}
