//! Shape and independently decoded bytes do not replace selected source replay.
use super::*;

pub(crate) fn decode(target: NativeTarget, bytes: &[u8]) -> Option<MachineRegister> {
    match target.architecture {
        Architecture::X86_64 => {
            isa_x86_64::decode_x86_64_selected_hosted_exit_process_i32(target, bytes)
        }
        Architecture::Aarch64 => {
            isa_aarch64::decode_aarch64_selected_hosted_exit_process_i32(target, bytes)
        }
    }
}

pub(crate) fn shape_is_exact(target: NativeTarget, settlement: &BoundarySettlementRecord) -> bool {
    let [argument] = settlement.runtime_scalar_arguments.as_slice() else {
        return false;
    };
    let InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { scalar_type, .. } =
        argument.source
    else {
        return false;
    };
    let byte_count = if target == NativeTarget::linux_x64() {
        12
    } else if target == NativeTarget::linux_arm64() || target == NativeTarget::macos_arm64() {
        16
    } else {
        return false;
    };
    matches!(scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed && integer.bits() == 32)
        && settlement.execution
            == BoundaryExecutionRecord::CompilerBuiltin(
                CompilerBuiltinExecution::HostedExitProcessI32,
            )
        && matches!(
            settlement.realization,
            target_operations::BoundaryRealization::HostedExitProcessI32(_)
        )
        && settlement.byte_count == byte_count
        && settlement.native_result.is_unit()
        && settlement.scalar_arguments.is_empty()
        && settlement.arguments.is_empty()
        && settlement.byte_sequence_arguments.is_empty()
        && settlement.completion_claim_sources.is_empty()
        && settlement.completion_receipts.is_empty()
        && settlement.completion_provider_custody.is_empty()
        && argument.parameter_index == 0
        && argument.code_offset == settlement.code_offset
        && argument.byte_count == settlement.byte_count
        && argument.placement.shape == ValueShape::integer(4, 4)
        && matches!(
            argument.placement.locations.as_slice(),
            [ValueLocation::Register {
                value_byte_offset: 0,
                byte_size: 4,
                ..
            }]
        )
}

pub(crate) fn bytes_are_exact(
    target: NativeTarget,
    settlement: &BoundarySettlementRecord,
    function_bytes: &[u8],
) -> bool {
    if !shape_is_exact(target, settlement) {
        return false;
    }
    let Some(end) = settlement.code_offset.checked_add(settlement.byte_count) else {
        return false;
    };
    let Some(bytes) = function_bytes.get(settlement.code_offset..end) else {
        return false;
    };
    let Some(register) = decode(target, bytes) else {
        return false;
    };
    settlement.runtime_scalar_arguments[0]
        .placement
        .locations
        .as_slice()
        == [ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 4,
        }]
}
