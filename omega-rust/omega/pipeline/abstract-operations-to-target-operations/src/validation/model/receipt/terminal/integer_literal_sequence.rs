//! Typed members and receipt for finite integer-literal Unit sequences.

use semantic_vocabulary::{EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerLiteralSequenceMember {
    operation: OperationId,
    result: ValueId,
    scalar_type: IntegerType,
    value: IntegerValue,
}

impl IntegerLiteralSequenceMember {
    pub(in crate::validation) const fn new(
        operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    ) -> Self {
        Self {
            operation,
            result,
            scalar_type,
            value,
        }
    }

    pub const fn operation(&self) -> OperationId {
        self.operation
    }

    pub const fn result(&self) -> ValueId {
        self.result
    }

    pub const fn scalar_type(&self) -> IntegerType {
        self.scalar_type
    }

    pub const fn value(&self) -> IntegerValue {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt {
    machine: MachineId,
    literals: Vec<IntegerLiteralSequenceMember>,
    return_edge: EdgeId,
}

impl StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt {
    pub(in crate::validation) fn new(
        machine: MachineId,
        literals: Vec<IntegerLiteralSequenceMember>,
        return_edge: EdgeId,
    ) -> Self {
        Self {
            machine,
            literals,
            return_edge,
        }
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub fn literals(&self) -> &[IntegerLiteralSequenceMember] {
        &self.literals
    }

    pub const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }
}
