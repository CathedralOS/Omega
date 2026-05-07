use crate::native::layout::TypeLayout;
use crate::native::plan::NativePlan;
use crate::native::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::native::state_storage::{StateMutationKind, StateMutationLowering};
use omega_core::arena::Arena;
use omega_typed_program::expression::Expression;
use omega_typed_program::types::PrimitiveType;

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
    pub byte_offset: usize,
    pub byte_size: usize,
    pub alignment: usize,
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
        let mut next_frame_offset = 0usize;

        for operation in operations {
            match &operation.kind {
                RuntimeDispatchBodyOperationKind::LocalStorage { name, type_name } => {
                    let layout = layout_for_type_name(native_plan, type_name);
                    let byte_offset = align_to(next_frame_offset, layout.alignment);
                    next_frame_offset = byte_offset
                        .checked_add(layout.size)
                        .expect("runtime frame slot size overflow");

                    plan.frame_slots.insert(RuntimeFrameSlot {
                        dispatch_index: body.dispatch_index,
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

fn layout_for_type_name(native_plan: &NativePlan, type_name: &str) -> TypeLayout {
    if let Some(data_layout) = native_plan
        .layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == type_name)
        .map(|(_, data_layout)| data_layout.layout)
    {
        return data_layout;
    }

    if let Some(machine_layout) = native_plan
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == type_name)
        .map(|(_, machine_layout)| machine_layout.layout)
    {
        return machine_layout;
    }

    if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
        return primitive_layout(native_plan, primitive_type);
    }

    TypeLayout::default()
}

fn primitive_layout(native_plan: &NativePlan, primitive_type: PrimitiveType) -> TypeLayout {
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
            size: native_plan.target.pointer_size,
            alignment: native_plan.target.pointer_alignment,
        },
        PrimitiveType::String => TypeLayout {
            size: native_plan.target.pointer_size * 2,
            alignment: native_plan.target.pointer_alignment,
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
