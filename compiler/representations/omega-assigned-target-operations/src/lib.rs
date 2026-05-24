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

pub type AssignedValueOperandKind = omega_target_operations::TargetValueOperand;
pub type AssignedValueOperandHandle = omega_target_operations::TargetValueOperandHandle;
pub type RuntimeValueOperand = AssignedValueOperandKind;
pub type RuntimeValueOperandHandle = AssignedValueOperandHandle;
pub type TargetValueOperand = AssignedValueOperandKind;
pub type TargetValueOperandHandle = AssignedValueOperandHandle;

pub type AssignedOperationKind = omega_target_operations::TargetOperationKind;
pub type SelectedInstructionKind = AssignedOperationKind;
pub type TargetOperationKind = AssignedOperationKind;
pub type TargetOperationPlan = omega_target_operations::TargetOperationPlan;

pub type AssignedValueHomeHandle = AssignedValueOperandHandle;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedOperation {
    pub kind: AssignedOperationKind,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type SelectedInstruction = AssignedOperation;
pub type TargetOperation = AssignedOperation;

impl Default for AssignedOperation {
    fn default() -> Self {
        Self {
            kind: AssignedOperationKind::EnterFunction,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedValueOperand {
    pub kind: AssignedValueOperandKind,
    pub home: AssignedValueHomeKind,
}

impl Default for AssignedValueOperand {
    fn default() -> Self {
        Self {
            kind: AssignedValueOperandKind::Immediate(0),
            home: AssignedValueHomeKind::Immediate,
        }
    }
}

pub fn assigned_operation_span_from_target(
    span: HandleSpan<omega_target_operations::TargetOperation>,
) -> HandleSpan<AssignedOperation> {
    if span.is_empty() {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(
            Handle::from_parts(span.start().arena_index(), span.start().generation()),
            span.count(),
        )
    }
}

pub fn target_operation_span_from_assigned(
    span: HandleSpan<AssignedOperation>,
) -> HandleSpan<omega_target_operations::TargetOperation> {
    if span.is_empty() {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(
            Handle::from_parts(span.start().arena_index(), span.start().generation()),
            span.count(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationPlan {
    pub target: NativeTarget,
    pub functions: Arena<AssignedTargetOperationFunction>,
    pub instructions: Arena<AssignedOperation>,
    pub operands: Arena<AssignedInstructionOperand>,
    pub runtime_value_operands: Arena<AssignedValueOperand>,
    target_runtime_value_operands: Arena<omega_target_operations::TargetValueOperand>,
    pub host_bindings: Arena<TargetHostBinding>,
}

impl Default for AssignedTargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0, 0)
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
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
            operands: Arena::with_capacity(operand_capacity),
            runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
            target_runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
            host_bindings: Arena::with_capacity(host_binding_capacity),
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
        if assigned_value_handle(handle)
            .is_valid()
            && self.runtime_value_operands.is_valid(assigned_value_handle(handle))
        {
            handle
        } else {
            AssignedValueHomeHandle::invalid()
        }
    }

    pub fn runtime_value_home(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<AssignedValueHomeKind> {
        self.runtime_value_operand(handle).map(|operand| operand.home)
    }

    pub fn runtime_value_operand(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<&AssignedValueOperand> {
        let handle = assigned_value_handle(handle);
        self.runtime_value_operands
            .is_valid(handle)
            .then(|| self.runtime_value_operands.get(handle))
    }

    pub fn runtime_values_with_homes(
        &self,
    ) -> impl Iterator<Item = (RuntimeValueOperandHandle, &AssignedValueOperand)> + '_ {
        self.runtime_value_operands
            .iter()
            .map(|(handle, operand)| (target_value_handle(handle), operand))
    }

    pub fn target_runtime_value_operands(
        &self,
    ) -> &Arena<omega_target_operations::TargetValueOperand> {
        &self.target_runtime_value_operands
    }

    pub fn set_target_runtime_value_operands(
        &mut self,
        operands: Arena<omega_target_operations::TargetValueOperand>,
    ) {
        self.target_runtime_value_operands = operands;
    }

    pub fn scratch_home_count(&self) -> usize {
        self.runtime_values_with_homes()
            .filter(|(_, operand)| matches!(operand.home, AssignedValueHomeKind::ScratchRegister { .. }))
            .count()
    }
}

fn assigned_value_handle(
    handle: RuntimeValueOperandHandle,
) -> Handle<AssignedValueOperand> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn target_value_handle(
    handle: Handle<AssignedValueOperand>,
) -> RuntimeValueOperandHandle {
    Handle::from_parts(handle.arena_index(), handle.generation())
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
                instructions: assigned_operation_span_from_target(function.instructions),
            });
        }

        let mut instructions = Arena::with_capacity(plan.instructions.len());
        for (_, instruction) in plan.instructions.iter() {
            instructions.insert(AssignedOperation {
                kind: instruction.kind.clone(),
                source_key: instruction.source_key,
                source_statement: instruction.source_statement,
            });
        }

        let mut runtime_value_operands = Arena::with_capacity(plan.runtime_value_operands.len());
        let mut target_runtime_value_operands =
            Arena::with_capacity(plan.runtime_value_operands.len());
        for (_, operand) in plan.runtime_value_operands.iter() {
            target_runtime_value_operands.insert(operand.clone());
            runtime_value_operands.insert(AssignedValueOperand {
                kind: operand.clone(),
                home: AssignedValueHomeKind::Immediate,
            });
        }

        Self {
            target: plan.target,
            functions,
            instructions,
            operands: plan.operands,
            runtime_value_operands,
            target_runtime_value_operands,
            host_bindings: plan.host_bindings,
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
                instructions: target_operation_span_from_assigned(function.instructions),
            });
        }

        let mut instructions = Arena::with_capacity(plan.instructions.len());
        for (_, instruction) in plan.instructions.iter() {
            instructions.insert(omega_target_operations::TargetOperation {
                kind: instruction.kind.clone(),
                source_key: instruction.source_key,
                source_statement: instruction.source_statement,
            });
        }

        Self {
            target: plan.target,
            functions,
            instructions,
            operands: plan.operands,
            runtime_value_operands: plan.target_runtime_value_operands,
            host_bindings: plan.host_bindings,
        }
    }
}
