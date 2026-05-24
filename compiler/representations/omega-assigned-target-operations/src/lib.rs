use omega_control_flow::StateKey;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_target::NativeTarget;
use std::sync::Arc;

pub use omega_target_operations::{
    HostOperationKey, RuntimeStorageRegion, RuntimeTextReadSource, StateGuardLowering,
    StateGuardOperator, TargetHostBinding,
};

pub type AssignedInstructionOperand = omega_target_operations::TargetInstructionOperand;
pub type AssignedInstructionOperandKind = omega_target_operations::TargetInstructionOperandKind;
pub type InstructionOperand = AssignedInstructionOperand;
pub type InstructionOperandKind = AssignedInstructionOperandKind;

pub type AssignedValueOperand = omega_target_operations::TargetValueOperand;
pub type AssignedValueOperandHandle = Handle<AssignedValueOperand>;
pub type RuntimeValueOperand = AssignedValueOperand;
pub type RuntimeValueOperandHandle = AssignedValueOperandHandle;
pub type TargetValueOperand = AssignedValueOperand;
pub type TargetValueOperandHandle = AssignedValueOperandHandle;

pub type AssignedOperation = omega_target_operations::TargetOperation;
pub type AssignedOperationKind = omega_target_operations::TargetOperationKind;
pub type AssignedOperationFunction = omega_target_operations::TargetOperationFunction;
pub type SelectedInstruction = AssignedOperation;
pub type SelectedInstructionKind = AssignedOperationKind;
pub type TargetOperation = AssignedOperation;
pub type TargetOperationKind = AssignedOperationKind;
pub type TargetOperationFunction = AssignedOperationFunction;
pub type TargetOperationPlan = omega_target_operations::TargetOperationPlan;

pub type AssignedValueHomeHandle = Handle<AssignedValueHome>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssignedRegisterBank {
    #[default]
    GeneralPurpose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64AssignedRegister {
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedRegisterName {
    Aarch64X(u8),
    X86_64(X86_64AssignedRegister),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedValueHomeKind {
    Immediate,
    StackSlot {
        byte_offset: usize,
        byte_size: usize,
    },
    RuntimeStorage {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
    },
    RuntimePointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    RuntimeFrameIndexed {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    ScratchRegister {
        bank: AssignedRegisterBank,
        name: AssignedRegisterName,
    },
}

impl Default for AssignedValueHomeKind {
    fn default() -> Self {
        Self::Immediate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AssignedValueHome {
    pub kind: AssignedValueHomeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationPlan {
    pub target: NativeTarget,
    pub functions: Arena<AssignedTargetOperationFunction>,
    pub instructions: Arena<AssignedOperation>,
    pub operands: Arena<AssignedInstructionOperand>,
    pub runtime_value_operands: Arena<AssignedValueOperand>,
    pub host_bindings: Arena<TargetHostBinding>,
    pub runtime_value_homes: Arena<AssignedValueHome>,
}

impl Default for AssignedTargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0, 0, 0)
    }
}

impl AssignedTargetOperationPlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
        operand_capacity: usize,
        runtime_value_operand_capacity: usize,
        host_binding_capacity: usize,
        runtime_value_home_capacity: usize,
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
            operands: Arena::with_capacity(operand_capacity),
            runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
            host_bindings: Arena::with_capacity(host_binding_capacity),
            runtime_value_homes: Arena::with_capacity(runtime_value_home_capacity),
        }
    }

    pub fn host_binding(&self, operation_key: HostOperationKey) -> Option<&TargetHostBinding> {
        self.host_bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation_key)
            .map(|(_, binding)| binding)
    }

    pub fn runtime_value_home_handle(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> AssignedValueHomeHandle {
        let home_handle = Handle::from_arena_index(handle.arena_index());
        if self.runtime_value_homes.is_valid(home_handle) {
            home_handle
        } else {
            AssignedValueHomeHandle::invalid()
        }
    }

    pub fn runtime_value_home(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<&AssignedValueHome> {
        let home_handle = self.runtime_value_home_handle(handle);
        self.runtime_value_homes
            .is_valid(home_handle)
            .then(|| self.runtime_value_homes.get(home_handle))
    }

    pub fn runtime_values_with_homes(
        &self,
    ) -> impl Iterator<Item = (RuntimeValueOperandHandle, &AssignedValueOperand, &AssignedValueHome)> + '_
    {
        self.runtime_value_operands.iter().filter_map(|(handle, operand)| {
            self.runtime_value_home(handle)
                .map(|home| (handle, operand, home))
        })
    }

    pub fn scratch_home_count(&self) -> usize {
        self.runtime_values_with_homes()
            .filter(|(_, _, home)| matches!(home.kind, AssignedValueHomeKind::ScratchRegister { .. }))
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationFunction {
    pub symbol: Arc<str>,
    pub source_key: StateKey,
    pub instructions: HandleSpan<AssignedOperation>,
}

impl Default for AssignedTargetOperationFunction {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}

impl From<omega_target_operations::TargetOperationPlan> for AssignedTargetOperationPlan {
    fn from(plan: omega_target_operations::TargetOperationPlan) -> Self {
        let mut functions = Arena::with_capacity(plan.functions.len());
        for (_, function) in plan.functions.iter() {
            functions.insert(AssignedTargetOperationFunction {
                symbol: Arc::clone(&function.symbol),
                source_key: function.source_key,
                instructions: function.instructions,
            });
        }

        Self {
            target: plan.target,
            functions,
            instructions: plan.instructions,
            operands: plan.operands,
            runtime_value_operands: plan.runtime_value_operands,
            host_bindings: plan.host_bindings,
            runtime_value_homes: Arena::new(),
        }
    }
}

impl From<AssignedTargetOperationPlan> for omega_target_operations::TargetOperationPlan {
    fn from(plan: AssignedTargetOperationPlan) -> Self {
        let mut functions = Arena::with_capacity(plan.functions.len());
        for (_, function) in plan.functions.iter() {
            functions.insert(omega_target_operations::TargetOperationFunction {
                symbol: Arc::clone(&function.symbol),
                source_key: function.source_key,
                instructions: function.instructions,
            });
        }

        Self {
            target: plan.target,
            functions,
            instructions: plan.instructions,
            operands: plan.operands,
            runtime_value_operands: plan.runtime_value_operands,
            host_bindings: plan.host_bindings,
        }
    }
}
