//! Shared field schema instantiated by each exact bitwise receipt leaf.

macro_rules! bitwise_parameter_receipt {
    ($name:ident, $operation:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            machine: psi_core::MachineId,
            $operation: psi_core::OperationId,
            return_edge: psi_core::EdgeId,
            source_value: psi_core::ValueId,
            scalar_type: psi_core::IntegerType,
            left_value: psi_core::ValueId,
            right_value: psi_core::ValueId,
            left_parameter_index: usize,
            right_parameter_index: usize,
            left_location: omega_target_operations::ScalarParameterLocation,
            right_location: omega_target_operations::ScalarParameterLocation,
        }

        impl $name {
            #[allow(clippy::too_many_arguments)]
            pub(in crate::validation) const fn new(
                machine: psi_core::MachineId,
                $operation: psi_core::OperationId,
                return_edge: psi_core::EdgeId,
                source_value: psi_core::ValueId,
                scalar_type: psi_core::IntegerType,
                left_value: psi_core::ValueId,
                right_value: psi_core::ValueId,
                left_parameter_index: usize,
                right_parameter_index: usize,
                left_location: omega_target_operations::ScalarParameterLocation,
                right_location: omega_target_operations::ScalarParameterLocation,
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

            pub const fn machine(self) -> psi_core::MachineId {
                self.machine
            }
            pub const fn $operation(self) -> psi_core::OperationId {
                self.$operation
            }
            pub const fn return_edge(self) -> psi_core::EdgeId {
                self.return_edge
            }
            pub const fn source_value(self) -> psi_core::ValueId {
                self.source_value
            }
            pub const fn scalar_type(self) -> psi_core::IntegerType {
                self.scalar_type
            }
            pub const fn left_value(self) -> psi_core::ValueId {
                self.left_value
            }
            pub const fn right_value(self) -> psi_core::ValueId {
                self.right_value
            }
            pub const fn left_parameter_index(self) -> usize {
                self.left_parameter_index
            }
            pub const fn right_parameter_index(self) -> usize {
                self.right_parameter_index
            }
            pub const fn left_location(self) -> omega_target_operations::ScalarParameterLocation {
                self.left_location
            }
            pub const fn right_location(self) -> omega_target_operations::ScalarParameterLocation {
                self.right_location
            }
        }
    };
}

pub(super) use bitwise_parameter_receipt;
