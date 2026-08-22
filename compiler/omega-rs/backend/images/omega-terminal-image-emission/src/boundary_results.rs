//! Final native result-placement replay for admitted boundary realizations.

use omega_calling_conventions::{MachineRegister, ValueLocation, ValuePlacement, ValueShape};
use omega_target::{Architecture, NativeTarget};
use omega_terminal_target_operations::TerminalBoundaryRealization;

pub(super) fn boundary_result_placement_is_exact(
    target: NativeTarget,
    realization: TerminalBoundaryRealization,
    placement: Option<&ValuePlacement>,
) -> bool {
    match realization {
        TerminalBoundaryRealization::MetadataOnlyPort(_) => placement.is_none(),
        TerminalBoundaryRealization::DirectPortReadU8(_) => {
            target.architecture == Architecture::X86_64
                && placement.is_some_and(|placement| {
                    placement.shape == ValueShape::integer(1, 1)
                        && placement.locations.as_slice()
                            == [ValueLocation::Register {
                                register: MachineRegister::X86Rax,
                                value_byte_offset: 0,
                                byte_size: 1,
                            }]
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_terminal_target_operations::{
        TerminalDirectPortReadU8Realization, TerminalMetadataOnlyPortRealization,
    };
    use psi_core::{OperationId, ServiceId};

    #[test]
    fn admitted_result_placement_is_exact_and_metadata_cannot_gain_one() {
        let direct =
            TerminalBoundaryRealization::DirectPortReadU8(TerminalDirectPortReadU8Realization {
                service: ServiceId::new(1).expect("service"),
                port: 0x60,
            });
        let placement = ValuePlacement {
            shape: ValueShape::integer(1, 1),
            locations: vec![ValueLocation::Register {
                register: MachineRegister::X86Rax,
                value_byte_offset: 0,
                byte_size: 1,
            }],
        };
        assert!(boundary_result_placement_is_exact(
            NativeTarget::linux_x64(),
            direct,
            Some(&placement),
        ));
        assert!(!boundary_result_placement_is_exact(
            NativeTarget::linux_arm64(),
            direct,
            Some(&placement),
        ));
        assert!(!boundary_result_placement_is_exact(
            NativeTarget::linux_x64(),
            direct,
            None,
        ));

        let metadata =
            TerminalBoundaryRealization::MetadataOnlyPort(TerminalMetadataOnlyPortRealization {
                effect_operation: OperationId::new(1).expect("operation"),
                service: ServiceId::new(1).expect("service"),
                port: 0x20,
                value: 0x20,
            });
        assert!(boundary_result_placement_is_exact(
            NativeTarget::linux_x64(),
            metadata,
            None,
        ));
        assert!(!boundary_result_placement_is_exact(
            NativeTarget::linux_x64(),
            metadata,
            Some(&placement),
        ));
    }
}
