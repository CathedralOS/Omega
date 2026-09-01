use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, IntegerType, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightLineWrappingIntegerShiftRightParametersTranslationReceipt {
    machine: MachineId,
    shift_operation: OperationId,
    return_edge: EdgeId,
    source_value: ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value: ValueId,
    count: ValueId,
    value_parameter_index: usize,
    count_parameter_index: usize,
    value_location: ScalarParameterLocation,
    count_location: ScalarParameterLocation,
}

impl StraightLineWrappingIntegerShiftRightParametersTranslationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::validation) const fn new(
        machine: MachineId,
        shift_operation: OperationId,
        return_edge: EdgeId,
        source_value: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
        value_parameter_index: usize,
        count_parameter_index: usize,
        value_location: ScalarParameterLocation,
        count_location: ScalarParameterLocation,
    ) -> Self {
        Self {
            machine,
            shift_operation,
            return_edge,
            source_value,
            value_type,
            count_type,
            value,
            count,
            value_parameter_index,
            count_parameter_index,
            value_location,
            count_location,
        }
    }

    pub const fn machine(self) -> MachineId {
        self.machine
    }

    pub const fn shift_operation(self) -> OperationId {
        self.shift_operation
    }

    pub const fn return_edge(self) -> EdgeId {
        self.return_edge
    }

    pub const fn source_value(self) -> ValueId {
        self.source_value
    }

    pub const fn value_type(self) -> IntegerType {
        self.value_type
    }

    pub const fn count_type(self) -> IntegerType {
        self.count_type
    }

    pub const fn value(self) -> ValueId {
        self.value
    }

    pub const fn count(self) -> ValueId {
        self.count
    }

    pub const fn value_parameter_index(self) -> usize {
        self.value_parameter_index
    }

    pub const fn count_parameter_index(self) -> usize {
        self.count_parameter_index
    }

    pub const fn value_location(self) -> ScalarParameterLocation {
        self.value_location
    }

    pub const fn count_location(self) -> ScalarParameterLocation {
        self.count_location
    }
}
