//! Typed receipt for exact constant saturating integer-multiply materialization.

use semantic_vocabulary::{EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineSaturatingIntegerMultiplyImmediateTranslationReceipt {
    machine: MachineId,
    left_constant_operation: OperationId,
    right_constant_operation: OperationId,
    saturating_mul_operation: OperationId,
    return_edge: EdgeId,
    left_constant_result: ValueId,
    right_constant_result: ValueId,
    saturating_mul_result: ValueId,
    scalar_type: IntegerType,
    left_value: IntegerValue,
    right_value: IntegerValue,
    materialized_value: IntegerValue,
}

impl StraightLineSaturatingIntegerMultiplyImmediateTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        left_constant_operation: OperationId,
        right_constant_operation: OperationId,
        saturating_mul_operation: OperationId,
        return_edge: EdgeId,
        left_constant_result: ValueId,
        right_constant_result: ValueId,
        saturating_mul_result: ValueId,
        scalar_type: IntegerType,
        left_value: IntegerValue,
        right_value: IntegerValue,
        materialized_value: IntegerValue,
    ) -> Self {
        Self {
            machine,
            left_constant_operation,
            right_constant_operation,
            saturating_mul_operation,
            return_edge,
            left_constant_result,
            right_constant_result,
            saturating_mul_result,
            scalar_type,
            left_value,
            right_value,
            materialized_value,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn left_constant_operation(self) -> OperationId {
        self.left_constant_operation
    }
    pub const fn right_constant_operation(self) -> OperationId {
        self.right_constant_operation
    }
    pub const fn saturating_mul_operation(self) -> OperationId {
        self.saturating_mul_operation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn left_constant_result(self) -> ValueId {
        self.left_constant_result
    }
    pub const fn right_constant_result(self) -> ValueId {
        self.right_constant_result
    }
    pub const fn saturating_mul_result(self) -> ValueId {
        self.saturating_mul_result
    }
    pub const fn scalar_type(self) -> IntegerType {
        self.scalar_type
    }
    pub const fn left_value(self) -> IntegerValue {
        self.left_value
    }
    pub const fn right_value(self) -> IntegerValue {
        self.right_value
    }
    pub const fn materialized_value(self) -> IntegerValue {
        self.materialized_value
    }
}
