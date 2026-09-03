use omega_calling_conventions::{MachineRegister, ValueLocation, ValueShape};
use omega_machine_code::{
    BoundaryExecutionRecord, BoundarySettlementRecord, InternalUnitScalarArgumentSourceRecord,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::CompilerBuiltinExecution;
use psi_core::{IntegerSign, IntegerType, ScalarType};

pub(crate) fn inspected_linux_read_byte_roots(
    settlements: &[BoundarySettlementRecord],
) -> std::collections::BTreeSet<psi_core::PlaceId> {
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
    settlements
        .iter()
        .filter_map(|read| {
            let result = read.native_result.structural()?;
            let exact_layout = result.layout.tag_byte_offset == 0
                && result.layout.tag_shape == ValueShape::integer(4, 4)
                && result.layout.shape == ValueShape::integer(8, 4)
                && result.layout.payload_byte_offset == 4
                && result.layout.common_fields.is_empty()
                && result.layout.cases.len() == 2
                && result.layout.cases[0].fields.is_empty()
                && result.layout.cases[1].fields.as_slice()
                    == [omega_calling_conventions::PackedFieldLayout {
                        shape: ValueShape::integer(4, 4),
                        byte_offset: 4,
                    }];
            let payload_offset = result.home_byte_offset.checked_add(4)?;
            let exact_consumer = settlements.iter().any(|write| {
                matches!(
                    write.realization,
                    omega_target_operations::BoundaryRealization::LinuxWriteByteI32(_)
                ) && write.operation_ordinal > read.operation_ordinal
                    && matches!(
                        write.runtime_scalar_arguments.as_slice(),
                        [omega_machine_code::ForeignCallScalarArgumentRecord {
                            source: InternalUnitScalarArgumentSourceRecord::Home(home),
                            ..
                        }] if home.defining_operation == result.defining_operation
                            && home.scalar_type == ScalarType::Integer(i32_type)
                            && home.shape == ValueShape::integer(4, 4)
                            && home.byte_offset == payload_offset
                    )
            });
            (matches!(
                read.realization,
                omega_target_operations::BoundaryRealization::LinuxReadByte(_)
            ) && exact_layout
                && exact_consumer)
                .then_some(result.result.place)
        })
        .collect()
}

pub(crate) fn linux_write_byte_custody_is_exact(
    target: NativeTarget,
    settlement: &BoundarySettlementRecord,
    all_settlements: &[BoundarySettlementRecord],
    integer_constants: &[omega_machine_code::UnitIntegerConstantRecord],
    scalar_homes: &[omega_machine_code::UnitScalarHomeRecord],
    preceding_home_producer_count: impl Fn(
        omega_machine_code::UnitScalarHomeRecord,
        usize,
        usize,
    ) -> usize,
    function_bytes: Option<&[u8]>,
) -> bool {
    let [argument] = settlement.runtime_scalar_arguments.as_slice() else {
        return false;
    };
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
    let expected_register = match (target.object_format, target.architecture) {
        (ObjectFormat::Elf, Architecture::X86_64) => MachineRegister::X86R11,
        (ObjectFormat::Elf, Architecture::Aarch64) => MachineRegister::Aarch64X(9),
        _ => return false,
    };
    let source_is_exact = match argument.source {
        InternalUnitScalarArgumentSourceRecord::Parameter { .. } => false,
        InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            scalar_type == i32_type
                && i32_type.admits(value)
                && integer_constants
                    .iter()
                    .filter(|constant| {
                        constant.defining_operation == defining_operation
                            && constant.source_value == source_value
                            && constant.scalar_type == scalar_type
                            && constant.value == value
                            && constant.operation_ordinal < settlement.operation_ordinal
                    })
                    .count()
                    == 1
        }
        InternalUnitScalarArgumentSourceRecord::BooleanImmediate { .. } => false,
        InternalUnitScalarArgumentSourceRecord::Home(home) => {
            let ordinary_home = scalar_homes
                .iter()
                .filter(|candidate| **candidate == home)
                .count()
                == 1
                && preceding_home_producer_count(
                    home,
                    settlement.operation_ordinal,
                    argument.code_offset,
                ) == 1;
            let inspected_payload = structural_payload_source_is_exact(
                target,
                settlement,
                all_settlements,
                home,
                function_bytes,
            );
            home.scalar_type == ScalarType::Integer(i32_type)
                && home.shape == ValueShape::integer(4, 4)
                && (ordinary_home || inspected_payload)
        }
    };
    let Some(materialization) = super::expected_foreign_scalar_argument_bytes(target, argument, 0)
    else {
        return false;
    };
    let suffix = match target.architecture {
        Architecture::X86_64 => omega_isa_x86_64::encode_linux_write_byte_i32_from_r11(),
        Architecture::Aarch64 => {
            let Ok(bytes) = omega_isa_aarch64::encode_linux_write_byte_i32_from_w9() else {
                return false;
            };
            bytes
        }
    };
    let materialization_end = argument.code_offset.checked_add(argument.byte_count);
    let settlement_end = settlement.code_offset.checked_add(settlement.byte_count);
    source_is_exact
        && settlement.execution
            == BoundaryExecutionRecord::CompilerBuiltin(CompilerBuiltinExecution::LinuxWriteByteI32)
        && settlement.scalar_arguments.is_empty()
        && settlement.arguments.is_empty()
        && settlement.byte_sequence_arguments.is_empty()
        && settlement.native_result.is_unit()
        && argument.parameter_index == 0
        && argument.placement.shape == ValueShape::integer(4, 4)
        && argument.placement.locations.as_slice()
            == [ValueLocation::Register {
                register: expected_register,
                value_byte_offset: 0,
                byte_size: 4,
            }]
        && argument.code_offset == settlement.code_offset
        && argument.byte_count == materialization.len()
        && function_bytes.is_none_or(|bytes| {
            materialization_end.and_then(|end| bytes.get(argument.code_offset..end))
                == Some(materialization.as_slice())
        })
        && materialization_end.and_then(|end| end.checked_add(suffix.len())) == settlement_end
        && function_bytes.is_none_or(|bytes| {
            materialization_end
                .and_then(|start| settlement_end.map(|end| (start, end)))
                .and_then(|(start, end)| bytes.get(start..end))
                == Some(suffix.as_slice())
        })
}

