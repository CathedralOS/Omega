//! Parameterless scalar-immediate receipts.

use psi_core::{EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineBooleanImmediateTranslationReceipt {
    machine: MachineId,
    constant_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    value: bool,
}

impl StraightLineBooleanImmediateTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        constant_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        value: bool,
    ) -> Self {
        Self {
            machine,
            constant_operation,
            return_edge,
            source_value,
            value,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn constant_operation(self) -> OperationId {
        self.constant_operation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn source_value(self) -> ValueId {
        self.source_value
    }
    pub const fn value(self) -> bool {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerImmediateTranslationReceipt {
    machine: MachineId,
    constant_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    scalar_type: IntegerType,
    value: IntegerValue,
}

impl StraightLineIntegerImmediateTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        constant_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    ) -> Self {
        Self {
            machine,
            constant_operation,
            return_edge,
            source_value,
            scalar_type,
            value,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn constant_operation(self) -> OperationId {
        self.constant_operation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn source_value(self) -> ValueId {
        self.source_value
    }
    pub const fn scalar_type(self) -> IntegerType {
        self.scalar_type
    }
    pub const fn value(self) -> IntegerValue {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerWidenImmediateTranslationReceipt {
    machine: MachineId,
    constant_operation: OperationId,
    widen_operation: OperationId,
    return_edge: EdgeId,
    constant_result: ValueId,
    widened_result: ValueId,
    source_type: IntegerType,
    target_type: IntegerType,
    source_value: IntegerValue,
    materialized_value: IntegerValue,
}

impl StraightLineIntegerWidenImmediateTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        constant_operation: OperationId,
        widen_operation: OperationId,
        return_edge: EdgeId,
        constant_result: ValueId,
        widened_result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        source_value: IntegerValue,
        materialized_value: IntegerValue,
    ) -> Self {
        Self {
            machine,
            constant_operation,
            widen_operation,
            return_edge,
            constant_result,
            widened_result,
            source_type,
            target_type,
            source_value,
            materialized_value,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn constant_operation(self) -> OperationId {
        self.constant_operation
    }
    pub const fn widen_operation(self) -> OperationId {
        self.widen_operation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn constant_result(self) -> ValueId {
        self.constant_result
    }
    pub const fn widened_result(self) -> ValueId {
        self.widened_result
    }
    pub const fn source_type(self) -> IntegerType {
        self.source_type
    }
    pub const fn target_type(self) -> IntegerType {
        self.target_type
    }
    pub const fn source_value(self) -> IntegerValue {
        self.source_value
    }
    pub const fn materialized_value(self) -> IntegerValue {
        self.materialized_value
    }
}
