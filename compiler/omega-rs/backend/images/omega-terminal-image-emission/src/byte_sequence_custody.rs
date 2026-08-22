//! Exact native byte-sequence settlement replay shared by object and
//! installation validation.

use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_machine_code::TerminalBoundarySettlementRecord;
use omega_terminal_target_operations::TerminalBoundaryRealization;

pub(crate) fn linux_write_line_custody_is_exact(
    target: NativeTarget,
    settlement: &TerminalBoundarySettlementRecord,
    function_bytes: Option<&[u8]>,
) -> bool {
    let TerminalBoundaryRealization::LinuxWriteLine(_) = settlement.realization else {
        return false;
    };
    let [custody] = settlement.byte_sequence_arguments.as_slice() else {
        return false;
    };
    if target.object_format != ObjectFormat::Elf
        || !matches!(
            target.architecture,
            Architecture::X86_64 | Architecture::Aarch64
        )
        || !settlement.scalar_arguments.is_empty()
        || settlement.arguments.as_slice() != [custody.argument.clone()]
        || !custody.argument.path.is_empty()
        || !matches!(
            custody.structural_type.shape,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView
            )
        )
        || settlement.native_result.is_some()
    {
        return false;
    }
    let encoded = match target.architecture {
        Architecture::X86_64 => omega_isa_x86_64::encode_linux_write_line_literal(&custody.bytes),
        Architecture::Aarch64 => omega_isa_aarch64::encode_linux_write_line_literal(&custody.bytes),
    };
    let Ok((encoded, data)) = encoded else {
        return false;
    };
    let exact_intervals = settlement.byte_count == encoded.len()
        && settlement.byte_count != 0
        && custody.code_offset == settlement.code_offset
        && custody.code_byte_count == data.start
        && custody.code_byte_count != 0
        && custody.data_offset
            == settlement
                .code_offset
                .checked_add(data.start)
                .unwrap_or(usize::MAX)
        && custody.data_byte_count == data.len()
        && custody.data_byte_count == custody.bytes.len().saturating_add(1)
        && encoded.get(data.clone()).is_some_and(|payload| {
            payload.strip_suffix(&[b'\n']) == Some(custody.bytes.as_slice())
        });
    if !exact_intervals {
        return false;
    }
    function_bytes.is_none_or(|function_bytes| {
        settlement
            .code_offset
            .checked_add(settlement.byte_count)
            .and_then(|end| function_bytes.get(settlement.code_offset..end))
            == Some(encoded.as_slice())
    })
}
