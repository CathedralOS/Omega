//! Fixed-work atomic forms for targets admitting FEAT_LSE.
//!
//! LDAR/STLR and acquire-release LSE updates provide the sequentially
//! consistent baseline mapping. The source's requested order is checked
//! upstream; stronger native ordering never repairs an illegal source order.
//! Every update returns the value observed by that same indivisible event.
//! Subtraction negates its input before LDADD; AND complements before LDCLR;
//! CAS copies its expected input into the observed output before CASAL.
//! These prefixes preserve incoming registers and do not touch memory or NZCV.
//! There are no hidden software retries or helper calls.
//!
//! This encoding module describes LSE, not generic AArch64 availability.
//! The target's machine catalog admits it only for the Apple Silicon macOS
//! baseline; ELF remains unadmitted until it has explicit feature custody.
//! Encodings follow Arm's A64 instruction definitions, independently pinned
//! by LLVM's `llvm/test/MC/AArch64/armv8.1a-atomic.s` assembler vectors.
use crate::machine_effects::form_alternative;

use super::{Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint};
use register_model::{RegisterConstraintKey, RegisterViewId};
use selected_instructions::{
    AtomicForm, AtomicOperation, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineCallEffect,
    MachineCleanupEffect, MachineEffectDeclaration, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};

pub(crate) fn effects(form: AtomicForm) -> MachineEncodedEffects {
    MachineEncodedEffects {
        memory: MachineEncodedMemoryEffect::AtomicPointerV1 {
            pointer_operand: 0,
            form,
        },
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ..MachineEncodedEffects::fallthrough_v1(
            (0..form.operation.input_count()).collect(),
            form.observed_operand().into_iter().collect(),
        )
    }
}

pub(crate) fn declaration(
    form: AtomicForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::AtomicAccess(form),
        constraint,
        memory: MachineMemoryEffect::AtomicPointerV1,
        trap: MachineTrapBehavior::MayArchitecturalFaultV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: form_alternative!(
            MachineAlternativeFamily::AtomicAccess(form),
            MachineSizeKnowledge::ExactBytes(if has_prefix(form.operation) { 8 } else { 4 }),
            effects(form)
        ),
    }
}

pub(crate) fn footprint(
    form: AtomicForm,
    operands: &[RegisterViewId],
) -> Aarch64SelectedFormFootprint {
    Aarch64SelectedFormFootprint {
        register_reads: operands[..usize::from(form.operation.input_count())].to_vec(),
        register_writes: form
            .observed_operand()
            .map(|output| operands[usize::from(output)])
            .into_iter()
            .collect(),
        writes_nzcv: false,
        encoded: effects(form),
    }
}

const fn has_prefix(operation: AtomicOperation) -> bool {
    matches!(
        operation,
        AtomicOperation::FetchSub | AtomicOperation::FetchAnd | AtomicOperation::CompareExchange
    )
}

