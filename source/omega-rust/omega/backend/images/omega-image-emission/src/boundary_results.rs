//! Final native result-placement replay for admitted boundary realizations.

use omega_calling_conventions::{MachineRegister, ValueLocation, ValueShape};
use omega_machine_code::BoundaryResultRecord;
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::BoundaryRealization;
use psi_core::{IntegerSign, IntegerType, ScalarType};

pub(super) fn boundary_result_is_exact(
    target: NativeTarget,
    realization: BoundaryRealization,
    result: Option<&BoundaryResultRecord>,
) -> bool {
    match realization {
        BoundaryRealization::MetadataOnlyPort(_) => result.is_none(),
        BoundaryRealization::ClaimCompletionOnly(_) => result.is_none(),
        BoundaryRealization::LinuxWriteLine(_) => result.is_none(),
        BoundaryRealization::LinuxExitGroupI32(_) => result.is_none(),
        BoundaryRealization::DirectPortReadU8(_) => {
            target.architecture == Architecture::X86_64
                && result.is_some_and(|result| {
                    result.scalar_type
                        == ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"),
                        )
                        && result.placement.shape == ValueShape::integer(1, 1)
                        && result.placement.locations.as_slice()
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
    use omega_calling_conventions::ValuePlacement;
    use omega_target_operations::{DirectPortReadU8Realization, MetadataOnlyPortRealization};
    use psi_core::{EdgeId, OperationId, ServiceId};

    #[test]
    fn admitted_result_placement_is_exact_and_metadata_cannot_gain_one() {
        let direct = BoundaryRealization::DirectPortReadU8(DirectPortReadU8Realization {
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
        let result = BoundaryResultRecord {
            value: psi_core::ValueId::new(1).expect("value"),
            scalar_type: ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            ),
            placement: placement.clone(),
            return_edge: EdgeId::new(1).expect("return edge"),
        };
        assert!(boundary_result_is_exact(
            NativeTarget::linux_x64(),
            direct,
            Some(&result),
        ));
        assert!(!boundary_result_is_exact(
            NativeTarget::linux_arm64(),
            direct,
            Some(&result),
        ));
        assert!(!boundary_result_is_exact(
            NativeTarget::linux_x64(),
            direct,
            None,
        ));
        let mut wrong_type = result.clone();
        wrong_type.scalar_type = ScalarType::Boolean;
        assert!(!boundary_result_is_exact(
            NativeTarget::linux_x64(),
            direct,
            Some(&wrong_type),
        ));

        let metadata = BoundaryRealization::MetadataOnlyPort(MetadataOnlyPortRealization {
            effect_operation: OperationId::new(1).expect("operation"),
            service: ServiceId::new(1).expect("service"),
            port: 0x20,
            value: 0x20,
        });
        assert!(boundary_result_is_exact(
            NativeTarget::linux_x64(),
            metadata,
            None,
        ));
        assert!(!boundary_result_is_exact(
            NativeTarget::linux_x64(),
            metadata,
            Some(&result),
        ));
    }
}
