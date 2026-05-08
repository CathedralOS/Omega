use crate::plan::NativePlan;
use crate::state_analysis::StateAnalysisContext;
use omega_core::arena::Arena;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::Program;
use omega_typed_program::expression::Expression;
use omega_typed_program::machine::Machine;
use omega_typed_program::statement::Statement;
use std::sync::Arc;

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
    let workers = WorkerPool::with_available_parallelism();

    build_state_storage_plan_with_workers(
        Arc::new(program.clone()),
        Arc::new(StateAnalysisContext::from_native_plan(native_plan)),
        workers.handle(),
    )
}

pub fn build_state_storage_plan_with_workers(
    program: Arc<Program>,
    context: Arc<StateAnalysisContext>,
    workers: WorkerPoolHandle,
) -> StateStoragePlan {
    if program.machines.is_empty() {
        return StateStoragePlan::default();
    }

    let machine_count = program.machines.len();
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines
            .get(index)
            .expect("state-storage worker index should be in range");

        build_machine_state_storage_plan(&program, &context, machine)
    });

    let mut plan = StateStoragePlan::default();

    for machine_plan in machine_plans {
        plan.locals
            .insert_many(machine_plan.locals.iter().map(|(_, local)| local.clone()));
        plan.mutations.insert_many(
            machine_plan
                .mutations
                .iter()
                .map(|(_, mutation)| mutation.clone()),
        );
    }

    plan
}

fn build_machine_state_storage_plan(
    program: &Program,
    context: &StateAnalysisContext,
    machine: &Machine,
) -> StateStoragePlan {
    let mut plan = StateStoragePlan::default();

    for state in &machine.states {
        let required = context.state_is_required(&machine.name, &state.name);

        for (statement_index, statement) in state.statements.iter().enumerate() {
            match statement {
                Statement::LocalData(local_data) => {
                    plan.locals.insert(StateLocalStorage {
                        machine: machine.name.to_string(),
                        state: state.name.to_string(),
                        statement_index,
                        name: local_data.name.to_string(),
                        type_name: local_data.type_reference.display_name(),
                        required,
                    });
                }
                Statement::Assignment(assignment) => {
                    let mutation_kind =
                        mutation_kind(program, &machine.name, state, &assignment.target);
                    plan.mutations.insert(StateMutation {
                        machine: machine.name.to_string(),
                        state: state.name.to_string(),
                        statement_index,
                        target: assignment.target.clone(),
                        value: assignment.value.clone(),
                        mutation_kind,
                        lowering: mutation_lowering(
                            context,
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

    plan
}

fn mutation_lowering(
    context: &StateAnalysisContext,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
    mutation_kind: StateMutationKind,
) -> StateMutationLowering {
    if context.state_mutation_is_already_lowered(machine_name, state_name, statement_index) {
        return StateMutationLowering::AlreadyLowered;
    }

    match mutation_kind {
        StateMutationKind::Local => StateMutationLowering::NeedsLocalWrite,
        StateMutationKind::MachineOwned => StateMutationLowering::NeedsMachineOwnedWrite,
        StateMutationKind::ParameterOrAlias => StateMutationLowering::NeedsAliasWrite,
        StateMutationKind::Unknown => StateMutationLowering::Unknown,
    }
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
        Expression::Name(path) => path.first().map(|name| name.as_str()),
        Expression::Indexed(indexed) => root_place_name(&indexed.collection),
        Expression::Mutable(expression) => root_place_name(expression),
        _ => None,
    }
}
