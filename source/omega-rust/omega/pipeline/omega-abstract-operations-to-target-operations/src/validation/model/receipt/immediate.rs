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
