//! Parameterless scalar-immediate receipts.

use psi_core::{EdgeId, IntegerType, IntegerValue, MachineId, ObligationId, OperationId, ValueId};

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
pub struct StraightLineBooleanNotImmediateTranslationReceipt {
    machine: MachineId,
    constant_operation: OperationId,
    boolean_not_operation: OperationId,
    return_edge: EdgeId,
    constant_result: ValueId,
    boolean_not_result: ValueId,
    source_value: bool,
    materialized_value: bool,
}

impl StraightLineBooleanNotImmediateTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        constant_operation: OperationId,
        boolean_not_operation: OperationId,
        return_edge: EdgeId,
        constant_result: ValueId,
        boolean_not_result: ValueId,
        source_value: bool,
        materialized_value: bool,
    ) -> Self {
        Self {
            machine,
            constant_operation,
            boolean_not_operation,
            return_edge,
            constant_result,
            boolean_not_result,
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
    pub const fn boolean_not_operation(self) -> OperationId {
        self.boolean_not_operation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn constant_result(self) -> ValueId {
        self.constant_result
    }
    pub const fn boolean_not_result(self) -> ValueId {
        self.boolean_not_result
    }
    pub const fn source_value(self) -> bool {
        self.source_value
    }
    pub const fn materialized_value(self) -> bool {
        self.materialized_value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineBooleanEqualImmediateTranslationReceipt {
    machine: MachineId,
    left_constant_operation: OperationId,
    right_constant_operation: OperationId,
    equal_operation: OperationId,
    return_edge: EdgeId,
    left_constant_result: ValueId,
    right_constant_result: ValueId,
    equal_result: ValueId,
    left_value: bool,
    right_value: bool,
    materialized_value: bool,
}

impl StraightLineBooleanEqualImmediateTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        left_constant_operation: OperationId,
        right_constant_operation: OperationId,
        equal_operation: OperationId,
        return_edge: EdgeId,
        left_constant_result: ValueId,
        right_constant_result: ValueId,
        equal_result: ValueId,
        left_value: bool,
        right_value: bool,
        materialized_value: bool,
    ) -> Self {
        Self {
            machine,
            left_constant_operation,
            right_constant_operation,
            equal_operation,
            return_edge,
            left_constant_result,
            right_constant_result,
            equal_result,
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
    pub const fn equal_operation(self) -> OperationId {
        self.equal_operation
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
    pub const fn equal_result(self) -> ValueId {
        self.equal_result
    }
    pub const fn left_value(self) -> bool {
        self.left_value
    }
    pub const fn right_value(self) -> bool {
        self.right_value
    }
    pub const fn materialized_value(self) -> bool {
        self.materialized_value
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerExactCastImmediateOperandTranslationReceipt {
    machine: MachineId,
    constant_operation: OperationId,
    cast_operation: OperationId,
    obligation: ObligationId,
    return_edge: EdgeId,
    constant_result: ValueId,
    cast_result: ValueId,
    source_type: IntegerType,
    target_type: IntegerType,
    source_value: IntegerValue,
    cast_value: IntegerValue,
}

impl StraightLineIntegerExactCastImmediateOperandTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        constant_operation: OperationId,
        cast_operation: OperationId,
        obligation: ObligationId,
        return_edge: EdgeId,
        constant_result: ValueId,
        cast_result: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        source_value: IntegerValue,
        cast_value: IntegerValue,
    ) -> Self {
        Self {
            machine,
            constant_operation,
            cast_operation,
            obligation,
            return_edge,
            constant_result,
            cast_result,
            source_type,
            target_type,
            source_value,
            cast_value,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn constant_operation(self) -> OperationId {
        self.constant_operation
    }
    pub const fn cast_operation(self) -> OperationId {
        self.cast_operation
    }
    pub const fn obligation(self) -> ObligationId {
        self.obligation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn constant_result(self) -> ValueId {
        self.constant_result
    }
    pub const fn cast_result(self) -> ValueId {
        self.cast_result
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
    pub const fn cast_value(self) -> IntegerValue {
        self.cast_value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerBitwiseNotImmediateTranslationReceipt {
    machine: MachineId,
    constant_operation: OperationId,
    bitwise_not_operation: OperationId,
    return_edge: EdgeId,
    constant_result: ValueId,
    bitwise_not_result: ValueId,
    scalar_type: IntegerType,
    source_value: IntegerValue,
    materialized_value: IntegerValue,
}

impl StraightLineIntegerBitwiseNotImmediateTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        constant_operation: OperationId,
        bitwise_not_operation: OperationId,
        return_edge: EdgeId,
        constant_result: ValueId,
        bitwise_not_result: ValueId,
        scalar_type: IntegerType,
        source_value: IntegerValue,
        materialized_value: IntegerValue,
    ) -> Self {
        Self {
            machine,
            constant_operation,
            bitwise_not_operation,
            return_edge,
            constant_result,
            bitwise_not_result,
            scalar_type,
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
    pub const fn bitwise_not_operation(self) -> OperationId {
        self.bitwise_not_operation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn constant_result(self) -> ValueId {
        self.constant_result
    }
    pub const fn bitwise_not_result(self) -> ValueId {
        self.bitwise_not_result
    }
    pub const fn scalar_type(self) -> IntegerType {
        self.scalar_type
    }
    pub const fn source_value(self) -> IntegerValue {
        self.source_value
    }
    pub const fn materialized_value(self) -> IntegerValue {
        self.materialized_value
    }
}
