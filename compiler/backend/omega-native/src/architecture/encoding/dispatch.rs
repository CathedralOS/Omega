use crate::architecture::aarch64;
use crate::target::Architecture;
use omega_core::diagnostics::Diagnostic;

pub fn encode_dispatch_loop_enter(
    architecture: Architecture,
    entry_dispatch_index: u32,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_loop_enter(entry_dispatch_index),
        Architecture::X86_64 => Ok(Vec::new()),
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
        Architecture::X86_64 => Ok(Vec::new()),
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
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_dispatch_case_leave(
    architecture: Architecture,
    loop_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_case_leave(loop_byte_distance),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_dispatch_guard_compare_static(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_dispatch_guard_compare_static(
            byte_offset,
            byte_size,
            expected_value,
            skip_byte_distance,
            branch_when_equal,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}
