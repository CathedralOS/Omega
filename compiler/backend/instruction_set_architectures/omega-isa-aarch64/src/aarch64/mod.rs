use crate::Aarch64CallOperand;
use crate::Aarch64CallOperand::*;
use omega_core::diagnostics::Diagnostic;

mod dispatch;
mod primitives;
mod runtime_storage;
mod runtime_text;
mod widths;

pub use dispatch::*;
use primitives::*;
pub use runtime_storage::*;
pub use runtime_text::*;
pub use widths::*;

pub fn encode_host_call_sequence(operands: &[Aarch64CallOperand]) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_call_operands(operands)?;
    bytes.extend(encode_branch_link_placeholder());
    Ok(bytes)
}

pub fn encode_syscall_sequence(
    operands: &[Aarch64CallOperand],
    syscall_number: u32,
    number_register: u8,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_call_operands(operands)?;
    bytes.extend(encode_unsigned_immediate(
        number_register,
        u64::from(syscall_number),
    ));
    bytes.extend(encode_svc(supervisor_call));
    Ok(bytes)
}

pub fn encode_return() -> Vec<u8> {
    encode_instruction(0xD65F03C0)
}

fn encode_call_operands(operands: &[Aarch64CallOperand]) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();
    let mut next_register = 0u8;

    for operand in operands {
        match operand {
            ImmediateInteger(value) => {
                bytes.extend(encode_immediate(next_register, *value)?);
                next_register += 1;
            }
            DataAddress { .. } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                next_register += 1;
            }
            RuntimeMachineStringPointer { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                next_register += 1;
            }
            RuntimeMachineStringLength { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    byte_offset + 8,
                )?);
                next_register += 1;
            }
            ByteLength(value) => {
                bytes.extend(encode_unsigned_immediate(next_register, *value as u64));
                next_register += 1;
            }
        }
    }

    Ok(bytes)
}
