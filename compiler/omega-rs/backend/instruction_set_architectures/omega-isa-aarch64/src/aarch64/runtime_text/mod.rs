mod append;
mod byte_io;
mod compare;
mod read;
mod write;

use omega_calling_conventions::MachineRegister;
use omega_core::diagnostics::Diagnostic;

#[derive(Clone, Copy)]
struct Aarch64SyscallRegisters {
    parameters: [u8; 3],
    result: u8,
    number: u8,
}

fn aarch64_syscall_registers(
    parameters: &[MachineRegister],
    result: MachineRegister,
    number: MachineRegister,
) -> Result<Aarch64SyscallRegisters, Diagnostic> {
    let [first, second, third] = parameters else {
        return Err(Diagnostic::error(format!(
            "AArch64 runtime-text syscall needs three parameter registers, found {}",
            parameters.len()
        )));
    };
    let to_x = |register: MachineRegister, role: &str| {
        let MachineRegister::Aarch64X(index) = register else {
            return Err(Diagnostic::error(format!(
                "AArch64 runtime-text syscall selected non-GPR {role} register {register:?}"
            )));
        };
        Ok(index)
    };
    Ok(Aarch64SyscallRegisters {
        parameters: [
            to_x(*first, "parameter")?,
            to_x(*second, "parameter")?,
            to_x(*third, "parameter")?,
        ],
        result: to_x(result, "result")?,
        number: to_x(number, "number")?,
    })
}

pub use append::{
    encode_runtime_text_buffer_materialize,
    encode_runtime_text_buffer_materialize_to_runtime_frame_indexed,
    encode_runtime_text_buffer_materialize_to_runtime_pointee, encode_runtime_text_literal_append,
    encode_runtime_text_literal_append_to_runtime_frame_indexed,
    encode_runtime_text_literal_append_to_runtime_pointee, encode_runtime_text_stored_place_append,
    encode_runtime_text_stored_place_append_to_runtime_frame_indexed,
    encode_runtime_text_stored_place_append_to_runtime_pointee,
    encode_runtime_text_stored_suffix_append,
};
pub use byte_io::{
    encode_runtime_byte_read_import, encode_runtime_byte_read_syscall,
    encode_runtime_byte_write_import, encode_runtime_byte_write_syscall,
};
pub use compare::{
    encode_runtime_text_literal_compare, encode_runtime_text_storage_compare_bytes,
    runtime_text_literal_compare_additional_machine_state,
    runtime_text_literal_compare_register_writes,
    runtime_text_storage_compare_additional_machine_state,
    runtime_text_storage_compare_register_writes,
};
pub use read::{
    encode_runtime_text_line_read_carrier_import, encode_runtime_text_line_read_carrier_syscall,
    encode_runtime_text_line_read_fixed_array_import,
    encode_runtime_text_line_read_fixed_array_syscall, encode_runtime_text_line_read_import,
    encode_runtime_text_line_read_syscall,
};
pub use write::{encode_runtime_text_literal_segment_write, encode_runtime_text_literal_write};
