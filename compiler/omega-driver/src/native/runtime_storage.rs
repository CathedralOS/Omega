use crate::ir::expression::Expression;
use crate::native::plan::NativePlan;
use crate::native::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::native::state_storage::{StateMutationKind, StateMutationLowering};
use omega_core::arena::Arena;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStoragePlan {
    pub frame_slots: Arena<RuntimeFrameSlot>,
    pub writes: Arena<RuntimeStorageWrite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeFrameSlot {
    pub dispatch_index: u32,
    pub source_machine: String,
    pub source_state: String,
    pub statement_index: usize,
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStorageWrite {
    pub dispatch_index: u32,
    pub source_machine: String,
    pub source_state: String,
    pub statement_index: usize,
    pub target: Expression,
    pub value: Expression,
    pub mutation_kind: StateMutationKind,
    pub lowering: StateMutationLowering,
}

impl Default for RuntimeStorageWrite {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_machine: String::new(),
            source_state: String::new(),
            statement_index: 0,
            target: Expression::Integer(0),
            value: Expression::Integer(0),
            mutation_kind: StateMutationKind::Unknown,
            lowering: StateMutationLowering::Unknown,
        }
    }
}

pub fn build_runtime_storage_plan(native_plan: &NativePlan) -> RuntimeStoragePlan {
    let mut plan = RuntimeStoragePlan::default();

    for (_, body) in native_plan.runtime_bodies.bodies.iter() {
        let Some(operations) = native_plan.runtime_bodies.operations.span(body.operations) else {
            continue;
        };

        for operation in operations {
            match &operation.kind {
                RuntimeDispatchBodyOperationKind::LocalStorage { name, type_name } => {
                    plan.frame_slots.insert(RuntimeFrameSlot {
                        dispatch_index: body.dispatch_index,
                        source_machine: operation.source_machine.clone(),
                        source_state: operation.source_state.clone(),
                        statement_index: operation.statement_index,
                        name: name.clone(),
                        type_name: type_name.clone(),
                    });
                }
                RuntimeDispatchBodyOperationKind::Mutation { lowering, .. }
                    if *lowering != StateMutationLowering::AlreadyLowered =>
                {
                    if let Some(mutation) = mutation_for_operation(
                        native_plan,
                        &operation.source_machine,
                        &operation.source_state,
                        operation.statement_index,
                    ) {
                        plan.writes.insert(RuntimeStorageWrite {
                            dispatch_index: body.dispatch_index,
                            source_machine: operation.source_machine.clone(),
                            source_state: operation.source_state.clone(),
                            statement_index: operation.statement_index,
                            target: mutation.target.clone(),
                            value: mutation.value.clone(),
                            mutation_kind: mutation.mutation_kind,
                            lowering: mutation.lowering,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    plan
}

fn mutation_for_operation<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan crate::native::state_storage::StateMutation> {
    native_plan
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.machine == machine_name
                && mutation.state == state_name
                && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}
