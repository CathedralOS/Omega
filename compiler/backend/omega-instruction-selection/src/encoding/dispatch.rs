use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_target::Architecture;
use omega_target_operations::StateGuardOperator;

pub fn encode_dispatch_loop_enter_bytes(
    architecture: Architecture,
    entry_dispatch_index: u32,
) -> Result<[u8; 4], Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_loop_enter_bytes(entry_dispatch_index),
        Architecture::X86_64 => unsupported_x86_64_fixed_encoding(),
    }
}

pub fn encode_dispatch_case_enter_bytes(
    architecture: Architecture,
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<[u8; 8], Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_dispatch_case_enter_bytes(dispatch_index, skip_byte_distance)
        }
        Architecture::X86_64 => unsupported_x86_64_double_fixed_encoding(),
    }
}

pub fn encode_dispatch_state_write_bytes(
    architecture: Architecture,
    dispatch_index: u32,
    case_leave_byte_distance: isize,
) -> Result<[u8; 8], Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_dispatch_state_write_bytes(dispatch_index, case_leave_byte_distance)
        }
        Architecture::X86_64 => unsupported_x86_64_double_fixed_encoding(),
    }
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

pub fn encode_dispatch_guard_compare_static_bytes(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    operator: StateGuardOperator,
) -> Result<[u8; 20], Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_guard_compare_static_bytes(
            byte_offset,
            byte_size,
            expected_value,
            skip_byte_distance,
            operator,
        ),
        Architecture::X86_64 => unsupported_x86_64_guard_compare_static_encoding(),
    }
}

fn unsupported_x86_64_guard_compare_static_encoding() -> Result<[u8; 20], Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 dispatch instruction encoding is not implemented",
    ))
}

fn unsupported_x86_64_fixed_encoding() -> Result<[u8; 4], Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 dispatch instruction encoding is not implemented",
    ))
}

fn unsupported_x86_64_double_fixed_encoding() -> Result<[u8; 8], Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 dispatch instruction encoding is not implemented",
    ))
}
