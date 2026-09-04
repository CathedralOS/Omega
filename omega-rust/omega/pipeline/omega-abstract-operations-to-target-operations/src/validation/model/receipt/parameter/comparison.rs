use omega_target_operations::ScalarParameterLocation;
use psi_core::{EdgeId, IntegerType, MachineId, OperationId, ValueId};

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

macro_rules! integer_comparison_receipt {
    ($name:ident, $operation:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            machine: MachineId,
            $operation: OperationId,
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

        impl $name {
            #[allow(clippy::too_many_arguments)]
            pub(in crate::validation) const fn new(
                machine: MachineId,
                $operation: OperationId,
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
                    $operation,
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
            pub const fn $operation(self) -> OperationId {
                self.$operation
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
    };
}

integer_comparison_receipt!(
    StraightLineIntegerEqualParametersTranslationReceipt,
    equal_operation
);
integer_comparison_receipt!(
    StraightLineIntegerLessThanParametersTranslationReceipt,
    less_than_operation
);
integer_comparison_receipt!(
    StraightLineIntegerLessOrEqualParametersTranslationReceipt,
    less_or_equal_operation
);
