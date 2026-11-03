//! Hosted runtime scalar custody replay: selected byte output, hosted read and
//! write byte roots, and process-exit settlement at native boundaries.

use calling_conventions::{MachineRegister, ValueLocation, ValueShape};
use machine_code::{
    BoundaryExecutionRecord, BoundarySettlementRecord, InternalUnitScalarArgumentSourceRecord,
};
use semantic_vocabulary::{IntegerSign, ScalarType};
use target::{Architecture, NativeTarget};
use target_operations::CompilerBuiltinExecution;

pub(crate) mod process_exit;

pub(crate) fn decode_selected_byte_output(
    target: NativeTarget,
    bytes: &[u8],
) -> Option<(MachineRegister, u32)> {
    if target == NativeTarget::linux_x64() || target == NativeTarget::macos_x64() {
        isa_x86_64::decode_x86_64_selected_hosted_write_byte_i32(target, bytes)
    } else if target == NativeTarget::linux_arm64() || target == NativeTarget::macos_arm64() {
        isa_aarch64::decode_aarch64_selected_hosted_write_byte_i32(target, bytes)
    } else {
        None
    }
}

/// Byte/shape checking only. Publication separately rejoins the selected SSA
/// source and operation-owned scratch against current fragment/frame evidence.
pub(crate) fn selected_byte_output_bytes_are_exact(
    target: NativeTarget,
    settlement: &BoundarySettlementRecord,
    function_bytes: &[u8],
) -> bool {
    let [argument] = settlement.runtime_scalar_arguments.as_slice() else {
        return false;
    };
    let InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
        scratch_byte_offset,
        ..
    } = argument.source
    else {
        return false;
    };
    let Some(end) = settlement.code_offset.checked_add(settlement.byte_count) else {
        return false;
    };
    let Some(bytes) = function_bytes.get(settlement.code_offset..end) else {
        return false;
    };
    let Some((register, offset)) = decode_selected_byte_output(target, bytes) else {
        return false;
    };
    selected_byte_output_shape_is_exact(target, settlement)
        && scratch_byte_offset == u64::from(offset)
        && argument.placement.locations.as_slice()
            == [ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 4,
            }]
}

/// The one runtime scalar argument a hosted i32 boundary carries, in slot 0 at
/// the settlement's own offset, four-byte ring width and optional register.
pub(crate) fn single_i32_register_argument(
    settlement: &BoundarySettlementRecord,
    expected_register: Option<MachineRegister>,
) -> Option<&machine_code::ForeignCallScalarArgumentRecord> {
    let [argument] = settlement.runtime_scalar_arguments.as_slice() else {
        return None;
    };
    (argument.parameter_index == 0
        && argument.code_offset == settlement.code_offset
        && argument.byte_count == settlement.byte_count
        && argument.placement.shape == ValueShape::integer(4, 4)
        && matches!(
            argument.placement.locations.as_slice(),
            [ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 4,
            }] if expected_register.is_none_or(|expected| *register == expected)
        ))
    .then_some(argument)
}

/// The i32 register argument record a hosted scalar boundary settles.
pub(crate) fn hosted_i32_argument(
    source: InternalUnitScalarArgumentSourceRecord,
    register: MachineRegister,
    code_offset: usize,
    byte_count: usize,
) -> machine_code::ForeignCallScalarArgumentRecord {
    machine_code::ForeignCallScalarArgumentRecord {
        parameter_index: 0,
        source,
        placement: calling_conventions::ValuePlacement {
            shape: ValueShape::integer(4, 4),
            locations: vec![ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 4,
            }],
        },
        code_offset,
        byte_count,
    }
}

/// Everything a hosted scalar settlement must not carry beside that argument.
pub(crate) fn settlement_carries_only_runtime_scalars(
    settlement: &BoundarySettlementRecord,
) -> bool {
    settlement.native_result.is_unit()
        && settlement.scalar_arguments.is_empty()
        && settlement.arguments.is_empty()
        && settlement.byte_sequence_arguments.is_empty()
        && settlement.completion_claim_sources.is_empty()
        && settlement.completion_receipts.is_empty()
        && settlement.completion_provider_custody.is_empty()
}

/// This validates transport shape, not source authority. Installation separately
/// compares the complete record against the replay-validated executable image.
pub(crate) fn selected_byte_output_shape_is_exact(
    target: NativeTarget,
    settlement: &BoundarySettlementRecord,
) -> bool {
    let Some(argument) = single_i32_register_argument(settlement, None) else {
        return false;
    };
    let InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
        scalar_type,
        scratch_byte_offset,
        ..
    } = argument.source
    else {
        return false;
    };
    // Darwin x86-64 adds one two-byte carry branch to the Linux leaf. An
    // AArch64 leaf whose home lies past the byte store's immediate reach
    // forms its address in page steps, so its width follows the home.
    let byte_count = if target == NativeTarget::linux_x64() {
        40
    } else if target == NativeTarget::macos_x64() {
        42
    } else if target == NativeTarget::linux_arm64() || target == NativeTarget::macos_arm64() {
        let Some(byte_count) = u32::try_from(scratch_byte_offset)
            .ok()
            .and_then(|home| isa_aarch64::aarch64_hosted_write_byte_i32_byte_count(target, home))
        else {
            return false;
        };
        byte_count
    } else {
        return false;
    };
    matches!(scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed && integer.bits() == 32)
        && settlement.byte_count == byte_count
        && matches!(
            settlement.execution,
            BoundaryExecutionRecord::AdmittedProvider(_)
                | BoundaryExecutionRecord::CompilerBuiltin(
                    CompilerBuiltinExecution::HostedWriteByteI32
                )
        )
        && matches!(
            settlement.realization,
            target_operations::BoundaryRealization::HostedWriteByteI32(_)
        )
        && settlement_carries_only_runtime_scalars(settlement)
}

#[cfg(test)]
mod tests {
    use super::{MachineRegister, NativeTarget, decode_selected_byte_output};

    #[test]
    fn selected_hosted_output_decoder_binds_the_exact_operating_system() {
        let linux = [
            0x3900_03e9_u32,
            0xd280_0020,
            0x9100_03e1,
            0xd280_0022,
            0xd280_0808,
            0xd400_0001,
            0xf100_001f,
            0x5400_004c,
            0xd420_0000,
        ];
        let macos = [
            0x3900_03e9_u32,
            0xd280_0020,
            0x9100_03e1,
            0xd280_0022,
            0xd280_0090,
            0xd400_1001,
            0x5400_0062,
            0xf100_041f,
            0x5400_0040,
            0xd420_0000,
        ];
        for (target, words) in [
            (NativeTarget::linux_arm64(), linux.as_slice()),
            (NativeTarget::macos_arm64(), macos.as_slice()),
        ] {
            let bytes = words
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<_>>();
            assert_eq!(
                decode_selected_byte_output(target, &bytes),
                Some((MachineRegister::Aarch64X(9), 0))
            );
            for wrong in [
                NativeTarget::linux_x64(),
                NativeTarget::windows_x64(),
                if target == NativeTarget::linux_arm64() {
                    NativeTarget::macos_arm64()
                } else {
                    NativeTarget::linux_arm64()
                },
            ] {
                assert_eq!(
                    decode_selected_byte_output(wrong, &bytes),
                    None,
                    "wrong target {wrong:?}"
                );
            }
        }
    }
}
