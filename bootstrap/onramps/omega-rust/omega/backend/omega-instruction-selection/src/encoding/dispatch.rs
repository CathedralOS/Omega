use omega_abstract_operations::StateGuardOperator;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use psi_diagnostics::Diagnostic;

pub fn encode_dispatch_loop_enter_bytes(
    architecture: Architecture,
    entry_dispatch_index: u32,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            Ok(aarch64::encode_dispatch_loop_enter_bytes(entry_dispatch_index)?.to_vec())
        }
        Architecture::X86_64 => x86_64::encode_dispatch_loop_enter_bytes(entry_dispatch_index),
    }
}

pub fn encode_dispatch_case_enter_bytes(
    architecture: Architecture,
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Ok(aarch64::encode_dispatch_case_enter_bytes(
            dispatch_index,
            skip_byte_distance,
        )?
        .to_vec()),
        Architecture::X86_64 => {
            x86_64::encode_dispatch_case_enter_bytes(dispatch_index, skip_byte_distance)
        }
    }
}

pub fn encode_dispatch_state_write_bytes(
    architecture: Architecture,
    dispatch_index: u32,
    case_leave_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Ok(aarch64::encode_dispatch_state_write_bytes(
            dispatch_index,
            case_leave_byte_distance,
        )?
        .to_vec()),
        Architecture::X86_64 => {
            x86_64::encode_dispatch_state_write_bytes(dispatch_index, case_leave_byte_distance)
        }
    }
}

pub fn encode_dispatch_case_leave_bytes(
    architecture: Architecture,
    loop_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            Ok(aarch64::encode_dispatch_case_leave_bytes(loop_byte_distance)?.to_vec())
        }
        Architecture::X86_64 => x86_64::encode_dispatch_case_leave_bytes(loop_byte_distance),
    }
}

pub fn encode_dispatch_guard_compare_static_bytes(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_guard_compare_static_bytes(
            byte_offset,
            byte_size,
            expected_value,
            skip_byte_distance,
            operator,
            is_float,
        ),
        Architecture::X86_64 => x86_64::encode_dispatch_guard_compare_static_bytes(
            byte_offset,
            byte_size,
            expected_value,
            skip_byte_distance,
            operator,
            is_float,
        ),
    }
}
