//! Typed ordered members and receipt for mixed integer/IEEE-literal Unit sequences.

use semantic_vocabulary::{
    EdgeId, IeeeFloatValue, IntegerType, IntegerValue, MachineId, OperationId, ValueId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerIeeeFloatLiteralSequenceMember {
    Integer {
        operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    IeeeFloat {
        operation: OperationId,
        result: ValueId,
        value: IeeeFloatValue,
    },
}

impl IntegerIeeeFloatLiteralSequenceMember {
    pub(in crate::validation) const fn integer(
        operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
    ) -> Self {
        Self::Integer {
            operation,
            result,
            scalar_type,
            value,
        }
    }

    pub(in crate::validation) const fn ieee_float(
        operation: OperationId,
        result: ValueId,
        value: IeeeFloatValue,
    ) -> Self {
        Self::IeeeFloat {
            operation,
            result,
            value,
        }
    }

    pub const fn operation(&self) -> OperationId {
        match self {
            Self::Integer { operation, .. } | Self::IeeeFloat { operation, .. } => *operation,
        }
    }

    pub const fn result(&self) -> ValueId {
        match self {
            Self::Integer { result, .. } | Self::IeeeFloat { result, .. } => *result,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt {
    machine: MachineId,
    literals: Vec<IntegerIeeeFloatLiteralSequenceMember>,
    return_edge: EdgeId,
}

impl StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt {
    pub(in crate::validation) fn new(
        machine: MachineId,
        literals: Vec<IntegerIeeeFloatLiteralSequenceMember>,
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

    pub fn literals(&self) -> &[IntegerIeeeFloatLiteralSequenceMember] {
        &self.literals
    }

    pub const fn return_edge(&self) -> EdgeId {
        self.return_edge
    }
}
