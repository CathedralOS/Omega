//! Shared field schema instantiated by each exact bitwise receipt leaf.

macro_rules! bitwise_parameter_receipt {
    ($name:ident, $operation:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            machine: semantic_vocabulary::MachineId,
            $operation: semantic_vocabulary::OperationId,
            return_edge: semantic_vocabulary::EdgeId,
            source_value: semantic_vocabulary::ValueId,
            scalar_type: semantic_vocabulary::IntegerType,
            left_value: semantic_vocabulary::ValueId,
            right_value: semantic_vocabulary::ValueId,
            left_parameter_index: usize,
            right_parameter_index: usize,
            left_location: target_operations::ScalarParameterLocation,
            right_location: target_operations::ScalarParameterLocation,
        }

        impl $name {
            #[allow(clippy::too_many_arguments)]
            pub(in crate::validation) const fn new(
                machine: semantic_vocabulary::MachineId,
                $operation: semantic_vocabulary::OperationId,
                return_edge: semantic_vocabulary::EdgeId,
                source_value: semantic_vocabulary::ValueId,
                scalar_type: semantic_vocabulary::IntegerType,
                left_value: semantic_vocabulary::ValueId,
                right_value: semantic_vocabulary::ValueId,
                left_parameter_index: usize,
                right_parameter_index: usize,
                left_location: target_operations::ScalarParameterLocation,
                right_location: target_operations::ScalarParameterLocation,
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

            pub const fn machine(self) -> semantic_vocabulary::MachineId {
                self.machine
            }
            pub const fn $operation(self) -> semantic_vocabulary::OperationId {
                self.$operation
            }
            pub const fn return_edge(self) -> semantic_vocabulary::EdgeId {
                self.return_edge
            }
            pub const fn source_value(self) -> semantic_vocabulary::ValueId {
                self.source_value
            }
            pub const fn scalar_type(self) -> semantic_vocabulary::IntegerType {
                self.scalar_type
            }
            pub const fn left_value(self) -> semantic_vocabulary::ValueId {
                self.left_value
            }
            pub const fn right_value(self) -> semantic_vocabulary::ValueId {
                self.right_value
            }
            pub const fn left_parameter_index(self) -> usize {
                self.left_parameter_index
            }
            pub const fn right_parameter_index(self) -> usize {
                self.right_parameter_index
            }
            pub const fn left_location(self) -> target_operations::ScalarParameterLocation {
                self.left_location
            }
            pub const fn right_location(self) -> target_operations::ScalarParameterLocation {
                self.right_location
            }
        }
    };
}

pub(super) use bitwise_parameter_receipt;
