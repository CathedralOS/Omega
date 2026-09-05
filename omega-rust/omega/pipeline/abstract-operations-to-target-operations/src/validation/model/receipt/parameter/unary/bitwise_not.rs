use semantic_vocabulary::{EdgeId, IntegerType, MachineId, OperationId, ValueId};
use target_operations::ScalarParameterLocation;

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
