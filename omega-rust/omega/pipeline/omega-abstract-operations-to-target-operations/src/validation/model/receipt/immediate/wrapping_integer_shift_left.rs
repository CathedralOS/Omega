//! Typed receipt for exact constant wrapping integer shift-left materialization.

use psi_core::{EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineWrappingIntegerShiftLeftImmediateTranslationReceipt {
    machine: MachineId,
    value_constant_operation: OperationId,
    count_constant_operation: OperationId,
    wrapping_shift_operation: OperationId,
    return_edge: EdgeId,
    value_constant_result: ValueId,
    count_constant_result: ValueId,
    wrapping_shift_result: ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value: IntegerValue,
    count: IntegerValue,
    materialized_value: IntegerValue,
}

impl StraightLineWrappingIntegerShiftLeftImmediateTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        value_constant_operation: OperationId,
        count_constant_operation: OperationId,
        wrapping_shift_operation: OperationId,
        return_edge: EdgeId,
        value_constant_result: ValueId,
        count_constant_result: ValueId,
        wrapping_shift_result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: IntegerValue,
        count: IntegerValue,
        materialized_value: IntegerValue,
    ) -> Self {
        Self {
            machine,
            value_constant_operation,
            count_constant_operation,
            wrapping_shift_operation,
            return_edge,
            value_constant_result,
            count_constant_result,
            wrapping_shift_result,
            value_type,
            count_type,
            value,
            count,
            materialized_value,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn value_constant_operation(self) -> OperationId {
        self.value_constant_operation
    }
    pub const fn count_constant_operation(self) -> OperationId {
        self.count_constant_operation
    }
    pub const fn wrapping_shift_operation(self) -> OperationId {
        self.wrapping_shift_operation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn value_constant_result(self) -> ValueId {
        self.value_constant_result
    }
    pub const fn count_constant_result(self) -> ValueId {
        self.count_constant_result
    }
    pub const fn wrapping_shift_result(self) -> ValueId {
        self.wrapping_shift_result
    }
    pub const fn value_type(self) -> IntegerType {
        self.value_type
    }
    pub const fn count_type(self) -> IntegerType {
        self.count_type
    }
    pub const fn value(self) -> IntegerValue {
        self.value
    }
    pub const fn count(self) -> IntegerValue {
        self.count
    }
    pub const fn materialized_value(self) -> IntegerValue {
        self.materialized_value
    }
}