fn structural_payload_source_is_exact(
    target: NativeTarget,
    write: &BoundarySettlementRecord,
    settlements: &[BoundarySettlementRecord],
    home: omega_machine_code::UnitScalarHomeRecord,
    function_bytes: Option<&[u8]>,
) -> bool {
    let matching_reads = settlements
        .iter()
        .filter_map(|read| {
            let result = read.native_result.structural()?;
            (matches!(
                read.realization,
                omega_target_operations::BoundaryRealization::LinuxReadByte(_)
            ) && read.operation_ordinal < write.operation_ordinal
                && result.defining_operation == home.defining_operation
                && result.layout.tag_byte_offset == 0
                && result.layout.tag_shape == ValueShape::integer(4, 4)
                && result.layout.shape == ValueShape::integer(8, 4)
                && result.layout.payload_byte_offset == 4
                && result.layout.common_fields.is_empty()
                && result.layout.cases.len() == 2
                && result.layout.cases[0].fields.is_empty()
                && result.layout.cases[1].fields.as_slice()
                    == [omega_calling_conventions::PackedFieldLayout {
                        shape: ValueShape::integer(4, 4),
                        byte_offset: 4,
                    }]
                && result.home_byte_offset.checked_add(4) == Some(home.byte_offset))
            .then_some((read, result))
        })
        .collect::<Vec<_>>();
    let [(read, result)] = matching_reads.as_slice() else {
        return false;
    };
    let Some(bytes) = function_bytes else {
        return true;
    };
    let Some(start) = read.code_offset.checked_add(read.byte_count) else {
        return false;
    };
    let target_offset = settlements
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.realization,
                omega_target_operations::BoundaryRealization::LinuxExitGroupI32(_)
            ) && candidate.operation_ordinal > write.operation_ordinal
        })
        .max_by_key(|candidate| candidate.operation_ordinal)
        .map(|candidate| candidate.code_offset);
    let Some(target_offset) = target_offset else {
        return false;
    };
    let tag_offset = result
        .home_byte_offset
        .checked_add(u32::from(result.layout.tag_byte_offset));
    let Some(tag_offset) = tag_offset else {
        return false;
    };
    let mut expected = Vec::new();
    match target.architecture {
        Architecture::X86_64 => {
            if super::instruction_loads::x86_replay_rsp_load(&mut expected, 0, tag_offset, 4)
                .is_none()
            {
                return false;
            }
            expected.push(0x3d);
            expected.extend_from_slice(&1_i32.to_le_bytes());
            let branch_offset = start + expected.len();
            let Ok(target) = i64::try_from(target_offset) else {
                return false;
            };
            let Ok(next) = i64::try_from(branch_offset + 6) else {
                return false;
            };
            let Ok(displacement) = i32::try_from(target - next) else {
                return false;
            };
            expected.extend_from_slice(&[0x0f, 0x85]);
            expected.extend_from_slice(&displacement.to_le_bytes());
        }
        Architecture::Aarch64 => {
            let Some(load) = super::instruction_loads::aarch64_replay_stack_load(9, tag_offset, 4)
            else {
                return false;
            };
            expected.extend_from_slice(&load.to_le_bytes());
            expected.extend_from_slice(&(0x7100_001f_u32 | (1 << 10) | (9 << 5)).to_le_bytes());
            let branch_offset = start + expected.len();
            let Some(distance) = target_offset
                .checked_sub(branch_offset)
                .filter(|distance| distance.is_multiple_of(4))
            else {
                return false;
            };
            let words = distance / 4;
            if words > 0x3ffff {
                return false;
            }
            expected.extend_from_slice(&(0x5400_0001_u32 | ((words as u32) << 5)).to_le_bytes());
        }
    }
    write.code_offset == start + expected.len()
        && bytes.get(start..write.code_offset) == Some(expected.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_machine_code::{
        BoundarySettlementRecord, ForeignCallScalarArgumentRecord, UnitIntegerConstantRecord,
        UnitScalarHomeRecord,
    };
    use omega_target_operations::{
        BoundaryRealization, CompilerBuiltinExecution, LinuxWriteByteI32Realization,
    };
    use psi_core::{BoundaryMachineId, IntegerValue, OperationId, ValueId};

    fn immediate_case(
        target: NativeTarget,
    ) -> (
        BoundarySettlementRecord,
        Vec<UnitIntegerConstantRecord>,
        Vec<u8>,
    ) {
        let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let defining_operation = OperationId::new(1).unwrap();
        let source_value = ValueId::new(1).unwrap();
        let source = InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value: IntegerValue::Signed(75),
        };
        let register = match target.architecture {
            Architecture::X86_64 => MachineRegister::X86R11,
            Architecture::Aarch64 => MachineRegister::Aarch64X(9),
        };
        let mut argument = ForeignCallScalarArgumentRecord {
            parameter_index: 0,
            source,
            placement: omega_calling_conventions::ValuePlacement {
                shape: ValueShape::integer(4, 4),
                locations: vec![ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: 4,
                }],
            },
            code_offset: 0,
            byte_count: 0,
        };
        let materialization =
            super::super::expected_foreign_scalar_argument_bytes(target, &argument, 0).unwrap();
        argument.byte_count = materialization.len();
        let suffix = match target.architecture {
            Architecture::X86_64 => omega_isa_x86_64::encode_linux_write_byte_i32_from_r11(),
            Architecture::Aarch64 => {
                omega_isa_aarch64::encode_linux_write_byte_i32_from_w9().unwrap()
            }
        };
        let mut bytes = materialization;
        bytes.extend_from_slice(&suffix);
        (
            BoundarySettlementRecord {
                psi_operation: OperationId::new(2).unwrap(),
                boundary: BoundaryMachineId::new(1).unwrap(),
                execution: BoundaryExecutionRecord::CompilerBuiltin(
                    CompilerBuiltinExecution::LinuxWriteByteI32,
                ),
                realization: BoundaryRealization::LinuxWriteByteI32(LinuxWriteByteI32Realization),
                scalar_arguments: Vec::new(),
                runtime_scalar_arguments: vec![argument],
                arguments: Vec::new(),
                byte_sequence_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
                completion_provider_custody: Vec::new(),
                native_result: omega_machine_code::BoundaryResultRecord::Unit,
                operation_ordinal: 1,
                code_offset: 0,
                byte_count: bytes.len(),
            },
            vec![UnitIntegerConstantRecord {
                defining_operation,
                source_value,
                scalar_type,
                value: IntegerValue::Signed(75),
                operation_ordinal: 0,
            }],
            bytes,
        )
    }

    #[test]
    fn immediate_materialization_and_suffix_are_exact_on_both_linux_isas() {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (settlement, constants, bytes) = immediate_case(target);
            assert!(linux_write_byte_custody_is_exact(
                target,
                &settlement,
                &[],
                &constants,
                &[],
                |_, _, _| 0,
                Some(&bytes),
            ));

            let mut changed = bytes.clone();
            changed[0] ^= 1;
            assert!(!linux_write_byte_custody_is_exact(
                target,
                &settlement,
                &[],
                &constants,
                &[],
                |_, _, _| 0,
                Some(&changed),
            ));

            let mut changed = settlement.clone();
            changed.runtime_scalar_arguments[0].byte_count += 1;
            assert!(!linux_write_byte_custody_is_exact(
                target,
                &changed,
                &[],
                &constants,
                &[],
                |_, _, _| 0,
                Some(&bytes),
            ));

            let mut changed = settlement.clone();
            changed.execution = BoundaryExecutionRecord::CompilerBuiltin(
                CompilerBuiltinExecution::LinuxExitGroupI32,
            );
            assert!(!linux_write_byte_custody_is_exact(
                target,
                &changed,
                &[],
                &constants,
                &[],
                |_, _, _| 0,
                Some(&bytes),
            ));
        }
    }

    #[test]
    fn home_source_requires_one_exact_retained_home_and_preceding_producer() {
        let target = NativeTarget::linux_x64();
        let (mut settlement, _, _) = immediate_case(target);
        let home = UnitScalarHomeRecord {
            defining_operation: OperationId::new(3).unwrap(),
            source_value: ValueId::new(3).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
            shape: ValueShape::integer(4, 4),
            byte_offset: 16,
        };
        settlement.runtime_scalar_arguments[0].source =
            InternalUnitScalarArgumentSourceRecord::Home(home);
        let materialization = super::super::expected_foreign_scalar_argument_bytes(
            target,
            &settlement.runtime_scalar_arguments[0],
            0,
        )
        .unwrap();
        settlement.runtime_scalar_arguments[0].byte_count = materialization.len();
        let mut bytes = materialization;
        bytes.extend_from_slice(&omega_isa_x86_64::encode_linux_write_byte_i32_from_r11());
        settlement.byte_count = bytes.len();
        assert!(linux_write_byte_custody_is_exact(
            target,
            &settlement,
            &[],
            &[],
            &[home],
            |candidate, consumer_ordinal, consumer_offset| {
                usize::from(candidate == home && consumer_ordinal == 1 && consumer_offset == 0)
            },
            Some(&bytes),
        ));
        assert!(!linux_write_byte_custody_is_exact(
            target,
            &settlement,
            &[],
            &[],
            &[],
            |_, _, _| 1,
            Some(&bytes),
        ));
        assert!(!linux_write_byte_custody_is_exact(
            target,
            &settlement,
            &[],
            &[],
            &[home],
            |_, _, _| 0,
            Some(&bytes),
        ));
        assert!(!linux_write_byte_custody_is_exact(
            target,
            &settlement,
            &[],
            &[],
            &[home],
            |_, _, _| 2,
            Some(&bytes),
        ));
    }
}
