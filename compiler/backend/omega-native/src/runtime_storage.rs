use crate::layout::{LayoutPlan, TypeLayout};
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::{
    RuntimeDispatchBody, RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind,
};
use crate::state_storage::{
    StateMutation, StateMutationKind, StateMutationLowering, StateStoragePlan,
};
use crate::target::NativeTarget;
use omega_core::arena::Arena;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;
use omega_typed_program::types::PrimitiveType;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStoragePlan {
    pub frame_slots: Arena<RuntimeFrameSlot>,
    pub writes: Arena<RuntimeStorageWrite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeFrameSlot {
    pub dispatch_index: u32,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub statement_index: usize,
    pub name: ProgramName,
    pub type_name: String,
    pub byte_offset: usize,
    pub byte_size: usize,
    pub alignment: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStorageWrite {
    pub dispatch_index: u32,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
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
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            statement_index: 0,
            target: Expression::Integer(0),
            value: Expression::Integer(0),
            mutation_kind: StateMutationKind::Unknown,
            lowering: StateMutationLowering::Unknown,
        }
    }
}

pub fn build_runtime_storage_plan(native_plan: &NativePlan) -> RuntimeStoragePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_runtime_storage_plan_with_workers(
        Arc::new(RuntimeStorageContext::from_native_plan(native_plan)),
        runtime_storage_body_inputs(native_plan),
        workers.handle(),
    )
}

pub fn build_runtime_storage_plan_with_workers(
    context: Arc<RuntimeStorageContext>,
    body_inputs: Vec<RuntimeStorageBodyInput>,
    workers: WorkerPoolHandle,
) -> RuntimeStoragePlan {
    if body_inputs.is_empty() {
        return RuntimeStoragePlan::default();
    }

    let body_inputs = Arc::new(body_inputs);
    let body_count = body_inputs.len();
    let context_for_bodies = Arc::clone(&context);
    let body_plans = workers.map_ordered(body_count, move |index| {
        let body_input = body_inputs
            .get(index)
            .expect("runtime-storage worker index should be in range");

        build_runtime_storage_body_plan(&context_for_bodies, body_input)
    });

    let mut plan = RuntimeStoragePlan::default();

    for body_plan in body_plans {
        plan.frame_slots.insert_many(
            body_plan
                .frame_slots
                .iter()
                .map(|(_, frame_slot)| frame_slot.clone()),
        );
        plan.writes
            .insert_many(body_plan.writes.iter().map(|(_, write)| write.clone()));
    }

    plan
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStorageContext {
    pub layouts: LayoutPlan,
    pub state_storage: StateStoragePlan,
    pub target: NativeTarget,
}

impl RuntimeStorageContext {
    pub fn from_native_plan(native_plan: &NativePlan) -> Self {
        Self {
            layouts: native_plan.layouts.clone(),
            state_storage: native_plan.state_storage.clone(),
            target: native_plan.target,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStorageBodyInput {
    pub body: RuntimeDispatchBody,
    pub operations: Vec<RuntimeDispatchBodyOperation>,
}

pub fn runtime_storage_body_inputs(native_plan: &NativePlan) -> Vec<RuntimeStorageBodyInput> {
    native_plan
        .runtime_bodies
        .bodies
        .iter()
        .map(|(_, body)| RuntimeStorageBodyInput {
            body: body.clone(),
            operations: native_plan
                .runtime_bodies
                .operations
                .span(body.operations)
                .unwrap_or(&[])
                .to_vec(),
        })
        .collect()
}

fn build_runtime_storage_body_plan(
    context: &RuntimeStorageContext,
    body_input: &RuntimeStorageBodyInput,
) -> RuntimeStoragePlan {
    let mut plan = RuntimeStoragePlan::default();
    let mut next_frame_offset = 0usize;

    for operation in &body_input.operations {
        match &operation.kind {
            RuntimeDispatchBodyOperationKind::LocalStorage { name, type_name } => {
                let layout = layout_for_type_name(context, type_name);
                let byte_offset = align_to(next_frame_offset, layout.alignment);
                next_frame_offset = byte_offset
                    .checked_add(layout.size)
                    .expect("runtime frame slot size overflow");

                plan.frame_slots.insert(RuntimeFrameSlot {
                    dispatch_index: body_input.body.dispatch_index,
                    source_machine: operation.source_machine.clone(),
                    source_state: operation.source_state.clone(),
                    statement_index: operation.statement_index,
                    name: name.clone(),
                    type_name: type_name.clone(),
                    byte_offset,
                    byte_size: layout.size,
                    alignment: layout.alignment,
                });
            }
            RuntimeDispatchBodyOperationKind::Mutation { lowering, .. }
                if *lowering != StateMutationLowering::AlreadyLowered =>
            {
                if let Some(mutation) = mutation_for_operation(
                    context,
                    &operation.source_machine,
                    &operation.source_state,
                    operation.statement_index,
                ) {
                    plan.writes.insert(RuntimeStorageWrite {
                        dispatch_index: body_input.body.dispatch_index,
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

    plan
}

pub fn runtime_frame_storage_size(plan: &RuntimeStoragePlan) -> usize {
    plan.frame_slots
        .iter()
        .map(|(_, slot)| slot.byte_offset + slot.byte_size)
        .max()
        .unwrap_or(0)
}

pub fn runtime_frame_storage_alignment(plan: &RuntimeStoragePlan) -> usize {
    plan.frame_slots
        .iter()
        .map(|(_, slot)| slot.alignment)
        .max()
        .unwrap_or(1)
}

fn layout_for_type_name(context: &RuntimeStorageContext, type_name: &str) -> TypeLayout {
    if let Some(data_layout) = context
        .layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == type_name)
        .map(|(_, data_layout)| data_layout.layout)
    {
        return data_layout;
    }

    if let Some(machine_layout) = context
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == type_name)
        .map(|(_, machine_layout)| machine_layout.layout)
    {
        return machine_layout;
    }

    if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
        return primitive_layout(context, primitive_type);
    }

    TypeLayout::default()
}

fn primitive_layout(context: &RuntimeStorageContext, primitive_type: PrimitiveType) -> TypeLayout {
    match primitive_type {
        PrimitiveType::Bool => TypeLayout {
            size: 1,
            alignment: 1,
        },
        PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => TypeLayout {
            size: 4,
            alignment: 4,
        },
        PrimitiveType::F64 | PrimitiveType::U64 => TypeLayout {
            size: 8,
            alignment: 8,
        },
        PrimitiveType::Usize => TypeLayout {
            size: context.target.pointer_size,
            alignment: context.target.pointer_alignment,
        },
        PrimitiveType::String => TypeLayout {
            size: context.target.pointer_size * 2,
            alignment: context.target.pointer_alignment,
        },
    }
}

fn align_to(offset: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    let remainder = offset % alignment;
    if remainder == 0 {
        offset
    } else {
        offset + alignment - remainder
    }
}

fn mutation_for_operation<'plan>(
    context: &'plan RuntimeStorageContext,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan StateMutation> {
    context
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
