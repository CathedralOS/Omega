use crate::{
    AssignedInstructionOperand, AssignedOperation, AssignedTargetOperationFunction,
    AssignedValueHomeHandle, AssignedValueHomeKind, AssignedValueOperand,
    RuntimeValueOperandHandle, assigned_operation_span_from_target, operands,
    target_operation_span_from_assigned,
};
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_target::NativeTarget;
use omega_target_operations::{HostOperationKey, TargetHostBinding};
use std::sync::Arc;

pub type TargetOperationPlan = omega_target_operations::TargetOperationPlan;
pub type AssignedValueSummary = omega_target_operations::TargetValueSummary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationPlan {
    pub target: NativeTarget,
    pub functions: Arena<AssignedTargetOperationFunction>,
    pub instructions: Arena<AssignedOperation>,
    pub operands: Arena<AssignedInstructionOperand>,
    pub runtime_value_operands: Arena<AssignedValueOperand>,
    pub host_bindings: Arena<TargetHostBinding>,
    pub values: AssignedValueSummary,
    pub boundary_edges: omega_target_operations::TargetBoundarySummary,
    pub ownership: omega_target_operations::TargetOwnershipSummary,
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
            host_bindings: Arena::with_capacity(host_binding_capacity),
            values: AssignedValueSummary::default(),
            boundary_edges: omega_target_operations::TargetBoundarySummary::default(),
            ownership: omega_target_operations::TargetOwnershipSummary::default(),
        }
    }

    pub fn host_binding(&self, operation_key: HostOperationKey) -> Option<&TargetHostBinding> {
        self.host_bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation_key)
            .map(|(_, binding)| binding)
    }

    pub fn instruction_operand(
        &self,
        handle: Handle<omega_target_operations::TargetInstructionOperand>,
    ) -> Option<&AssignedInstructionOperand> {
        let handle = assigned_instruction_handle(handle);
        self.operands
            .is_valid(handle)
            .then(|| self.operands.get(handle))
    }

    pub fn instruction_operands(
        &self,
        span: HandleSpan<omega_target_operations::TargetInstructionOperand>,
    ) -> Option<&[AssignedInstructionOperand]> {
        let span = assigned_instruction_span(span);
        self.operands.span(span)
    }

    pub fn runtime_value_home_handle(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> AssignedValueHomeHandle {
        if operands::assigned_value_handle(handle).is_valid()
            && self
                .runtime_value_operands
                .is_valid(operands::assigned_value_handle(handle))
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
        let handle = operands::assigned_value_handle(handle);
        self.runtime_value_operands
            .is_valid(handle)
            .then(|| self.runtime_value_operands.get(handle))
    }

    pub fn runtime_values_with_homes(
        &self,
    ) -> impl Iterator<Item = (RuntimeValueOperandHandle, &AssignedValueOperand)> + '_ {
        self.runtime_value_operands
            .iter()
            .map(|(handle, operand)| (operands::target_value_handle(handle), operand))
    }

    pub fn scratch_home_count(&self) -> usize {
        self.runtime_values_with_homes()
            .filter(|(_, operand)| {
                matches!(operand.home, AssignedValueHomeKind::ScratchRegister { .. })
            })
            .count()
    }
}

fn assigned_instruction_handle(
    handle: Handle<omega_target_operations::TargetInstructionOperand>,
) -> Handle<AssignedInstructionOperand> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn assigned_instruction_span(
    span: HandleSpan<omega_target_operations::TargetInstructionOperand>,
) -> HandleSpan<AssignedInstructionOperand> {
    if span.is_empty() {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(assigned_instruction_handle(span.start()), span.count())
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
                kind: instruction.kind.clone().into(),
                source_key: instruction.source_key,
                source_statement: instruction.source_statement,
            });
        }

        let mut runtime_value_operands = Arena::with_capacity(plan.runtime_value_operands.len());
        for (_, operand) in plan.runtime_value_operands.iter() {
            runtime_value_operands.insert(AssignedValueOperand {
                kind: operand.clone().into(),
                home: AssignedValueHomeKind::Immediate,
            });
        }

        Self {
            target: plan.target,
            functions,
            instructions,
            operands: {
                let mut operands = Arena::with_capacity(plan.operands.len());
                for (_, operand) in plan.operands.iter() {
                    operands.insert(AssignedInstructionOperand {
                        kind: operand.kind.clone().into(),
                    });
                }
                operands
            },
            runtime_value_operands,
            host_bindings: plan.host_bindings,
            values: plan.values,
            boundary_edges: plan.boundary_edges,
            ownership: plan.ownership,
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
                kind: instruction.kind.clone().into(),
                source_key: instruction.source_key,
                source_statement: instruction.source_statement,
            });
        }

        let runtime_value_operands = {
            let mut runtime_value_operands =
                Arena::with_capacity(plan.runtime_value_operands.len());
            for (_, operand) in plan.runtime_value_operands.iter() {
                runtime_value_operands.insert(operand.kind.clone().into());
            }
            runtime_value_operands
        };

        Self {
            target: plan.target,
            functions,
            instructions,
            operands: {
                let mut operands = Arena::with_capacity(plan.operands.len());
                for (_, operand) in plan.operands.iter() {
                    operands.insert(omega_target_operations::TargetInstructionOperand {
                        kind: operand.kind.clone().into(),
                    });
                }
                operands
            },
            runtime_value_operands,
            host_bindings: plan.host_bindings,
            values: plan.values,
            boundary_edges: plan.boundary_edges,
            ownership: plan.ownership,
        }
    }
}
