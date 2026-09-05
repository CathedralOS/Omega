//! Typed receipt for exact constant integer bitwise-AND materialization.

use semantic_vocabulary::{EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerBitwiseAndImmediateTranslationReceipt {
    machine: MachineId,
    left_constant_operation: OperationId,
    right_constant_operation: OperationId,
    bitwise_and_operation: OperationId,
    return_edge: EdgeId,
    left_constant_result: ValueId,
    right_constant_result: ValueId,
    bitwise_and_result: ValueId,
    scalar_type: IntegerType,
    left_value: IntegerValue,
    right_value: IntegerValue,
    materialized_value: IntegerValue,
}

impl StraightLineIntegerBitwiseAndImmediateTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        left_constant_operation: OperationId,
        right_constant_operation: OperationId,
        bitwise_and_operation: OperationId,
        return_edge: EdgeId,
        left_constant_result: ValueId,
        right_constant_result: ValueId,
        bitwise_and_result: ValueId,
        scalar_type: IntegerType,
        left_value: IntegerValue,
        right_value: IntegerValue,
        materialized_value: IntegerValue,
    ) -> Self {
        Self {
            machine,
            left_constant_operation,
            right_constant_operation,
            bitwise_and_operation,
            return_edge,
            left_constant_result,
            right_constant_result,
            bitwise_and_result,
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
    pub const fn bitwise_and_operation(self) -> OperationId {
        self.bitwise_and_operation
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
    pub const fn bitwise_and_result(self) -> ValueId {
        self.bitwise_and_result
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