fn validate_registers(
    form: AtomicForm,
    registers: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if registers.len() != usize::from(form.operand_count()) {
        return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
    }
    if registers.iter().any(|register| *register > 30)
        || form.observed_operand().is_some_and(|output| {
            registers[..usize::from(output)].contains(&registers[usize::from(output)])
        })
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

pub(crate) fn encode(
    form: AtomicForm,
    registers: &[u8],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    validate_registers(form, registers)?;
    let address = u32::from(registers[0]);
    let size = u32::from(form.width.ordinal()) << 30;
    let mut words = Vec::new();
    match form.operation {
        AtomicOperation::Load => {
            words.push(size | 0x08df_fc00 | address << 5 | u32::from(registers[1]))
        }
        AtomicOperation::Store => {
            words.push(size | 0x089f_fc00 | address << 5 | u32::from(registers[1]))
        }
        AtomicOperation::CompareExchange => {
            let expected = u32::from(registers[1]);
            let replacement = u32::from(registers[2]);
            let observed = u32::from(registers[3]);
            words.push(0xaa00_03e0 | expected << 16 | observed);
            words.push(size | 0x08e0_fc00 | observed << 16 | address << 5 | replacement);
        }
        operation => {
            let mut source = u32::from(registers[1]);
            let observed = u32::from(registers[2]);
            let opcode = match operation {
                AtomicOperation::FetchSub => {
                    words.push(0xcb00_03e0 | source << 16 | observed);
                    source = observed;
                    0
                }
                AtomicOperation::FetchAnd => {
                    words.push(0xaa20_03e0 | source << 16 | observed);
                    source = observed;
                    1
                }
                AtomicOperation::FetchOr => 3,
                AtomicOperation::FetchXor => 2,
                AtomicOperation::Swap => 8,
                AtomicOperation::FetchAdd => 0,
                _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
            };
            words.push(size | 0x38e0_0000 | source << 16 | opcode << 12 | address << 5 | observed);
        }
    }
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

/// Decode the actual opcode, width and register fields. Do not accept a
/// producer-supplied effects record as evidence of the emitted operation.
pub(crate) fn validate(
    form: AtomicForm,
    registers: &[u8],
    bytes: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    validate_registers(form, registers)?;
    let expected_count = if has_prefix(form.operation) { 2 } else { 1 };
    if bytes.len() != expected_count * 4 {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let (words, _) = bytes.as_chunks::<4>();
    let register = |word: u32, shift: u32| ((word >> shift) & 31) as u8;
    if has_prefix(form.operation) {
        let prefix = u32::from_le_bytes(words[0]);
        let opcode = match form.operation {
            AtomicOperation::FetchSub => 0xcb00_03e0,
            AtomicOperation::FetchAnd => 0xaa20_03e0,
            AtomicOperation::CompareExchange => 0xaa00_03e0,
            _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
        };
        if prefix & !0x001f_001f != opcode
            || register(prefix, 16) != registers[1]
            || Some(register(prefix, 0))
                != form
                    .observed_operand()
                    .map(|output| registers[usize::from(output)])
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
    }
    let word = u32::from_le_bytes(words[expected_count - 1]);
    let same_width = (word >> 30) as u8 == form.width.ordinal();
    let same_address = register(word, 5) == registers[0];
    let matched = match form.operation {
        AtomicOperation::Load => {
            word & 0x3fff_fc00 == 0x08df_fc00 && register(word, 0) == registers[1]
        }
        AtomicOperation::Store => {
            word & 0x3fff_fc00 == 0x089f_fc00 && register(word, 0) == registers[1]
        }
        AtomicOperation::CompareExchange => {
            word & 0x3fe0_fc00 == 0x08e0_fc00
                && register(word, 16) == registers[3]
                && register(word, 0) == registers[2]
        }
        operation => {
            let opcode = match operation {
                AtomicOperation::FetchAdd | AtomicOperation::FetchSub => 0,
                AtomicOperation::FetchAnd => 1,
                AtomicOperation::FetchXor => 2,
                AtomicOperation::FetchOr => 3,
                AtomicOperation::Swap => 8,
                _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
            };
            word & 0x3fe0_fc00 == 0x38e0_0000 | opcode << 12
                && register(word, 16) == registers[if has_prefix(operation) { 2 } else { 1 }]
                && register(word, 0) == registers[2]
        }
    };
    if same_width && same_address && matched {
        Ok(())
    } else {
        Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::reject_every_bit_flip;
    use selected_instructions::{AtomicWidth, SelectedInstructionKind};

    #[test]
    fn atomic_catalog_requires_an_lse_target_and_binds_exact_effects() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let constraints = crate::validate_aarch64_register_constraint_catalog(
            crate::aarch64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap();
        for target in [
            target::NativeTarget::macos_arm64(),
            target::NativeTarget::linux_arm64(),
        ] {
            let catalog = crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
            let available = target == target::NativeTarget::macos_arm64();
            for form in AtomicForm::ALL {
                let semantic = MachineSemanticKind::AtomicAccess(form);
                assert_eq!(
                    catalog.selected_keys.for_semantic(semantic).is_some(),
                    available
                );
                assert_eq!(
                    catalog
                        .declarations
                        .iter()
                        .any(|declaration| declaration.semantic == semantic),
                    available
                );
            }
            crate::validate_aarch64_machine_effect_catalog(target, &constraints, catalog).unwrap();
        }
    }

    #[test]
    fn atomic_words_pin_lse_encoding_and_reject_every_changed_bit() {
        for form in AtomicForm::ALL {
            let registers = [2, 0, 1, 3];
            let registers = &registers[..usize::from(form.operand_count())];
            let bytes = encode(form, registers).unwrap();
            validate(form, registers, &bytes).unwrap();
            reject_every_bit_flip(&bytes, &format!("{form:?}"), |changed| {
                validate(form, registers, &changed).is_err()
            });
            assert!(validate(form, registers, &bytes[..bytes.len() - 1]).is_err());
        }
        // LLVM assembler vectors: ldeoral x0,x1,[x2]; swpal x0,x1,[x2].
        for (operation, expected) in [
            (AtomicOperation::FetchXor, [0x41, 0x20, 0xe0, 0xf8]),
            (AtomicOperation::Swap, [0x41, 0x80, 0xe0, 0xf8]),
        ] {
            assert_eq!(
                encode(
                    AtomicForm {
                        operation,
                        width: AtomicWidth::Bits64
                    },
                    &[2, 0, 1]
                )
                .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn atomic_public_encoder_binds_registers_effects_and_output_interference() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let views =
            ["x2", "x0", "x1", "x3"].map(|name| physical.model().view_named(name).unwrap().id);
        for form in AtomicForm::ALL {
            let operands = &views[..usize::from(form.operand_count())];
            let kind = SelectedInstructionKind::AtomicAccess { form };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::AtomicAccess(form),
                variant: 0,
            };
            let encoded =
                crate::encode_aarch64_selected_form(&physical, kind, alternative, operands)
                    .unwrap();
            assert_eq!(encoded.footprint(), &footprint(form, operands));
            assert!(
                crate::encode_aarch64_selected_form(
                    &physical,
                    kind,
                    MachineAlternativeKey {
                        variant: 1,
                        ..alternative
                    },
                    operands
                )
                .is_err()
            );
            if let Some(output) = form.observed_operand() {
                let mut aliased = operands.to_vec();
                aliased[usize::from(output)] = operands[0];
                assert!(
                    crate::encode_aarch64_selected_form(&physical, kind, alternative, &aliased)
                        .is_err()
                );
            }
        }
    }
}
