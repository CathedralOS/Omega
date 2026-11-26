pub(crate) use target_x86_64::form_effects::atomic::{effects, writes_rflags};

use super::emit::{append_boolean, modrm, rex, rr64};
use super::{X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint};
use register_model::RegisterViewId;
use selected_instructions::{AtomicForm, AtomicOperation as Op, AtomicWidth};

pub(crate) fn footprint(
    form: AtomicForm,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    X86_64SelectedFormFootprint {
        register_reads: operands[..usize::from(form.operation.input_count())].to_vec(),
        register_writes: form
            .output_operands()
            .map(|output| operands[usize::from(output)])
            .collect(),
        writes_rflags: writes_rflags(form.operation),
        encoded: effects(form),
    }
}

const fn bits_of(width: AtomicWidth) -> u16 {
    match width {
        AtomicWidth::Bits8 => 8,
        AtomicWidth::Bits16 => 16,
        AtomicWidth::Bits32 => 32,
        AtomicWidth::Bits64 => 64,
    }
}

fn append_mem_modrm(bytes: &mut Vec<u8>, reg: u8, base: u8) {
    bytes.push(modrm(2, reg, base));
    if base & 7 == 4 {
        bytes.push(0x24);
    }
    bytes.extend(0i32.to_le_bytes());
}

fn append_prefixes(
    bytes: &mut Vec<u8>,
    bits: u16,
    locked: bool,
    operand_sized: bool,
    wide: bool,
    reg: u8,
    base: u8,
) {
    if operand_sized && bits == 16 {
        bytes.push(0x66);
    }
    if locked {
        bytes.push(0xf0);
    }
    let prefix = rex(wide, reg, 0, base);
    if prefix != 0x40 || bits == 8 {
        bytes.push(prefix);
    }
}

fn validate_registers(form: AtomicForm, regs: &[u8]) -> Result<(), Error> {
    if regs.len() != usize::from(form.operand_count()) {
        return Err(Error::OperandCountMismatch);
    }
    if regs.iter().any(|&r| r > 15) {
        return Err(Error::EncodedFormMismatch);
    }
    for out in form.output_operands() {
        let idx = usize::from(out);
        if regs[..idx].contains(&regs[idx]) {
            return Err(Error::EncodedFormMismatch);
        }
    }
    if matches!(
        form.operation,
        Op::CompareExchange | Op::CompareExchangeOnce
    ) && regs[1] != 0
    {
        return Err(Error::EncodedFormMismatch);
    }
    if form.operation.is_bitwise_fetch() && regs.contains(&0) {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(())
}

pub(crate) fn encode(form: AtomicForm, regs: &[u8]) -> Result<Vec<u8>, Error> {
    validate_registers(form, regs)?;
    let bits = bits_of(form.width);
    let base = regs[0];
    let mut bytes = Vec::new();
    match form.operation {
        Op::Load => {
            let dst = regs[1];
            append_prefixes(&mut bytes, bits, false, false, bits != 32, dst, base);
            if bits <= 16 {
                bytes.push(0x0f);
            }
            bytes.push(match bits {
                8 => 0xb6,
                16 => 0xb7,
                _ => 0x8b,
            });
            append_mem_modrm(&mut bytes, dst, base);
        }
        Op::Store => {
            let src = regs[1];
            append_prefixes(&mut bytes, bits, false, true, bits == 64, src, base);
            bytes.push(if bits == 8 { 0x88 } else { 0x89 });
            append_mem_modrm(&mut bytes, src, base);
            bytes.extend([0x0f, 0xae, 0xf0]);
        }
        Op::CompareExchange | Op::CompareExchangeOnce => {
            let repl = regs[2];
            append_prefixes(&mut bytes, bits, true, true, bits == 64, repl, base);
            bytes.extend([0x0f, if bits == 8 { 0xb0 } else { 0xb1 }]);
            append_mem_modrm(&mut bytes, repl, base);
            rr64(&mut bytes, 0x89, 0, regs[3]);
            if let Some(outcome) = form.outcome_operand() {
                append_boolean(&mut bytes, regs[usize::from(outcome)], 4);
            }
        }
        Op::Swap => {
            let dst = regs[2];
            rr64(&mut bytes, 0x89, regs[1], dst);
            append_prefixes(&mut bytes, bits, false, true, bits == 64, dst, base);
            bytes.push(if bits == 8 { 0x86 } else { 0x87 });
            append_mem_modrm(&mut bytes, dst, base);
        }
        Op::FetchAdd | Op::FetchSub => {
            let dst = regs[2];
            rr64(&mut bytes, 0x89, regs[1], dst);
            if form.operation == Op::FetchSub {
                append_prefixes(&mut bytes, bits, false, true, bits == 64, 0, dst);
                bytes.extend([if bits == 8 { 0xf6 } else { 0xf7 }, modrm(3, 3, dst)]);
            }
            append_prefixes(&mut bytes, bits, true, true, bits == 64, dst, base);
            bytes.extend([0x0f, if bits == 8 { 0xc0 } else { 0xc1 }]);
            append_mem_modrm(&mut bytes, dst, base);
        }
        Op::FetchAnd | Op::FetchOr | Op::FetchXor => {
            bytes.extend(encode(
                AtomicForm {
                    width: form.width,
                    operation: Op::Load,
                },
                &[base, 0],
            )?);
            let retry = bytes.len();
            let (mask, observed) = (regs[1], regs[2]);
            rr64(&mut bytes, 0x89, 0, observed);
            append_prefixes(&mut bytes, bits, false, true, bits == 64, mask, observed);
            let base_op = match form.operation {
                Op::FetchAnd => 0x20,
                Op::FetchOr => 0x08,
                _ => 0x30,
            };
            bytes.extend([base_op + u8::from(bits != 8), modrm(3, mask, observed)]);
            append_prefixes(&mut bytes, bits, true, true, bits == 64, observed, base);
            bytes.extend([0x0f, if bits == 8 { 0xb0 } else { 0xb1 }]);
            append_mem_modrm(&mut bytes, observed, base);
            let disp = i8::try_from(retry as isize - (bytes.len() + 2) as isize)
                .map_err(|_| Error::EncodedFormMismatch)?;
            bytes.extend([0x75, disp as u8]);
            rr64(&mut bytes, 0x89, 0, observed);
        }
    }
    Ok(bytes)
}
