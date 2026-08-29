//! Direct and derived parameter receipts.

use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, IntegerType, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerParameterTranslationReceipt {
    machine: MachineId,
    return_edge: EdgeId,
    source_value: ValueId,
    scalar_type: IntegerType,
    parameter_index: usize,
    location: ScalarParameterLocation,
}

impl StraightLineIntegerParameterTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        return_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        parameter_index: usize,
        location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            return_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
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
    pub const fn parameter_index(self) -> usize {
        self.parameter_index
    }
    pub const fn location(self) -> ScalarParameterLocation {
        self.location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineBooleanParameterTranslationReceipt {
    machine: MachineId,
    return_edge: EdgeId,
    source_value: ValueId,
    parameter_index: usize,
    location: ScalarParameterLocation,
}

impl StraightLineBooleanParameterTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        return_edge: EdgeId,
        source_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            return_edge,
            source_value,
            parameter_index,
            location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn source_value(self) -> ValueId {
        self.source_value
    }
    pub const fn parameter_index(self) -> usize {
        self.parameter_index
    }
    pub const fn location(self) -> ScalarParameterLocation {
        self.location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineBooleanNotParameterTranslationReceipt {
    machine: MachineId,
    not_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    operand_value: ValueId,
    parameter_index: usize,
    location: ScalarParameterLocation,
}

impl StraightLineBooleanNotParameterTranslationReceipt {
    pub(in crate::validation) const fn new(
        machine: MachineId,
        not_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        operand_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            not_operation,
            return_edge,
            source_value,
            operand_value,
            parameter_index,
            location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn not_operation(self) -> OperationId {
        self.not_operation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn source_value(self) -> ValueId {
        self.source_value
    }
    pub const fn operand_value(self) -> ValueId {
        self.operand_value
    }
    pub const fn parameter_index(self) -> usize {
        self.parameter_index
    }
    pub const fn location(self) -> ScalarParameterLocation {
        self.location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerBitwiseNotParameterTranslationReceipt {
    machine: MachineId,
    bitwise_not_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    scalar_type: IntegerType,
    operand_value: ValueId,
    parameter_index: usize,
    location: ScalarParameterLocation,
}

impl StraightLineIntegerBitwiseNotParameterTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        bitwise_not_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        operand_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            bitwise_not_operation,
            return_edge,
            source_value,
            scalar_type,
            operand_value,
            parameter_index,
            location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn bitwise_not_operation(self) -> OperationId {
        self.bitwise_not_operation
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
    pub const fn operand_value(self) -> ValueId {
        self.operand_value
    }
    pub const fn parameter_index(self) -> usize {
        self.parameter_index
    }
    pub const fn location(self) -> ScalarParameterLocation {
        self.location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineBooleanEqualParametersTranslationReceipt {
    machine: MachineId,
    equal_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    left_value: ValueId,
    right_value: ValueId,
    left_parameter_index: usize,
    right_parameter_index: usize,
    left_location: ScalarParameterLocation,
    right_location: ScalarParameterLocation,
}

impl StraightLineBooleanEqualParametersTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        equal_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        left_value: ValueId,
        right_value: ValueId,
        left_parameter_index: usize,
        right_parameter_index: usize,
        left_location: ScalarParameterLocation,
        right_location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            equal_operation,
            return_edge,
            source_value,
            left_value,
            right_value,
            left_parameter_index,
            right_parameter_index,
            left_location,
            right_location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn equal_operation(self) -> OperationId {
        self.equal_operation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn source_value(self) -> ValueId {
        self.source_value
    }
    pub const fn left_value(self) -> ValueId {
        self.left_value
    }
    pub const fn right_value(self) -> ValueId {
        self.right_value
    }
    pub const fn left_parameter_index(self) -> usize {
        self.left_parameter_index
    }
    pub const fn right_parameter_index(self) -> usize {
        self.right_parameter_index
    }
    pub const fn left_location(self) -> ScalarParameterLocation {
        self.left_location
    }
    pub const fn right_location(self) -> ScalarParameterLocation {
        self.right_location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerEqualParametersTranslationReceipt {
    machine: MachineId,
    equal_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    scalar_type: IntegerType,
    left_value: ValueId,
    right_value: ValueId,
    left_parameter_index: usize,
    right_parameter_index: usize,
    left_location: ScalarParameterLocation,
    right_location: ScalarParameterLocation,
}

impl StraightLineIntegerEqualParametersTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        equal_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        left_value: ValueId,
        right_value: ValueId,
        left_parameter_index: usize,
        right_parameter_index: usize,
        left_location: ScalarParameterLocation,
        right_location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            equal_operation,
            return_edge,
            source_value,
            scalar_type,
            left_value,
            right_value,
            left_parameter_index,
            right_parameter_index,
            left_location,
            right_location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn equal_operation(self) -> OperationId {
        self.equal_operation
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
    pub const fn left_value(self) -> ValueId {
        self.left_value
    }
    pub const fn right_value(self) -> ValueId {
        self.right_value
    }
    pub const fn left_parameter_index(self) -> usize {
        self.left_parameter_index
    }
    pub const fn right_parameter_index(self) -> usize {
        self.right_parameter_index
    }
    pub const fn left_location(self) -> ScalarParameterLocation {
        self.left_location
    }
    pub const fn right_location(self) -> ScalarParameterLocation {
        self.right_location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerLessThanParametersTranslationReceipt {
    machine: MachineId,
    less_than_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    scalar_type: IntegerType,
    left_value: ValueId,
    right_value: ValueId,
    left_parameter_index: usize,
    right_parameter_index: usize,
    left_location: ScalarParameterLocation,
    right_location: ScalarParameterLocation,
}

impl StraightLineIntegerLessThanParametersTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        less_than_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        left_value: ValueId,
        right_value: ValueId,
        left_parameter_index: usize,
        right_parameter_index: usize,
        left_location: ScalarParameterLocation,
        right_location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            less_than_operation,
            return_edge,
            source_value,
            scalar_type,
            left_value,
            right_value,
            left_parameter_index,
            right_parameter_index,
            left_location,
            right_location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn less_than_operation(self) -> OperationId {
        self.less_than_operation
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
    pub const fn left_value(self) -> ValueId {
        self.left_value
    }
    pub const fn right_value(self) -> ValueId {
        self.right_value
    }
    pub const fn left_parameter_index(self) -> usize {
        self.left_parameter_index
    }
    pub const fn right_parameter_index(self) -> usize {
        self.right_parameter_index
    }
    pub const fn left_location(self) -> ScalarParameterLocation {
        self.left_location
    }
    pub const fn right_location(self) -> ScalarParameterLocation {
        self.right_location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineIntegerLessOrEqualParametersTranslationReceipt {
    machine: MachineId,
    less_or_equal_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    scalar_type: IntegerType,
    left_value: ValueId,
    right_value: ValueId,
    left_parameter_index: usize,
    right_parameter_index: usize,
    left_location: ScalarParameterLocation,
    right_location: ScalarParameterLocation,
}

impl StraightLineIntegerLessOrEqualParametersTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        less_or_equal_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        scalar_type: IntegerType,
        left_value: ValueId,
        right_value: ValueId,
        left_parameter_index: usize,
        right_parameter_index: usize,
        left_location: ScalarParameterLocation,
        right_location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            less_or_equal_operation,
            return_edge,
            source_value,
            scalar_type,
            left_value,
            right_value,
            left_parameter_index,
            right_parameter_index,
            left_location,
            right_location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn less_or_equal_operation(self) -> OperationId {
        self.less_or_equal_operation
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
    pub const fn left_value(self) -> ValueId {
        self.left_value
    }
    pub const fn right_value(self) -> ValueId {
        self.right_value
    }
    pub const fn left_parameter_index(self) -> usize {
        self.left_parameter_index
    }
    pub const fn right_parameter_index(self) -> usize {
        self.right_parameter_index
    }
    pub const fn left_location(self) -> ScalarParameterLocation {
        self.left_location
    }
    pub const fn right_location(self) -> ScalarParameterLocation {
        self.right_location
    }
}
