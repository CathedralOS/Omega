//! Typed receipt for exact constant inclusive integer ordering materialization.

use psi_core::{EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerLessOrEqualImmediateTranslationReceipt {
    machine: MachineId,
    left_constant_operation: OperationId,
    right_constant_operation: OperationId,
    less_or_equal_operation: OperationId,
    return_edge: EdgeId,
    left_constant_result: ValueId,
    right_constant_result: ValueId,
    less_or_equal_result: ValueId,
    scalar_type: IntegerType,
    left_value: IntegerValue,
    right_value: IntegerValue,
    materialized_value: bool,
}

impl StraightLineIntegerLessOrEqualImmediateTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        left_constant_operation: OperationId,
        right_constant_operation: OperationId,
        less_or_equal_operation: OperationId,
        return_edge: EdgeId,
        left_constant_result: ValueId,
        right_constant_result: ValueId,
        less_or_equal_result: ValueId,
        scalar_type: IntegerType,
        left_value: IntegerValue,
        right_value: IntegerValue,
        materialized_value: bool,
    ) -> Self {
        Self {
            machine,
            left_constant_operation,
            right_constant_operation,
            less_or_equal_operation,
            return_edge,
            left_constant_result,
            right_constant_result,
            less_or_equal_result,
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

    pub const fn less_or_equal_operation(self) -> OperationId {
        self.less_or_equal_operation
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

    pub const fn less_or_equal_result(self) -> ValueId {
        self.less_or_equal_result
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

    pub const fn materialized_value(self) -> bool {
        self.materialized_value
    }
}
