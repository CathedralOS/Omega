//! Fixed-work atomic forms over the LOCK-prefixed vocabulary every x86-64
//! target has carried since the 486.
//!
//! Sequential consistency follows the usual TSO recipe: loads and narrow
//! updates are plain `MOVZX`/`MOV`, stores are `MOV` followed by `MFENCE`
//! because `XCHG` would destroy the incoming value register, swap is a plain
//! `XCHG`, fetch-add is `LOCK XADD`, fetch-sub negates the in-place register
//! before `LOCK XADD`, and compare-exchange is `LOCK CMPXCHG` with its
//! expected input pinned to RAX and the observed output copied out of RAX
//! afterward. `XADD`/`XCHG` update the same register they read, so the
//! sequence first copies the incoming value into the observed output
//! register. There are no hidden software retries or helper calls:
//! fetch-and/or/xor have no fixed-work form and stay unadmitted at this
//! encoder.
//!
//! Every memory operand is `[base + disp32]` with the displacement zero, the
//! same shape the ordinary load/store forms use; the cell effect lives in
//! `AtomicPointerV1`, not in these bytes.

use super::{X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint};
use crate::selected_form_encoding::instruction_bytes::{modrm, rex};
use register_model::RegisterViewId;
use selected_instructions::{
    AtomicForm, AtomicOperation, AtomicWidth, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedTrapBehavior,
};

/// Whether this operation's constraint row carries the RFLAGS clobber its
/// encoded effects must restate. `LOCK XADD`, `LOCK CMPXCHG` and the `NEG`
/// prefix of fetch-sub genuinely write the flags, and the update row's
/// clobber is the family's shared budget: Swap's `xchg` never touches them
/// yet still declares the same contracted interference every update row
/// allocation is planned against.
pub(crate) const fn writes_rflags(operation: AtomicOperation) -> bool {
    !matches!(operation, AtomicOperation::Load | AtomicOperation::Store)
}

pub(crate) fn effects(form: AtomicForm) -> MachineEncodedEffects {
    let mut effects = MachineEncodedEffects {
        memory: MachineEncodedMemoryEffect::AtomicPointerV1 {
            pointer_operand: 0,
            form,
        },
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ..MachineEncodedEffects::fallthrough_v1(
            (0..form.operation.input_count()).collect(),
            form.observed_operand().into_iter().collect(),
        )
    };
    let physical = crate::x86_64_physical_register_model_ref();
    let units = |name: &str| {
        physical
            .view_named(name)
            .unwrap_or_else(|| panic!("canonical x86-64 model declares {name}"))
            .units
            .clone()
    };
    if writes_rflags(form.operation) {
        effects.implicit_unit_clobbers.extend(units("rflags"));
    }
    if form.operation == AtomicOperation::CompareExchange {
        effects.implicit_unit_clobbers.extend(units("rax"));
    }
    effects.implicit_unit_clobbers.sort_unstable();
    effects
}

pub(crate) fn footprint(
    form: AtomicForm,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    X86_64SelectedFormFootprint {
        register_reads: operands[..usize::from(form.operation.input_count())].to_vec(),
        register_writes: form
            .observed_operand()
            .map(|output| operands[usize::from(output)])
            .into_iter()
            .collect(),
        writes_rflags: writes_rflags(form.operation),
        encoded: effects(form),
    }
}

/// Bits of the width-sized operand the instruction moves.
const fn operand_bits(width: AtomicWidth) -> u16 {
    match width {
        AtomicWidth::Bits8 => 8,
        AtomicWidth::Bits16 => 16,
        AtomicWidth::Bits32 => 32,
        AtomicWidth::Bits64 => 64,
    }
}

/// `[rex] opcode modrm(disp32) [sib] disp32` — the memory operand every
/// atomic form shares. The operand-size prefix (0x66 for 16-bit) and the
/// LOCK prefix sit before the REX the caller already emitted.
fn append_memory_modrm(bytes: &mut Vec<u8>, register: u8, base: u8) {
    bytes.push(modrm(2, register & 7, base & 7));
    if base & 7 == 4 {
        // rsp/r12 require a SIB; the 0x24 form names the base directly.
        bytes.push(0x24);
    }
    bytes.extend(0i32.to_le_bytes());
}

/// `mov destination, source`: the update forms produce their observed
/// output in the register the LOCK form also reads, so the incoming value
/// is copied there first.
fn copy_into_observed(bytes: &mut Vec<u8>, destination: u8, source: u8) {
    bytes.push(rex(source, 0, destination));
    bytes.push(0x89);
    bytes.push(modrm(3, source & 7, destination & 7));
}

