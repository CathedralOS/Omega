//! Final native result-placement replay for admitted boundary realizations.

use calling_conventions::{MachineRegister, ValueLocation, ValueShape};
use machine_code::BoundaryResultRecord;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use target::{Architecture, NativeTarget};
use target_operations::BoundaryRealization;

pub(super) fn boundary_result_is_exact(
    target: NativeTarget,
    realization: BoundaryRealization,
    result: &BoundaryResultRecord,
) -> bool {
    match realization {
        BoundaryRealization::MetadataOnlyPort(_) => result.is_unit(),
        BoundaryRealization::ClaimCompletionOnly(_) => result.is_unit(),
        BoundaryRealization::LinuxWriteLine(_) => result.is_unit(),
        BoundaryRealization::LinuxExitGroupI32(_) => result.is_unit(),
        BoundaryRealization::LinuxWriteByteI32(_) => result.is_unit(),
        BoundaryRealization::DirectPortReadU8(_) => {
            target.architecture == Architecture::X86_64
                && result.scalar().is_some_and(|result| {
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
        BoundaryRealization::LinuxReadByte(_) => {
            let Some(result) = result.structural() else {
                return false;
            };
            matches!(
                target.architecture,
                Architecture::X86_64 | Architecture::Aarch64
            ) && result.layout.tag_byte_offset == 0
                && result.layout.tag_shape == ValueShape::integer(4, 4)
                && result.layout.common_fields.is_empty()
                && result.layout.payload_byte_offset == 4
                && result.layout.cases.len() == 2
                && result.layout.cases[0].fields.is_empty()
                && result.layout.cases[1].fields.len() == 1
                && result.layout.cases[1].fields[0].byte_offset == 4
                && result.layout.cases[1].fields[0].shape == ValueShape::integer(4, 4)
                && result.layout.shape == ValueShape::integer(8, 4)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::ValuePlacement;
    use machine_code::BoundaryScalarResultRecord;
    use semantic_vocabulary::{EdgeId, OperationId, PlaceId, ServiceId, StructuralTypeId};
    use target_operations::{
        DirectPortReadU8Realization, LinuxReadByteRealization, MetadataOnlyPortRealization,
    };

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
        let result = BoundaryResultRecord::Scalar(BoundaryScalarResultRecord {
            value: semantic_vocabulary::ValueId::new(1).expect("value"),
            scalar_type: ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            ),
            placement: placement.clone(),
            return_edge: EdgeId::new(1).expect("return edge"),
        });
        assert!(boundary_result_is_exact(
            NativeTarget::linux_x64(),
            direct,
            &result,
        ));
        assert!(!boundary_result_is_exact(
            NativeTarget::linux_arm64(),
            direct,
            &result,
        ));
        assert!(!boundary_result_is_exact(
            NativeTarget::linux_x64(),
            direct,
            &BoundaryResultRecord::Unit,
        ));
        let mut wrong_type = result.scalar().expect("scalar result").clone();
        wrong_type.scalar_type = ScalarType::Boolean;
        let wrong_type = BoundaryResultRecord::Scalar(wrong_type);
        assert!(!boundary_result_is_exact(
            NativeTarget::linux_x64(),
            direct,
            &wrong_type,
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
            &BoundaryResultRecord::Unit,
        ));
        assert!(!boundary_result_is_exact(
            NativeTarget::linux_x64(),
            metadata,
            &result,
        ));
    }

    #[test]
    fn linux_read_byte_requires_the_exact_structural_sum_home() {
        let layout = calling_conventions::evaluate_conventional_sum_layout(
            &[],
            &[vec![], vec![ValueShape::integer(4, 4)]],
        )
        .unwrap();
        let result =
            BoundaryResultRecord::Structural(machine_code::BoundaryStructuralResultRecord {
                defining_operation: OperationId::new(1).unwrap(),
                result: terminal_psi::StructuralOperationResult {
                    place: PlaceId::new(2).unwrap(),
                    structural_type: StructuralTypeId::new(3).unwrap(),
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                },
                layout,
                home_byte_offset: 16,
            });
        assert!(boundary_result_is_exact(
            NativeTarget::linux_x64(),
            BoundaryRealization::LinuxReadByte(LinuxReadByteRealization),
            &result,
        ));
        let mut wrong = result.structural().unwrap().clone();
        wrong.layout.payload_byte_offset = 8;
        assert!(!boundary_result_is_exact(
            NativeTarget::linux_x64(),
            BoundaryRealization::LinuxReadByte(LinuxReadByteRealization),
            &BoundaryResultRecord::Structural(wrong),
        ));
    }
}
