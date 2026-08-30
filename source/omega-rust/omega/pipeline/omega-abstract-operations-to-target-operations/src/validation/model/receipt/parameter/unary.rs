use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, IntegerType, MachineId, OperationId, ValueId};

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
pub struct StraightLineIntegerWidenParameterTranslationReceipt {
    machine: MachineId,
    widen_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    source_type: IntegerType,
    target_type: IntegerType,
    operand_value: ValueId,
    parameter_index: usize,
    location: ScalarParameterLocation,
}

impl StraightLineIntegerWidenParameterTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        widen_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        source_type: IntegerType,
        target_type: IntegerType,
        operand_value: ValueId,
        parameter_index: usize,
        location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            widen_operation,
            return_edge,
            source_value,
            source_type,
            target_type,
            operand_value,
            parameter_index,
            location,
        }
    }
    pub const fn machine(self) -> MachineId {
        self.machine
    }
    pub const fn widen_operation(self) -> OperationId {
        self.widen_operation
    }
    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }
    pub const fn source_value(self) -> ValueId {
        self.source_value
    }
    pub const fn source_type(self) -> IntegerType {
        self.source_type
    }
    pub const fn target_type(self) -> IntegerType {
        self.target_type
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
