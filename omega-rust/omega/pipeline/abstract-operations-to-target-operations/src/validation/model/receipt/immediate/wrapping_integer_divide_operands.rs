//! Typed receipt for proof-bearing wrapping divide over constant operands.

use semantic_vocabulary::{
    EdgeId, IntegerType, IntegerValue, MachineId, ObligationId, OperationId, ValueId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineWrappingIntegerDivideImmediateOperandsTranslationReceipt {
    machine: MachineId,
    left_constant_operation: OperationId,
    right_constant_operation: OperationId,
    divide_operation: OperationId,
    obligation: ObligationId,
    return_edge: EdgeId,
    left_constant_result: ValueId,
    right_constant_result: ValueId,
    divide_result: ValueId,
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
    quotient: IntegerValue,
}

impl StraightLineWrappingIntegerDivideImmediateOperandsTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        left_constant_operation: OperationId,
        right_constant_operation: OperationId,
        divide_operation: OperationId,
        obligation: ObligationId,
        return_edge: EdgeId,
        left_constant_result: ValueId,
        right_constant_result: ValueId,
        divide_result: ValueId,
        scalar_type: IntegerType,
        left: IntegerValue,
        right: IntegerValue,
        quotient: IntegerValue,
    ) -> Self {
        Self {
            machine,
            left_constant_operation,
            right_constant_operation,
            divide_operation,
            obligation,
            return_edge,
            left_constant_result,
            right_constant_result,
            divide_result,
            scalar_type,
            left,
            right,
            quotient,
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
    pub const fn divide_operation(self) -> OperationId {
        self.divide_operation
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
    pub const fn divide_result(self) -> ValueId {
        self.divide_result
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
    pub const fn quotient(self) -> IntegerValue {
        self.quotient
    }
}
