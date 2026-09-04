//! Typed receipt for proof-bearing wrapping remainder over constant operands.

use psi_core::{EdgeId, IntegerType, IntegerValue, MachineId, ObligationId, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineWrappingIntegerRemainderImmediateOperandsTranslationReceipt {
    machine: MachineId,
    left_constant_operation: OperationId,
    right_constant_operation: OperationId,
    remainder_operation: OperationId,
    obligation: ObligationId,
    return_edge: EdgeId,
    left_constant_result: ValueId,
    right_constant_result: ValueId,
    remainder_result: ValueId,
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
    remainder: IntegerValue,
}

impl StraightLineWrappingIntegerRemainderImmediateOperandsTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        left_constant_operation: OperationId,
        right_constant_operation: OperationId,
        remainder_operation: OperationId,
        obligation: ObligationId,
        return_edge: EdgeId,
        left_constant_result: ValueId,
        right_constant_result: ValueId,
        remainder_result: ValueId,
        scalar_type: IntegerType,
        left: IntegerValue,
        right: IntegerValue,
        remainder: IntegerValue,
    ) -> Self {
        Self {
            machine,
            left_constant_operation,
            right_constant_operation,
            remainder_operation,
            obligation,
            return_edge,
            left_constant_result,
            right_constant_result,
            remainder_result,
            scalar_type,
            left,
            right,
            remainder,
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
    pub const fn remainder_operation(self) -> OperationId {
        self.remainder_operation
    }
    pub const fn obligation(self) -> ObligationId {
        self.obligation
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
    pub const fn remainder_result(self) -> ValueId {
        self.remainder_result
    }
    pub const fn scalar_type(self) -> IntegerType {
        self.scalar_type
    }
    pub const fn left(self) -> IntegerValue {
        self.left
    }
    pub const fn right(self) -> IntegerValue {
        self.right
    }
    pub const fn remainder(self) -> IntegerValue {
        self.remainder
    }
}
