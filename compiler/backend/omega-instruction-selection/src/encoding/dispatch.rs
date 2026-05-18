use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_target::Architecture;
use omega_target_operations::StateGuardOperator;

pub fn encode_dispatch_loop_enter(
    architecture: Architecture,
    entry_dispatch_index: u32,
) -> Result<Vec<u8>, Diagnostic> {
    Ok(Vec::from(encode_dispatch_loop_enter_bytes(
        architecture,
        entry_dispatch_index,
    )?))
}

pub fn encode_dispatch_loop_enter_bytes(
    architecture: Architecture,
    entry_dispatch_index: u32,
) -> Result<[u8; 4], Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_loop_enter_bytes(entry_dispatch_index),
        Architecture::X86_64 => unsupported_x86_64_fixed_encoding(),
    }
}

pub fn encode_dispatch_case_enter(
    architecture: Architecture,
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_dispatch_case_enter(dispatch_index, skip_byte_distance)
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_dispatch_state_write(
    architecture: Architecture,
    dispatch_index: u32,
    case_leave_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_dispatch_state_write(dispatch_index, case_leave_byte_distance)
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_dispatch_case_leave(
    architecture: Architecture,
    loop_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    Ok(Vec::from(encode_dispatch_case_leave_bytes(
        architecture,
        loop_byte_distance,
    )?))
}

pub fn encode_dispatch_case_leave_bytes(
    architecture: Architecture,
    loop_byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_case_leave_bytes(loop_byte_distance),
        Architecture::X86_64 => unsupported_x86_64_fixed_encoding(),
    }
}

pub fn encode_dispatch_guard_compare_static(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_guard_compare_static(
            byte_offset,
            byte_size,
            expected_value,
            skip_byte_distance,
            operator,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

fn unsupported_x86_64_encoding() -> Result<Vec<u8>, Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 dispatch instruction encoding is not implemented",
    ))
}

fn unsupported_x86_64_fixed_encoding() -> Result<[u8; 4], Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 dispatch instruction encoding is not implemented",
    ))
}
