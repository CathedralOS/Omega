use super::operand_handles::{assigned_instruction_handle, assigned_instruction_span};
use crate::{
    AssignedInstructionOperand, AssignedTargetOperationPlan, AssignedValueHomeHandle,
    AssignedValueHomeKind, AssignedValueOperand, RuntimeValueOperandHandle, value_operands,
};
use omega_target_operations::{HostOperationKey, TargetHostBinding};
use psi_arena::{Handle, HandleSpan};

impl AssignedTargetOperationPlan {
    pub fn host_binding(&self, operation_key: HostOperationKey) -> Option<&TargetHostBinding> {
        self.code
            .host_bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation_key)
            .map(|(_, binding)| binding)
    }

    pub fn instruction_operand(
        &self,
        handle: Handle<omega_target_operations::TargetInstructionOperand>,
    ) -> Option<&AssignedInstructionOperand> {
        let handle = assigned_instruction_handle(handle);
        self.code
            .operands
            .is_valid(handle)
            .then(|| self.code.operands.get(handle))
    }

    pub fn instruction_operands(
        &self,
        span: HandleSpan<omega_target_operations::TargetInstructionOperand>,
    ) -> Option<&[AssignedInstructionOperand]> {
        let span = assigned_instruction_span(span);
        self.code.operands.span(span)
    }

    pub fn runtime_value_home_handle(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> AssignedValueHomeHandle {
        if value_operands::assigned_value_handle(handle).is_valid()
            && self
                .code
                .runtime_value_operands
                .is_valid(value_operands::assigned_value_handle(handle))
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
        self.runtime_value_operand(handle)
            .map(|operand| operand.home)
    }

    pub fn runtime_value_operand(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<&AssignedValueOperand> {
        let handle = value_operands::assigned_value_handle(handle);
        self.code
            .runtime_value_operands
            .is_valid(handle)
            .then(|| self.code.runtime_value_operands.get(handle))
    }

    pub fn runtime_values_with_homes(
        &self,
    ) -> impl Iterator<Item = (RuntimeValueOperandHandle, &AssignedValueOperand)> + '_ {
        self.code
            .runtime_value_operands
            .iter()
            .map(|(handle, operand)| (value_operands::target_value_handle(handle), operand))
    }

    pub fn scratch_home_count(&self) -> usize {
        self.runtime_values_with_homes()
            .filter(|(_, operand)| {
                matches!(operand.home, AssignedValueHomeKind::ScratchRegister { .. })
            })
            .count()
    }
}
