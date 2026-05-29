use crate::{
    AssignedInstructionOperand, AssignedOperation, AssignedTargetOperationFunction,
    AssignedTargetOperationPlan, AssignedValueHomeKind, AssignedValueOperand,
    assigned_operation_span_from_target, target_operation_span_from_assigned,
};
use omega_core::arena::Arena;
use std::sync::Arc;

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
            values: plan.semantics.values,
            boundary_edges: plan.semantics.boundary_edges,
            ownership: plan.semantics.ownership,
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
            semantics: omega_target_operations::TargetSemanticSummary {
                values: plan.values,
                boundary_edges: plan.boundary_edges,
                ownership: plan.ownership,
            },
        }
    }
}
