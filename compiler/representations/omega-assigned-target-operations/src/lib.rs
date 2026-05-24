use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;
use std::sync::Arc;

pub use omega_target_operations::{
    HostOperationKey, InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
    RuntimeTextReadSource, RuntimeValueOperand, RuntimeValueOperandHandle, SelectedInstruction,
    SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationPlan {
    pub target: NativeTarget,
    pub functions: Arena<AssignedTargetOperationFunction>,
    pub instructions: Arena<SelectedInstruction>,
    pub operands: Arena<InstructionOperand>,
    pub runtime_value_operands: Arena<RuntimeValueOperand>,
}

impl Default for AssignedTargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0)
    }
}

impl AssignedTargetOperationPlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
        operand_capacity: usize,
        runtime_value_operand_capacity: usize,
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
            operands: Arena::with_capacity(operand_capacity),
            runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationFunction {
    pub symbol: Arc<str>,
    pub source_key: StateKey,
    pub instructions: HandleSpan<SelectedInstruction>,
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

impl From<omega_target_operations::InstructionPlan> for AssignedTargetOperationPlan {
    fn from(plan: omega_target_operations::InstructionPlan) -> Self {
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
        }
    }
}

impl From<AssignedTargetOperationPlan> for omega_target_operations::InstructionPlan {
    fn from(plan: AssignedTargetOperationPlan) -> Self {
        let mut functions = Arena::with_capacity(plan.functions.len());
        for (_, function) in plan.functions.iter() {
            functions.insert(omega_target_operations::FunctionInstructionPlan {
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
        }
    }
}
