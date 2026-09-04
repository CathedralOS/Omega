use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, IntegerType, MachineId, ValueId};

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
