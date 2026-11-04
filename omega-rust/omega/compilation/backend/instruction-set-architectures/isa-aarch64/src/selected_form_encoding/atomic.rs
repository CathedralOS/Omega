//! Baseline AArch64 atomic events. Loads/stores use LDAR/STLR. Fetch/swap
//! and strong compare-exchange retry failed exclusive stores and return the
//! final attempt's observation. Strong compare-exchange exits on mismatch.
//! Once remains one attempt. Full barriers preserve source ordering, while
//! encoded effects expose the retry work rather than promising fixed work.

mod compare_exchange;
mod compare_exchange_once;
mod retry_update;

use super::{Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint};
use register_model::{RegisterConstraintKey, RegisterViewId};
use selected_instructions::{
    AtomicForm, AtomicOperation, MachineAlternativeFamily, MachineBarrier,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};

pub(crate) fn effects(form: AtomicForm) -> MachineEncodedEffects {
    let retry = form
        .operation
        .retries_until_commit(target::Architecture::Aarch64);
    let mut effects = MachineEncodedEffects {
        memory: MachineEncodedMemoryEffect::AtomicPointerV1 {
            pointer_operand: 0,
            form,
        },
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ..MachineEncodedEffects::fallthrough_v1(
            (0..form.operation.input_count()).collect(),
            form.output_operands().collect(),
        )
    };
    if retry {
        effects.control = MachineEncodedControlEffect::AtomicRetryUntilCommitV1;
    }
    let compares = matches!(
        form.operation,
        AtomicOperation::CompareExchange | AtomicOperation::CompareExchangeOnce
    );
    effects.implicit_unit_clobbers = ["x16", "x17"]
        .into_iter()
        .filter(|_| retry)
        .chain(compares.then_some("nzcv"))
        .flat_map(|name| {
            crate::register_model::aarch64_physical_register_model_ref()
                .view_named(name)
                .into_iter()
                .flat_map(|view| view.units.iter().copied())
        })
        .collect();
    effects.implicit_unit_clobbers.sort_unstable();
    effects
}

pub(crate) fn declaration(
    form: AtomicForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    let byte_count = match form.operation {
        AtomicOperation::Load | AtomicOperation::Store => 4,
        AtomicOperation::CompareExchangeOnce => compare_exchange_once::byte_count(form.width),
        AtomicOperation::CompareExchange => compare_exchange::byte_count(form.width),
        _ => retry_update::BYTE_COUNT,
    };
    MachineEffectDeclaration::single(
        MachineSemanticKind::AtomicAccess(form),
        constraint,
        MachineMemoryEffect::AtomicPointerV1,
        MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineBarrier::ExternalEffect,
        MachineAlternativeFamily::AtomicAccess(form),
        MachineSizeKnowledge::ExactBytes(byte_count),
        effects(form),
    )
}

pub(crate) fn footprint(
    form: AtomicForm,
    operands: &[RegisterViewId],
) -> Aarch64SelectedFormFootprint {
    Aarch64SelectedFormFootprint {
        register_reads: operands[..usize::from(form.operation.input_count())].to_vec(),
        register_writes: form
            .output_operands()
            .map(|output| operands[usize::from(output)])
            .collect(),
        writes_nzcv: matches!(
            form.operation,
            AtomicOperation::CompareExchange | AtomicOperation::CompareExchangeOnce
        ),
        encoded: effects(form),
    }
}

