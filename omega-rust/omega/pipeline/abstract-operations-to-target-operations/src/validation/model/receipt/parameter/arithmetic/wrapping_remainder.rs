use semantic_vocabulary::{EdgeId, IntegerType, MachineId, ObligationId, OperationId, ValueId};
use target_operations::ScalarParameterLocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineWrappingIntegerRemainderParametersTranslationReceipt {
    machine: MachineId,
    remainder_operation: OperationId,
    obligation: ObligationId,
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

impl StraightLineWrappingIntegerRemainderParametersTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        remainder_operation: OperationId,
        obligation: ObligationId,
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
            remainder_operation,
            obligation,
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

    pub const fn remainder_operation(self) -> OperationId {
        self.remainder_operation
    }

    pub const fn obligation(self) -> ObligationId {
        self.obligation
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