fn append_width_prefixes(
    bytes: &mut Vec<u8>,
    width: AtomicWidth,
    locked: bool,
    operand_sized: bool,
    wide: bool,
    register: u8,
    base: u8,
) {
    if operand_sized && operand_bits(width) == 16 {
        bytes.push(0x66);
    }
    if locked {
        bytes.push(0xf0);
    }
    let byte = rex_for(wide, register, base);
    // The byte is emitted whenever a field needs extending or a W bit
    // applies; 8-bit forms also need it to reach the low views of
    // rbp/rsi/rdi, so they keep it unconditionally.
    if byte != 0x40 || operand_bits(width) == 8 {
        bytes.push(byte);
    }
}

/// REX for a register/memory pair: R extends the register field, B the
/// base. `wide` keeps the W bit for the 64-bit operand size and for the
/// `MOVZX` loads that write a full 64-bit home.
fn rex_for(wide: bool, register: u8, base: u8) -> u8 {
    let mut byte = rex(register, 0, base);
    if !wide {
        byte &= 0xf7;
    }
    byte
}

fn validate_registers(
    form: AtomicForm,
    registers: &[u8],
) -> Result<(), X86_64SelectedFormEncodingError> {
    if registers.len() != usize::from(form.operand_count()) {
        return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
    }
    if registers.iter().any(|register| *register > 15) {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    // The observed output is an early-clobber definition: it may not share
    // a register with any input the form reads, because the realization
    // writes into it before the LOCK form runs.
    if let Some(observed) = form.observed_operand() {
        let output = registers[usize::from(observed)];
        if registers[..usize::from(form.operation.input_count())].contains(&output) {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
    }
    // CMPXCHG reads the expected input from RAX: only that operand is
    // pinned; the observed output is copied out of RAX after the LOCK form.
    if form.operation == AtomicOperation::CompareExchange && registers[1] != 0 {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

pub(crate) fn encode(
    form: AtomicForm,
    registers: &[u8],
) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
    validate_registers(form, registers)?;
    let width = form.width;
    let bits = operand_bits(width);
    let base = registers[0];
    let mut bytes = Vec::new();
    match form.operation {
        AtomicOperation::Load => {
            let destination = registers[1];
            let opcode: u8 = match bits {
                8 => 0xb6,
                16 => 0xb7,
                _ => 0x8b,
            };
            // `MOVZX`/`MOV` write the full 64-bit home for every width but
            // the 32-bit form, which zero-extends the same result on its own.
            // The destination, not the memory operand, sizes this form, so
            // no 0x66 applies to the 16-bit load.
            append_width_prefixes(
                &mut bytes,
                width,
                false,
                false,
                bits != 32,
                destination,
                base,
            );
            if bits <= 16 {
                bytes.push(0x0f);
            }
            bytes.push(opcode);
            append_memory_modrm(&mut bytes, destination, base);
        }
        AtomicOperation::Store => {
            let source = registers[1];
            append_width_prefixes(&mut bytes, width, false, true, bits == 64, source, base);
            bytes.push(if bits == 8 { 0x88 } else { 0x89 });
            append_memory_modrm(&mut bytes, source, base);
            bytes.extend([0x0f, 0xae, 0xf0]);
        }
        AtomicOperation::CompareExchange => {
            let replacement = registers[2];
            append_width_prefixes(&mut bytes, width, true, true, bits == 64, replacement, base);
            bytes.extend([0x0f, if bits == 8 { 0xb0 } else { 0xb1 }]);
            append_memory_modrm(&mut bytes, replacement, base);
            // CMPXCHG leaves the observed prior cell in RAX; the observed
            // output register receives it from there.
            copy_into_observed(&mut bytes, registers[3], 0);
        }
        AtomicOperation::Swap => {
            let register = registers[2];
            copy_into_observed(&mut bytes, registers[2], registers[1]);
            append_width_prefixes(&mut bytes, width, false, true, bits == 64, register, base);
            bytes.push(if bits == 8 { 0x86 } else { 0x87 });
            append_memory_modrm(&mut bytes, register, base);
        }
        AtomicOperation::FetchAdd | AtomicOperation::FetchSub => {
            let register = registers[2];
            copy_into_observed(&mut bytes, registers[2], registers[1]);
            if form.operation == AtomicOperation::FetchSub {
                // The cell gains the negated delta while the observed output
                // still receives the prior value; `NEG` is the prefix the
                // output register absorbs before the atomic.
                append_width_prefixes(&mut bytes, width, false, true, bits == 64, 0, register);
                bytes.extend([
                    if bits == 8 { 0xf6 } else { 0xf7 },
                    modrm(3, 3, register & 7),
                ]);
            }
            append_width_prefixes(&mut bytes, width, true, true, bits == 64, register, base);
            bytes.extend([0x0f, if bits == 8 { 0xc0 } else { 0xc1 }]);
            append_memory_modrm(&mut bytes, register, base);
        }
        AtomicOperation::FetchAnd | AtomicOperation::FetchOr | AtomicOperation::FetchXor => {
            return Err(X86_64SelectedFormEncodingError::UnsupportedAtomicForm);
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use register_model::validate_physical_register_model;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
    };
    use target::NativeTarget;

    const fn form_registers(form: AtomicForm) -> [u8; 4] {
        match form.operation {
            AtomicOperation::Load => [2, 3, 0, 0],
            AtomicOperation::Store => [2, 5, 0, 0],
            AtomicOperation::CompareExchange => [2, 0, 6, 7],
            _ => [2, 5, 6, 0],
        }
    }

    #[test]
    fn atomic_catalog_admits_every_form_on_every_x86_64_target() {
        let physical =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let constraints = crate::validate_x86_64_register_constraint_catalog(
            crate::x86_64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap();
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let catalog = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
            for form in AtomicForm::ALL {
                let semantic = selected_instructions::MachineSemanticKind::AtomicAccess(form);
                assert!(catalog.selected_keys.for_semantic(semantic).is_some());
                assert!(
                    catalog
                        .declarations
                        .iter()
                        .any(|declaration| declaration.semantic == semantic),
                    "{form:?}"
                );
            }
            crate::validate_x86_64_machine_effect_catalog(target, &constraints, catalog).unwrap();
        }
    }

    #[test]
    fn atomic_words_pin_lock_encoding_and_reject_every_changed_bit() {
        let physical =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        for form in AtomicForm::ALL {
            let codes = form_registers(form);
            let registers = &codes[..usize::from(form.operand_count())];
            if matches!(
                form.operation,
                AtomicOperation::FetchAnd | AtomicOperation::FetchOr | AtomicOperation::FetchXor
            ) {
                assert_eq!(
                    encode(form, registers),
                    Err(X86_64SelectedFormEncodingError::UnsupportedAtomicForm),
                    "{form:?}"
                );
                continue;
            }
            let bytes = encode(form, registers).unwrap();
            let name = |code: u8| {
                [
                    "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10",
                    "r11", "r12", "r13", "r14", "r15",
                ][usize::from(code)]
            };
            let operands: Vec<RegisterViewId> = registers
                .iter()
                .map(|code| physical.model().view_named(name(*code)).unwrap().id)
                .collect();
            let kind = SelectedInstructionKind::AtomicAccess { form };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::AtomicAccess(form),
                variant: 0,
            };
            crate::validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &operands,
                &bytes,
            )
            .unwrap();
            for bit in 0..bytes.len() * 8 {
                let mut changed = bytes.clone();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    crate::validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &changed,
                    )
                    .is_err(),
                    "{form:?}, bit {bit}"
                );
            }
            assert!(
                crate::validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &bytes[..bytes.len() - 1],
                )
                .is_err()
            );
        }
    }

    #[test]
    fn atomic_public_encoder_binds_registers_effects_and_output_interference() {
        let physical =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let views: Vec<RegisterViewId> = ["rdx", "rax", "rbp", "rsi", "rdi"]
            .into_iter()
            .map(|name| physical.model().view_named(name).unwrap().id)
            .collect();
        for form in AtomicForm::ALL {
            let operand_views: Vec<RegisterViewId> = match form.operation {
                AtomicOperation::Load => vec![views[0], views[3]],
                AtomicOperation::Store => vec![views[0], views[3]],
                AtomicOperation::CompareExchange => {
                    vec![views[0], views[1], views[3], views[4]]
                }
                _ => vec![views[0], views[2], views[3]],
            };
            let kind = SelectedInstructionKind::AtomicAccess { form };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::AtomicAccess(form),
                variant: 0,
            };
            if matches!(
                form.operation,
                AtomicOperation::FetchAnd | AtomicOperation::FetchOr | AtomicOperation::FetchXor
            ) {
                assert_eq!(
                    crate::encode_x86_64_selected_form(
                        &physical,
                        kind,
                        alternative,
                        &operand_views
                    ),
                    Err(X86_64SelectedFormEncodingError::UnsupportedAtomicForm),
                    "{form:?}"
                );
                continue;
            }
            let encoded =
                crate::encode_x86_64_selected_form(&physical, kind, alternative, &operand_views)
                    .unwrap();
            assert_eq!(encoded.footprint(), &footprint(form, &operand_views));
            assert!(
                crate::encode_x86_64_selected_form(
                    &physical,
                    kind,
                    MachineAlternativeKey {
                        variant: 1,
                        ..alternative
                    },
                    &operand_views
                )
                .is_err()
            );
            if let Some(output) = form.observed_operand() {
                let mut aliased = operand_views.clone();
                aliased[usize::from(output)] = operand_views[0];
                assert!(
                    crate::encode_x86_64_selected_form(&physical, kind, alternative, &aliased)
                        .is_err(),
                    "{form:?}"
                );
            }
        }
    }
}