fn validate_registers(
    form: AtomicForm,
    registers: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if registers.len() != usize::from(form.operand_count()) {
        return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
    }
    let retry_registers: &[u8] = if form.operation == AtomicOperation::CompareExchange {
        &compare_exchange::REGISTERS
    } else {
        &retry_update::REGISTERS
    };
    if form
        .operation
        .retries_until_commit(target::Architecture::Aarch64)
        && registers != retry_registers
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    if registers.iter().any(|register| *register > 30)
        || form.output_operands().any(|output| {
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
    match form.operation {
        AtomicOperation::CompareExchangeOnce => {
            compare_exchange_once::encode(form.width, registers)
        }
        AtomicOperation::CompareExchange => Ok(compare_exchange::encode(form.width)),
        AtomicOperation::Load | AtomicOperation::Store => {
            let opcode = if form.operation == AtomicOperation::Load {
                0x08df_fc00
            } else {
                0x089f_fc00
            };
            let word = u32::from(form.width.ordinal()) << 30
                | opcode
                | u32::from(registers[0]) << 5
                | u32::from(registers[1]);
            Ok(word.to_le_bytes().to_vec())
        }
        _ => retry_update::encode(form),
    }
}

/// Validate actual opcodes and operands independently of producer effects.
pub(crate) fn validate(
    form: AtomicForm,
    registers: &[u8],
    bytes: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    validate_registers(form, registers)?;
    match form.operation {
        AtomicOperation::CompareExchangeOnce => {
            compare_exchange_once::validate(form.width, registers, bytes)
        }
        AtomicOperation::CompareExchange => compare_exchange::validate(form.width, bytes),
        AtomicOperation::Load | AtomicOperation::Store => {
            let encoded: [u8; 4] = bytes
                .try_into()
                .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
            let word = u32::from_le_bytes(encoded);
            let opcode = if form.operation == AtomicOperation::Load {
                0x08df_fc00
            } else {
                0x089f_fc00
            };
            if word & !0xc000_03ff == opcode
                && (word >> 30) as u8 == form.width.ordinal()
                && (word >> 5) & 31 == u32::from(registers[0])
                && word & 31 == u32::from(registers[1])
            {
                Ok(())
            } else {
                Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
            }
        }
        _ => retry_update::validate(form, bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::reject_every_bit_flip;
    use selected_instructions::{AtomicWidth, MachineAlternativeKey, SelectedInstructionKind};

    #[test]
    fn atomic_catalog_admits_baseline_exclusive_updates_on_both_hosts() {
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
            for form in AtomicForm::ALL {
                let available = true;
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
    fn atomic_words_pin_encoding_and_reject_every_changed_bit() {
        for form in AtomicForm::ALL {
            let ordinary = [2, 0, 1, 3, 4];
            let registers = if form.operation == AtomicOperation::CompareExchange {
                &compare_exchange::REGISTERS[..]
            } else if form
                .operation
                .retries_until_commit(target::Architecture::Aarch64)
            {
                &retry_update::REGISTERS[..]
            } else {
                &ordinary[..usize::from(form.operand_count())]
            };
            let bytes = encode(form, registers).unwrap();
            validate(form, registers, &bytes).unwrap();
            reject_every_bit_flip(&bytes, &format!("{form:?}"), |changed| {
                validate(form, registers, changed).is_err()
            });
            assert!(validate(form, registers, &bytes[..bytes.len() - 1]).is_err());
        }
        // Baseline SWP replacement is an ordinary register move inside the
        // exclusive loop, never an LSE instruction or a second memory event.
        let swapped = encode(
            AtomicForm {
                operation: AtomicOperation::Swap,
                width: AtomicWidth::Bits64,
            },
            &retry_update::REGISTERS,
        )
        .unwrap();
        assert_eq!(&swapped[8..12], &0xaa0a_03f0_u32.to_le_bytes());
    }

    #[test]
    fn single_attempt_words_match_baseline_armv8_assembler_vectors() {
        // clang -target aarch64-linux-gnu -march=armv8-a. Narrow cases must
        // truncate a sign-extended expected carrier before raw comparison.
        for (width, load, normalization, comparison, store) in [
            (
                AtomicWidth::Bits8,
                0x085f_fc03,
                Some(0x5300_1c24),
                0x6b04_007f,
                0x0804_fc02,
            ),
            (
                AtomicWidth::Bits16,
                0x485f_fc03,
                Some(0x5300_3c24),
                0x6b04_007f,
                0x4804_fc02,
            ),
            (
                AtomicWidth::Bits32,
                0x885f_fc03,
                None,
                0x6b01_007f,
                0x8804_fc02,
            ),
            (
                AtomicWidth::Bits64,
                0xc85f_fc03,
                None,
                0xeb01_007f,
                0xc804_fc02,
            ),
        ] {
            let mut words: Vec<u32> = vec![0xd503_3fbf, load];
            words.extend(normalization);
            words.extend([
                comparison,
                0x5400_0081,
                store,
                0x1100_0484,
                0x1400_0003,
                0xd503_3f5f,
                0x5280_0004,
                0xd503_3fbf,
            ]);
            let expected = words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            let form = AtomicForm {
                operation: AtomicOperation::CompareExchangeOnce,
                width,
            };
            assert_eq!(encode(form, &[0, 1, 2, 3, 4]).unwrap(), expected);
        }
    }

    #[test]
    fn atomic_public_encoder_binds_registers_effects_and_output_interference() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        for form in AtomicForm::ALL {
            let views = if form.operation == AtomicOperation::CompareExchange {
                ["x9", "x10", "x11", "x12", "x4"]
            } else if form
                .operation
                .retries_until_commit(target::Architecture::Aarch64)
            {
                ["x9", "x10", "x11", "x3", "x4"]
            } else {
                ["x2", "x0", "x1", "x3", "x4"]
            }
            .map(|name| physical.model().view_named(name).unwrap().id);
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
            for output in form.output_operands() {
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
