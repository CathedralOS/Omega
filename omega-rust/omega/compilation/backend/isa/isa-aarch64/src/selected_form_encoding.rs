/*
We encode, structurally decode, and validate all AArch64 selected instruction
forms (`encode_aarch64_selected_form` and
`validate_aarch64_selected_form_encoding`), conditional and unconditional
branches, fused compare-and-branch forms, scalar and dynamic call templates,
normalized foreign-call templates, atomic operations, IEEE floating-point
arithmetic, and hosted I/O leaves.
*/

pub(crate) mod atomic {
    mod compare_exchange {
        use super::Aarch64SelectedFormEncodingError;
        use selected_instructions::AtomicWidth;

        pub(super) const REGISTERS: [u8; 4] = [9, 10, 11, 12];
        const BARRIER: u32 = 0xd503_3fbf;
        const CLEAR_EXCLUSIVE: u32 = 0xd503_3f5f;

        pub(super) const fn byte_count(width: AtomicWidth) -> u16 {
            if matches!(width, AtomicWidth::Bits8 | AtomicWidth::Bits16) {
                40
            } else {
                36
            }
        }

        pub(super) fn encode(width: AtomicWidth) -> Vec<u8> {
            let size = u32::from(width.ordinal()) << 30;
            let mut words = vec![BARRIER, size | 0x085f_fc00 | 9 << 5 | 12];
            let expected = match width {
                AtomicWidth::Bits8 => {
                    words.push(0x5300_1c00 | 10 << 5 | 16);
                    16
                }
                AtomicWidth::Bits16 => {
                    words.push(0x5300_3c00 | 10 << 5 | 16);
                    16
                }
                AtomicWidth::Bits32 | AtomicWidth::Bits64 => 10,
            };
            let comparison = if width == AtomicWidth::Bits64 {
                0xeb00_001f
            } else {
                0x6b00_001f
            };
            let retry = if expected == 16 {
                0x35ff_ff71
            } else {
                0x35ff_ff91
            };
            words.extend([
                comparison | expected << 16 | 12 << 5,
                0x5400_0081,
                size | 0x0800_fc00 | 17 << 16 | 9 << 5 | 11,
                retry,
                0x1400_0002,
                CLEAR_EXCLUSIVE,
                BARRIER,
            ]);
            words.into_iter().flat_map(u32::to_le_bytes).collect()
        }

        pub(super) fn validate(
            width: AtomicWidth,
            bytes: &[u8],
        ) -> Result<(), Aarch64SelectedFormEncodingError> {
            let narrow = matches!(width, AtomicWidth::Bits8 | AtomicWidth::Bits16);
            let (encoded, tail) = bytes.as_chunks::<4>();
            if encoded.len() != 9 + usize::from(narrow) || !tail.is_empty() {
                return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
            }
            let words = encoded
                .iter()
                .copied()
                .map(u32::from_le_bytes)
                .collect::<Vec<_>>();
            let register = |word: u32, shift: u32| (word >> shift) & 31;
            let normalized = !narrow
                || (words[2] & 0xffff_0000 == 0x5300_0000
                    && (words[2] >> 10) & 63 == u32::from(width.bytes()) * 8 - 1
                    && register(words[2], 5) == 10
                    && register(words[2], 0) == 16);
            let suffix = &words[2 + usize::from(narrow)..];
            let comparison = if width == AtomicWidth::Bits64 {
                0xeb00_001f
            } else {
                0x6b00_001f
            };
            let matches = normalized
                && words[0] == BARRIER
                && words[1] & !0xc000_03ff == 0x085f_fc00
                && (words[1] >> 30) as u8 == width.ordinal()
                && register(words[1], 5) == 9
                && register(words[1], 0) == 12
                && suffix[0] & !0x001f_03e0 == comparison
                && register(suffix[0], 16) == if narrow { 16 } else { 10 }
                && register(suffix[0], 5) == 12
                && suffix[1] == 0x5400_0081
                && suffix[2] & !0xc01f_03ff == 0x0800_fc00
                && (suffix[2] >> 30) as u8 == width.ordinal()
                && register(suffix[2], 16) == 17
                && register(suffix[2], 5) == 9
                && register(suffix[2], 0) == 11
                && suffix[3] == if narrow { 0x35ff_ff71 } else { 0x35ff_ff91 }
                && suffix[4] == 0x1400_0002
                && suffix[5] == CLEAR_EXCLUSIVE
                && suffix[6] == BARRIER;
            if matches {
                Ok(())
            } else {
                Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
            }
        }
    }
    mod compare_exchange_once {
        use super::Aarch64SelectedFormEncodingError;
        use selected_instructions::AtomicWidth;

        const FULL_SYSTEM_BARRIER: u32 = 0xd503_3fbf;
        const CLEAR_EXCLUSIVE: u32 = 0xd503_3f5f;

        pub(super) const fn byte_count(width: AtomicWidth) -> u16 {
            if matches!(width, AtomicWidth::Bits8 | AtomicWidth::Bits16) {
                44
            } else {
                40
            }
        }

        pub(super) fn encode(
            width: AtomicWidth,
            registers: &[u8],
        ) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
            let [address, expected, replacement, observed, outcome] = registers else {
                return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
            };
            let address = u32::from(*address);
            let expected = u32::from(*expected);
            let replacement = u32::from(*replacement);
            let observed = u32::from(*observed);
            let outcome = u32::from(*outcome);
            let size = u32::from(width.ordinal()) << 30;
            let comparison = if width == AtomicWidth::Bits64 {
                0xeb00_001f
            } else {
                0x6b00_001f
            };
            let mut words = vec![
                FULL_SYSTEM_BARRIER,
                size | 0x085f_fc00 | address << 5 | observed,
            ];
            let normalization = match width {
                AtomicWidth::Bits8 => Some(0x5300_1c00),
                AtomicWidth::Bits16 => Some(0x5300_3c00),
                AtomicWidth::Bits32 | AtomicWidth::Bits64 => None,
            };
            let comparison_expected = if let Some(normalization) = normalization {
                words.push(normalization | expected << 5 | outcome);
                outcome
            } else {
                expected
            };
            words.extend([
                comparison | comparison_expected << 16 | observed << 5,
                0x5400_0081,
                size | 0x0800_fc00 | outcome << 16 | address << 5 | replacement,
                0x1100_0400 | outcome << 5 | outcome,
                0x1400_0003,
                CLEAR_EXCLUSIVE,
                0x5280_0000 | outcome,
                FULL_SYSTEM_BARRIER,
            ]);
            Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
        }

        pub(super) fn validate(
            width: AtomicWidth,
            registers: &[u8],
            bytes: &[u8],
        ) -> Result<(), Aarch64SelectedFormEncodingError> {
            let [address, expected, replacement, observed, outcome] = registers else {
                return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
            };
            let narrow = matches!(width, AtomicWidth::Bits8 | AtomicWidth::Bits16);
            let (encoded, tail) = bytes.as_chunks::<4>();
            if encoded.len() != 10 + usize::from(narrow) || !tail.is_empty() {
                return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
            }
            let words = encoded
                .iter()
                .copied()
                .map(u32::from_le_bytes)
                .collect::<Vec<_>>();
            let register = |word: u32, shift: u32| ((word >> shift) & 31) as u8;
            let comparison = if width == AtomicWidth::Bits64 {
                0xeb00_001f
            } else {
                0x6b00_001f
            };
            let normalized = !narrow
                || (words[2] & 0xffff_0000 == 0x5300_0000
                    && (words[2] >> 10) & 63 == u32::from(width.bytes()) * 8 - 1
                    && register(words[2], 5) == *expected
                    && register(words[2], 0) == *outcome);
            let completion = &words[2 + usize::from(narrow)..];
            let comparison_expected = if narrow { *outcome } else { *expected };
            let matched = normalized
                && words[0] == FULL_SYSTEM_BARRIER
                && words[1] & !0xc000_03ff == 0x085f_fc00
                && (words[1] >> 30) as u8 == width.ordinal()
                && register(words[1], 5) == *address
                && register(words[1], 0) == *observed
                && completion[0] & !0x001f_03e0 == comparison
                && register(completion[0], 16) == comparison_expected
                && register(completion[0], 5) == *observed
                && completion[1] == 0x5400_0081
                && completion[2] & !0xc01f_03ff == 0x0800_fc00
                && (completion[2] >> 30) as u8 == width.ordinal()
                && register(completion[2], 16) == *outcome
                && register(completion[2], 5) == *address
                && register(completion[2], 0) == *replacement
                && completion[3] & !0x0000_03ff == 0x1100_0400
                && register(completion[3], 5) == *outcome
                && register(completion[3], 0) == *outcome
                && completion[4] == 0x1400_0003
                && completion[5] == CLEAR_EXCLUSIVE
                && completion[6] & !0x0000_001f == 0x5280_0000
                && register(completion[6], 0) == *outcome
                && completion[7] == FULL_SYSTEM_BARRIER;
            if matched {
                Ok(())
            } else {
                Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
            }
        }
    }
    mod retry_update {
        use super::Aarch64SelectedFormEncodingError;
        use selected_instructions::{AtomicForm, AtomicOperation, AtomicWidth};

        pub(super) const BYTE_COUNT: u16 = 24;
        pub(super) const REGISTERS: [u8; 3] = [9, 10, 11];
        const BARRIER: u32 = 0xd503_3fbf;

        pub(super) fn encode(
            form: AtomicForm,
        ) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
            let logical = match form.operation {
                AtomicOperation::FetchAdd => 0x0b00_0000,
                AtomicOperation::FetchSub => 0x4b00_0000,
                AtomicOperation::Swap => 0x2a00_0000,
                AtomicOperation::FetchAnd => 0x0a00_0000,
                AtomicOperation::FetchOr => 0x2a00_0000,
                AtomicOperation::FetchXor => 0x4a00_0000,
                _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
            };
            let size = u32::from(form.width.ordinal()) << 30;
            let wide = u32::from(form.width == AtomicWidth::Bits64) << 31;
            let left = if form.operation == AtomicOperation::Swap {
                31
            } else {
                11
            };
            let words = [
                BARRIER,
                size | 0x085f_fc00 | 9 << 5 | 11,
                wide | logical | 10 << 16 | left << 5 | 16,
                size | 0x0800_fc00 | 17 << 16 | 9 << 5 | 16,
                0x35ff_ffb1,
                BARRIER,
            ];
            Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
        }

        pub(super) fn validate(
            form: AtomicForm,
            bytes: &[u8],
        ) -> Result<(), Aarch64SelectedFormEncodingError> {
            let opcode = match form.operation {
                AtomicOperation::FetchAdd => 0x0b00_0000,
                AtomicOperation::FetchSub => 0x4b00_0000,
                AtomicOperation::Swap => 0x2a00_0000,
                AtomicOperation::FetchAnd => 0x0a00_0000,
                AtomicOperation::FetchOr => 0x2a00_0000,
                AtomicOperation::FetchXor => 0x4a00_0000,
                _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
            };
            let (encoded, tail) = bytes.as_chunks::<4>();
            if encoded.len() != 6 || !tail.is_empty() {
                return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
            }
            let words: [u32; 6] = std::array::from_fn(|index| u32::from_le_bytes(encoded[index]));
            let register = |word: u32, shift: u32| (word >> shift) & 31;
            let matches = words[0] == BARRIER
                && words[1] & !0xc000_03ff == 0x085f_fc00
                && (words[1] >> 30) as u8 == form.width.ordinal()
                && register(words[1], 5) == 9
                && register(words[1], 0) == 11
                && words[2] & !0x801f_03ff == opcode
                && (words[2] >> 31 != 0) == (form.width == AtomicWidth::Bits64)
                && register(words[2], 16) == 10
                && register(words[2], 5)
                    == if form.operation == AtomicOperation::Swap {
                        31
                    } else {
                        11
                    }
                && register(words[2], 0) == 16
                && words[3] & !0xc01f_03ff == 0x0800_fc00
                && (words[3] >> 30) as u8 == form.width.ordinal()
                && register(words[3], 16) == 17
                && register(words[3], 5) == 9
                && register(words[3], 0) == 16
                && words[4] == 0x35ff_ffb1
                && words[5] == BARRIER;
            if matches {
                Ok(())
            } else {
                Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn exclusive_words_match_baseline_armv8_assembler_vectors() {
                for (width, load, store) in [
                    (AtomicWidth::Bits8, 0x085f_fd2b, 0x0811_fd30),
                    (AtomicWidth::Bits16, 0x485f_fd2b, 0x4811_fd30),
                    (AtomicWidth::Bits32, 0x885f_fd2b, 0x8811_fd30),
                    (AtomicWidth::Bits64, 0xc85f_fd2b, 0xc811_fd30),
                ] {
                    for (operation, narrow, wide) in [
                        (AtomicOperation::FetchAdd, 0x0b0a_0170, 0x8b0a_0170),
                        (AtomicOperation::FetchSub, 0x4b0a_0170, 0xcb0a_0170),
                        (AtomicOperation::Swap, 0x2a0a_03f0, 0xaa0a_03f0),
                        (AtomicOperation::FetchAnd, 0x0a0a_0170, 0x8a0a_0170),
                        (AtomicOperation::FetchOr, 0x2a0a_0170, 0xaa0a_0170),
                        (AtomicOperation::FetchXor, 0x4a0a_0170, 0xca0a_0170),
                    ] {
                        let form = AtomicForm { operation, width };
                        let logical = if width == AtomicWidth::Bits64 {
                            wide
                        } else {
                            narrow
                        };
                        let expected = [BARRIER, load, logical, store, 0x35ff_ffb1, BARRIER]
                            .into_iter()
                            .flat_map(u32::to_le_bytes)
                            .collect::<Vec<_>>();
                        assert_eq!(encode(form).unwrap(), expected);
                        validate(form, &expected).unwrap();
                    }
                }
            }
        }
    }
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
                crate::validate_aarch64_machine_effect_catalog(target, &constraints, catalog)
                    .unwrap();
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
}
pub(crate) mod atomic_fence {
    use register_model::RegisterConstraintKey;
    use selected_instructions::{
        MachineAlternativeFamily, MachineBarrier, MachineEffectDeclaration, MachineEncodedEffects,
        MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    };

    pub(crate) fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            MachineSemanticKind::AtomicFence,
            constraint,
            MachineMemoryEffect::AtomicFenceV1,
            MachineTrapBehavior::NeverV1,
            MachineBarrier::ExternalEffect,
            MachineAlternativeFamily::AtomicFence,
            MachineSizeKnowledge::ExactBytes(4),
            MachineEncodedEffects::atomic_fence_v1(),
        )
    }

    #[cfg(test)]
    mod tests {
        use crate::test_support::rejects_every_bit_flip;
        use selected_instructions::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
            SelectedInstructionKind,
        };

        #[test]
        fn atomic_fence_pins_full_barrier_bytes_and_rejects_substitutions() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let kind = SelectedInstructionKind::AtomicFence;
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::AtomicFence,
                variant: 0,
            };
            let encoded =
                crate::encode_aarch64_selected_form(&physical, kind, alternative, &[]).unwrap();
            assert_eq!(encoded.bytes(), &[0xbf, 0x3f, 0x03, 0xd5]);
            assert_eq!(
                encoded.footprint().encoded,
                MachineEncodedEffects::atomic_fence_v1()
            );
            rejects_every_bit_flip(&physical, kind, alternative, &[], encoded.bytes());
            let mut extended = encoded.bytes().to_vec();
            extended.push(0);
            for bytes in [&extended[..], &encoded.bytes()[..encoded.bytes().len() - 1]] {
                assert!(
                    crate::validate_aarch64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        bytes
                    )
                    .is_err()
                );
            }
            let operand = physical.model().view_named("x0").unwrap().id;
            assert!(
                crate::encode_aarch64_selected_form(&physical, kind, alternative, &[operand])
                    .is_err()
            );
            assert!(
                crate::encode_aarch64_selected_form(
                    &physical,
                    kind,
                    MachineAlternativeKey {
                        variant: 1,
                        ..alternative
                    },
                    &[]
                )
                .is_err()
            );
            assert!(
                crate::encode_aarch64_selected_form(
                    &physical,
                    kind,
                    MachineAlternativeKey {
                        family: MachineAlternativeFamily::Crash,
                        variant: 0
                    },
                    &[]
                )
                .is_err()
            );
        }
    }
}
#[cfg(test)]
mod boolean_materialization_tests {
    use super::{encode_aarch64_selected_form, validate_aarch64_selected_form_encoding};
    use crate::aarch64_physical_register_model;
    use crate::test_support::rejects_every_bit_flip;
    use register_model::validate_physical_register_model;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
        SelectedInstructionKind,
    };

    fn forms() -> [(SelectedInstructionKind, MachineAlternativeFamily); 5] {
        [
            (
                SelectedInstructionKind::MaterializeBooleanEqual,
                MachineAlternativeFamily::MaterializeBooleanEqual,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanU64LessThan,
                MachineAlternativeFamily::MaterializeBooleanU64LessThan,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanI64LessThan,
                MachineAlternativeFamily::MaterializeBooleanI64LessThan,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
                MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual,
            ),
        ]
    }

    #[test]
    fn conditions_bind_registers_full_width_and_preserved_flags() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        for name in ["x0", "x1", "x9", "x16", "x29", "x30"] {
            let destination = physical.model().view_named(name).unwrap().id;
            for (kind, family) in forms() {
                let key = MachineAlternativeKey { family, variant: 0 };
                let encoded =
                    encode_aarch64_selected_form(&physical, kind, key, &[destination]).unwrap();
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &[destination],
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), 4);
                let footprint = encoded.footprint();
                assert!(footprint.register_reads.is_empty());
                assert_eq!(footprint.register_writes, vec![destination]);
                assert!(!footprint.writes_nzcv);
                let mut expected = MachineEncodedEffects::fallthrough_v1(vec![], vec![0]);
                expected.implicit_unit_uses =
                    physical.model().view_named("nzcv").unwrap().units.clone();
                assert_eq!(footprint.encoded, expected);
                for (other_kind, other_family) in forms() {
                    if kind == other_kind {
                        continue;
                    }
                    let other_key = MachineAlternativeKey {
                        family: other_family,
                        variant: 0,
                    };
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical,
                            other_kind,
                            other_key,
                            &[destination],
                            encoded.bytes()
                        )
                        .is_err()
                    );
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical,
                            kind,
                            other_key,
                            &[destination],
                            encoded.bytes()
                        )
                        .is_err()
                    );
                }
                rejects_every_bit_flip(&physical, kind, key, &[destination], encoded.bytes());
                for length in 0..encoded.bytes().len() {
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical,
                            kind,
                            key,
                            &[destination],
                            &encoded.bytes()[..length]
                        )
                        .is_err()
                    );
                }
                let mut extra = encoded.bytes().to_vec();
                extra.extend_from_slice(encoded.bytes());
                assert!(
                    validate_aarch64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &[destination],
                        &extra
                    )
                    .is_err()
                );
                for invalid in ["sp", "xzr", "w0", "nzcv"] {
                    let view = physical.model().view_named(invalid).unwrap().id;
                    assert!(
                        encode_aarch64_selected_form(&physical, kind, key, &[view]).is_err(),
                        "invalid destination {invalid}"
                    );
                }
            }
        }
    }

    #[test]
    fn cset_uses_inverse_condition_and_two_zero_register_inputs() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let destination = physical.model().view_named("x0").unwrap().id;
        for ((kind, family), word) in forms().into_iter().zip([
            0x9a9f_17e0_u32,
            0x9a9f_27e0,
            0x9a9f_a7e0,
            0x9a9f_87e0,
            0x9a9f_c7e0,
        ]) {
            let key = MachineAlternativeKey { family, variant: 0 };
            assert_eq!(
                encode_aarch64_selected_form(&physical, kind, key, &[destination])
                    .unwrap()
                    .bytes(),
                word.to_le_bytes()
            );
        }
    }
}
mod branch_forms {
    use crate::aarch64_physical_register_model;
    use crate::selected_form_encoding::decoding::footprint;
    use crate::selected_form_encoding::selected_forms::resolve_registers;
    use crate::selected_form_encoding::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
        ValidatedAarch64SelectedFormEncoding,
    };
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, SelectedInstructionKind,
    };

    pub const AARCH64_CONDITIONAL_BRANCH_BYTE_REACH: std::ops::Range<i64> = -(1 << 20)..(1 << 20);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64ConditionalBranchForm {
        CanonicalWordV1,
        WidenedPairV1,
    }

    struct Aarch64ConditionalBranchArm {
        condition: u32,
        inverted_condition: u32,
        kind: SelectedInstructionKind,
    }

    fn conditional_branch_arm(
        family: MachineAlternativeFamily,
    ) -> Option<Aarch64ConditionalBranchArm> {
        let (condition, inverted_condition, kind) = match family {
            MachineAlternativeFamily::ConditionalBranchNonZero => {
                (0x1, 0x0, SelectedInstructionKind::ConditionalBranchNonZero)
            }
            MachineAlternativeFamily::ConditionalBranchU64LessThan => (
                0x3,
                0x2,
                SelectedInstructionKind::ConditionalBranchU64LessThan,
            ),
            MachineAlternativeFamily::ConditionalBranchI64LessThan => (
                0xb,
                0xa,
                SelectedInstructionKind::ConditionalBranchI64LessThan,
            ),
            _ => return None,
        };
        Some(Aarch64ConditionalBranchArm {
            condition,
            inverted_condition,
            kind,
        })
    }

    pub fn encode_aarch64_selected_conditional_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        family: MachineAlternativeFamily,
        form: Aarch64ConditionalBranchForm,
        byte_displacement_from_instruction: i64,
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let arm = conditional_branch_request(physical, alternative, family)?;
        let bytes = match form {
            Aarch64ConditionalBranchForm::CanonicalWordV1 => {
                let word_displacement =
                    branch_word_displacement(byte_displacement_from_instruction)?;
                let word =
                    0x5400_0000 | (((word_displacement as u32) & 0x7ffff) << 5) | arm.condition;
                word.to_le_bytes().to_vec()
            }
            Aarch64ConditionalBranchForm::WidenedPairV1 => {
                let word_displacement =
                    widened_branch_word_displacement(byte_displacement_from_instruction)?;
                let skip = 0x5400_0000 | (2 << 5) | arm.inverted_condition;
                let target = 0x1400_0000 | ((word_displacement as u32) & 0x03ff_ffff);
                let mut bytes = Vec::with_capacity(8);
                crate::codec_bytes::push_encoded(&mut bytes, skip.to_le_bytes());
                crate::codec_bytes::push_encoded(&mut bytes, target.to_le_bytes());
                bytes
            }
        };
        validate_aarch64_selected_conditional_branch_form(
            physical,
            alternative,
            family,
            form,
            byte_displacement_from_instruction,
            &bytes,
        )
    }

    pub fn validate_aarch64_selected_conditional_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        family: MachineAlternativeFamily,
        form: Aarch64ConditionalBranchForm,
        byte_displacement_from_instruction: i64,
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let arm = conditional_branch_request(physical, alternative, family)?;
        match form {
            Aarch64ConditionalBranchForm::CanonicalWordV1 => {
                branch_word_displacement(byte_displacement_from_instruction)?;
                let word = bytes
                    .try_into()
                    .ok()
                    .map(u32::from_le_bytes)
                    .filter(|word| word & 0xff00_001f == 0x5400_0000 | arm.condition)
                    .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
                let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
                let decoded_words = (encoded_imm19 << 13) >> 13;
                if i64::from(decoded_words) * 4 != byte_displacement_from_instruction {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
                Ok(ValidatedAarch64SelectedFormEncoding {
                    bytes: bytes.to_vec(),
                    footprint: footprint(arm.kind, &[]),
                })
            }
            Aarch64ConditionalBranchForm::WidenedPairV1 => {
                widened_branch_word_displacement(byte_displacement_from_instruction)?;
                let bytes: [u8; 8] = bytes
                    .try_into()
                    .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
                let first = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
                if first != (0x5400_0000 | (2 << 5) | arm.inverted_condition) {
                    return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
                }
                let second = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
                if second & 0xfc00_0000 != 0x1400_0000 {
                    return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
                }
                let decoded = i64::from(((second & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
                if decoded != byte_displacement_from_instruction - 4 {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
                Ok(ValidatedAarch64SelectedFormEncoding {
                    bytes: bytes.to_vec(),
                    footprint: footprint(arm.kind, &[]),
                })
            }
        }
    }

    fn conditional_branch_request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        family: MachineAlternativeFamily,
    ) -> Result<Aarch64ConditionalBranchArm, Aarch64SelectedFormEncodingError> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
        }
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
        }
        conditional_branch_arm(family).ok_or(Aarch64SelectedFormEncodingError::AlternativeMismatch)
    }

    fn widened_branch_word_displacement(
        byte_displacement: i64,
    ) -> Result<i32, Aarch64SelectedFormEncodingError> {
        if byte_displacement % 4 != 0 {
            return Err(Aarch64SelectedFormEncodingError::BranchDisplacementMisaligned);
        }
        let words = byte_displacement / 4 - 1;
        if !(-(1_i64 << 25)..(1_i64 << 25)).contains(&words) {
            return Err(Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm26);
        }
        Ok(words as i32)
    }

    fn branch_word_displacement(
        byte_displacement: i64,
    ) -> Result<i32, Aarch64SelectedFormEncodingError> {
        if byte_displacement % 4 != 0 {
            return Err(Aarch64SelectedFormEncodingError::BranchDisplacementMisaligned);
        }
        if !AARCH64_CONDITIONAL_BRANCH_BYTE_REACH.contains(&byte_displacement) {
            return Err(Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm19);
        }
        Ok((byte_displacement / 4) as i32)
    }

    pub fn encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
        physical: &ValidatedPhysicalRegisterModel,
        source: RegisterViewId,
        byte_displacement_from_instruction: i64,
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let register = validate_cbnz_request(physical, source)?;
        let word_displacement = branch_word_displacement(byte_displacement_from_instruction)?;
        let word =
            0xb500_0000 | (((word_displacement as u32) & 0x7ffff) << 5) | u32::from(register);
        validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
            physical,
            source,
            byte_displacement_from_instruction,
            &word.to_le_bytes(),
        )
    }

    pub fn validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
        physical: &ValidatedPhysicalRegisterModel,
        source: RegisterViewId,
        byte_displacement_from_instruction: i64,
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let register = validate_cbnz_request(physical, source)?;
        branch_word_displacement(byte_displacement_from_instruction)?;
        let word = bytes
            .try_into()
            .ok()
            .map(u32::from_le_bytes)
            .filter(|word| word & 0xff00_0000 == 0xb500_0000)
            .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
        if word & 0x1f != u32::from(register) {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let encoded_imm19 = ((word >> 5) & 0x7ffff) as i32;
        let decoded_words = (encoded_imm19 << 13) >> 13;
        if i64::from(decoded_words) * 4 != byte_displacement_from_instruction {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedAarch64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: cbnz_footprint(source),
        })
    }

    fn validate_cbnz_request(
        physical: &ValidatedPhysicalRegisterModel,
        source: RegisterViewId,
    ) -> Result<u8, Aarch64SelectedFormEncodingError> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
        }
        let registers = resolve_registers(physical, &[source])?;
        registers
            .first()
            .copied()
            .ok_or(Aarch64SelectedFormEncodingError::OperandCountMismatch)
    }

    fn cbnz_footprint(source: RegisterViewId) -> Aarch64SelectedFormFootprint {
        let physical = aarch64_physical_register_model();
        let pc = physical.view_named("pc").unwrap().units.clone();
        Aarch64SelectedFormFootprint {
            register_reads: vec![source],
            register_writes: vec![],
            writes_nzcv: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: vec![],
                external_operand_writes: vec![],
                implicit_unit_uses: pc.clone(),
                implicit_unit_defs: pc,
                implicit_unit_clobbers: vec![],
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            },
        }
    }
}
#[cfg(test)]
mod byte_literal_address_tests {
    use super::{
        AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET,
        AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET, encode_aarch64_selected_form,
        validate_aarch64_selected_form_encoding,
    };
    use crate::test_support::rejects_every_bit_flip;
    use register_model::validate_physical_register_model;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
        SelectedInstructionKind,
    };
    use semantic_vocabulary::{OperationId, PlaceId};

    #[test]
    fn byte_literal_address_is_an_unresolved_page_pair_into_its_destination() {
        let physical =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let views = ["x5", "x6"].map(|name| physical.model().view_named(name).unwrap().id);
        let kind = SelectedInstructionKind::ImageAddress {
            reference: selected_instructions::SelectedImageAddressReference::ByteLiteral {
                operation: OperationId::new(7).unwrap(),
                place: PlaceId::new(3).unwrap(),
            },
        };
        let key = MachineAlternativeKey {
            family: MachineAlternativeFamily::ImageAddress,
            variant: 0,
        };
        let encoded = encode_aarch64_selected_form(&physical, kind, key, &views[..1]).unwrap();
        let mut expected = 0x9000_0005_u32.to_le_bytes().to_vec();
        crate::codec_bytes::push_encoded(&mut expected, 0x9100_00a5_u32.to_le_bytes());
        assert_eq!(encoded.bytes(), expected);
        assert_eq!(AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET, 0);
        assert_eq!(AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET, 4);
        validate_aarch64_selected_form_encoding(&physical, kind, key, &views[..1], encoded.bytes())
            .unwrap();
        assert!(
            encoded
                .footprint()
                .encoded
                .external_operand_reads
                .is_empty()
        );
        assert_eq!(encoded.footprint().encoded.external_operand_writes, [0]);
        assert_eq!(
            encoded.footprint().encoded.memory,
            MachineEncodedMemoryEffect::NoneV1
        );
        assert!(
            validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                key,
                &views[1..],
                encoded.bytes()
            )
            .is_err()
        );
        rejects_every_bit_flip(&physical, kind, key, &views[..1], encoded.bytes());
    }
}
#[cfg(test)]
mod byte_view_address_tests {
    use super::{encode_aarch64_selected_form, validate_aarch64_selected_form_encoding};
    use crate::test_support::rejects_every_bit_flip;
    use register_model::validate_physical_register_model;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::MachineEncodedMemoryEffect;
    use selected_instructions::SelectedInstructionKind;

    #[test]
    fn byte_view_address_has_distinct_family_and_exact_register_bytes() {
        let physical =
            validate_physical_register_model(crate::aarch64_physical_register_model()).unwrap();
        let views =
            ["x9", "x10", "x11", "x12"].map(|name| physical.model().view_named(name).unwrap().id);
        let operands = &views[..3];
        let kind = SelectedInstructionKind::ByteViewAddress;
        let key = MachineAlternativeKey {
            family: MachineAlternativeFamily::ByteViewAddress,
            variant: 0,
        };
        let encoded = encode_aarch64_selected_form(&physical, kind, key, operands).unwrap();
        validate_aarch64_selected_form_encoding(&physical, kind, key, operands, encoded.bytes())
            .unwrap();
        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
        assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
        assert_eq!(
            encoded.footprint().encoded.memory,
            MachineEncodedMemoryEffect::NoneV1
        );
        assert!(encoded.footprint().encoded.implicit_unit_defs.is_empty());
        assert!(
            encoded
                .footprint()
                .encoded
                .implicit_unit_clobbers
                .is_empty()
        );
        let wrong_family = MachineAlternativeKey {
            family: MachineAlternativeFamily::ExactAddI64,
            variant: 0,
        };
        assert!(
            validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                wrong_family,
                operands,
                encoded.bytes()
            )
            .is_err()
        );
        for position in 0..3 {
            let mut changed = operands.to_vec();
            changed[position] = views[3];
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &changed,
                    encoded.bytes()
                )
                .is_err()
            );
        }
        rejects_every_bit_flip(&physical, kind, key, operands, encoded.bytes());
    }

    #[test]
    fn byte_view_address_arithmetic_wraps_only_exclusive_bound_empty_carrier() {
        let bound = 1_u128 << 64;
        for (backing, root_length, offset, length) in [
            (0_u64, 0_u64, 0_u64, 0_u64),
            (16, 32, 7, 25),
            (u64::MAX - 7, 8, 0, 8),
            (u64::MAX - 7, 8, 7, 1),
            (u64::MAX - 7, 8, 8, 0),
            (u64::MAX, 1, 1, 0),
        ] {
            assert!(u128::from(backing) + u128::from(root_length) <= bound);
            assert!(u128::from(offset) + u128::from(length) <= u128::from(root_length));
            let mathematical = u128::from(backing) + u128::from(offset);
            let address = backing.wrapping_add(offset);
            if mathematical == bound {
                assert_eq!(length, 0);
                assert_eq!(address, 0);
            } else {
                assert_eq!(u128::from(address), mathematical);
            }
        }
    }
}
mod compare_byte_sequences {
    use super::plan_words::{self, COND_NE, Instruction};
    use super::{
        Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding, decode_all,
    };
    use crate::selected_form_encoding::selected_forms::request_registers;
    use register_model::RegisterViewId;
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::MachineEncodedMemoryEffect;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Label {
        Loop,
        Equal,
        Done,
    }

    fn layout(registers: [u8; 8]) -> Vec<(Option<Label>, Instruction<Label>)> {
        let [
            left_base,
            left_count,
            right_base,
            right_count,
            cursor,
            left_byte,
            right_byte,
            result,
        ] = registers;
        vec![
            (None, Instruction::Zero(cursor)),
            (None, Instruction::Zero(left_byte)),
            (None, Instruction::Zero(right_byte)),
            (None, Instruction::Zero(result)),
            (
                None,
                Instruction::CompareRegister {
                    left: left_count,
                    right: right_count,
                },
            ),
            (
                None,
                Instruction::BranchConditional {
                    condition: COND_NE,
                    target: Label::Done,
                },
            ),
            (None, Instruction::Cbz(left_count, Label::Equal)),
            (
                Some(Label::Loop),
                Instruction::LdrbRegister {
                    destination: left_byte,
                    base: left_base,
                    index: cursor,
                },
            ),
            (
                None,
                Instruction::LdrbRegister {
                    destination: right_byte,
                    base: right_base,
                    index: cursor,
                },
            ),
            (
                None,
                Instruction::CompareRegister {
                    left: left_byte,
                    right: right_byte,
                },
            ),
            (
                None,
                Instruction::BranchConditional {
                    condition: COND_NE,
                    target: Label::Done,
                },
            ),
            (
                None,
                Instruction::AddImmediate {
                    destination: cursor,
                    source: cursor,
                    immediate: 1,
                },
            ),
            (
                None,
                Instruction::CompareRegister {
                    left: cursor,
                    right: left_count,
                },
            ),
            (
                None,
                Instruction::BranchConditional {
                    condition: COND_NE,
                    target: Label::Loop,
                },
            ),
            (Some(Label::Equal), Instruction::One(result)),
            (Some(Label::Done), Instruction::Zero(cursor)),
        ]
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<[u8; 8], Aarch64SelectedFormEncodingError> {
        request_registers(
            physical,
            alternative,
            MachineAlternativeFamily::CompareByteSequences,
            operands,
            4,
        )
    }

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let registers = request(physical, alternative, operands)?;
        let bytes = plan_words::assemble(&layout(registers))?;
        validate(physical, alternative, operands, &bytes)
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let registers = request(physical, alternative, operands)?;
        if decode_all(bytes, plan_words::decode_word)?
            != plan_words::expected_stream(&layout(registers))?
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        super::nzcv_effects_encoding(
            physical,
            operands,
            bytes,
            4,
            MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                left_base_operand: 0,
                left_count_operand: 1,
                right_base_operand: 2,
                right_count_operand: 3,
            },
        )
    }

    #[cfg(test)]
    mod tests {
        use super::{encode, validate};
        use crate::aarch64_physical_register_model;
        use crate::selected_form_encoding::selected_forms::{
            encode_aarch64_selected_form, validate_aarch64_selected_form_encoding,
        };
        use crate::test_support::reject_every_bit_flip;
        use selected_instructions::MachineAlternativeFamily;
        use selected_instructions::MachineAlternativeKey;
        use selected_instructions::SelectedInstructionKind;

        fn kind() -> SelectedInstructionKind {
            SelectedInstructionKind::CompareByteSequences
        }

        fn alternative() -> MachineAlternativeKey {
            MachineAlternativeKey {
                family: MachineAlternativeFamily::CompareByteSequences,
                variant: 0,
            }
        }

        #[test]
        fn constraint_catalog_declares_the_compare_register_contract() {
            let physical =
                register_model::validate_physical_register_model(aarch64_physical_register_model())
                    .unwrap();
            let catalog = crate::aarch64_register_constraint_catalog(&physical);
            let row = catalog
                .constraint(crate::AARCH64_COMPARE_BYTE_SEQUENCES)
                .expect("catalog declares the compare constraint");
            assert_eq!(row.operands.len(), 8);
            let (inputs, scratches) = row.operands.split_at(4);
            for input in inputs {
                assert!(input.access == register_model::RegisterOperandAccess::Use);
            }
            for scratch in scratches {
                assert!(scratch.early_clobber);
            }
        }

        #[test]
        fn encode_and_validate_replay_every_compare_word_for_word() {
            let physical =
                register_model::validate_physical_register_model(aarch64_physical_register_model())
                    .unwrap();
            let operands = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
                .map(|name| physical.model().view_named(name).unwrap().id);
            let alternative = alternative();
            let encoded = encode(&physical, alternative, &operands).unwrap();
            let through_selected_form =
                encode_aarch64_selected_form(&physical, kind(), alternative, &operands).unwrap();
            assert_eq!(encoded.bytes, through_selected_form.bytes);
            validate_aarch64_selected_form_encoding(
                &physical,
                kind(),
                alternative,
                &operands,
                &encoded.bytes,
            )
            .unwrap();
            reject_every_bit_flip(&encoded.bytes, "compare word stream", |changed| {
                validate(&physical, alternative, &operands, changed).is_err()
            });
        }
    }
}
mod copy_bytes {
    use super::{Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding};
    use crate::selected_form_encoding::selected_forms::request_registers;
    use register_model::RegisterViewId;
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::MachineEncodedMemoryEffect;
    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<[u8; 5], Aarch64SelectedFormEncodingError> {
        request_registers(
            physical,
            alternative,
            MachineAlternativeFamily::CopyBytes,
            operands,
            3,
        )
    }

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let [source, destination, count, cursor, byte] =
            request(physical, alternative, operands)?.map(u32::from);
        let words = [
            0xd280_0000 | cursor,
            0xb400_00c0 | count,
            0x3860_6800 | (cursor << 16) | (source << 5) | byte,
            0x3820_6800 | (cursor << 16) | (destination << 5) | byte,
            0x9100_0400 | (cursor << 5) | cursor,
            0xeb00_001f | (count << 16) | (cursor << 5),
            0x54ff_ff81,
        ];
        let bytes = words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        validate(physical, alternative, operands, &bytes)
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let [source, destination, count, cursor, byte] =
            request(physical, alternative, operands)?.map(u32::from);
        let (word_bytes, tail) = bytes.as_chunks::<4>();
        let word_bytes: &[[u8; 4]; 7] = word_bytes
            .try_into()
            .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
        if !tail.is_empty() {
            return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
        }
        let [initialize, skip, load, store, advance, compare, repeat] =
            word_bytes.map(u32::from_le_bytes);
        if initialize & !31 != 0xd280_0000
            || initialize & 31 != cursor
            || skip & 0xff00_0000 != 0xb400_0000
            || skip & 31 != count
            || (skip >> 5) & 0x7ffff != 6
            || load & 0xffe0_fc00 != 0x3860_6800
            || load & 31 != byte
            || (load >> 5) & 31 != source
            || (load >> 16) & 31 != cursor
            || store & 0xffe0_fc00 != 0x3820_6800
            || store & 31 != byte
            || (store >> 5) & 31 != destination
            || (store >> 16) & 31 != cursor
            || advance & !0x3ff != 0x9100_0400
            || advance & 31 != cursor
            || (advance >> 5) & 31 != cursor
            || compare & 0xffe0_fc1f != 0xeb00_001f
            || (compare >> 5) & 31 != cursor
            || (compare >> 16) & 31 != count
            || repeat != 0x54ff_ff81
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        super::nzcv_effects_encoding(
            physical,
            operands,
            bytes,
            3,
            MachineEncodedMemoryEffect::CopyBytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
                count_operand: 2,
            },
        )
    }

    #[cfg(test)]
    mod tests {
        use super::{encode, validate};
        use crate::aarch64_physical_register_model;
        use crate::test_support::reject_every_bit_flip;
        use selected_instructions::MachineAlternativeFamily;
        use selected_instructions::MachineAlternativeKey;

        #[test]
        fn copy_loop_replays_every_bit_and_rejects_scratch_aliases() {
            let physical =
                register_model::validate_physical_register_model(aarch64_physical_register_model())
                    .unwrap();
            let operands = ["x0", "x1", "x2", "x3", "x4"]
                .map(|name| physical.model().view_named(name).unwrap().id);
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::CopyBytes,
                variant: 0,
            };
            let encoded = encode(&physical, alternative, &operands).unwrap();
            reject_every_bit_flip(&encoded.bytes, "copy word stream", |changed| {
                validate(&physical, alternative, &operands, changed).is_err()
            });
            for scratch in 3..5 {
                for input in 0..5 {
                    if input == scratch {
                        continue;
                    }
                    let mut changed = operands;
                    changed[scratch] = changed[input];
                    assert!(encode(&physical, alternative, &changed).is_err());
                }
            }
        }
    }
}
pub(crate) mod crash {
    use register_model::RegisterInstructionConstraint;
    use selected_instructions::{
        MachineAlternativeFamily, MachineBarrier, MachineEffectDeclaration, MachineEncodedEffects,
        MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    };

    pub(crate) fn declaration(
        constraint: &RegisterInstructionConstraint,
    ) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            MachineSemanticKind::Crash,
            constraint.key,
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::ExplicitCrashV1,
            MachineBarrier::ControlFlow,
            MachineAlternativeFamily::Crash,
            MachineSizeKnowledge::ExactBytes(4),
            MachineEncodedEffects::crash_v1(&constraint.implicit_uses),
        )
    }

    #[cfg(test)]
    mod tests {
        use crate::selected_form_encoding::ValidatedAarch64SelectedFormEncoding;
        use selected_instructions::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, fixture,
        };

        impl fixture::EncodedWord for ValidatedAarch64SelectedFormEncoding {
            fn encoded_bytes(&self) -> &[u8] {
                self.bytes()
            }

            fn encoded_effects(&self) -> &MachineEncodedEffects {
                &self.footprint().encoded
            }
        }

        #[test]
        fn crash_rejects_changed_bytes_operands_and_alternatives() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            fixture::assert_crash_refusals(
                fixture::CrashWord {
                    physical: &physical,
                    bytes: &[0x00, 0x00, 0x20, 0xd4],
                    program_counter: "pc",
                    foreign_operand: "x0",
                },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::Crash,
                    variant: 0,
                },
                |kind, alternative, operands| {
                    crate::encode_aarch64_selected_form(&physical, kind, alternative, operands)
                },
                |kind, alternative, operands, bytes| {
                    crate::validate_aarch64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative,
                        operands,
                        bytes,
                    )
                },
            );
        }
    }
}
pub(crate) mod decoding {
    use crate::aarch64_physical_register_model_ref;
    use crate::saturating_forms::SaturatingRealization;
    use crate::selected_form_encoding::selected_forms::BOUND_WORDS;
    use crate::selected_form_encoding::selected_forms::{integer_bits, u12};
    use crate::selected_form_encoding::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
    };
    use register_model::RegisterViewId;
    use selected_instructions::{
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedInstructionKind,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum DecodedWord {
        RegisterAccess {
            operation: selected_instructions::RegisterOperation,
            operand: u8,
        },
        AtomicFence,
        Crash,
        BranchOverTrap {
            condition: u8,
        },
        BranchNonZeroOverTrap {
            register: u8,
        },
        CompareExtended {
            left: u8,
            right: u8,
            extension: u8,
        },
        CompareNegativeOne {
            source: u8,
        },
        ConditionalCompareOneOnEqual {
            register: u8,
        },
        TestHighBits {
            source: u8,
            from_bit: u8,
        },
        SignedDivide {
            dividend: u8,
            divisor: u8,
            destination: u8,
        },
        MultiplySubtract {
            left: u8,
            right: u8,
            minuend: u8,
            destination: u8,
        },
        Multiply {
            left: u8,
            right: u8,
            destination: u8,
        },
        SignedMultiplyHigh {
            left: u8,
            right: u8,
            destination: u8,
        },
        UnsignedMultiplyHigh {
            left: u8,
            right: u8,
            destination: u8,
        },
        CompareWithSignOf {
            high: u8,
            low: u8,
        },
        SelectMaximumUnlessEqual {
            source: u8,
            destination: u8,
        },
        UnsignedDivide {
            dividend: u8,
            divisor: u8,
            destination: u8,
        },
        ShiftLeftVariable {
            value: u8,
            count: u8,
            destination: u8,
        },
        ShiftRightLogicalVariable {
            value: u8,
            count: u8,
            destination: u8,
        },
        ShiftRightArithmeticVariable {
            value: u8,
            count: u8,
            destination: u8,
        },
        AddWithFlags {
            left: u8,
            right: u8,
            destination: u8,
        },
        SelectMaximumOnCarry {
            source: u8,
            destination: u8,
        },
        SubtractWithFlags {
            left: u8,
            right: u8,
            destination: u8,
        },
        SelectZeroOnBorrow {
            source: u8,
            destination: u8,
        },
        MaterializeBound {
            destination: u8,
            value: u64,
        },
        SelectOnEqual {
            source: u8,
            destination: u8,
        },
        SelectOnOverflow {
            source: u8,
            destination: u8,
        },
        ArithmeticShiftRight63 {
            source: u8,
            destination: u8,
        },
        ExclusiveOrI64Maximum {
            register: u8,
        },
        ConditionalCompareMinusOne {
            register: u8,
        },
        SelectOnGreater {
            source: u8,
            destination: u8,
        },
        SelectOnLess {
            source: u8,
            destination: u8,
        },
        SelectOnNotEqual {
            source: u8,
            destination: u8,
        },
        SelectOnHigher {
            source: u8,
            destination: u8,
        },
        BitwiseXor {
            left: u8,
            right: u8,
            destination: u8,
        },
        BitwiseOr {
            left: u8,
            right: u8,
            destination: u8,
        },
        BitwiseNot {
            source: u8,
            destination: u8,
        },
        BitwiseAnd {
            left: u8,
            right: u8,
            destination: u8,
        },
        SetBoolean {
            condition: u8,
            destination: u8,
        },
        ZeroExtendU16 {
            source: u8,
            destination: u8,
        },
        SignExtendI8 {
            source: u8,
            destination: u8,
        },
        SignExtendI16 {
            source: u8,
            destination: u8,
        },
        SignExtendI32 {
            source: u8,
            destination: u8,
        },
        ZeroExtendU8 {
            source: u8,
            destination: u8,
        },
        ZeroExtendU32 {
            source: u8,
            destination: u8,
        },
        MovN {
            register: u8,
            shift: u8,
            immediate: u16,
        },
        MovZ {
            register: u8,
            shift: u8,
            immediate: u16,
        },
        MovK {
            register: u8,
            shift: u8,
            immediate: u16,
        },
        Copy {
            source: u8,
            destination: u8,
        },
        CompareZero {
            source: u8,
        },
        CompareImmediate {
            source: u8,
            immediate: u16,
        },
        Compare {
            left: u8,
            right: u8,
        },
        Add {
            left: u8,
            right: u8,
            destination: u8,
        },
        AddImmediate {
            source: u8,
            immediate: u16,
            destination: u8,
        },
        Subtract {
            left: u8,
            right: u8,
            destination: u8,
        },
        SubtractImmediate {
            source: u8,
            immediate: u16,
            destination: u8,
        },
        Return,
        UnresolvedPageAddress {
            destination: u8,
        },
    }

    pub(crate) fn decode_words(
        bytes: &[u8],
    ) -> Result<Vec<DecodedWord>, Aarch64SelectedFormEncodingError> {
        if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
            return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
        }
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| decode_word(u32::from_le_bytes(*bytes)))
            .collect()
    }

    const CONVERSION_MASK: u32 = 0xffff_fc00;

    #[rustfmt::skip]
const CONVERSIONS: [(u32, fn(u8, u8) -> DecodedWord); 6] = [
    (0xd340_1c00, |source, destination| DecodedWord::ZeroExtendU8 { source, destination }),
    (0xd340_3c00, |source, destination| DecodedWord::ZeroExtendU16 { source, destination }),
    (0x9340_1c00, |source, destination| DecodedWord::SignExtendI8 { source, destination }),
    (0x9340_3c00, |source, destination| DecodedWord::SignExtendI16 { source, destination }),
    (0x9340_7c00, |source, destination| DecodedWord::SignExtendI32 { source, destination }),
    (0xd340_7c00, |source, destination| DecodedWord::ZeroExtendU32 { source, destination }),
];

    fn decode_word(word: u32) -> Result<DecodedWord, Aarch64SelectedFormEncodingError> {
        macro_rules! source_destination {
            ($name:ident) => {
                Ok(DecodedWord::$name {
                    source: ((word >> 5) & 31) as u8,
                    destination: (word & 31) as u8,
                })
            };
        }

        if let Some((operation, operand)) = super::register_access::decode(word) {
            return Ok(DecodedWord::RegisterAccess { operation, operand });
        }
        if word == 0xd503_3fbf {
            return Ok(DecodedWord::AtomicFence);
        }
        if word == 0xd420_0000 {
            return Ok(DecodedWord::Crash);
        }
        if word & 0xffff_ffe0 == 0x9000_0000 {
            return Ok(DecodedWord::UnresolvedPageAddress {
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffff_fff0 == 0x5400_0040 {
            return Ok(DecodedWord::BranchOverTrap {
                condition: (word & 15) as u8,
            });
        }
        if word & 0xffff_ffe0 == 0xb500_0040 {
            return Ok(DecodedWord::BranchNonZeroOverTrap {
                register: (word & 31) as u8,
            });
        }
        if word & 0xffe0_1c1f == 0xeb20_001f && matches!((word >> 13) & 7, 0 | 1 | 2 | 4 | 5 | 6) {
            return Ok(DecodedWord::CompareExtended {
                left: ((word >> 5) & 31) as u8,
                right: ((word >> 16) & 31) as u8,
                extension: ((word >> 13) & 7) as u8,
            });
        }
        if word & 0xffff_fc1f == 0xb100_041f {
            return Ok(DecodedWord::CompareNegativeOne {
                source: ((word >> 5) & 31) as u8,
            });
        }
        if word & 0xffff_fc1f == 0xfa41_0800 {
            return Ok(DecodedWord::ConditionalCompareOneOnEqual {
                register: ((word >> 5) & 31) as u8,
            });
        }
        if word & 0xffc0_001f == 0xf240_001f {
            let rotation = (word >> 16) & 63;
            let ones = ((word >> 10) & 63) + 1;
            if rotation != 0 && rotation == ones {
                return Ok(DecodedWord::TestHighBits {
                    source: ((word >> 5) & 31) as u8,
                    from_bit: (64 - ones) as u8,
                });
            }
        }
        if word & 0xffe0_fc00 == 0x9ac0_0c00 {
            return Ok(DecodedWord::SignedDivide {
                dividend: ((word >> 5) & 31) as u8,
                divisor: ((word >> 16) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffe0_8000 == 0x9b00_8000 {
            return Ok(DecodedWord::MultiplySubtract {
                left: ((word >> 5) & 31) as u8,
                right: ((word >> 16) & 31) as u8,
                minuend: ((word >> 10) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffe0_fc00 == 0x9b00_7c00 {
            return Ok(DecodedWord::Multiply {
                left: ((word >> 5) & 31) as u8,
                right: ((word >> 16) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffe0_fc00 == 0x9b40_7c00 {
            return Ok(DecodedWord::SignedMultiplyHigh {
                left: ((word >> 5) & 31) as u8,
                right: ((word >> 16) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffe0_fc00 == 0x9bc0_7c00 {
            return Ok(DecodedWord::UnsignedMultiplyHigh {
                left: ((word >> 5) & 31) as u8,
                right: ((word >> 16) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffe0_fc1f == 0xeb80_fc1f {
            return Ok(DecodedWord::CompareWithSignOf {
                high: ((word >> 5) & 31) as u8,
                low: ((word >> 16) & 31) as u8,
            });
        }
        if word & 0xffff_fc00 == 0xda9f_0000 {
            return source_destination!(SelectMaximumUnlessEqual);
        }
        if word & 0xffe0_fc00 == 0x9ac0_0800 {
            return Ok(DecodedWord::UnsignedDivide {
                dividend: ((word >> 5) & 31) as u8,
                divisor: ((word >> 16) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffe0_fc00 == 0x9ac0_2000 {
            return Ok(DecodedWord::ShiftLeftVariable {
                value: ((word >> 5) & 31) as u8,
                count: ((word >> 16) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffe0_fc00 == 0x9ac0_2400 {
            return Ok(DecodedWord::ShiftRightLogicalVariable {
                value: ((word >> 5) & 31) as u8,
                count: ((word >> 16) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffe0_fc00 == 0x9ac0_2800 {
            return Ok(DecodedWord::ShiftRightArithmeticVariable {
                value: ((word >> 5) & 31) as u8,
                count: ((word >> 16) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffe0_fc00 == 0xab00_0000 && word & 31 != 31 {
            return Ok(DecodedWord::AddWithFlags {
                left: ((word >> 5) & 31) as u8,
                right: ((word >> 16) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffff_fc00 == 0xda9f_3000 {
            return source_destination!(SelectMaximumOnCarry);
        }
        if word & 0xffe0_fc00 == 0xeb00_0000 && word & 31 != 31 {
            return Ok(DecodedWord::SubtractWithFlags {
                left: ((word >> 5) & 31) as u8,
                right: ((word >> 16) & 31) as u8,
                destination: (word & 31) as u8,
            });
        }
        if word & 0xffff_fc00 == 0x9a9f_2000 {
            return source_destination!(SelectZeroOnBorrow);
        }
        if let Some((value, _)) = BOUND_WORDS
            .iter()
            .find(|(_, bound_word)| word & 0xffff_ffe0 == *bound_word)
        {
            return Ok(DecodedWord::MaterializeBound {
                destination: (word & 31) as u8,
                value: *value,
            });
        }
        if word & 0xffe0_fc00 == 0x9a80_0000 && (word >> 16) & 31 == word & 31 {
            return source_destination!(SelectOnEqual);
        }
        if word & 0xffe0_fc00 == 0x9a80_6000 && (word >> 16) & 31 == word & 31 {
            return source_destination!(SelectOnOverflow);
        }
        if word & 0xffff_fc00 == 0x937f_fc00 {
            return source_destination!(ArithmeticShiftRight63);
        }
        if word & 0xffff_fc00 == 0xd240_f800 && (word >> 5) & 31 == word & 31 {
            return Ok(DecodedWord::ExclusiveOrI64Maximum {
                register: (word & 31) as u8,
            });
        }
        if word & 0xffff_fc1f == 0xba41_0800 {
            return Ok(DecodedWord::ConditionalCompareMinusOne {
                register: ((word >> 5) & 31) as u8,
            });
        }
        if word & 0xffe0_fc00 == 0x9a80_c000 && (word >> 16) & 31 == word & 31 {
            return source_destination!(SelectOnGreater);
        }
        if word & 0xffe0_fc00 == 0x9a80_b000 && (word >> 16) & 31 == word & 31 {
            return source_destination!(SelectOnLess);
        }
        if word & 0xffe0_fc00 == 0x9a80_1000 && (word >> 16) & 31 == word & 31 {
            return source_destination!(SelectOnNotEqual);
        }
        if word & 0xffe0_fc00 == 0x9a80_8000 && (word >> 16) & 31 == word & 31 {
            return source_destination!(SelectOnHigher);
        }
        if word & 0xffff_0fe0 == 0x9a9f_07e0 {
            let condition = (((word >> 12) & 15) ^ 1) as u8;
            if matches!(condition, 0 | 3 | 9 | 11 | 13) {
                return Ok(DecodedWord::SetBoolean {
                    condition,
                    destination: (word & 31) as u8,
                });
            }
        }

        for &(value, build) in &CONVERSIONS {
            if word & CONVERSION_MASK == value {
                return Ok(build(((word >> 5) & 31) as u8, (word & 31) as u8));
            }
        }
        let register = (word & 0x1f) as u8;
        let shift = ((word >> 21) & 0x3) as u8;
        let immediate = ((word >> 5) & 0xffff) as u16;
        if word & 0xff80_0000 == 0x9280_0000 {
            return Ok(DecodedWord::MovN {
                register,
                shift,
                immediate,
            });
        }
        if word & 0xffe0_0000 == 0xd280_0000 {
            return Ok(DecodedWord::MovZ {
                register,
                shift,
                immediate,
            });
        }
        if word & 0xff80_0000 == 0xf280_0000 {
            return Ok(DecodedWord::MovK {
                register,
                shift,
                immediate,
            });
        }
        if word & 0xffe0_ffe0 == 0xaa00_03e0 {
            return Ok(DecodedWord::Copy {
                source: ((word >> 16) & 0x1f) as u8,
                destination: register,
            });
        }
        if word & 0xffe0_fc00 == 0xaa00_0000 {
            return Ok(DecodedWord::BitwiseOr {
                left: ((word >> 5) & 0x1f) as u8,
                right: ((word >> 16) & 0x1f) as u8,
                destination: register,
            });
        }
        if word & 0xffe0_ffe0 == 0xaa20_03e0 {
            return Ok(DecodedWord::BitwiseNot {
                source: ((word >> 16) & 0x1f) as u8,
                destination: register,
            });
        }
        if word & 0xffff_fc1f == 0xf100_001f {
            return Ok(DecodedWord::CompareZero {
                source: ((word >> 5) & 0x1f) as u8,
            });
        }
        if word & 0xffc0_001f == 0xf100_001f {
            return Ok(DecodedWord::CompareImmediate {
                source: ((word >> 5) & 0x1f) as u8,
                immediate: ((word >> 10) & 0xfff) as u16,
            });
        }
        if word & 0xffe0_fc1f == 0xeb00_001f {
            return Ok(DecodedWord::Compare {
                left: ((word >> 5) & 0x1f) as u8,
                right: ((word >> 16) & 0x1f) as u8,
            });
        }
        if word & 0xffe0_fc00 == 0x8a00_0000 {
            return Ok(DecodedWord::BitwiseAnd {
                left: ((word >> 5) & 0x1f) as u8,
                right: ((word >> 16) & 0x1f) as u8,
                destination: register,
            });
        }
        if word & 0xffe0_fc00 == 0xca00_0000 {
            return Ok(DecodedWord::BitwiseXor {
                left: ((word >> 5) & 0x1f) as u8,
                right: ((word >> 16) & 0x1f) as u8,
                destination: register,
            });
        }
        if word & 0xffe0_fc00 == 0x8b00_0000 {
            return Ok(DecodedWord::Add {
                left: ((word >> 5) & 0x1f) as u8,
                right: ((word >> 16) & 0x1f) as u8,
                destination: register,
            });
        }
        if word & 0xffc0_0000 == 0x9100_0000 {
            return Ok(DecodedWord::AddImmediate {
                source: ((word >> 5) & 0x1f) as u8,
                immediate: ((word >> 10) & 0xfff) as u16,
                destination: register,
            });
        }
        if word & 0xffe0_fc00 == 0xcb00_0000 {
            return Ok(DecodedWord::Subtract {
                left: ((word >> 5) & 0x1f) as u8,
                right: ((word >> 16) & 0x1f) as u8,
                destination: register,
            });
        }
        if word & 0xffc0_0000 == 0xd100_0000 {
            return Ok(DecodedWord::SubtractImmediate {
                source: ((word >> 5) & 0x1f) as u8,
                immediate: ((word >> 10) & 0xfff) as u16,
                destination: register,
            });
        }
        if word == 0xd65f_03c0 {
            return Ok(DecodedWord::Return);
        }
        Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
    }

    pub(crate) fn validate_decoded(
        kind: SelectedInstructionKind,
        registers: &[u8],
        decoded: &[DecodedWord],
    ) -> Result<(), Aarch64SelectedFormEncodingError> {
        let valid = match kind {
            SelectedInstructionKind::RegisterAccess { operation } => {
                matches!((registers, decoded), ([operand], [DecodedWord::RegisterAccess { operation: actual, operand: actual_operand }])
                if operation == *actual && operand == actual_operand)
            }
            SelectedInstructionKind::PrivilegedCrash
            | SelectedInstructionKind::InstalledInterruptCpuCheck { .. }
            | SelectedInstructionKind::InstalledInterruptTaskCommit { .. }
            | SelectedInstructionKind::InstalledInterruptTablePublication { .. }
            | SelectedInstructionKind::PortWrite { .. } => false,
            SelectedInstructionKind::AccessorScalarAccess {
                form, byte_offset, ..
            } => {
                let ordinary = form
                    .ordinary_kind(byte_offset)
                    .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
                return validate_decoded(ordinary, registers, decoded);
            }
            SelectedInstructionKind::AtomicAccess { .. } => false,
            SelectedInstructionKind::NearestIeeeFloatArithmetic { .. } => false,
            SelectedInstructionKind::FloatResultPolicy { .. } => false,
            SelectedInstructionKind::AtomicFence => {
                registers.is_empty() && decoded == [DecodedWord::AtomicFence]
            }
            SelectedInstructionKind::Crash => {
                registers.is_empty() && decoded == [DecodedWord::Crash]
            }
            SelectedInstructionKind::TrappingInteger { form } => {
                decoded == super::trapping_forms::realization(form, registers)
            }
            SelectedInstructionKind::ImageAddress { .. } => {
                decoded
                    == [
                        DecodedWord::UnresolvedPageAddress {
                            destination: registers[0],
                        },
                        DecodedWord::AddImmediate {
                            source: registers[0],
                            immediate: 0,
                            destination: registers[0],
                        },
                    ]
            }
            SelectedInstructionKind::MaterializeBooleanEqual
            | SelectedInstructionKind::MaterializeBooleanU64LessThan
            | SelectedInstructionKind::MaterializeBooleanI64LessThan
            | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
            | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
                let condition = match kind {
                    SelectedInstructionKind::MaterializeBooleanEqual => 0,
                    SelectedInstructionKind::MaterializeBooleanU64LessThan => 3,
                    SelectedInstructionKind::MaterializeBooleanI64LessThan => 11,
                    SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => 9,
                    SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => 13,
                    _ => unreachable!("Boolean condition arm"),
                };
                decoded
                    == [DecodedWord::SetBoolean {
                        condition,
                        destination: registers[0],
                    }]
            }
            SelectedInstructionKind::ZeroExtendU8 => {
                decoded
                    == [DecodedWord::ZeroExtendU8 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            SelectedInstructionKind::ZeroExtendU16 => {
                decoded
                    == [DecodedWord::ZeroExtendU16 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            SelectedInstructionKind::SignExtendI8 => {
                decoded
                    == [DecodedWord::SignExtendI8 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            SelectedInstructionKind::SignExtendI16 => {
                decoded
                    == [DecodedWord::SignExtendI16 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            SelectedInstructionKind::SignExtendI32 => {
                decoded
                    == [DecodedWord::SignExtendI32 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            SelectedInstructionKind::ZeroExtendU32 => {
                decoded
                    == [DecodedWord::ZeroExtendU32 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            SelectedInstructionKind::MaterializeI64 { value } => {
                decode_materialization(decoded, registers[0]) == integer_bits(value).ok()
            }
            SelectedInstructionKind::CopyI64 => {
                decoded
                    == [DecodedWord::Copy {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            SelectedInstructionKind::CompareI64Zero => {
                decoded
                    == [DecodedWord::CompareZero {
                        source: registers[0],
                    }]
            }
            SelectedInstructionKind::CompareI64Immediate { immediate } => {
                let immediate = u12(immediate)?;
                decoded
                    == [if immediate == 0 {
                        DecodedWord::CompareZero {
                            source: registers[0],
                        }
                    } else {
                        DecodedWord::CompareImmediate {
                            source: registers[0],
                            immediate,
                        }
                    }]
            }
            SelectedInstructionKind::CompareI64 => {
                decoded
                    == [DecodedWord::Compare {
                        left: registers[0],
                        right: registers[1],
                    }]
            }
            SelectedInstructionKind::ByteViewAddress
            | SelectedInstructionKind::WrappingAddI64
            | SelectedInstructionKind::ExactAddI64 { .. } => {
                decoded
                    == [DecodedWord::Add {
                        left: registers[0],
                        right: registers[1],
                        destination: registers[2],
                    }]
            }
            SelectedInstructionKind::BitwiseAndI64 => {
                decoded
                    == [DecodedWord::BitwiseAnd {
                        left: registers[0],
                        right: registers[1],
                        destination: registers[2],
                    }]
            }
            SelectedInstructionKind::BitwiseOrI64 => {
                decoded
                    == [DecodedWord::BitwiseOr {
                        left: registers[0],
                        right: registers[1],
                        destination: registers[2],
                    }]
            }
            SelectedInstructionKind::BitwiseNotI64 => {
                decoded
                    == [DecodedWord::BitwiseNot {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            selected_instructions::saturating_add_or_subtract_pattern!()
            | SelectedInstructionKind::SaturatingDivide { .. }
            | SelectedInstructionKind::SaturatingMultiply { .. } => {
                let realization = SaturatingRealization::of_kind(kind)
                    .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
                let distinct_outputs = realization.operand_count() == 3
                    || (!registers[..2].contains(&registers[2])
                        && !registers[..3].contains(&registers[3]));
                distinct_outputs && decoded == expected_saturating(realization, registers)
            }
            SelectedInstructionKind::SaturatingRemainder { .. } => {
                let realization = SaturatingRealization::of_kind(kind)
                    .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
                registers[2] != registers[0]
                    && registers[2] != registers[1]
                    && decoded == expected_saturating(realization, registers)
            }
            SelectedInstructionKind::ExactDivideU64 { .. } => {
                decoded
                    == [DecodedWord::UnsignedDivide {
                        dividend: registers[0],
                        divisor: registers[1],
                        destination: registers[2],
                    }]
            }
            SelectedInstructionKind::WrappingDivideI64 { .. }
            | SelectedInstructionKind::ExactDivideI64 { .. } => {
                decoded
                    == [DecodedWord::SignedDivide {
                        dividend: registers[0],
                        divisor: registers[1],
                        destination: registers[2],
                    }]
            }
            SelectedInstructionKind::ExactRemainderU64 { .. } => {
                registers[2] != registers[0]
                    && registers[2] != registers[1]
                    && decoded
                        == [
                            DecodedWord::UnsignedDivide {
                                dividend: registers[0],
                                divisor: registers[1],
                                destination: registers[2],
                            },
                            DecodedWord::MultiplySubtract {
                                left: registers[2],
                                right: registers[1],
                                minuend: registers[0],
                                destination: registers[2],
                            },
                        ]
            }
            SelectedInstructionKind::WrappingRemainderI64 { .. }
            | SelectedInstructionKind::ExactRemainderI64 { .. } => {
                registers[2] != registers[0]
                    && registers[2] != registers[1]
                    && decoded
                        == [
                            DecodedWord::SignedDivide {
                                dividend: registers[0],
                                divisor: registers[1],
                                destination: registers[2],
                            },
                            DecodedWord::MultiplySubtract {
                                left: registers[2],
                                right: registers[1],
                                minuend: registers[0],
                                destination: registers[2],
                            },
                        ]
            }
            SelectedInstructionKind::BitwiseXorI64 => {
                decoded
                    == [DecodedWord::BitwiseXor {
                        left: registers[0],
                        right: registers[1],
                        destination: registers[2],
                    }]
            }
            SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
                decoded
                    == [DecodedWord::AddImmediate {
                        source: registers[0],
                        immediate: u12(immediate)?,
                        destination: registers[1],
                    }]
            }
            SelectedInstructionKind::ExactSubtractI64 { .. }
            | SelectedInstructionKind::WrappingSubtractI64 => {
                decoded
                    == [DecodedWord::Subtract {
                        left: registers[0],
                        right: registers[1],
                        destination: registers[2],
                    }]
            }
            SelectedInstructionKind::ExactMultiplyI64 { .. }
            | SelectedInstructionKind::WrappingMultiplyI64 => {
                decoded
                    == [DecodedWord::Multiply {
                        left: registers[0],
                        right: registers[1],
                        destination: registers[2],
                    }]
            }
            SelectedInstructionKind::WrappingShiftLeftI64
            | SelectedInstructionKind::WrappingShiftRightI64
            | SelectedInstructionKind::WrappingShiftRightU64
            | selected_instructions::exact_shift_i64_pair_pattern!()
            | SelectedInstructionKind::ExactShiftRightU64 { .. } => {
                let shift = match kind {
                    SelectedInstructionKind::WrappingShiftLeftI64
                    | SelectedInstructionKind::ExactShiftLeftI64 { .. } => {
                        DecodedWord::ShiftLeftVariable {
                            value: registers[0],
                            count: registers[1],
                            destination: registers[2],
                        }
                    }
                    SelectedInstructionKind::WrappingShiftRightU64
                    | SelectedInstructionKind::ExactShiftRightU64 { .. } => {
                        DecodedWord::ShiftRightLogicalVariable {
                            value: registers[0],
                            count: registers[1],
                            destination: registers[2],
                        }
                    }
                    _ => DecodedWord::ShiftRightArithmeticVariable {
                        value: registers[0],
                        count: registers[1],
                        destination: registers[2],
                    },
                };
                decoded == [shift]
            }
            SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
                decoded
                    == [DecodedWord::SubtractImmediate {
                        source: registers[0],
                        immediate: u12(immediate)?,
                        destination: registers[1],
                    }]
            }
            SelectedInstructionKind::ReturnScalar
            | SelectedInstructionKind::ReturnAggregate { .. }
            | SelectedInstructionKind::ReturnUnit => decoded == [DecodedWord::Return],
            selected_instructions::layout_dependent_selected_transfers!()
            | selected_instructions::dedicated_selected_forms!() => false,
        };
        if valid {
            Ok(())
        } else {
            Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
        }
    }

    fn decode_materialization(decoded: &[DecodedWord], register: u8) -> Option<u64> {
        let (mut value, start) = match decoded.first()? {
            DecodedWord::MovZ {
                register: actual,
                shift: 0,
                immediate,
            } if *actual == register => (u64::from(*immediate), 1),
            _ => return None,
        };
        let mut previous_shift = 0;
        for word in &decoded[start..] {
            let DecodedWord::MovK {
                register: actual,
                shift,
                immediate,
            } = word
            else {
                return None;
            };
            if *actual != register || *shift <= previous_shift || *shift > 3 || *immediate == 0 {
                return None;
            }
            previous_shift = *shift;
            let shift = u64::from(*shift) * 16;
            value = (value & !(0xffff_u64 << shift)) | (u64::from(*immediate) << shift);
        }
        Some(value)
    }

    pub(crate) fn footprint(
        kind: SelectedInstructionKind,
        operands: &[RegisterViewId],
    ) -> Aarch64SelectedFormFootprint {
        let (read_indices, write_indices, writes_nzcv) = match kind {
            SelectedInstructionKind::AccessorScalarAccess { form, .. } => {
                if form.write {
                    (vec![0, 1], vec![], false)
                } else {
                    (vec![0], vec![1], false)
                }
            }
            SelectedInstructionKind::AtomicAccess { form } => {
                return super::atomic::footprint(form, operands);
            }
            SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => {
                return super::ieee_float_arithmetic::footprint(form, operands);
            }
            SelectedInstructionKind::FloatResultPolicy { form } => {
                return super::float_result_policy::footprint(form, operands);
            }
            SelectedInstructionKind::RegisterAccess {
                operation: selected_instructions::RegisterOperation::MemoryFence(_),
            } => (Vec::new(), Vec::new(), false),
            SelectedInstructionKind::RegisterAccess { operation } => {
                if operation.produces_value() {
                    (vec![], vec![0], false)
                } else {
                    (vec![0], vec![], false)
                }
            }
            SelectedInstructionKind::PrivilegedCrash
            | SelectedInstructionKind::InstalledInterruptCpuCheck { .. }
            | SelectedInstructionKind::InstalledInterruptTaskCommit { .. }
            | SelectedInstructionKind::InstalledInterruptTablePublication { .. }
            | SelectedInstructionKind::PortWrite { .. } => {
                unreachable!("x86 port I/O cannot form an AArch64 request")
            }
            SelectedInstructionKind::AtomicFence | SelectedInstructionKind::Crash => {
                (Vec::new(), Vec::new(), false)
            }
            SelectedInstructionKind::TrappingInteger { form } => (
                (0..form.source_count() as u16).collect(),
                (form.source_count() as u16..super::trapping_forms::operand_count(form) as u16)
                    .collect(),
                true,
            ),
            SelectedInstructionKind::MaterializeBooleanEqual
            | SelectedInstructionKind::MaterializeBooleanU64LessThan
            | SelectedInstructionKind::MaterializeBooleanI64LessThan
            | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
            | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => (vec![], vec![0], false),
            SelectedInstructionKind::MaterializeI64 { .. } => (vec![], vec![0], false),
            SelectedInstructionKind::ImageAddress { .. } => (vec![], vec![0], false),
            SelectedInstructionKind::CopyI64
            | SelectedInstructionKind::ZeroExtendU8
            | SelectedInstructionKind::ZeroExtendU16
            | SelectedInstructionKind::SignExtendI8
            | SelectedInstructionKind::SignExtendI16
            | SelectedInstructionKind::SignExtendI32
            | SelectedInstructionKind::ZeroExtendU32 => (vec![0], vec![1], false),
            SelectedInstructionKind::CompareI64Zero
            | SelectedInstructionKind::CompareI64Immediate { .. } => (vec![0], vec![], true),
            SelectedInstructionKind::ExactDivideU64 { .. }
            | selected_instructions::exact_wrapping_divrem_set_pattern!(..) => {
                (vec![0, 1], vec![2], false)
            }
            SelectedInstructionKind::CompareI64 => (vec![0, 1], vec![], true),
            selected_instructions::saturating_add_or_subtract_pattern!()
            | SelectedInstructionKind::SaturatingDivide { .. }
            | SelectedInstructionKind::SaturatingRemainder { .. }
            | SelectedInstructionKind::SaturatingMultiply { .. } => {
                let realization = SaturatingRealization::of_kind(kind)
                    .expect("saturating kinds have a realization");
                (
                    vec![0, 1],
                    (2..realization.operand_count() as u16).collect(),
                    realization.defines_nzcv(),
                )
            }
            SelectedInstructionKind::BitwiseNotI64 => (vec![0], vec![1], false),
            SelectedInstructionKind::ByteViewAddress
            | SelectedInstructionKind::BitwiseAndI64
            | SelectedInstructionKind::BitwiseOrI64
            | SelectedInstructionKind::BitwiseXorI64
            | SelectedInstructionKind::WrappingAddI64
            | SelectedInstructionKind::WrappingSubtractI64
            | SelectedInstructionKind::WrappingMultiplyI64
            | SelectedInstructionKind::WrappingShiftLeftI64
            | SelectedInstructionKind::WrappingShiftRightI64
            | SelectedInstructionKind::WrappingShiftRightU64
            | selected_instructions::exact_shift_i64_pair_pattern!()
            | SelectedInstructionKind::ExactShiftRightU64 { .. }
            | SelectedInstructionKind::ExactAddI64 { .. }
            | SelectedInstructionKind::ExactMultiplyI64 { .. }
            | SelectedInstructionKind::ExactSubtractI64 { .. } => (vec![0, 1], vec![2], false),
            selected_instructions::exact_add_or_sub_pair_pattern!(..) => (vec![0], vec![1], false),
            SelectedInstructionKind::ReturnScalar
            | SelectedInstructionKind::ReturnAggregate { .. }
            | SelectedInstructionKind::ReturnUnit
            | SelectedInstructionKind::ConditionalBranchNonZero
            | SelectedInstructionKind::ConditionalBranchU64LessThan
            | SelectedInstructionKind::ConditionalBranchI64LessThan => (vec![], vec![], false),
            SelectedInstructionKind::Jump | selected_instructions::dedicated_selected_forms!() => {
                unreachable!("non-scalar forms have dedicated footprint decoders")
            }
        };
        let reads = read_indices
            .iter()
            .map(|index| operands[usize::from(*index)])
            .collect();
        let writes = write_indices
            .iter()
            .map(|index| operands[usize::from(*index)])
            .collect();
        let physical = aarch64_physical_register_model_ref();
        let units = |name: &str| physical.view_named(name).unwrap().units.clone();
        let encoded = if matches!(
            kind,
            SelectedInstructionKind::RegisterAccess {
                operation: selected_instructions::RegisterOperation::MemoryFence(_)
            }
        ) {
            MachineEncodedEffects::catalog_fence_v1()
        } else if let SelectedInstructionKind::RegisterAccess { operation } = kind {
            super::register_access::effects(operation.produces_value())
        } else if kind == SelectedInstructionKind::AtomicFence {
            MachineEncodedEffects::atomic_fence_v1()
        } else if kind == SelectedInstructionKind::Crash {
            selected_instructions::MachineEncodedEffects::crash_v1(&units("pc"))
        } else if let SelectedInstructionKind::TrappingInteger { .. } = kind {
            super::trapping_forms::effects(read_indices, write_indices)
        } else if matches!(
            kind,
            SelectedInstructionKind::ReturnScalar
                | SelectedInstructionKind::ReturnAggregate { .. }
                | SelectedInstructionKind::ReturnUnit
        ) {
            MachineEncodedEffects {
                external_operand_reads: vec![],
                external_operand_writes: vec![],
                implicit_unit_uses: units("x30"),
                implicit_unit_defs: units("pc"),
                implicit_unit_clobbers: vec![],
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                    target: physical.view_named("x30").unwrap().id,
                },
            }
        } else if matches!(
            kind,
            SelectedInstructionKind::ConditionalBranchNonZero
                | SelectedInstructionKind::ConditionalBranchU64LessThan
                | SelectedInstructionKind::ConditionalBranchI64LessThan
        ) {
            let mut uses = units("nzcv");
            uses.extend(units("pc"));
            uses.sort_unstable();
            uses.dedup();
            MachineEncodedEffects {
                external_operand_reads: vec![],
                external_operand_writes: vec![],
                implicit_unit_uses: uses,
                implicit_unit_defs: units("pc"),
                implicit_unit_clobbers: vec![],
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            }
        } else {
            let mut effects = MachineEncodedEffects::fallthrough_v1(read_indices, write_indices);
            if writes_nzcv {
                effects.implicit_unit_defs = units("nzcv");
            }
            if matches!(
                kind,
                SelectedInstructionKind::MaterializeBooleanEqual
                    | SelectedInstructionKind::MaterializeBooleanU64LessThan
                    | SelectedInstructionKind::MaterializeBooleanI64LessThan
                    | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                    | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
            ) {
                effects.implicit_unit_uses = units("nzcv");
            }
            effects
        };
        Aarch64SelectedFormFootprint {
            register_reads: reads,
            register_writes: writes,
            writes_nzcv,
            encoded,
        }
    }

    fn expected_saturating(
        realization: SaturatingRealization,
        registers: &[u8],
    ) -> Vec<DecodedWord> {
        let (left, right, value) = (registers[0], registers[1], registers[2]);
        let compare = |scratch| DecodedWord::Compare {
            left: value,
            right: scratch,
        };
        match realization {
            SaturatingRealization::AddU64 => vec![
                DecodedWord::AddWithFlags {
                    left,
                    right,
                    destination: value,
                },
                DecodedWord::SelectMaximumOnCarry {
                    source: value,
                    destination: value,
                },
            ],
            SaturatingRealization::SubtractUnsigned => vec![
                DecodedWord::SubtractWithFlags {
                    left,
                    right,
                    destination: value,
                },
                DecodedWord::SelectZeroOnBorrow {
                    source: value,
                    destination: value,
                },
            ],
            SaturatingRealization::DivideUnsigned => vec![DecodedWord::UnsignedDivide {
                dividend: left,
                divisor: right,
                destination: value,
            }],
            SaturatingRealization::Remainder { signed } => vec![
                if signed {
                    DecodedWord::SignedDivide {
                        dividend: left,
                        divisor: right,
                        destination: value,
                    }
                } else {
                    DecodedWord::UnsignedDivide {
                        dividend: left,
                        divisor: right,
                        destination: value,
                    }
                },
                DecodedWord::MultiplySubtract {
                    left: value,
                    right,
                    minuend: left,
                    destination: value,
                },
            ],
            SaturatingRealization::ClampNarrow { operation, .. } => {
                let scratch = registers[3];
                let mut expected = vec![match operation {
                    selected_instructions::SaturatingOperation::Add => DecodedWord::Add {
                        left,
                        right,
                        destination: value,
                    },
                    selected_instructions::SaturatingOperation::Subtract => DecodedWord::Subtract {
                        left,
                        right,
                        destination: value,
                    },
                    selected_instructions::SaturatingOperation::Divide => {
                        DecodedWord::SignedDivide {
                            dividend: left,
                            divisor: right,
                            destination: value,
                        }
                    }
                    selected_instructions::SaturatingOperation::Multiply => DecodedWord::Multiply {
                        left,
                        right,
                        destination: value,
                    },
                    selected_instructions::SaturatingOperation::Remainder => {
                        unreachable!("remainder is realized through the divide/MSUB pair")
                    }
                }];
                for (index, bound) in realization.clamp_bounds().into_iter().enumerate() {
                    expected.push(DecodedWord::MaterializeBound {
                        destination: scratch,
                        value: bound,
                    });
                    expected.push(compare(scratch));
                    expected.push(if index == 0 && realization.upper_condition() == 0x8 {
                        DecodedWord::SelectOnHigher {
                            source: scratch,
                            destination: value,
                        }
                    } else if index == 0 {
                        DecodedWord::SelectOnGreater {
                            source: scratch,
                            destination: value,
                        }
                    } else {
                        DecodedWord::SelectOnLess {
                            source: scratch,
                            destination: value,
                        }
                    });
                }
                expected
            }
            SaturatingRealization::OverflowI64 { subtract } => {
                let scratch = registers[3];
                vec![
                    DecodedWord::ArithmeticShiftRight63 {
                        source: left,
                        destination: scratch,
                    },
                    DecodedWord::ExclusiveOrI64Maximum { register: scratch },
                    if subtract {
                        DecodedWord::SubtractWithFlags {
                            left,
                            right,
                            destination: value,
                        }
                    } else {
                        DecodedWord::AddWithFlags {
                            left,
                            right,
                            destination: value,
                        }
                    },
                    DecodedWord::SelectOnOverflow {
                        source: scratch,
                        destination: value,
                    },
                ]
            }
            SaturatingRealization::DivideI64 => {
                let scratch = registers[3];
                vec![
                    DecodedWord::SignedDivide {
                        dividend: left,
                        divisor: right,
                        destination: value,
                    },
                    DecodedWord::MaterializeBound {
                        destination: scratch,
                        value: i64::MIN as u64,
                    },
                    compare(scratch),
                    DecodedWord::ConditionalCompareMinusOne { register: right },
                    DecodedWord::MaterializeBound {
                        destination: scratch,
                        value: i64::MAX as u64,
                    },
                    DecodedWord::SelectOnEqual {
                        source: scratch,
                        destination: value,
                    },
                ]
            }
            SaturatingRealization::MultiplyI64 => {
                let scratch = registers[3];
                vec![
                    DecodedWord::SignedMultiplyHigh {
                        left,
                        right,
                        destination: scratch,
                    },
                    DecodedWord::Multiply {
                        left,
                        right,
                        destination: value,
                    },
                    DecodedWord::CompareWithSignOf {
                        high: scratch,
                        low: value,
                    },
                    DecodedWord::ArithmeticShiftRight63 {
                        source: scratch,
                        destination: scratch,
                    },
                    DecodedWord::ExclusiveOrI64Maximum { register: scratch },
                    DecodedWord::SelectOnNotEqual {
                        source: scratch,
                        destination: value,
                    },
                ]
            }
            SaturatingRealization::MultiplyU64 => {
                let scratch = registers[3];
                vec![
                    DecodedWord::UnsignedMultiplyHigh {
                        left,
                        right,
                        destination: scratch,
                    },
                    DecodedWord::Multiply {
                        left,
                        right,
                        destination: value,
                    },
                    DecodedWord::CompareZero { source: scratch },
                    DecodedWord::SelectMaximumUnlessEqual {
                        source: value,
                        destination: value,
                    },
                ]
            }
        }
    }
}
mod encoded_scalar {
    use super::selected_forms::request_registers;
    use super::{Aarch64SelectedFormEncodingError as Error, ValidatedAarch64SelectedFormEncoding};
    use encoded_scalar_plan::{Step, plan};
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
        SelectedInstructionKind,
    };

    fn codec_plan(load: bool, registers: [u8; 6]) -> Vec<Step> {
        let [_, _, cursor, count, _, _] = registers;
        plan(load, registers, Step::Compare(cursor, count))
    }

    fn encode_word(word: Step, position: usize) -> Result<u32, Error> {
        let relative = |target: usize, bits: u32| -> Result<u32, Error> {
            let displacement = target as i64 - position as i64;
            let bound = 1i64 << (bits - 1);
            if !(-bound..bound).contains(&displacement) {
                return Err(Error::MalformedEncoding);
            }
            Ok((displacement as u32) & ((1 << bits) - 1))
        };
        Ok(match word {
            Step::Zero(destination) => 0xd280_0000 | u32::from(destination),
            Step::Copy(destination, source) => {
                0xaa00_03e0 | (u32::from(source) << 16) | u32::from(destination)
            }
            Step::ByteCount(destination, source) => {
                0xd340_1c00 | (u32::from(source) << 5) | u32::from(destination)
            }
            Step::SubOne(register) => {
                0xd100_0400 | (u32::from(register) << 5) | u32::from(register)
            }
            Step::AddOne(register) => {
                0x9100_0400 | (u32::from(register) << 5) | u32::from(register)
            }
            Step::LoadByte(destination, base, index) => {
                0x3860_6800
                    | (u32::from(index) << 16)
                    | (u32::from(base) << 5)
                    | u32::from(destination)
            }
            Step::StoreByte(source, base, index) => {
                0x3820_6800 | (u32::from(index) << 16) | (u32::from(base) << 5) | u32::from(source)
            }
            Step::SignByte(register) => {
                0x9340_1c00 | (u32::from(register) << 5) | u32::from(register)
            }
            Step::ShiftLeft8(register) => {
                0xd378_dc00 | (u32::from(register) << 5) | u32::from(register)
            }
            Step::ShiftRight8(register) => {
                0xd348_fc00 | (u32::from(register) << 5) | u32::from(register)
            }
            Step::Or(destination, source) => {
                0xaa00_0000
                    | (u32::from(source) << 16)
                    | (u32::from(destination) << 5)
                    | u32::from(destination)
            }
            Step::Compare(left, right) => {
                0xeb00_001f | (u32::from(right) << 16) | (u32::from(left) << 5)
            }
            Step::TestSigned(register, target) => {
                0x3640_0000 | (relative(target, 14)? << 5) | u32::from(register)
            }
            Step::ZeroBranch(register, target) => {
                0xb400_0000 | (relative(target, 19)? << 5) | u32::from(register)
            }
            Step::LessBranch(target) => 0x5400_0003 | (relative(target, 19)? << 5),
            Step::Branch(target) => 0x1400_0000 | relative(target, 26)?,
        })
    }

    fn decode_word(encoded: u32, position: usize) -> Result<Step, Error> {
        let destination = (encoded & 31) as u8;
        let source = ((encoded >> 5) & 31) as u8;
        let other = ((encoded >> 16) & 31) as u8;
        let target = |value: u32, bits: u32| -> Result<usize, Error> {
            let displacement = ((value << (32 - bits)) as i32) >> (32 - bits);
            usize::try_from(position as i64 + i64::from(displacement))
                .map_err(|_| Error::MalformedEncoding)
        };
        if encoded & 0xffff_ffe0 == 0xd280_0000 {
            return Ok(Step::Zero(destination));
        }
        if encoded & 0xffe0_ffe0 == 0xaa00_03e0 {
            return Ok(Step::Copy(destination, other));
        }
        if encoded & 0xffff_fc00 == 0xd340_1c00 {
            return Ok(Step::ByteCount(destination, source));
        }
        if encoded & 0xffff_fc00 == 0xd100_0400 && destination == source {
            return Ok(Step::SubOne(destination));
        }
        if encoded & 0xffff_fc00 == 0x9100_0400 && destination == source {
            return Ok(Step::AddOne(destination));
        }
        if encoded & 0xffe0_fc00 == 0x3860_6800 {
            return Ok(Step::LoadByte(destination, source, other));
        }
        if encoded & 0xffe0_fc00 == 0x3820_6800 {
            return Ok(Step::StoreByte(destination, source, other));
        }
        if destination == source {
            match encoded & 0xffff_fc00 {
                0x9340_1c00 => return Ok(Step::SignByte(destination)),
                0xd378_dc00 => return Ok(Step::ShiftLeft8(destination)),
                0xd348_fc00 => return Ok(Step::ShiftRight8(destination)),
                _ => {}
            }
        }
        if encoded & 0xffe0_fc00 == 0xaa00_0000 && destination == source {
            return Ok(Step::Or(destination, other));
        }
        if encoded & 0xffe0_fc1f == 0xeb00_001f {
            return Ok(Step::Compare(source, other));
        }
        if encoded & 0xfff8_0000 == 0x3640_0000 {
            return Ok(Step::TestSigned(
                destination,
                target((encoded >> 5) & 0x3fff, 14)?,
            ));
        }
        if encoded & 0xff00_0000 == 0xb400_0000 {
            return Ok(Step::ZeroBranch(
                destination,
                target((encoded >> 5) & 0x7ffff, 19)?,
            ));
        }
        if encoded & 0xff00_001f == 0x5400_0003 {
            return Ok(Step::LessBranch(target((encoded >> 5) & 0x7ffff, 19)?));
        }
        if encoded & 0xfc00_0000 == 0x1400_0000 {
            return Ok(Step::Branch(target(encoded & 0x03ff_ffff, 26)?));
        }
        Err(Error::MalformedEncoding)
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<(bool, [u8; 6]), Error> {
        let load = kind == SelectedInstructionKind::LoadEncodedScalar;
        let family = if load {
            MachineAlternativeFamily::LoadEncodedScalar
        } else {
            MachineAlternativeFamily::StoreEncodedScalar
        };
        if !matches!(
            kind,
            SelectedInstructionKind::LoadEncodedScalar
                | SelectedInstructionKind::StoreEncodedScalar
        ) {
            return Err(Error::AlternativeMismatch);
        }
        let inputs = if load { 2 } else { 3 };
        let registers: [u8; 6] =
            request_registers(physical, alternative, family, operands, inputs)?;
        Ok((load, registers))
    }

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Error> {
        let (load, registers) = request(physical, kind, alternative, operands)?;
        let mut bytes = Vec::new();
        for (position, word) in codec_plan(load, registers).into_iter().enumerate() {
            bytes.extend(encode_word(word, position)?.to_le_bytes());
        }
        validate(physical, kind, alternative, operands, &bytes)
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Error> {
        let (load, registers) = request(physical, kind, alternative, operands)?;
        let (words, tail) = bytes.as_chunks::<4>();
        if !tail.is_empty() {
            return Err(Error::MalformedEncoding);
        }
        let decoded = words
            .iter()
            .enumerate()
            .map(|(position, word)| decode_word(u32::from_le_bytes(*word), position))
            .collect::<Result<Vec<_>, _>>()?;
        if decoded != codec_plan(load, registers) {
            return Err(Error::EncodedFormMismatch);
        }
        let inputs = if load { 2 } else { 3 };
        super::nzcv_effects_encoding(
            physical,
            operands,
            bytes,
            inputs,
            if load {
                MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
                    pointer_operand: 0,
                    codec_operand: 1,
                }
            } else {
                MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
                    pointer_operand: 0,
                    codec_operand: 1,
                }
            },
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::selected_form_encoding::selected_forms::resolve_registers;
        use encoded_scalar_plan::{harness, replay};

        fn form() -> (ValidatedPhysicalRegisterModel, [RegisterViewId; 6]) {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let operands = ["x0", "x1", "x2", "x3", "x4", "x5"]
                .map(|name| physical.model().view_named(name).unwrap().id);
            (physical, operands)
        }

        fn execute(bytes: &[u8], state: &mut [u64; 32], memory: &mut [u8]) -> Vec<usize> {
            replay::execute(
                bytes,
                bytes.len() / 4,
                |bytes, position| {
                    let word: [u8; 4] = bytes[position * 4..position * 4 + 4].try_into().unwrap();
                    (decode_word(u32::from_le_bytes(word), position).unwrap(), 1)
                },
                state,
                memory,
            )
        }

        #[test]
        fn encoded_scalar_access_has_exact_footprint_and_extension() {
            let (physical, operands) = form();
            let registers: [u8; 6] = resolve_registers(&physical, &operands)
                .unwrap()
                .try_into()
                .unwrap();
            harness::exact_footprint_and_extension(
                registers,
                operands,
                |kind, alternative, operands| {
                    encode(&physical, kind, alternative, operands).map(|encoded| encoded.bytes)
                },
                execute,
            );
        }

        #[test]
        fn encoded_scalar_stream_rejects_every_single_bit_mutation() {
            let (physical, operands) = form();
            harness::rejects_every_single_bit_mutation(
                operands,
                |kind, alternative, operands| {
                    encode(&physical, kind, alternative, operands).map(|encoded| encoded.bytes)
                },
                |kind, alternative, operands, bytes| {
                    validate(&physical, kind, alternative, operands, bytes).is_err()
                },
            );
        }
    }
}
mod encoding_types {
    use register_model::RegisterViewId;
    use selected_instructions::MachineEncodedEffects;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Aarch64SelectedFormFootprint {
        pub register_reads: Vec<RegisterViewId>,
        pub register_writes: Vec<RegisterViewId>,
        pub writes_nzcv: bool,
        pub encoded: MachineEncodedEffects,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedAarch64SelectedFormEncoding {
        pub(crate) bytes: Vec<u8>,
        pub(crate) footprint: Aarch64SelectedFormFootprint,
    }

    impl ValidatedAarch64SelectedFormEncoding {
        pub fn bytes(&self) -> &[u8] {
            &self.bytes
        }

        pub const fn footprint(&self) -> &Aarch64SelectedFormFootprint {
            &self.footprint
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Aarch64SelectedFormEncodingError {
        NonCanonicalPhysicalModel,
        LayoutDependentForm,
        AlternativeMismatch,
        OperandCountMismatch,
        UnknownOrNonGpr64View(RegisterViewId),
        IntegerOutsideI64Bits,
        ImmediateOutsideU12,
        BranchDisplacementMisaligned,
        BranchDisplacementOutsideImm19,
        BranchDisplacementOutsideImm26,
        MovnMaterializationDoesNotShrink,
        MalformedEncoding,
        EncodedFormMismatch,
    }

    impl std::fmt::Display for Aarch64SelectedFormEncodingError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "invalid AArch64 selected-form encoding: {self:?}"
            )
        }
    }

    impl std::error::Error for Aarch64SelectedFormEncodingError {}
}
mod float_bits {
    use super::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
        ValidatedAarch64SelectedFormEncoding,
    };
    use crate::selected_form_encoding::selected_forms::resolve_registers;
    use register_model::RegisterViewId;
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::MachineEncodedEffects;
    use selected_instructions::SelectedInstructionKind;
    use selected_instructions::float_bit_transfer_properties;
    type Error = Aarch64SelectedFormEncodingError;
    type Encoding = ValidatedAarch64SelectedFormEncoding;

    fn operands(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        values: &[RegisterViewId],
    ) -> Result<(u8, u8, bool, bool), Error> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        let (family, to_bits, wide) =
            float_bit_transfer_properties(kind).ok_or(Error::AlternativeMismatch)?;
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Error::AlternativeMismatch);
        }
        let [source, destination] = values else {
            return Err(Error::OperandCountMismatch);
        };
        let (float, bits) = if to_bits {
            (*source, *destination)
        } else {
            (*destination, *source)
        };
        let float_view = physical
            .model()
            .view(float)
            .ok_or(Error::UnknownOrNonGpr64View(float))?;
        let float_register = float_view
            .name
            .strip_prefix("d")
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .filter(|number| *number <= 31)
            .ok_or(Error::UnknownOrNonGpr64View(float))?;
        if float_view.bits != 64 || !float_view.allocatable {
            return Err(Error::UnknownOrNonGpr64View(float));
        }
        let bits_register = resolve_registers(physical, &[bits])?[0];
        Ok((float_register, bits_register, to_bits, wide))
    }
    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        values: &[RegisterViewId],
    ) -> Result<Encoding, Error> {
        let (float, bits, to_bits, wide) = operands(physical, kind, alternative, values)?;
        let opcode = match (to_bits, wide) {
            (true, false) => 0x1e26_0000_u32,
            (true, true) => 0x9e66_0000,
            (false, false) => 0x1e27_0000,
            (false, true) => 0x9e67_0000,
        };
        let (source, destination) = if to_bits {
            (float, bits)
        } else {
            (bits, float)
        };
        let bytes = (opcode | (u32::from(source) << 5) | u32::from(destination))
            .to_le_bytes()
            .to_vec();
        validate(physical, kind, alternative, values, &bytes)
    }
    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        values: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<Encoding, Error> {
        let (float, bits, to_bits, wide) = operands(physical, kind, alternative, values)?;
        let raw: [u8; 4] = bytes.try_into().map_err(|_| Error::MalformedEncoding)?;
        let word = u32::from_le_bytes(raw);
        let (decoded_to_bits, decoded_wide) = match word & 0xffff_fc00 {
            0x1e26_0000 => (true, false),
            0x9e66_0000 => (true, true),
            0x1e27_0000 => (false, false),
            0x9e67_0000 => (false, true),
            _ => return Err(Error::MalformedEncoding),
        };
        let source = ((word >> 5) & 31) as u8;
        let destination = (word & 31) as u8;
        let (decoded_float, decoded_bits) = if decoded_to_bits {
            (source, destination)
        } else {
            (destination, source)
        };
        if (decoded_float, decoded_bits, decoded_to_bits, decoded_wide)
            != (float, bits, to_bits, wide)
        {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(Encoding {
            bytes: bytes.to_vec(),
            footprint: Aarch64SelectedFormFootprint {
                register_reads: vec![values[0]],
                register_writes: vec![values[1]],
                writes_nzcv: false,
                encoded: MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]),
            },
        })
    }
    #[cfg(test)]
    mod tests {
        use super::{encode, float_bit_transfer_properties, validate};
        use selected_instructions::MachineAlternativeFamily;
        use selected_instructions::MachineAlternativeKey;
        use selected_instructions::MachineEncodedEffects;
        use selected_instructions::SelectedInstructionKind;
        #[test]
        fn raw_transfers_decode_exact_width_direction_and_registers() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            for (kind, family) in [
                (
                    SelectedInstructionKind::Float32ToBits,
                    MachineAlternativeFamily::Float32ToBits,
                ),
                (
                    SelectedInstructionKind::Float64ToBits,
                    MachineAlternativeFamily::Float64ToBits,
                ),
                (
                    SelectedInstructionKind::BitsToFloat32,
                    MachineAlternativeFamily::BitsToFloat32,
                ),
                (
                    SelectedInstructionKind::BitsToFloat64,
                    MachineAlternativeFamily::BitsToFloat64,
                ),
            ] {
                for float_name in ["d0", "d31"] {
                    for bits_name in ["x0", "x28"] {
                        let float = physical.model().view_named(float_name).unwrap().id;
                        let bits = physical.model().view_named(bits_name).unwrap().id;
                        let values = if float_bit_transfer_properties(kind).unwrap().1 {
                            [float, bits]
                        } else {
                            [bits, float]
                        };
                        let alternative = MachineAlternativeKey { family, variant: 0 };
                        let encoded = encode(&physical, kind, alternative, &values).unwrap();
                        assert_eq!(
                            encoded.footprint.encoded,
                            MachineEncodedEffects::fallthrough_v1(vec![0], vec![1])
                        );
                        for byte_index in 0..encoded.bytes.len() {
                            let mut changed = encoded.bytes.clone();
                            changed[byte_index] ^= 1;
                            assert!(
                                validate(&physical, kind, alternative, &values, &changed).is_err()
                            );
                        }
                        assert!(
                            encode(&physical, kind, alternative, &[values[1], values[0]]).is_err()
                        );
                    }
                }
            }
        }
    }
}
pub(crate) mod float_result_policy {
    use super::{Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint};
    use register_model::{RegisterConstraintKey, RegisterUnitId, RegisterViewId};
    use selected_instructions::{
        FloatResultPolicyForm, FloatResultPolicyKind, IeeeFloatFormat, MachineAlternativeFamily,
        MachineBarrier, MachineEffectDeclaration, MachineEncodedEffects, MachineMemoryEffect,
        MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    };

    fn clobbers(form: FloatResultPolicyForm) -> Vec<RegisterUnitId> {
        let physical = crate::aarch64_physical_register_model_ref();
        let names: &[&str] = if form.can_trap() {
            &["q16", "nzcv"]
        } else {
            &["q16", "q17", "nzcv"]
        };
        let mut units = names
            .iter()
            .copied()
            .flat_map(|name| physical.views.iter().filter(move |view| view.name == name))
            .flat_map(|view| view.units.iter().copied())
            .collect::<Vec<_>>();
        units.sort_unstable();
        units.dedup();
        units
    }

    pub(crate) fn effects(form: FloatResultPolicyForm) -> MachineEncodedEffects {
        MachineEncodedEffects::float_result_policy_v1(
            form.operand_count() as u16,
            clobbers(form),
            form.can_trap(),
        )
    }

    pub(crate) fn declaration(
        form: FloatResultPolicyForm,
        constraint: RegisterConstraintKey,
    ) -> MachineEffectDeclaration {
        let trapping = form.policy == FloatResultPolicyKind::Trapping;
        MachineEffectDeclaration::single(
            MachineSemanticKind::FloatResultPolicy(form),
            constraint,
            MachineMemoryEffect::NoneV1,
            if trapping {
                MachineTrapBehavior::TrappingIntegerV1
            } else {
                MachineTrapBehavior::NeverV1
            },
            if trapping {
                MachineBarrier::ExternalEffect
            } else {
                MachineBarrier::None
            },
            MachineAlternativeFamily::FloatResultPolicy(form),
            MachineSizeKnowledge::ExactBytes((program(form, &[0; 4]).len() * 4) as u16),
            effects(form),
        )
    }

    pub(crate) fn footprint(
        form: FloatResultPolicyForm,
        operands: &[RegisterViewId],
    ) -> Aarch64SelectedFormFootprint {
        let inputs = form.operand_count();
        Aarch64SelectedFormFootprint {
            register_reads: operands[..inputs].to_vec(),
            register_writes: operands[inputs..inputs + 1].to_vec(),
            writes_nzcv: true,
            encoded: effects(form),
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Word {
        Save {
            wide: bool,
            general: u8,
            vector: u8,
        },
        Restore {
            wide: bool,
            vector: u8,
            general: u8,
        },
        Extract {
            wide: bool,
            source: u8,
            result: u8,
            first: u8,
            count: u8,
        },
        Compare {
            wide: bool,
            source: u8,
            immediate: u16,
        },
        Conditional {
            equal: bool,
            words: i32,
        },
        Jump {
            words: i32,
        },
        SubtractOne {
            wide: bool,
            general: u8,
        },
        Crash,
    }

    fn program(form: FloatResultPolicyForm, registers: &[u8]) -> Vec<Word> {
        let wide = form.format == IeeeFloatFormat::Binary64;
        let (fraction, exponent) = if wide { (52, 11) } else { (23, 8) };
        let exponent_mask = (1_u16 << exponent) - 1;
        let inputs = form.operand_count();
        let result = registers[inputs];
        let mut words = Vec::new();
        for (position, &general) in registers[..inputs].iter().enumerate() {
            words.push(Word::Save {
                wide,
                general,
                vector: 16 + position as u8,
            });
        }
        let restore = |vector| Word::Restore {
            wide,
            vector,
            general: result,
        };
        let extract = |first, count| Word::Extract {
            wide,
            source: result,
            result,
            first,
            count,
        };
        let compare = |immediate| Word::Compare {
            wide,
            source: result,
            immediate,
        };
        if form.policy == FloatResultPolicyKind::Trapping {
            words.extend([
                restore(16),
                extract(fraction, exponent),
                compare(exponent_mask),
                Word::Conditional {
                    equal: false,
                    words: 2,
                },
                Word::Crash,
                restore(16),
            ]);
            return words;
        }
        let mut retain_raw_branches = Vec::new();
        words.extend([restore(16), extract(0, 8), compare(0)]);
        retain_raw_branches.push(words.len());
        words.push(Word::Conditional {
            equal: true,
            words: 0,
        });
        for (first, count, immediate) in [(fraction, exponent, exponent_mask), (0, fraction, 0)] {
            words.extend([restore(17), extract(first, count), compare(immediate)]);
            retain_raw_branches.push(words.len());
            words.push(Word::Conditional {
                equal: false,
                words: 0,
            });
        }
        words.extend([
            restore(17),
            Word::SubtractOne {
                wide,
                general: result,
            },
            Word::Jump { words: 2 },
        ]);
        let retain_raw = words.len();
        words.push(restore(17));
        for position in retain_raw_branches {
            if let Word::Conditional { words: offset, .. } = &mut words[position] {
                *offset = (retain_raw - position) as i32;
            }
        }
        words
    }

    fn encode_word(word: Word) -> u32 {
        match word {
            Word::Save {
                wide,
                general,
                vector,
            } => {
                (if wide { 0x9e67_0000 } else { 0x1e27_0000 })
                    | u32::from(general) << 5
                    | u32::from(vector)
            }
            Word::Restore {
                wide,
                vector,
                general,
            } => {
                (if wide { 0x9e66_0000 } else { 0x1e26_0000 })
                    | u32::from(vector) << 5
                    | u32::from(general)
            }
            Word::Extract {
                wide,
                source,
                result,
                first,
                count,
            } => {
                (if wide { 0xd340_0000 } else { 0x5300_0000 })
                    | u32::from(first) << 16
                    | u32::from(first + count - 1) << 10
                    | u32::from(source) << 5
                    | u32::from(result)
            }
            Word::Compare {
                wide,
                source,
                immediate,
            } => {
                (if wide { 0xf100_001f } else { 0x7100_001f })
                    | u32::from(immediate) << 10
                    | u32::from(source) << 5
            }
            Word::Conditional { equal, words } => {
                0x5400_0000 | ((words as u32 & 0x7ffff) << 5) | u32::from(!equal)
            }
            Word::Jump { words } => 0x1400_0000 | (words as u32 & 0x3ff_ffff),
            Word::SubtractOne { wide, general } => {
                (if wide { 0xd100_0400 } else { 0x5100_0400 })
                    | u32::from(general) << 5
                    | u32::from(general)
            }
            Word::Crash => 0xd420_0000,
        }
    }

    fn decode_word(word: u32) -> Option<Word> {
        let register = |shift: u32| ((word >> shift) & 31_u32) as u8;
        let wide = word >> 31 != 0;
        match word & 0xffff_fc00 {
            0x9e67_0000 | 0x1e27_0000 => {
                return Some(Word::Save {
                    wide,
                    general: register(5),
                    vector: register(0),
                });
            }
            0x9e66_0000 | 0x1e26_0000 => {
                return Some(Word::Restore {
                    wide,
                    vector: register(5),
                    general: register(0),
                });
            }
            0xd100_0400 | 0x5100_0400 if register(5) == register(0) => {
                return Some(Word::SubtractOne {
                    wide,
                    general: register(0),
                });
            }
            _ => {}
        }
        if matches!(word & 0xffc0_0000, 0xd340_0000 | 0x5300_0000) {
            let first = ((word >> 16) & 63) as u8;
            let last = ((word >> 10) & 63) as u8;
            if first > last || last >= if wide { 64 } else { 32 } {
                return None;
            }
            return Some(Word::Extract {
                wide,
                source: register(5),
                result: register(0),
                first,
                count: last - first + 1,
            });
        }
        if matches!(word & 0xffc0_001f, 0xf100_001f | 0x7100_001f) {
            return Some(Word::Compare {
                wide,
                source: register(5),
                immediate: ((word >> 10) & 4095) as u16,
            });
        }
        if word & 0xff00_001e == 0x5400_0000 {
            return Some(Word::Conditional {
                equal: word & 1 == 0,
                words: ((word as i32) << 8) >> 13,
            });
        }
        if word & 0xfc00_0000 == 0x1400_0000 {
            return Some(Word::Jump {
                words: ((word as i32) << 6) >> 6,
            });
        }
        (word == 0xd420_0000).then_some(Word::Crash)
    }

    pub(crate) fn encode(form: FloatResultPolicyForm, registers: &[u8]) -> Result<Vec<u8>, Error> {
        if registers.len() != form.operand_count() + 1
            || registers.iter().any(|register| *register > 30)
        {
            return Err(Error::OperandCountMismatch);
        }
        Ok(program(form, registers)
            .into_iter()
            .flat_map(|word| encode_word(word).to_le_bytes())
            .collect())
    }

    pub(crate) fn validate(
        form: FloatResultPolicyForm,
        registers: &[u8],
        bytes: &[u8],
    ) -> Result<(), Error> {
        if registers.len() != form.operand_count() + 1
            || registers.iter().any(|register| *register > 30)
        {
            return Err(Error::OperandCountMismatch);
        }
        let decoded = super::word_stream(bytes)
            .ok_or(Error::MalformedEncoding)?
            .into_iter()
            .map(decode_word)
            .collect::<Option<Vec<_>>>()
            .ok_or(Error::MalformedEncoding)?;
        if decoded != program(form, registers) {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use isa_conformance::float_result_policy as policy;

        fn encoded(form: FloatResultPolicyForm, registers: &[u8]) -> Vec<u8> {
            encode(form, registers).expect("admitted registers must encode")
        }

        fn validated(form: FloatResultPolicyForm, registers: &[u8], bytes: &[u8]) -> bool {
            validate(form, registers, bytes).is_ok()
        }

        fn roster(form: FloatResultPolicyForm) -> Vec<u8> {
            if form.can_trap() {
                vec![3, 4]
            } else {
                vec![1, 3, 4]
            }
        }

        fn replay(bytes: &[u8], registers: &[u8], payloads: &[u64]) -> Option<u64> {
            let mut general = [0_u64; 31];
            let mut vectors = [0_u64; 32];
            for (&register, &payload) in registers.iter().zip(payloads) {
                general[usize::from(register)] = payload;
            }
            let sized = |value, wide| if wide { value } else { value & 0xffff_ffff };
            let (instructions, remainder) = bytes.as_chunks::<4>();
            assert!(remainder.is_empty());
            let mut position = 0_usize;
            let mut equal = false;
            let mut steps = 0;
            while position < instructions.len() {
                steps += 1;
                assert!(steps <= instructions.len(), "policy must have no loop");
                match decode_word(u32::from_le_bytes(instructions[position])).expect("decoded word")
                {
                    Word::Save {
                        wide,
                        general: source,
                        vector,
                    } => vectors[usize::from(vector)] = sized(general[usize::from(source)], wide),
                    Word::Restore {
                        wide,
                        vector,
                        general: result,
                    } => general[usize::from(result)] = sized(vectors[usize::from(vector)], wide),
                    Word::Extract {
                        wide,
                        source,
                        result,
                        first,
                        count,
                    } => {
                        general[usize::from(result)] = (sized(general[usize::from(source)], wide)
                            >> first)
                            & ((1_u64 << count) - 1)
                    }
                    Word::Compare {
                        wide,
                        source,
                        immediate,
                    } => equal = sized(general[usize::from(source)], wide) == u64::from(immediate),
                    Word::Conditional {
                        equal: condition,
                        words,
                    } if equal == condition => {
                        position = usize::try_from(position as i64 + i64::from(words)).unwrap();
                        continue;
                    }
                    Word::Conditional { .. } => {}
                    Word::Jump { words } => {
                        position = usize::try_from(position as i64 + i64::from(words)).unwrap();
                        continue;
                    }
                    Word::SubtractOne {
                        wide,
                        general: result,
                    } => {
                        general[usize::from(result)] =
                            sized(general[usize::from(result)].wrapping_sub(1), wide)
                    }
                    Word::Crash => return None,
                }
                position += 1;
            }
            assert_eq!(position, instructions.len());
            Some(general[usize::from(*registers.last().unwrap())])
        }

        #[test]
        fn trapping_words_match_assembler_and_corrupted_policy_bytes_reject() {
            let form = FloatResultPolicyForm {
                format: IeeeFloatFormat::Binary64,
                policy: FloatResultPolicyKind::Trapping,
            };
            let expected = [
                0x9e67_0070_u32,
                0x9e66_0204,
                0xd374_f884,
                0xf11f_fc9f,
                0x5400_0041,
                0xd420_0000,
                0x9e66_0204,
            ]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
            policy::exact_stream(form, &[3, 4], &expected, encoded, validated);
            policy::rejects_corrupted_and_substituted_streams(roster, encoded, validated);
        }

        #[test]
        fn decoded_policy_matches_ieee_semantics_with_aliased_outputs() {
            for form in FloatResultPolicyForm::ALL {
                for case in policy::cases(form.format == IeeeFloatFormat::Binary64) {
                    let expected = policy::expected(form, case);
                    let inputs = policy::inputs(form, case);
                    for output in 1..=inputs.len() as u8 + 1 {
                        let mut registers = (1..=inputs.len() as u8).collect::<Vec<_>>();
                        registers.push(output);
                        let bytes = encoded(form, &registers);
                        validate(form, &registers, &bytes).unwrap();
                        assert_eq!(
                            replay(&bytes, &registers, &inputs),
                            expected,
                            "{form:?} {inputs:x?} output {output}"
                        );
                    }
                }
            }
        }

        #[test]
        fn policy_constraints_and_effects_declare_only_actual_scratch_and_control() {
            let physical = crate::aarch64_physical_register_model();
            let model = register_model::validate_physical_register_model(physical.clone()).unwrap();
            let constraints = crate::aarch64_register_constraint_catalog(&model);
            for form in FloatResultPolicyForm::ALL {
                let key = if form.can_trap() {
                    crate::AARCH64_FLOAT_RESULT_TRAPPING
                } else {
                    crate::AARCH64_FLOAT_RESULT_SATURATING
                };
                let constraint =
                    register_model::find_register_constraint(&constraints.constraints, key)
                        .unwrap();
                let declaration = declaration(form, key);
                let registers = (1..=form.operand_count() as u8 + 1).collect::<Vec<_>>();
                let operands = registers
                    .iter()
                    .map(|register| physical.view_named(&format!("x{register}")).unwrap().id)
                    .collect::<Vec<_>>();
                let footprint = footprint(form, &operands);
                assert!(constraint.implicit_uses.is_empty());
                assert!(footprint.encoded.implicit_unit_uses.is_empty());
                let expected_views: &[&str] = if form.can_trap() {
                    &["q16", "nzcv"]
                } else {
                    &["q16", "q17", "nzcv"]
                };
                let mut expected_units = expected_views
                    .iter()
                    .flat_map(|name| {
                        physical
                            .view_named(name)
                            .expect("canonical scratch view")
                            .units
                            .iter()
                            .copied()
                    })
                    .collect::<Vec<_>>();
                expected_units.sort_unstable();
                expected_units.dedup();
                assert_eq!(constraint.clobbers, expected_units);
                assert_eq!(footprint.encoded.implicit_unit_clobbers, expected_units);
                assert_eq!(
                    constraint.clobbers,
                    footprint.encoded.implicit_unit_clobbers
                );
                assert_eq!(constraint.operands.len(), form.operand_count() + 1);
                assert!(footprint.writes_nzcv);
                assert_eq!(declaration.alternatives[0].encoded, footprint.encoded);
                let size = MachineSizeKnowledge::ExactBytes(
                    encode(form, &registers).unwrap().len() as u16,
                );
                assert_eq!(declaration.alternatives[0].size, size);
                assert_eq!(
                    declaration.trap,
                    if form.can_trap() {
                        MachineTrapBehavior::TrappingIntegerV1
                    } else {
                        MachineTrapBehavior::NeverV1
                    }
                );
                assert_eq!(
                    declaration.barrier,
                    if form.can_trap() {
                        MachineBarrier::ExternalEffect
                    } else {
                        MachineBarrier::None
                    }
                );
            }
        }
    }
}
pub(crate) mod floating_control {
    use super::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
        ValidatedAarch64SelectedFormEncoding,
    };
    use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeKey, MachineBarrier, MachineEffectDeclaration,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
        MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior, SelectedInstructionKind,
    };

    pub(crate) fn clobbers(restore: bool) -> Vec<register_model::RegisterUnitId> {
        let physical = crate::aarch64_physical_register_model();
        let mut units = physical
            .view_named("x9")
            .expect("canonical floating-control scratch")
            .units
            .clone();
        if !restore {
            units.extend(
                physical
                    .view_named("x16")
                    .expect("canonical floating-control address scratch")
                    .units
                    .iter()
                    .copied(),
            );
        }
        units.sort_unstable();
        units
    }

    pub(crate) fn effects(restore: bool) -> MachineEncodedEffects {
        let physical = crate::aarch64_physical_register_model();
        let view = |name| {
            physical
                .view_named(name)
                .expect("canonical floating-control register")
        };
        let mut uses = view("sp").units.clone();
        if !restore {
            uses.extend(view("fpcr").units.iter().copied());
        }
        uses.sort_unstable();
        MachineEncodedEffects {
            external_operand_reads: Vec::new(),
            external_operand_writes: Vec::new(),
            implicit_unit_uses: uses,
            implicit_unit_defs: if restore {
                view("fpcr").units.clone()
            } else {
                Vec::new()
            },
            implicit_unit_clobbers: clobbers(restore),
            memory: if restore {
                MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                    stack_pointer: view("sp").id,
                    byte_count: 8,
                }
            } else {
                MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                    stack_pointer: view("sp").id,
                    byte_count: 8,
                }
            },
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::FallThroughV1,
        }
    }

    pub(crate) fn declaration(
        semantic: MachineSemanticKind,
        constraint: RegisterConstraintKey,
    ) -> MachineEffectDeclaration {
        let restore = semantic == MachineSemanticKind::RestoreFloatingControl;
        MachineEffectDeclaration::single(
            semantic,
            constraint,
            if restore {
                MachineMemoryEffect::ReadFrameStorageV1
            } else {
                MachineMemoryEffect::WriteFrameStorageV1
            },
            MachineTrapBehavior::MayArchitecturalFaultV1,
            MachineBarrier::ExternalEffect,
            semantic.into(),
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 8,
                maximum_bytes: Some(crate::floating_control::MAX_FPCR_SEQUENCE_BYTES),
            },
            effects(restore),
        )
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<bool, Aarch64SelectedFormEncodingError> {
        let semantic = match kind {
            SelectedInstructionKind::SaveFloatingControl {
                slot: LocalStorageSlotId::Boundary { .. },
            } => MachineSemanticKind::SaveFloatingControl,
            SelectedInstructionKind::RestoreFloatingControl {
                slot: LocalStorageSlotId::Boundary { .. },
            } => MachineSemanticKind::RestoreFloatingControl,
            _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
        };
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || !operands.is_empty()
            || alternative
                != (MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                })
            || u32::try_from(displacement).is_err()
            || displacement % 8 != 0
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(semantic == MachineSemanticKind::RestoreFloatingControl)
    }

    pub fn encode_aarch64_selected_floating_control_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let restore = request(physical, kind, alternative, operands, displacement)?;
        let offset = u32::try_from(displacement)
            .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        let bytes = if restore {
            crate::encode_restore_fpcr_from_sp_displacement(offset)
        } else {
            crate::encode_save_fpcr_to_sp_displacement(offset)
        }
        .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        validate_aarch64_selected_floating_control_form(
            physical,
            kind,
            alternative,
            operands,
            displacement,
            &bytes,
        )
    }

    pub(crate) fn validate_aarch64_selected_floating_control_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let restore = request(physical, kind, alternative, operands, displacement)?;
        if decode(bytes) != Some((restore, displacement)) {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedAarch64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: Aarch64SelectedFormFootprint {
                register_reads: Vec::new(),
                register_writes: Vec::new(),
                writes_nzcv: false,
                encoded: effects(restore),
            },
        })
    }

    fn decode(bytes: &[u8]) -> Option<(bool, i64)> {
        let words = super::word_stream(bytes)?;
        let (restore, steps, access, address) = match words.as_slice() {
            [steps @ .., access, 0xd51b_4409] => (true, steps, *access, 9),
            [0xd53b_4409, steps @ .., access] => (false, steps, *access, 16),
            _ => return None,
        };
        let pages = if steps.is_empty() {
            0
        } else {
            crate::immediate_offset::decode_address_words(steps, address, 31)?
        };
        let offset = u32::try_from(pages + u64::from((access >> 10) & 0xfff) * 8).ok()?;
        let expected = if restore {
            crate::encode_restore_fpcr_from_sp_displacement(offset)
        } else {
            crate::encode_save_fpcr_to_sp_displacement(offset)
        }
        .ok()?;
        (expected == bytes).then_some((restore, i64::from(offset)))
    }

    #[cfg(test)]
    mod tests {
        use super::{
            effects, encode_aarch64_selected_floating_control_form,
            validate_aarch64_selected_floating_control_form,
        };
        use crate::test_support::reject_every_bit_flip;
        use selected_instructions::{
            LocalStorageSlotId, MachineAlternativeKey, MachineSemanticKind, SelectedInstructionKind,
        };
        #[test]
        fn floating_controls_bind_direction_slot_displacement_and_every_encoded_bit() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let slot = LocalStorageSlotId::Boundary {
                operation: semantic_vocabulary::OperationId::new(7).unwrap(),
            };
            for restore in [false, true] {
                let kind = if restore {
                    SelectedInstructionKind::RestoreFloatingControl { slot }
                } else {
                    SelectedInstructionKind::SaveFloatingControl { slot }
                };
                let semantic = if restore {
                    MachineSemanticKind::RestoreFloatingControl
                } else {
                    MachineSemanticKind::SaveFloatingControl
                };
                let alternative = MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                };
                for displacement in [0, 24, 32760, 32768, 65_544, 33_554_440] {
                    let encoded = encode_aarch64_selected_floating_control_form(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        displacement,
                    )
                    .unwrap();
                    assert_eq!(encoded.footprint().encoded, effects(restore));
                    reject_every_bit_flip(encoded.bytes(), &format!("{kind:?}"), |changed| {
                        validate_aarch64_selected_floating_control_form(
                            &physical,
                            kind,
                            alternative,
                            &[],
                            displacement,
                            changed,
                        )
                        .is_err()
                    });
                    assert!(
                        validate_aarch64_selected_floating_control_form(
                            &physical,
                            kind,
                            alternative,
                            &[],
                            displacement + 8,
                            encoded.bytes()
                        )
                        .is_err()
                    );
                    let mut extra = encoded.bytes().to_vec();
                    extra.push(0);
                    assert!(
                        validate_aarch64_selected_floating_control_form(
                            &physical,
                            kind,
                            alternative,
                            &[],
                            displacement,
                            &extra
                        )
                        .is_err()
                    );
                }
                for displacement in [-8, 4, i64::from(u32::MAX) + 1] {
                    assert!(
                        encode_aarch64_selected_floating_control_form(
                            &physical,
                            kind,
                            alternative,
                            &[],
                            displacement
                        )
                        .is_err()
                    );
                }
                let wrong = SelectedInstructionKind::SaveFloatingControl {
                    slot: LocalStorageSlotId::Structural {
                        operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                        place: semantic_vocabulary::PlaceId::new(1).unwrap(),
                    },
                };
                assert!(
                    encode_aarch64_selected_floating_control_form(
                        &physical,
                        wrong,
                        alternative,
                        &[],
                        0
                    )
                    .is_err()
                );
            }
        }
    }
}
pub(crate) mod memory_fence {
    use register_model::RegisterConstraintKey;
    use selected_instructions::{
        MachineAlternativeFamily, MachineBarrier, MachineEffectDeclaration, MachineEncodedEffects,
        MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
        MemoryFenceKind,
    };

    pub(crate) fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            MachineSemanticKind::MemoryFence,
            constraint,
            MachineMemoryEffect::CatalogFenceV1,
            MachineTrapBehavior::NeverV1,
            MachineBarrier::ExternalEffect,
            MachineAlternativeFamily::MemoryFence,
            MachineSizeKnowledge::ExactBytes(4),
            MachineEncodedEffects::catalog_fence_v1(),
        )
    }

    pub(crate) fn encode(kind: MemoryFenceKind) -> Option<Vec<u8>> {
        let option = match kind {
            MemoryFenceKind::Aarch64Load => 0xd,
            MemoryFenceKind::Aarch64Store => 0xe,
            MemoryFenceKind::Aarch64Full => 0xf,
            _ => return None,
        };
        Some((0xd503_30bf_u32 | (option << 8)).to_le_bytes().to_vec())
    }

    pub(crate) fn decode(bytes: &[u8]) -> Option<MemoryFenceKind> {
        let word = u32::from_le_bytes(bytes.try_into().ok()?);
        if word & !0x0000_0f00 != 0xd503_30bf {
            return None;
        }
        match (word >> 8) & 0xf {
            0xd => Some(MemoryFenceKind::Aarch64Load),
            0xe => Some(MemoryFenceKind::Aarch64Store),
            0xf => Some(MemoryFenceKind::Aarch64Full),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::test_support::rejects_every_bit_flip;
        use selected_instructions::{
            MachineAlternativeKey, RegisterOperation, SelectedInstructionKind,
        };
        #[test]
        fn catalog_fences_keep_exact_bytes_and_reject_other_effects() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::MemoryFence,
                variant: 0,
            };
            for (kind, expected) in [
                (MemoryFenceKind::Aarch64Load, &[0xbf, 0x3d, 0x03, 0xd5][..]),
                (MemoryFenceKind::Aarch64Store, &[0xbf, 0x3e, 0x03, 0xd5][..]),
                (MemoryFenceKind::Aarch64Full, &[0xbf, 0x3f, 0x03, 0xd5][..]),
            ] {
                let instruction = SelectedInstructionKind::RegisterAccess {
                    operation: RegisterOperation::MemoryFence(kind),
                };
                let realized =
                    crate::encode_aarch64_selected_form(&physical, instruction, alternative, &[])
                        .unwrap();
                assert_eq!(realized.bytes(), expected);
                assert_eq!(
                    realized.footprint().encoded,
                    MachineEncodedEffects::catalog_fence_v1()
                );
                assert!(
                    realized.footprint().register_reads.is_empty()
                        && realized.footprint().register_writes.is_empty()
                );
                assert!(!realized.footprint().writes_nzcv);
                rejects_every_bit_flip(&physical, instruction, alternative, &[], expected);
                let mut doubled = expected.to_vec();
                doubled.extend(expected);
                for changed in [&doubled[..], &expected[..expected.len() - 1], &[]] {
                    assert!(
                        crate::validate_aarch64_selected_form_encoding(
                            &physical,
                            instruction,
                            alternative,
                            &[],
                            changed
                        )
                        .is_err()
                    );
                }
                let wrong_kind = SelectedInstructionKind::RegisterAccess {
                    operation: RegisterOperation::MemoryFence(MemoryFenceKind::Full),
                };
                assert!(
                    crate::encode_aarch64_selected_form(&physical, wrong_kind, alternative, &[])
                        .is_err()
                );
                assert!(
                    crate::encode_aarch64_selected_form(
                        &physical,
                        instruction,
                        MachineAlternativeKey {
                            family: MachineAlternativeFamily::AtomicFence,
                            variant: 0
                        },
                        &[]
                    )
                    .is_err()
                );
                let operand = physical.model().view_named("x0").unwrap().id;
                assert!(
                    crate::encode_aarch64_selected_form(
                        &physical,
                        instruction,
                        alternative,
                        &[operand]
                    )
                    .is_err()
                );
            }
        }
    }
}
mod plan_words {
    use super::Aarch64SelectedFormEncodingError;

    pub(super) const COND_EQ: u8 = 0x0;
    pub(super) const COND_NE: u8 = 0x1;
    pub(super) const COND_HS: u8 = 0x2;
    pub(super) const COND_LO: u8 = 0x3;
    pub(super) const COND_HI: u8 = 0x8;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(super) enum Instruction<Label> {
        Zero(u8),
        One(u8),
        Cbz(u8, Label),
        Tbnz7(u8, Label),
        LdrbRegister {
            destination: u8,
            base: u8,
            index: u8,
        },
        AddImmediate {
            destination: u8,
            source: u8,
            immediate: u16,
        },
        CompareRegister {
            left: u8,
            right: u8,
        },
        CompareImmediate {
            register: u8,
            immediate: u16,
        },
        BranchConditional {
            condition: u8,
            target: Label,
        },
        Branch(Label),
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(super) enum Decoded {
        Zero(u8),
        One(u8),
        Cbz(u8, i32),
        Tbnz7(u8, i32),
        LdrbRegister {
            destination: u8,
            base: u8,
            index: u8,
        },
        AddImmediate {
            destination: u8,
            source: u8,
            immediate: u16,
        },
        CompareRegister {
            left: u8,
            right: u8,
        },
        CompareImmediate {
            register: u8,
            immediate: u16,
        },
        BranchConditional {
            condition: u8,
            target: i32,
        },
        Branch(i32),
    }

    pub(super) fn sign_extend(value: u32, bits: u32) -> i32 {
        let shift = 32 - bits;
        ((value << shift) as i32) >> shift
    }

    pub(super) fn words<Label: Copy + Eq>(
        plan: &[(Option<Label>, Instruction<Label>)],
    ) -> Result<Vec<u32>, Aarch64SelectedFormEncodingError> {
        let labels = plan
            .iter()
            .enumerate()
            .filter_map(|(index, (label, _))| label.map(|label| (label, index)))
            .collect::<Vec<_>>();
        plan.iter()
            .enumerate()
            .map(|(position, (_, instruction))| emit_word(*instruction, position, &labels))
            .collect()
    }

    pub(super) fn assemble<Label: Copy + Eq>(
        plan: &[(Option<Label>, Instruction<Label>)],
    ) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
        Ok(words(plan)?
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect())
    }

    pub(super) fn expected_stream<Label: Copy + Eq>(
        plan: &[(Option<Label>, Instruction<Label>)],
    ) -> Result<Vec<Decoded>, Aarch64SelectedFormEncodingError> {
        words(plan)?
            .into_iter()
            .enumerate()
            .map(|(position, word)| decode_word(word, position))
            .collect()
    }

    fn emit_word<Label: Copy + Eq>(
        instruction: Instruction<Label>,
        position: usize,
        labels: &[(Label, usize)],
    ) -> Result<u32, Aarch64SelectedFormEncodingError> {
        let target_word = |label: Label| -> usize {
            labels
                .iter()
                .find(|(candidate, _)| *candidate == label)
                .map(|(_, index)| *index)
                .expect("the plan defines every branch label")
        };
        let relative = |label: Label, bits: u32| -> Result<u32, Aarch64SelectedFormEncodingError> {
            let displacement = target_word(label) as i64 - position as i64;
            let limit = 1i64 << (bits - 1);
            if !(-limit..limit).contains(&displacement) {
                return Err(match bits {
                    14 | 19 => Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm19,
                    _ => Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm26,
                });
            }
            Ok((displacement as i32) as u32 & ((1u32 << bits) - 1))
        };
        Ok(match instruction {
            Instruction::Zero(register) => 0xd280_0000 | register as u32,
            Instruction::One(register) => 0xd280_0020 | register as u32,
            Instruction::Cbz(register, target) => {
                0xb400_0000 | (relative(target, 19)? << 5) | register as u32
            }
            Instruction::Tbnz7(register, target) => {
                0x3738_0000 | (relative(target, 14)? << 5) | register as u32
            }
            Instruction::LdrbRegister {
                destination,
                base,
                index,
            } => 0x3860_6800 | ((index as u32) << 16) | ((base as u32) << 5) | destination as u32,
            Instruction::AddImmediate {
                destination,
                source,
                immediate,
            } => {
                0x9100_0000
                    | ((immediate as u32) << 10)
                    | ((source as u32) << 5)
                    | destination as u32
            }
            Instruction::CompareRegister { left, right } => {
                0xeb00_001f | ((right as u32) << 16) | ((left as u32) << 5)
            }
            Instruction::CompareImmediate {
                register,
                immediate,
            } => 0xf100_0000 | ((immediate as u32) << 10) | ((register as u32) << 5) | 0x1f,
            Instruction::BranchConditional { condition, target } => {
                0x5400_0000 | (relative(target, 19)? << 5) | condition as u32
            }
            Instruction::Branch(target) => 0x1400_0000 | relative(target, 26)?,
        })
    }

    pub(super) fn decode_word(
        word: u32,
        position: usize,
    ) -> Result<Decoded, Aarch64SelectedFormEncodingError> {
        let register = (word & 31) as u8;
        if word & 0xffe0_0000 == 0xd280_0000 {
            return Ok(match (word >> 5) & 0xffff {
                0 => Decoded::Zero(register),
                1 => Decoded::One(register),
                _ => return Err(Aarch64SelectedFormEncodingError::MalformedEncoding),
            });
        }
        if word & 0xff00_0000 == 0xb400_0000 {
            let displacement = sign_extend((word >> 5) & 0x7ffff, 19);
            return Ok(Decoded::Cbz(register, position as i32 + displacement));
        }
        if word & 0xfff8_0000 == 0x3738_0000 {
            let displacement = sign_extend((word >> 5) & 0x3fff, 14);
            return Ok(Decoded::Tbnz7(register, position as i32 + displacement));
        }
        if word & 0xffe0_fc00 == 0x3860_6800 {
            return Ok(Decoded::LdrbRegister {
                destination: register,
                base: ((word >> 5) & 31) as u8,
                index: ((word >> 16) & 31) as u8,
            });
        }
        if word & 0xffc0_0000 == 0x9100_0000 {
            return Ok(Decoded::AddImmediate {
                destination: register,
                source: ((word >> 5) & 31) as u8,
                immediate: ((word >> 10) & 0xfff) as u16,
            });
        }
        if word & 0xffe0_fc1f == 0xeb00_001f {
            return Ok(Decoded::CompareRegister {
                left: ((word >> 5) & 31) as u8,
                right: ((word >> 16) & 31) as u8,
            });
        }
        if word & 0xffc0_0000 == 0xf100_0000 && register == 0x1f {
            return Ok(Decoded::CompareImmediate {
                register: ((word >> 5) & 31) as u8,
                immediate: ((word >> 10) & 0xfff) as u16,
            });
        }
        if word & 0xff00_0010 == 0x5400_0000 {
            let displacement = sign_extend((word >> 5) & 0x7ffff, 19);
            return Ok(Decoded::BranchConditional {
                condition: register & 15,
                target: position as i32 + displacement,
            });
        }
        if word & 0xfc00_0000 == 0x1400_0000 {
            let displacement = sign_extend(word & 0x03ff_ffff, 26);
            return Ok(Decoded::Branch(position as i32 + displacement));
        }
        Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
    }
}
pub(crate) mod register_access {
    use super::Aarch64SelectedFormEncodingError;
    use register_model::RegisterConstraintKey;
    use selected_instructions::{
        MachineAlternativeFamily, MachineBarrier, MachineEffectDeclaration, MachineEncodedEffects,
        MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
        MachineTrapBehavior, RegisterOperation, SystemRegister,
    };

    pub(crate) fn effects(read: bool) -> MachineEncodedEffects {
        let physical = crate::aarch64_physical_register_model_ref();
        let scratch = if read {
            &["x9", "x15"][..]
        } else {
            &["x9", "x10", "x11", "x15"][..]
        };
        let mut clobbers = scratch
            .iter()
            .flat_map(|name| {
                physical
                    .view_named(name)
                    .expect("canonical AArch64 scratch register")
                    .units
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        clobbers.sort_unstable();
        clobbers.dedup();
        MachineEncodedEffects {
            implicit_unit_clobbers: clobbers,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            ..MachineEncodedEffects::fallthrough_v1(
                if read { vec![] } else { vec![0] },
                if read { vec![0] } else { vec![] },
            )
        }
    }

    pub(crate) fn declaration(
        read: bool,
        constraint: RegisterConstraintKey,
    ) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            if read {
                MachineSemanticKind::SystemRegisterRead
            } else {
                MachineSemanticKind::SystemRegisterWrite
            },
            constraint,
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            MachineBarrier::ExternalEffect,
            if read {
                MachineAlternativeFamily::SystemRegisterRead
            } else {
                MachineAlternativeFamily::SystemRegisterWrite
            },
            MachineSizeKnowledge::ExactBytes(4),
            effects(read),
        )
    }

    pub(crate) fn encode(
        operation: RegisterOperation,
        registers: &[u8],
    ) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
        let [operand] = registers else {
            return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
        };
        if *operand > 30 || !operation.is_admitted() {
            return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
        }
        let (base, register) = match operation {
            RegisterOperation::SystemRead(register) => (0xd520_0000, register),
            RegisterOperation::SystemWrite(register) => (0xd500_0000, register),
            _ => return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch),
        };
        let selector: u16 = match register {
            SystemRegister::SctlrEl1 => 0xc080,
            SystemRegister::TcrEl1 => 0xc102,
            SystemRegister::Ttbr0El1 => 0xc100,
            SystemRegister::Ttbr1El1 => 0xc101,
            SystemRegister::MairEl1 => 0xc510,
            SystemRegister::VbarEl1 => 0xc600,
            SystemRegister::TpidrEl1 => 0xc684,
            SystemRegister::EsrEl1 => 0xc290,
            SystemRegister::FarEl1 => 0xc300,
            SystemRegister::Rflags | SystemRegister::Control(_) => {
                return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
            }
        };
        let word = base | (u32::from(selector) << 5) | u32::from(*operand);
        Ok(word.to_le_bytes().to_vec())
    }

    pub(crate) fn decode(word: u32) -> Option<(RegisterOperation, u8)> {
        let read = match word & 0xffe0_0000 {
            0xd520_0000 => true,
            0xd500_0000 => false,
            _ => return None,
        };
        let register = match (word >> 5) & 0xffff {
            0xc080 => SystemRegister::SctlrEl1,
            0xc102 => SystemRegister::TcrEl1,
            0xc100 => SystemRegister::Ttbr0El1,
            0xc101 => SystemRegister::Ttbr1El1,
            0xc510 => SystemRegister::MairEl1,
            0xc600 => SystemRegister::VbarEl1,
            0xc684 => SystemRegister::TpidrEl1,
            0xc290 => SystemRegister::EsrEl1,
            0xc300 => SystemRegister::FarEl1,
            _ => return None,
        };
        let operation = if read {
            RegisterOperation::SystemRead(register)
        } else {
            RegisterOperation::SystemWrite(register)
        };
        operation
            .is_admitted()
            .then_some((operation, (word & 31) as u8))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use selected_instructions::{MachineAlternativeKey, SelectedInstructionKind};

        #[test]
        fn system_register_transfers_match_llvm_and_reject_changed_instruction_fields() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let vectors = [
                (
                    SystemRegister::SctlrEl1,
                    0xd5381013_u32,
                    Some(0xd5181014_u32),
                ),
                (SystemRegister::TcrEl1, 0xd5382053, Some(0xd5182054)),
                (SystemRegister::Ttbr0El1, 0xd5382013, Some(0xd5182014)),
                (SystemRegister::Ttbr1El1, 0xd5382033, Some(0xd5182034)),
                (SystemRegister::MairEl1, 0xd538a213, Some(0xd518a214)),
                (SystemRegister::VbarEl1, 0xd538c013, Some(0xd518c014)),
                (SystemRegister::TpidrEl1, 0xd538d093, Some(0xd518d094)),
                (SystemRegister::EsrEl1, 0xd5385213, None),
                (SystemRegister::FarEl1, 0xd5386013, None),
            ];
            for (register, read_word, write_word) in vectors {
                for (read, word) in [(true, Some(read_word)), (false, write_word)] {
                    let Some(word) = word else {
                        continue;
                    };
                    let operation = if read {
                        RegisterOperation::SystemRead(register)
                    } else {
                        RegisterOperation::SystemWrite(register)
                    };
                    let operand = physical
                        .model()
                        .view_named(if read { "x19" } else { "x20" })
                        .unwrap()
                        .id;
                    let kind = SelectedInstructionKind::RegisterAccess { operation };
                    let alternative = MachineAlternativeKey {
                        family: if read {
                            MachineAlternativeFamily::SystemRegisterRead
                        } else {
                            MachineAlternativeFamily::SystemRegisterWrite
                        },
                        variant: 0,
                    };
                    let encoded = crate::encode_aarch64_selected_form(
                        &physical,
                        kind,
                        alternative,
                        &[operand],
                    )
                    .unwrap();
                    assert_eq!(encoded.bytes(), &word.to_le_bytes());
                    assert_eq!(encoded.footprint().encoded, effects(read));
                    assert_eq!(
                        encoded.footprint().register_reads,
                        if read { vec![] } else { vec![operand] }
                    );
                    assert_eq!(
                        encoded.footprint().register_writes,
                        if read { vec![operand] } else { vec![] }
                    );
                    for bit in 0..32 {
                        let changed = (word ^ (1 << bit)).to_le_bytes();
                        assert!(
                            crate::validate_aarch64_selected_form_encoding(
                                &physical,
                                kind,
                                alternative,
                                &[operand],
                                &changed
                            )
                            .is_err(),
                            "{operation:?}, bit {bit}"
                        );
                    }
                    let mut extended = encoded.bytes().to_vec();
                    extended.push(0);
                    for bytes in [&extended[..], &encoded.bytes()[..3]] {
                        assert!(
                            crate::validate_aarch64_selected_form_encoding(
                                &physical,
                                kind,
                                alternative,
                                &[operand],
                                bytes
                            )
                            .is_err()
                        );
                    }
                    let narrow = physical.model().view_named("w19").unwrap().id;
                    assert!(
                        crate::encode_aarch64_selected_form(
                            &physical,
                            kind,
                            alternative,
                            &[narrow]
                        )
                        .is_err()
                    );
                    assert!(
                        crate::encode_aarch64_selected_form(&physical, kind, alternative, &[])
                            .is_err()
                    );
                    assert!(
                        crate::encode_aarch64_selected_form(
                            &physical,
                            kind,
                            MachineAlternativeKey {
                                variant: 1,
                                ..alternative
                            },
                            &[operand]
                        )
                        .is_err()
                    );
                }
            }
            for register in [SystemRegister::EsrEl1, SystemRegister::FarEl1] {
                assert!(encode(RegisterOperation::SystemWrite(register), &[20]).is_err());
            }
            assert!(decode(0xd518_5214).is_none());
            assert!(decode(0xd518_6014).is_none());
            assert!(decode(0xd538_0000).is_none());
        }
    }
}
mod scan_byte_sequence {
    use super::plan_words::{self, COND_EQ, COND_HI, COND_HS, COND_LO, Instruction};
    use super::{
        Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding, decode_all,
    };
    use crate::selected_form_encoding::selected_forms::request_registers;
    use register_model::RegisterViewId;
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::ByteSequencePredicate;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::MachineEncodedMemoryEffect;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Label {
        Loop,
        Done,
        Fail,
        Advance1,
        Cont1,
        Cont2Any,
        Lead3,
        E0Path,
        EdPath,
        Cont3Any,
        F0Path,
        F4Path,
    }

    fn compare_immediate_branch(
        label: Option<Label>,
        register: u8,
        immediate: u16,
        condition: u8,
        target: Label,
    ) -> [(Option<Label>, Instruction<Label>); 2] {
        [
            (
                label,
                Instruction::CompareImmediate {
                    register,
                    immediate,
                },
            ),
            (None, Instruction::BranchConditional { condition, target }),
        ]
    }

    macro_rules! branch {
        ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr) => {
            compare_immediate_branch($a, $b, $c, $d, $e)
        };
    }

    fn layout(
        predicate: ByteSequencePredicate,
        registers: [u8; 5],
    ) -> Vec<(Option<Label>, Instruction<Label>)> {
        let [base, count, cursor, byte, result] = registers;
        let mut out: Vec<(Option<Label>, Instruction<Label>)> = Vec::new();
        out.extend([
            (None, Instruction::Zero(cursor)),
            (None, Instruction::Zero(byte)),
            (None, Instruction::Zero(result)),
        ]);
        if predicate == ByteSequencePredicate::NonEmpty {
            out.extend([
                (None, Instruction::Cbz(count, Label::Done)),
                (None, Instruction::One(result)),
                (Some(Label::Done), Instruction::Zero(cursor)),
            ]);
            return out;
        }
        out.push((None, Instruction::One(result)));
        match predicate {
            ByteSequencePredicate::NoNul | ByteSequencePredicate::AsciiOnly => {
                out.push((None, Instruction::Cbz(count, Label::Done)));
                out.push((
                    Some(Label::Loop),
                    Instruction::LdrbRegister {
                        destination: byte,
                        base,
                        index: cursor,
                    },
                ));
                out.push((
                    None,
                    if predicate == ByteSequencePredicate::NoNul {
                        Instruction::Cbz(byte, Label::Fail)
                    } else {
                        Instruction::Tbnz7(byte, Label::Fail)
                    },
                ));
                out.extend([
                    (
                        None,
                        Instruction::AddImmediate {
                            destination: cursor,
                            source: cursor,
                            immediate: 1,
                        },
                    ),
                    (
                        None,
                        Instruction::CompareRegister {
                            left: cursor,
                            right: count,
                        },
                    ),
                    (
                        None,
                        Instruction::BranchConditional {
                            condition: COND_LO,
                            target: Label::Loop,
                        },
                    ),
                    (None, Instruction::Branch(Label::Done)),
                    (Some(Label::Fail), Instruction::Zero(result)),
                    (Some(Label::Done), Instruction::Zero(cursor)),
                ]);
                return out;
            }
            _ => {}
        }
        out.extend([
            (None, Instruction::Cbz(count, Label::Done)),
            (
                Some(Label::Loop),
                Instruction::CompareRegister {
                    left: cursor,
                    right: count,
                },
            ),
            (
                None,
                Instruction::BranchConditional {
                    condition: COND_HS,
                    target: Label::Done,
                },
            ),
            (
                None,
                Instruction::LdrbRegister {
                    destination: byte,
                    base,
                    index: cursor,
                },
            ),
        ]);
        out.extend(branch!(None => byte => 0x80 => COND_LO => Label::Advance1));
        out.extend(branch!(None => byte => 0xC2 => COND_LO => Label::Fail));
        out.extend(branch!(None => byte => 0xE0 => COND_LO => Label::Cont1));
        out.extend(branch!(None => byte => 0xF0 => COND_HS => Label::Lead3));
        out.extend(branch!(None => byte => 0xE0 => COND_EQ => Label::E0Path));
        out.extend(branch!(None => byte => 0xED => COND_EQ => Label::EdPath));
        out.push((None, Instruction::Branch(Label::Cont2Any)));
        out.extend(branch!(Some(Label::Lead3) => byte => 0xF5 => COND_HS => Label::Fail));
        out.extend(branch!(None => byte => 0xF0 => COND_EQ => Label::F0Path));
        out.extend(branch!(None => byte => 0xF4 => COND_EQ => Label::F4Path));
        out.push((None, Instruction::Branch(Label::Cont3Any)));
        for (label, width, low, high) in [
            (Label::Cont1, 1u16, 0x80u16, 0xBFu16),
            (Label::Cont2Any, 2, 0x80, 0xBF),
            (Label::E0Path, 2, 0xA0, 0xBF),
            (Label::EdPath, 2, 0x80, 0x9F),
            (Label::Cont3Any, 3, 0x80, 0xBF),
            (Label::F0Path, 3, 0x90, 0xBF),
            (Label::F4Path, 3, 0x80, 0x8F),
        ] {
            emit_continuation_tail(&mut out, label, width, low, high, registers);
        }
        out.extend([
            (
                Some(Label::Advance1),
                Instruction::AddImmediate {
                    destination: cursor,
                    source: cursor,
                    immediate: 1,
                },
            ),
            (None, Instruction::Branch(Label::Loop)),
            (Some(Label::Fail), Instruction::Zero(result)),
            (Some(Label::Done), Instruction::Zero(cursor)),
        ]);
        out
    }

    fn emit_continuation_tail(
        out: &mut Vec<(Option<Label>, Instruction<Label>)>,
        label: Label,
        width: u16,
        low: u16,
        high: u16,
        registers: [u8; 5],
    ) {
        let [base, count, cursor, byte, _result] = registers;
        out.push((
            Some(label),
            Instruction::AddImmediate {
                destination: byte,
                source: cursor,
                immediate: width,
            },
        ));
        out.extend([
            (
                None,
                Instruction::CompareRegister {
                    left: byte,
                    right: count,
                },
            ),
            (
                None,
                Instruction::BranchConditional {
                    condition: COND_HS,
                    target: Label::Fail,
                },
            ),
        ]);
        for offset in 1..=width {
            out.extend([
                (
                    None,
                    Instruction::AddImmediate {
                        destination: byte,
                        source: cursor,
                        immediate: offset,
                    },
                ),
                (
                    None,
                    Instruction::LdrbRegister {
                        destination: byte,
                        base,
                        index: byte,
                    },
                ),
            ]);
            let (floor, ceiling) = if offset == 1 {
                (low, high)
            } else {
                (0x80, 0xBF)
            };
            out.extend(branch!(None => byte => floor => COND_LO => Label::Fail));
            out.extend(branch!(None => byte => ceiling => COND_HI => Label::Fail));
        }
        out.extend([
            (
                None,
                Instruction::AddImmediate {
                    destination: cursor,
                    source: cursor,
                    immediate: width + 1,
                },
            ),
            (None, Instruction::Branch(Label::Loop)),
        ]);
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<[u8; 5], Aarch64SelectedFormEncodingError> {
        request_registers(
            physical,
            alternative,
            MachineAlternativeFamily::ScanByteSequence,
            operands,
            2,
        )
    }

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        predicate: ByteSequencePredicate,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let registers = request(physical, alternative, operands)?;
        let bytes = plan_words::assemble(&layout(predicate, registers))?;
        validate(physical, predicate, alternative, operands, &bytes)
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        predicate: ByteSequencePredicate,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let registers = request(physical, alternative, operands)?;
        if decode_all(bytes, plan_words::decode_word)?
            != plan_words::expected_stream(&layout(predicate, registers))?
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        super::nzcv_effects_encoding(
            physical,
            operands,
            bytes,
            2,
            MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                base_pointer_operand: 0,
                count_operand: 1,
            },
        )
    }

    #[cfg(test)]
    mod tests {
        use super::{encode, validate};
        use crate::aarch64_physical_register_model;
        use crate::selected_form_encoding::selected_forms::{
            encode_aarch64_selected_form, validate_aarch64_selected_form_encoding,
        };
        use crate::test_support::reject_every_bit_flip;
        use selected_instructions::ByteSequencePredicate;
        use selected_instructions::MachineAlternativeFamily;
        use selected_instructions::MachineAlternativeKey;
        use selected_instructions::SelectedInstructionKind;

        fn kind(predicate: ByteSequencePredicate) -> SelectedInstructionKind {
            SelectedInstructionKind::ScanByteSequence { predicate }
        }

        #[test]
        fn constraint_catalog_declares_the_scan_register_contract() {
            let physical =
                register_model::validate_physical_register_model(aarch64_physical_register_model())
                    .unwrap();
            let catalog = crate::aarch64_register_constraint_catalog(&physical);
            let row = catalog
                .constraint(crate::AARCH64_SCAN_BYTE_SEQUENCE)
                .expect("catalog declares the scan constraint");
            assert_eq!(row.operands.len(), 5);
            assert!(row.operands[0].access == register_model::RegisterOperandAccess::Use);
            assert!(row.operands[1].access == register_model::RegisterOperandAccess::Use);
            for scratch in &row.operands[2..] {
                assert!(scratch.early_clobber);
            }
        }

        #[test]
        fn encode_and_validate_replay_every_predicate_word_for_word() {
            let physical =
                register_model::validate_physical_register_model(aarch64_physical_register_model())
                    .unwrap();
            let operands = ["x0", "x1", "x2", "x3", "x4"]
                .map(|name| physical.model().view_named(name).unwrap().id);
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::ScanByteSequence,
                variant: 0,
            };
            for predicate in ByteSequencePredicate::ALL {
                let encoded = encode(&physical, predicate, alternative, &operands).unwrap();
                let through_selected_form = encode_aarch64_selected_form(
                    &physical,
                    kind(predicate),
                    alternative,
                    &operands,
                )
                .unwrap();
                assert_eq!(encoded.bytes, through_selected_form.bytes);
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind(predicate),
                    alternative,
                    &operands,
                    &encoded.bytes,
                )
                .unwrap();
                reject_every_bit_flip(&encoded.bytes, &format!("{predicate:?}"), |changed| {
                    validate(&physical, predicate, alternative, &operands, changed).is_err()
                });
            }
        }
    }
}
pub(crate) mod trapping_forms {
    use super::decoding::DecodedWord;
    use register_model::RegisterConstraintKey;
    use selected_instructions::{
        MachineAlternativeFamily, MachineBarrier, MachineEffectDeclaration,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedTrapBehavior,
        MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
        SaturatingCarrier, TrappingForm, TrappingOperation,
    };
    use semantic_vocabulary::IntegerSign;

    const EQUAL: u8 = 0;
    const CARRY_SET: u8 = 2;
    const CARRY_CLEAR: u8 = 3;
    const OVERFLOW_CLEAR: u8 = 7;

    const fn extension(carrier: SaturatingCarrier) -> u8 {
        match carrier {
            SaturatingCarrier::U8 => 0,
            SaturatingCarrier::U16 => 1,
            SaturatingCarrier::U32 => 2,
            SaturatingCarrier::I8 => 4,
            SaturatingCarrier::I16 => 5,
            SaturatingCarrier::I32 => 6,
            SaturatingCarrier::I64 | SaturatingCarrier::U64 => {
                panic!("64-bit carriers have no narrow extension check")
            }
        }
    }

    pub(crate) fn effects(reads: Vec<u16>, writes: Vec<u16>) -> MachineEncodedEffects {
        let physical = crate::aarch64_physical_register_model();
        MachineEncodedEffects {
            implicit_unit_clobbers: physical
                .view_named("nzcv")
                .expect("canonical AArch64 model declares nzcv")
                .units
                .clone(),
            trap: MachineEncodedTrapBehavior::TrappingIntegerV1,
            control: MachineEncodedControlEffect::FallThroughOrTrapV1,
            ..MachineEncodedEffects::fallthrough_v1(reads, writes)
        }
    }

    pub(crate) fn declaration(
        form: TrappingForm,
        constraint: RegisterConstraintKey,
    ) -> MachineEffectDeclaration {
        let sources = form.source_count() as u16;
        MachineEffectDeclaration::single(
            MachineSemanticKind::TrappingInteger(form),
            constraint,
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::TrappingIntegerV1,
            MachineBarrier::ExternalEffect,
            MachineAlternativeFamily::TrappingInteger(form),
            MachineSizeKnowledge::ExactBytes(byte_size(form)),
            effects(
                (0..sources).collect(),
                (sources..operand_count(form) as u16).collect(),
            ),
        )
    }

    pub(crate) const fn operand_count(form: TrappingForm) -> usize {
        form.source_count() + 2
    }

    const fn conversion_high_bit(source: IntegerSign, carrier: SaturatingCarrier) -> Option<u8> {
        match (source, carrier) {
            (IntegerSign::Signed, SaturatingCarrier::I64)
            | (IntegerSign::Unsigned, SaturatingCarrier::U64) => None,
            (IntegerSign::Signed, carrier) if carrier.is_signed() => None,
            (IntegerSign::Signed, SaturatingCarrier::U64) => Some(63),
            (_, carrier) if carrier.is_signed() => Some(carrier.bits() as u8 - 1),
            (_, carrier) => Some(carrier.bits() as u8),
        }
    }

    pub(crate) fn realization(form: TrappingForm, registers: &[u8]) -> Vec<DecodedWord> {
        let carrier = form.carrier;
        let skip = |condition: u8| DecodedWord::BranchOverTrap { condition };
        let narrow_check = |value: u8| {
            [
                DecodedWord::CompareExtended {
                    left: value,
                    right: value,
                    extension: extension(carrier),
                },
                skip(EQUAL),
                DecodedWord::Crash,
            ]
        };
        let mut words = Vec::new();
        if let TrappingOperation::Convert { source } = form.operation {
            let (operand, result) = (registers[0], registers[1]);
            match (source, conversion_high_bit(source, carrier)) {
                (_, Some(bit)) => words.extend([
                    DecodedWord::TestHighBits {
                        source: operand,
                        from_bit: bit,
                    },
                    skip(EQUAL),
                    DecodedWord::Crash,
                ]),
                (IntegerSign::Signed, None) if carrier != SaturatingCarrier::I64 => {
                    words.extend(narrow_check(operand));
                }
                (_, None) => {}
            }
            words.push(DecodedWord::Copy {
                source: operand,
                destination: result,
            });
            return words;
        }
        let (left, right, value, scratch) =
            (registers[0], registers[1], registers[2], registers[3]);
        match form.operation {
            TrappingOperation::Add | TrappingOperation::Subtract if !carrier.is_narrow() => {
                let destination = value;
                words.push(if form.operation == TrappingOperation::Add {
                    DecodedWord::AddWithFlags {
                        left,
                        right,
                        destination,
                    }
                } else {
                    DecodedWord::SubtractWithFlags {
                        left,
                        right,
                        destination,
                    }
                });
                words.push(skip(match (form.operation, carrier.is_signed()) {
                    (_, true) => OVERFLOW_CLEAR,
                    (TrappingOperation::Add, false) => CARRY_CLEAR,
                    _ => CARRY_SET,
                }));
                words.push(DecodedWord::Crash);
            }
            TrappingOperation::Add | TrappingOperation::Subtract | TrappingOperation::Multiply
                if carrier.is_narrow() =>
            {
                let destination = value;
                words.push(match form.operation {
                    TrappingOperation::Add => DecodedWord::Add {
                        left,
                        right,
                        destination,
                    },
                    TrappingOperation::Subtract => DecodedWord::Subtract {
                        left,
                        right,
                        destination,
                    },
                    _ => DecodedWord::Multiply {
                        left,
                        right,
                        destination,
                    },
                });
                words.extend(narrow_check(value));
            }
            TrappingOperation::Multiply => {
                words.push(if carrier.is_signed() {
                    DecodedWord::SignedMultiplyHigh {
                        left,
                        right,
                        destination: scratch,
                    }
                } else {
                    DecodedWord::UnsignedMultiplyHigh {
                        left,
                        right,
                        destination: scratch,
                    }
                });
                words.push(DecodedWord::Multiply {
                    left,
                    right,
                    destination: value,
                });
                words.push(if carrier.is_signed() {
                    DecodedWord::CompareWithSignOf {
                        high: scratch,
                        low: value,
                    }
                } else {
                    DecodedWord::CompareZero { source: scratch }
                });
                words.push(skip(EQUAL));
                words.push(DecodedWord::Crash);
            }
            TrappingOperation::Divide | TrappingOperation::Remainder => {
                words.push(DecodedWord::BranchNonZeroOverTrap { register: right });
                words.push(DecodedWord::Crash);
                if carrier == SaturatingCarrier::I64 {
                    words.push(DecodedWord::CompareNegativeOne { source: right });
                    words.push(DecodedWord::ConditionalCompareOneOnEqual { register: left });
                    words.push(skip(OVERFLOW_CLEAR));
                    words.push(DecodedWord::Crash);
                }
                let quotient = if form.operation == TrappingOperation::Divide {
                    value
                } else {
                    scratch
                };
                words.push(if carrier.is_signed() {
                    DecodedWord::SignedDivide {
                        dividend: left,
                        divisor: right,
                        destination: quotient,
                    }
                } else {
                    DecodedWord::UnsignedDivide {
                        dividend: left,
                        divisor: right,
                        destination: quotient,
                    }
                });
                if carrier.is_signed() && carrier.is_narrow() {
                    words.extend(narrow_check(quotient));
                }
                if form.operation == TrappingOperation::Remainder {
                    words.push(DecodedWord::MultiplySubtract {
                        left: scratch,
                        right,
                        minuend: left,
                        destination: value,
                    });
                }
            }
            TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => {
                let (shifted, count) = (left, right);
                words.push(DecodedWord::CompareImmediate {
                    source: count,
                    immediate: carrier.bits(),
                });
                words.push(skip(CARRY_CLEAR));
                words.push(DecodedWord::Crash);
                if form.operation == TrappingOperation::ShiftRight {
                    words.push(if carrier.is_signed() {
                        DecodedWord::ShiftRightArithmeticVariable {
                            value: shifted,
                            count,
                            destination: value,
                        }
                    } else {
                        DecodedWord::ShiftRightLogicalVariable {
                            value: shifted,
                            count,
                            destination: value,
                        }
                    });
                    return words;
                }
                words.push(DecodedWord::ShiftLeftVariable {
                    value: shifted,
                    count,
                    destination: value,
                });
                if carrier.is_narrow() {
                    words.extend(narrow_check(value));
                } else {
                    words.push(if carrier.is_signed() {
                        DecodedWord::ShiftRightArithmeticVariable {
                            value,
                            count,
                            destination: scratch,
                        }
                    } else {
                        DecodedWord::ShiftRightLogicalVariable {
                            value,
                            count,
                            destination: scratch,
                        }
                    });
                    words.push(DecodedWord::Compare {
                        left: scratch,
                        right: shifted,
                    });
                    words.push(skip(EQUAL));
                    words.push(DecodedWord::Crash);
                }
            }
            TrappingOperation::Add | TrappingOperation::Subtract => {
                unreachable!("narrow and 64-bit add/subtract are both matched above")
            }
            TrappingOperation::Convert { .. } => unreachable!("conversions return above"),
        }
        words
    }

    pub(crate) fn byte_size(form: TrappingForm) -> u16 {
        let registers: Vec<u8> = (1..=operand_count(form) as u8).collect();
        4 * realization(form, &registers).len() as u16
    }

    pub(crate) fn encode_word(word: DecodedWord) -> u32 {
        let three = |opcode: u32, left: u8, right: u8, destination: u8| {
            opcode | (u32::from(right) << 16) | (u32::from(left) << 5) | u32::from(destination)
        };
        match word {
            DecodedWord::Crash => 0xd420_0000,
            DecodedWord::BranchOverTrap { condition } => 0x5400_0040 | u32::from(condition),
            DecodedWord::BranchNonZeroOverTrap { register } => 0xb500_0040 | u32::from(register),
            DecodedWord::CompareExtended {
                left,
                right,
                extension,
            } => {
                0xeb20_001f
                    | (u32::from(right) << 16)
                    | (u32::from(extension) << 13)
                    | (u32::from(left) << 5)
            }
            DecodedWord::CompareNegativeOne { source } => 0xb100_041f | (u32::from(source) << 5),
            DecodedWord::ConditionalCompareOneOnEqual { register } => {
                0xfa41_0800 | (u32::from(register) << 5)
            }
            DecodedWord::TestHighBits { source, from_bit } => {
                0xf240_001f
                    | ((64 - u32::from(from_bit)) << 16)
                    | ((63 - u32::from(from_bit)) << 10)
                    | (u32::from(source) << 5)
            }
            DecodedWord::AddWithFlags {
                left,
                right,
                destination,
            } => three(0xab00_0000, left, right, destination),
            DecodedWord::SubtractWithFlags {
                left,
                right,
                destination,
            } => three(0xeb00_0000, left, right, destination),
            DecodedWord::Add {
                left,
                right,
                destination,
            } => three(0x8b00_0000, left, right, destination),
            DecodedWord::Subtract {
                left,
                right,
                destination,
            } => three(0xcb00_0000, left, right, destination),
            DecodedWord::Multiply {
                left,
                right,
                destination,
            } => three(0x9b00_7c00, left, right, destination),
            DecodedWord::SignedMultiplyHigh {
                left,
                right,
                destination,
            } => three(0x9b40_7c00, left, right, destination),
            DecodedWord::UnsignedMultiplyHigh {
                left,
                right,
                destination,
            } => three(0x9bc0_7c00, left, right, destination),
            DecodedWord::CompareWithSignOf { high, low } => {
                0xeb80_fc1f | (u32::from(low) << 16) | (u32::from(high) << 5)
            }
            DecodedWord::CompareZero { source } => 0xf100_001f | (u32::from(source) << 5),
            DecodedWord::CompareImmediate { source, immediate } => {
                0xf100_001f | (u32::from(immediate) << 10) | (u32::from(source) << 5)
            }
            DecodedWord::Compare { left, right } => {
                0xeb00_001f | (u32::from(right) << 16) | (u32::from(left) << 5)
            }
            DecodedWord::SignedDivide {
                dividend,
                divisor,
                destination,
            } => three(0x9ac0_0c00, dividend, divisor, destination),
            DecodedWord::UnsignedDivide {
                dividend,
                divisor,
                destination,
            } => three(0x9ac0_0800, dividend, divisor, destination),
            DecodedWord::MultiplySubtract {
                left,
                right,
                minuend,
                destination,
            } => {
                0x9b00_8000
                    | (u32::from(right) << 16)
                    | (u32::from(minuend) << 10)
                    | (u32::from(left) << 5)
                    | u32::from(destination)
            }
            DecodedWord::ShiftLeftVariable {
                value,
                count,
                destination,
            } => three(0x9ac0_2000, value, count, destination),
            DecodedWord::ShiftRightLogicalVariable {
                value,
                count,
                destination,
            } => three(0x9ac0_2400, value, count, destination),
            DecodedWord::ShiftRightArithmeticVariable {
                value,
                count,
                destination,
            } => three(0x9ac0_2800, value, count, destination),
            DecodedWord::Copy {
                source,
                destination,
            } => 0xaa00_03e0 | (u32::from(source) << 16) | u32::from(destination),
            other => unreachable!("{other:?} is not a Trapping realization word"),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{byte_size, realization};
        use crate::aarch64_physical_register_model;
        use crate::selected_form_encoding::decoding::{DecodedWord, decode_words};
        use crate::selected_form_encoding::{
            encode_aarch64_selected_form, validate_aarch64_selected_form_encoding,
        };
        use crate::test_support::rejects_every_bit_flip;
        use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
        use selected_instructions::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
            MachineEncodedTrapBehavior, SaturatingCarrier, SelectedInstructionKind, TrappingForm,
            TrappingOperation,
        };
        use semantic_vocabulary::IntegerSign;

        macro_rules! r {
            ($op:ident => $carrier:ident => $words:expr) => {
                (TrappingOperation::$op, SaturatingCarrier::$carrier, $words)
            };
        }

        const fn convert_words(
            source: IntegerSign,
            carrier: SaturatingCarrier,
            words: &'static [u32],
        ) -> (TrappingOperation, SaturatingCarrier, &'static [u32]) {
            (TrappingOperation::Convert { source }, carrier, words)
        }

        const CLANG_TRAPPING_WORDS: [(TrappingOperation, SaturatingCarrier, &[u32]); 72] = [
            r!(Add => I8 => &[0x8b02_0023, 0xeb23_807f, 0x5400_0040, 0xd420_0000]),
            r!(Add => I16 => &[0x8b02_0023, 0xeb23_a07f, 0x5400_0040, 0xd420_0000]),
            r!(Add => I32 => &[0x8b02_0023, 0xeb23_c07f, 0x5400_0040, 0xd420_0000]),
            r!(Add => I64 => &[0xab02_0023, 0x5400_0047, 0xd420_0000]),
            r!(Add => U8 => &[0x8b02_0023, 0xeb23_007f, 0x5400_0040, 0xd420_0000]),
            r!(Add => U16 => &[0x8b02_0023, 0xeb23_207f, 0x5400_0040, 0xd420_0000]),
            r!(Add => U32 => &[0x8b02_0023, 0xeb23_407f, 0x5400_0040, 0xd420_0000]),
            r!(Add => U64 => &[0xab02_0023, 0x5400_0043, 0xd420_0000]),
            r!(Subtract => I8 => &[0xcb02_0023, 0xeb23_807f, 0x5400_0040, 0xd420_0000]),
            r!(Subtract => I16 => &[0xcb02_0023, 0xeb23_a07f, 0x5400_0040, 0xd420_0000]),
            r!(Subtract => I32 => &[0xcb02_0023, 0xeb23_c07f, 0x5400_0040, 0xd420_0000]),
            r!(Subtract => I64 => &[0xeb02_0023, 0x5400_0047, 0xd420_0000]),
            r!(Subtract => U8 => &[0xcb02_0023, 0xeb23_007f, 0x5400_0040, 0xd420_0000]),
            r!(Subtract => U16 => &[0xcb02_0023, 0xeb23_207f, 0x5400_0040, 0xd420_0000]),
            r!(Subtract => U32 => &[0xcb02_0023, 0xeb23_407f, 0x5400_0040, 0xd420_0000]),
            r!(Subtract => U64 => &[0xeb02_0023, 0x5400_0042, 0xd420_0000]),
            r!(Multiply => I8 => &[0x9b02_7c23, 0xeb23_807f, 0x5400_0040, 0xd420_0000]),
            r!(Multiply => I16 => &[0x9b02_7c23, 0xeb23_a07f, 0x5400_0040, 0xd420_0000]),
            r!(Multiply => I32 => &[0x9b02_7c23, 0xeb23_c07f, 0x5400_0040, 0xd420_0000]),
            (
                TrappingOperation::Multiply,
                SaturatingCarrier::I64,
                &[
                    0x9b42_7c24,
                    0x9b02_7c23,
                    0xeb83_fc9f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            r!(Multiply => U8 => &[0x9b02_7c23, 0xeb23_007f, 0x5400_0040, 0xd420_0000]),
            r!(Multiply => U16 => &[0x9b02_7c23, 0xeb23_207f, 0x5400_0040, 0xd420_0000]),
            r!(Multiply => U32 => &[0x9b02_7c23, 0xeb23_407f, 0x5400_0040, 0xd420_0000]),
            (
                TrappingOperation::Multiply,
                SaturatingCarrier::U64,
                &[
                    0x9bc2_7c24,
                    0x9b02_7c23,
                    0xf100_009f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::Divide,
                SaturatingCarrier::I8,
                &[
                    0xb500_0042,
                    0xd420_0000,
                    0x9ac2_0c23,
                    0xeb23_807f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::Divide,
                SaturatingCarrier::I16,
                &[
                    0xb500_0042,
                    0xd420_0000,
                    0x9ac2_0c23,
                    0xeb23_a07f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::Divide,
                SaturatingCarrier::I32,
                &[
                    0xb500_0042,
                    0xd420_0000,
                    0x9ac2_0c23,
                    0xeb23_c07f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::Divide,
                SaturatingCarrier::I64,
                &[
                    0xb500_0042,
                    0xd420_0000,
                    0xb100_045f,
                    0xfa41_0820,
                    0x5400_0047,
                    0xd420_0000,
                    0x9ac2_0c23,
                ],
            ),
            r!(Divide => U8 => &[0xb500_0042, 0xd420_0000, 0x9ac2_0823]),
            r!(Divide => U16 => &[0xb500_0042, 0xd420_0000, 0x9ac2_0823]),
            r!(Divide => U32 => &[0xb500_0042, 0xd420_0000, 0x9ac2_0823]),
            r!(Divide => U64 => &[0xb500_0042, 0xd420_0000, 0x9ac2_0823]),
            (
                TrappingOperation::Remainder,
                SaturatingCarrier::I8,
                &[
                    0xb500_0042,
                    0xd420_0000,
                    0x9ac2_0c24,
                    0xeb24_809f,
                    0x5400_0040,
                    0xd420_0000,
                    0x9b02_8483,
                ],
            ),
            (
                TrappingOperation::Remainder,
                SaturatingCarrier::I16,
                &[
                    0xb500_0042,
                    0xd420_0000,
                    0x9ac2_0c24,
                    0xeb24_a09f,
                    0x5400_0040,
                    0xd420_0000,
                    0x9b02_8483,
                ],
            ),
            (
                TrappingOperation::Remainder,
                SaturatingCarrier::I32,
                &[
                    0xb500_0042,
                    0xd420_0000,
                    0x9ac2_0c24,
                    0xeb24_c09f,
                    0x5400_0040,
                    0xd420_0000,
                    0x9b02_8483,
                ],
            ),
            (
                TrappingOperation::Remainder,
                SaturatingCarrier::I64,
                &[
                    0xb500_0042,
                    0xd420_0000,
                    0xb100_045f,
                    0xfa41_0820,
                    0x5400_0047,
                    0xd420_0000,
                    0x9ac2_0c24,
                    0x9b02_8483,
                ],
            ),
            r!(Remainder => U8 => &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483]),
            r!(Remainder => U16 => &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483]),
            r!(Remainder => U32 => &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483]),
            r!(Remainder => U64 => &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483]),
            (
                TrappingOperation::ShiftLeft,
                SaturatingCarrier::I8,
                &[
                    0xf100_205f,
                    0x5400_0043,
                    0xd420_0000,
                    0x9ac2_2023,
                    0xeb23_807f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::ShiftLeft,
                SaturatingCarrier::I16,
                &[
                    0xf100_405f,
                    0x5400_0043,
                    0xd420_0000,
                    0x9ac2_2023,
                    0xeb23_a07f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::ShiftLeft,
                SaturatingCarrier::I32,
                &[
                    0xf100_805f,
                    0x5400_0043,
                    0xd420_0000,
                    0x9ac2_2023,
                    0xeb23_c07f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::ShiftLeft,
                SaturatingCarrier::I64,
                &[
                    0xf101_005f,
                    0x5400_0043,
                    0xd420_0000,
                    0x9ac2_2023,
                    0x9ac2_2864,
                    0xeb01_009f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::ShiftLeft,
                SaturatingCarrier::U8,
                &[
                    0xf100_205f,
                    0x5400_0043,
                    0xd420_0000,
                    0x9ac2_2023,
                    0xeb23_007f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::ShiftLeft,
                SaturatingCarrier::U16,
                &[
                    0xf100_405f,
                    0x5400_0043,
                    0xd420_0000,
                    0x9ac2_2023,
                    0xeb23_207f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::ShiftLeft,
                SaturatingCarrier::U32,
                &[
                    0xf100_805f,
                    0x5400_0043,
                    0xd420_0000,
                    0x9ac2_2023,
                    0xeb23_407f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            (
                TrappingOperation::ShiftLeft,
                SaturatingCarrier::U64,
                &[
                    0xf101_005f,
                    0x5400_0043,
                    0xd420_0000,
                    0x9ac2_2023,
                    0x9ac2_2464,
                    0xeb01_009f,
                    0x5400_0040,
                    0xd420_0000,
                ],
            ),
            r!(ShiftRight => I8 => &[0xf100_205f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823]),
            r!(ShiftRight => I16 => &[0xf100_405f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823]),
            r!(ShiftRight => I32 => &[0xf100_805f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823]),
            r!(ShiftRight => I64 => &[0xf101_005f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823]),
            r!(ShiftRight => U8 => &[0xf100_205f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423]),
            r!(ShiftRight => U16 => &[0xf100_405f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423]),
            r!(ShiftRight => U32 => &[0xf100_805f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423]),
            r!(ShiftRight => U64 => &[0xf101_005f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423]),
            (
                TrappingOperation::Convert {
                    source: IntegerSign::Signed,
                },
                SaturatingCarrier::I8,
                &[0xeb21_803f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Signed,
                SaturatingCarrier::I16,
                &[0xeb21_a03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Signed,
                SaturatingCarrier::I32,
                &[0xeb21_c03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(IntegerSign::Signed, SaturatingCarrier::I64, &[0xaa01_03e3]),
            convert_words(
                IntegerSign::Signed,
                SaturatingCarrier::U8,
                &[0xf278_dc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Signed,
                SaturatingCarrier::U16,
                &[0xf270_bc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Signed,
                SaturatingCarrier::U32,
                &[0xf260_7c3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Signed,
                SaturatingCarrier::U64,
                &[0xf241_003f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Unsigned,
                SaturatingCarrier::I8,
                &[0xf279_e03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Unsigned,
                SaturatingCarrier::I16,
                &[0xf271_c03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Unsigned,
                SaturatingCarrier::I32,
                &[0xf261_803f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Unsigned,
                SaturatingCarrier::I64,
                &[0xf241_003f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Unsigned,
                SaturatingCarrier::U8,
                &[0xf278_dc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Unsigned,
                SaturatingCarrier::U16,
                &[0xf270_bc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Unsigned,
                SaturatingCarrier::U32,
                &[0xf260_7c3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
            ),
            convert_words(
                IntegerSign::Unsigned,
                SaturatingCarrier::U64,
                &[0xaa01_03e3],
            ),
        ];

        fn physical() -> ValidatedPhysicalRegisterModel {
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap()
        }

        fn view(physical: &ValidatedPhysicalRegisterModel, name: &str) -> RegisterViewId {
            physical.model().view_named(name).unwrap().id
        }

        fn canonical_operands(
            physical: &ValidatedPhysicalRegisterModel,
            form: TrappingForm,
        ) -> Vec<RegisterViewId> {
            let names: &[&str] = if form.source_count() == 1 {
                &["x1", "x3", "x4"]
            } else {
                &["x1", "x2", "x3", "x4"]
            };
            names.iter().map(|name| view(physical, name)).collect()
        }

        fn key(form: TrappingForm) -> MachineAlternativeKey {
            MachineAlternativeKey {
                family: MachineAlternativeFamily::TrappingInteger(form),
                variant: 0,
            }
        }

        fn little_endian(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        #[test]
        fn every_trapping_form_matches_clang_and_declares_its_trap() {
            let physical = physical();
            assert_eq!(TrappingForm::ALL.len(), CLANG_TRAPPING_WORDS.len());
            for (operation, carrier, words) in CLANG_TRAPPING_WORDS {
                let form = TrappingForm { operation, carrier };
                let kind = SelectedInstructionKind::TrappingInteger { form };
                let operands = canonical_operands(&physical, form);
                let encoded = encode_aarch64_selected_form(&physical, kind, key(form), &operands)
                    .unwrap_or_else(|error| panic!("{form:?} encodes: {error:?}"));
                assert_eq!(encoded.bytes(), little_endian(words), "{form:?}");
                assert_eq!(usize::from(byte_size(form)), 4 * words.len(), "{form:?}");
                let footprint = encoded.footprint();
                let sources = form.source_count();
                assert_eq!(footprint.register_reads, operands[..sources]);
                assert_eq!(footprint.register_writes, operands[sources..]);
                assert_eq!(
                    footprint.encoded.implicit_unit_clobbers,
                    physical.model().view_named("nzcv").unwrap().units
                );
                assert!(footprint.encoded.implicit_unit_defs.is_empty());
                assert_eq!(
                    footprint.encoded.trap,
                    MachineEncodedTrapBehavior::TrappingIntegerV1
                );
                assert_eq!(
                    footprint.encoded.control,
                    MachineEncodedControlEffect::FallThroughOrTrapV1
                );
                let decoded = decode_words(encoded.bytes()).unwrap();
                for (index, word) in decoded.iter().enumerate() {
                    if *word == DecodedWord::Crash {
                        assert!(matches!(
                            decoded[index - 1],
                            DecodedWord::BranchOverTrap { .. }
                                | DecodedWord::BranchNonZeroOverTrap { .. }
                        ));
                    }
                }
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Outcome {
            Value(u64),
            Trap,
        }

        #[derive(Default)]
        struct Flags {
            zero: bool,
            carry: bool,
            overflow: bool,
        }

        impl Flags {
            fn subtract(left: u64, right: u64) -> Self {
                let result = left.wrapping_sub(right);
                Self {
                    zero: result == 0,
                    carry: left >= right,
                    overflow: (left as i64).checked_sub(right as i64).is_none(),
                }
            }

            fn add(left: u64, right: u64) -> Self {
                let result = left.wrapping_add(right);
                Self {
                    zero: result == 0,
                    carry: left.checked_add(right).is_none(),
                    overflow: (left as i64).checked_add(right as i64).is_none(),
                }
            }

            fn holds(&self, condition: u8) -> bool {
                match condition {
                    0 => self.zero,
                    1 => !self.zero,
                    2 => self.carry,
                    3 => !self.carry,
                    6 => self.overflow,
                    7 => !self.overflow,
                    _ => panic!("condition {condition} is not a Trapping skip condition"),
                }
            }
        }

        fn extend(value: u64, extension: u8) -> u64 {
            match extension {
                0 => value as u8 as u64,
                1 => value as u16 as u64,
                2 => value as u32 as u64,
                4 => value as i8 as i64 as u64,
                5 => value as i16 as i64 as u64,
                6 => value as i32 as i64 as u64,
                _ => panic!("extension {extension} is not a narrow carrier's"),
            }
        }

        fn execute(decoded: &[DecodedWord], inputs: &[(u8, u64)], result: u8) -> Outcome {
            let mut registers = [0_u64; 32];
            for (register, value) in inputs {
                registers[usize::from(*register)] = *value;
            }
            let mut flags = Flags::default();
            let mut index = 0;
            while index < decoded.len() {
                let mut next = index + 1;
                let read = |register: u8| registers[usize::from(register)];
                match decoded[index] {
                    DecodedWord::Crash => return Outcome::Trap,
                    DecodedWord::BranchOverTrap { condition } => {
                        if flags.holds(condition) {
                            next += 1;
                        }
                    }
                    DecodedWord::BranchNonZeroOverTrap { register } => {
                        if read(register) != 0 {
                            next += 1;
                        }
                    }
                    DecodedWord::AddWithFlags {
                        left,
                        right,
                        destination,
                    } => {
                        flags = Flags::add(read(left), read(right));
                        registers[usize::from(destination)] = read(left).wrapping_add(read(right));
                    }
                    DecodedWord::SubtractWithFlags {
                        left,
                        right,
                        destination,
                    } => {
                        flags = Flags::subtract(read(left), read(right));
                        registers[usize::from(destination)] = read(left).wrapping_sub(read(right));
                    }
                    DecodedWord::Add {
                        left,
                        right,
                        destination,
                    } => {
                        registers[usize::from(destination)] = read(left).wrapping_add(read(right));
                    }
                    DecodedWord::Subtract {
                        left,
                        right,
                        destination,
                    } => {
                        registers[usize::from(destination)] = read(left).wrapping_sub(read(right));
                    }
                    DecodedWord::Multiply {
                        left,
                        right,
                        destination,
                    } => {
                        registers[usize::from(destination)] = read(left).wrapping_mul(read(right));
                    }
                    DecodedWord::SignedMultiplyHigh {
                        left,
                        right,
                        destination,
                    } => {
                        let product =
                            i128::from(read(left) as i64) * i128::from(read(right) as i64);
                        registers[usize::from(destination)] = (product >> 64) as u64;
                    }
                    DecodedWord::UnsignedMultiplyHigh {
                        left,
                        right,
                        destination,
                    } => {
                        let product = u128::from(read(left)) * u128::from(read(right));
                        registers[usize::from(destination)] = (product >> 64) as u64;
                    }
                    DecodedWord::CompareWithSignOf { high, low } => {
                        flags = Flags::subtract(read(high), ((read(low) as i64) >> 63) as u64);
                    }
                    DecodedWord::CompareZero { source } => flags = Flags::subtract(read(source), 0),
                    DecodedWord::CompareImmediate { source, immediate } => {
                        flags = Flags::subtract(read(source), u64::from(immediate));
                    }
                    DecodedWord::Compare { left, right } => {
                        flags = Flags::subtract(read(left), read(right));
                    }
                    DecodedWord::CompareExtended {
                        left,
                        right,
                        extension,
                    } => {
                        flags = Flags::subtract(read(left), extend(read(right), extension));
                    }
                    DecodedWord::CompareNegativeOne { source } => {
                        flags = Flags::add(read(source), 1)
                    }
                    DecodedWord::ConditionalCompareOneOnEqual { register } => {
                        flags = if flags.zero {
                            Flags::subtract(read(register), 1)
                        } else {
                            Flags::default()
                        };
                    }
                    DecodedWord::TestHighBits { source, from_bit } => {
                        let masked = read(source) & (u64::MAX << from_bit);
                        flags = Flags {
                            zero: masked == 0,
                            carry: false,
                            overflow: false,
                        };
                    }
                    DecodedWord::SignedDivide {
                        dividend,
                        divisor,
                        destination,
                    } => {
                        let (dividend, divisor) = (read(dividend) as i64, read(divisor) as i64);
                        registers[usize::from(destination)] = if divisor == 0 {
                            0
                        } else {
                            dividend.wrapping_div(divisor) as u64
                        };
                    }
                    DecodedWord::UnsignedDivide {
                        dividend,
                        divisor,
                        destination,
                    } => {
                        registers[usize::from(destination)] =
                            read(dividend).checked_div(read(divisor)).unwrap_or(0);
                    }
                    DecodedWord::MultiplySubtract {
                        left,
                        right,
                        minuend,
                        destination,
                    } => {
                        registers[usize::from(destination)] =
                            read(minuend).wrapping_sub(read(left).wrapping_mul(read(right)));
                    }
                    DecodedWord::ShiftLeftVariable {
                        value,
                        count,
                        destination,
                    } => {
                        registers[usize::from(destination)] = read(value) << (read(count) & 63);
                    }
                    DecodedWord::ShiftRightLogicalVariable {
                        value,
                        count,
                        destination,
                    } => {
                        registers[usize::from(destination)] = read(value) >> (read(count) & 63);
                    }
                    DecodedWord::ShiftRightArithmeticVariable {
                        value,
                        count,
                        destination,
                    } => {
                        registers[usize::from(destination)] =
                            ((read(value) as i64) >> (read(count) & 63)) as u64;
                    }
                    DecodedWord::Copy {
                        source,
                        destination,
                    } => {
                        registers[usize::from(destination)] = read(source);
                    }
                    other => panic!("{other:?} is not a Trapping realization word"),
                }
                index = next;
            }
            Outcome::Value(registers[usize::from(result)])
        }

        fn bounds(carrier: SaturatingCarrier) -> (i128, i128) {
            let bits = u32::from(carrier.bits());
            if carrier.is_signed() {
                (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
            } else {
                (0, (1_i128 << bits) - 1)
            }
        }

        fn reference(form: TrappingForm, left: i128, right: i128) -> Outcome {
            let (minimum, maximum) = bounds(form.carrier);
            let width = i128::from(form.carrier.bits());
            let exact = match form.operation {
                TrappingOperation::Add => Some(left + right),
                TrappingOperation::Subtract => Some(left - right),
                TrappingOperation::Multiply => left.checked_mul(right),
                TrappingOperation::Divide => (right != 0).then(|| left / right),
                TrappingOperation::Remainder => (right != 0)
                    .then(|| left / right)
                    .filter(|quotient| (minimum..=maximum).contains(quotient))
                    .map(|_| left % right),
                TrappingOperation::ShiftLeft => (0..width).contains(&right).then(|| left << right),
                TrappingOperation::ShiftRight => (0..width).contains(&right).then(|| left >> right),
                TrappingOperation::Convert { .. } => Some(left),
            };
            match exact {
                Some(value) if (minimum..=maximum).contains(&value) => {
                    Outcome::Value(value as i64 as u64)
                }
                _ => Outcome::Trap,
            }
        }

        fn register(value: i128) -> u64 {
            value as i64 as u64
        }

        fn edges(carrier: SaturatingCarrier) -> Vec<i128> {
            let (minimum, maximum) = bounds(carrier);
            let mut values = vec![
                minimum,
                minimum + 1,
                -2,
                -1,
                0,
                1,
                2,
                3,
                7,
                maximum - 1,
                maximum,
            ];
            values.extend([maximum / 2, maximum / 2 + 1, minimum / 2, minimum / 2 - 1]);
            values.retain(|value| (minimum..=maximum).contains(value));
            values.sort_unstable();
            values.dedup();
            values
        }

        fn counts(width: i128) -> Vec<(i128, u64)> {
            [-129, -1, 0, 1, width - 1, width, width + 1, 63, 64, 65, 200]
                .into_iter()
                .map(|count| (count, register(count)))
                .collect()
        }

        #[test]
        fn every_trapping_form_executes_the_policy_answer_at_its_carriers_edges() {
            let mut checked = 0;
            for form in TrappingForm::ALL {
                let registers: &[u8] = if form.source_count() == 1 {
                    &[1, 3, 4]
                } else {
                    &[1, 2, 3, 4]
                };
                let decoded = realization(form, registers);
                let result = registers[form.source_count()];
                match form.operation {
                    TrappingOperation::Convert { source } => {
                        for source_carrier in SaturatingCarrier::ALL
                            .into_iter()
                            .filter(|carrier| carrier.sign() == source)
                        {
                            for value in edges(source_carrier) {
                                let actual = execute(&decoded, &[(1, register(value))], result);
                                assert_eq!(actual, reference(form, value, 0), "{form:?} {value}");
                                checked += 1;
                            }
                        }
                    }
                    TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => {
                        for value in edges(form.carrier) {
                            for (count, count_register) in counts(i128::from(form.carrier.bits())) {
                                let actual = execute(
                                    &decoded,
                                    &[(1, register(value)), (2, count_register)],
                                    result,
                                );
                                assert_eq!(
                                    actual,
                                    reference(form, value, count),
                                    "{form:?} {value} << {count}"
                                );
                                checked += 1;
                            }
                        }
                    }
                    _ => {
                        for left in edges(form.carrier) {
                            for right in edges(form.carrier) {
                                let actual = execute(
                                    &decoded,
                                    &[(1, register(left)), (2, register(right))],
                                    result,
                                );
                                assert_eq!(
                                    actual,
                                    reference(form, left, right),
                                    "{form:?} {left} {right}"
                                );
                                checked += 1;
                            }
                        }
                    }
                }
            }
            assert!(checked > 5_000, "{checked}");
        }

        #[test]
        fn named_trapping_boundaries_trap_or_return_exactly() {
            let form = |operation, carrier| TrappingForm { operation, carrier };
            let run = |form: TrappingForm, left: i128, right: i128| {
                execute(
                    &realization(form, &[1, 2, 3, 4]),
                    &[(1, register(left)), (2, register(right))],
                    3,
                )
            };
            let convert = |source, carrier, value: i128| {
                execute(
                    &realization(
                        TrappingForm {
                            operation: TrappingOperation::Convert { source },
                            carrier,
                        },
                        &[1, 3, 4],
                    ),
                    &[(1, register(value))],
                    3,
                )
            };
            use SaturatingCarrier::{I8, I32, I64, U8, U32, U64};
            use TrappingOperation::{
                Add, Divide, Multiply, Remainder, ShiftLeft, ShiftRight, Subtract,
            };
            assert_eq!(run(form(Add, U8), 200, 100), Outcome::Trap);
            assert_eq!(run(form(Add, U8), 200, 55), Outcome::Value(255));
            assert_eq!(run(form(Add, I32), i128::from(i32::MAX), 1), Outcome::Trap);
            assert_eq!(run(form(Add, I64), i128::from(i64::MAX), 1), Outcome::Trap);
            assert_eq!(run(form(Add, U64), i128::from(u64::MAX), 1), Outcome::Trap);
            assert_eq!(run(form(Subtract, U32), 0, 1), Outcome::Trap);
            assert_eq!(
                run(form(Subtract, I64), i128::from(i64::MIN), 1),
                Outcome::Trap
            );
            assert_eq!(
                run(form(Multiply, U32), i128::from(u32::MAX), 2),
                Outcome::Trap
            );
            assert_eq!(run(form(Multiply, I64), 1 << 32, 1 << 31), Outcome::Trap);
            assert_eq!(
                run(form(Multiply, I64), -(1 << 32), 1 << 31),
                Outcome::Value(i64::MIN as u64)
            );
            assert_eq!(run(form(Multiply, U64), 1 << 32, 1 << 32), Outcome::Trap);
            for carrier in [I8, I32, I64] {
                let (minimum, _) = bounds(carrier);
                assert_eq!(run(form(Divide, carrier), minimum, -1), Outcome::Trap);
                assert_eq!(run(form(Remainder, carrier), minimum, -1), Outcome::Trap);
                assert_eq!(
                    run(form(Divide, carrier), minimum, 1),
                    Outcome::Value(register(minimum))
                );
                assert_eq!(run(form(Divide, carrier), -1, -1), Outcome::Value(1));
                assert_eq!(run(form(Divide, carrier), 7, 0), Outcome::Trap);
                assert_eq!(
                    run(form(Remainder, carrier), -7, 2),
                    Outcome::Value(register(-1))
                );
            }
            assert_eq!(run(form(Divide, U64), 7, 0), Outcome::Trap);
            assert_eq!(
                run(form(Remainder, U64), i128::from(u64::MAX), 10),
                Outcome::Value(5)
            );
            assert_eq!(run(form(ShiftLeft, U32), 1, 32), Outcome::Trap);
            assert_eq!(run(form(ShiftLeft, U32), 1, 31), Outcome::Value(1 << 31));
            assert_eq!(run(form(ShiftLeft, I32), 1, 31), Outcome::Trap);
            assert_eq!(
                run(form(ShiftLeft, I64), -1, 63),
                Outcome::Value(i64::MIN as u64)
            );
            assert_eq!(run(form(ShiftLeft, U64), 3, 63), Outcome::Trap);
            assert_eq!(
                run(form(ShiftRight, I8), -128, 7),
                Outcome::Value(register(-1))
            );
            assert_eq!(run(form(ShiftRight, I8), -128, -1), Outcome::Trap);
            assert_eq!(run(form(ShiftRight, U64), 1, 64), Outcome::Trap);
            assert_eq!(
                convert(IntegerSign::Unsigned, I8, i128::from(u64::MAX)),
                Outcome::Trap
            );
            assert_eq!(convert(IntegerSign::Signed, U64, -1), Outcome::Trap);
            assert_eq!(convert(IntegerSign::Signed, U8, 255), Outcome::Value(255));
            assert_eq!(convert(IntegerSign::Signed, I8, -129), Outcome::Trap);
            assert_eq!(
                convert(IntegerSign::Unsigned, I64, i128::from(i64::MAX) + 1),
                Outcome::Trap
            );
            assert_eq!(
                convert(IntegerSign::Signed, I64, -1),
                Outcome::Value(u64::MAX)
            );
        }

        #[test]
        fn corrupted_trapping_encodings_are_rejected() {
            let physical = physical();
            for form in TrappingForm::ALL {
                let kind = SelectedInstructionKind::TrappingInteger { form };
                let operands = canonical_operands(&physical, form);
                let bytes = encode_aarch64_selected_form(&physical, kind, key(form), &operands)
                    .unwrap()
                    .bytes()
                    .to_vec();
                rejects_every_bit_flip(&physical, kind, key(form), &operands, &bytes);
                assert!(
                    validate_aarch64_selected_form_encoding(
                        &physical,
                        kind,
                        key(form),
                        &operands,
                        &bytes[4..]
                    )
                    .is_err()
                );
                assert!(
                    encode_aarch64_selected_form(&physical, kind, key(form), &operands[1..])
                        .is_err()
                );
                assert!(
                    encode_aarch64_selected_form(
                        &physical,
                        kind,
                        MachineAlternativeKey {
                            variant: 1,
                            ..key(form)
                        },
                        &operands
                    )
                    .is_err()
                );
                let sources = form.source_count();
                for aliased in [0, sources + 1] {
                    let mut shared = operands.clone();
                    shared[sources] = operands[aliased];
                    assert!(
                        encode_aarch64_selected_form(&physical, kind, key(form), &shared).is_err()
                    );
                }
            }
            let form = TrappingForm {
                operation: TrappingOperation::Add,
                carrier: SaturatingCarrier::I32,
            };
            let other = TrappingForm {
                carrier: SaturatingCarrier::I16,
                ..form
            };
            let operands = canonical_operands(&physical, form);
            let bytes = encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::TrappingInteger { form },
                key(form),
                &operands,
            )
            .unwrap()
            .bytes()
            .to_vec();
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    SelectedInstructionKind::TrappingInteger { form: other },
                    key(other),
                    &operands,
                    &bytes
                )
                .is_err()
            );
        }
    }
}
mod zero_bytes {
    use super::{Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding};
    use crate::selected_form_encoding::selected_forms::request_registers;
    use register_model::RegisterViewId;
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::MachineEncodedMemoryEffect;

    const ZERO_REGISTER: u32 = 31;

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<[u8; 3], Aarch64SelectedFormEncodingError> {
        request_registers(
            physical,
            alternative,
            MachineAlternativeFamily::ZeroBytes,
            operands,
            2,
        )
    }

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let [destination, count, cursor] = request(physical, alternative, operands)?.map(u32::from);
        let words = [
            0xd280_0000 | cursor,
            0xb400_00a0 | count,
            0x3820_6800 | (cursor << 16) | (destination << 5) | ZERO_REGISTER,
            0x9100_0400 | (cursor << 5) | cursor,
            0xeb00_001f | (count << 16) | (cursor << 5),
            0x54ff_ffa1,
        ];
        let bytes = words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        validate(physical, alternative, operands, &bytes)
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let [destination, count, cursor] = request(physical, alternative, operands)?.map(u32::from);
        let (word_bytes, tail) = bytes.as_chunks::<4>();
        let word_bytes: &[[u8; 4]; 6] = word_bytes
            .try_into()
            .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
        if !tail.is_empty() {
            return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
        }
        let [initialize, skip, store, advance, compare, repeat] =
            word_bytes.map(u32::from_le_bytes);
        if initialize & !31 != 0xd280_0000
            || initialize & 31 != cursor
            || skip & 0xff00_0000 != 0xb400_0000
            || skip & 31 != count
            || (skip >> 5) & 0x7ffff != 5
            || store & 0xffe0_fc00 != 0x3820_6800
            || store & 31 != ZERO_REGISTER
            || (store >> 5) & 31 != destination
            || (store >> 16) & 31 != cursor
            || advance & !0x3ff != 0x9100_0400
            || advance & 31 != cursor
            || (advance >> 5) & 31 != cursor
            || compare & 0xffe0_fc1f != 0xeb00_001f
            || (compare >> 5) & 31 != cursor
            || (compare >> 16) & 31 != count
            || repeat != 0x54ff_ffa1
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        super::nzcv_effects_encoding(
            physical,
            operands,
            bytes,
            2,
            MachineEncodedMemoryEffect::ZeroBytesV1 {
                destination_pointer_operand: 0,
                count_operand: 1,
            },
        )
    }

    #[cfg(test)]
    mod tests {
        use super::{encode, validate};
        use crate::aarch64_physical_register_model;
        use crate::test_support::reject_every_bit_flip;
        use selected_instructions::MachineAlternativeFamily;
        use selected_instructions::MachineAlternativeKey;

        #[test]
        fn zero_loop_replays_every_bit_and_rejects_cursor_aliases() {
            let physical =
                register_model::validate_physical_register_model(aarch64_physical_register_model())
                    .unwrap();
            let operands =
                ["x0", "x1", "x2"].map(|name| physical.model().view_named(name).unwrap().id);
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::ZeroBytes,
                variant: 0,
            };
            let encoded = encode(&physical, alternative, &operands).unwrap();
            assert_eq!(
                encoded.bytes,
                [
                    0xd280_0002_u32,
                    0xb400_00a1,
                    0x3822_681f,
                    0x9100_0442,
                    0xeb01_005f,
                    0x54ff_ffa1,
                ]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
            );
            assert_eq!(encoded.footprint.register_reads, operands[..2]);
            assert_eq!(encoded.footprint.register_writes, operands[2..]);
            reject_every_bit_flip(&encoded.bytes, "zero word stream", |changed| {
                validate(&physical, alternative, &operands, changed).is_err()
            });
            for length in 0..encoded.bytes.len() {
                assert!(
                    validate(&physical, alternative, &operands, &encoded.bytes[..length]).is_err()
                );
            }
            for input in 0..2 {
                let mut changed = operands;
                changed[2] = changed[input];
                assert!(encode(&physical, alternative, &changed).is_err());
            }
            let copy = MachineAlternativeKey {
                family: MachineAlternativeFamily::CopyBytes,
                variant: 0,
            };
            assert!(encode(&physical, copy, &operands).is_err());
        }
    }
}
pub use floating_control::encode_aarch64_selected_floating_control_form;
pub(crate) mod hosted_exit_process {
    use super::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
        ValidatedAarch64SelectedFormEncoding,
    };
    use crate::selected_form_encoding::selected_forms::resolve_registers;
    use register_model::RegisterConstraintKey;
    use register_model::RegisterViewId;
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineEffectDeclaration,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
        MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior, SelectedInstructionKind,
    };
    use target::NativeTarget;

    pub(crate) fn effects(
        target: NativeTarget,
    ) -> Result<MachineEncodedEffects, Aarch64SelectedFormEncodingError> {
        let physical = crate::aarch64_physical_register_model();
        let view = |name: &str| {
            physical
                .view_named(name)
                .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
        };
        let mut clobbers = Vec::new();
        let syscall_register = if target == NativeTarget::macos_arm64() {
            "x16"
        } else {
            "x8"
        };
        for name in ["x0", syscall_register, "nzcv"] {
            clobbers.extend(view(name)?.units.iter().copied());
        }
        clobbers.sort_unstable();
        clobbers.dedup();
        Ok(MachineEncodedEffects {
            external_operand_reads: vec![0],
            external_operand_writes: Vec::new(),
            implicit_unit_uses: view("pc")?.units.clone(),
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: clobbers,
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::HostedExitReturnedV1,
            control: MachineEncodedControlEffect::HostedExitOrTrapV1,
        })
    }

    pub(crate) fn declaration(
        target: NativeTarget,
        constraint: RegisterConstraintKey,
    ) -> Result<MachineEffectDeclaration, Aarch64SelectedFormEncodingError> {
        Ok(MachineEffectDeclaration::single(
            MachineSemanticKind::HostedExitProcessI32,
            constraint,
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::HostedExitReturnedV1,
            MachineBarrier::ExternalEffect,
            MachineAlternativeFamily::HostedExitProcessI32,
            MachineSizeKnowledge::ExactBytes(16),
            effects(target)?,
        ))
    }

    fn request(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<u8, Aarch64SelectedFormEncodingError> {
        if ![NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target)
            || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || kind != SelectedInstructionKind::HostedExitProcessI32
            || alternative
                != (MachineAlternativeKey {
                    family: MachineAlternativeFamily::HostedExitProcessI32,
                    variant: 0,
                })
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let registers = resolve_registers(physical, operands)?;
        let [register] = registers.as_slice() else {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        };
        Ok(*register)
    }

    pub fn encode_aarch64_selected_hosted_exit_process_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let register = request(target, physical, kind, alternative, operands)?;
        let words = [
            0x2a00_03e0 | (u32::from(register) << 16),
            if target == NativeTarget::macos_arm64() {
                0xd280_0030
            } else {
                0xd280_0bc8
            },
            if target == NativeTarget::macos_arm64() {
                0xd400_1001
            } else {
                0xd400_0001
            },
            0xd420_0000,
        ];
        let bytes = words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        validate_aarch64_selected_hosted_exit_process_form(
            target,
            physical,
            kind,
            alternative,
            operands,
            &bytes,
        )
    }

    pub fn validate_aarch64_selected_hosted_exit_process_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let register = request(target, physical, kind, alternative, operands)?;
        if decode(target, bytes) != Some(register) {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedAarch64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: Aarch64SelectedFormFootprint {
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                writes_nzcv: true,
                encoded: effects(target)?,
            },
        })
    }

    fn decode(target: NativeTarget, bytes: &[u8]) -> Option<u8> {
        if ![NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target)
            || bytes.len() != 16
        {
            return None;
        }
        let words = [
            crate::codec_bytes::u32_at(bytes, 0)?,
            crate::codec_bytes::u32_at(bytes, 4)?,
            crate::codec_bytes::u32_at(bytes, 8)?,
            crate::codec_bytes::u32_at(bytes, 12)?,
        ];
        let register = ((words[0] >> 16) & 31) as u8;
        if words[0] & !0x001f_0000 != 0x2a00_03e0
            || register == 31
            || words[1]
                != if target == NativeTarget::macos_arm64() {
                    0xd280_0030
                } else {
                    0xd280_0bc8
                }
            || words[2]
                != if target == NativeTarget::macos_arm64() {
                    0xd400_1001
                } else {
                    0xd400_0001
                }
            || words[3] != 0xd420_0000
        {
            return None;
        }
        Some(register)
    }

    pub fn decode_aarch64_selected_hosted_exit_process_i32(
        target: NativeTarget,
        bytes: &[u8],
    ) -> Option<calling_conventions::MachineRegister> {
        let register = decode(target, bytes)?;
        Some(calling_conventions::MachineRegister::Aarch64X(register))
    }

    #[cfg(test)]
    mod tests {
        use super::{
            NativeTarget, decode_aarch64_selected_hosted_exit_process_i32, effects,
            encode_aarch64_selected_hosted_exit_process_form,
            validate_aarch64_selected_hosted_exit_process_form,
        };
        use crate::test_support::reject_every_bit_flip;
        use register_model::RegisterViewId;
        use selected_instructions::{
            MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
        };
        #[test]
        fn hosted_exit_replays_target_input_and_every_encoded_bit() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let kind = SelectedInstructionKind::HostedExitProcessI32;
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedExitProcessI32,
                variant: 0,
            };
            let encode = |target, kind, operands: &[RegisterViewId]| {
                encode_aarch64_selected_hosted_exit_process_form(
                    target,
                    &physical,
                    kind,
                    alternative,
                    operands,
                )
            };
            let validate = |target, operands: &[RegisterViewId], bytes: &[u8]| {
                validate_aarch64_selected_hosted_exit_process_form(
                    target,
                    &physical,
                    kind,
                    alternative,
                    operands,
                    bytes,
                )
            };
            for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
                for name in ["x0", "x1", "x8", "x16", "x18", "x30"] {
                    let operands = [physical.model().view_named(name).unwrap().id];
                    let encoded = encode(target, kind, &operands).unwrap();
                    assert_eq!(encoded.footprint().encoded, effects(target).unwrap());
                    reject_every_bit_flip(encoded.bytes(), &format!("{kind:?}"), |corrupted| {
                        validate(target, &operands, corrupted).is_err()
                    });
                    let different = [physical
                        .model()
                        .view_named(if name == "x0" { "x1" } else { "x0" })
                        .unwrap()
                        .id];
                    assert!(validate(target, &different, encoded.bytes()).is_err());
                    assert!(encode(target, kind, &[]).is_err());
                    assert!(encode(target, kind, &[operands[0], operands[0]]).is_err());
                    assert!(
                        encode(target, SelectedInstructionKind::ReturnUnit, &operands).is_err()
                    );
                    for wrong_target in [
                        NativeTarget::windows_x64(),
                        NativeTarget {
                            pointer_size: 4,
                            ..target
                        },
                        if target == NativeTarget::linux_arm64() {
                            NativeTarget::macos_arm64()
                        } else {
                            NativeTarget::linux_arm64()
                        },
                    ] {
                        assert!(validate(wrong_target, &operands, encoded.bytes()).is_err());
                    }
                    let mut extra = encoded.bytes().to_vec();
                    extra.push(0);
                    assert!(
                        decode_aarch64_selected_hosted_exit_process_i32(target, &extra).is_none()
                    );
                }
            }
        }
    }
}
pub(crate) mod hosted_read_byte {
    use super::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
        ValidatedAarch64SelectedFormEncoding,
    };
    use register_model::RegisterConstraintKey;
    use register_model::RegisterViewId;
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier,
        MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
        MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
        SelectedInstructionKind,
    };
    use target::NativeTarget;

    fn size(target: NativeTarget) -> MachineSizeKnowledge {
        let address = crate::immediate_offset::MAX_ADDRESS_BYTES;
        if target == NativeTarget::macos_arm64() {
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 64,
                maximum_bytes: Some(56 + 2 * address),
            }
        } else {
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 56,
                maximum_bytes: Some(52 + address),
            }
        }
    }

    pub(crate) fn effects(target: NativeTarget) -> MachineEncodedEffects {
        let physical = crate::aarch64_physical_register_model();
        let view = |name: &str| {
            physical
                .view_named(name)
                .expect("canonical hosted read register")
        };
        let mut uses = view("sp").units.clone();
        uses.extend(view("pc").units.iter().copied());
        uses.sort_unstable();
        uses.dedup();
        let mut clobbers = Vec::new();
        let syscall_register = if target == NativeTarget::macos_arm64() {
            "x16"
        } else {
            "x8"
        };
        for name in ["x0", "x1", "x2", syscall_register, "x9", "nzcv"] {
            clobbers.extend(view(name).units.iter().copied());
        }
        clobbers.sort_unstable();
        clobbers.dedup();
        MachineEncodedEffects {
            external_operand_reads: Vec::new(),
            external_operand_writes: Vec::new(),
            implicit_unit_uses: uses,
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: clobbers,
            memory: MachineEncodedMemoryEffect::HostedReadByteV1 {
                stack_pointer: view("sp").id,
            },
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::HostedReadFailureV1,
            control: MachineEncodedControlEffect::HostedReadReturnOrTrapV1,
        }
    }

    pub(crate) fn declaration(
        target: NativeTarget,
        constraint: RegisterConstraintKey,
    ) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            MachineSemanticKind::HostedReadByte,
            constraint,
            MachineMemoryEffect::HostedReadByteV1,
            MachineTrapBehavior::HostedReadFailureV1,
            MachineBarrier::ExternalEffect,
            MachineAlternativeFamily::HostedReadByte,
            size(target),
            effects(target),
        )
    }

    fn request(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<(), Aarch64SelectedFormEncodingError> {
        if (target != NativeTarget::linux_arm64() && target != NativeTarget::macos_arm64())
            || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || !matches!(
                kind,
                SelectedInstructionKind::HostedReadByte {
                    slot: LocalStorageSlotId::Structural { .. }
                }
            )
            || alternative
                != (MachineAlternativeKey {
                    family: MachineAlternativeFamily::HostedReadByte,
                    variant: 0,
                })
            || !operands.is_empty()
            || displacement < 0
            || displacement % 4 != 0
            || displacement
                .checked_add(4)
                .is_none_or(|payload| u32::try_from(payload).is_err())
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(())
    }

    pub fn encode_aarch64_selected_hosted_read_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        request(target, physical, kind, alternative, operands, displacement)?;
        let displacement = u32::try_from(displacement)
            .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        let bytes = if target == NativeTarget::macos_arm64() {
            crate::encode_macos_read_byte_to_stack(displacement, displacement + 4)
        } else {
            crate::encode_linux_read_byte_to_stack(displacement, displacement + 4)
        }
        .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        validate_aarch64_selected_hosted_read_byte_form(
            target,
            physical,
            kind,
            alternative,
            operands,
            i64::from(displacement),
            &bytes,
        )
    }

    pub fn validate_aarch64_selected_hosted_read_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        request(target, physical, kind, alternative, operands, displacement)?;
        if decode_aarch64_selected_hosted_read_byte(target, bytes).map(i64::from)
            != Some(displacement)
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedAarch64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: Aarch64SelectedFormFootprint {
                register_reads: Vec::new(),
                register_writes: Vec::new(),
                writes_nzcv: true,
                encoded: effects(target),
            },
        })
    }

    pub fn decode_aarch64_selected_hosted_read_byte(
        target: NativeTarget,
        bytes: &[u8],
    ) -> Option<u32> {
        let words = super::word_stream(bytes)?;
        let reach = words
            .iter()
            .take_while(|word| *word & 0xff80_001f == 0x9100_0001)
            .count();
        let address = &words[..reach];
        let payload = u32::try_from(crate::immediate_offset::decode_address_words(
            address, 1, 31,
        )?)
        .ok()?;
        if !payload.is_multiple_of(4)
            || payload < 4
            || crate::hosted_sequences::read_byte_payload_address(payload).as_slice() != address
        {
            return None;
        }
        let reach = u32::try_from(reach).ok()?;
        let (calls, readdress): (&[u32], bool) = if target == NativeTarget::linux_arm64() {
            (
                &[
                    0xd280_0000,
                    0xd280_0022,
                    0xd280_07e8,
                    0xd400_0001,
                    0xb400_00e0,
                    0xf100_041f,
                    0x5400_0081,
                    0x5280_0029,
                ],
                false,
            )
        } else if target == NativeTarget::macos_arm64() {
            (
                &[
                    0xd280_0000,
                    0xd280_0022,
                    0xd280_0070,
                    0xd400_1001,
                    0x5400_0002 | ((7 + reach) << 5),
                    0xb400_0000 | ((7 + reach) << 5),
                    0xf100_041f,
                    0x5400_0001 | ((4 + reach) << 5),
                    0x5280_0029,
                ],
                true,
            )
        } else {
            return None;
        };
        let mut expected = address.to_vec();
        expected.extend([0xb81f_c03f, 0xb900_003f]);
        expected.extend_from_slice(calls);
        if readdress {
            expected.extend_from_slice(address);
        }
        expected.extend([0xb81f_c029, 0x1400_0002, 0xd420_0000]);
        (words == expected).then_some(payload - 4)
    }

    #[cfg(test)]
    mod tests {
        use super::{
            NativeTarget, decode_aarch64_selected_hosted_read_byte, effects,
            encode_aarch64_selected_hosted_read_byte_form,
            validate_aarch64_selected_hosted_read_byte_form,
        };
        use crate::test_support::reject_every_bit_flip;
        use register_model::RegisterViewId;
        use selected_instructions::{
            LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey,
            MachineEncodedStackEffect, MachineSemanticKind, MachineSizeKnowledge,
            SelectedInstructionKind,
        };
        use semantic_vocabulary::{OperationId, PlaceId};

        #[test]
        fn read_byte_replays_every_bit_and_rejects_wrong_home_source_slot_and_alternative() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let operation = OperationId::new(7).unwrap();
            let kind = SelectedInstructionKind::HostedReadByte {
                slot: LocalStorageSlotId::Structural {
                    operation,
                    place: PlaceId::new(1).unwrap(),
                },
            };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedReadByte,
                variant: 0,
            };
            let encode =
                |target, kind, alternative, source: &[RegisterViewId], displacement: u32| {
                    encode_aarch64_selected_hosted_read_byte_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        source,
                        i64::from(displacement),
                    )
                };
            let validate = |target, displacement: u32, bytes: &[u8]| {
                validate_aarch64_selected_hosted_read_byte_form(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &[],
                    i64::from(displacement),
                    bytes,
                )
            };
            for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
                for displacement in [
                    0,
                    32,
                    4088,
                    4092,
                    4096,
                    1_048_572,
                    900_000,
                    16_777_208,
                    16_777_212,
                    33_554_428,
                    u32::MAX - 7,
                ] {
                    let encoded = encode(target, kind, alternative, &[], displacement).unwrap();
                    assert_eq!(
                        decode_aarch64_selected_hosted_read_byte(target, encoded.bytes()),
                        Some(displacement)
                    );
                    assert_eq!(encoded.footprint().encoded, effects(target));
                    assert!(encoded.footprint().register_reads.is_empty());
                    assert!(encoded.footprint().register_writes.is_empty());
                    assert_eq!(
                        encoded.footprint().encoded.stack,
                        MachineEncodedStackEffect::UnchangedV1
                    );
                    reject_every_bit_flip(encoded.bytes(), "hosted read byte", |changed| {
                        assert_ne!(
                            decode_aarch64_selected_hosted_read_byte(target, changed),
                            Some(displacement),
                            "decoder bit"
                        );
                        validate(target, displacement, changed).is_err()
                    });
                    for invalid_offset in [1, 3, u32::MAX - 3, u32::MAX] {
                        assert!(encode(target, kind, alternative, &[], invalid_offset).is_err());
                    }
                    assert!(validate(target, displacement ^ 4, encoded.bytes()).is_err());
                    let boundary = SelectedInstructionKind::HostedReadByte {
                        slot: LocalStorageSlotId::Boundary { operation },
                    };
                    assert!(encode(target, boundary, alternative, &[], displacement).is_err());
                    let source = [physical.model().view_named("x0").unwrap().id];
                    assert!(encode(target, kind, alternative, &source, displacement).is_err());
                    let wrong = MachineAlternativeKey {
                        variant: 1,
                        ..alternative
                    };
                    assert!(encode(target, kind, wrong, &[], displacement).is_err());
                    assert!(validate(target, displacement, &encoded.bytes()[1..]).is_err());
                    let mut padded = encoded.bytes().to_vec();
                    padded.push(0);
                    assert_eq!(
                        decode_aarch64_selected_hosted_read_byte(target, &padded),
                        None
                    );
                    let other_target = if target == NativeTarget::linux_arm64() {
                        NativeTarget::macos_arm64()
                    } else {
                        NativeTarget::linux_arm64()
                    };
                    assert!(validate(other_target, displacement, encoded.bytes()).is_err());
                    assert_eq!(
                        decode_aarch64_selected_hosted_read_byte(other_target, encoded.bytes()),
                        None
                    );
                }
            }
        }

        #[test]
        fn read_byte_linux_golden_and_darwin_return_paths() {
            let linux = crate::encode_linux_read_byte_to_stack(16, 20).unwrap();
            let golden: [u32; 14] = [
                0x9100_53e1,
                0xb81f_c03f,
                0xb900_003f,
                0xd280_0000,
                0xd280_0022,
                0xd280_07e8,
                0xd400_0001,
                0xb400_00e0,
                0xf100_041f,
                0x5400_0081,
                0x5280_0029,
                0xb81f_c029,
                0x1400_0002,
                0xd420_0000,
            ];
            assert_eq!(
                linux,
                golden
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            );
            let darwin_words = |home: u32| {
                crate::encode_macos_read_byte_to_stack(home, home + 4)
                    .unwrap()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|word| u32::from_le_bytes(*word))
                    .collect::<Vec<_>>()
            };
            let words = darwin_words(16);
            assert_eq!(&words[..3], &golden[..3]);
            assert_eq!(
                &words[3..13],
                &[
                    0xd280_0000,
                    0xd280_0022,
                    0xd280_0070,
                    0xd400_1001,
                    0x5400_0102,
                    0xb400_0100,
                    0xf100_041f,
                    0x5400_00a1,
                    0x5280_0029,
                    0x9100_53e1,
                ]
            );
            assert_eq!(&words[13..], &golden[11..]);
            assert_eq!(&darwin_words(900_000)[..2], &[0x9143_6fe1, 0x912e_9021]);
            assert_eq!(&darwin_words(33_554_428)[..2], &[0x917f_ffe1, 0x917f_fc21]);
            for (home, payload) in [(0, 0), (0, 8), (2, 6), (u32::MAX, 3)] {
                assert!(crate::encode_macos_read_byte_to_stack(home, payload).is_err());
                assert!(crate::encode_linux_read_byte_to_stack(home, payload).is_err());
            }
            for home in [16, 900_000] {
                let words = darwin_words(home);
                let address = &words[..if home < 4092 { 1 } else { 2 }];
                for carry in [false, true] {
                    for count in [0, 1, 2, 4, u64::MAX] {
                        let mut position = address.len() + 6;
                        let mut zero = false;
                        let mut tag = 0;
                        let mut readdressed = false;
                        let mut trapped = false;
                        for _ in 0..10 {
                            if position == words.len() {
                                break;
                            }
                            let word = words[position];
                            if word & 0xff00_0010 == 0x5400_0000 {
                                let condition = word & 15;
                                let taken = match condition {
                                    1 => !zero,
                                    2 => carry,
                                    _ => panic!("unexpected condition"),
                                };
                                position += if taken {
                                    ((word >> 5) & 0x7ffff) as usize
                                } else {
                                    1
                                };
                            } else if word & 0xff00_001f == 0xb400_0000 {
                                position += if count == 0 {
                                    ((word >> 5) & 0x7ffff) as usize
                                } else {
                                    1
                                };
                            } else if words[position..].starts_with(address) {
                                readdressed = true;
                                position += address.len();
                            } else {
                                match word {
                                    0xf100_041f => {
                                        zero = count == 1;
                                        position += 1;
                                    }
                                    0x5280_0029 => {
                                        position += 1;
                                    }
                                    0xb81f_c029 => {
                                        assert!(readdressed, "tag stored before the address");
                                        tag = 1;
                                        position += 1;
                                    }
                                    0x1400_0002 => {
                                        position += (word & 0x03ff_ffff) as usize;
                                    }
                                    0xd420_0000 => {
                                        trapped = true;
                                        break;
                                    }
                                    _ => panic!("unexpected instruction"),
                                }
                            }
                        }
                        assert_eq!(trapped, carry || count > 1, "carry {carry}, count {count}");
                        if !trapped {
                            assert_eq!(position, words.len());
                            assert_eq!(tag, u32::from(count == 1));
                        }
                    }
                }
            }
        }

        #[test]
        fn read_catalog_retains_complete_zero_operand_constraint_and_effect_clobbers() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let constraints = crate::validate_aarch64_register_constraint_catalog(
                crate::aarch64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap();
            for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
                let catalog = crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
                let validated = crate::validate_aarch64_machine_effect_catalog(
                    target,
                    &constraints,
                    catalog.clone(),
                )
                .unwrap();
                let declaration = validated
                    .catalog()
                    .declaration_by_semantic(MachineSemanticKind::HostedReadByte)
                    .unwrap();
                let expected_key = if target == NativeTarget::linux_arm64() {
                    crate::AARCH64_HOSTED_READ_BYTE
                } else {
                    crate::AARCH64_DARWIN_HOSTED_READ_BYTE
                };
                assert_eq!(declaration.constraint, expected_key);
                assert_eq!(
                    declaration.alternatives[0].size,
                    MachineSizeKnowledge::EncoderResolved {
                        minimum_bytes: if target == NativeTarget::linux_arm64() {
                            56
                        } else {
                            64
                        },
                        maximum_bytes: Some(if target == NativeTarget::linux_arm64() {
                            52 + 1_032
                        } else {
                            56 + 2 * 1_032
                        }),
                    }
                );
                assert_eq!(declaration.alternatives[0].encoded, effects(target));
                let mut damaged = catalog;
                let row = damaged
                    .declaration_by_semantic_mut(MachineSemanticKind::HostedReadByte)
                    .unwrap();
                row.alternatives[0].encoded.implicit_unit_clobbers.pop();
                assert!(
                    crate::validate_aarch64_machine_effect_catalog(target, &constraints, damaged)
                        .is_err()
                );
            }
        }
    }
}
pub(crate) mod hosted_write_byte {
    use super::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
        ValidatedAarch64SelectedFormEncoding,
    };
    use crate::immediate_offset::{MAX_ADDRESS_BYTES, address_words, decode_address_words};
    use crate::selected_form_encoding::selected_forms::resolve_registers;
    use register_model::RegisterConstraintKey;
    use register_model::RegisterViewId;
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier,
        MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
        MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
        SelectedInstructionKind,
    };
    use target::NativeTarget;

    pub(crate) fn effects(target: NativeTarget) -> MachineEncodedEffects {
        let physical = crate::aarch64_physical_register_model();
        let view = |name: &str| {
            physical
                .view_named(name)
                .expect("canonical hosted write register")
        };
        let mut uses = view("sp").units.clone();
        uses.extend(view("pc").units.iter().copied());
        uses.sort_unstable();
        uses.dedup();
        let mut clobbers = Vec::new();
        let syscall_register = if target == NativeTarget::macos_arm64() {
            "x16"
        } else {
            "x8"
        };
        for name in ["x0", "x1", "x2", syscall_register, "nzcv"] {
            clobbers.extend(view(name).units.iter().copied());
        }
        clobbers.sort_unstable();
        clobbers.dedup();
        MachineEncodedEffects {
            external_operand_reads: vec![0],
            external_operand_writes: Vec::new(),
            implicit_unit_uses: uses,
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: clobbers,
            memory: MachineEncodedMemoryEffect::HostedWriteByteV1 {
                stack_pointer: view("sp").id,
            },
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::HostedWriteFailureV1,
            control: MachineEncodedControlEffect::HostedWriteReturnOrTrapV1,
        }
    }

    pub(crate) fn declaration(
        target: NativeTarget,
        constraint: RegisterConstraintKey,
    ) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            MachineSemanticKind::HostedWriteByteI32,
            constraint,
            MachineMemoryEffect::HostedWriteByteV1,
            MachineTrapBehavior::HostedWriteFailureV1,
            MachineBarrier::ExternalEffect,
            MachineAlternativeFamily::HostedWriteByteI32,
            {
                let short = if target == NativeTarget::macos_arm64() {
                    40
                } else {
                    36
                };
                MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes: short,
                    maximum_bytes: Some(short + MAX_ADDRESS_BYTES),
                }
            },
            effects(target),
        )
    }

    fn request(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<u8, Aarch64SelectedFormEncodingError> {
        if ![NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target)
            || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || !matches!(
                kind,
                SelectedInstructionKind::HostedWriteByteI32 {
                    slot: LocalStorageSlotId::Boundary { .. }
                }
            )
            || alternative
                != (MachineAlternativeKey {
                    family: MachineAlternativeFamily::HostedWriteByteI32,
                    variant: 0,
                })
            || u32::try_from(displacement).is_err()
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let registers = resolve_registers(physical, operands)?;
        let [register] = registers.as_slice() else {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        };
        Ok(*register)
    }

    pub fn encode_aarch64_selected_hosted_write_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let register = request(target, physical, kind, alternative, operands, displacement)?;
        let displacement = u32::try_from(displacement)
            .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        let mut words = store_and_buffer_words(register, displacement);
        words.extend_from_slice(
            system_call_words(target)
                .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?,
        );
        let bytes = words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        validate_aarch64_selected_hosted_write_byte_form(
            target,
            physical,
            kind,
            alternative,
            operands,
            i64::from(displacement),
            &bytes,
        )
    }

    pub fn validate_aarch64_selected_hosted_write_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let register = request(target, physical, kind, alternative, operands, displacement)?;
        if decode(target, bytes).map(|(register, offset)| (register, i64::from(offset)))
            != Some((register, displacement))
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedAarch64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: Aarch64SelectedFormFootprint {
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                writes_nzcv: true,
                encoded: effects(target),
            },
        })
    }

    fn store_and_buffer_words(register: u8, displacement: u32) -> Vec<u32> {
        let register = u32::from(register);
        if displacement <= 4095 {
            return vec![
                0x3900_03e0 | (displacement << 10) | register,
                0xd280_0020,
                0x9100_03e1 | (displacement << 10),
                0xd280_0022,
            ];
        }
        let mut words = vec![0x2a00_03e2 | (register << 16)];
        words.extend(address_words(1, 31, u64::from(displacement)));
        words.extend([0x3900_0022, 0xd280_0020, 0xd280_0022]);
        words
    }

    fn system_call_words(target: NativeTarget) -> Option<&'static [u32]> {
        if target == NativeTarget::linux_arm64() {
            Some(&[
                0xd280_0808,
                0xd400_0001,
                0xf100_001f,
                0x5400_004c,
                0xd420_0000,
            ])
        } else if target == NativeTarget::macos_arm64() {
            Some(&[
                0xd280_0090,
                0xd400_1001,
                0x5400_0062,
                0xf100_041f,
                0x5400_0040,
                0xd420_0000,
            ])
        } else {
            None
        }
    }

    pub fn aarch64_hosted_write_byte_i32_byte_count(
        target: NativeTarget,
        displacement: u32,
    ) -> Option<usize> {
        Some((store_and_buffer_words(0, displacement).len() + system_call_words(target)?.len()) * 4)
    }

    fn decode(target: NativeTarget, bytes: &[u8]) -> Option<(u8, u32)> {
        let system_call = system_call_words(target)?;
        let words = super::word_stream(bytes)?;
        let first = *words.first()?;
        let (register, displacement) = if first & 0xffc0_03e0 == 0x3900_03e0 {
            (first & 31, (first >> 10) & 4095)
        } else if first & 0xffe0_ffff == 0x2a00_03e2 {
            let reach = words[1..]
                .iter()
                .take_while(|word| *word & 0xff80_001f == 0x9100_0001)
                .count();
            let displacement = decode_address_words(&words[1..=reach], 1, 31)?;
            ((first >> 16) & 31, u32::try_from(displacement).ok()?)
        } else {
            return None;
        };
        if register == 31 {
            return None;
        }
        let register = register as u8;
        let mut expected = store_and_buffer_words(register, displacement);
        expected.extend_from_slice(system_call);
        (words == expected).then_some((register, displacement))
    }

    pub fn decode_aarch64_selected_hosted_write_byte_i32(
        target: NativeTarget,
        bytes: &[u8],
    ) -> Option<(calling_conventions::MachineRegister, u32)> {
        let (register, displacement) = decode(target, bytes)?;
        Some((
            calling_conventions::MachineRegister::Aarch64X(register),
            displacement,
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::{
            NativeTarget, decode, effects, encode_aarch64_selected_hosted_write_byte_form,
            validate_aarch64_selected_hosted_write_byte_form,
        };
        use crate::test_support::reject_every_bit_flip;
        use register_model::RegisterViewId;
        use selected_instructions::{
            LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey,
            MachineEncodedStackEffect, SelectedInstructionKind,
        };
        use semantic_vocabulary::{OperationId, PlaceId};

        #[test]
        fn hosted_write_byte_replays_source_frame_offset_syscall_failure_branch_and_every_bit() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let kind = SelectedInstructionKind::HostedWriteByteI32 {
                slot: LocalStorageSlotId::Boundary {
                    operation: OperationId::new(7).unwrap(),
                },
            };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedWriteByteI32,
                variant: 0,
            };
            let encode = |target, operands: &[RegisterViewId], displacement| {
                encode_aarch64_selected_hosted_write_byte_form(
                    target,
                    &physical,
                    kind,
                    alternative,
                    operands,
                    displacement,
                )
            };
            let validate =
                |target, kind, operands: &[RegisterViewId], displacement, bytes: &[u8]| {
                    validate_aarch64_selected_hosted_write_byte_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        operands,
                        displacement,
                        bytes,
                    )
                };
            for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
                for name in ["x0", "x1", "x2", "x8", "x9", "x16", "x28"] {
                    let operands = [physical.model().view_named(name).unwrap().id];
                    for displacement in [0, 31, 4095, 4096, 16_777_223, 33_554_448] {
                        let encoded = encode(target, &operands, displacement).unwrap();
                        assert_eq!(encoded.footprint().encoded, effects(target));
                        let other_target = if target == NativeTarget::linux_arm64() {
                            NativeTarget::macos_arm64()
                        } else {
                            NativeTarget::linux_arm64()
                        };
                        assert!(decode(other_target, encoded.bytes()).is_none());
                        assert!(decode(NativeTarget::linux_x64(), encoded.bytes()).is_none());
                        assert!(
                            encode(NativeTarget::linux_x64(), &operands, displacement).is_err()
                        );
                        assert!(encoded.footprint().register_writes.is_empty());
                        assert_eq!(
                            encoded.footprint().encoded.stack,
                            MachineEncodedStackEffect::UnchangedV1
                        );
                        reject_every_bit_flip(encoded.bytes(), &format!("{kind:?}"), |changed| {
                            validate(target, kind, &operands, displacement, changed).is_err()
                        });
                        let rejects = |kind, displacement, bytes: &[u8]| {
                            validate(target, kind, &operands, displacement, bytes).is_err()
                        };
                        assert!(rejects(kind, displacement ^ 1, encoded.bytes()));
                        assert!(rejects(kind, displacement, &encoded.bytes()[1..]));
                        let mut trailing = encoded.bytes().to_vec();
                        trailing.push(0);
                        assert!(rejects(kind, displacement, &trailing));
                        let wrong_slot = SelectedInstructionKind::HostedWriteByteI32 {
                            slot: LocalStorageSlotId::Structural {
                                operation: OperationId::new(7).unwrap(),
                                place: PlaceId::new(1).unwrap(),
                            },
                        };
                        assert!(rejects(wrong_slot, displacement, encoded.bytes()));
                        assert_eq!(
                            super::aarch64_hosted_write_byte_i32_byte_count(
                                target,
                                displacement as u32
                            ),
                            Some(encoded.bytes().len())
                        );
                        assert!(encode(target, &operands, i64::from(u32::MAX) + 1).is_err());
                        assert!(encode(target, &[], displacement).is_err());
                    }
                }
            }
        }

        #[test]
        fn darwin_success_requires_clear_carry_and_exactly_one_byte() {
            let words = [
                0x3900_03e9_u32,
                0xd280_0020,
                0x9100_03e1,
                0xd280_0022,
                0xd280_0090,
                0xd400_1001,
                0x5400_0062,
                0xf100_041f,
                0x5400_0040,
                0xd420_0000,
            ];
            let bytes = words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            assert_eq!(decode(NativeTarget::macos_arm64(), &bytes), Some((9, 0)));
            for (word, replacement) in [(6, 0xd503_201f_u32), (7, 0xf100_001f), (8, 0x5400_004c)] {
                let mut changed = bytes.clone();
                crate::codec_bytes::patch_encoded(
                    &mut changed,
                    word * 4,
                    replacement.to_le_bytes(),
                );
                assert!(decode(NativeTarget::macos_arm64(), &changed).is_none());
            }
        }

        #[test]
        fn target_effects_bind_the_exact_syscall_register() {
            let physical = crate::aarch64_physical_register_model();
            for (target, used, preserved) in [
                (NativeTarget::linux_arm64(), "x8", "x16"),
                (NativeTarget::macos_arm64(), "x16", "x8"),
            ] {
                let effects = effects(target);
                assert!(
                    physical
                        .view_named(used)
                        .unwrap()
                        .units
                        .iter()
                        .all(|unit| effects.implicit_unit_clobbers.contains(unit))
                );
                assert!(
                    physical
                        .view_named(preserved)
                        .unwrap()
                        .units
                        .iter()
                        .all(|unit| !effects.implicit_unit_clobbers.contains(unit))
                );
            }
        }
    }
}
pub(crate) mod ieee_float_arithmetic {
    use super::{Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint};
    use register_model::{RegisterConstraintKey, RegisterUnitId, RegisterViewId, sorted_units};
    use selected_instructions::{
        IeeeFloatArithmeticForm, MachineAlternativeFamily, MachineBarrier,
        MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
        MachineTrapBehavior, ieee_float_arithmetic_is_trapping,
    };
    use selected_instructions::{
        IeeeFloatArithmeticOperation, IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection,
        IeeeFloatToIntegerDomain,
    };

    mod conversions {
        use super::{
            IeeeFloatIntegerCarrier, LEFT_SCRATCH, MASK_SCRATCH, RIGHT_SCRATCH, Word, materialize,
        };

        fn bounds(target: IeeeFloatIntegerCarrier) -> (u64, u64) {
            let magnitude_bits = u32::from(target.bits()) - u32::from(target.is_signed());
            let upper = u64::from(1023 + magnitude_bits) << 52;
            let lower = if target.is_signed() {
                (1_u64 << 63)
                    | upper
                    | if magnitude_bits <= 52 {
                        1_u64 << (52 - magnitude_bits)
                    } else {
                        1
                    }
            } else {
                (-1.0_f64).to_bits()
            };
            (lower, upper)
        }

        pub(super) fn trapping_truncation(
            double: bool,
            target: IeeeFloatIntegerCarrier,
            source: u8,
            result: u8,
        ) -> Vec<Word> {
            let mut words = vec![Word::ToFloat {
                double,
                general: source,
                vector: LEFT_SCRATCH,
            }];
            if !double {
                words.push(Word::OneSource {
                    double: false,
                    opcode: 0b00_0101,
                    source: LEFT_SCRATCH,
                    result: LEFT_SCRATCH,
                });
            }
            let (lower, upper) = bounds(target);
            for (bound, left, right) in [
                (lower, LEFT_SCRATCH, RIGHT_SCRATCH),
                (upper, RIGHT_SCRATCH, LEFT_SCRATCH),
            ] {
                words.extend(materialize(bound, result));
                words.extend([
                    Word::ToFloat {
                        double: true,
                        general: result,
                        vector: RIGHT_SCRATCH,
                    },
                    Word::Compare {
                        equal: false,
                        double: true,
                        left,
                        right,
                        result: MASK_SCRATCH,
                    },
                    Word::ToGeneral {
                        double: true,
                        vector: MASK_SCRATCH,
                        general: result,
                    },
                    Word::BranchNonzeroOverCrash { general: result },
                    Word::Crash,
                ]);
            }
            words.push(Word::FloatToInteger {
                wide: target.bits() == 64,
                double: true,
                unsigned: !target.is_signed(),
                vector: LEFT_SCRATCH,
                general: result,
            });
            if target.is_signed() && target.bits() <= 32 {
                words.push(Word::SignExtend32 {
                    source: result,
                    result,
                });
            }
            words
        }

        #[cfg(test)]
        mod tests {
            use super::super::{declaration, decode_word, encode, footprint, validate};
            use super::*;
            use selected_instructions::{
                IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatFormat,
                IeeeFloatToIntegerDomain, MachineBarrier, MachineEncodedControlEffect,
                MachineEncodedTrapBehavior, MachineSizeKnowledge, MachineTrapBehavior,
            };

            fn form(
                target: IeeeFloatIntegerCarrier,
                format: IeeeFloatFormat,
            ) -> IeeeFloatArithmeticForm {
                IeeeFloatArithmeticForm {
                    operation: IeeeFloatArithmeticOperation::TruncateToInteger {
                        target,
                        domain: IeeeFloatToIntegerDomain::Trapping,
                    },
                    format,
                }
            }

            #[test]
            fn trapping_words_pin_the_bounds_branches_and_conversion() {
                let selected = form(IeeeFloatIntegerCarrier::I32, IeeeFloatFormat::Binary64);
                let expected = [
                    0x9e67_0070_u32,
                    0xd2f8_3c04,
                    0xf2a0_0404,
                    0x9e67_0091,
                    0x7ef1_e612,
                    0x9e66_0244,
                    0xb500_0044,
                    0xd420_0000,
                    0xd2e8_3c04,
                    0x9e67_0091,
                    0x7ef0_e632,
                    0x9e66_0244,
                    0xb500_0044,
                    0xd420_0000,
                    0x1e78_0204,
                    0x9340_7c84,
                ]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
                assert_eq!(encode(selected, &[3, 4]).unwrap(), expected);
                validate(selected, &[3, 4], &expected).unwrap();
                let mut changed = expected.clone();
                crate::codec_bytes::patch_encoded(
                    &mut changed,
                    6 * 4,
                    0xb500_0084_u32.to_le_bytes(),
                );
                assert!(validate(selected, &[3, 4], &changed).is_err());
                changed = expected;
                crate::codec_bytes::patch_encoded(
                    &mut changed,
                    7 * 4,
                    0xd503_201f_u32.to_le_bytes(),
                );
                assert!(validate(selected, &[3, 4], &changed).is_err());
            }

            #[test]
            fn unsigned_64_conversion_words_select_the_unsigned_wide_carrier() {
                for (format, to_float, from_float, move_in, move_out) in [
                    (
                        IeeeFloatFormat::Binary32,
                        0x9e23_0070_u32,
                        0x9e39_0204_u32,
                        0x1e27_0070_u32,
                        0x1e26_0204_u32,
                    ),
                    (
                        IeeeFloatFormat::Binary64,
                        0x9e63_0070,
                        0x9e79_0204,
                        0x9e67_0070,
                        0x9e66_0204,
                    ),
                ] {
                    let selected = IeeeFloatArithmeticForm {
                        operation: IeeeFloatArithmeticOperation::FromInteger(
                            IeeeFloatIntegerCarrier::U64,
                        ),
                        format,
                    };
                    assert_eq!(
                        encode(selected, &[3, 4]).unwrap(),
                        [to_float, move_out]
                            .into_iter()
                            .flat_map(u32::to_le_bytes)
                            .collect::<Vec<_>>()
                    );
                    for domain in [
                        IeeeFloatToIntegerDomain::Exact,
                        IeeeFloatToIntegerDomain::Saturating,
                    ] {
                        let selected = IeeeFloatArithmeticForm {
                            operation: IeeeFloatArithmeticOperation::TruncateToInteger {
                                target: IeeeFloatIntegerCarrier::U64,
                                domain,
                            },
                            format,
                        };
                        assert_eq!(
                            encode(selected, &[3, 4]).unwrap(),
                            [move_in, from_float]
                                .into_iter()
                                .flat_map(u32::to_le_bytes)
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }

            #[test]
            fn trapping_footprint_declares_crash_without_flags_or_extra_scratch() {
                let model = crate::aarch64_physical_register_model();
                let operands = [
                    model.view_named("x3").unwrap().id,
                    model.view_named("x4").unwrap().id,
                ];
                for target in IeeeFloatIntegerCarrier::ALL {
                    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                        let selected = form(target, format);
                        let declaration = declaration(selected, crate::AARCH64_IEEE_FLOAT_UNARY);
                        let footprint = footprint(selected, &operands);
                        assert_eq!(declaration.trap, MachineTrapBehavior::TrappingIntegerV1);
                        assert_eq!(declaration.barrier, MachineBarrier::ExternalEffect);
                        assert_eq!(
                            footprint.encoded.trap,
                            MachineEncodedTrapBehavior::TrappingIntegerV1
                        );
                        assert_eq!(
                            footprint.encoded.control,
                            MachineEncodedControlEffect::FallThroughOrTrapV1
                        );
                        assert_eq!(footprint.register_reads, operands[..1]);
                        assert_eq!(footprint.register_writes, operands[1..]);
                        assert!(!footprint.writes_nzcv);
                        assert_eq!(
                            footprint.encoded.implicit_unit_clobbers,
                            super::super::clobbers()
                        );
                        assert_eq!(declaration.alternatives[0].encoded, footprint.encoded);
                        assert_eq!(
                            declaration.alternatives[0].size,
                            MachineSizeKnowledge::ExactBytes(
                                encode(selected, &[3, 4]).unwrap().len() as u16
                            )
                        );
                    }
                }
            }

            fn replay(bytes: &[u8], source: u64, input: usize, output: usize) -> Option<u64> {
                let mut general = [0_u64; 31];
                let mut vector = [0_u64; 32];
                general[input] = source;
                let words = bytes.as_chunks::<4>().0;
                let mut position = 0;
                while position < words.len() {
                    match decode_word(u32::from_le_bytes(words[position])).unwrap() {
                        Word::ToFloat {
                            double,
                            general: source,
                            vector: destination,
                        } => {
                            vector[usize::from(destination)] = if double {
                                general[usize::from(source)]
                            } else {
                                u64::from(general[usize::from(source)] as u32)
                            };
                        }
                        Word::ToGeneral {
                            double: true,
                            vector: source,
                            general: destination,
                        } => general[usize::from(destination)] = vector[usize::from(source)],
                        Word::OneSource {
                            double: false,
                            opcode: 0b00_0101,
                            source,
                            result,
                        } => {
                            vector[usize::from(result)] =
                                f64::from(f32::from_bits(vector[usize::from(source)] as u32))
                                    .to_bits()
                        }
                        Word::MoveWide {
                            keep,
                            shift,
                            immediate,
                            general: destination,
                        } => {
                            let shift = u32::from(shift) * 16;
                            let preserved = if keep {
                                general[usize::from(destination)] & !(0xffff_u64 << shift)
                            } else {
                                0
                            };
                            general[usize::from(destination)] =
                                preserved | (u64::from(immediate) << shift);
                        }
                        Word::Compare {
                            equal: false,
                            double: true,
                            left,
                            right,
                            result,
                        } => {
                            vector[usize::from(result)] =
                                if f64::from_bits(vector[usize::from(left)])
                                    > f64::from_bits(vector[usize::from(right)])
                                {
                                    u64::MAX
                                } else {
                                    0
                                }
                        }
                        Word::BranchNonzeroOverCrash { general: source } => {
                            if general[usize::from(source)] != 0 {
                                position += 1;
                            }
                        }
                        Word::Crash => return None,
                        Word::FloatToInteger {
                            wide,
                            double: true,
                            unsigned,
                            vector: source,
                            general: destination,
                        } => {
                            let source = f64::from_bits(vector[usize::from(source)]);
                            general[usize::from(destination)] = match (wide, unsigned) {
                                (true, true) => source as u64,
                                (true, false) => source as i64 as u64,
                                (false, true) => u64::from(source as u32),
                                (false, false) => u64::from(source as i32 as u32),
                            };
                        }
                        Word::SignExtend32 { source, result } => {
                            general[usize::from(result)] =
                                general[usize::from(source)] as i32 as i64 as u64
                        }
                        other => panic!("unexpected trapping conversion instruction {other:?}"),
                    }
                    position += 1;
                }
                Some(general[output])
            }

            #[test]
            fn trapping_replay_matches_integer_bounds_at_fractional_edges_and_nonfinite_values() {
                for target in IeeeFloatIntegerCarrier::ALL {
                    let magnitude_bits = target.bits() - u16::from(target.is_signed());
                    let maximum = (1_i128 << magnitude_bits) - 1;
                    let minimum = if target.is_signed() {
                        -(1_i128 << magnitude_bits)
                    } else {
                        0
                    };
                    let mut sources = vec![
                        f64::NAN,
                        f64::NEG_INFINITY,
                        f64::INFINITY,
                        -0.0,
                        0.0,
                        -0.75,
                        0.75,
                        -1.0,
                        -1.5,
                        1.5,
                    ];
                    for edge in [
                        minimum as f64,
                        maximum as f64,
                        (minimum - 1) as f64,
                        (maximum + 1) as f64,
                    ] {
                        sources.extend([
                            edge,
                            edge.next_down(),
                            edge.next_up(),
                            f64::from((edge as f32).next_down()),
                            f64::from((edge as f32).next_up()),
                            edge - 0.5,
                            edge + 0.5,
                        ]);
                    }
                    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                        for &source in &sources {
                            let (source, payload) = match format {
                                IeeeFloatFormat::Binary32 => (
                                    f64::from(source as f32),
                                    u64::from((source as f32).to_bits()),
                                ),
                                IeeeFloatFormat::Binary64 => (source, source.to_bits()),
                            };
                            let integer = source.trunc() as i128;
                            let expected = (source.is_finite()
                                && (minimum..=maximum).contains(&integer))
                            .then_some(integer as u64);
                            for registers in [[3, 4], [3, 3]] {
                                let bytes = encode(form(target, format), &registers).unwrap();
                                assert_eq!(
                                    replay(
                                        &bytes,
                                        payload,
                                        usize::from(registers[0]),
                                        usize::from(registers[1])
                                    ),
                                    expected,
                                    "{target:?} {format:?} {source} {registers:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    const LEFT_SCRATCH: u8 = 16;
    const RIGHT_SCRATCH: u8 = 17;
    const MASK_SCRATCH: u8 = 18;

    fn view_units(name: &str) -> Vec<RegisterUnitId> {
        crate::aarch64_physical_register_model()
            .view_named(name)
            .unwrap_or_else(|| panic!("canonical AArch64 model declares {name}"))
            .units
            .clone()
    }

    pub(crate) fn implicit_uses() -> Vec<RegisterUnitId> {
        sorted_units(view_units("fpcr"))
    }

    pub(crate) fn clobbers() -> Vec<RegisterUnitId> {
        sorted_units(["q16", "q17", "q18"].into_iter().flat_map(view_units))
    }

    pub(crate) fn effects(form: IeeeFloatArithmeticForm) -> MachineEncodedEffects {
        let inputs = form.operand_count() as u16;
        let mut effects = MachineEncodedEffects {
            implicit_unit_uses: implicit_uses(),
            implicit_unit_clobbers: clobbers(),
            ..MachineEncodedEffects::fallthrough_v1((0..inputs).collect(), vec![inputs])
        };
        if ieee_float_arithmetic_is_trapping(form) {
            effects.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
            effects.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
        }
        effects
    }

    pub(crate) fn declaration(
        form: IeeeFloatArithmeticForm,
        constraint: RegisterConstraintKey,
    ) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            MachineSemanticKind::NearestIeeeFloatArithmetic(form),
            constraint,
            MachineMemoryEffect::NoneV1,
            if ieee_float_arithmetic_is_trapping(form) {
                MachineTrapBehavior::TrappingIntegerV1
            } else {
                MachineTrapBehavior::NeverV1
            },
            if ieee_float_arithmetic_is_trapping(form) {
                MachineBarrier::ExternalEffect
            } else {
                MachineBarrier::None
            },
            MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
            MachineSizeKnowledge::ExactBytes(4 * program(form, &[0, 0, 0, 0]).len() as u16),
            effects(form),
        )
    }

    pub(crate) fn footprint(
        form: IeeeFloatArithmeticForm,
        operands: &[RegisterViewId],
    ) -> Aarch64SelectedFormFootprint {
        let inputs = form.operand_count();
        Aarch64SelectedFormFootprint {
            register_reads: operands[..inputs].to_vec(),
            register_writes: operands[inputs..inputs + 1].to_vec(),
            writes_nzcv: false,
            encoded: effects(form),
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Word {
        ExtractUnsigned {
            first: u8,
            width: u8,
            source: u8,
            result: u8,
        },
        ShiftLeft32 {
            source: u8,
            result: u8,
        },
        SubtractImmediate {
            immediate: u16,
            source: u8,
            result: u8,
        },
        BranchZero {
            general: u8,
            displacement: i32,
        },
        Jump {
            displacement: i32,
        },
        BitwiseOr {
            left: u8,
            right: u8,
            result: u8,
        },
        FloatingControl {
            read: bool,
            general: u8,
        },
        BranchNonzeroOverCrash {
            general: u8,
        },
        Crash,
        ToFloat {
            double: bool,
            general: u8,
            vector: u8,
        },
        ToGeneral {
            double: bool,
            vector: u8,
            general: u8,
        },
        TwoSource {
            double: bool,
            opcode: u32,
            left: u8,
            right: u8,
            result: u8,
        },
        FusedMultiplyAdd {
            double: bool,
            left: u8,
            right: u8,
            addend: u8,
            result: u8,
        },
        OneSource {
            double: bool,
            opcode: u32,
            source: u8,
            result: u8,
        },
        IntegerToFloat {
            wide: bool,
            double: bool,
            unsigned: bool,
            general: u8,
            vector: u8,
        },
        FloatToInteger {
            wide: bool,
            double: bool,
            unsigned: bool,
            vector: u8,
            general: u8,
        },
        SignExtend32 {
            source: u8,
            result: u8,
        },
        ExtendNarrow {
            signed: bool,
            halfword: bool,
            source: u8,
            result: u8,
        },
        MoveWide {
            keep: bool,
            shift: u8,
            immediate: u16,
            general: u8,
        },
        Compare {
            equal: bool,
            double: bool,
            left: u8,
            right: u8,
            result: u8,
        },
        BitwiseSelect {
            mask: u8,
            if_set: u8,
            if_clear: u8,
        },
        AddImmediate {
            wide: bool,
            immediate: u16,
            source: u8,
            result: u8,
        },
        LogicalShiftRight {
            wide: bool,
            count: u8,
            source: u8,
            result: u8,
        },
    }

    fn encode_word(word: Word) -> u32 {
        let reg = |register: u8| u32::from(register);
        let width = |double: bool| u32::from(double);
        match word {
            Word::ExtractUnsigned {
                first,
                width,
                source,
                result,
            } => {
                0xd340_0000
                    | u32::from(first) << 16
                    | u32::from(first + width - 1) << 10
                    | reg(source) << 5
                    | reg(result)
            }
            Word::ShiftLeft32 { source, result } => 0xd360_7c00 | reg(source) << 5 | reg(result),
            Word::SubtractImmediate {
                immediate,
                source,
                result,
            } => 0xd100_0000 | u32::from(immediate) << 10 | reg(source) << 5 | reg(result),
            Word::BranchZero {
                general,
                displacement,
            } => 0xb400_0000 | (((displacement / 4) as u32 & 0x7ffff) << 5) | reg(general),
            Word::Jump { displacement } => 0x1400_0000 | ((displacement / 4) as u32 & 0x03ff_ffff),
            Word::BitwiseOr {
                left,
                right,
                result,
            } => 0x0ea0_1c00 | reg(right) << 16 | reg(left) << 5 | reg(result),
            Word::FloatingControl { read, general } => {
                (if read { 0xd53b_4400 } else { 0xd51b_4400 }) | reg(general)
            }
            Word::BranchNonzeroOverCrash { general } => 0xb500_0040 | reg(general),
            Word::Crash => 0xd420_0000,
            Word::ToFloat {
                double,
                general,
                vector,
            } => (if double { 0x9e67_0000 } else { 0x1e27_0000 }) | reg(general) << 5 | reg(vector),
            Word::ToGeneral {
                double,
                vector,
                general,
            } => (if double { 0x9e66_0000 } else { 0x1e26_0000 }) | reg(vector) << 5 | reg(general),
            Word::TwoSource {
                double,
                opcode,
                left,
                right,
                result,
            } => {
                0x1e20_0800
                    | width(double) << 22
                    | reg(right) << 16
                    | opcode << 12
                    | reg(left) << 5
                    | reg(result)
            }
            Word::FusedMultiplyAdd {
                double,
                left,
                right,
                addend,
                result,
            } => {
                0x1f00_0000
                    | width(double) << 22
                    | reg(right) << 16
                    | reg(addend) << 10
                    | reg(left) << 5
                    | reg(result)
            }
            Word::OneSource {
                double,
                opcode,
                source,
                result,
            } => 0x1e20_4000 | width(double) << 22 | opcode << 15 | reg(source) << 5 | reg(result),
            Word::IntegerToFloat {
                wide,
                double,
                unsigned,
                general,
                vector,
            } => {
                0x1e22_0000
                    | width(wide) << 31
                    | width(double) << 22
                    | u32::from(unsigned) << 16
                    | reg(general) << 5
                    | reg(vector)
            }
            Word::FloatToInteger {
                wide,
                double,
                unsigned,
                vector,
                general,
            } => {
                0x1e38_0000
                    | width(wide) << 31
                    | width(double) << 22
                    | u32::from(unsigned) << 16
                    | reg(vector) << 5
                    | reg(general)
            }
            Word::SignExtend32 { source, result } => 0x9340_7c00 | reg(source) << 5 | reg(result),
            Word::ExtendNarrow {
                signed,
                halfword,
                source,
                result,
            } => {
                (if signed { 0x1300_0000 } else { 0x5300_0000 })
                    | (if halfword { 15 } else { 7 }) << 10
                    | reg(source) << 5
                    | reg(result)
            }
            Word::MoveWide {
                keep,
                shift,
                immediate,
                general,
            } => {
                (if keep { 0xf280_0000 } else { 0xd280_0000 })
                    | u32::from(shift) << 21
                    | u32::from(immediate) << 5
                    | reg(general)
            }
            Word::Compare {
                equal,
                double,
                left,
                right,
                result,
            } => {
                (if equal { 0x5e20_e400 } else { 0x7ea0_e400 })
                    | width(double) << 22
                    | reg(right) << 16
                    | reg(left) << 5
                    | reg(result)
            }
            Word::AddImmediate {
                wide,
                immediate,
                source,
                result,
            } => {
                0x1100_0000
                    | width(wide) << 31
                    | u32::from(immediate) << 10
                    | reg(source) << 5
                    | reg(result)
            }
            Word::LogicalShiftRight {
                wide,
                count,
                source,
                result,
            } => {
                (if wide { 0xd340_fc00 } else { 0x5300_7c00 })
                    | u32::from(count) << 16
                    | reg(source) << 5
                    | reg(result)
            }
            Word::BitwiseSelect {
                mask,
                if_set,
                if_clear,
            } => 0x2e60_1c00 | reg(if_clear) << 16 | reg(if_set) << 5 | reg(mask),
        }
    }

    fn decode_word(word: u32) -> Option<Word> {
        if word & !31 == 0xd53b_4400 || word & !31 == 0xd51b_4400 {
            return Some(Word::FloatingControl {
                read: word & !31 == 0xd53b_4400,
                general: (word & 31) as u8,
            });
        }
        let field = |shift: u32| ((word >> shift) & 31) as u8;
        let bit = |shift: u32| (word >> shift) & 1 == 1;
        let float_type = |shift: u32| match (word >> shift) & 3 {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        };
        Some(match word & 0xffff_fc00 {
            _ if word & 0xffff_fc00 == 0xd360_7c00 => Word::ShiftLeft32 {
                source: field(5),
                result: field(0),
            },
            _ if word & 0xffc0_0000 == 0xd340_0000
                && (word >> 10 & 63) >= (word >> 16 & 63)
                && (word >> 10 & 63) != 63 =>
            {
                Word::ExtractUnsigned {
                    first: (word >> 16 & 63) as u8,
                    width: ((word >> 10 & 63) - (word >> 16 & 63) + 1) as u8,
                    source: field(5),
                    result: field(0),
                }
            }
            _ if word & 0xffc0_0000 == 0xd100_0000 => Word::SubtractImmediate {
                immediate: (word >> 10 & 0xfff) as u16,
                source: field(5),
                result: field(0),
            },
            _ if word & 0xff00_0000 == 0xb400_0000 => Word::BranchZero {
                general: field(0),
                displacement: (((word << 8) as i32) >> 13) * 4,
            },
            _ if word & 0xfc00_0000 == 0x1400_0000 => Word::Jump {
                displacement: (((word << 6) as i32) >> 6) * 4,
            },
            _ if word & 0xffe0_fc00 == 0x0ea0_1c00 => Word::BitwiseOr {
                left: field(5),
                right: field(16),
                result: field(0),
            },

            _ if word == 0xd420_0000 => Word::Crash,
            _ if word & 0xffff_ffe0 == 0xb500_0040 => {
                Word::BranchNonzeroOverCrash { general: field(0) }
            }
            0x1e27_0000 | 0x9e67_0000 if bit(31) == bit(22) => Word::ToFloat {
                double: bit(31),
                general: field(5),
                vector: field(0),
            },
            0x1e26_0000 | 0x9e66_0000 if bit(31) == bit(22) => Word::ToGeneral {
                double: bit(31),
                vector: field(5),
                general: field(0),
            },
            _ if word & 0xff20_0c00 == 0x1e20_0800 => Word::TwoSource {
                double: float_type(22)?,
                opcode: (word >> 12) & 15,
                left: field(5),
                right: field(16),
                result: field(0),
            },
            _ if word & 0xff20_8000 == 0x1f00_0000 => Word::FusedMultiplyAdd {
                double: float_type(22)?,
                left: field(5),
                right: field(16),
                addend: field(10),
                result: field(0),
            },
            _ if word & 0xff20_7c00 == 0x1e20_4000 => Word::OneSource {
                double: float_type(22)?,
                opcode: (word >> 15) & 63,
                source: field(5),
                result: field(0),
            },
            _ if word & 0x7f3e_fc00 == 0x1e22_0000 => Word::IntegerToFloat {
                wide: bit(31),
                double: float_type(22)?,
                unsigned: bit(16),
                general: field(5),
                vector: field(0),
            },
            _ if word & 0x7f3e_fc00 == 0x1e38_0000 => Word::FloatToInteger {
                wide: bit(31),
                double: float_type(22)?,
                unsigned: bit(16),
                vector: field(5),
                general: field(0),
            },
            0x9340_7c00 => Word::SignExtend32 {
                source: field(5),
                result: field(0),
            },
            base @ (0x1300_1c00 | 0x1300_3c00 | 0x5300_1c00 | 0x5300_3c00) => Word::ExtendNarrow {
                signed: base & 0x4000_0000 == 0,
                halfword: base & 0x2000 != 0,
                source: field(5),
                result: field(0),
            },
            _ if word & 0xdf80_0000 == 0xd280_0000 => Word::MoveWide {
                keep: bit(29),
                shift: ((word >> 21) & 3) as u8,
                immediate: ((word >> 5) & 0xffff) as u16,
                general: field(0),
            },
            _ if matches!(word & 0xffa0_fc00, 0x7ea0_e400 | 0x5e20_e400) => Word::Compare {
                equal: word & 0xffa0_fc00 == 0x5e20_e400,
                double: bit(22),
                left: field(5),
                right: field(16),
                result: field(0),
            },
            _ if word & 0x7fc0_0000 == 0x1100_0000 => Word::AddImmediate {
                wide: bit(31),
                immediate: ((word >> 10) & 0xfff) as u16,
                source: field(5),
                result: field(0),
            },
            _ if matches!(word & 0xffc0_fc00, 0x5300_7c00 | 0xd340_fc00) => {
                let count = ((word >> 16) & 63) as u8;
                if !bit(31) && count >= 32 {
                    return None;
                }
                Word::LogicalShiftRight {
                    wide: bit(31),
                    count,
                    source: field(5),
                    result: field(0),
                }
            }
            _ if word & 0xffe0_fc00 == 0x2e60_1c00 => Word::BitwiseSelect {
                mask: field(0),
                if_set: field(5),
                if_clear: field(16),
            },
            _ => return None,
        })
    }

    fn classification_program(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Vec<Word> {
        let double = !form.is_binary32();
        let result = registers[1];
        let restore = Word::ToGeneral {
            double: true,
            vector: LEFT_SCRATCH,
            general: result,
        };
        let output = |class| Word::MoveWide {
            keep: false,
            shift: 0,
            immediate: match form.operation {
                IeeeFloatArithmeticOperation::Classify => class,
                IeeeFloatArithmeticOperation::IsNan => u16::from(class == 0),
                IeeeFloatArithmeticOperation::IsInfinite => u16::from(class == 1),
                IeeeFloatArithmeticOperation::IsFinite => u16::from(class >= 2),
                IeeeFloatArithmeticOperation::IsNormal => u16::from(class == 2),
                IeeeFloatArithmeticOperation::IsSubnormal => u16::from(class == 3),
                _ => unreachable!("classification operation"),
            },
            general: result,
        };
        let branch = Word::BranchZero {
            general: result,
            displacement: 0,
        };
        let jump = Word::Jump { displacement: 0 };
        let mut instructions = vec![
            Word::ToFloat {
                double,
                general: registers[0],
                vector: LEFT_SCRATCH,
            },
            Word::ExtractUnsigned {
                first: if double { 52 } else { 23 },
                width: if double { 11 } else { 8 },
                source: registers[0],
                result,
            },
            branch,
            Word::SubtractImmediate {
                immediate: if double { 2047 } else { 255 },
                source: result,
                result,
            },
            branch,
            output(2),
            jump,
        ];
        let subnormal = instructions.len();
        instructions.extend([
            restore,
            Word::ExtractUnsigned {
                first: 0,
                width: if double { 63 } else { 31 },
                source: result,
                result,
            },
            branch,
            output(3),
            jump,
        ]);
        let zero = instructions.len();
        instructions.extend([output(4), jump]);
        let nonfinite = instructions.len();
        instructions.extend([
            restore,
            Word::ExtractUnsigned {
                first: 0,
                width: if double { 52 } else { 23 },
                source: result,
                result,
            },
            branch,
            output(0),
            jump,
        ]);
        let infinity = instructions.len();
        instructions.push(output(1));
        let end = instructions.len();
        if form.operation == IeeeFloatArithmeticOperation::Classify {
            instructions.extend([
                branch,
                Word::ToFloat {
                    double: true,
                    general: result,
                    vector: RIGHT_SCRATCH,
                },
                restore,
                Word::LogicalShiftRight {
                    wide: true,
                    count: if double { 63 } else { 31 },
                    source: result,
                    result,
                },
                Word::ShiftLeft32 {
                    source: result,
                    result,
                },
                Word::ToFloat {
                    double: true,
                    general: result,
                    vector: LEFT_SCRATCH,
                },
                Word::BitwiseOr {
                    left: LEFT_SCRATCH,
                    right: RIGHT_SCRATCH,
                    result: LEFT_SCRATCH,
                },
                restore,
            ]);
        }
        let mut branches = vec![
            (2, subnormal),
            (4, nonfinite),
            (6, end),
            (subnormal + 2, zero),
            (subnormal + 4, end),
            (zero + 1, end),
            (nonfinite + 2, infinity),
            (nonfinite + 4, end),
        ];
        if form.operation == IeeeFloatArithmeticOperation::Classify {
            branches.push((end, instructions.len()));
        }
        for (index, destination) in branches {
            let displacement = (destination as i32 - index as i32) * 4;
            match &mut instructions[index] {
                Word::BranchZero {
                    displacement: field,
                    ..
                }
                | Word::Jump {
                    displacement: field,
                } => *field = displacement,
                _ => unreachable!("classification branch"),
            }
        }
        instructions
    }

    const fn two_source_opcode(operation: IeeeFloatArithmeticOperation) -> Option<u32> {
        match operation {
            IeeeFloatArithmeticOperation::Multiply => Some(0b0000),
            IeeeFloatArithmeticOperation::Divide => Some(0b0001),
            IeeeFloatArithmeticOperation::Add => Some(0b0010),
            IeeeFloatArithmeticOperation::Subtract => Some(0b0011),
            _ => None,
        }
    }

    const fn directed_mode(rounding: IeeeFloatRoundingDirection) -> u16 {
        match rounding.ordinal() {
            0 => 3,
            1 => 1,
            2 => 2,
            _ => unreachable!(),
        }
    }

    fn program(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Vec<Word> {
        let double = !form.is_binary32();
        let result = registers[form.operand_count()];
        let move_in = |general: u8, vector: u8, double: bool| Word::ToFloat {
            double,
            general,
            vector,
        };
        let move_out = |vector: u8| Word::ToGeneral {
            double,
            vector,
            general: result,
        };
        let (first, second, mask) = (LEFT_SCRATCH, RIGHT_SCRATCH, MASK_SCRATCH);
        let directed = |rounding, mut words: Vec<Word>, operation: Word| {
            words.extend([
                Word::FloatingControl {
                    read: true,
                    general: result,
                },
                move_in(result, mask, true),
                Word::MoveWide {
                    keep: false,
                    shift: 1,
                    immediate: directed_mode(rounding) << 6,
                    general: result,
                },
                Word::FloatingControl {
                    read: false,
                    general: result,
                },
                operation,
                Word::ToGeneral {
                    double: true,
                    vector: mask,
                    general: result,
                },
                Word::FloatingControl {
                    read: false,
                    general: result,
                },
                move_out(first),
            ]);
            words
        };
        match form.operation {
            IeeeFloatArithmeticOperation::DirectedBinary {
                operation,
                rounding,
            } => {
                let opcode = match operation.ordinal() {
                    0 => 2,
                    1 => 3,
                    2 => 0,
                    3 => 1,
                    _ => unreachable!(),
                };
                directed(
                    rounding,
                    vec![
                        move_in(registers[0], first, double),
                        move_in(registers[1], second, double),
                    ],
                    Word::TwoSource {
                        double,
                        opcode,
                        left: first,
                        right: second,
                        result: first,
                    },
                )
            }
            IeeeFloatArithmeticOperation::DirectedSquareRoot { rounding } => directed(
                rounding,
                vec![move_in(registers[0], first, double)],
                Word::OneSource {
                    double,
                    opcode: 0b00_0011,
                    source: first,
                    result: first,
                },
            ),
            IeeeFloatArithmeticOperation::IsNan
            | IeeeFloatArithmeticOperation::IsInfinite
            | IeeeFloatArithmeticOperation::IsFinite
            | IeeeFloatArithmeticOperation::IsNormal
            | IeeeFloatArithmeticOperation::IsSubnormal
            | IeeeFloatArithmeticOperation::Classify => classification_program(form, registers),
            IeeeFloatArithmeticOperation::Compare(comparison) => {
                use semantic_vocabulary::IeeeFloatComparisonOperation as Relation;
                let compare = |equal, left, right, result| Word::Compare {
                    equal,
                    double,
                    left,
                    right,
                    result,
                };
                let mut words = vec![
                    move_in(registers[0], first, double),
                    move_in(registers[1], second, double),
                ];
                match comparison {
                    Relation::Equal | Relation::NotEqual => {
                        words.push(compare(true, first, second, mask));
                    }
                    Relation::Less | Relation::Greater => {
                        let (left, right) = if comparison == Relation::Less {
                            (second, first)
                        } else {
                            (first, second)
                        };
                        words.push(compare(false, left, right, mask));
                    }
                    Relation::LessOrEqual | Relation::GreaterOrEqual => {
                        let (left, right) = if comparison == Relation::LessOrEqual {
                            (second, first)
                        } else {
                            (first, second)
                        };
                        words.extend([
                            compare(true, first, second, mask),
                            compare(false, left, right, first),
                            Word::BitwiseOr {
                                left: mask,
                                right: first,
                                result: mask,
                            },
                        ]);
                    }
                }
                words.extend([
                    move_out(mask),
                    Word::ExtractUnsigned {
                        first: 0,
                        width: 1,
                        source: result,
                        result,
                    },
                ]);
                if comparison == Relation::NotEqual {
                    words.extend([
                        Word::SubtractImmediate {
                            immediate: 1,
                            source: result,
                            result,
                        },
                        Word::ExtractUnsigned {
                            first: 0,
                            width: 1,
                            source: result,
                            result,
                        },
                    ]);
                }
                words
            }
            IeeeFloatArithmeticOperation::FusedMultiplyAdd => vec![
                move_in(registers[0], first, double),
                move_in(registers[1], second, double),
                move_in(registers[2], mask, double),
                Word::FusedMultiplyAdd {
                    double,
                    left: first,
                    right: second,
                    addend: mask,
                    result: first,
                },
                move_out(first),
            ],
            IeeeFloatArithmeticOperation::MultiplyThenAdd => vec![
                move_in(registers[0], first, double),
                move_in(registers[1], second, double),
                Word::TwoSource {
                    double,
                    opcode: 0b0000,
                    left: first,
                    right: second,
                    result: first,
                },
                move_in(registers[2], second, double),
                Word::TwoSource {
                    double,
                    opcode: 0b0010,
                    left: first,
                    right: second,
                    result: first,
                },
                move_out(first),
            ],
            IeeeFloatArithmeticOperation::Minimum | IeeeFloatArithmeticOperation::Maximum => {
                let (left, right) = if form.operation == IeeeFloatArithmeticOperation::Minimum {
                    (second, first)
                } else {
                    (first, second)
                };
                vec![
                    move_in(registers[0], first, double),
                    move_in(registers[1], second, double),
                    Word::Compare {
                        equal: false,
                        double,
                        left,
                        right,
                        result: mask,
                    },
                    Word::BitwiseSelect {
                        mask,
                        if_set: first,
                        if_clear: second,
                    },
                    move_out(mask),
                ]
            }
            IeeeFloatArithmeticOperation::Negate | IeeeFloatArithmeticOperation::SquareRoot => {
                let opcode = if form.operation == IeeeFloatArithmeticOperation::Negate {
                    0b00_0010
                } else {
                    0b00_0011
                };
                vec![
                    move_in(registers[0], first, double),
                    Word::OneSource {
                        double,
                        opcode,
                        source: first,
                        result: first,
                    },
                    move_out(first),
                ]
            }
            IeeeFloatArithmeticOperation::ConvertFormat => vec![
                move_in(registers[0], first, !double),
                Word::OneSource {
                    double: !double,
                    opcode: if double { 0b00_0101 } else { 0b00_0100 },
                    source: first,
                    result: first,
                },
                move_out(first),
            ],
            IeeeFloatArithmeticOperation::TruncateToInteger {
                target,
                domain: IeeeFloatToIntegerDomain::Trapping,
            } => conversions::trapping_truncation(double, target, registers[0], result),
            IeeeFloatArithmeticOperation::TruncateToInteger { target, .. }
                if target.bits() < 32 =>
            {
                narrow_truncation(double, target, registers[0], result)
            }
            IeeeFloatArithmeticOperation::FromInteger(source) if source.bits() < 32 => vec![
                Word::ExtendNarrow {
                    signed: source.is_signed(),
                    halfword: source.bits() == 16,
                    source: registers[0],
                    result,
                },
                Word::IntegerToFloat {
                    wide: false,
                    double,
                    unsigned: !source.is_signed(),
                    general: result,
                    vector: first,
                },
                move_out(first),
            ],
            IeeeFloatArithmeticOperation::TruncateToInteger { target, .. } => {
                let mut words = vec![
                    move_in(registers[0], first, double),
                    Word::FloatToInteger {
                        wide: target.bits() == 64,
                        double,
                        unsigned: !target.is_signed(),
                        vector: first,
                        general: result,
                    },
                ];
                if target.is_signed() && target.bits() == 32 {
                    words.push(Word::SignExtend32 {
                        source: result,
                        result,
                    });
                }
                words
            }
            IeeeFloatArithmeticOperation::FromInteger(source) => vec![
                Word::IntegerToFloat {
                    wide: source.bits() == 64,
                    double,
                    unsigned: !source.is_signed(),
                    general: registers[0],
                    vector: first,
                },
                move_out(first),
            ],
            arithmetic => {
                let opcode = two_source_opcode(arithmetic)
                    .expect("every remaining operation is two-source arithmetic");
                vec![
                    move_in(registers[0], first, double),
                    move_in(registers[1], second, double),
                    Word::TwoSource {
                        double,
                        opcode,
                        left: first,
                        right: second,
                        result: first,
                    },
                    move_out(first),
                ]
            }
        }
    }

    const fn narrow_bounds(target: IeeeFloatIntegerCarrier) -> (u64, u64) {
        match target {
            IeeeFloatIntegerCarrier::I8 => (0xc060_0000_0000_0000, 0x405f_c000_0000_0000),
            IeeeFloatIntegerCarrier::I16 => (0xc0e0_0000_0000_0000, 0x40df_ffc0_0000_0000),
            IeeeFloatIntegerCarrier::U8 => (0, 0x406f_e000_0000_0000),
            IeeeFloatIntegerCarrier::U16 => (0, 0x40ef_ffe0_0000_0000),
            _ => (0, 0),
        }
    }

    fn materialize(bits: u64, general: u8) -> Vec<Word> {
        let mut words = vec![Word::MoveWide {
            keep: false,
            shift: 3,
            immediate: (bits >> 48) as u16,
            general,
        }];
        for shift in (0..3).rev() {
            let immediate = (bits >> (16 * shift)) as u16;
            if immediate != 0 {
                words.push(Word::MoveWide {
                    keep: true,
                    shift,
                    immediate,
                    general,
                });
            }
        }
        words
    }

    fn narrow_truncation(
        double: bool,
        target: IeeeFloatIntegerCarrier,
        source: u8,
        result: u8,
    ) -> Vec<Word> {
        let (first, second) = (LEFT_SCRATCH, RIGHT_SCRATCH);
        let mut words = vec![Word::ToFloat {
            double,
            general: source,
            vector: first,
        }];
        if !double {
            words.push(Word::OneSource {
                double: false,
                opcode: 0b00_0101,
                source: first,
                result: first,
            });
        }
        let (minimum, maximum) = narrow_bounds(target);
        for (bound, opcode) in [(minimum, 0b0100), (maximum, 0b0101)] {
            words.extend(materialize(bound, result));
            words.push(Word::ToFloat {
                double: true,
                general: result,
                vector: second,
            });
            words.push(Word::TwoSource {
                double: true,
                opcode,
                left: first,
                right: second,
                result: first,
            });
        }
        words.push(Word::FloatToInteger {
            wide: false,
            double: true,
            unsigned: !target.is_signed(),
            vector: first,
            general: result,
        });
        if target.is_signed() {
            words.push(Word::SignExtend32 {
                source: result,
                result,
            });
        }
        words
    }

    fn validate_registers(
        form: IeeeFloatArithmeticForm,
        registers: &[u8],
    ) -> Result<(), Aarch64SelectedFormEncodingError> {
        if registers.len() != form.operand_count() + 1 {
            return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
        }
        if registers.iter().any(|register| *register > 30) {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(())
    }

    pub(crate) fn encode(
        form: IeeeFloatArithmeticForm,
        registers: &[u8],
    ) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
        validate_registers(form, registers)?;
        Ok(program(form, registers)
            .into_iter()
            .flat_map(|word| encode_word(word).to_le_bytes())
            .collect())
    }

    pub(crate) fn validate(
        form: IeeeFloatArithmeticForm,
        registers: &[u8],
        bytes: &[u8],
    ) -> Result<(), Aarch64SelectedFormEncodingError> {
        validate_registers(form, registers)?;
        let (words, rest) = bytes.as_chunks::<4>();
        if !rest.is_empty() {
            return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
        }
        let decoded = words
            .iter()
            .map(|word| decode_word(u32::from_le_bytes(*word)))
            .collect::<Option<Vec<_>>>()
            .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
        if decoded != program(form, registers) {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::{encode, validate};
        use selected_instructions::{
            IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatFormat,
            IeeeFloatIntegerCarrier, IeeeFloatToIntegerDomain,
        };

        fn form(
            operation: IeeeFloatArithmeticOperation,
            format: IeeeFloatFormat,
        ) -> IeeeFloatArithmeticForm {
            IeeeFloatArithmeticForm { operation, format }
        }

        fn words(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Vec<u32> {
            encode(form, registers)
                .unwrap()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|word| u32::from_le_bytes(*word))
                .collect()
        }

        #[test]
        fn directed_division_uses_output_after_capturing_inputs_and_restores_fpcr() {
            for (operation, expected_mode) in [(54, 3), (55, 1), (56, 2)] {
                for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                    let form = form(IeeeFloatArithmeticOperation::ALL[operation], format);
                    for registers in [[1, 2, 1], [1, 2, 2], [0, 0, 0], [28, 29, 30]] {
                        let program = super::program(form, &registers);
                        assert_eq!(
                            program[2],
                            super::Word::FloatingControl {
                                read: true,
                                general: registers[2]
                            }
                        );
                        assert_eq!(
                            program[4],
                            super::Word::MoveWide {
                                keep: false,
                                shift: 1,
                                immediate: expected_mode << 6,
                                general: registers[2]
                            }
                        );
                        assert_eq!(
                            program[8],
                            super::Word::FloatingControl {
                                read: false,
                                general: registers[2]
                            }
                        );
                        let bytes = super::encode(form, &registers).unwrap();
                        super::validate(form, &registers, &bytes).unwrap();
                        assert_eq!(
                            super::effects(form).stack,
                            selected_instructions::MachineEncodedStackEffect::UnchangedV1
                        );
                    }
                }
            }
        }

        #[test]
        fn directed_square_root_restores_fpcr_after_one_source_fsqrt() {
            for (operation, expected_mode) in [(67, 3), (68, 1), (69, 2)] {
                for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                    let form = form(IeeeFloatArithmeticOperation::ALL[operation], format);
                    for registers in [[1, 1], [1, 2], [0, 0], [28, 30]] {
                        let program = super::program(form, &registers);
                        assert_eq!(
                            program[1],
                            super::Word::FloatingControl {
                                read: true,
                                general: registers[1]
                            }
                        );
                        assert_eq!(
                            program[3],
                            super::Word::MoveWide {
                                keep: false,
                                shift: 1,
                                immediate: expected_mode << 6,
                                general: registers[1]
                            }
                        );
                        assert_eq!(
                            program[5],
                            super::Word::OneSource {
                                double: !form.is_binary32(),
                                opcode: 0b00_0011,
                                source: super::LEFT_SCRATCH,
                                result: super::LEFT_SCRATCH,
                            }
                        );
                        assert_eq!(
                            program[7],
                            super::Word::FloatingControl {
                                read: false,
                                general: registers[1]
                            }
                        );
                        let bytes = super::encode(form, &registers).unwrap();
                        super::validate(form, &registers, &bytes).unwrap();
                    }
                }
            }
        }

        #[test]
        fn fused_multiply_add_preserves_three_sources_and_one_rounding_instruction() {
            use IeeeFloatArithmeticOperation::{FusedMultiplyAdd, MultiplyThenAdd};
            use IeeeFloatFormat::{Binary32, Binary64};
            for (format, expected) in [
                (
                    Binary32,
                    [
                        0x1e27_0030,
                        0x1e27_0051,
                        0x1e27_0072,
                        0x1f11_4a10,
                        0x1e26_0200,
                    ],
                ),
                (
                    Binary64,
                    [
                        0x9e67_0030,
                        0x9e67_0051,
                        0x9e67_0072,
                        0x1f51_4a10,
                        0x9e66_0200,
                    ],
                ),
            ] {
                let fused = form(FusedMultiplyAdd, format);
                assert_eq!(words(fused, &[1, 2, 3, 0]), expected);
                for registers in [[1, 2, 3, 1], [1, 2, 3, 2], [1, 2, 3, 3], [0, 0, 0, 0]] {
                    let bytes = encode(fused, &registers).unwrap();
                    validate(fused, &registers, &bytes).unwrap();
                    let unfused = encode(form(MultiplyThenAdd, format), &registers).unwrap();
                    assert!(validate(fused, &registers, &unfused).is_err());
                    assert!(validate(form(MultiplyThenAdd, format), &registers, &bytes).is_err());
                }
            }
        }

        #[test]
        fn encodings_match_the_architectural_words() {
            use IeeeFloatArithmeticOperation as Operation;
            use IeeeFloatFormat::{Binary32, Binary64};
            assert_eq!(
                words(form(Operation::Classify, Binary64), &[0, 0]),
                [
                    0x9e67_0010,
                    0xd374_f800,
                    0xb400_00a0,
                    0xd11f_fc00,
                    0xb400_0140,
                    0xd280_0040,
                    0x1400_000e,
                    0x9e66_0200,
                    0xd340_f800,
                    0xb400_0060,
                    0xd280_0060,
                    0x1400_0009,
                    0xd280_0080,
                    0x1400_0007,
                    0x9e66_0200,
                    0xd340_cc00,
                    0xb400_0060,
                    0xd280_0000,
                    0x1400_0002,
                    0xd280_0020,
                    0xb400_0100,
                    0x9e67_0011,
                    0x9e66_0200,
                    0xd37f_fc00,
                    0xd360_7c00,
                    0x9e67_0010,
                    0x0eb1_1e10,
                    0x9e66_0200,
                ]
            );
            assert_eq!(
                words(form(Operation::MultiplyThenAdd, Binary64), &[1, 2, 3, 0]),
                [
                    0x9e67_0030,
                    0x9e67_0051,
                    0x1e71_0a10,
                    0x9e67_0071,
                    0x1e71_2a10,
                    0x9e66_0200
                ]
            );
            assert_eq!(
                words(form(Operation::Add, Binary64), &[1, 2, 0]),
                [0x9e67_0030, 0x9e67_0051, 0x1e71_2a10, 0x9e66_0200]
            );
            assert_eq!(
                words(form(Operation::Divide, Binary32), &[3, 4, 5]),
                [0x1e27_0070, 0x1e27_0091, 0x1e31_1a10, 0x1e26_0205]
            );
            assert_eq!(
                words(form(Operation::Subtract, Binary64), &[0, 0, 0])[2],
                0x1e71_3a10
            );
            assert_eq!(
                words(form(Operation::Multiply, Binary32), &[0, 0, 0])[2],
                0x1e31_0a10
            );
            assert_eq!(
                words(form(Operation::Minimum, Binary64), &[1, 2, 0]),
                [
                    0x9e67_0030,
                    0x9e67_0051,
                    0x7ef0_e632,
                    0x2e71_1e12,
                    0x9e66_0240
                ]
            );
            assert_eq!(
                words(form(Operation::Maximum, Binary32), &[1, 2, 0])[2],
                0x7eb1_e612
            );
            assert_eq!(
                words(form(Operation::Negate, Binary64), &[3, 4]),
                [0x9e67_0070, 0x1e61_4210, 0x9e66_0204]
            );
            assert_eq!(
                words(form(Operation::SquareRoot, Binary32), &[3, 4]),
                [0x1e27_0070, 0x1e21_c210, 0x1e26_0204]
            );
            assert_eq!(
                words(form(Operation::ConvertFormat, Binary64), &[3, 4]),
                [0x1e27_0070, 0x1e22_c210, 0x9e66_0204]
            );
            assert_eq!(
                words(form(Operation::ConvertFormat, Binary32), &[3, 4]),
                [0x9e67_0070, 0x1e62_4210, 0x1e26_0204]
            );
            assert_eq!(
                words(
                    form(
                        Operation::FromInteger(IeeeFloatIntegerCarrier::I32),
                        Binary64
                    ),
                    &[3, 4]
                ),
                [0x1e62_0070, 0x9e66_0204]
            );
            assert_eq!(
                words(
                    form(
                        Operation::FromInteger(IeeeFloatIntegerCarrier::I64),
                        Binary32
                    ),
                    &[3, 4]
                ),
                [0x9e22_0070, 0x1e26_0204]
            );
            assert_eq!(
                words(
                    form(
                        Operation::FromInteger(IeeeFloatIntegerCarrier::U32),
                        Binary64
                    ),
                    &[3, 4]
                ),
                [0x1e63_0070, 0x9e66_0204]
            );
        }

        fn shares_realization(
            left: IeeeFloatArithmeticForm,
            right: IeeeFloatArithmeticForm,
        ) -> bool {
            use IeeeFloatArithmeticOperation::TruncateToInteger;
            left == right
                || matches!(
                    (left.operation, right.operation),
                    (
                        TruncateToInteger { target: left_target, domain: IeeeFloatToIntegerDomain::Exact | IeeeFloatToIntegerDomain::Saturating },
                        TruncateToInteger { target: right_target, domain: IeeeFloatToIntegerDomain::Exact | IeeeFloatToIntegerDomain::Saturating },
                    ) if left_target == right_target && left.format == right.format
                )
        }

        #[test]
        fn truncation_encodings_match_the_architectural_words() {
            use IeeeFloatArithmeticOperation::TruncateToInteger;
            use selected_instructions::IeeeFloatToIntegerDomain::{Exact, Saturating};
            assert_eq!(
                words(
                    form(
                        TruncateToInteger {
                            target: IeeeFloatIntegerCarrier::I32,
                            domain: Saturating,
                        },
                        IeeeFloatFormat::Binary64,
                    ),
                    &[3, 4]
                ),
                [0x9e67_0070, 0x1e78_0204, 0x9340_7c84]
            );
            assert_eq!(
                words(
                    form(
                        TruncateToInteger {
                            target: IeeeFloatIntegerCarrier::I64,
                            domain: Exact,
                        },
                        IeeeFloatFormat::Binary32,
                    ),
                    &[3, 4]
                ),
                [0x1e27_0070, 0x9e38_0204]
            );
            assert_eq!(
                words(
                    form(
                        TruncateToInteger {
                            target: IeeeFloatIntegerCarrier::U32,
                            domain: Saturating,
                        },
                        IeeeFloatFormat::Binary64,
                    ),
                    &[3, 4]
                ),
                [0x9e67_0070, 0x1e79_0204]
            );
        }

        #[test]
        fn narrow_carrier_encodings_match_the_architectural_words() {
            use IeeeFloatArithmeticOperation::{FromInteger, TruncateToInteger};
            use selected_instructions::IeeeFloatToIntegerDomain::Saturating;
            assert_eq!(
                words(
                    form(
                        FromInteger(IeeeFloatIntegerCarrier::I8),
                        IeeeFloatFormat::Binary64
                    ),
                    &[3, 4]
                ),
                [0x1300_1c64, 0x1e62_0090, 0x9e66_0204]
            );
            assert_eq!(
                words(
                    form(
                        FromInteger(IeeeFloatIntegerCarrier::U16),
                        IeeeFloatFormat::Binary32
                    ),
                    &[3, 4]
                ),
                [0x5300_3c64, 0x1e23_0090, 0x1e26_0204]
            );
            assert_eq!(
                words(
                    form(
                        TruncateToInteger {
                            target: IeeeFloatIntegerCarrier::I8,
                            domain: Saturating,
                        },
                        IeeeFloatFormat::Binary64,
                    ),
                    &[3, 4]
                ),
                [
                    0x9e67_0070,
                    0xd2f8_0c04,
                    0x9e67_0091,
                    0x1e71_4a10,
                    0xd2e8_0be4,
                    0xf2d8_0004,
                    0x9e67_0091,
                    0x1e71_5a10,
                    0x1e78_0204,
                    0x9340_7c84
                ]
            );
        }

        #[test]
        fn validation_binds_every_bit_operation_format_and_register() {
            for form in IeeeFloatArithmeticForm::ALL {
                let registers = &[6, 7, 30, 12][4 - form.operand_count() - 1..];
                let bytes = encode(form, registers).unwrap();
                validate(form, registers, &bytes).unwrap();
                for byte_index in 0..bytes.len() {
                    for bit in 0..8 {
                        let mut changed = bytes.clone();
                        changed[byte_index] ^= 1 << bit;
                        assert!(
                            validate(form, registers, &changed).is_err(),
                            "{form:?} accepted byte {byte_index} bit {bit}"
                        );
                    }
                }
                for other in IeeeFloatArithmeticForm::ALL {
                    if other.operand_count() == form.operand_count()
                        && !shares_realization(other, form)
                    {
                        assert!(
                            validate(other, registers, &bytes).is_err(),
                            "{other:?} as {form:?}"
                        );
                    }
                }
                let mut result_changed = registers.to_vec();
                *result_changed.last_mut().unwrap() += 1;
                assert!(validate(form, &result_changed, &bytes).is_err());
                let mut operands_swapped = registers.to_vec();
                operands_swapped.swap(0, registers.len() - 1);
                assert!(validate(form, &operands_swapped, &bytes).is_err());
                let mut wide = vec![31];
                wide.extend_from_slice(&registers[1..]);
                assert!(encode(form, &wide).is_err());
                assert!(encode(form, &[1, 2, 3, 4, 5]).is_err());
                assert!(validate(form, registers, &bytes[..bytes.len() - 4]).is_err());
            }
        }
    }
}
#[cfg(test)]
mod integer_normalization_tests {
    use crate::test_support::rejects_every_bit_flip;
    use register_model::validate_physical_register_model;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
        SelectedInstructionKind,
    };

    use super::{encode_aarch64_selected_form, validate_aarch64_selected_form_encoding};
    use crate::aarch64_physical_register_model;

    type Kind = SelectedInstructionKind;
    type Family = MachineAlternativeFamily;

    macro_rules! r {
        ($a:ident => $b:ident) => {
            (Kind::$a, Family::$b)
        };
        ($a:ident => $b:ident => $c:expr => $d:expr => $e:expr) => {
            (Kind::$a, Family::$b, $c, $d, $e)
        };
    }

    fn forms() -> [(SelectedInstructionKind, MachineAlternativeFamily); 6] {
        [
            r!(ZeroExtendU8 => ZeroExtendU8),
            r!(ZeroExtendU16 => ZeroExtendU16),
            r!(ZeroExtendU32 => ZeroExtendU32),
            r!(SignExtendI8 => SignExtendI8),
            r!(SignExtendI16 => SignExtendI16),
            r!(SignExtendI32 => SignExtendI32),
        ]
    }

    #[test]
    fn normalization_binds_signedness_width_registers_and_full_destination_effects() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        for (source, destination) in [
            ("x0", "x0"),
            ("x0", "x1"),
            ("x1", "x9"),
            ("x9", "x16"),
            ("x16", "x29"),
            ("x29", "x30"),
            ("x30", "x0"),
            ("x30", "x30"),
        ] {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            for (kind, family) in forms() {
                let key = MachineAlternativeKey { family, variant: 0 };
                let encoded =
                    encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
                let footprint = encoded.footprint();
                assert_eq!(footprint.register_reads, vec![operands[0]]);
                assert_eq!(footprint.register_writes, vec![operands[1]]);
                assert!(!footprint.writes_nzcv);
                assert_eq!(
                    footprint.encoded,
                    MachineEncodedEffects::fallthrough_v1(vec![0], vec![1])
                );
                for (other_kind, other_family) in forms() {
                    if other_kind == kind {
                        continue;
                    }
                    let other_key = MachineAlternativeKey {
                        family: other_family,
                        variant: 0,
                    };
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical,
                            other_kind,
                            other_key,
                            &operands,
                            encoded.bytes()
                        )
                        .is_err(),
                        "{kind:?} must not replay as {other_kind:?}"
                    );
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical,
                            kind,
                            other_key,
                            &operands,
                            encoded.bytes()
                        )
                        .is_err()
                    );
                }
                rejects_every_bit_flip(&physical, kind, key, &operands, encoded.bytes());
            }
        }
    }

    #[test]
    fn normalization_matches_clang_assembler_oracle() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let fixtures: [(
            SelectedInstructionKind,
            MachineAlternativeFamily,
            &str,
            &str,
            &[u8],
        ); 4] = [
            r!(ZeroExtendU16 => ZeroExtendU16 => "x16" => "x9" => &[0x09, 0x3e, 0x40, 0xd3]),
            r!(SignExtendI8 => SignExtendI8 => "x16" => "x9" => &[0x09, 0x1e, 0x40, 0x93]),
            r!(SignExtendI16 => SignExtendI16 => "x16" => "x9" => &[0x09, 0x3e, 0x40, 0x93]),
            r!(SignExtendI32 => SignExtendI32 => "x16" => "x9" => &[0x09, 0x7e, 0x40, 0x93]),
        ];
        for (kind, family, source, destination, expected) in fixtures {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            let key = MachineAlternativeKey { family, variant: 0 };
            assert_eq!(
                encode_aarch64_selected_form(&physical, kind, key, &operands)
                    .unwrap()
                    .bytes(),
                expected
            );
            validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, expected)
                .unwrap();
        }
    }
}
mod jump {
    use super::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
        ValidatedAarch64SelectedFormEncoding,
    };
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::MachineEncodedControlEffect;
    use selected_instructions::MachineEncodedEffects;
    use selected_instructions::MachineEncodedMemoryEffect;
    use selected_instructions::MachineEncodedStackEffect;
    use selected_instructions::MachineEncodedTrapBehavior;
    pub fn encode_aarch64_selected_jump_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        displacement: i64,
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        if displacement % 4 != 0 {
            return Err(Aarch64SelectedFormEncodingError::BranchDisplacementMisaligned);
        }
        let words = displacement / 4;
        if !(-(1_i64 << 25)..(1_i64 << 25)).contains(&words) {
            return Err(Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm26);
        }
        let word = 0x1400_0000 | ((words as u32) & 0x03ff_ffff);
        validate_aarch64_selected_jump_form(
            physical,
            alternative,
            displacement,
            &word.to_le_bytes(),
        )
    }

    pub fn validate_aarch64_selected_jump_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
        }
        if alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::Jump,
                variant: 0,
            })
        {
            return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
        }
        let bytes: [u8; 4] = bytes
            .try_into()
            .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
        let word = u32::from_le_bytes(bytes);
        let actual = i64::from(((word & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
        if word & 0xfc00_0000 != 0x1400_0000 || actual != displacement {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let pc = physical
            .model()
            .view_named("pc")
            .expect("canonical model has PC")
            .units
            .clone();
        Ok(ValidatedAarch64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: Aarch64SelectedFormFootprint {
                register_reads: vec![],
                register_writes: vec![],
                writes_nzcv: false,
                encoded: MachineEncodedEffects {
                    external_operand_reads: vec![],
                    external_operand_writes: vec![],
                    implicit_unit_uses: pc.clone(),
                    implicit_unit_defs: pc,
                    implicit_unit_clobbers: vec![],
                    memory: MachineEncodedMemoryEffect::NoneV1,
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
                },
            },
        })
    }

    #[cfg(test)]
    mod tests {
        use super::{encode_aarch64_selected_jump_form, validate_aarch64_selected_jump_form};
        use crate::aarch64_physical_register_model;
        use selected_instructions::MachineAlternativeFamily;
        use selected_instructions::MachineAlternativeKey;
        use selected_instructions::MachineEncodedControlEffect;
        #[test]
        fn jump_round_trip_rejects_wrong_target_and_opcode() {
            let physical =
                register_model::validate_physical_register_model(aarch64_physical_register_model())
                    .unwrap();
            let key = MachineAlternativeKey {
                family: MachineAlternativeFamily::Jump,
                variant: 0,
            };
            for displacement in [-134217728, -4, 0, 8, 134217724] {
                let encoded =
                    encode_aarch64_selected_jump_form(&physical, key, displacement).unwrap();
                assert_eq!(encoded.bytes().len(), 4);
                assert!(
                    validate_aarch64_selected_jump_form(
                        &physical,
                        key,
                        displacement + 4,
                        encoded.bytes()
                    )
                    .is_err()
                );
                let mut corrupt = encoded.bytes().to_vec();
                corrupt[3] ^= 0x80;
                assert!(
                    validate_aarch64_selected_jump_form(&physical, key, displacement, &corrupt)
                        .is_err()
                );
                assert_eq!(
                    encoded.footprint().encoded.control,
                    MachineEncodedControlEffect::UnconditionalRelativeBranchV1
                );
            }
            assert!(encode_aarch64_selected_jump_form(&physical, key, 134217728).is_err());
            assert!(encode_aarch64_selected_jump_form(&physical, key, 1).is_err());
        }
    }
}
mod memory {
    use super::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
        ValidatedAarch64SelectedFormEncoding, word_stream,
    };
    use crate::selected_form_encoding::selected_forms::{request_registers, resolve_registers};
    use register_model::RegisterViewId;
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::MachineEncodedControlEffect;
    use selected_instructions::MachineEncodedEffects;
    use selected_instructions::MachineEncodedMemoryEffect;
    use selected_instructions::MachineEncodedStackEffect;
    use selected_instructions::MachineEncodedTrapBehavior;
    use selected_instructions::SelectedInstructionKind;
    mod frame {
        use super::{
            Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
            MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
            MachineEncodedTrapBehavior, RegisterViewId, SelectedInstructionKind,
            ValidatedAarch64SelectedFormEncoding, ValidatedPhysicalRegisterModel,
            resolve_registers, word_stream,
        };
        use crate::immediate_offset::{
            ADD_IMMEDIATE, address_words, append_page_steps, decode_address_words,
        };

        fn store_address_words(scratch: u8, displacement: u32) -> Vec<u32> {
            let pages = u64::from(displacement >> 12);
            if pages == 0 {
                return vec![ADD_IMMEDIATE | (1 << 22) | (31 << 5) | u32::from(scratch)];
            }
            let mut words = Vec::new();
            append_page_steps(&mut words, ADD_IMMEDIATE, scratch, 31, pages);
            words
        }

        const fn store_word(register: u8, scratch: u8, displacement: u32) -> u32 {
            0xf900_0000
                | (((displacement & 4095) / 8) << 10)
                | ((scratch as u32) << 5)
                | register as u32
        }

        fn request(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
        ) -> Result<(u8, Option<u8>), Aarch64SelectedFormEncodingError> {
            let store = matches!(kind, SelectedInstructionKind::Store64 { .. });
            let family = if store {
                MachineAlternativeFamily::Store64
            } else {
                MachineAlternativeFamily::FrameAddress
            };
            if matches!(
                kind,
                SelectedInstructionKind::Store64 {
                    slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                    ..
                }
            ) || matches!(kind, SelectedInstructionKind::FrameAddress { slot: selected_instructions::FrameStorageSlotId::ActivationBase, byte_offset } if byte_offset != 0)
            {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            if matches!(
                kind,
                SelectedInstructionKind::Store64 {
                    slot: selected_instructions::FrameStorageSlotId::Incoming { .. },
                    ..
                }
            ) {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            if physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
                || !matches!(
                    kind,
                    SelectedInstructionKind::Store64 { .. }
                        | SelectedInstructionKind::FrameAddress { .. }
                )
                || alternative != (MachineAlternativeKey { family, variant: 0 })
                || (store && !displacement.is_multiple_of(8))
            {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            let registers = resolve_registers(physical, operands)?;
            match registers.as_slice() {
                [value, scratch] if store && value != scratch => Ok((*value, Some(*scratch))),
                [register] if !store => Ok((*register, None)),
                _ => Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
            }
        }

        pub fn encode(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
            alignment_bytes: u16,
        ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError>
        {
            if matches!(
                kind,
                SelectedInstructionKind::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                    ..
                }
            ) && (displacement != 0 || alignment_bytes != 1)
            {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            let (register, scratch) = request(physical, kind, alternative, operands, displacement)?;
            let words = if alignment_bytes != 1 {
                let member = match kind {
                    SelectedInstructionKind::FrameAddress { byte_offset, .. }
                    | SelectedInstructionKind::Store64 { byte_offset, .. } => byte_offset,
                    _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
                };
                if scratch.is_some() && !member.is_multiple_of(8) {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
                let address = scratch.unwrap_or(register);
                let mut words = crate::immediate_offset::aligned_frame_address_words(
                    address,
                    displacement,
                    alignment_bytes,
                    member,
                )
                .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
                if scratch.is_some() {
                    words.push(store_word(register, address, 0));
                }
                words
            } else if let Some(scratch) = scratch {
                let mut words = store_address_words(scratch, displacement);
                words.push(store_word(register, scratch, displacement));
                words
            } else {
                address_words(register, 31, u64::from(displacement))
            };
            let bytes = words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<u8>>();
            validate(
                physical,
                kind,
                alternative,
                operands,
                displacement,
                alignment_bytes,
                &bytes,
            )
        }

        pub fn validate(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
            alignment_bytes: u16,
            bytes: &[u8],
        ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError>
        {
            if matches!(
                kind,
                SelectedInstructionKind::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                    ..
                }
            ) && (displacement != 0 || alignment_bytes != 1)
            {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            let (register, scratch) = request(physical, kind, alternative, operands, displacement)?;
            let words =
                word_stream(bytes).ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
            if alignment_bytes != 1 {
                let member = match kind {
                    SelectedInstructionKind::FrameAddress { byte_offset, .. }
                    | SelectedInstructionKind::Store64 { byte_offset, .. } => byte_offset,
                    _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
                };
                let address = scratch.unwrap_or(register);
                let prefix = if scratch.is_some() {
                    if !member.is_multiple_of(8)
                        || words.last() != Some(&store_word(register, address, 0))
                    {
                        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                    }
                    &words[..words.len() - 1]
                } else {
                    words.as_slice()
                };
                if !crate::immediate_offset::validate_aligned_frame_address_words(
                    prefix,
                    address,
                    displacement,
                    alignment_bytes,
                    member,
                ) {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
            } else if let Some(scratch) = scratch {
                let address = store_address_words(scratch, displacement);
                let [prefix @ .., store] = words.as_slice() else {
                    return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
                };
                if prefix.len() != address.len() {
                    return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
                }
                let pages = if displacement >> 12 == 0 {
                    (prefix == address.as_slice()).then_some(0)
                } else {
                    decode_address_words(prefix, scratch, 31)
                };
                if pages != Some(u64::from(displacement & !4095))
                    || store & 0xffc0_0000 != 0xf900_0000
                    || store & 31 != u32::from(register)
                    || (store >> 5) & 31 != u32::from(scratch)
                    || ((store >> 10) & 4095) * 8 != (displacement & 4095)
                {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
            } else {
                let expected = address_words(register, 31, u64::from(displacement));
                if words.len() != expected.len() {
                    return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
                }
                if decode_address_words(&words, register, 31) != Some(u64::from(displacement)) {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
            }
            let store = scratch.is_some();
            let stack_pointer = physical
                .model()
                .view_named("sp")
                .expect("canonical stack pointer");
            Ok(ValidatedAarch64SelectedFormEncoding {
                bytes: bytes.to_vec(),
                footprint: Aarch64SelectedFormFootprint {
                    register_reads: if store { vec![operands[0]] } else { Vec::new() },
                    register_writes: vec![operands[usize::from(store)]],
                    writes_nzcv: false,
                    encoded: MachineEncodedEffects {
                        external_operand_reads: if store { vec![0] } else { Vec::new() },
                        external_operand_writes: vec![u16::from(store)],
                        implicit_unit_uses: stack_pointer.units.clone(),
                        implicit_unit_defs: Vec::new(),
                        implicit_unit_clobbers: Vec::new(),
                        memory: if store {
                            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                                stack_pointer: stack_pointer.id,
                                byte_count: 8,
                            }
                        } else {
                            MachineEncodedMemoryEffect::NoneV1
                        },
                        stack: MachineEncodedStackEffect::UnchangedV1,
                        trap: if store {
                            MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                        } else {
                            MachineEncodedTrapBehavior::NeverV1
                        },
                        control: MachineEncodedControlEffect::FallThroughV1,
                    },
                },
            })
        }

        #[cfg(test)]
        mod tests {
            use super::{
                MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind, encode,
                validate,
            };
            use crate::aarch64_physical_register_model;
            use crate::test_support::reject_every_bit_flip;
            use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};
            use semantic_vocabulary::{OperationId, PlaceId};

            #[test]
            fn aligned_frame_forms_keep_sp_fixed_and_members_after_alignment() {
                let physical = register_model::validate_physical_register_model(
                    aarch64_physical_register_model(),
                )
                .unwrap();
                let slot = FrameStorageSlotId::Local(LocalStorageSlotId::Structural {
                    operation: OperationId::new(104).unwrap(),
                    place: PlaceId::new(102).unwrap(),
                });
                let value = physical.model().view_named("x9").unwrap().id;
                let scratch = physical.model().view_named("x10").unwrap().id;
                for alignment in [32_u16, 64, 4096, 32768] {
                    for frame_offset in [0_u32, 16, 4096, 16_777_216] {
                        for member in [0, 8, 24] {
                            let kind = SelectedInstructionKind::FrameAddress {
                                slot,
                                byte_offset: member,
                            };
                            let alternative = MachineAlternativeKey {
                                family: MachineAlternativeFamily::FrameAddress,
                                variant: 0,
                            };
                            let encoded = encode(
                                &physical,
                                kind,
                                alternative,
                                &[value],
                                frame_offset,
                                alignment,
                            )
                            .unwrap();
                            assert!(!encoded.footprint.writes_nzcv);
                            for residue in (0..u64::from(alignment)).step_by(16) {
                                let stack = 0x10_0000 + residue;
                                let mut address = stack;
                                for word in encoded
                                    .bytes()
                                    .as_chunks::<4>()
                                    .0
                                    .iter()
                                    .map(|bytes| u32::from_le_bytes(*bytes))
                                {
                                    if word & 0xffc0_0000 == 0x9240_0000 {
                                        let mask = (u64::MAX >> (63 - ((word >> 10) & 63)))
                                            .rotate_right((word >> 16) & 63);
                                        address &= mask;
                                    } else {
                                        assert_eq!(
                                            word & 0xff80_0000,
                                            crate::immediate_offset::ADD_IMMEDIATE
                                        );
                                        let base = if (word >> 5) & 31 == 31 {
                                            stack
                                        } else {
                                            address
                                        };
                                        let immediate = u64::from((word >> 10) & 4095)
                                            << if word & (1 << 22) != 0 { 12 } else { 0 };
                                        address = base + immediate;
                                    }
                                }
                                let home = address - u64::from(member);
                                assert!(home.is_multiple_of(u64::from(alignment)));
                                let reserved_start = stack + u64::from(frame_offset);
                                assert!(home >= reserved_start);
                                assert!(home - reserved_start <= u64::from(alignment - 16));
                            }
                            reject_every_bit_flip(encoded.bytes(), "frame form", |corrupt| {
                                validate(
                                    &physical,
                                    kind,
                                    alternative,
                                    &[value],
                                    frame_offset,
                                    alignment,
                                    corrupt,
                                )
                                .is_err()
                            });
                            let store = SelectedInstructionKind::Store64 {
                                slot,
                                byte_offset: member,
                            };
                            let store_alternative = MachineAlternativeKey {
                                family: MachineAlternativeFamily::Store64,
                                variant: 0,
                            };
                            let stored = encode(
                                &physical,
                                store,
                                store_alternative,
                                &[value, scratch],
                                frame_offset,
                                alignment,
                            )
                            .unwrap();
                            assert_eq!(stored.footprint.register_writes, vec![scratch]);
                            assert!(
                                validate(
                                    &physical,
                                    store,
                                    store_alternative,
                                    &[value, scratch],
                                    frame_offset,
                                    alignment,
                                    stored.bytes()
                                )
                                .is_ok()
                            );
                        }
                    }
                }
            }

            #[test]
            fn local_frame_forms_replay_sp_offset_register_and_all_opcode_bits() {
                let physical = register_model::validate_physical_register_model(
                    aarch64_physical_register_model(),
                )
                .unwrap();
                let slot = FrameStorageSlotId::Local(LocalStorageSlotId::Structural {
                    operation: OperationId::new(104).unwrap(),
                    place: PlaceId::new(102).unwrap(),
                });
                for store in [false, true] {
                    let kind = if store {
                        SelectedInstructionKind::Store64 {
                            slot,
                            byte_offset: 0,
                        }
                    } else {
                        SelectedInstructionKind::FrameAddress {
                            slot,
                            byte_offset: 0,
                        }
                    };
                    let alternative = MachineAlternativeKey {
                        family: if store {
                            MachineAlternativeFamily::Store64
                        } else {
                            MachineAlternativeFamily::FrameAddress
                        },
                        variant: 0,
                    };
                    let displacements: &[u32] = if store {
                        &[
                            0,
                            8,
                            32760,
                            32768,
                            35040,
                            65_536,
                            16_777_208,
                            16_777_216,
                            33_558_536,
                            u32::MAX - 7,
                        ]
                    } else {
                        &[
                            0,
                            17,
                            4095,
                            4096,
                            4192,
                            16_777_215,
                            16_773_120,
                            33_554_431,
                            u32::MAX,
                        ]
                    };
                    for displacement in displacements.iter().copied() {
                        let mut operands = vec![physical.model().view_named("x9").unwrap().id];
                        if store {
                            operands.push(physical.model().view_named("x10").unwrap().id);
                        }
                        let encoded =
                            encode(&physical, kind, alternative, &operands, displacement, 1)
                                .unwrap();
                        assert_eq!(
                            encoded.footprint.encoded.implicit_unit_uses,
                            physical.model().view_named("sp").unwrap().units
                        );
                        if store {
                            let pages = u64::from(displacement >> 12).div_ceil(4095).max(1);
                            assert_eq!(encoded.bytes().len() as u64, (pages + 1) * 4);
                            assert_eq!(encoded.footprint.register_writes, vec![operands[1]]);
                            assert!(
                                encode(
                                    &physical,
                                    kind,
                                    alternative,
                                    &[operands[0], operands[0]],
                                    displacement,
                                    1
                                )
                                .is_err()
                            );
                        } else {
                            assert_eq!(
                                encoded.bytes().len() as u64,
                                crate::immediate_offset::address_word_count(u64::from(
                                    displacement
                                )) * 4
                            );
                        }
                        assert!(!encoded.footprint.writes_nzcv);
                        reject_every_bit_flip(encoded.bytes(), "frame round trip", |corrupt| {
                            validate(
                                &physical,
                                kind,
                                alternative,
                                &operands,
                                displacement,
                                1,
                                corrupt,
                            )
                            .is_err()
                        });
                        assert!(
                            validate(
                                &physical,
                                kind,
                                alternative,
                                &[physical.model().view_named("x10").unwrap().id],
                                displacement,
                                1,
                                encoded.bytes()
                            )
                            .is_err()
                        );
                        assert!(
                            validate(
                                &physical,
                                kind,
                                alternative,
                                &operands,
                                displacement ^ 8,
                                1,
                                encoded.bytes()
                            )
                            .is_err()
                        );
                        assert!(
                            encode(
                                &physical,
                                kind,
                                alternative,
                                &[physical.model().view_named("sp").unwrap().id],
                                displacement,
                                1
                            )
                            .is_err()
                        );
                        assert!(
                            encode(&physical, kind, alternative, &[], displacement, 1).is_err()
                        );
                    }
                    if store {
                        assert!(
                            encode(
                                &physical,
                                kind,
                                alternative,
                                &[physical.model().view_named("x9").unwrap().id],
                                1,
                                1
                            )
                            .is_err()
                        );
                    }
                }
            }
        }
    }
    pub use frame::{
        encode as encode_aarch64_selected_frame_form,
        validate as validate_aarch64_selected_frame_form,
    };
    mod indexed {
        use super::{
            Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
            MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
            MachineEncodedTrapBehavior, RegisterViewId, ValidatedAarch64SelectedFormEncoding,
            ValidatedPhysicalRegisterModel, request_registers,
        };
        pub(super) fn encode(
            physical: &ValidatedPhysicalRegisterModel,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
        ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError>
        {
            let [base, index, destination] =
                request(physical, alternative, operands, displacement)?;
            let word = 0x3860_6800
                | (u32::from(index) << 16)
                | (u32::from(base) << 5)
                | u32::from(destination);
            validate(
                physical,
                alternative,
                operands,
                displacement,
                &word.to_le_bytes(),
            )
        }

        pub(super) fn validate(
            physical: &ValidatedPhysicalRegisterModel,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
            bytes: &[u8],
        ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError>
        {
            let [base, index, destination] =
                request(physical, alternative, operands, displacement)?;
            let word = bytes
                .try_into()
                .ok()
                .map(u32::from_le_bytes)
                .filter(|word| word & 0xffe0_fc00 == 0x3860_6800)
                .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
            if (word & 31) != u32::from(destination)
                || ((word >> 5) & 31) != u32::from(base)
                || ((word >> 16) & 31) != u32::from(index)
            {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            Ok(ValidatedAarch64SelectedFormEncoding {
                bytes: bytes.to_vec(),
                footprint: Aarch64SelectedFormFootprint {
                    register_reads: vec![operands[0], operands[1]],
                    register_writes: vec![operands[2]],
                    writes_nzcv: false,
                    encoded: MachineEncodedEffects {
                        external_operand_reads: vec![0, 1],
                        external_operand_writes: vec![2],
                        implicit_unit_uses: Vec::new(),
                        implicit_unit_defs: Vec::new(),
                        implicit_unit_clobbers: Vec::new(),
                        memory: MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                            pointer_operand: 0,
                            index_operand: 1,
                            byte_count: 1,
                        },
                        stack: MachineEncodedStackEffect::UnchangedV1,
                        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                        control: MachineEncodedControlEffect::FallThroughV1,
                    },
                },
            })
        }

        fn request(
            physical: &ValidatedPhysicalRegisterModel,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
        ) -> Result<[u8; 3], Aarch64SelectedFormEncodingError> {
            if displacement != 0 {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            request_registers(
                physical,
                alternative,
                MachineAlternativeFamily::Load8Indexed,
                operands,
                3,
            )
        }

        #[cfg(test)]
        mod tests {
            use super::super::{
                SelectedInstructionKind, check_memory, encode_aarch64_selected_memory_form,
            };
            use super::{
                MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
            };
            use crate::test_support::reject_every_bit_flip;

            #[test]
            fn indexed_byte_load_decodes_every_bit_and_operand_role() {
                let physical = register_model::validate_physical_register_model(
                    crate::aarch64_physical_register_model(),
                )
                .unwrap();
                let operands =
                    ["x1", "x2", "x3"].map(|name| physical.model().view_named(name).unwrap().id);
                let kind = SelectedInstructionKind::Load8Indexed;
                let alternative = MachineAlternativeKey {
                    family: MachineAlternativeFamily::Load8Indexed,
                    variant: 0,
                };
                let encoded =
                    encode_aarch64_selected_memory_form(&physical, kind, alternative, &operands, 0)
                        .unwrap();
                assert_eq!(encoded.bytes(), &0x3862_6823u32.to_le_bytes());
                assert_eq!(encoded.footprint().register_reads, operands[..2]);
                assert_eq!(encoded.footprint().register_writes, operands[2..]);
                assert_eq!(
                    encoded.footprint().encoded.memory,
                    MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                        pointer_operand: 0,
                        index_operand: 1,
                        byte_count: 1,
                    }
                );
                reject_every_bit_flip(encoded.bytes(), &format!("{kind:?}"), |corrupt| {
                    check_memory(&physical, kind, alternative, &operands, 0, corrupt).is_err()
                });
                for role in 0..3 {
                    let mut corrupt = operands;
                    corrupt[role] = operands[(role + 1) % 3];
                    assert!(
                        check_memory(&physical, kind, alternative, &corrupt, 0, encoded.bytes(),)
                            .is_err()
                    );
                }
                assert!(
                    check_memory(&physical, kind, alternative, &operands, 1, encoded.bytes(),)
                        .is_err()
                );
                assert!(
                    check_memory(
                        &physical,
                        kind,
                        alternative,
                        &operands[..2],
                        0,
                        encoded.bytes(),
                    )
                    .is_err()
                );
                assert!(
                    check_memory(
                        &physical,
                        kind,
                        MachineAlternativeKey {
                            family: MachineAlternativeFamily::Load64,
                            variant: 0,
                        },
                        &operands,
                        0,
                        encoded.bytes(),
                    )
                    .is_err()
                );
                for length in 0..encoded.bytes().len() {
                    assert!(
                        check_memory(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            0,
                            &encoded.bytes()[..length],
                        )
                        .is_err()
                    );
                }
                let mut trailing = encoded.bytes().to_vec();
                trailing.push(0);
                assert!(
                    check_memory(&physical, kind, alternative, &operands, 0, &trailing,).is_err()
                );
                let stack = physical.model().view_named("sp").unwrap().id;
                for role in 0..3 {
                    let mut invalid = operands;
                    invalid[role] = stack;
                    assert!(
                        encode_aarch64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &invalid,
                            0,
                        )
                        .is_err()
                    );
                }
                for result in operands[..2].iter().copied() {
                    assert!(
                        encode_aarch64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &[operands[0], operands[1], result],
                            0,
                        )
                        .is_ok()
                    );
                }
            }
        }
    }
    #[cfg(test)]
    mod load32_tests {
        use super::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
            SelectedInstructionKind, check_memory, encode_aarch64_selected_memory_form,
        };
        #[test]
        fn load32_preserves_exact_four_byte_extent_and_rejects_a_widened_load() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let operands = [
                physical.model().view_named("x1").unwrap().id,
                physical.model().view_named("x2").unwrap().id,
            ];
            let kind = SelectedInstructionKind::Load32 { byte_offset: 4 };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::Load32,
                variant: 0,
            };
            let encoded =
                encode_aarch64_selected_memory_form(&physical, kind, alternative, &operands, 4)
                    .unwrap();
            assert_eq!(encoded.bytes(), &[0x22, 0x04, 0x40, 0xb9]);
            assert_eq!(
                encoded.footprint().encoded.memory,
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: 4
                }
            );
            let mut widened = encoded.bytes().to_vec();
            widened[3] |= 0x40;
            assert!(check_memory(&physical, kind, alternative, &operands, 4, &widened).is_err());
            assert!(
                check_memory(&physical, kind, alternative, &operands, 8, encoded.bytes()).is_err()
            );
            assert!(
                check_memory(
                    &physical,
                    kind,
                    alternative,
                    &[operands[1], operands[0]],
                    4,
                    encoded.bytes()
                )
                .is_err()
            );
        }
    }
    #[cfg(test)]
    mod narrow_load_tests {
        use super::{encode_aarch64_selected_memory_form, validate_aarch64_selected_memory_form};
        use crate::selected_form_encoding::ValidatedAarch64SelectedFormEncoding;
        use isa_conformance::narrow_load::{self, Exact, Footprint, Roster};

        fn observed(encoded: &ValidatedAarch64SelectedFormEncoding) -> Footprint {
            Footprint {
                bytes: encoded.bytes().to_vec(),
                register_reads: encoded.footprint().register_reads.clone(),
                register_writes: encoded.footprint().register_writes.clone(),
                memory: encoded.footprint().encoded.memory,
                stack: encoded.footprint().encoded.stack,
                trap: encoded.footprint().encoded.trap,
                flags_untouched: !encoded.footprint().writes_nzcv,
            }
        }

        #[test]
        fn narrow_loads_bind_widths_footprints_opcodes_and_refuse_substituted_requests() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            narrow_load::conformance(
                &physical,
                &Roster {
                    bases: &["x0", "x1", "x15", "x28"],
                    destination: "x2",
                    offsets: |width| vec![0, u32::from(width), 4095 * u32::from(width)],
                },
                &Exact {
                    operands: ["x1", "x2"],
                    expected: [(1, &[0x22, 0, 0x40, 0x39]), (2, &[0x22, 0, 0x40, 0x79])],
                    signed_opcode: (2, [0x80, 0x80]),
                    invalid_views: &["sp", "w1", "d0"],
                    invalid_offsets: &[(2, 1), (2, 8192), (1, 4096)],
                },
                |kind, alternative, operands, displacement| {
                    encode_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        operands,
                        displacement,
                    )
                    .map(|encoded| observed(&encoded))
                },
                |kind, alternative, operands, displacement, bytes| {
                    validate_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        operands,
                        displacement,
                        bytes,
                    )
                    .map(|encoded| observed(&encoded))
                },
            );
        }
    }
    mod packed {
        use super::{
            Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
            MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
            MachineEncodedTrapBehavior, RegisterViewId, SelectedInstructionKind,
            ValidatedAarch64SelectedFormEncoding, ValidatedPhysicalRegisterModel,
            request_registers,
        };
        use selected_instructions::PackedByteWidth;

        fn request(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
        ) -> Result<(bool, u8, [u8; 3]), Aarch64SelectedFormEncodingError> {
            let (load, byte_offset, width, family) = match kind {
                SelectedInstructionKind::LoadPacked { byte_offset, width } => (
                    true,
                    byte_offset,
                    width,
                    match width {
                        PackedByteWidth::Three => MachineAlternativeFamily::LoadPacked3,
                        PackedByteWidth::Five => MachineAlternativeFamily::LoadPacked5,
                        PackedByteWidth::Six => MachineAlternativeFamily::LoadPacked6,
                        PackedByteWidth::Seven => MachineAlternativeFamily::LoadPacked7,
                    },
                ),
                SelectedInstructionKind::StorePacked { byte_offset, width } => (
                    false,
                    byte_offset,
                    width,
                    MachineAlternativeFamily::StorePacked,
                ),
                _ => return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch),
            };
            let width = width.byte_size();
            let registers: [u8; 3] = request_registers(physical, alternative, family, operands, 2)?;
            let [base, value, _] = registers;
            if load && base == value {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            if byte_offset != displacement
                || displacement
                    .checked_add(u32::from(width) - 1)
                    .is_none_or(|last| last > 4095)
            {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            Ok((load, width, registers))
        }

        pub(super) fn encode(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
        ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError>
        {
            let (load, width, [base, value, scratch]) =
                request(physical, kind, alternative, operands, displacement)?;
            let (base, value, scratch) = (u32::from(base), u32::from(value), u32::from(scratch));
            let mut bytes = Vec::with_capacity(usize::from(width) * 8);
            if !load {
                crate::codec_bytes::push_encoded(
                    &mut bytes,
                    (0xaa00_03e0 | (value << 16) | scratch).to_le_bytes(),
                );
            }
            for byte in 0..u32::from(width) {
                let offset = displacement + byte;
                if load {
                    let destination = if byte == 0 { value } else { scratch };
                    bytes.extend_from_slice(
                        &(0x3940_0000 | (offset << 10) | (base << 5) | destination).to_le_bytes(),
                    );
                    if byte != 0 {
                        bytes.extend_from_slice(
                            &(0xaa00_0000
                                | (scratch << 16)
                                | ((byte * 8) << 10)
                                | (value << 5)
                                | value)
                                .to_le_bytes(),
                        );
                    }
                } else {
                    bytes.extend_from_slice(
                        &(0x3900_0000 | (offset << 10) | (base << 5) | scratch).to_le_bytes(),
                    );
                    if byte + 1 != u32::from(width) {
                        crate::codec_bytes::push_encoded(
                            &mut bytes,
                            (0xd348_fc00 | (scratch << 5) | scratch).to_le_bytes(),
                        );
                    }
                }
            }
            validate(physical, kind, alternative, operands, displacement, &bytes)
        }

        pub(super) fn validate(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
            bytes: &[u8],
        ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError>
        {
            let (load, width, [base, value, scratch]) =
                request(physical, kind, alternative, operands, displacement)?;
            let instruction_count = usize::from(width) * 2 - usize::from(load);
            if bytes.len() != instruction_count * 4 {
                return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
            }
            let (base, value, scratch) = (u32::from(base), u32::from(value), u32::from(scratch));
            let mut words = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| u32::from_le_bytes(*bytes));
            let invalid = || Aarch64SelectedFormEncodingError::EncodedFormMismatch;
            if !load {
                let copy = words.next().ok_or_else(invalid)?;
                if copy & !((31 << 16) | 31) != 0xaa00_03e0
                    || (copy >> 16) & 31 != value
                    || copy & 31 != scratch
                {
                    return Err(invalid());
                }
            }
            for byte in 0..u32::from(width) {
                let memory = words.next().ok_or_else(invalid)?;
                let register = if load && byte == 0 { value } else { scratch };
                if memory & 0xffc0_0000 != if load { 0x3940_0000 } else { 0x3900_0000 }
                    || memory & 31 != register
                    || (memory >> 5) & 31 != base
                    || (memory >> 10) & 4095 != displacement + byte
                {
                    return Err(invalid());
                }
                if load && byte != 0 {
                    let combine = words.next().ok_or_else(invalid)?;
                    if combine & 0xffe0_0000 != 0xaa00_0000
                        || (combine >> 16) & 31 != scratch
                        || (combine >> 10) & 63 != byte * 8
                        || (combine >> 5) & 31 != value
                        || combine & 31 != value
                    {
                        return Err(invalid());
                    }
                } else if !load && byte + 1 != u32::from(width) {
                    let shift = words.next().ok_or_else(invalid)?;
                    if shift & !((31 << 5) | 31) != 0xd348_fc00
                        || (shift >> 5) & 31 != scratch
                        || shift & 31 != scratch
                    {
                        return Err(invalid());
                    }
                }
            }
            if words.next().is_some() {
                return Err(invalid());
            }
            Ok(ValidatedAarch64SelectedFormEncoding {
                bytes: bytes.to_vec(),
                footprint: Aarch64SelectedFormFootprint {
                    register_reads: if load {
                        vec![operands[0]]
                    } else {
                        vec![operands[0], operands[1]]
                    },
                    register_writes: if load {
                        vec![operands[1], operands[2]]
                    } else {
                        vec![operands[2]]
                    },
                    writes_nzcv: false,
                    encoded: MachineEncodedEffects {
                        external_operand_reads: if load { vec![0] } else { vec![0, 1] },
                        external_operand_writes: if load { vec![1, 2] } else { vec![2] },
                        implicit_unit_uses: Vec::new(),
                        implicit_unit_defs: Vec::new(),
                        implicit_unit_clobbers: Vec::new(),
                        memory: if load {
                            MachineEncodedMemoryEffect::ReadPointerV1 {
                                pointer_operand: 0,
                                byte_count: u16::from(width),
                            }
                        } else {
                            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
                        },
                        stack: MachineEncodedStackEffect::UnchangedV1,
                        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                        control: MachineEncodedControlEffect::FallThroughV1,
                    },
                },
            })
        }

        #[cfg(test)]
        mod tests {
            use super::super::{check_memory, encode_aarch64_selected_memory_form};
            use super::{MachineAlternativeKey, PackedByteWidth, SelectedInstructionKind};
            use crate::aarch64_physical_register_model;
            use crate::test_support::reject_every_bit_flip;
            use selected_instructions::{MachineSemanticKind, MachineSizeKnowledge};

            fn case(
                load: bool,
                width: PackedByteWidth,
                byte_offset: u32,
            ) -> (
                SelectedInstructionKind,
                MachineAlternativeKey,
                MachineSemanticKind,
            ) {
                let semantic = if load {
                    match width {
                        PackedByteWidth::Three => MachineSemanticKind::LoadPacked3,
                        PackedByteWidth::Five => MachineSemanticKind::LoadPacked5,
                        PackedByteWidth::Six => MachineSemanticKind::LoadPacked6,
                        PackedByteWidth::Seven => MachineSemanticKind::LoadPacked7,
                    }
                } else {
                    MachineSemanticKind::StorePacked
                };
                (
                    if load {
                        SelectedInstructionKind::LoadPacked { byte_offset, width }
                    } else {
                        SelectedInstructionKind::StorePacked { byte_offset, width }
                    },
                    MachineAlternativeKey {
                        family: semantic.into(),
                        variant: 0,
                    },
                    semantic,
                )
            }

            #[test]
            fn packed_forms_access_exact_bytes_and_declare_all_scratch_writes() {
                let physical = register_model::validate_physical_register_model(
                    aarch64_physical_register_model(),
                )
                .unwrap();
                let constraints = crate::validate_aarch64_register_constraint_catalog(
                    crate::aarch64_register_constraint_catalog(&physical),
                    &physical,
                )
                .unwrap();
                let operands =
                    ["x3", "x7", "x12"].map(|name| physical.model().view_named(name).unwrap().id);
                for target in [
                    target::NativeTarget::linux_arm64(),
                    target::NativeTarget::macos_arm64(),
                ] {
                    let catalog =
                        crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
                    for width in [
                        PackedByteWidth::Three,
                        PackedByteWidth::Five,
                        PackedByteWidth::Six,
                        PackedByteWidth::Seven,
                    ] {
                        for load in [true, false] {
                            for offset in [0, 4096 - u32::from(width.byte_size())] {
                                let (kind, alternative, semantic) = case(load, width, offset);
                                let encoded = encode_aarch64_selected_memory_form(
                                    &physical,
                                    kind,
                                    alternative,
                                    &operands,
                                    i64::from(offset),
                                )
                                .unwrap();
                                let effects = &encoded.footprint().encoded;
                                let row = catalog.declaration_by_semantic(semantic).unwrap();
                                assert_eq!(effects, &row.alternatives[0].encoded);
                                assert_eq!(
                                    encoded.bytes().len(),
                                    (usize::from(width.byte_size()) * 2 - usize::from(load)) * 4
                                );
                                if load {
                                    assert_eq!(
                                        row.alternatives[0].size,
                                        MachineSizeKnowledge::ExactBytes(
                                            (u16::from(width.byte_size()) * 2 - 1) * 4
                                        )
                                    );
                                }
                                assert_eq!(
                                    effects.external_operand_writes,
                                    if load { vec![1, 2] } else { vec![2] }
                                );
                                assert_eq!(
                                    effects.external_operand_reads,
                                    if load { vec![0] } else { vec![0, 1] }
                                );
                                assert!(effects.implicit_unit_clobbers.is_empty());
                                assert!(effects.implicit_unit_defs.is_empty());
                                assert!(!encoded.footprint().writes_nzcv);
                                let accesses = encoded
                                    .bytes()
                                    .as_chunks::<4>()
                                    .0
                                    .iter()
                                    .map(|bytes| u32::from_le_bytes(*bytes))
                                    .filter(|word| {
                                        word & 0xffc0_0000
                                            == if load { 0x3940_0000 } else { 0x3900_0000 }
                                    })
                                    .map(|word| (word >> 10) & 4095)
                                    .collect::<Vec<_>>();
                                assert_eq!(
                                    accesses,
                                    (offset..offset + u32::from(width.byte_size()))
                                        .collect::<Vec<_>>()
                                );
                                assert_eq!(
                                    check_memory(
                                        &physical,
                                        kind,
                                        alternative,
                                        &operands,
                                        i64::from(offset),
                                        encoded.bytes()
                                    )
                                    .unwrap(),
                                    encoded
                                );
                            }
                        }
                    }
                }
            }

            #[test]
            fn packed_forms_reject_width_offset_alias_and_instruction_mutations() {
                let physical = register_model::validate_physical_register_model(
                    aarch64_physical_register_model(),
                )
                .unwrap();
                let operands =
                    ["x3", "x7", "x12"].map(|name| physical.model().view_named(name).unwrap().id);
                for load in [true, false] {
                    let (kind, alternative, _) = case(load, PackedByteWidth::Three, 16);
                    let encoded = encode_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        16,
                    )
                    .unwrap();
                    reject_every_bit_flip(encoded.bytes(), &format!("{kind:?}"), |changed| {
                        check_memory(&physical, kind, alternative, &operands, 16, changed).is_err()
                    });
                    for width in [
                        PackedByteWidth::Five,
                        PackedByteWidth::Six,
                        PackedByteWidth::Seven,
                    ] {
                        let (other_kind, other_alternative, _) = case(load, width, 16);
                        assert!(
                            check_memory(
                                &physical,
                                other_kind,
                                other_alternative,
                                &operands,
                                16,
                                encoded.bytes()
                            )
                            .is_err()
                        );
                    }
                    for offset in [4094, u32::MAX] {
                        let (kind, alternative, _) = case(load, PackedByteWidth::Three, offset);
                        assert!(
                            encode_aarch64_selected_memory_form(
                                &physical,
                                kind,
                                alternative,
                                &operands,
                                i64::from(offset)
                            )
                            .is_err()
                        );
                    }
                    assert!(
                        encode_aarch64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            17
                        )
                        .is_err()
                    );
                    assert!(
                        encode_aarch64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands[..2],
                            16
                        )
                        .is_err()
                    );
                    for alias in [0, 1] {
                        let mut changed = operands;
                        changed[2] = changed[alias];
                        assert!(
                            encode_aarch64_selected_memory_form(
                                &physical,
                                kind,
                                alternative,
                                &changed,
                                16
                            )
                            .is_err()
                        );
                    }
                    let mut changed = operands;
                    changed[1] = changed[0];
                    assert_eq!(
                        encode_aarch64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &changed,
                            16
                        )
                        .is_err(),
                        load
                    );
                    let mut changed = operands;
                    changed[2] = physical.model().view_named("d0").unwrap().id;
                    assert!(
                        encode_aarch64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &changed,
                            16
                        )
                        .is_err()
                    );
                    assert!(
                        check_memory(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            16,
                            &encoded.bytes()[..encoded.bytes().len() - 4]
                        )
                        .is_err()
                    );
                }
            }

            #[test]
            fn packed_constraints_require_early_clobbers_and_effects_reject_scratch_omission() {
                let physical = register_model::validate_physical_register_model(
                    aarch64_physical_register_model(),
                )
                .unwrap();
                let catalog = crate::aarch64_register_constraint_catalog(&physical);
                for (key, load) in [
                    (crate::AARCH64_LOAD_PACKED, true),
                    (crate::AARCH64_STORE_PACKED, false),
                ] {
                    let row = catalog.constraint(key).unwrap();
                    assert_eq!(row.operands.len(), 3);
                    assert_eq!(row.operands[1].early_clobber, load);
                    assert!(row.operands[2].early_clobber);
                    assert!(row.clobbers.is_empty());
                    let mut changed = catalog.clone();
                    register_model::find_register_constraint_mut(&mut changed.constraints, key)
                        .unwrap()
                        .operands[2]
                        .early_clobber = false;
                    assert!(
                        crate::validate_aarch64_register_constraint_catalog(changed, &physical)
                            .is_err()
                    );
                }
                let constraints =
                    crate::validate_aarch64_register_constraint_catalog(catalog, &physical)
                        .unwrap();
                for target in [
                    target::NativeTarget::linux_arm64(),
                    target::NativeTarget::macos_arm64(),
                ] {
                    let mut effects =
                        crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
                    effects
                        .declaration_by_semantic_mut(MachineSemanticKind::StorePacked)
                        .unwrap()
                        .alternatives[0]
                        .encoded
                        .external_operand_writes
                        .clear();
                    assert!(
                        crate::validate_aarch64_machine_effect_catalog(
                            target,
                            &constraints,
                            effects
                        )
                        .is_err()
                    );
                }
            }
        }
    }
    #[cfg(test)]
    mod placed_scalar_tests {
        use crate::test_support::reject_every_bit_flip;
        use selected_instructions::{
            AccessorScalarForm, MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier,
            MachineEncodedMemoryEffect, MachineSemanticKind, SelectedInstructionKind,
        };

        fn accessor_scalar_request(
            form: AccessorScalarForm,
        ) -> (SelectedInstructionKind, MachineAlternativeKey) {
            (
                SelectedInstructionKind::AccessorScalarAccess {
                    form,
                    contract: [3; 32],
                    byte_offset: 8,
                },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::AccessorScalarAccess(form),
                    variant: 0,
                },
            )
        }

        #[test]
        fn accessor_scalar_forms_decode_one_exact_width_transfer() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let constraints = crate::validate_aarch64_register_constraint_catalog(
                crate::aarch64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap();
            let target = target::NativeTarget::linux_arm64();
            let catalog = crate::validate_aarch64_machine_effect_catalog(
                target,
                &constraints,
                crate::aarch64_machine_effect_catalog(target, &constraints).unwrap(),
            )
            .unwrap();
            let base = physical.model().view_named("x1").unwrap().id;
            let data = physical.model().view_named("x2").unwrap().id;
            for form in AccessorScalarForm::ALL {
                let (kind, alternative) = accessor_scalar_request(form);
                let declaration = catalog
                    .catalog()
                    .declaration_by_semantic(MachineSemanticKind::AccessorScalarAccess(form))
                    .unwrap();
                assert_eq!(declaration.barrier, MachineBarrier::ExternalEffect);
                let encoded = super::encode_aarch64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &[base, data],
                    8,
                )
                .unwrap();
                let expected = if form.write {
                    MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
                } else {
                    MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: u16::from(form.bytes),
                    }
                };
                assert_eq!(encoded.footprint().encoded.memory, expected);
                assert_eq!(
                    encoded.footprint().register_reads,
                    if form.write {
                        vec![base, data]
                    } else {
                        vec![base]
                    }
                );
                assert_eq!(
                    encoded.footprint().register_writes,
                    if form.write { vec![] } else { vec![data] }
                );
                let verify = |kind,
                              alternative,
                              operands: &[register_model::RegisterViewId],
                              displacement,
                              bytes: &[u8]| {
                    super::check_memory(&physical, kind, alternative, operands, displacement, bytes)
                };
                let verify_request =
                    |kind: SelectedInstructionKind, alternative: MachineAlternativeKey| {
                        verify(kind, alternative, &[base, data], 8, encoded.bytes())
                    };
                verify_request(kind, alternative).unwrap();
                assert!(verify(kind, alternative, &[data, base], 8, encoded.bytes()).is_err());
                assert!(verify(kind, alternative, &[data, data], 8, encoded.bytes()).is_err());
                assert!(verify(kind, alternative, &[base, data], 16, encoded.bytes()).is_err());
                let mut appended = encoded.bytes().to_vec();
                appended.extend_from_slice(encoded.bytes());
                assert!(verify(kind, alternative, &[base, data], 8, &appended).is_err());
                let changed = AccessorScalarForm {
                    write: !form.write,
                    bytes: form.bytes,
                };
                let (changed_kind, changed_alternative) = accessor_scalar_request(changed);
                assert!(verify_request(changed_kind, changed_alternative).is_err());
                let changed = AccessorScalarForm {
                    write: form.write,
                    bytes: if form.bytes == 8 { 4 } else { 8 },
                };
                let (changed_kind, changed_alternative) = accessor_scalar_request(changed);
                assert!(verify_request(changed_kind, changed_alternative).is_err());
                let undeclared = MachineAlternativeKey {
                    family: alternative.family,
                    variant: 1,
                };
                assert!(verify(kind, undeclared, &[base, data], 8, encoded.bytes()).is_err());
                reject_every_bit_flip(encoded.bytes(), &format!("{form:?}"), |changed| {
                    verify(kind, alternative, &[base, data], 8, changed).is_err()
                });
            }
        }
    }
    mod pointer {
        use super::{
            Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
            MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
            MachineEncodedTrapBehavior, RegisterViewId, SelectedInstructionKind,
            ValidatedAarch64SelectedFormEncoding, ValidatedPhysicalRegisterModel,
            request_registers, word_stream,
        };
        use crate::immediate_offset::{address_word_count, address_words, decode_address_words};

        fn request(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
        ) -> Result<(u32, u32, u8, u8), Aarch64SelectedFormEncodingError> {
            let (family, byte_offset, opcode, scale) = match kind {
                SelectedInstructionKind::Store {
                    byte_offset,
                    byte_size,
                } => {
                    let opcode = match byte_size {
                        1 => 0x3900_0000,
                        2 => 0x7900_0000,
                        4 => 0xb900_0000,
                        8 => 0xf900_0000,
                        _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
                    };
                    (
                        MachineAlternativeFamily::Store,
                        byte_offset,
                        opcode,
                        u32::from(byte_size),
                    )
                }
                SelectedInstructionKind::AddressOffset { byte_offset } => (
                    MachineAlternativeFamily::AddressOffset,
                    byte_offset,
                    0x9100_0000,
                    1,
                ),
                _ => return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch),
            };
            let [first, second] =
                request_registers::<2>(physical, alternative, family, operands, 2)?;
            if byte_offset != displacement
                || (matches!(kind, SelectedInstructionKind::Store { .. })
                    && (!displacement.is_multiple_of(scale) || displacement / scale > 4095))
            {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            Ok((opcode, scale, first, second))
        }

        pub(super) fn encode(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
        ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError>
        {
            let (opcode, scale, base, register) =
                request(physical, kind, alternative, operands, displacement)?;
            let bytes = if matches!(kind, SelectedInstructionKind::AddressOffset { .. }) {
                address_words(register, base, u64::from(displacement))
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect()
            } else {
                let word = opcode
                    | ((displacement / scale) << 10)
                    | (u32::from(base) << 5)
                    | u32::from(register);
                word.to_le_bytes().to_vec()
            };
            validate(physical, kind, alternative, operands, displacement, &bytes)
        }

        pub(super) fn validate(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
            bytes: &[u8],
        ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError>
        {
            let (opcode, scale, base, register) =
                request(physical, kind, alternative, operands, displacement)?;
            let store = matches!(kind, SelectedInstructionKind::Store { .. });
            if store {
                let word = bytes
                    .try_into()
                    .ok()
                    .map(u32::from_le_bytes)
                    .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
                if word & 0xffc0_0000 != opcode
                    || word & 31 != u32::from(register)
                    || (word >> 5) & 31 != u32::from(base)
                    || ((word >> 10) & 4095) * scale != displacement
                {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
            } else {
                let words = word_stream(bytes)
                    .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
                if words.len() as u64 != address_word_count(u64::from(displacement)) {
                    return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
                }
                if decode_address_words(&words, register, base) != Some(u64::from(displacement)) {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
            }
            Ok(ValidatedAarch64SelectedFormEncoding {
                bytes: bytes.to_vec(),
                footprint: Aarch64SelectedFormFootprint {
                    register_reads: if store {
                        operands.to_vec()
                    } else {
                        vec![operands[0]]
                    },
                    register_writes: if store { Vec::new() } else { vec![operands[1]] },
                    writes_nzcv: false,
                    encoded: MachineEncodedEffects {
                        external_operand_reads: if store { vec![0, 1] } else { vec![0] },
                        external_operand_writes: if store { Vec::new() } else { vec![1] },
                        implicit_unit_uses: Vec::new(),
                        implicit_unit_defs: Vec::new(),
                        implicit_unit_clobbers: Vec::new(),
                        memory: if store {
                            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
                        } else {
                            MachineEncodedMemoryEffect::NoneV1
                        },
                        stack: MachineEncodedStackEffect::UnchangedV1,
                        trap: if store {
                            MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                        } else {
                            MachineEncodedTrapBehavior::NeverV1
                        },
                        control: MachineEncodedControlEffect::FallThroughV1,
                    },
                },
            })
        }
    }
    #[cfg(test)]
    mod pointer_tests {
        use super::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
            RegisterViewId, SelectedInstructionKind, ValidatedPhysicalRegisterModel, check_memory,
            encode_aarch64_selected_memory_form,
        };
        use crate::test_support::reject_every_bit_flip;
        #[test]
        fn pointer_store_known_bytes_preserve_the_requested_width() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let operands = ["x12", "x9"].map(|name| physical.model().view_named(name).unwrap().id);
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::Store,
                variant: 0,
            };
            for (byte_size, expected) in [
                (1, vec![0x89, 0x21, 0x00, 0x39]),
                (2, vec![0x89, 0x11, 0x00, 0x79]),
                (4, vec![0x89, 0x09, 0x00, 0xb9]),
                (8, vec![0x89, 0x05, 0x00, 0xf9]),
            ] {
                let kind = SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size,
                };
                let encoded =
                    encode_aarch64_selected_memory_form(&physical, kind, alternative, &operands, 8)
                        .unwrap();
                assert_eq!(encoded.bytes(), expected);
                assert!(
                    check_memory(
                        &physical,
                        kind,
                        alternative,
                        &[operands[1], operands[0]],
                        8,
                        encoded.bytes()
                    )
                    .is_err()
                );
                for wrong_width in [1, 2, 4, 8] {
                    if wrong_width != byte_size {
                        assert!(
                            check_memory(
                                &physical,
                                SelectedInstructionKind::Store {
                                    byte_offset: 8,
                                    byte_size: wrong_width
                                },
                                alternative,
                                &operands,
                                8,
                                encoded.bytes()
                            )
                            .is_err()
                        );
                    }
                }
            }
            let address = encode_aarch64_selected_memory_form(
                &physical,
                SelectedInstructionKind::AddressOffset { byte_offset: 2 },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::AddressOffset,
                    variant: 0,
                },
                &operands,
                2,
            )
            .unwrap();
            assert_eq!(address.bytes(), [0x89, 0x09, 0x00, 0x91]);
            let wide_address = encode_aarch64_selected_memory_form(
                &physical,
                SelectedInstructionKind::AddressOffset { byte_offset: 16400 },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::AddressOffset,
                    variant: 0,
                },
                &operands,
                16400,
            )
            .unwrap();
            assert_eq!(
                wide_address.bytes(),
                [0x89, 0x11, 0x40, 0x91, 0x29, 0x41, 0x00, 0x91]
            );
            let paged_address = encode_aarch64_selected_memory_form(
                &physical,
                SelectedInstructionKind::AddressOffset {
                    byte_offset: 0x0100_0000,
                },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::AddressOffset,
                    variant: 0,
                },
                &operands,
                0x0100_0000,
            )
            .unwrap();
            assert_eq!(
                paged_address.bytes(),
                [0x917f_fd89_u32, 0x9140_0529]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            );
        }

        #[test]
        fn pointer_stores_and_offsets_replay_every_bit_and_operand() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            for base_name in ["x0", "x1", "x9", "x16", "x29", "x30"] {
                for value_name in ["x0", "x1", "x9", "x16", "x29", "x30"] {
                    let operands = [base_name, value_name]
                        .map(|name| physical.model().view_named(name).unwrap().id);
                    for byte_size in [1, 2, 4, 8] {
                        for byte_offset in [0, u32::from(byte_size), 4095 * u32::from(byte_size)] {
                            let kind = SelectedInstructionKind::Store {
                                byte_offset,
                                byte_size,
                            };
                            check_replay(
                                &physical,
                                kind,
                                MachineAlternativeFamily::Store,
                                &operands,
                                i64::from(byte_offset),
                            );
                        }
                    }
                    for byte_offset in [
                        0,
                        2,
                        4095,
                        4096,
                        16400,
                        0x00ff_ffff,
                        0x0100_0000,
                        33_554_431,
                        u32::MAX,
                    ] {
                        check_replay(
                            &physical,
                            SelectedInstructionKind::AddressOffset { byte_offset },
                            MachineAlternativeFamily::AddressOffset,
                            &operands,
                            i64::from(byte_offset),
                        );
                    }
                }
            }
        }

        fn check_replay(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            family: MachineAlternativeFamily,
            operands: &[RegisterViewId],
            displacement: i64,
        ) {
            let alternative = MachineAlternativeKey { family, variant: 0 };
            let encoded = encode_aarch64_selected_memory_form(
                physical,
                kind,
                alternative,
                operands,
                displacement,
            )
            .unwrap();
            let store = matches!(kind, SelectedInstructionKind::Store { .. });
            assert_eq!(
                encoded.footprint().encoded.external_operand_reads,
                if store { vec![0, 1] } else { vec![0] }
            );
            assert_eq!(
                encoded.footprint().encoded.external_operand_writes,
                if store { vec![] } else { vec![1] }
            );
            assert_eq!(
                encoded.footprint().encoded.memory,
                if store {
                    MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
                } else {
                    MachineEncodedMemoryEffect::NoneV1
                }
            );
            reject_every_bit_flip(encoded.bytes(), &format!("{kind:?}"), |corrupt| {
                check_memory(physical, kind, alternative, operands, displacement, corrupt).is_err()
            });
            for end in 0..encoded.bytes().len() {
                assert!(
                    check_memory(
                        physical,
                        kind,
                        alternative,
                        operands,
                        displacement,
                        &encoded.bytes()[..end]
                    )
                    .is_err()
                );
            }
            let mut trailing = encoded.bytes().to_vec();
            trailing.push(0);
            assert!(
                check_memory(
                    physical,
                    kind,
                    alternative,
                    operands,
                    displacement,
                    &trailing
                )
                .is_err()
            );
            assert!(
                check_memory(
                    physical,
                    kind,
                    alternative,
                    operands,
                    displacement + 1,
                    encoded.bytes()
                )
                .is_err()
            );
            assert!(
                encode_aarch64_selected_memory_form(
                    physical,
                    kind,
                    MachineAlternativeKey { family, variant: 1 },
                    operands,
                    displacement
                )
                .is_err()
            );
            assert!(
                encode_aarch64_selected_memory_form(
                    physical,
                    kind,
                    alternative,
                    &operands[..1],
                    displacement
                )
                .is_err()
            );
        }

        #[test]
        fn pointer_stores_reject_unsupported_widths_and_offsets() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let operands = ["x1", "x9"].map(|name| physical.model().view_named(name).unwrap().id);
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::Store,
                variant: 0,
            };
            for (byte_size, byte_offset) in [
                (0, 0),
                (3, 0),
                (16, 0),
                (8, u32::MAX),
                (2, 1),
                (4, 2),
                (8, 4),
                (1, 4096),
            ] {
                assert!(
                    encode_aarch64_selected_memory_form(
                        &physical,
                        SelectedInstructionKind::Store {
                            byte_offset,
                            byte_size
                        },
                        alternative,
                        &operands,
                        i64::from(byte_offset)
                    )
                    .is_err()
                );
            }
        }

        #[test]
        fn incoming_slot_address_and_pointer_load_replay_without_a_store() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let address_view = physical.model().view_named("x9").unwrap().id;
            let pointer_view = physical.model().view_named("x10").unwrap().id;
            let slot = selected_instructions::FrameStorageSlotId::Incoming {
                parameter_index: 8,
                abi_stack_byte_offset: 16,
            };
            let address_kind = SelectedInstructionKind::FrameAddress {
                slot,
                byte_offset: 0,
            };
            let address_key = MachineAlternativeKey {
                family: MachineAlternativeFamily::FrameAddress,
                variant: 0,
            };
            let encoded = encode_aarch64_selected_memory_form(
                &physical,
                address_kind,
                address_key,
                &[address_view],
                48,
            )
            .unwrap();
            check_memory(
                &physical,
                address_kind,
                address_key,
                &[address_view],
                48,
                encoded.bytes(),
            )
            .unwrap();
            assert!(
                check_memory(
                    &physical,
                    address_kind,
                    address_key,
                    &[address_view],
                    56,
                    encoded.bytes()
                )
                .is_err()
            );
            let load_kind = SelectedInstructionKind::Load64 { byte_offset: 0 };
            let load_key = MachineAlternativeKey {
                family: MachineAlternativeFamily::Load64,
                variant: 0,
            };
            let load = encode_aarch64_selected_memory_form(
                &physical,
                load_kind,
                load_key,
                &[address_view, pointer_view],
                0,
            )
            .unwrap();
            check_memory(
                &physical,
                load_kind,
                load_key,
                &[address_view, pointer_view],
                0,
                load.bytes(),
            )
            .unwrap();
            let store_kind = SelectedInstructionKind::Store64 {
                slot,
                byte_offset: 0,
            };
            let store_key = MachineAlternativeKey {
                family: MachineAlternativeFamily::Store64,
                variant: 0,
            };
            assert!(
                encode_aarch64_selected_memory_form(
                    &physical,
                    store_kind,
                    store_key,
                    &[pointer_view, address_view],
                    48
                )
                .is_err()
            );
            assert!(
                check_memory(
                    &physical,
                    store_kind,
                    store_key,
                    &[pointer_view, address_view],
                    48,
                    encoded.bytes()
                )
                .is_err()
            );
        }

        #[test]
        fn activation_base_encodes_only_the_unshifted_stack_pointer() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            let address = physical.model().view_named("x9").unwrap().id;
            let kind = SelectedInstructionKind::FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                byte_offset: 0,
            };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::FrameAddress,
                variant: 0,
            };
            let encoded =
                encode_aarch64_selected_memory_form(&physical, kind, alternative, &[address], 0)
                    .unwrap();
            assert_eq!(encoded.bytes(), [0xe9, 0x03, 0x00, 0x91]);
            check_memory(&physical, kind, alternative, &[address], 0, encoded.bytes()).unwrap();
            assert!(
                encode_aarch64_selected_memory_form(&physical, kind, alternative, &[address], 8)
                    .is_err()
            );
            assert!(
                check_memory(&physical, kind, alternative, &[address], 8, encoded.bytes()).is_err()
            );
            let shifted = SelectedInstructionKind::FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                byte_offset: 8,
            };
            assert!(
                encode_aarch64_selected_memory_form(&physical, shifted, alternative, &[address], 0)
                    .is_err()
            );
            let store = SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                byte_offset: 0,
            };
            let store_alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::Store64,
                variant: 0,
            };
            let value = physical.model().view_named("x10").unwrap().id;
            assert!(
                encode_aarch64_selected_memory_form(
                    &physical,
                    store,
                    store_alternative,
                    &[value, address],
                    0
                )
                .is_err()
            );
            for bit in 0..encoded.bytes().len() * 8 {
                let mut changed = encoded.bytes().to_vec();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    check_memory(&physical, kind, alternative, &[address], 0, &changed).is_err()
                );
            }
        }
    }
    pub fn encode_aarch64_selected_memory_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let (kind, alternative) = accessor_scalar_ordinary(kind, alternative)?;
        if matches!(
            kind,
            SelectedInstructionKind::SaveFloatingControl { .. }
                | SelectedInstructionKind::RestoreFloatingControl { .. }
        ) {
            return super::floating_control::encode_aarch64_selected_floating_control_form(
                physical,
                kind,
                alternative,
                operands,
                displacement,
            );
        }
        let displacement = u32::try_from(displacement)
            .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        if matches!(
            kind,
            SelectedInstructionKind::LoadPacked { .. }
                | SelectedInstructionKind::StorePacked { .. }
        ) {
            return packed::encode(physical, kind, alternative, operands, displacement);
        }
        if matches!(
            kind,
            SelectedInstructionKind::Store { .. } | SelectedInstructionKind::AddressOffset { .. }
        ) {
            return pointer::encode(physical, kind, alternative, operands, displacement);
        }
        if matches!(
            kind,
            SelectedInstructionKind::Store64 { .. } | SelectedInstructionKind::FrameAddress { .. }
        ) {
            return frame::encode(physical, kind, alternative, operands, displacement, 1);
        }
        if kind == SelectedInstructionKind::Load8Indexed {
            return indexed::encode(physical, alternative, operands, displacement);
        }
        let [base, destination] = request(physical, kind, alternative, operands, displacement)?;
        let width = load_width(kind)?;
        let opcode = match kind {
            SelectedInstructionKind::Load8 { .. } => 0x3940_0000,
            SelectedInstructionKind::Load16 { .. } => 0x7940_0000,
            SelectedInstructionKind::Load32 { .. } => 0xb940_0000,
            SelectedInstructionKind::Load64 { .. } => 0xf940_0000,
            _ => return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch),
        };
        let word = opcode
            | ((displacement / width) << 10)
            | (u32::from(base) << 5)
            | u32::from(destination);
        validate_aarch64_selected_memory_form(
            physical,
            kind,
            alternative,
            operands,
            i64::from(displacement),
            &word.to_le_bytes(),
        )
    }

    pub fn validate_aarch64_selected_memory_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let (kind, alternative) = accessor_scalar_ordinary(kind, alternative)?;
        if matches!(
            kind,
            SelectedInstructionKind::SaveFloatingControl { .. }
                | SelectedInstructionKind::RestoreFloatingControl { .. }
        ) {
            return super::floating_control::validate_aarch64_selected_floating_control_form(
                physical,
                kind,
                alternative,
                operands,
                displacement,
                bytes,
            );
        }
        let displacement = u32::try_from(displacement)
            .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        if matches!(
            kind,
            SelectedInstructionKind::LoadPacked { .. }
                | SelectedInstructionKind::StorePacked { .. }
        ) {
            return packed::validate(physical, kind, alternative, operands, displacement, bytes);
        }
        if matches!(
            kind,
            SelectedInstructionKind::Store { .. } | SelectedInstructionKind::AddressOffset { .. }
        ) {
            return pointer::validate(physical, kind, alternative, operands, displacement, bytes);
        }
        if matches!(
            kind,
            SelectedInstructionKind::Store64 { .. } | SelectedInstructionKind::FrameAddress { .. }
        ) {
            return frame::validate(
                physical,
                kind,
                alternative,
                operands,
                displacement,
                1,
                bytes,
            );
        }
        if kind == SelectedInstructionKind::Load8Indexed {
            return indexed::validate(physical, alternative, operands, displacement, bytes);
        }
        let [base, destination] = request(physical, kind, alternative, operands, displacement)?;
        let width = load_width(kind)?;
        let word = bytes
            .try_into()
            .ok()
            .map(u32::from_le_bytes)
            .filter(|word| {
                matches!(
                    (kind, word & 0xffc0_0000),
                    (SelectedInstructionKind::Load8 { .. }, 0x3940_0000)
                        | (SelectedInstructionKind::Load16 { .. }, 0x7940_0000)
                        | (SelectedInstructionKind::Load32 { .. }, 0xb940_0000)
                        | (SelectedInstructionKind::Load64 { .. }, 0xf940_0000)
                )
            })
            .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
        if (word & 31) != u32::from(destination)
            || ((word >> 5) & 31) != u32::from(base)
            || ((word >> 10) & 4095) * width != displacement
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedAarch64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: Aarch64SelectedFormFootprint {
                register_reads: vec![operands[0]],
                register_writes: vec![operands[1]],
                writes_nzcv: false,
                encoded: MachineEncodedEffects {
                    external_operand_reads: vec![0],
                    external_operand_writes: vec![1],
                    implicit_unit_uses: Vec::new(),
                    implicit_unit_defs: Vec::new(),
                    implicit_unit_clobbers: Vec::new(),
                    memory: MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: width as u16,
                    },
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::FallThroughV1,
                },
            },
        })
    }

    fn accessor_scalar_ordinary(
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
    ) -> Result<(SelectedInstructionKind, MachineAlternativeKey), Aarch64SelectedFormEncodingError>
    {
        let SelectedInstructionKind::AccessorScalarAccess {
            form, byte_offset, ..
        } = kind
        else {
            return Ok((kind, alternative));
        };
        if alternative.family != MachineAlternativeFamily::AccessorScalarAccess(form)
            || alternative.variant != 0
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let ordinary = form
            .ordinary_kind(byte_offset)
            .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        let semantic = form
            .ordinary_semantic()
            .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        Ok((
            ordinary,
            MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            },
        ))
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<[u8; 2], Aarch64SelectedFormEncodingError> {
        let width = load_width(kind)?;
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || !matches!(kind, selected_instructions::load_width_triple_pattern!(byte_offset, ..) | SelectedInstructionKind::Load64 { byte_offset } if byte_offset == displacement)
            || alternative
                != (MachineAlternativeKey {
                    family: match kind {
                        SelectedInstructionKind::Load8 { .. } => MachineAlternativeFamily::Load8,
                        SelectedInstructionKind::Load16 { .. } => MachineAlternativeFamily::Load16,
                        SelectedInstructionKind::Load32 { .. } => MachineAlternativeFamily::Load32,
                        SelectedInstructionKind::Load64 { .. } => MachineAlternativeFamily::Load64,
                        _ => return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch),
                    },
                    variant: 0,
                })
            || !displacement.is_multiple_of(width)
            || displacement / width > 4095
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        resolve_registers(physical, operands)?
            .try_into()
            .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    }

    fn load_width(kind: SelectedInstructionKind) -> Result<u32, Aarch64SelectedFormEncodingError> {
        match kind {
            SelectedInstructionKind::Load8 { .. } => Ok(1),
            SelectedInstructionKind::Load16 { .. } => Ok(2),
            SelectedInstructionKind::Load32 { .. } => Ok(4),
            SelectedInstructionKind::Load64 { .. } => Ok(8),
            _ => Err(Aarch64SelectedFormEncodingError::AlternativeMismatch),
        }
    }

    #[cfg(test)]
    fn check_memory(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        validate_aarch64_selected_memory_form(
            physical,
            kind,
            alternative,
            operands,
            displacement,
            bytes,
        )
    }

    #[cfg(test)]
    mod tests {
        use super::{check_memory, encode_aarch64_selected_memory_form};
        use crate::aarch64_physical_register_model;
        use selected_instructions::MachineAlternativeFamily;
        use selected_instructions::MachineAlternativeKey;
        use selected_instructions::SelectedInstructionKind;

        #[test]
        fn pointer_load_replays_opcode_registers_and_scaled_offset() {
            let physical =
                register_model::validate_physical_register_model(aarch64_physical_register_model())
                    .unwrap();
            let operands = [
                physical.model().view_named("x1").unwrap().id,
                physical.model().view_named("x2").unwrap().id,
            ];
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::Load64,
                variant: 0,
            };
            for byte_offset in [0, 8, 32760] {
                let kind = SelectedInstructionKind::Load64 { byte_offset };
                let displacement = i64::from(byte_offset);
                let encoded = encode_aarch64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), 4);
                for byte in 0..4 {
                    let mut corrupt = encoded.bytes().to_vec();
                    corrupt[byte] ^= 1;
                    assert!(
                        check_memory(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            displacement,
                            &corrupt
                        )
                        .is_err()
                    );
                }
                assert!(
                    check_memory(
                        &physical,
                        kind,
                        alternative,
                        &[operands[1], operands[0]],
                        displacement,
                        encoded.bytes()
                    )
                    .is_err()
                );
            }
            for byte_offset in [1, 32768] {
                assert!(
                    encode_aarch64_selected_memory_form(
                        &physical,
                        SelectedInstructionKind::Load64 { byte_offset },
                        alternative,
                        &operands,
                        i64::from(byte_offset)
                    )
                    .is_err()
                );
            }
        }
    }
}
pub(crate) mod movn_materialization {
    pub(crate) fn append_canonical_materialization(words: &mut Vec<u32>, register: u8, value: u64) {
        let chunks =
            std::array::from_fn::<_, 4, _>(|index| ((value >> (index * 16)) & 0xffff) as u16);
        words.push(0xd280_0000 | (u32::from(chunks[0]) << 5) | u32::from(register));
        for (index, chunk) in chunks.into_iter().enumerate().skip(1) {
            if chunk != 0 {
                words.push(
                    0xf280_0000
                        | (u32::try_from(index).expect("halfword index fits u32") << 21)
                        | (u32::from(chunk) << 5)
                        | u32::from(register),
                );
            }
        }
    }
}
mod normalized_foreign_call {
    use normalized_foreign_call_plan::{Fixup, Template};
    use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
        SelectedNormalizedForeignCall,
    };
    use semantic_vocabulary::BoundaryMachineId;
    use target::NativeTarget;

    use crate::codec_bytes::push_encoded;
    use crate::machine_effects::aarch64_selected_abi;

    pub(crate) const AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH: u8 = 4;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64NormalizedForeignCallFixupKind {
        BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64NormalizedForeignCallFixupState {
        UnresolvedImportFieldV1,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Aarch64CallbackMaterializationDestination {
        Register(calling_conventions::MachineRegister),
        RecordField {
            root_register: calling_conventions::MachineRegister,
            placements: calling_conventions::PrivateCallbackPlacements,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Aarch64CallbackAddressMaterialization {
        pub destination: Aarch64CallbackMaterializationDestination,
        pub materialization_byte_offset: u16,
        pub materialization_byte_count: u16,
        pub page_relocation_byte_offset: u16,
        pub page_offset_relocation_byte_offset: u16,
    }

    pub type Aarch64NormalizedForeignCallFixup = Fixup<
        Aarch64NormalizedForeignCallFixupKind,
        Aarch64NormalizedForeignCallFixupState,
        Aarch64CallbackAddressMaterialization,
    >;

    pub type ValidatedAarch64SelectedNormalizedForeignCallTemplate =
        Template<Aarch64NormalizedForeignCallFixup>;

    pub type Aarch64NormalizedForeignCallTemplateError = normalized_foreign_call_plan::Error;

    pub fn encode_aarch64_selected_normalized_foreign_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        effects: &MachineEncodedEffects,
        callback_destinations: &[Aarch64CallbackMaterializationDestination],
        record: &SelectedNormalizedForeignCall,
    ) -> Result<
        ValidatedAarch64SelectedNormalizedForeignCallTemplate,
        Aarch64NormalizedForeignCallTemplateError,
    > {
        let (boundary, ordinal) = match kind {
            SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => {
                (boundary, ordinal)
            }
            _ => return Err(Aarch64NormalizedForeignCallTemplateError::InstructionKindMismatch),
        };
        let bytes = canonical_call_bytes(callback_destinations)
            .ok_or(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)?;
        let fixup = callback_fixup(boundary, ordinal, callback_destinations)
            .ok_or(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)?;
        validate_aarch64_selected_normalized_foreign_call_template(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            operand_accesses,
            effects,
            &bytes,
            fixup,
            callback_destinations,
            record,
        )
    }

    pub fn validate_aarch64_selected_normalized_foreign_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        effects: &MachineEncodedEffects,
        bytes: &[u8],
        fixup: Aarch64NormalizedForeignCallFixup,
        callback_destinations: &[Aarch64CallbackMaterializationDestination],
        record: &SelectedNormalizedForeignCall,
    ) -> Result<
        ValidatedAarch64SelectedNormalizedForeignCallTemplate,
        Aarch64NormalizedForeignCallTemplateError,
    > {
        let (boundary, ordinal) = normalized_foreign_call_plan::admit(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            operand_accesses,
            effects,
            record,
            |native| aarch64_selected_abi(native).is_ok(),
            crate::canonical_aarch64_physical_register_model_identity,
            crate::machine_effects::normalized_foreign_constraint,
            crate::aarch64_normalized_foreign_call_declaration,
        )?;
        let destinations = record
            .call
            .callbacks
            .iter()
            .map(|callback| {
                let [calling_conventions::ValueLocation::Register { register, .. }] =
                    callback.application.placement.locations.as_slice()
                else {
                    return None;
                };
                match callback.destination.clone() {
                    calling_conventions::NativePlace::Parameter(_) => Some(
                        Aarch64CallbackMaterializationDestination::Register(*register),
                    ),
                    calling_conventions::NativePlace::Field { .. } => {
                        Some(Aarch64CallbackMaterializationDestination::RecordField {
                            root_register: *register,
                            placements: callback.field_placements.clone()?,
                        })
                    }
                }
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
        if destinations != callback_destinations {
            return Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch);
        }
        let expected_bytes = canonical_call_bytes(callback_destinations)
            .ok_or(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate)?;
        if bytes != expected_bytes.as_slice() {
            return Err(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate);
        }
        let expected_fixup = callback_fixup(boundary, ordinal, callback_destinations)
            .ok_or(Aarch64NormalizedForeignCallTemplateError::FixupMismatch)?;
        if fixup != expected_fixup {
            return Err(Aarch64NormalizedForeignCallTemplateError::FixupMismatch);
        }
        Ok(Template::new(
            kind,
            alternative,
            operand_views,
            effects,
            bytes,
            fixup,
        ))
    }

    fn canonical_call_bytes(
        destinations: &[Aarch64CallbackMaterializationDestination],
    ) -> Option<Vec<u8>> {
        let mut bytes = Vec::new();
        for destination in destinations {
            bytes.extend(callback_group_bytes(destination)?);
        }
        push_encoded(&mut bytes, 0x9400_0000_u32.to_le_bytes());
        Some(bytes)
    }

    pub fn callback_group_bytes(
        destination: &Aarch64CallbackMaterializationDestination,
    ) -> Option<Vec<u8>> {
        let mut bytes = Vec::new();
        let register = match destination {
            Aarch64CallbackMaterializationDestination::Register(register) => *register,
            Aarch64CallbackMaterializationDestination::RecordField { .. } => {
                calling_conventions::MachineRegister::Aarch64X(16)
            }
        };
        let calling_conventions::MachineRegister::Aarch64X(index) = register else {
            return None;
        };
        if index > 30 {
            return None;
        }
        let index = u32::from(index);
        push_encoded(&mut bytes, (0x9000_0000_u32 | index).to_le_bytes());
        push_encoded(
            &mut bytes,
            (0x9100_0000_u32 | (index << 5) | index).to_le_bytes(),
        );
        if let Aarch64CallbackMaterializationDestination::RecordField {
            root_register,
            placements,
        } = destination
        {
            let calling_conventions::MachineRegister::Aarch64X(root) = *root_register else {
                return None;
            };
            if placements.as_slice().is_empty() {
                return None;
            }
            for placement in placements.as_slice() {
                crate::post_handoff_writer::append_fragment(
                    &mut bytes, 16, root, 15, 17, *placement,
                )?;
            }
        }
        Some(bytes)
    }

    fn callback_fixup(
        boundary: BoundaryMachineId,
        ordinal: u32,
        destinations: &[Aarch64CallbackMaterializationDestination],
    ) -> Option<Aarch64NormalizedForeignCallFixup> {
        let mut materializations = Vec::with_capacity(destinations.len());
        let mut offset = 0_u16;
        for destination in destinations {
            let byte_count = u16::try_from(callback_group_bytes(destination)?.len()).ok()?;
            materializations.push(Aarch64CallbackAddressMaterialization {
                destination: destination.clone(),
                materialization_byte_offset: offset,
                materialization_byte_count: byte_count,
                page_relocation_byte_offset: offset,
                page_offset_relocation_byte_offset: offset + 4,
            });
            offset = offset.checked_add(byte_count)?;
        }
        Some(Aarch64NormalizedForeignCallFixup {
        kind: Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        state: Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_byte_offset: offset,
        patch_byte_offset: offset,
        reference_byte_offset: offset,
        patch_byte_width: AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH,
        callbacks: materializations,
    })
    }

    #[cfg(test)]
    mod tests {
        use super::{
            callback_fixup, encode_aarch64_selected_normalized_foreign_call_template,
            validate_aarch64_selected_normalized_foreign_call_template,
        };
        use normalized_foreign_call_plan::fixture;
        use register_model::validate_physical_register_model;
        use selected_instructions::{
            MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
            MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedNormalizedForeignCall,
        };
        use target::NativeTarget;

        use crate::aarch64_physical_register_model;

        fn callback_destination(
            placement: fixture::CallbackPlacement,
        ) -> super::Aarch64CallbackMaterializationDestination {
            match placement.field_byte_offset {
                None => {
                    super::Aarch64CallbackMaterializationDestination::Register(placement.register)
                }
                Some(byte_offset) => {
                    super::Aarch64CallbackMaterializationDestination::RecordField {
                        root_register: placement.register,
                        placements: calling_conventions::PrivateCallbackPlacements::new(&[
                            layout_plans::LayoutPlacementReport::At {
                                offset: u64::try_from(byte_offset).unwrap(),
                            },
                        ]),
                    }
                }
            }
        }

        const CALLBACK_BYTE_COUNTS: fixture::CallbackByteCounts = fixture::CallbackByteCounts {
            register: 8,
            field: 16,
        };

        fn inputs(darwin: bool, input_layout: usize, result: usize) -> fixture::Case {
            fixture::case(
                validate_physical_register_model(aarch64_physical_register_model()).unwrap(),
                record(darwin, input_layout, result),
                crate::machine_effects::normalized_foreign_constraint,
                |row, _| MachineEncodedEffects {
                    external_operand_reads: row.operand_reads(),
                    external_operand_writes: row.operand_writes(),
                    implicit_unit_uses: row.implicit_uses.clone(),
                    implicit_unit_defs: row.implicit_defs.clone(),
                    implicit_unit_clobbers: row.clobbers.clone(),
                    memory: MachineEncodedMemoryEffect::NoneV1,
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::DirectRelativeCallV1,
                },
            )
        }

        #[test]
        fn table_calls_retain_full_offsets_and_independently_reject_changed_bytes() {
            use super::super::table_call::{
                encode_aarch64_selected_table_call_form, validate_aarch64_selected_table_call_form,
            };
            for byte_offset in [7, 65_536, u32::MAX] {
                let (_, _, alternative, operands, accesses, _, mut record) = inputs(false, 1, 1);
                let contribution = record
                    .call
                    .binding
                    .same_stack_contribution()
                    .unwrap()
                    .clone();
                record.call.binding.mechanism =
                    target_operations::NormalizedForeignCallMechanism::Table {
                        target: target::TargetProfile::LinuxArm64,
                        byte_offset,
                        dispatch_only: false,
                        same_stack_contribution: contribution,
                    };
                let encoded = encode_aarch64_selected_table_call_form(
                    &record,
                    alternative,
                    &operands,
                    &accesses,
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), 20);
                assert!(matches!(encoded.footprint().encoded.memory,
                MachineEncodedMemoryEffect::ReadTableFieldV1 { byte_offset: retained, .. } if retained == byte_offset));
                assert_eq!(
                    encoded.footprint().encoded.control,
                    MachineEncodedControlEffect::IndirectExternalCallV1
                );
                for position in 0..encoded.bytes().len() {
                    let mut changed = encoded.bytes().to_vec();
                    changed[position] ^= 1;
                    assert!(
                        validate_aarch64_selected_table_call_form(
                            &record,
                            alternative,
                            &operands,
                            &accesses,
                            &changed
                        )
                        .is_err()
                    );
                }
            }
        }

        #[test]
        fn table_dispatch_only_preserves_wire_plan_and_refuses_ip1_aliases() {
            use super::super::table_call::encode_aarch64_selected_table_call_form;
            let (_, _, alternative, _, _, _, mut record) = inputs(true, 1, 1);
            let original = record.call.binding.boundary_entry_plan.clone();
            let contribution = record
                .call
                .binding
                .same_stack_contribution()
                .unwrap()
                .clone();
            record.call.binding.mechanism =
                target_operations::NormalizedForeignCallMechanism::Table {
                    target: target::TargetProfile::MacosArm64,
                    byte_offset: 8,
                    dispatch_only: true,
                    same_stack_contribution: contribution,
                };
            let constraint =
                crate::machine_effects::normalized_foreign_constraint(&record).unwrap();
            let views = constraint
                .operands
                .iter()
                .map(|operand| operand.fixed_view.unwrap())
                .collect::<Vec<_>>();
            let accesses = constraint
                .operands
                .iter()
                .map(|operand| operand.access)
                .collect::<Vec<_>>();
            let encoded =
                encode_aarch64_selected_table_call_form(&record, alternative, &views, &accesses)
                    .unwrap();
            assert_eq!(&encoded.bytes()[8..12], &0x8b11_0211_u32.to_le_bytes());
            assert_eq!(record.call.binding.boundary_entry_plan, original);
            let calling_conventions::ValueLocation::Register { register, .. } =
                &mut record.call.binding.boundary_entry_plan.call.parameters[0].locations[0]
            else {
                panic!("register argument");
            };
            *register = calling_conventions::MachineRegister::Aarch64X(17);
            assert!(
                encode_aarch64_selected_table_call_form(&record, alternative, &views, &accesses)
                    .is_err()
            );
            let calling_conventions::ValueLocation::Register { register, .. } =
                &mut record.call.binding.boundary_entry_plan.call.parameters[0].locations[0]
            else {
                panic!("register argument");
            };
            *register = calling_conventions::MachineRegister::Aarch64X(16);
            assert!(
                encode_aarch64_selected_table_call_form(&record, alternative, &views, &accesses)
                    .is_err()
            );
        }

        fn record(
            darwin: bool,
            input_layout: usize,
            result: usize,
        ) -> SelectedNormalizedForeignCall {
            use calling_conventions::ValueShape;
            use target::TargetProfile;

            fixture::record(
                if darwin {
                    NativeTarget::macos_arm64()
                } else {
                    NativeTarget::linux_arm64()
                },
                9,
                ValueShape::homogeneous_float_aggregate(8, 2),
                if darwin {
                    fixture::mach_o_locator()
                } else {
                    fixture::elf_locator(TargetProfile::LinuxArm64)
                },
                input_layout,
                result,
            )
        }

        type Entrances = fixture::Entrances<
            super::Aarch64CallbackMaterializationDestination,
            super::Aarch64NormalizedForeignCallFixupKind,
            super::Aarch64NormalizedForeignCallFixupState,
            super::Aarch64CallbackAddressMaterialization,
        >;

        fn entrances() -> Entrances {
            fixture::Entrances {
                call: fixture::CallBytes {
                    bytes: 0x9400_0000_u32.to_le_bytes().to_vec(),
                    patch_byte_offset: 0,
                    reference_byte_offset: 0,
                },
                constraint: crate::machine_effects::normalized_foreign_constraint,
                declaration: crate::aarch64_normalized_foreign_call_declaration,
                fixed_view: crate::aarch64_fixed_register_view,
                encode: encode_aarch64_selected_normalized_foreign_call_template,
                validate: validate_aarch64_selected_normalized_foreign_call_template,
                fixup: |boundary, ordinal, destinations| {
                    callback_fixup(boundary, ordinal, destinations)
                },
                fixup_row: |callback| {
                    (
                        callback.destination.clone(),
                        callback.materialization_byte_offset,
                        callback.materialization_byte_count,
                    )
                },
                advance: |callback| callback.materialization_byte_offset += 1,
            }
        }

        #[test]
        fn retained_multiple_callbacks_bind_operands_effects_bytes_and_fixups() {
            fixture::retained_callbacks(
                &entrances(),
                inputs,
                callback_destination,
                CALLBACK_BYTE_COUNTS,
            );
        }

        #[test]
        fn direct_and_hidden_aggregate_results_share_the_import_template() {
            fixture::direct_and_hidden_aggregate_results_share_the_import_template(
                &entrances(),
                inputs,
            );
        }

        #[test]
        fn template_has_exact_bytes_and_fixup() {
            fixture::template_has_exact_bytes_and_fixup(&entrances(), inputs);
        }

        #[test]
        fn evaluated_plans_select_exact_operands_and_effects_under_one_template() {
            fixture::evaluated_plans_select_exact_operands_and_effects_under_one_template(
                &entrances(),
                inputs,
                0..81,
                "x8",
            );
        }

        #[test]
        fn rejects_wrong_kind_target_alternative_and_fixup() {
            fixture::rejects_wrong_kind_target_alternative_and_fixup(
                &entrances(),
                inputs,
                &[
                    NativeTarget::linux_x64(),
                    NativeTarget::windows_x64(),
                    NativeTarget {
                        object_format: target::ObjectFormat::Coff,
                        ..NativeTarget::macos_arm64()
                    },
                ],
                &0x1400_0000_u32.to_le_bytes(),
            );
        }
    }
}
pub use normalized_foreign_call::callback_group_bytes;
mod scalar_call {
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, SelectedInstructionKind,
    };
    use semantic_vocabulary::MachineId;
    use target::NativeTarget;

    use crate::machine_effects::{Aarch64SelectedAbi, aarch64_selected_abi};
    use crate::{aarch64_aapcs64_register_call_keys, aarch64_register_constraint_catalog_for};

    pub(crate) const AARCH64_SCALAR_CALL_TEMPLATE_BYTE_COUNT: usize = 4;
    pub(crate) const AARCH64_SCALAR_CALL_OPCODE_OFFSET: u16 = 0;
    pub(crate) const AARCH64_SCALAR_CALL_PATCH_OFFSET: u16 = 0;
    pub(crate) const AARCH64_SCALAR_CALL_REFERENCE_OFFSET: u16 = 0;
    pub(crate) const AARCH64_SCALAR_CALL_PATCH_WIDTH: u8 = 4;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64ScalarCallFixupKind {
        SignedImmediate26WordsFromInstructionToInternalMachineV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64ScalarCallFixupState {
        UnresolvedZeroImmediateV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Aarch64ScalarCallFixup {
        pub kind: Aarch64ScalarCallFixupKind,
        pub state: Aarch64ScalarCallFixupState,
        pub callee: MachineId,
        pub opcode_byte_offset: u16,
        pub patch_byte_offset: u16,
        pub reference_byte_offset: u16,
        pub patch_byte_width: u8,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedAarch64SelectedScalarCallTemplate {
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: Vec<RegisterViewId>,
        effects: MachineEncodedEffects,
        bytes: [u8; AARCH64_SCALAR_CALL_TEMPLATE_BYTE_COUNT],
        fixup: Aarch64ScalarCallFixup,
    }

    impl ValidatedAarch64SelectedScalarCallTemplate {
        pub const fn kind(&self) -> SelectedInstructionKind {
            self.kind
        }

        pub const fn alternative(&self) -> MachineAlternativeKey {
            self.alternative
        }

        pub fn operand_views(&self) -> &[RegisterViewId] {
            &self.operand_views
        }

        pub const fn effects(&self) -> &MachineEncodedEffects {
            &self.effects
        }

        pub const fn bytes(&self) -> &[u8; AARCH64_SCALAR_CALL_TEMPLATE_BYTE_COUNT] {
            &self.bytes
        }

        pub const fn fixup(&self) -> Aarch64ScalarCallFixup {
            self.fixup
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64ScalarCallTemplateError {
        UnsupportedTarget,
        NonCanonicalPhysicalModel,
        InstructionKindMismatch,
        AlternativeMismatch,
        OperandViewMismatch,
        EffectMismatch,
        MalformedTemplate,
        FixupMismatch,
    }

    impl std::fmt::Display for Aarch64ScalarCallTemplateError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "invalid AArch64 scalar-call template: {self:?}")
        }
    }

    impl std::error::Error for Aarch64ScalarCallTemplateError {}

    pub fn encode_aarch64_selected_scalar_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
    ) -> Result<ValidatedAarch64SelectedScalarCallTemplate, Aarch64ScalarCallTemplateError> {
        let callee = kind
            .machine_callee()
            .ok_or(Aarch64ScalarCallTemplateError::InstructionKindMismatch)?;
        let bytes = crate::ORDINARY_DIRECT_CALL_BYTES;
        let fixup = canonical_fixup(callee);
        validate_aarch64_selected_scalar_call_template(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            effects,
            &bytes,
            fixup,
        )
    }

    pub fn validate_aarch64_selected_scalar_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
        bytes: &[u8],
        fixup: Aarch64ScalarCallFixup,
    ) -> Result<ValidatedAarch64SelectedScalarCallTemplate, Aarch64ScalarCallTemplateError> {
        let abi = aarch64_selected_abi(target)
            .map_err(|_| Aarch64ScalarCallTemplateError::UnsupportedTarget)?;
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Aarch64ScalarCallTemplateError::NonCanonicalPhysicalModel);
        }
        let callee = kind
            .machine_callee()
            .ok_or(Aarch64ScalarCallTemplateError::InstructionKindMismatch)?;
        if alternative != kind.call_alternative_key() {
            return Err(Aarch64ScalarCallTemplateError::AlternativeMismatch);
        }
        let (expected_operand_views, expected) =
            if matches!(kind, SelectedInstructionKind::InvokeNoNormal { .. }) {
                expected_no_normal_mechanics(abi, physical, operand_views, effects)?
            } else {
                expected_call_mechanics(abi, physical, kind, operand_views)?
            };
        if effects != &expected {
            return Err(Aarch64ScalarCallTemplateError::EffectMismatch);
        }
        let bytes: [u8; AARCH64_SCALAR_CALL_TEMPLATE_BYTE_COUNT] = bytes
            .try_into()
            .map_err(|_| Aarch64ScalarCallTemplateError::MalformedTemplate)?;
        if u32::from_le_bytes(bytes) != 0x9400_0000 {
            return Err(Aarch64ScalarCallTemplateError::MalformedTemplate);
        }
        if fixup != canonical_fixup(callee) {
            return Err(Aarch64ScalarCallTemplateError::FixupMismatch);
        }
        Ok(ValidatedAarch64SelectedScalarCallTemplate {
            kind,
            alternative,
            operand_views: expected_operand_views,
            effects: effects.clone(),
            bytes,
            fixup,
        })
    }

    fn canonical_fixup(callee: MachineId) -> Aarch64ScalarCallFixup {
        Aarch64ScalarCallFixup {
            kind:
                Aarch64ScalarCallFixupKind::SignedImmediate26WordsFromInstructionToInternalMachineV1,
            state: Aarch64ScalarCallFixupState::UnresolvedZeroImmediateV1,
            callee,
            opcode_byte_offset: AARCH64_SCALAR_CALL_OPCODE_OFFSET,
            patch_byte_offset: AARCH64_SCALAR_CALL_PATCH_OFFSET,
            reference_byte_offset: AARCH64_SCALAR_CALL_REFERENCE_OFFSET,
            patch_byte_width: AARCH64_SCALAR_CALL_PATCH_WIDTH,
        }
    }

    fn expected_operand_views(
        physical: &ValidatedPhysicalRegisterModel,
        arity: usize,
    ) -> Vec<RegisterViewId> {
        ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
            .into_iter()
            .take(arity)
            .chain(["x0"])
            .map(|name| {
                physical
                    .model()
                    .view_named(name)
                    .expect("canonical ABI register")
                    .id
            })
            .collect()
    }

    fn expected_effects(
        abi: Aarch64SelectedAbi,
        physical: &ValidatedPhysicalRegisterModel,
        arity: usize,
    ) -> MachineEncodedEffects {
        let catalog = aarch64_register_constraint_catalog_for(physical);
        let row = catalog
            .constraints
            .iter()
            .find(|row| {
                row.key
                    == match abi {
                        Aarch64SelectedAbi::Aapcs64 => aarch64_aapcs64_register_call_keys()[arity],
                        Aarch64SelectedAbi::Darwin => {
                            crate::aarch64_darwin_register_call_keys()[arity]
                        }
                    }
            })
            .expect("canonical AArch64 catalog contains scalar-call constraint");
        MachineEncodedEffects {
            external_operand_reads: (0..arity as u16).collect(),
            external_operand_writes: vec![arity as u16],
            implicit_unit_uses: row.implicit_uses.clone(),
            implicit_unit_defs: row.implicit_defs.clone(),
            implicit_unit_clobbers: row.clobbers.clone(),
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::DirectRelativeCallV1,
        }
    }

    #[cfg(test)]
    mod mixed_calls {
        use super::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, MachineId,
            NativeTarget, RegisterViewId, SelectedInstructionKind,
            ValidatedAarch64SelectedScalarCallTemplate, aarch64_selected_abi, canonical_fixup,
            encode_aarch64_selected_scalar_call_template, expected_effects,
        };
        use crate::{aarch64_physical_register_model, aarch64_register_constraint_catalog};
        use register_model::validate_physical_register_model;
        use selected_instructions::fixture;

        impl fixture::CallTemplate for ValidatedAarch64SelectedScalarCallTemplate {
            fn bound_operands(&self) -> &[RegisterViewId] {
                self.operand_views()
            }

            fn names_callee(&self, callee: MachineId) -> bool {
                self.fixup() == canonical_fixup(callee)
            }
        }

        macro_rules! a {
            ($cond:expr => $msg:expr) => {
                assert!($cond, $msg);
            };
        }

        #[test]
        fn mixed_aggregate_call_templates_retain_integer_results_and_ieee_inputs() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let catalog = aarch64_register_constraint_catalog(&physical);
            let kind = SelectedInstructionKind::CallAggregate {
                target: selected_instructions::SelectedCallTarget::Machine(
                    MachineId::new(7).unwrap(),
                ),
            };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::CallAggregate,
                variant: 0,
            };
            for (target, unit_keys) in [
                (
                    NativeTarget::linux_arm64(),
                    crate::aarch64_aapcs64_mixed_unit_call_keys(),
                ),
                (
                    NativeTarget::macos_arm64(),
                    crate::aarch64_darwin_mixed_unit_call_keys(),
                ),
            ] {
                for ordinal in [0, 7, 8, 71, 72, 79, 80, 143] {
                    let fragments = ordinal / 72 + 1;
                    let general_count = ordinal % 72 / 8;
                    let float_count = ordinal % 8 + 1;
                    let arity = general_count + float_count;
                    let operands = (0..general_count)
                        .map(|position| format!("x{position}"))
                        .chain((0..float_count).map(|position| format!("d{position}")))
                        .chain((0..fragments).map(|position| format!("x{position}")))
                        .map(|name| physical.model().view_named(&name).unwrap().id)
                        .collect::<Vec<_>>();
                    let unit = catalog.constraint(unit_keys[ordinal % 72]).unwrap();
                    let mut effects =
                        expected_effects(aarch64_selected_abi(target).unwrap(), &physical, 0);
                    effects.external_operand_reads = (0..arity as u16).collect();
                    effects.external_operand_writes =
                        (arity as u16..(arity + fragments) as u16).collect();
                    effects.implicit_unit_uses = unit.implicit_uses.clone();
                    effects.implicit_unit_defs = unit.implicit_defs.clone();
                    effects.implicit_unit_clobbers = unit.clobbers.clone();
                    for name in ["x0", "x1"].into_iter().take(fragments) {
                        effects.implicit_unit_clobbers.retain(|unit| {
                            !physical
                                .model()
                                .view_named(name)
                                .unwrap()
                                .write_units
                                .contains(unit)
                        });
                    }
                    let encode = |operands: &[RegisterViewId], effects: &MachineEncodedEffects| {
                        encode_aarch64_selected_scalar_call_template(
                            target,
                            &physical,
                            kind,
                            alternative,
                            operands,
                            effects,
                        )
                    };
                    let template = encode(&operands, &effects).expect("mixed aggregate template");
                    assert_eq!(template.operand_views(), operands);
                    assert_eq!(
                        template.fixup(),
                        canonical_fixup(MachineId::new(7).unwrap())
                    );
                    let mut changed = operands.clone();
                    changed[arity] = physical.model().view_named("d0").unwrap().id;
                    a!(encode(&changed, &effects).is_err() => "result bank substitution");
                    let mut changed = operands.clone();
                    changed[general_count] = physical.model().view_named("x2").unwrap().id;
                    a!(encode(&changed, &effects).is_err() => "IEEE input bank substitution");
                    assert!(
                        encode(&operands[..operands.len() - 1], &effects).is_err(),
                        "missing result definition"
                    );
                    let mut changed = effects.clone();
                    changed.external_operand_writes.clear();
                    a!(encode(&operands, &changed).is_err() => "missing result effect");
                    let mut changed = effects.clone();
                    changed.implicit_unit_clobbers = unit.clobbers.clone();
                    a!(encode(&operands, &changed).is_err() => "result cannot remain an unknown clobber");
                    let mut changed = effects.clone();
                    changed.implicit_unit_clobbers.retain(|unit| {
                        !physical
                            .model()
                            .view_named("x2")
                            .unwrap()
                            .write_units
                            .contains(unit)
                    });
                    a!(encode(&operands, &changed).is_err() => "other caller clobbers remain required");
                }
            }
        }

        #[test]
        fn every_mixed_unit_call_row_has_exact_template_operands_and_effects() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let catalog = aarch64_register_constraint_catalog(&physical);
            let callee = MachineId::new(7).unwrap();
            let stack_pointer = physical.model().view_named("sp").unwrap().id;
            for (target, keys) in [
                (
                    NativeTarget::linux_arm64(),
                    crate::aarch64_aapcs64_mixed_unit_call_keys(),
                ),
                (
                    NativeTarget::macos_arm64(),
                    crate::aarch64_darwin_mixed_unit_call_keys(),
                ),
            ] {
                let base = expected_effects(aarch64_selected_abi(target).unwrap(), &physical, 0);
                for key in keys {
                    fixture::assert_mixed_unit_call_row(
                        catalog.constraint(key).unwrap(),
                        base.clone(),
                        callee,
                        stack_pointer,
                        |kind, alternative, operands, effects| {
                            encode_aarch64_selected_scalar_call_template(
                                target,
                                &physical,
                                kind,
                                alternative,
                                operands,
                                effects,
                            )
                        },
                    );
                }
            }
        }
    }
    #[cfg(test)]
    mod target_abis {
        use super::{
            Aarch64ScalarCallTemplateError, MachineAlternativeFamily, MachineAlternativeKey,
            MachineId, NativeTarget, SelectedInstructionKind, aarch64_selected_abi,
            encode_aarch64_selected_scalar_call_template, expected_effects,
            validate_aarch64_selected_scalar_call_template,
        };
        use crate::{aarch64_physical_register_model, aarch64_register_constraint_catalog};
        use register_model::validate_physical_register_model;

        #[test]
        fn target_register_arities_encode_and_reject_opposite_abi_effects() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let constraints = crate::validate_aarch64_register_constraint_catalog(
                aarch64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap();
            assert!(
                crate::AARCH64_REQUIRED_REGISTER_CONSTRAINTS
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
            );
            let cases: [(NativeTarget, &[&str]); 2] = [
                (
                    NativeTarget::linux_arm64(),
                    &["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"],
                ),
                (
                    NativeTarget::macos_arm64(),
                    &["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"],
                ),
            ];
            let kind = SelectedInstructionKind::CallScalar {
                target: selected_instructions::SelectedCallTarget::Machine(
                    MachineId::new(7).unwrap(),
                ),
            };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::CallScalar,
                variant: 0,
            };
            for (target, arguments) in cases {
                let catalog = crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
                for arity in 0..=arguments.len() {
                    let operands = arguments[..arity]
                        .iter()
                        .copied()
                        .chain(["x0"])
                        .map(|name| physical.model().view_named(name).unwrap().id)
                        .collect::<Vec<_>>();
                    let effects = &catalog
                        .declarations
                        .iter()
                        .find(|row| {
                            row.semantic == selected_instructions::MachineSemanticKind::CallScalar
                                && row.alternatives[0].encoded.external_operand_reads.len() == arity
                        })
                        .unwrap()
                        .alternatives[0]
                        .encoded;
                    let template = encode_aarch64_selected_scalar_call_template(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        effects,
                    )
                    .unwrap();
                    assert_eq!(
                        validate_aarch64_selected_scalar_call_template(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            effects,
                            template.bytes(),
                            template.fixup(),
                        )
                        .unwrap(),
                        template
                    );
                    let opposite = if target == NativeTarget::linux_arm64() {
                        NativeTarget::macos_arm64()
                    } else {
                        NativeTarget::linux_arm64()
                    };
                    let mut changed = effects.clone();
                    changed.implicit_unit_clobbers =
                        expected_effects(aarch64_selected_abi(opposite).unwrap(), &physical, 0)
                            .implicit_unit_clobbers;
                    assert_eq!(
                        validate_aarch64_selected_scalar_call_template(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &changed,
                            template.bytes(),
                            template.fixup(),
                        ),
                        Err(Aarch64ScalarCallTemplateError::EffectMismatch)
                    );
                    let mut changed = operands.clone();
                    *changed.last_mut().unwrap() = physical.model().view_named("sp").unwrap().id;
                    assert_eq!(
                        validate_aarch64_selected_scalar_call_template(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &changed,
                            effects,
                            template.bytes(),
                            template.fixup(),
                        ),
                        Err(Aarch64ScalarCallTemplateError::OperandViewMismatch)
                    );
                }
            }
        }
    }
    #[cfg(test)]
    mod unit_calls {
        use super::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineId, NativeTarget,
            SelectedInstructionKind, aarch64_selected_abi,
            encode_aarch64_selected_scalar_call_template, expected_effects, expected_operand_views,
            validate_aarch64_selected_scalar_call_template,
        };
        use crate::{aarch64_physical_register_model, aarch64_register_constraint_catalog};
        use register_model::{RegisterOperandAccess, validate_physical_register_model};

        #[test]
        fn unit_call_templates_cover_native_register_arities_and_reject_forged_results() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let catalog = aarch64_register_constraint_catalog(&physical);
            for (target, keys) in [
                (
                    NativeTarget::linux_arm64(),
                    crate::aarch64_aapcs64_register_unit_call_keys(),
                ),
                (
                    NativeTarget::macos_arm64(),
                    crate::aarch64_darwin_register_unit_call_keys(),
                ),
            ] {
                for (arity, key) in keys.iter().enumerate() {
                    let row = catalog.constraint(*key).unwrap();
                    assert_eq!(row.operands.len(), arity);
                    assert!(
                        row.operands
                            .iter()
                            .all(|operand| operand.access == RegisterOperandAccess::Use)
                    );
                    let mut missing_clobber = catalog.clone();
                    register_model::find_register_constraint_mut(
                        &mut missing_clobber.constraints,
                        *key,
                    )
                    .unwrap()
                    .clobbers
                    .clear();
                    assert!(
                        crate::validate_aarch64_register_constraint_catalog(
                            missing_clobber,
                            &physical
                        )
                        .is_err()
                    );
                    let mut operands = expected_operand_views(&physical, arity);
                    operands.pop();
                    let mut effects =
                        expected_effects(aarch64_selected_abi(target).unwrap(), &physical, arity);
                    effects.external_operand_writes.clear();
                    effects.implicit_unit_uses = row.implicit_uses.clone();
                    effects.implicit_unit_defs = row.implicit_defs.clone();
                    effects.implicit_unit_clobbers = row.clobbers.clone();
                    let kind = SelectedInstructionKind::CallUnit {
                        target: selected_instructions::SelectedCallTarget::Machine(
                            MachineId::new(7).unwrap(),
                        ),
                    };
                    let alternative = MachineAlternativeKey {
                        family: MachineAlternativeFamily::CallUnit,
                        variant: 0,
                    };
                    let template = encode_aarch64_selected_scalar_call_template(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &effects,
                    )
                    .unwrap();
                    assert_eq!(template.operand_views(), operands);
                    validate_aarch64_selected_scalar_call_template(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &effects,
                        template.bytes(),
                        template.fixup(),
                    )
                    .unwrap();
                    let mut forged = effects.clone();
                    forged.external_operand_writes.push(arity as u16);
                    assert!(
                        encode_aarch64_selected_scalar_call_template(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &forged
                        )
                        .is_err()
                    );
                    let wrong_family = MachineAlternativeKey {
                        family: MachineAlternativeFamily::CallScalar,
                        variant: 0,
                    };
                    assert!(
                        encode_aarch64_selected_scalar_call_template(
                            target,
                            &physical,
                            kind,
                            wrong_family,
                            &operands,
                            &effects
                        )
                        .is_err()
                    );
                    if !operands.is_empty() {
                        let mut wrong = operands.clone();
                        wrong[0] = physical.model().view_named("sp").unwrap().id;
                        assert!(
                            encode_aarch64_selected_scalar_call_template(
                                target,
                                &physical,
                                kind,
                                alternative,
                                &wrong,
                                &effects
                            )
                            .is_err()
                        );
                    }
                }
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::{
            Aarch64ScalarCallFixup, Aarch64ScalarCallTemplateError, Aarch64SelectedAbi,
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, MachineId,
            NativeTarget, RegisterViewId, SelectedInstructionKind, ValidatedPhysicalRegisterModel,
            aarch64_aapcs64_register_call_keys, aarch64_selected_abi, canonical_fixup,
            encode_aarch64_selected_scalar_call_template, expected_effects, expected_operand_views,
            validate_aarch64_selected_scalar_call_template,
        };
        use crate::aarch64_physical_register_model;
        use register_model::validate_physical_register_model;

        fn inputs() -> (
            ValidatedPhysicalRegisterModel,
            SelectedInstructionKind,
            MachineAlternativeKey,
            Vec<RegisterViewId>,
            MachineEncodedEffects,
        ) {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let kind = SelectedInstructionKind::CallScalar {
                target: selected_instructions::SelectedCallTarget::Machine(
                    MachineId::new(7).unwrap(),
                ),
            };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::CallScalar,
                variant: 0,
            };
            let operands = expected_operand_views(&physical, 2);
            let effects = expected_effects(Aarch64SelectedAbi::Aapcs64, &physical, 2);
            (physical, kind, alternative, operands, effects)
        }

        #[test]
        fn every_register_arity_has_exact_operands_and_effects() {
            let (physical, kind, alternative, _, _) = inputs();
            let catalog = crate::aarch64_register_constraint_catalog(&physical);
            for (arity, key) in aarch64_aapcs64_register_call_keys().into_iter().enumerate() {
                let row = catalog.constraint(key).unwrap();
                let operands = expected_operand_views(&physical, arity);
                assert_eq!(row.operands.len(), arity + 1);
                assert_eq!(
                    row.operands
                        .iter()
                        .map(|operand| operand.fixed_view.unwrap())
                        .collect::<Vec<_>>(),
                    operands
                );
                let effects = expected_effects(Aarch64SelectedAbi::Aapcs64, &physical, arity);
                assert_eq!(
                    effects.external_operand_reads,
                    (0..arity as u16).collect::<Vec<_>>()
                );
                assert_eq!(effects.external_operand_writes, vec![arity as u16]);
                let template = encode_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                )
                .unwrap();
                assert_eq!(template.operand_views(), operands.as_slice());
                let mut wrong_effects = effects.clone();
                wrong_effects.external_operand_writes = vec![(arity + 1) as u16];
                assert!(
                    encode_aarch64_selected_scalar_call_template(
                        NativeTarget::linux_arm64(),
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &wrong_effects,
                    )
                    .is_err()
                );
                let mut wrong_views = operands.clone();
                wrong_views[arity] = physical.model().view_named("sp").unwrap().id;
                assert!(
                    encode_aarch64_selected_scalar_call_template(
                        NativeTarget::linux_arm64(),
                        &physical,
                        kind,
                        alternative,
                        &wrong_views,
                        &effects,
                    )
                    .is_err()
                );
            }
            let oversized = vec![physical.model().view_named("x0").unwrap().id; 10];
            assert_eq!(
                encode_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &oversized,
                    &expected_effects(Aarch64SelectedAbi::Aapcs64, &physical, 0),
                ),
                Err(Aarch64ScalarCallTemplateError::OperandViewMismatch)
            );
        }

        #[test]
        fn scalar_call_abi_matrix_resolves_declared_pairs_and_fails_closed() {
            assert_eq!(
                aarch64_selected_abi(NativeTarget::linux_arm64()),
                Ok(Aarch64SelectedAbi::Aapcs64)
            );
            assert_eq!(
                aarch64_selected_abi(NativeTarget::macos_arm64()),
                Ok(Aarch64SelectedAbi::Darwin)
            );
            for undeclared in [
                NativeTarget {
                    object_format: target::ObjectFormat::Coff,
                    ..NativeTarget::linux_arm64()
                },
                NativeTarget::linux_x64(),
                NativeTarget::windows_x64(),
            ] {
                assert_eq!(
                aarch64_selected_abi(undeclared),
                Err(
                    crate::machine_effects::Aarch64MachineEffectCatalogValidationError::UnsupportedTargetAbi,
                )
            );
            }
        }

        #[test]
        fn scalar_call_template_has_exact_bytes_and_fixup() {
            let (physical, kind, alternative, operands, effects) = inputs();
            let template = encode_aarch64_selected_scalar_call_template(
                NativeTarget::linux_arm64(),
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
            )
            .unwrap();
            assert_eq!(template.bytes(), &0x9400_0000_u32.to_le_bytes());
            assert_eq!(template.kind(), kind);
            assert_eq!(template.alternative(), alternative);
            assert_eq!(template.operand_views(), &operands);
            assert_eq!(template.effects(), &effects);
            assert_eq!(
                template.fixup(),
                canonical_fixup(MachineId::new(7).unwrap())
            );
        }

        #[test]
        fn scalar_call_validation_rejects_malformed_selected_inputs() {
            let (physical, kind, alternative, operands, effects) = inputs();
            let fixup = canonical_fixup(MachineId::new(7).unwrap());
            assert_eq!(
                validate_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                    &0x9400_0000_u32.to_le_bytes(),
                    fixup,
                ),
                Err(Aarch64ScalarCallTemplateError::UnsupportedTarget)
            );
            let undeclared_coff = NativeTarget {
                object_format: target::ObjectFormat::Coff,
                ..NativeTarget::linux_arm64()
            };
            assert_eq!(
                validate_aarch64_selected_scalar_call_template(
                    undeclared_coff,
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                    &0x9400_0000_u32.to_le_bytes(),
                    fixup,
                ),
                Err(Aarch64ScalarCallTemplateError::UnsupportedTarget)
            );
            assert_eq!(
                validate_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    SelectedInstructionKind::ReturnUnit,
                    alternative,
                    &operands,
                    &effects,
                    &0x9400_0000_u32.to_le_bytes(),
                    fixup,
                ),
                Err(Aarch64ScalarCallTemplateError::InstructionKindMismatch)
            );
            assert_eq!(
                validate_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    MachineAlternativeKey {
                        variant: 1,
                        ..alternative
                    },
                    &operands,
                    &effects,
                    &0x9400_0000_u32.to_le_bytes(),
                    fixup,
                ),
                Err(Aarch64ScalarCallTemplateError::AlternativeMismatch)
            );
            assert_eq!(
                validate_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &operands[..2],
                    &effects,
                    &0x9400_0000_u32.to_le_bytes(),
                    fixup,
                ),
                Err(Aarch64ScalarCallTemplateError::OperandViewMismatch)
            );
            let mut malformed_effects = effects.clone();
            malformed_effects.external_operand_reads.clear();
            assert_eq!(
                validate_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &malformed_effects,
                    &0x9400_0000_u32.to_le_bytes(),
                    fixup,
                ),
                Err(Aarch64ScalarCallTemplateError::EffectMismatch)
            );
        }

        #[test]
        fn scalar_call_validation_rejects_nonzero_or_trailing_bytes_and_bad_fixup() {
            let (physical, kind, alternative, operands, effects) = inputs();
            let fixup = canonical_fixup(MachineId::new(7).unwrap());
            for malformed in [
                &0x1400_0000_u32.to_le_bytes()[..],
                &0x9400_0001_u32.to_le_bytes()[..],
                &[0, 0, 0x94][..],
                &[0, 0, 0, 0x94, 0][..],
            ] {
                assert_eq!(
                    validate_aarch64_selected_scalar_call_template(
                        NativeTarget::linux_arm64(),
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &effects,
                        malformed,
                        fixup,
                    ),
                    Err(Aarch64ScalarCallTemplateError::MalformedTemplate)
                );
            }
            assert_eq!(
                validate_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                    &0x9400_0000_u32.to_le_bytes(),
                    Aarch64ScalarCallFixup {
                        callee: MachineId::new(8).unwrap(),
                        ..fixup
                    },
                ),
                Err(Aarch64ScalarCallTemplateError::FixupMismatch)
            );
            assert_eq!(
                validate_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                    &0x9400_0000_u32.to_le_bytes(),
                    Aarch64ScalarCallFixup {
                        reference_byte_offset: 4,
                        ..fixup
                    },
                ),
                Err(Aarch64ScalarCallTemplateError::FixupMismatch)
            );
        }
    }

    fn expected_no_normal_mechanics(
        abi: Aarch64SelectedAbi,
        physical: &ValidatedPhysicalRegisterModel,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
    ) -> Result<(Vec<RegisterViewId>, MachineEncodedEffects), Aarch64ScalarCallTemplateError> {
        let darwin = abi == Aarch64SelectedAbi::Darwin;
        let keys = if darwin {
            crate::aarch64_darwin_register_call_keys()
                .into_iter()
                .chain(crate::aarch64_darwin_register_unit_call_keys())
                .chain(crate::aarch64_darwin_mixed_unit_call_keys())
                .collect::<Vec<_>>()
        } else {
            crate::aarch64_aapcs64_register_call_keys()
                .into_iter()
                .chain(crate::aarch64_aapcs64_register_unit_call_keys())
                .chain(crate::aarch64_aapcs64_mixed_unit_call_keys())
                .collect::<Vec<_>>()
        };
        let keys = keys
            .into_iter()
            .chain(crate::aarch64_float_scalar_call_keys(darwin))
            .chain(crate::aarch64_register_aggregate_call_keys(darwin))
            .chain(crate::aarch64_mixed_aggregate_call_keys(darwin))
            .chain(crate::aarch64_indirect_aggregate_call_keys(darwin))
            .collect::<Vec<_>>();
        let catalog = aarch64_register_constraint_catalog_for(physical);
        for row in &catalog.constraints {
            if !keys.contains(&row.key)
                || row.operands.len() != operand_views.len()
                || !row
                    .operands
                    .iter()
                    .zip(operand_views)
                    .all(|(operand, view)| operand.fixed_view == Some(*view))
            {
                continue;
            }
            let mut expected = expected_effects(abi, physical, 0);
            expected.external_operand_reads = row.operand_reads();
            expected.external_operand_writes = row.operand_writes();
            expected.implicit_unit_uses = row.implicit_uses.clone();
            expected.implicit_unit_defs = row.implicit_defs.clone();
            expected.implicit_unit_clobbers = row.clobbers.clone();
            if &expected == effects {
                return Ok((operand_views.to_vec(), expected));
            }
        }
        Err(Aarch64ScalarCallTemplateError::EffectMismatch)
    }

    fn expected_call_mechanics(
        abi: Aarch64SelectedAbi,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        operand_views: &[RegisterViewId],
    ) -> Result<(Vec<RegisterViewId>, MachineEncodedEffects), Aarch64ScalarCallTemplateError> {
        let unit = matches!(kind, SelectedInstructionKind::CallUnit { .. });
        let aggregate = matches!(kind, SelectedInstructionKind::CallAggregate { .. });
        let floating_scalar = !unit
            && !aggregate
            && operand_views.iter().any(|id| {
                physical
                    .model()
                    .views
                    .iter()
                    .any(|view| view.id == *id && view.name.starts_with("d"))
            });
        let (expected_operand_views, expected) = if unit || aggregate || floating_scalar {
            let darwin = abi == Aarch64SelectedAbi::Darwin;
            let keys = if aggregate {
                crate::aarch64_register_aggregate_call_keys(darwin)
                    .into_iter()
                    .chain(crate::aarch64_mixed_aggregate_call_keys(darwin))
                    .chain(crate::aarch64_indirect_aggregate_call_keys(darwin))
                    .collect::<Vec<_>>()
            } else if floating_scalar {
                crate::aarch64_float_scalar_call_keys(darwin)
            } else {
                match abi {
                    Aarch64SelectedAbi::Aapcs64 => crate::aarch64_aapcs64_register_unit_call_keys()
                        .into_iter()
                        .chain(crate::aarch64_aapcs64_mixed_unit_call_keys())
                        .collect::<Vec<_>>(),
                    Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_register_unit_call_keys()
                        .into_iter()
                        .chain(crate::aarch64_darwin_mixed_unit_call_keys())
                        .collect::<Vec<_>>(),
                }
            };
            let catalog = aarch64_register_constraint_catalog_for(physical);
            let row = catalog
                .constraints
                .iter()
                .find(|row| {
                    keys.contains(&row.key)
                        && row.operands.len() == operand_views.len()
                        && row
                            .operands
                            .iter()
                            .zip(operand_views)
                            .all(|(operand, view)| operand.fixed_view == Some(*view))
                })
                .ok_or(Aarch64ScalarCallTemplateError::OperandViewMismatch)?;
            let mut expected = expected_effects(abi, physical, 0);
            expected.external_operand_reads = row.operand_reads();
            expected.external_operand_writes = row.operand_writes();
            expected.implicit_unit_uses = row.implicit_uses.clone();
            expected.implicit_unit_defs = row.implicit_defs.clone();
            expected.implicit_unit_clobbers = row.clobbers.clone();
            (operand_views.to_vec(), expected)
        } else {
            let arity = operand_views
                .len()
                .checked_sub(1)
                .filter(|arity| *arity <= 8)
                .ok_or(Aarch64ScalarCallTemplateError::OperandViewMismatch)?;
            let expected_operand_views = expected_operand_views(physical, arity);
            if operand_views != expected_operand_views {
                return Err(Aarch64ScalarCallTemplateError::OperandViewMismatch);
            }
            (
                expected_operand_views,
                expected_effects(abi, physical, arity),
            )
        };

        Ok((expected_operand_views, expected))
    }

    pub fn encode_aarch64_selected_dynamic_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
    ) -> Result<(Vec<u8>, MachineEncodedEffects), Aarch64ScalarCallTemplateError> {
        let abi = aarch64_selected_abi(target)
            .map_err(|_| Aarch64ScalarCallTemplateError::UnsupportedTarget)?;
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Aarch64ScalarCallTemplateError::NonCanonicalPhysicalModel);
        }
        let Some(offset) = kind
            .call_target()
            .and_then(|target| target.indirect_code_offset())
        else {
            return Err(Aarch64ScalarCallTemplateError::InstructionKindMismatch);
        };
        let family = match kind {
            SelectedInstructionKind::CallUnit { .. } => MachineAlternativeFamily::CallUnit,
            SelectedInstructionKind::CallScalar { .. } => MachineAlternativeFamily::CallScalar,
            SelectedInstructionKind::CallAggregate { .. } => {
                MachineAlternativeFamily::CallAggregate
            }
            _ => return Err(Aarch64ScalarCallTemplateError::InstructionKindMismatch),
        };
        if alternative != (MachineAlternativeKey { family, variant: 1 }) {
            return Err(Aarch64ScalarCallTemplateError::AlternativeMismatch);
        }
        let selector = physical
            .model()
            .view_named("x16")
            .ok_or(Aarch64ScalarCallTemplateError::OperandViewMismatch)?
            .id;
        let mut ordinary = operand_views.to_vec();
        let position = ordinary
            .iter()
            .position(|view| *view == selector)
            .ok_or(Aarch64ScalarCallTemplateError::OperandViewMismatch)?;
        ordinary.remove(position);
        let (_, baseline) = expected_call_mechanics(abi, physical, kind, &ordinary)?;
        if position != baseline.external_operand_reads.len() {
            return Err(Aarch64ScalarCallTemplateError::OperandViewMismatch);
        }
        let expected = selected_instructions::calls::dynamic::encoded_effects(
            &baseline,
            physical.model().architecture,
        )
        .ok_or(Aarch64ScalarCallTemplateError::EffectMismatch)?;
        if *effects != expected {
            return Err(Aarch64ScalarCallTemplateError::EffectMismatch);
        }

        let mut bytes = Vec::with_capacity(36);
        for half in 0..4 {
            let immediate = ((offset >> (half * 16)) & 0xffff) as u32;
            let opcode = if half == 0 { 0xd280_0000 } else { 0xf280_0000 };
            bytes.extend_from_slice(
                &(opcode | (half as u32) << 21 | immediate << 5 | 17).to_le_bytes(),
            );
        }
        for word in [
            0x8b11_0211_u32,
            0xf940_0231,
            0xb500_0051,
            0xd420_0000,
            0xd63f_0220,
        ] {
            crate::codec_bytes::push_encoded(&mut bytes, word.to_le_bytes());
        }
        Ok((bytes, expected))
    }

    #[cfg(test)]
    mod dynamic_call_tests {
        use super::*;
        #[test]
        fn dispatch_register_is_separate_from_each_supported_data_arity_and_result() {
            let physical = register_model::validate_physical_register_model(
                crate::aarch64_physical_register_model(),
            )
            .unwrap();
            for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
                let abi = aarch64_selected_abi(target).unwrap();
                for arity in [0, 1, 8] {
                    let mut operands = expected_operand_views(&physical, arity);
                    let selector = physical.model().view_named("x16").unwrap().id;
                    operands.insert(arity, selector);
                    let baseline = expected_effects(abi, &physical, arity);
                    let effects = selected_instructions::calls::dynamic::encoded_effects(
                        &baseline,
                        physical.model().architecture,
                    )
                    .unwrap();
                    let kind = SelectedInstructionKind::CallScalar {
                        target: selected_instructions::SelectedCallTarget::Dynamic {
                            interface: 7,
                            slot: 0x1234_5678,
                        },
                    };
                    let alternative = MachineAlternativeKey {
                        family: MachineAlternativeFamily::CallScalar,
                        variant: 1,
                    };
                    let (bytes, actual) = encode_aarch64_selected_dynamic_call_template(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &effects,
                    )
                    .unwrap();
                    assert_eq!(bytes.len(), 36);
                    assert_eq!(
                        actual.external_operand_reads,
                        (0..=arity as u16).collect::<Vec<_>>()
                    );
                    assert_eq!(actual.external_operand_writes, vec![arity as u16 + 1]);
                    assert_eq!(
                        actual.control,
                        MachineEncodedControlEffect::IndirectTableCallOrTrapV1
                    );
                    assert_eq!(&bytes[28..32], &0xd420_0000_u32.to_le_bytes());
                    assert_eq!(&bytes[32..36], &0xd63f_0220_u32.to_le_bytes());
                    let mut missing_dispatch = operands.clone();
                    missing_dispatch.remove(arity);
                    assert!(
                        encode_aarch64_selected_dynamic_call_template(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &missing_dispatch,
                            &effects
                        )
                        .is_err()
                    );
                    let mut altered = effects.clone();
                    altered.external_operand_writes = vec![arity as u16];
                    assert!(
                        encode_aarch64_selected_dynamic_call_template(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &altered
                        )
                        .is_err()
                    );
                }
            }
        }
    }
}
mod selected_forms {
    use crate::saturating_forms::SaturatingRealization;
    use crate::selected_form_encoding::compare_byte_sequences;
    use crate::selected_form_encoding::copy_bytes;
    use crate::selected_form_encoding::decoding::{decode_words, footprint, validate_decoded};
    use crate::selected_form_encoding::encoded_scalar;
    use crate::selected_form_encoding::float_bits;
    use crate::selected_form_encoding::movn_materialization::append_canonical_materialization;
    use crate::selected_form_encoding::scan_byte_sequence;
    use crate::selected_form_encoding::zero_bytes;
    use crate::selected_form_encoding::{
        Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding,
    };
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
        is_float_bit_transfer,
    };
    use semantic_vocabulary::IntegerValue;

    pub fn encode_aarch64_selected_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        if kind == SelectedInstructionKind::CopyBytes {
            return copy_bytes::encode(physical, alternative, operands);
        }
        if kind == SelectedInstructionKind::ZeroBytes {
            return zero_bytes::encode(physical, alternative, operands);
        }
        if matches!(
            kind,
            SelectedInstructionKind::LoadEncodedScalar
                | SelectedInstructionKind::StoreEncodedScalar
        ) {
            return encoded_scalar::encode(physical, kind, alternative, operands);
        }
        if let SelectedInstructionKind::ScanByteSequence { predicate } = kind {
            return scan_byte_sequence::encode(physical, predicate, alternative, operands);
        }
        if kind == SelectedInstructionKind::CompareByteSequences {
            return compare_byte_sequences::encode(physical, alternative, operands);
        }
        if is_float_bit_transfer(kind) {
            return float_bits::encode(physical, kind, alternative, operands);
        }
        validate_request(physical, kind, alternative, operands)?;
        let registers = resolve_scalar_registers(physical, kind, operands)?;
        validate_return_home(kind, &registers)?;
        let bytes = encode_unchecked(kind, &registers)?;
        validate_aarch64_selected_form_encoding(physical, kind, alternative, operands, &bytes)
    }

    pub fn validate_aarch64_selected_form_encoding(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        if kind == SelectedInstructionKind::CopyBytes {
            return copy_bytes::validate(physical, alternative, operands, bytes);
        }
        if kind == SelectedInstructionKind::ZeroBytes {
            return zero_bytes::validate(physical, alternative, operands, bytes);
        }
        if matches!(
            kind,
            SelectedInstructionKind::LoadEncodedScalar
                | SelectedInstructionKind::StoreEncodedScalar
        ) {
            return encoded_scalar::validate(physical, kind, alternative, operands, bytes);
        }
        if let SelectedInstructionKind::ScanByteSequence { predicate } = kind {
            return scan_byte_sequence::validate(physical, predicate, alternative, operands, bytes);
        }
        if kind == SelectedInstructionKind::CompareByteSequences {
            return compare_byte_sequences::validate(physical, alternative, operands, bytes);
        }
        if is_float_bit_transfer(kind) {
            return float_bits::validate(physical, kind, alternative, operands, bytes);
        }
        validate_request(physical, kind, alternative, operands)?;
        let registers = resolve_scalar_registers(physical, kind, operands)?;
        validate_return_home(kind, &registers)?;
        if let SelectedInstructionKind::RegisterAccess {
            operation: selected_instructions::RegisterOperation::MemoryFence(expected),
        } = kind
        {
            if super::memory_fence::decode(bytes) != Some(expected) {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
        } else if let SelectedInstructionKind::AtomicAccess { form } = kind {
            super::atomic::validate(form, &registers, bytes)?;
        } else if let SelectedInstructionKind::NearestIeeeFloatArithmetic { form } = kind {
            super::ieee_float_arithmetic::validate(form, &registers, bytes)?;
        } else if let SelectedInstructionKind::FloatResultPolicy { form } = kind {
            super::float_result_policy::validate(form, &registers, bytes)?;
        } else {
            let decoded = decode_words(bytes)?;
            validate_decoded(kind, &registers, &decoded)?;
        }
        let canonical = encode_unchecked(kind, &registers)?;
        if bytes != canonical {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedAarch64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: footprint(kind, operands),
        })
    }

    fn validate_request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<(), Aarch64SelectedFormEncodingError> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
        }
        let (family, count) = family_and_operand_count(kind)?;
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
        }
        if operands.len() != count {
            return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
        }
        Ok(())
    }

    fn family_and_operand_count(
        kind: SelectedInstructionKind,
    ) -> Result<(MachineAlternativeFamily, usize), Aarch64SelectedFormEncodingError> {
        Ok(match kind {
            SelectedInstructionKind::AccessorScalarAccess { form, .. } => {
                (MachineAlternativeFamily::AccessorScalarAccess(form), 2)
            }
            SelectedInstructionKind::AtomicAccess { form } => (
                MachineAlternativeFamily::AtomicAccess(form),
                usize::from(form.operand_count()),
            ),
            SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => (
                MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
                form.operand_count() + 1,
            ),
            SelectedInstructionKind::FloatResultPolicy { form } => (
                MachineAlternativeFamily::FloatResultPolicy(form),
                form.operand_count() + 1,
            ),
            SelectedInstructionKind::RegisterAccess {
                operation: selected_instructions::RegisterOperation::MemoryFence(kind),
            } => {
                if kind.target() != selected_instructions::RegisterTargetApplicability::Aarch64 {
                    return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
                }
                (MachineAlternativeFamily::MemoryFence, 0)
            }
            SelectedInstructionKind::RegisterAccess { operation } => (
                if operation.produces_value() {
                    MachineAlternativeFamily::SystemRegisterRead
                } else {
                    MachineAlternativeFamily::SystemRegisterWrite
                },
                1,
            ),
            SelectedInstructionKind::PrivilegedCrash
            | SelectedInstructionKind::InstalledInterruptCpuCheck { .. }
            | SelectedInstructionKind::InstalledInterruptTaskCommit { .. }
            | SelectedInstructionKind::InstalledInterruptTablePublication { .. }
            | SelectedInstructionKind::PortWrite { .. } => {
                return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
            }
            SelectedInstructionKind::AtomicFence => (MachineAlternativeFamily::AtomicFence, 0),
            SelectedInstructionKind::Crash => (MachineAlternativeFamily::Crash, 0),
            SelectedInstructionKind::TrappingInteger { form } => (
                MachineAlternativeFamily::TrappingInteger(form),
                super::trapping_forms::operand_count(form),
            ),
            SelectedInstructionKind::CompareByteSequences => {
                (MachineAlternativeFamily::CompareByteSequences, 8)
            }
            SelectedInstructionKind::CompareI64Zero => {
                (MachineAlternativeFamily::CompareI64Zero, 1)
            }
            SelectedInstructionKind::CompareI64Immediate { .. } => {
                (MachineAlternativeFamily::CompareI64Immediate, 1)
            }
            SelectedInstructionKind::CompareI64 => (MachineAlternativeFamily::CompareI64, 2),
            SelectedInstructionKind::MaterializeI64 { .. } => {
                (MachineAlternativeFamily::MaterializeI64, 1)
            }
            SelectedInstructionKind::ImageAddress { .. } => {
                (MachineAlternativeFamily::ImageAddress, 1)
            }
            SelectedInstructionKind::CopyI64 => (MachineAlternativeFamily::CopyI64, 2),
            SelectedInstructionKind::MaterializeBooleanEqual => {
                (MachineAlternativeFamily::MaterializeBooleanEqual, 1)
            }
            SelectedInstructionKind::MaterializeBooleanU64LessThan => {
                (MachineAlternativeFamily::MaterializeBooleanU64LessThan, 1)
            }
            SelectedInstructionKind::MaterializeBooleanI64LessThan => {
                (MachineAlternativeFamily::MaterializeBooleanI64LessThan, 1)
            }
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => (
                MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual,
                1,
            ),
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => (
                MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual,
                1,
            ),
            SelectedInstructionKind::ZeroExtendU8 => (MachineAlternativeFamily::ZeroExtendU8, 2),
            SelectedInstructionKind::ZeroExtendU16 => (MachineAlternativeFamily::ZeroExtendU16, 2),
            SelectedInstructionKind::SignExtendI8 => (MachineAlternativeFamily::SignExtendI8, 2),
            SelectedInstructionKind::SignExtendI16 => (MachineAlternativeFamily::SignExtendI16, 2),
            SelectedInstructionKind::SignExtendI32 => (MachineAlternativeFamily::SignExtendI32, 2),
            SelectedInstructionKind::ZeroExtendU32 => (MachineAlternativeFamily::ZeroExtendU32, 2),
            SelectedInstructionKind::ByteViewAddress => {
                (MachineAlternativeFamily::ByteViewAddress, 3)
            }
            SelectedInstructionKind::ExactAddI64 { .. } => {
                (MachineAlternativeFamily::ExactAddI64, 3)
            }
            SelectedInstructionKind::WrappingAddI64 => {
                (MachineAlternativeFamily::WrappingAddI64, 3)
            }
            SelectedInstructionKind::WrappingSubtractI64 => {
                (MachineAlternativeFamily::WrappingSubtractI64, 3)
            }
            SelectedInstructionKind::WrappingMultiplyI64 => {
                (MachineAlternativeFamily::WrappingMultiplyI64, 3)
            }
            SelectedInstructionKind::BitwiseAndI64 => (MachineAlternativeFamily::BitwiseAndI64, 3),
            SelectedInstructionKind::BitwiseOrI64 => (MachineAlternativeFamily::BitwiseOrI64, 3),
            SelectedInstructionKind::BitwiseXorI64 => (MachineAlternativeFamily::BitwiseXorI64, 3),
            SelectedInstructionKind::BitwiseNotI64 => (MachineAlternativeFamily::BitwiseNotI64, 2),
            SelectedInstructionKind::SaturatingAdd { carrier } => (
                MachineAlternativeFamily::SaturatingAdd(carrier),
                SaturatingRealization::of_kind(kind)
                    .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
                    .operand_count(),
            ),
            SelectedInstructionKind::SaturatingSubtract { carrier } => (
                MachineAlternativeFamily::SaturatingSubtract(carrier),
                SaturatingRealization::of_kind(kind)
                    .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
                    .operand_count(),
            ),
            SelectedInstructionKind::SaturatingDivide { carrier, .. } => (
                MachineAlternativeFamily::SaturatingDivide(carrier),
                SaturatingRealization::of_kind(kind)
                    .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
                    .operand_count(),
            ),
            SelectedInstructionKind::SaturatingRemainder { carrier, .. } => (
                MachineAlternativeFamily::SaturatingRemainder(carrier),
                SaturatingRealization::of_kind(kind)
                    .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
                    .operand_count(),
            ),
            SelectedInstructionKind::SaturatingMultiply { carrier } => (
                MachineAlternativeFamily::SaturatingMultiply(carrier),
                SaturatingRealization::of_kind(kind)
                    .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
                    .operand_count(),
            ),
            SelectedInstructionKind::ExactDivideU64 { .. } => {
                (MachineAlternativeFamily::ExactDivideU64, 3)
            }
            SelectedInstructionKind::ExactRemainderU64 { .. } => {
                (MachineAlternativeFamily::ExactRemainderU64, 3)
            }
            SelectedInstructionKind::WrappingRemainderI64 { .. } => {
                (MachineAlternativeFamily::WrappingRemainderI64, 3)
            }
            SelectedInstructionKind::WrappingDivideI64 { .. } => {
                (MachineAlternativeFamily::WrappingDivideI64, 3)
            }
            SelectedInstructionKind::ExactDivideI64 { .. } => {
                (MachineAlternativeFamily::ExactDivideI64, 3)
            }
            SelectedInstructionKind::ExactRemainderI64 { .. } => {
                (MachineAlternativeFamily::ExactRemainderI64, 3)
            }
            SelectedInstructionKind::ExactSubtractI64 { .. } => {
                (MachineAlternativeFamily::ExactSubtractI64, 3)
            }
            SelectedInstructionKind::ExactMultiplyI64 { .. } => {
                (MachineAlternativeFamily::ExactMultiplyI64, 3)
            }
            SelectedInstructionKind::WrappingShiftLeftI64 => {
                (MachineAlternativeFamily::WrappingShiftLeftI64, 3)
            }
            SelectedInstructionKind::WrappingShiftRightI64 => {
                (MachineAlternativeFamily::WrappingShiftRightI64, 3)
            }
            SelectedInstructionKind::WrappingShiftRightU64 => {
                (MachineAlternativeFamily::WrappingShiftRightU64, 3)
            }
            SelectedInstructionKind::ExactShiftLeftI64 { .. } => {
                (MachineAlternativeFamily::ExactShiftLeftI64, 3)
            }
            SelectedInstructionKind::ExactShiftRightI64 { .. } => {
                (MachineAlternativeFamily::ExactShiftRightI64, 3)
            }
            SelectedInstructionKind::ExactShiftRightU64 { .. } => {
                (MachineAlternativeFamily::ExactShiftRightU64, 3)
            }
            SelectedInstructionKind::ExactAddI64Immediate { .. } => {
                (MachineAlternativeFamily::ExactAddI64Immediate, 2)
            }
            SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
                (MachineAlternativeFamily::ExactSubtractI64Immediate, 2)
            }
            SelectedInstructionKind::ReturnScalar => (MachineAlternativeFamily::ReturnScalar, 1),
            SelectedInstructionKind::ReturnAggregate { fragment_count } => {
                if !(1..=2).contains(&fragment_count) {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
                (
                    MachineAlternativeFamily::ReturnAggregate,
                    usize::from(fragment_count),
                )
            }
            SelectedInstructionKind::ReturnUnit => (MachineAlternativeFamily::ReturnUnit, 0),
            SelectedInstructionKind::ConditionalBranchNonZero => {
                return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
            }
            SelectedInstructionKind::ConditionalBranchU64LessThan => {
                return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
            }
            SelectedInstructionKind::ConditionalBranchI64LessThan => {
                return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
            }
            SelectedInstructionKind::Float32ToBits
            | SelectedInstructionKind::Float64ToBits
            | SelectedInstructionKind::BitsToFloat32
            | SelectedInstructionKind::BitsToFloat64
            | SelectedInstructionKind::CopyBytes
            | SelectedInstructionKind::ZeroBytes
            | SelectedInstructionKind::ScanByteSequence { .. }
            | SelectedInstructionKind::LoadEncodedScalar
            | SelectedInstructionKind::StoreEncodedScalar
            | SelectedInstructionKind::Load8Indexed
            | selected_instructions::load_width_triple_pattern!(..)
            | SelectedInstructionKind::Load64 { .. }
            | SelectedInstructionKind::LoadPacked { .. }
            | SelectedInstructionKind::StorePacked { .. }
            | SelectedInstructionKind::Store { .. }
            | SelectedInstructionKind::AddressOffset { .. }
            | SelectedInstructionKind::Store64 { .. }
            | SelectedInstructionKind::HostedExitProcessI32
            | selected_instructions::hosted_slot_quad_pattern!(..)
            | SelectedInstructionKind::FrameAddress { .. }
            | SelectedInstructionKind::InvokeNoNormal { .. }
            | SelectedInstructionKind::CallUnit { .. }
            | SelectedInstructionKind::Jump
            | SelectedInstructionKind::CallAggregate { .. }
            | SelectedInstructionKind::NormalizedForeignCall { .. }
            | SelectedInstructionKind::CallScalar { .. } => {
                return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
            }
        })
    }

    fn validate_return_home(
        kind: SelectedInstructionKind,
        registers: &[u8],
    ) -> Result<(), Aarch64SelectedFormEncodingError> {
        if matches!(kind, SelectedInstructionKind::ReturnScalar) && registers != [0] {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        if let SelectedInstructionKind::ReturnAggregate { fragment_count } = kind
            && (!(1..=2).contains(&fragment_count)
                || registers != &([0, 1][..usize::from(fragment_count)]))
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(())
    }

    fn resolve_scalar_registers(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
        if kind == SelectedInstructionKind::ReturnScalar
            && physical
                .model()
                .view_named("d0")
                .is_some_and(|view| operands == [view.id])
        {
            return Ok(vec![0]);
        }
        resolve_registers(physical, operands)
    }

    pub(super) fn request_registers<const COUNT: usize>(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        family: MachineAlternativeFamily,
        operands: &[RegisterViewId],
        first_early: usize,
    ) -> Result<[u8; COUNT], Aarch64SelectedFormEncodingError> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
        }
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
        }
        let registers: [u8; COUNT] = resolve_registers(physical, operands)?
            .try_into()
            .map_err(|_| Aarch64SelectedFormEncodingError::OperandCountMismatch)?;
        if (first_early..COUNT).any(|slot| registers[..slot].contains(&registers[slot])) {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(registers)
    }

    pub(crate) fn resolve_registers(
        physical: &ValidatedPhysicalRegisterModel,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
        operands
            .iter()
            .map(|id| {
                let view = physical
                    .model()
                    .view(*id)
                    .ok_or(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
                let Some(index) = view.name.strip_prefix('x') else {
                    return Err(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
                };
                let index = index
                    .parse::<u8>()
                    .ok()
                    .filter(|index| *index <= 30)
                    .ok_or(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
                if view.bits != 64 || !view.allocatable {
                    return Err(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
                }
                Ok(index)
            })
            .collect()
    }

    pub(crate) fn integer_bits(
        value: IntegerValue,
    ) -> Result<u64, Aarch64SelectedFormEncodingError> {
        match value {
            IntegerValue::Signed(value) => i64::try_from(value)
                .map(|value| value as u64)
                .map_err(|_| Aarch64SelectedFormEncodingError::IntegerOutsideI64Bits),
            IntegerValue::Unsigned(value) => u64::try_from(value)
                .map_err(|_| Aarch64SelectedFormEncodingError::IntegerOutsideI64Bits),
        }
    }

    pub(crate) fn u12(value: IntegerValue) -> Result<u16, Aarch64SelectedFormEncodingError> {
        match value {
            IntegerValue::Unsigned(value) if value <= 4095 => Ok(value as u16),
            _ => Err(Aarch64SelectedFormEncodingError::ImmediateOutsideU12),
        }
    }

    fn encode_unchecked(
        kind: SelectedInstructionKind,
        registers: &[u8],
    ) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
        let mut words = Vec::new();
        macro_rules! push_three {
            ($opcode:expr $(,)?) => {
                words.push(three_address(
                    $opcode,
                    registers[0],
                    registers[1],
                    registers[2],
                ))
            };
        }
        match kind {
            SelectedInstructionKind::AccessorScalarAccess { .. } => {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            SelectedInstructionKind::AtomicAccess { form } => {
                return super::atomic::encode(form, registers);
            }
            SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => {
                return super::ieee_float_arithmetic::encode(form, registers);
            }
            SelectedInstructionKind::FloatResultPolicy { form } => {
                return super::float_result_policy::encode(form, registers);
            }
            SelectedInstructionKind::RegisterAccess {
                operation: selected_instructions::RegisterOperation::MemoryFence(kind),
            } => {
                if !registers.is_empty() {
                    return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
                }
                return super::memory_fence::encode(kind)
                    .ok_or(Aarch64SelectedFormEncodingError::AlternativeMismatch);
            }
            SelectedInstructionKind::RegisterAccess { operation } => {
                return super::register_access::encode(operation, registers);
            }
            SelectedInstructionKind::PrivilegedCrash
            | SelectedInstructionKind::InstalledInterruptCpuCheck { .. }
            | SelectedInstructionKind::InstalledInterruptTaskCommit { .. }
            | SelectedInstructionKind::InstalledInterruptTablePublication { .. }
            | SelectedInstructionKind::PortWrite { .. } => {
                return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
            }
            SelectedInstructionKind::AtomicFence => words.push(0xd503_3fbf),
            SelectedInstructionKind::Crash => words.push(0xd420_0000),
            SelectedInstructionKind::TrappingInteger { form } => {
                let sources = form.source_count();
                if registers[sources..]
                    .iter()
                    .enumerate()
                    .any(|(index, late)| {
                        registers[..sources].contains(late)
                            || registers[sources + index + 1..].contains(late)
                    })
                {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
                words.extend(
                    super::trapping_forms::realization(form, registers)
                        .into_iter()
                        .map(super::trapping_forms::encode_word),
                );
            }
            SelectedInstructionKind::MaterializeI64 { value } => {
                append_canonical_materialization(&mut words, registers[0], integer_bits(value)?);
            }
            SelectedInstructionKind::ImageAddress { .. } => {
                let [page, page_offset] = byte_literal_address_words(registers[0]);
                words.extend([page, page_offset]);
            }
            SelectedInstructionKind::MaterializeBooleanEqual
            | SelectedInstructionKind::MaterializeBooleanU64LessThan
            | SelectedInstructionKind::MaterializeBooleanI64LessThan
            | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
            | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
                let condition: u32 = match kind {
                    SelectedInstructionKind::MaterializeBooleanEqual => 0,
                    SelectedInstructionKind::MaterializeBooleanU64LessThan => 3,
                    SelectedInstructionKind::MaterializeBooleanI64LessThan => 11,
                    SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => 9,
                    SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => 13,
                    _ => unreachable!("Boolean condition arm"),
                };
                words.push(0x9a9f_07e0 | ((condition ^ 1) << 12) | u32::from(registers[0]));
            }
            SelectedInstructionKind::ZeroExtendU8 => {
                words.push(two_address(0xd340_1c00, registers[0], registers[1]));
            }
            SelectedInstructionKind::ZeroExtendU16 => {
                words.push(two_address(0xd340_3c00, registers[0], registers[1]));
            }
            SelectedInstructionKind::SignExtendI8 => {
                words.push(two_address(0x9340_1c00, registers[0], registers[1]));
            }
            SelectedInstructionKind::SignExtendI16 => {
                words.push(two_address(0x9340_3c00, registers[0], registers[1]));
            }
            SelectedInstructionKind::SignExtendI32 => {
                words.push(two_address(0x9340_7c00, registers[0], registers[1]));
            }
            SelectedInstructionKind::ZeroExtendU32 => {
                words.push(two_address(0xd340_7c00, registers[0], registers[1]));
            }
            SelectedInstructionKind::CopyI64 => {
                words.push(0xaa00_03e0 | (u32::from(registers[0]) << 16) | u32::from(registers[1]));
            }
            SelectedInstructionKind::CompareI64Zero => {
                words.push(0xf100_001f | (u32::from(registers[0]) << 5));
            }
            SelectedInstructionKind::CompareI64Immediate { immediate } => {
                words.push(
                    0xf100_001f
                        | (u32::from(u12(immediate)?) << 10)
                        | (u32::from(registers[0]) << 5),
                );
            }
            SelectedInstructionKind::CompareI64 => {
                words.push(
                    0xeb00_001f | (u32::from(registers[1]) << 16) | (u32::from(registers[0]) << 5),
                );
            }
            SelectedInstructionKind::ByteViewAddress
            | SelectedInstructionKind::WrappingAddI64
            | SelectedInstructionKind::ExactAddI64 { .. } => {
                push_three!(0x8b00_0000);
            }
            SelectedInstructionKind::BitwiseAndI64 => {
                push_three!(0x8a00_0000);
            }
            SelectedInstructionKind::BitwiseOrI64 => {
                push_three!(0xaa00_0000);
            }
            SelectedInstructionKind::BitwiseNotI64 => {
                words.push(0xaa20_03e0 | (u32::from(registers[0]) << 16) | u32::from(registers[1]));
            }
            selected_instructions::saturating_add_or_subtract_pattern!()
            | SelectedInstructionKind::SaturatingDivide { .. }
            | SelectedInstructionKind::SaturatingRemainder { .. }
            | SelectedInstructionKind::SaturatingMultiply { .. } => {
                append_saturating(&mut words, kind, registers)?;
            }
            SelectedInstructionKind::ExactDivideU64 { .. } => {
                push_three!(0x9ac0_0800);
            }
            SelectedInstructionKind::WrappingDivideI64 { .. } => {
                push_three!(0x9ac0_0c00);
            }
            SelectedInstructionKind::WrappingRemainderI64 { .. }
            | SelectedInstructionKind::ExactRemainderI64 { .. } => {
                if registers[2] == registers[0] || registers[2] == registers[1] {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
                push_three!(0x9ac0_0c00);
                words.push(
                    0x9b00_8000
                        | (u32::from(registers[1]) << 16)
                        | (u32::from(registers[0]) << 10)
                        | (u32::from(registers[2]) << 5)
                        | u32::from(registers[2]),
                );
            }
            SelectedInstructionKind::ExactDivideI64 { .. } => {
                push_three!(0x9ac0_0c00);
            }
            SelectedInstructionKind::ExactRemainderU64 { .. } => {
                if registers[2] == registers[0] || registers[2] == registers[1] {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
                push_three!(0x9ac0_0800);
                words.push(
                    0x9b00_8000
                        | (u32::from(registers[1]) << 16)
                        | (u32::from(registers[0]) << 10)
                        | (u32::from(registers[2]) << 5)
                        | u32::from(registers[2]),
                );
            }
            SelectedInstructionKind::BitwiseXorI64 => {
                push_three!(0xca00_0000);
            }
            SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
                words.push(
                    0x9100_0000
                        | (u32::from(u12(immediate)?) << 10)
                        | (u32::from(registers[0]) << 5)
                        | u32::from(registers[1]),
                );
            }
            SelectedInstructionKind::ExactSubtractI64 { .. }
            | SelectedInstructionKind::WrappingSubtractI64 => {
                push_three!(0xcb00_0000);
            }
            SelectedInstructionKind::ExactMultiplyI64 { .. }
            | SelectedInstructionKind::WrappingMultiplyI64 => {
                push_three!(0x9b00_7c00);
            }
            SelectedInstructionKind::WrappingShiftLeftI64
            | SelectedInstructionKind::WrappingShiftRightI64
            | SelectedInstructionKind::WrappingShiftRightU64
            | selected_instructions::exact_shift_i64_pair_pattern!()
            | SelectedInstructionKind::ExactShiftRightU64 { .. } => {
                push_three!(match kind {
                    SelectedInstructionKind::WrappingShiftLeftI64
                    | SelectedInstructionKind::ExactShiftLeftI64 { .. } => 0x9ac0_2000,
                    SelectedInstructionKind::WrappingShiftRightU64
                    | SelectedInstructionKind::ExactShiftRightU64 { .. } => 0x9ac0_2400,
                    _ => 0x9ac0_2800,
                },);
            }
            SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
                words.push(
                    0xd100_0000
                        | (u32::from(u12(immediate)?) << 10)
                        | (u32::from(registers[0]) << 5)
                        | u32::from(registers[1]),
                );
            }
            SelectedInstructionKind::ReturnScalar
            | SelectedInstructionKind::ReturnAggregate { .. }
            | SelectedInstructionKind::ReturnUnit => {
                words.push(u32::from_le_bytes(crate::ORDINARY_RETURN_BYTES))
            }
            selected_instructions::layout_dependent_selected_transfers!()
            | selected_instructions::dedicated_selected_forms!() => {
                return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
            }
        }
        Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
    }

    pub(crate) const BOUND_WORDS: [(u64, u32); 11] = [
        (i8::MAX as u64, 0xb240_1be0),
        (i8::MIN as i64 as u64, 0xb279_e3e0),
        (i16::MAX as u64, 0xb240_3be0),
        (i16::MIN as i64 as u64, 0xb271_c3e0),
        (i32::MAX as u64, 0xb240_7be0),
        (i32::MIN as i64 as u64, 0xb261_83e0),
        (i64::MAX as u64, 0xb240_fbe0),
        (i64::MIN as u64, 0xb241_03e0),
        (u8::MAX as u64, 0xb240_1fe0),
        (u16::MAX as u64, 0xb240_3fe0),
        (u32::MAX as u64, 0xb240_7fe0),
    ];

    fn bound_word(bound: u64, destination: u8) -> Result<u32, Aarch64SelectedFormEncodingError> {
        BOUND_WORDS
            .iter()
            .find(|(value, _)| *value == bound)
            .map(|(_, word)| word | u32::from(destination))
            .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    }

    fn select(value: u8, scratch: u8, condition: u32) -> u32 {
        0x9a80_0000
            | (u32::from(value) << 16)
            | (condition << 12)
            | (u32::from(scratch) << 5)
            | u32::from(value)
    }

    const MULTIPLY: u32 = 0x9b00_7c00;
    const SIGNED_MULTIPLY_HIGH: u32 = 0x9b40_7c00;
    const UNSIGNED_MULTIPLY_HIGH: u32 = 0x9bc0_7c00;

    fn three_address(opcode: u32, left: u8, right: u8, destination: u8) -> u32 {
        opcode | (u32::from(right) << 16) | (u32::from(left) << 5) | u32::from(destination)
    }

    fn two_address(opcode: u32, left: u8, destination: u8) -> u32 {
        opcode | (u32::from(left) << 5) | u32::from(destination)
    }

    fn append_saturating(
        words: &mut Vec<u32>,
        kind: SelectedInstructionKind,
        registers: &[u8],
    ) -> Result<(), Aarch64SelectedFormEncodingError> {
        let realization = SaturatingRealization::of_kind(kind)
            .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        let (left, right, value) = (registers[0], registers[1], registers[2]);
        if realization.operand_count() == 4
            && (registers[2..]
                .iter()
                .any(|late| registers[..2].contains(late))
                || registers[2] == registers[3])
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        match realization {
            SaturatingRealization::AddU64 => {
                words.push(three_address(0xab00_0000, left, right, value));
                words.push(0xda9f_3000 | (u32::from(value) << 5) | u32::from(value));
            }
            SaturatingRealization::SubtractUnsigned => {
                words.push(three_address(0xeb00_0000, left, right, value));
                words.push(0x9a9f_2000 | (u32::from(value) << 5) | u32::from(value));
            }
            SaturatingRealization::DivideUnsigned => {
                words.push(three_address(0x9ac0_0800, left, right, value));
            }
            SaturatingRealization::Remainder { signed } => {
                if value == left || value == right {
                    return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
                }
                words.push(three_address(
                    if signed { 0x9ac0_0c00 } else { 0x9ac0_0800 },
                    left,
                    right,
                    value,
                ));
                words.push(
                    0x9b00_8000
                        | (u32::from(right) << 16)
                        | (u32::from(left) << 10)
                        | (u32::from(value) << 5)
                        | u32::from(value),
                );
            }
            SaturatingRealization::ClampNarrow { operation, .. } => {
                let scratch = registers[3];
                words.push(three_address(
                    match operation {
                        selected_instructions::SaturatingOperation::Add => 0x8b00_0000,
                        selected_instructions::SaturatingOperation::Subtract => 0xcb00_0000,
                        selected_instructions::SaturatingOperation::Divide => 0x9ac0_0c00,
                        selected_instructions::SaturatingOperation::Multiply => MULTIPLY,
                        selected_instructions::SaturatingOperation::Remainder => {
                            unreachable!("remainder is realized through the divide/MSUB pair")
                        }
                    },
                    left,
                    right,
                    value,
                ));
                for (index, bound) in realization.clamp_bounds().into_iter().enumerate() {
                    words.push(bound_word(bound, scratch)?);
                    words.push(0xeb00_001f | (u32::from(scratch) << 16) | (u32::from(value) << 5));
                    words.push(select(
                        value,
                        scratch,
                        if index == 0 {
                            realization.upper_condition()
                        } else {
                            0xb
                        },
                    ));
                }
            }
            SaturatingRealization::MultiplyI64 => {
                let scratch = registers[3];
                words.push(three_address(SIGNED_MULTIPLY_HIGH, left, right, scratch));
                words.push(three_address(MULTIPLY, left, right, value));
                words.push(0xeb80_fc1f | (u32::from(value) << 16) | (u32::from(scratch) << 5));
                words.push(0x937f_fc00 | (u32::from(scratch) << 5) | u32::from(scratch));
                words.push(0xd240_f800 | (u32::from(scratch) << 5) | u32::from(scratch));
                words.push(select(value, scratch, 0x1));
            }
            SaturatingRealization::MultiplyU64 => {
                let scratch = registers[3];
                words.push(three_address(UNSIGNED_MULTIPLY_HIGH, left, right, scratch));
                words.push(three_address(MULTIPLY, left, right, value));
                words.push(0xf100_001f | (u32::from(scratch) << 5));
                words.push(0xda9f_0000 | (u32::from(value) << 5) | u32::from(value));
            }
            SaturatingRealization::OverflowI64 { subtract } => {
                let scratch = registers[3];
                words.push(0x937f_fc00 | (u32::from(left) << 5) | u32::from(scratch));
                words.push(0xd240_f800 | (u32::from(scratch) << 5) | u32::from(scratch));
                words.push(three_address(
                    if subtract { 0xeb00_0000 } else { 0xab00_0000 },
                    left,
                    right,
                    value,
                ));
                words.push(select(value, scratch, 0x6));
            }
            SaturatingRealization::DivideI64 => {
                let scratch = registers[3];
                words.push(three_address(0x9ac0_0c00, left, right, value));
                words.push(bound_word(i64::MIN as u64, scratch)?);
                words.push(0xeb00_001f | (u32::from(scratch) << 16) | (u32::from(value) << 5));
                words.push(0xba41_0800 | (u32::from(right) << 5));
                words.push(bound_word(i64::MAX as u64, scratch)?);
                words.push(select(value, scratch, 0x0));
            }
        }
        Ok(())
    }

    pub const AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET: u16 = 0;
    pub const AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET: u16 = 4;

    pub(crate) const fn byte_literal_address_words(register: u8) -> [u32; 2] {
        let register = register as u32;
        [
            0x9000_0000 | register,
            0x9100_0000 | (register << 5) | register,
        ]
    }
}
pub(crate) mod system_call {
    use super::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
        ValidatedAarch64SelectedFormEncoding,
    };
    use register_model::{
        RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel, views_with_access,
    };
    use selected_instructions::{
        MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
    };
    use target::NativeTarget;

    pub fn encode_aarch64_selected_system_call_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        number: i64,
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let mut bytes = Vec::with_capacity(20);
        for halfword in 0..4 {
            let immediate = ((number as u64 >> (halfword * 16)) & 0xffff) as u32;
            let opcode = if halfword == 0 {
                0xd280_0000
            } else {
                0xf280_0000
            };
            let word = opcode | (halfword << 21) | (immediate << 5) | 8;
            crate::codec_bytes::push_encoded(&mut bytes, word.to_le_bytes());
        }
        crate::codec_bytes::push_encoded(&mut bytes, 0xd400_0001u32.to_le_bytes());
        validate_aarch64_selected_system_call_form(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            operand_accesses,
            number,
            &bytes,
        )
    }

    pub fn validate_aarch64_selected_system_call_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        number: i64,
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        if target != NativeTarget::linux_arm64()
            || number < 0
            || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || !matches!(kind, SelectedInstructionKind::NormalizedForeignCall { .. })
            || alternative != kind.call_alternative_key()
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        if bytes.len() != 20 {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let mut decoded_number = 0u64;
        for (halfword, chunk) in bytes[..16].as_chunks::<4>().0.iter().enumerate() {
            let word = u32::from_le_bytes(*chunk);
            let opcode = if halfword == 0 {
                0xd280_0000
            } else {
                0xf280_0000
            };
            if word & !0x001f_ffe0 != opcode | ((halfword as u32) << 21) | 8 {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            decoded_number |= u64::from((word >> 5) & 0xffff) << (halfword * 16);
        }
        if bytes[16..] != [0x01, 0x00, 0x00, 0xd4] || decoded_number != number as u64 {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let catalog = crate::aarch64_register_constraint_catalog(physical);
        let keys = crate::aarch64_linux_system_call_keys();
        let row = catalog
            .row_matching(&keys, operand_views, operand_accesses)
            .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        let effects = MachineEncodedEffects::supervisor_call_v1(row);
        let views_for = |access| views_with_access(operand_views, operand_accesses, access);
        Ok(ValidatedAarch64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: Aarch64SelectedFormFootprint {
                register_reads: views_for(RegisterOperandAccess::Use),
                register_writes: views_for(RegisterOperandAccess::Def),
                writes_nzcv: true,
                encoded: effects,
            },
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use isa_conformance::system_call::{Encoding, conformance};
        use semantic_vocabulary::BoundaryMachineId;

        #[test]
        fn system_call_encoding_replays_all_transport_rows_and_rejects_substituted_bytes() {
            let physical = crate::validated_aarch64_physical_register_model();
            let target = NativeTarget::linux_arm64();
            let catalog = crate::aarch64_register_constraint_catalog(physical);
            let kind = SelectedInstructionKind::NormalizedForeignCall {
                boundary: BoundaryMachineId::new(3).unwrap(),
                ordinal: 0,
            };
            let alternative = MachineAlternativeKey {
                family: selected_instructions::MachineAlternativeFamily::NormalizedForeignCall,
                variant: 0,
            };
            let summarize = |encoded: ValidatedAarch64SelectedFormEncoding| Encoding {
                bytes: encoded.bytes().to_vec(),
                effects: encoded.footprint().encoded.clone(),
            };
            let rows: Vec<_> = crate::aarch64_linux_system_call_keys()
                .into_iter()
                .map(|key| catalog.constraint(key).unwrap())
                .collect();
            conformance(
                &rows,
                physical.model().view_named("x16").unwrap().id,
                |views, accesses, number| {
                    encode_aarch64_selected_system_call_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        views,
                        accesses,
                        number,
                    )
                    .map(summarize)
                },
                |views, accesses, number, bytes| {
                    validate_aarch64_selected_system_call_form(
                        target,
                        physical,
                        kind,
                        alternative,
                        views,
                        accesses,
                        number,
                        bytes,
                    )
                    .map(summarize)
                },
                |views, accesses, number, bytes| {
                    validate_aarch64_selected_system_call_form(
                        NativeTarget::macos_arm64(),
                        physical,
                        kind,
                        alternative,
                        views,
                        accesses,
                        number,
                        bytes,
                    )
                    .is_err()
                },
            );
        }
    }
}
pub(crate) mod table_call {
    use super::{
        Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
        ValidatedAarch64SelectedFormEncoding,
    };
    use register_model::{RegisterOperandAccess, RegisterViewId, views_with_access};
    use selected_instructions::{MachineAlternativeKey, SelectedNormalizedForeignCall};

    pub fn encode_aarch64_selected_table_call_form(
        record: &SelectedNormalizedForeignCall,
        alternative: MachineAlternativeKey,
        views: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let (base, offset) =
            table_address(record).ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        let mut bytes = Vec::with_capacity(20);
        for word in [
            0xd280_0011 | ((offset & 0xffff) << 5),
            0xf2a0_0011 | ((offset >> 16) << 5),
            0x8b11_0011 | (u32::from(base) << 5),
            0xf940_0231,
            0xd63f_0220,
        ] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        validate_aarch64_selected_table_call_form(record, alternative, views, accesses, &bytes)
    }

    pub fn validate_aarch64_selected_table_call_form(
        record: &SelectedNormalizedForeignCall,
        alternative: MachineAlternativeKey,
        views: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
        bytes: &[u8],
    ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
        let invalid = || Aarch64SelectedFormEncodingError::EncodedFormMismatch;
        let (base, offset) = table_address(record).ok_or_else(invalid)?;
        let words = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect::<Vec<_>>();
        let [low, high, add, load, call] = words.as_slice() else {
            return Err(invalid());
        };
        if bytes.len() != 20
            || low & 0xffe0_001f != 0xd280_0011
            || high & 0xffe0_001f != 0xf2a0_0011
            || ((low >> 5) & 0xffff) != offset & 0xffff
            || ((high >> 5) & 0xffff) != offset >> 16
            || *add != 0x8b11_0011 | (u32::from(base) << 5)
            || *load != 0xf940_0231
            || *call != 0xd63f_0220
        {
            return Err(invalid());
        }
        let row =
            crate::machine_effects::normalized_foreign_constraint(record).ok_or_else(invalid)?;
        if views.len() != row.operands.len()
            || accesses.len() != row.operands.len()
            || row.operands.iter().zip(views.iter().zip(accesses)).any(
                |(operand, (view, access))| {
                    operand.fixed_view != Some(*view) || operand.access != *access
                },
            )
        {
            return Err(invalid());
        }
        let declaration =
            crate::aarch64_normalized_foreign_call_declaration(record, &row).ok_or_else(invalid)?;
        let [form] = declaration.alternatives.as_slice() else {
            return Err(invalid());
        };
        if form.key != alternative {
            return Err(invalid());
        }
        let registers = |access| views_with_access(views, accesses, access);
        Ok(ValidatedAarch64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: Aarch64SelectedFormFootprint {
                register_reads: registers(RegisterOperandAccess::Use),
                register_writes: registers(RegisterOperandAccess::Def),
                writes_nzcv: true,
                encoded: form.encoded.clone(),
            },
        })
    }

    pub(crate) fn table_address(record: &SelectedNormalizedForeignCall) -> Option<(u8, u32)> {
        use calling_conventions::{MachineRegister, ValueLocation};
        let (byte_offset, dispatch_only) = record.call.binding.table_dispatch()?;
        if !record.call.callbacks.is_empty() {
            return None;
        }
        let plan = &record.call.binding.boundary_entry_plan.call;
        let scratch = |location: &ValueLocation| {
            location.direct_register() == Some(MachineRegister::Aarch64X(17))
                || dispatch_only
                    && location.direct_register() == Some(MachineRegister::Aarch64X(16))
        };
        if plan
            .parameters
            .iter()
            .flat_map(|parameter| &parameter.locations)
            .any(scratch)
            || plan
                .result
                .iter()
                .flat_map(|result| &result.locations)
                .any(|location| {
                    matches!(location, ValueLocation::Indirect { .. }) && scratch(location)
                })
        {
            return None;
        }
        let operands = record.call.binding.operand_placements();
        let parameter = operands.get(0)?;
        let [
            ValueLocation::Register {
                register: MachineRegister::Aarch64X(register),
                byte_size: 8,
                value_byte_offset: 0,
            },
        ] = parameter.locations.as_slice()
        else {
            return None;
        };
        (*register <= 30 && *register != 17).then_some((*register, byte_offset))
    }
}
#[cfg(test)]
mod tests {
    use crate::test_support::rejects_every_bit_flip;

    use super::{
        Aarch64ConditionalBranchForm, Aarch64SelectedFormEncodingError,
        encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
        encode_aarch64_selected_conditional_branch_form, encode_aarch64_selected_form,
        validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
        validate_aarch64_selected_conditional_branch_form, validate_aarch64_selected_form_encoding,
    };
    use crate::aarch64_physical_register_model;
    use crate::saturating_forms::SaturatingRealization;
    use crate::selected_form_encoding::decoding::DecodedWord;
    use crate::selected_form_encoding::decoding::decode_words;
    use optimization_core::AcceptedObligationFactIdentity;
    use register_model::validate_physical_register_model;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::MachineEncodedControlEffect;
    use selected_instructions::MachineEncodedEffects;
    use selected_instructions::MachineEncodedMemoryEffect;
    use selected_instructions::MachineEncodedStackEffect;
    use selected_instructions::MachineEncodedTrapBehavior;
    use selected_instructions::SelectedInstructionKind;
    use selected_instructions::{SaturatingCarrier, SaturatingOperation};
    use semantic_vocabulary::{IntegerValue, MachineId, ObligationId};

    mod control {
        use super::*;

        #[test]
        fn ret_x30_is_exact_and_separates_abi_result_custody_from_encoded_effects() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let x0 = physical.model().view_named("x0").unwrap().id;
            let x1 = physical.model().view_named("x1").unwrap().id;
            let x30 = physical.model().view_named("x30").unwrap();
            let pc = physical.model().view_named("pc").unwrap();
            let kind = SelectedInstructionKind::ReturnScalar;
            let alternative = alternative(MachineAlternativeFamily::ReturnScalar);
            let encoded =
                encode_aarch64_selected_form(&physical, kind, alternative, &[x0]).unwrap();

            assert_eq!(encoded.bytes(), [0xc0, 0x03, 0x5f, 0xd6]);
            assert!(encoded.footprint().register_reads.is_empty());
            assert!(encoded.footprint().register_writes.is_empty());
            assert!(
                encoded
                    .footprint()
                    .encoded
                    .external_operand_reads
                    .is_empty()
            );
            assert!(
                encoded
                    .footprint()
                    .encoded
                    .external_operand_writes
                    .is_empty()
            );
            assert_eq!(encoded.footprint().encoded.implicit_unit_uses, x30.units);
            assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
            assert_eq!(
                encoded.footprint().encoded.memory,
                MachineEncodedMemoryEffect::NoneV1
            );
            assert_eq!(
                encoded.footprint().encoded.stack,
                MachineEncodedStackEffect::UnchangedV1
            );
            assert_eq!(
                encoded.footprint().encoded.trap,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            );
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target: x30.id }
            );
            assert!(encode_aarch64_selected_form(&physical, kind, alternative, &[x1]).is_err());
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &[x0],
                    &0xd65f_03a0_u32.to_le_bytes()
                )
                .is_err()
            );
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &[x0],
                    &[0xc0, 0x03, 0x5f, 0xd6, 0xc0, 0x03, 0x5f, 0xd6]
                )
                .is_err()
            );
        }

        #[test]
        fn unit_return_is_a_distinct_zero_operand_ret_x30() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let kind = SelectedInstructionKind::ReturnUnit;
            let return_alternative = alternative(MachineAlternativeFamily::ReturnUnit);
            let encoded =
                encode_aarch64_selected_form(&physical, kind, return_alternative, &[]).unwrap();

            assert_eq!(encoded.bytes(), [0xc0, 0x03, 0x5f, 0xd6]);
            assert!(encoded.footprint().register_reads.is_empty());
            assert!(encoded.footprint().register_writes.is_empty());
            assert!(matches!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. }
            ));
            assert!(
                encode_aarch64_selected_form(
                    &physical,
                    kind,
                    alternative(MachineAlternativeFamily::ReturnScalar),
                    &[]
                )
                .is_err()
            );
        }

        #[test]
        fn nonzero_branch_has_exact_instruction_relative_imm19_and_effects() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero);
            for displacement in [-1_048_576, -4, 0, 4, 1_048_572] {
                let encoded = encode_aarch64_selected_conditional_branch_form(
                    &physical,
                    alternative,
                    MachineAlternativeFamily::ConditionalBranchNonZero,
                    Aarch64ConditionalBranchForm::CanonicalWordV1,
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), 4);
                assert_eq!(encoded.bytes()[0] & 0x1f, 1);
                assert!(encoded.footprint().register_reads.is_empty());
                assert!(encoded.footprint().register_writes.is_empty());
                assert_eq!(
                    encoded.footprint().encoded.control,
                    MachineEncodedControlEffect::ConditionalRelativeBranchV1
                );
            }
            for displacement in [-1_048_580, 1_048_576, 2] {
                assert!(
                    encode_aarch64_selected_conditional_branch_form(
                        &physical,
                        alternative,
                        MachineAlternativeFamily::ConditionalBranchNonZero,
                        Aarch64ConditionalBranchForm::CanonicalWordV1,
                        displacement
                    )
                    .is_err()
                );
            }
            assert!(
                validate_aarch64_selected_conditional_branch_form(
                    &physical,
                    alternative,
                    MachineAlternativeFamily::ConditionalBranchNonZero,
                    Aarch64ConditionalBranchForm::CanonicalWordV1,
                    0,
                    &0x5400_0000_u32.to_le_bytes()
                )
                .is_err()
            );
            assert!(
                validate_aarch64_selected_conditional_branch_form(
                    &physical,
                    alternative,
                    MachineAlternativeFamily::ConditionalBranchNonZero,
                    Aarch64ConditionalBranchForm::CanonicalWordV1,
                    0,
                    &[1, 0, 0, 0, 0]
                )
                .is_err()
            );
        }

        #[test]
        fn u64_less_than_branch_is_exact_b_lo_imm19_with_flag_control_effects() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let nzcv = physical.model().view_named("nzcv").unwrap();
            let pc = physical.model().view_named("pc").unwrap();
            let alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan);
            for (displacement, expected) in [
                (-4, [0xe3, 0xff, 0xff, 0x54]),
                (0, [0x03, 0x00, 0x00, 0x54]),
                (4, [0x23, 0x00, 0x00, 0x54]),
            ] {
                let encoded = encode_aarch64_selected_conditional_branch_form(
                    &physical,
                    alternative,
                    MachineAlternativeFamily::ConditionalBranchU64LessThan,
                    Aarch64ConditionalBranchForm::CanonicalWordV1,
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.bytes(), expected);
                assert!(encoded.footprint().register_reads.is_empty());
                assert!(encoded.footprint().register_writes.is_empty());
                assert_eq!(
                    encoded.footprint().encoded.control,
                    MachineEncodedControlEffect::ConditionalRelativeBranchV1
                );
                assert!(nzcv.units.iter().all(|unit| {
                    encoded
                        .footprint()
                        .encoded
                        .implicit_unit_uses
                        .contains(unit)
                }));
                assert!(pc.units.iter().all(|unit| {
                    encoded
                        .footprint()
                        .encoded
                        .implicit_unit_uses
                        .contains(unit)
                }));
                assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
            }
            for displacement in [-1_048_576, 1_048_572] {
                assert!(
                    encode_aarch64_selected_conditional_branch_form(
                        &physical,
                        alternative,
                        MachineAlternativeFamily::ConditionalBranchU64LessThan,
                        Aarch64ConditionalBranchForm::CanonicalWordV1,
                        displacement,
                    )
                    .is_ok()
                );
            }
            for displacement in [-1_048_580, 1_048_576, 2] {
                assert!(
                    encode_aarch64_selected_conditional_branch_form(
                        &physical,
                        alternative,
                        MachineAlternativeFamily::ConditionalBranchU64LessThan,
                        Aarch64ConditionalBranchForm::CanonicalWordV1,
                        displacement,
                    )
                    .is_err()
                );
            }
            for bytes in [
                0x5400_0002_u32.to_le_bytes(),
                0x5400_0009_u32.to_le_bytes(),
                0x1400_0003_u32.to_le_bytes(),
            ] {
                assert_eq!(
                    validate_aarch64_selected_conditional_branch_form(
                        &physical,
                        alternative,
                        MachineAlternativeFamily::ConditionalBranchU64LessThan,
                        Aarch64ConditionalBranchForm::CanonicalWordV1,
                        0,
                        &bytes,
                    ),
                    Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
                );
            }
            assert_eq!(
                validate_aarch64_selected_conditional_branch_form(
                    &physical,
                    alternative,
                    MachineAlternativeFamily::ConditionalBranchU64LessThan,
                    Aarch64ConditionalBranchForm::CanonicalWordV1,
                    0,
                    &0x5400_0023_u32.to_le_bytes(),
                ),
                Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
            );
        }

        #[test]
        fn i64_less_than_branch_is_exact_b_lt_imm19() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let alternative = alternative(MachineAlternativeFamily::ConditionalBranchI64LessThan);
            for (displacement, expected) in [
                (-4, [0xeb, 0xff, 0xff, 0x54]),
                (0, [0x0b, 0x00, 0x00, 0x54]),
                (4, [0x2b, 0x00, 0x00, 0x54]),
            ] {
                let encoded = encode_aarch64_selected_conditional_branch_form(
                    &physical,
                    alternative,
                    MachineAlternativeFamily::ConditionalBranchI64LessThan,
                    Aarch64ConditionalBranchForm::CanonicalWordV1,
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.bytes(), expected);
                assert_eq!(
                    encoded.footprint().encoded.control,
                    MachineEncodedControlEffect::ConditionalRelativeBranchV1
                );
            }
            assert_eq!(
                validate_aarch64_selected_conditional_branch_form(
                    &physical,
                    alternative,
                    MachineAlternativeFamily::ConditionalBranchI64LessThan,
                    Aarch64ConditionalBranchForm::CanonicalWordV1,
                    0,
                    &0x5400_0003_u32.to_le_bytes(),
                ),
                Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
            );
        }

        #[test]
        fn fused_cbnz_is_exact_rejects_nearby_opcodes_and_does_not_read_nzcv() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let x0 = physical.model().view_named("x0").unwrap();
            let x30 = physical.model().view_named("x30").unwrap();
            let pc = physical.model().view_named("pc").unwrap();
            let nzcv = physical.model().view_named("nzcv").unwrap();

            for (source, register) in [(x0.id, 0_u8), (x30.id, 30_u8)] {
                for displacement in [-1_048_576, -4, 0, 4, 1_048_572] {
                    let encoded =
                        encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                            &physical,
                            source,
                            displacement,
                        )
                        .unwrap();
                    assert_eq!(encoded.bytes().len(), 4);
                    assert_eq!(encoded.bytes()[0] & 0x1f, register);
                    assert_eq!(encoded.footprint().register_reads, [source]);
                    assert!(encoded.footprint().register_writes.is_empty());
                    assert!(!encoded.footprint().writes_nzcv);
                    assert!(
                        encoded
                            .footprint()
                            .encoded
                            .external_operand_reads
                            .is_empty()
                    );
                    assert_eq!(encoded.footprint().encoded.implicit_unit_uses, pc.units);
                    assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
                    assert!(
                        encoded
                            .footprint()
                            .encoded
                            .implicit_unit_uses
                            .iter()
                            .all(|unit| !nzcv.units.contains(unit))
                    );
                }
            }

            for displacement in [-1_048_580, 1_048_576, 2] {
                assert!(
                    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                        &physical,
                        x0.id,
                        displacement,
                    )
                    .is_err()
                );
            }
            for word in [0xb400_0000_u32, 0x3500_0000_u32, 0xb500_001e_u32] {
                assert!(
                    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                        &physical,
                        x0.id,
                        0,
                        &word.to_le_bytes(),
                    )
                    .is_err()
                );
            }
            assert!(
                validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                    &physical,
                    x0.id,
                    0,
                    &[0, 0, 0, 0, 0],
                )
                .is_err()
            );
        }

        #[test]
        fn widened_branch_forms_encode_inverted_skip_plus_unconditional_imm26() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let cases: [(MachineAlternativeFamily, u32); 3] = [
                (MachineAlternativeFamily::ConditionalBranchNonZero, 0x0),
                (MachineAlternativeFamily::ConditionalBranchU64LessThan, 0x2),
                (MachineAlternativeFamily::ConditionalBranchI64LessThan, 0xa),
            ];
            for (family, inverted_condition) in cases {
                let alternative = alternative(family);
                for row_displacement in [-1_048_580, -4, 4, 1_048_576, 134_217_724, 134_217_728] {
                    let encoded = encode_aarch64_selected_conditional_branch_form(
                        &physical,
                        alternative,
                        family,
                        Aarch64ConditionalBranchForm::WidenedPairV1,
                        row_displacement,
                    )
                    .unwrap();
                    assert_eq!(encoded.bytes().len(), 8);
                    let skip = u32::from_le_bytes(encoded.bytes()[0..4].try_into().unwrap());
                    assert_eq!(skip, 0x5400_0000 | (2 << 5) | inverted_condition);
                    let target = u32::from_le_bytes(encoded.bytes()[4..8].try_into().unwrap());
                    assert_eq!(target & 0xfc00_0000, 0x1400_0000);
                    let decoded = i64::from(((target & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
                    assert_eq!(decoded, row_displacement - 4);
                    assert_eq!(
                        encoded.footprint().encoded.control,
                        MachineEncodedControlEffect::ConditionalRelativeBranchV1
                    );
                }
                for displacement in [-134_217_732, 134_217_732, 2] {
                    assert!(
                        encode_aarch64_selected_conditional_branch_form(
                            &physical,
                            alternative,
                            family,
                            Aarch64ConditionalBranchForm::WidenedPairV1,
                            displacement,
                        )
                        .is_err()
                    );
                }
            }
        }

        #[test]
        fn widened_branch_forms_decode_and_reject_corruption() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let cases: [(MachineAlternativeFamily, u32); 3] = [
                (MachineAlternativeFamily::ConditionalBranchNonZero, 0x0),
                (MachineAlternativeFamily::ConditionalBranchU64LessThan, 0x2),
                (MachineAlternativeFamily::ConditionalBranchI64LessThan, 0xa),
            ];
            for (family, inverted_condition) in cases {
                let alternative = alternative(family);
                let row_displacement = 2_000_000;
                let check = |displacement, bytes: &[u8]| {
                    validate_aarch64_selected_conditional_branch_form(
                        &physical,
                        alternative,
                        family,
                        Aarch64ConditionalBranchForm::WidenedPairV1,
                        displacement,
                        bytes,
                    )
                };
                let encoded = encode_aarch64_selected_conditional_branch_form(
                    &physical,
                    alternative,
                    family,
                    Aarch64ConditionalBranchForm::WidenedPairV1,
                    row_displacement,
                )
                .unwrap();
                let round_trip = check(row_displacement, encoded.bytes()).unwrap();
                assert_eq!(round_trip.bytes(), encoded.bytes());
                assert_eq!(
                    check(0, &encoded.bytes()[0..4]),
                    Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
                );
                let wrong_skip = 0x5400_0000_u32 | (2 << 5) | (inverted_condition ^ 0xf);
                let mut corrupt = encoded.bytes().to_vec();
                crate::codec_bytes::patch_encoded(&mut corrupt, 0, wrong_skip.to_le_bytes());
                assert_eq!(
                    check(row_displacement, &corrupt),
                    Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
                );
                let mut corrupt = encoded.bytes().to_vec();
                crate::codec_bytes::patch_encoded(&mut corrupt, 4, 0x5400_000b_u32.to_le_bytes());
                assert_eq!(
                    check(row_displacement, &corrupt),
                    Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
                );
                let other = encode_aarch64_selected_conditional_branch_form(
                    &physical,
                    alternative,
                    family,
                    Aarch64ConditionalBranchForm::WidenedPairV1,
                    row_displacement - 8,
                )
                .unwrap();
                assert_eq!(
                    check(row_displacement, other.bytes()),
                    Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
                );
                let foreign = self::alternative(MachineAlternativeFamily::Jump);
                assert_eq!(
                    encode_aarch64_selected_conditional_branch_form(
                        &physical,
                        foreign,
                        family,
                        Aarch64ConditionalBranchForm::WidenedPairV1,
                        row_displacement,
                    )
                    .map(|encoding| encoding.bytes().len()),
                    Err(Aarch64SelectedFormEncodingError::AlternativeMismatch)
                );
            }
        }
    }
    mod saturating {
        use super::*;
        use super::{SaturatingCarrier as Carrier, SaturatingOperation as Op};

        #[test]
        fn saturating_forms_bind_registers_condition_and_flag_effects() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            for (kind, family) in [
                (
                    SelectedInstructionKind::SaturatingSubtract {
                        carrier: SaturatingCarrier::U64,
                    },
                    MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::U64),
                ),
                (
                    SelectedInstructionKind::SaturatingAdd {
                        carrier: SaturatingCarrier::U64,
                    },
                    MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::U64),
                ),
            ] {
                let key = alternative(family);
                for left in ["x0", "x1", "x9", "x29"] {
                    for right in ["x0", "x1", "x9", "x29"] {
                        for destination in ["x0", "x1", "x9", "x29"] {
                            let operands = [left, right, destination]
                                .map(|name| physical.model().view_named(name).unwrap().id);
                            let encoded =
                                encode_aarch64_selected_form(&physical, kind, key, &operands)
                                    .unwrap();
                            assert_eq!(encoded.bytes().len(), 8);
                            let decoded = validate_aarch64_selected_form_encoding(
                                &physical,
                                kind,
                                key,
                                &operands,
                                encoded.bytes(),
                            )
                            .unwrap();
                            assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
                            assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
                            assert!(!decoded.footprint().encoded.implicit_unit_defs.is_empty());
                            for byte in 0..encoded.bytes().len() {
                                let mut changed = encoded.bytes().to_vec();
                                changed[byte] ^= 1;
                                assert!(
                                    validate_aarch64_selected_form_encoding(
                                        &physical, kind, key, &operands, &changed
                                    )
                                    .is_err(),
                                    "changed byte {byte}"
                                );
                            }
                        }
                    }
                }
            }
        }

        #[test]
        fn saturation_high_register_bytes_match_independent_assembler() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let operands =
                ["x9", "x10", "x11"].map(|name| physical.model().view_named(name).unwrap().id);
            for (kind, family, bytes) in [
                (
                    SelectedInstructionKind::SaturatingSubtract {
                        carrier: SaturatingCarrier::U64,
                    },
                    MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::U64),
                    [0xeb0a012bu32, 0x9a9f216b]
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect::<Vec<_>>(),
                ),
                (
                    SelectedInstructionKind::SaturatingAdd {
                        carrier: SaturatingCarrier::U64,
                    },
                    MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::U64),
                    [0xab0a012bu32, 0xda9f316b]
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect::<Vec<_>>(),
                ),
            ] {
                let key = alternative(family);
                let encoded =
                    encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
                assert_eq!(encoded.bytes(), bytes);
                validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, &bytes)
                    .unwrap();
            }
        }

        fn saturating_kinds() -> Vec<(
            SaturatingOperation,
            SaturatingCarrier,
            SelectedInstructionKind,
            MachineAlternativeFamily,
        )> {
            let mut kinds = Vec::new();
            for carrier in SaturatingCarrier::ALL {
                kinds.push((
                    SaturatingOperation::Add,
                    carrier,
                    SelectedInstructionKind::SaturatingAdd { carrier },
                    MachineAlternativeFamily::SaturatingAdd(carrier),
                ));
                kinds.push((
                    SaturatingOperation::Subtract,
                    carrier,
                    SelectedInstructionKind::SaturatingSubtract { carrier },
                    MachineAlternativeFamily::SaturatingSubtract(carrier),
                ));
                kinds.push((
                    SaturatingOperation::Divide,
                    carrier,
                    SelectedInstructionKind::SaturatingDivide {
                        carrier,
                        obligation: ObligationId::new(1).unwrap(),
                        accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                    MachineAlternativeFamily::SaturatingDivide(carrier),
                ));
                kinds.push((
                    SaturatingOperation::Remainder,
                    carrier,
                    SelectedInstructionKind::SaturatingRemainder {
                        carrier,
                        obligation: ObligationId::new(1).unwrap(),
                        accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                    MachineAlternativeFamily::SaturatingRemainder(carrier),
                ));
                kinds.push((
                    SaturatingOperation::Multiply,
                    carrier,
                    SelectedInstructionKind::SaturatingMultiply { carrier },
                    MachineAlternativeFamily::SaturatingMultiply(carrier),
                ));
            }
            kinds
        }

        const fn words(
            operation: SaturatingOperation,
            carrier: SaturatingCarrier,
            encoded: &'static [u32],
        ) -> (SaturatingOperation, SaturatingCarrier, &'static [u32]) {
            (operation, carrier, encoded)
        }

        const CLANG_SATURATING_WORDS: [(SaturatingOperation, SaturatingCarrier, &[u32]); 40] = [
            words(
                Op::Add,
                Carrier::I8,
                &[
                    0x8b02_0023,
                    0xb240_1be4,
                    0xeb04_007f,
                    0x9a83_c083,
                    0xb279_e3e4,
                    0xeb04_007f,
                    0x9a83_b083,
                ],
            ),
            words(
                Op::Add,
                Carrier::I16,
                &[
                    0x8b02_0023,
                    0xb240_3be4,
                    0xeb04_007f,
                    0x9a83_c083,
                    0xb271_c3e4,
                    0xeb04_007f,
                    0x9a83_b083,
                ],
            ),
            words(
                Op::Add,
                Carrier::I32,
                &[
                    0x8b02_0023,
                    0xb240_7be4,
                    0xeb04_007f,
                    0x9a83_c083,
                    0xb261_83e4,
                    0xeb04_007f,
                    0x9a83_b083,
                ],
            ),
            words(
                Op::Add,
                Carrier::I64,
                &[0x937f_fc24, 0xd240_f884, 0xab02_0023, 0x9a83_6083],
            ),
            words(
                Op::Add,
                Carrier::U8,
                &[0x8b02_0023, 0xb240_1fe4, 0xeb04_007f, 0x9a83_c083],
            ),
            words(
                Op::Add,
                Carrier::U16,
                &[0x8b02_0023, 0xb240_3fe4, 0xeb04_007f, 0x9a83_c083],
            ),
            words(
                Op::Add,
                Carrier::U32,
                &[0x8b02_0023, 0xb240_7fe4, 0xeb04_007f, 0x9a83_c083],
            ),
            words(Op::Add, Carrier::U64, &[0xab02_0023, 0xda9f_3063]),
            words(
                Op::Subtract,
                Carrier::I8,
                &[
                    0xcb02_0023,
                    0xb240_1be4,
                    0xeb04_007f,
                    0x9a83_c083,
                    0xb279_e3e4,
                    0xeb04_007f,
                    0x9a83_b083,
                ],
            ),
            words(
                Op::Subtract,
                Carrier::I16,
                &[
                    0xcb02_0023,
                    0xb240_3be4,
                    0xeb04_007f,
                    0x9a83_c083,
                    0xb271_c3e4,
                    0xeb04_007f,
                    0x9a83_b083,
                ],
            ),
            words(
                Op::Subtract,
                Carrier::I32,
                &[
                    0xcb02_0023,
                    0xb240_7be4,
                    0xeb04_007f,
                    0x9a83_c083,
                    0xb261_83e4,
                    0xeb04_007f,
                    0x9a83_b083,
                ],
            ),
            words(
                Op::Subtract,
                Carrier::I64,
                &[0x937f_fc24, 0xd240_f884, 0xeb02_0023, 0x9a83_6083],
            ),
            words(Op::Subtract, Carrier::U8, &[0xeb02_0023, 0x9a9f_2063]),
            words(Op::Subtract, Carrier::U16, &[0xeb02_0023, 0x9a9f_2063]),
            words(Op::Subtract, Carrier::U32, &[0xeb02_0023, 0x9a9f_2063]),
            words(Op::Subtract, Carrier::U64, &[0xeb02_0023, 0x9a9f_2063]),
            words(
                Op::Divide,
                Carrier::I8,
                &[0x9ac2_0c23, 0xb240_1be4, 0xeb04_007f, 0x9a83_c083],
            ),
            words(
                Op::Divide,
                Carrier::I16,
                &[0x9ac2_0c23, 0xb240_3be4, 0xeb04_007f, 0x9a83_c083],
            ),
            words(
                Op::Divide,
                Carrier::I32,
                &[0x9ac2_0c23, 0xb240_7be4, 0xeb04_007f, 0x9a83_c083],
            ),
            words(
                Op::Divide,
                Carrier::I64,
                &[
                    0x9ac2_0c23,
                    0xb241_03e4,
                    0xeb04_007f,
                    0xba41_0840,
                    0xb240_fbe4,
                    0x9a83_0083,
                ],
            ),
            words(Op::Divide, Carrier::U8, &[0x9ac2_0823]),
            words(Op::Divide, Carrier::U16, &[0x9ac2_0823]),
            words(Op::Divide, Carrier::U32, &[0x9ac2_0823]),
            words(Op::Divide, Carrier::U64, &[0x9ac2_0823]),
            words(Op::Remainder, Carrier::I8, &[0x9ac2_0c23, 0x9b02_8463]),
            words(Op::Remainder, Carrier::I16, &[0x9ac2_0c23, 0x9b02_8463]),
            words(Op::Remainder, Carrier::I32, &[0x9ac2_0c23, 0x9b02_8463]),
            words(Op::Remainder, Carrier::I64, &[0x9ac2_0c23, 0x9b02_8463]),
            words(Op::Remainder, Carrier::U8, &[0x9ac2_0823, 0x9b02_8463]),
            words(Op::Remainder, Carrier::U16, &[0x9ac2_0823, 0x9b02_8463]),
            words(Op::Remainder, Carrier::U32, &[0x9ac2_0823, 0x9b02_8463]),
            words(Op::Remainder, Carrier::U64, &[0x9ac2_0823, 0x9b02_8463]),
            words(
                Op::Multiply,
                Carrier::I8,
                &[
                    0x9b02_7c23,
                    0xb240_1be4,
                    0xeb04_007f,
                    0x9a83_c083,
                    0xb279_e3e4,
                    0xeb04_007f,
                    0x9a83_b083,
                ],
            ),
            words(
                Op::Multiply,
                Carrier::I16,
                &[
                    0x9b02_7c23,
                    0xb240_3be4,
                    0xeb04_007f,
                    0x9a83_c083,
                    0xb271_c3e4,
                    0xeb04_007f,
                    0x9a83_b083,
                ],
            ),
            words(
                Op::Multiply,
                Carrier::I32,
                &[
                    0x9b02_7c23,
                    0xb240_7be4,
                    0xeb04_007f,
                    0x9a83_c083,
                    0xb261_83e4,
                    0xeb04_007f,
                    0x9a83_b083,
                ],
            ),
            words(
                Op::Multiply,
                Carrier::I64,
                &[
                    0x9b42_7c24,
                    0x9b02_7c23,
                    0xeb83_fc9f,
                    0x937f_fc84,
                    0xd240_f884,
                    0x9a83_1083,
                ],
            ),
            words(
                Op::Multiply,
                Carrier::U8,
                &[0x9b02_7c23, 0xb240_1fe4, 0xeb04_007f, 0x9a83_8083],
            ),
            words(
                Op::Multiply,
                Carrier::U16,
                &[0x9b02_7c23, 0xb240_3fe4, 0xeb04_007f, 0x9a83_8083],
            ),
            words(
                Op::Multiply,
                Carrier::U32,
                &[0x9b02_7c23, 0xb240_7fe4, 0xeb04_007f, 0x9a83_8083],
            ),
            words(
                Op::Multiply,
                Carrier::U64,
                &[0x9bc2_7c24, 0x9b02_7c23, 0xf100_009f, 0xda9f_0063],
            ),
        ];

        fn clang_words(operation: SaturatingOperation, carrier: SaturatingCarrier) -> Vec<u8> {
            CLANG_SATURATING_WORDS
                .iter()
                .find(|(row_operation, row_carrier, _)| {
                    *row_operation == operation && *row_carrier == carrier
                })
                .map(|(_, _, words)| words.iter().copied().flat_map(u32::to_le_bytes).collect())
                .unwrap()
        }

        #[test]
        fn every_saturating_carrier_matches_independent_assembler_and_rejects_mutation() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let names = ["x1", "x2", "x3", "x4"];
            for (operation, carrier, kind, family) in saturating_kinds() {
                let key = alternative(family);
                let expected = clang_words(operation, carrier);
                let operand_count = SaturatingRealization::of(operation, carrier).operand_count();
                let operands: Vec<_> = names[..operand_count]
                    .iter()
                    .map(|name| physical.model().view_named(name).unwrap().id)
                    .collect();
                let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands)
                    .unwrap_or_else(|error| panic!("{kind:?}: {error:?}"));
                assert_eq!(encoded.bytes(), expected, "{kind:?}");
                assert_eq!(encoded.footprint().register_reads, operands[..2]);
                assert_eq!(encoded.footprint().register_writes, operands[2..]);
                assert_eq!(
                    encoded.footprint().writes_nzcv,
                    !(operation == SaturatingOperation::Divide && !carrier.is_signed())
                        && operation != SaturatingOperation::Remainder,
                    "{kind:?}"
                );
                assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(
                    encoded.footprint().encoded.external_operand_writes,
                    (2..operand_count as u16).collect::<Vec<_>>()
                );
                for byte_position in 0..expected.len() {
                    let mut changed = expected.clone();
                    changed[byte_position] ^= 1;
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical, kind, key, &operands, &changed
                        )
                        .is_err(),
                        "{kind:?} mutated byte {byte_position}"
                    );
                }
                for operand_position in 0..operands.len() {
                    let mut changed = operands.clone();
                    changed[operand_position] = physical.model().view_named("x8").unwrap().id;
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical, kind, key, &changed, &expected
                        )
                        .is_err(),
                        "{kind:?} substituted operand {operand_position}"
                    );
                }
                for (other_operation, other_carrier, other_kind, other_family) in saturating_kinds()
                {
                    if (other_operation, other_carrier) == (operation, carrier) {
                        continue;
                    }
                    let same_words = clang_words(other_operation, other_carrier) == expected;
                    assert_eq!(
                        validate_aarch64_selected_form_encoding(
                            &physical,
                            other_kind,
                            alternative(other_family),
                            &operands,
                            &expected,
                        )
                        .is_ok(),
                        same_words,
                        "{kind:?} bytes under {other_kind:?}"
                    );
                }
            }
        }

        #[test]
        fn only_unsigned_subtract_divide_and_remainders_share_a_realization_across_carriers() {
            let mut shared = Vec::new();
            for (operation, carrier, _, _) in saturating_kinds() {
                let words = clang_words(operation, carrier);
                for (other_operation, other_carrier, _, _) in saturating_kinds() {
                    if (other_operation, other_carrier) != (operation, carrier)
                        && clang_words(other_operation, other_carrier) == words
                    {
                        shared.push((operation, carrier));
                        break;
                    }
                }
            }
            assert!(shared.iter().all(|(operation, carrier)| {
                (!carrier.is_signed()
                    && matches!(
                        operation,
                        SaturatingOperation::Subtract | SaturatingOperation::Divide
                    ))
                    || *operation == SaturatingOperation::Remainder
            }));
            assert_eq!(shared.len(), 16);
        }

        #[test]
        fn four_operand_saturating_forms_reject_aliased_outputs() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            for (operation, carrier, kind, family) in saturating_kinds() {
                let key = alternative(family);
                if SaturatingRealization::of(operation, carrier).operand_count() != 4 {
                    continue;
                }
                for names in [
                    ["x9", "x10", "x11", "x9"],
                    ["x9", "x10", "x11", "x10"],
                    ["x9", "x10", "x11", "x11"],
                    ["x9", "x10", "x9", "x12"],
                    ["x9", "x10", "x10", "x12"],
                ] {
                    let aliased = names.map(|name| physical.model().view_named(name).unwrap().id);
                    assert!(
                        encode_aarch64_selected_form(&physical, kind, key, &aliased).is_err(),
                        "{kind:?} {names:?}"
                    );
                }
            }
        }

        fn reference(
            operation: SaturatingOperation,
            carrier: SaturatingCarrier,
            left: i128,
            right: i128,
        ) -> i128 {
            let exact = match operation {
                SaturatingOperation::Add => left + right,
                SaturatingOperation::Subtract => left - right,
                SaturatingOperation::Divide => left / right,
                SaturatingOperation::Remainder => left % right,
                SaturatingOperation::Multiply => left.checked_mul(right).unwrap_or(i128::MAX),
            };
            let (minimum, maximum) = carrier_bounds(carrier);
            exact.clamp(minimum, maximum)
        }

        fn carrier_bounds(carrier: SaturatingCarrier) -> (i128, i128) {
            if carrier.is_signed() {
                (
                    i128::from(carrier.minimum_bits() as i64),
                    i128::from(carrier.maximum_bits() as i64),
                )
            } else {
                (0, i128::from(carrier.maximum_bits()))
            }
        }

        fn interpret(decoded: &[DecodedWord], left: i128, right: i128) -> i128 {
            let mut registers = [0_u64; 32];
            registers[1] = left as u64;
            registers[2] = right as u64;
            let (mut n, mut z, mut c, mut v) = (false, false, false, false);
            let read = |registers: &[u64; 32], index: u8| {
                if index == 31 {
                    0
                } else {
                    registers[index as usize]
                }
            };
            let set_flags = |n: &mut bool,
                             z: &mut bool,
                             c: &mut bool,
                             v: &mut bool,
                             left: u64,
                             right: u64,
                             subtract: bool|
             -> u64 {
                let (result, carry) = if subtract {
                    let (result, borrow) = left.overflowing_sub(right);
                    (result, !borrow)
                } else {
                    left.overflowing_add(right)
                };
                let overflow = if subtract {
                    (left as i64).overflowing_sub(right as i64).1
                } else {
                    (left as i64).overflowing_add(right as i64).1
                };
                *n = (result as i64) < 0;
                *z = result == 0;
                *c = carry;
                *v = overflow;
                result
            };
            for word in decoded {
                match *word {
                    DecodedWord::Add {
                        left,
                        right,
                        destination,
                    } => {
                        registers[destination as usize] =
                            read(&registers, left).wrapping_add(read(&registers, right));
                    }
                    DecodedWord::Subtract {
                        left,
                        right,
                        destination,
                    } => {
                        registers[destination as usize] =
                            read(&registers, left).wrapping_sub(read(&registers, right));
                    }
                    DecodedWord::SignedDivide {
                        dividend,
                        divisor,
                        destination,
                    } => {
                        registers[destination as usize] = (read(&registers, dividend) as i64)
                            .wrapping_div(read(&registers, divisor) as i64)
                            as u64;
                    }
                    DecodedWord::UnsignedDivide {
                        dividend,
                        divisor,
                        destination,
                    } => {
                        registers[destination as usize] =
                            read(&registers, dividend) / read(&registers, divisor);
                    }
                    DecodedWord::MultiplySubtract {
                        left,
                        right,
                        minuend,
                        destination,
                    } => {
                        registers[destination as usize] = read(&registers, minuend).wrapping_sub(
                            read(&registers, left).wrapping_mul(read(&registers, right)),
                        );
                    }
                    DecodedWord::AddWithFlags {
                        left,
                        right,
                        destination,
                    } => {
                        registers[destination as usize] = set_flags(
                            &mut n,
                            &mut z,
                            &mut c,
                            &mut v,
                            read(&registers, left),
                            read(&registers, right),
                            false,
                        );
                    }
                    DecodedWord::SubtractWithFlags {
                        left,
                        right,
                        destination,
                    } => {
                        registers[destination as usize] = set_flags(
                            &mut n,
                            &mut z,
                            &mut c,
                            &mut v,
                            read(&registers, left),
                            read(&registers, right),
                            true,
                        );
                    }
                    DecodedWord::Compare { left, right } => {
                        set_flags(
                            &mut n,
                            &mut z,
                            &mut c,
                            &mut v,
                            read(&registers, left),
                            read(&registers, right),
                            true,
                        );
                    }
                    DecodedWord::SelectMaximumOnCarry {
                        source,
                        destination,
                    } => {
                        registers[destination as usize] = if c {
                            u64::MAX
                        } else {
                            read(&registers, source)
                        };
                    }
                    DecodedWord::SelectZeroOnBorrow {
                        source,
                        destination,
                    } => {
                        registers[destination as usize] =
                            if c { read(&registers, source) } else { 0 };
                    }
                    DecodedWord::MaterializeBound { destination, value } => {
                        registers[destination as usize] = value;
                    }
                    DecodedWord::SelectOnGreater {
                        source,
                        destination,
                    } => {
                        if !z && n == v {
                            registers[destination as usize] = read(&registers, source);
                        }
                    }
                    DecodedWord::SelectOnLess {
                        source,
                        destination,
                    } => {
                        if n != v {
                            registers[destination as usize] = read(&registers, source);
                        }
                    }
                    DecodedWord::SelectOnEqual {
                        source,
                        destination,
                    } => {
                        if z {
                            registers[destination as usize] = read(&registers, source);
                        }
                    }
                    DecodedWord::SelectOnOverflow {
                        source,
                        destination,
                    } => {
                        if v {
                            registers[destination as usize] = read(&registers, source);
                        }
                    }
                    DecodedWord::ArithmeticShiftRight63 {
                        source,
                        destination,
                    } => {
                        registers[destination as usize] =
                            ((read(&registers, source) as i64) >> 63) as u64;
                    }
                    DecodedWord::ExclusiveOrI64Maximum { register } => {
                        registers[register as usize] ^= i64::MAX as u64;
                    }
                    DecodedWord::Multiply {
                        left,
                        right,
                        destination,
                    } => {
                        registers[destination as usize] =
                            read(&registers, left).wrapping_mul(read(&registers, right));
                    }
                    DecodedWord::SignedMultiplyHigh {
                        left,
                        right,
                        destination,
                    } => {
                        let product = i128::from(read(&registers, left) as i64)
                            * i128::from(read(&registers, right) as i64);
                        registers[destination as usize] = (product >> 64) as u64;
                    }
                    DecodedWord::UnsignedMultiplyHigh {
                        left,
                        right,
                        destination,
                    } => {
                        let product = u128::from(read(&registers, left))
                            * u128::from(read(&registers, right));
                        registers[destination as usize] = (product >> 64) as u64;
                    }
                    DecodedWord::CompareWithSignOf { high, low } => {
                        let sign = ((read(&registers, low) as i64) >> 63) as u64;
                        set_flags(
                            &mut n,
                            &mut z,
                            &mut c,
                            &mut v,
                            read(&registers, high),
                            sign,
                            true,
                        );
                    }
                    DecodedWord::CompareZero { source } => {
                        set_flags(
                            &mut n,
                            &mut z,
                            &mut c,
                            &mut v,
                            read(&registers, source),
                            0,
                            true,
                        );
                    }
                    DecodedWord::SelectOnNotEqual {
                        source,
                        destination,
                    } => {
                        if !z {
                            registers[destination as usize] = read(&registers, source);
                        }
                    }
                    DecodedWord::SelectOnHigher {
                        source,
                        destination,
                    } => {
                        if c && !z {
                            registers[destination as usize] = read(&registers, source);
                        }
                    }
                    DecodedWord::SelectMaximumUnlessEqual {
                        source,
                        destination,
                    } => {
                        registers[destination as usize] = if z {
                            read(&registers, source)
                        } else {
                            u64::MAX
                        };
                    }
                    DecodedWord::ConditionalCompareMinusOne { register } => {
                        if z {
                            set_flags(
                                &mut n,
                                &mut z,
                                &mut c,
                                &mut v,
                                read(&registers, register),
                                1,
                                false,
                            );
                        } else {
                            (n, z, c, v) = (false, false, false, false);
                        }
                    }
                    other => panic!("unexpected saturating word {other:?}"),
                }
            }
            assert_eq!((registers[1], registers[2]), (left as u64, right as u64));
            i128::from(registers[3] as i64)
        }

        #[test]
        fn every_saturating_carrier_clamps_its_edges_in_the_decoded_words() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            for (operation, carrier, kind, family) in saturating_kinds() {
                let operand_count = SaturatingRealization::of(operation, carrier).operand_count();
                let operands: Vec<_> = ["x1", "x2", "x3", "x4"][..operand_count]
                    .iter()
                    .map(|name| physical.model().view_named(name).unwrap().id)
                    .collect();
                let encoded =
                    encode_aarch64_selected_form(&physical, kind, alternative(family), &operands)
                        .unwrap();
                let decoded = decode_words(encoded.bytes()).unwrap();
                let (minimum, maximum) = carrier_bounds(carrier);
                let mut samples = vec![minimum, minimum + 1, 0, 1, 2, 7, 40, maximum - 1, maximum];
                if carrier.is_signed() {
                    samples.extend([-1, -2, -7, -40]);
                }
                for &left in &samples {
                    for &right in &samples {
                        if matches!(
                            operation,
                            SaturatingOperation::Divide | SaturatingOperation::Remainder
                        ) && right == 0
                        {
                            continue;
                        }
                        let observed = interpret(&decoded, left, right);
                        let observed = if carrier.is_signed() {
                            observed
                        } else {
                            i128::from(observed as u64)
                        };
                        assert_eq!(
                            observed,
                            reference(operation, carrier, left, right),
                            "{kind:?} {left} {right}"
                        );
                    }
                }
            }
        }
    }
    mod scalar {
        use super::super::ValidatedAarch64SelectedFormEncoding;
        use super::*;
        use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};

        fn check_encoding(
            physical: &ValidatedPhysicalRegisterModel,
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            bytes: &[u8],
        ) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError>
        {
            validate_aarch64_selected_form_encoding(physical, kind, alternative, operands, bytes)
        }

        #[test]
        fn scalar_call_is_explicitly_refused_before_encoding() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            assert_eq!(
                encode_aarch64_selected_form(
                    &physical,
                    SelectedInstructionKind::CallScalar {
                        target: selected_instructions::SelectedCallTarget::Machine(
                            MachineId::new(1).unwrap()
                        ),
                    },
                    alternative(MachineAlternativeFamily::ReturnUnit),
                    &[],
                ),
                Err(Aarch64SelectedFormEncodingError::LayoutDependentForm),
            );
        }

        #[test]
        fn zero_extend_u8_binds_width_registers_and_is_not_a_copy() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            for source in ["x0", "x1", "x9", "x16", "x29"] {
                for destination in ["x0", "x1", "x9", "x16", "x29"] {
                    let operands = [
                        physical.model().view_named(source).unwrap().id,
                        physical.model().view_named(destination).unwrap().id,
                    ];
                    let kind = SelectedInstructionKind::ZeroExtendU8;
                    let key = alternative(MachineAlternativeFamily::ZeroExtendU8);
                    let encoded =
                        encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
                    assert_eq!(encoded.bytes().len(), 4);
                    check_encoding(&physical, kind, key, &operands, encoded.bytes()).unwrap();
                    let copy = encode_aarch64_selected_form(
                        &physical,
                        SelectedInstructionKind::CopyI64,
                        alternative(MachineAlternativeFamily::CopyI64),
                        &operands,
                    )
                    .unwrap();
                    assert!(check_encoding(&physical, kind, key, &operands, copy.bytes()).is_err());
                    for byte in 0..4 {
                        let mut changed = encoded.bytes().to_vec();
                        changed[byte] ^= 1;
                        assert!(check_encoding(&physical, kind, key, &operands, &changed).is_err());
                    }
                }
            }
        }

        #[test]
        fn zero_extend_u32_binds_width_registers_and_is_not_a_copy() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            for source in ["x0", "x1", "x9", "x16", "x29"] {
                for destination in ["x0", "x1", "x9", "x16", "x29"] {
                    let operands = [
                        physical.model().view_named(source).unwrap().id,
                        physical.model().view_named(destination).unwrap().id,
                    ];
                    let kind = SelectedInstructionKind::ZeroExtendU32;
                    let key = alternative(MachineAlternativeFamily::ZeroExtendU32);
                    let encoded =
                        encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
                    assert_eq!(encoded.bytes().len(), 4);
                    let word = u32::from_le_bytes(encoded.bytes().try_into().unwrap());
                    assert_eq!(word & 0xffff_fc00, 0xd340_7c00);
                    check_encoding(&physical, kind, key, &operands, encoded.bytes()).unwrap();
                    let narrow = encode_aarch64_selected_form(
                        &physical,
                        SelectedInstructionKind::ZeroExtendU8,
                        alternative(MachineAlternativeFamily::ZeroExtendU8),
                        &operands,
                    )
                    .unwrap();
                    assert!(
                        check_encoding(&physical, kind, key, &operands, narrow.bytes()).is_err()
                    );
                    let copy = encode_aarch64_selected_form(
                        &physical,
                        SelectedInstructionKind::CopyI64,
                        alternative(MachineAlternativeFamily::CopyI64),
                        &operands,
                    )
                    .unwrap();
                    assert!(check_encoding(&physical, kind, key, &operands, copy.bytes()).is_err());
                    for byte in 0..4 {
                        let mut changed = encoded.bytes().to_vec();
                        changed[byte] ^= 1;
                        assert!(check_encoding(&physical, kind, key, &operands, &changed).is_err());
                    }
                }
            }
        }

        #[test]
        fn zero_seeded_materialization_is_canonical_and_decoded_independently() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let x9 = physical.model().view_named("x9").unwrap().id;
            for (value, byte_count) in [
                (IntegerValue::Unsigned(0), 4),
                (IntegerValue::Unsigned(u64::MAX as u128), 16),
                (IntegerValue::Unsigned(0x1234_0000_5678_0000), 12),
                (IntegerValue::Unsigned(0x1234_5678_9abc_def0), 16),
            ] {
                let kind = SelectedInstructionKind::MaterializeI64 { value };
                let encoded = encode_aarch64_selected_form(
                    &physical,
                    kind,
                    alternative(MachineAlternativeFamily::MaterializeI64),
                    &[x9],
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), byte_count);
                let mut corrupted = encoded.bytes().to_vec();
                corrupted[0] ^= 0x20;
                assert!(
                    check_encoding(
                        &physical,
                        kind,
                        alternative(MachineAlternativeFamily::MaterializeI64),
                        &[x9],
                        &corrupted,
                    )
                    .is_err()
                );
            }
        }

        #[test]
        fn bitwise_xor_preserves_exact_eor_registers_aliases_and_effects() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let registers = [("x3", 3_u32), ("x9", 9), ("x20", 20)]
                .map(|(name, number)| (physical.model().view_named(name).unwrap().id, number));
            let kind = SelectedInstructionKind::BitwiseXorI64;
            let alternative = alternative(MachineAlternativeFamily::BitwiseXorI64);
            for (left, left_number) in registers {
                for (right, right_number) in registers {
                    for (output, output_number) in registers {
                        let operands = [left, right, output];
                        let expected =
                            0xca00_0000 | (right_number << 16) | (left_number << 5) | output_number;
                        let encoded =
                            encode_aarch64_selected_form(&physical, kind, alternative, &operands)
                                .unwrap();
                        assert_eq!(encoded.bytes(), expected.to_le_bytes());
                        let footprint = encoded.footprint();
                        assert_eq!(footprint.register_reads, [left, right]);
                        assert_eq!(footprint.register_writes, [output]);
                        assert!(!footprint.writes_nzcv);
                        assert_eq!(footprint.encoded.external_operand_reads, [0, 1]);
                        assert_eq!(footprint.encoded.external_operand_writes, [2]);
                        assert_eq!(footprint.encoded.memory, MachineEncodedMemoryEffect::NoneV1);
                        for mask in [
                            1_u32,
                            1 << 5,
                            1 << 16,
                            1 << 30,
                            1 << 29,
                            1 << 21,
                            1 << 31,
                            1 << 10,
                        ] {
                            assert!(
                                check_encoding(
                                    &physical,
                                    kind,
                                    alternative,
                                    &operands,
                                    &(expected ^ mask).to_le_bytes(),
                                )
                                .is_err(),
                                "mutation {mask:#x}"
                            );
                        }
                        assert!(
                            check_encoding(
                                &physical,
                                kind,
                                alternative,
                                &operands,
                                &encoded.bytes()[..3],
                            )
                            .is_err()
                        );
                        let duplicated = [encoded.bytes(), encoded.bytes()].concat();
                        assert!(
                            check_encoding(&physical, kind, alternative, &operands, &duplicated,)
                                .is_err()
                        );
                    }
                }
            }
            let operands = registers.map(|(view, _)| view);
            let wrong_alternative = self::alternative(MachineAlternativeFamily::BitwiseAndI64);
            assert!(
                encode_aarch64_selected_form(&physical, kind, wrong_alternative, &operands)
                    .is_err()
            );
        }

        #[test]
        fn wrapping_add_preserves_aliases_and_rejects_exact_family_or_changed_bytes() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let views =
                ["x3", "x9", "x20"].map(|name| physical.model().view_named(name).unwrap().id);
            let kind = SelectedInstructionKind::WrappingAddI64;
            let key = alternative(MachineAlternativeFamily::WrappingAddI64);
            for left in views {
                for right in views {
                    for output in views {
                        let operands = [left, right, output];
                        let encoded =
                            encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
                        check_encoding(&physical, kind, key, &operands, encoded.bytes()).unwrap();
                        assert!(!encoded.footprint().writes_nzcv);
                        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                        assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                        let mut changed = encoded.bytes().to_vec();
                        changed[0] ^= 1;
                        assert!(check_encoding(&physical, kind, key, &operands, &changed).is_err());
                        assert!(
                            encode_aarch64_selected_form(
                                &physical,
                                kind,
                                alternative(MachineAlternativeFamily::ExactAddI64),
                                &operands
                            )
                            .is_err()
                        );
                        assert!(
                            check_encoding(
                                &physical,
                                kind,
                                alternative(MachineAlternativeFamily::ExactAddI64),
                                &operands,
                                encoded.bytes()
                            )
                            .is_err()
                        );
                    }
                }
            }
        }

        #[test]
        fn bitwise_and_preserves_aliases_and_rejects_opcode_corruption() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let views =
                ["x3", "x9", "x20"].map(|name| physical.model().view_named(name).unwrap().id);
            let kind = SelectedInstructionKind::BitwiseAndI64;
            let alternative = alternative(MachineAlternativeFamily::BitwiseAndI64);
            for left in views {
                for right in views {
                    for output in views {
                        let operands = [left, right, output];
                        let encoded =
                            encode_aarch64_selected_form(&physical, kind, alternative, &operands)
                                .unwrap();
                        assert_eq!(encoded.bytes().len(), 4);
                        assert!(!encoded.footprint().writes_nzcv);
                        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                        assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                        for mask in [1_u32, 1 << 5, 1 << 16, 1 << 29] {
                            let word = u32::from_le_bytes(encoded.bytes().try_into().unwrap());
                            let corrupted = (word ^ mask).to_le_bytes();
                            assert!(
                                check_encoding(&physical, kind, alternative, &operands, &corrupted)
                                    .is_err()
                            );
                        }
                    }
                }
            }
        }

        #[test]
        fn scalar_forms_report_exact_decoded_footprints() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let views =
                ["x3", "x4", "x5"].map(|name| physical.model().view_named(name).unwrap().id);
            let fact = optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]);
            let cases = [
                (
                    SelectedInstructionKind::CopyI64,
                    MachineAlternativeFamily::CopyI64,
                    2,
                ),
                (
                    SelectedInstructionKind::CompareI64Zero,
                    MachineAlternativeFamily::CompareI64Zero,
                    1,
                ),
                (
                    SelectedInstructionKind::CompareI64,
                    MachineAlternativeFamily::CompareI64,
                    2,
                ),
                (
                    SelectedInstructionKind::ExactAddI64 {
                        obligation: ObligationId::new(1).unwrap(),
                        accepted_fact: fact,
                    },
                    MachineAlternativeFamily::ExactAddI64,
                    3,
                ),
                (
                    SelectedInstructionKind::ExactAddI64Immediate {
                        immediate: IntegerValue::Unsigned(4095),
                        obligation: ObligationId::new(2).unwrap(),
                        accepted_fact: fact,
                    },
                    MachineAlternativeFamily::ExactAddI64Immediate,
                    2,
                ),
                (
                    SelectedInstructionKind::ExactSubtractI64 {
                        obligation: ObligationId::new(3).unwrap(),
                        accepted_fact: fact,
                    },
                    MachineAlternativeFamily::ExactSubtractI64,
                    3,
                ),
                (
                    SelectedInstructionKind::ExactSubtractI64Immediate {
                        immediate: IntegerValue::Unsigned(5),
                        obligation: ObligationId::new(4).unwrap(),
                        accepted_fact: fact,
                    },
                    MachineAlternativeFamily::ExactSubtractI64Immediate,
                    2,
                ),
                (
                    SelectedInstructionKind::ExactMultiplyI64 {
                        obligation: ObligationId::new(5).unwrap(),
                        accepted_fact: fact,
                    },
                    MachineAlternativeFamily::ExactMultiplyI64,
                    3,
                ),
            ];
            for (kind, family, count) in cases {
                let encoded = encode_aarch64_selected_form(
                    &physical,
                    kind,
                    alternative(family),
                    &views[..count],
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), 4);
                if matches!(
                    kind,
                    SelectedInstructionKind::ExactSubtractI64Immediate { .. }
                ) {
                    assert_eq!(encoded.bytes(), [0x64, 0x14, 0x00, 0xd1]);
                    assert!(!encoded.footprint().writes_nzcv);
                    assert!(encoded.footprint().encoded.implicit_unit_defs.is_empty());
                }
            }
            let compare = encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::CompareI64,
                alternative(MachineAlternativeFamily::CompareI64),
                &views[..2],
            )
            .unwrap();
            assert_eq!(compare.bytes(), [0x7f, 0x00, 0x04, 0xeb]);
            assert_eq!(compare.footprint().register_reads, views[..2]);
            assert!(compare.footprint().register_writes.is_empty());
            assert!(compare.footprint().writes_nzcv);
            assert_eq!(compare.footprint().encoded.external_operand_reads, [0, 1]);
            let multiply = encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::ExactMultiplyI64 {
                    obligation: ObligationId::new(5).unwrap(),
                    accepted_fact: fact,
                },
                alternative(MachineAlternativeFamily::ExactMultiplyI64),
                &views,
            )
            .unwrap();
            assert_eq!(multiply.bytes(), [0x65, 0x7c, 0x04, 0x9b]);
            assert_eq!(multiply.footprint().register_reads, views[..2]);
            assert_eq!(multiply.footprint().register_writes, views[2..]);
            assert!(!multiply.footprint().writes_nzcv);
            assert_eq!(multiply.footprint().encoded.external_operand_reads, [0, 1]);
            assert_eq!(multiply.footprint().encoded.external_operand_writes, [2]);
        }

        #[test]
        fn compare_i64_immediate_is_subs_xzr_imm12_with_exact_footprint() {
            let physical =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let x3 = physical.model().view_named("x3").unwrap().id;
            let x12 = physical.model().view_named("x12").unwrap().id;
            let alternative = alternative(MachineAlternativeFamily::CompareI64Immediate);
            for (operand, immediate, expected) in [
                (x3, 5, [0x7f, 0x14, 0x00, 0xf1]),
                (x3, 4095, [0x7f, 0xfc, 0x3f, 0xf1]),
                (x12, 4095, [0x9f, 0xfd, 0x3f, 0xf1]),
            ] {
                let kind = SelectedInstructionKind::CompareI64Immediate {
                    immediate: IntegerValue::Unsigned(immediate),
                };
                let encoded =
                    encode_aarch64_selected_form(&physical, kind, alternative, &[operand]).unwrap();
                assert_eq!(encoded.bytes(), expected);
                assert_eq!(encoded.footprint().register_reads, [operand]);
                assert!(encoded.footprint().register_writes.is_empty());
                assert!(encoded.footprint().writes_nzcv);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, [0]);
                assert!(
                    encoded
                        .footprint()
                        .encoded
                        .external_operand_writes
                        .is_empty()
                );
                assert!(!encoded.footprint().encoded.implicit_unit_defs.is_empty());
                assert!(
                    check_encoding(&physical, kind, alternative, &[operand], encoded.bytes())
                        .is_ok()
                );
                for mask in [1_u32, 1 << 5, 1 << 10, 1 << 22, 1 << 30] {
                    let word = u32::from_le_bytes(encoded.bytes().try_into().unwrap());
                    let corrupted = (word ^ mask).to_le_bytes();
                    assert!(
                        check_encoding(&physical, kind, alternative, &[operand], &corrupted)
                            .is_err()
                    );
                }
            }
            let zero = SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(0),
            };
            let encoded =
                encode_aarch64_selected_form(&physical, zero, alternative, &[x3]).unwrap();
            assert_eq!(encoded.bytes(), [0x7f, 0x00, 0x00, 0xf1]);
            assert!(check_encoding(&physical, zero, alternative, &[x3], encoded.bytes()).is_ok());
            for immediate in [IntegerValue::Unsigned(4096), IntegerValue::Signed(-1)] {
                let kind = SelectedInstructionKind::CompareI64Immediate { immediate };
                assert!(encode_aarch64_selected_form(&physical, kind, alternative, &[x3]).is_err());
            }
        }
    }
    macro_rules! r {
        ($kind:expr => $family:ident => $count:expr) => {
            ($kind, MachineAlternativeFamily::$family, $count)
        };
    }

    fn alternative(family: MachineAlternativeFamily) -> MachineAlternativeKey {
        MachineAlternativeKey { family, variant: 0 }
    }

    #[test]
    fn scalar_operand_effects_cover_remainder_wrapping_and_bitwise_forms() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let obligation = ObligationId::new(1).unwrap();
        let accepted_fact = AcceptedObligationFactIdentity::from_bytes([3; 32]);
        for (kind, family, count) in [
            r!(SelectedInstructionKind::WrappingSubtractI64 => WrappingSubtractI64 => 3),
            r!(SelectedInstructionKind::WrappingMultiplyI64 => WrappingMultiplyI64 => 3),
            r!(SelectedInstructionKind::BitwiseOrI64 => BitwiseOrI64 => 3),
            r!(SelectedInstructionKind::BitwiseNotI64 => BitwiseNotI64 => 2),
            (
                SelectedInstructionKind::ExactRemainderU64 {
                    obligation,
                    accepted_fact,
                },
                MachineAlternativeFamily::ExactRemainderU64,
                3,
            ),
            (
                SelectedInstructionKind::WrappingDivideI64 {
                    obligation,
                    accepted_fact,
                },
                MachineAlternativeFamily::WrappingDivideI64,
                3,
            ),
            r!(SelectedInstructionKind::WrappingShiftLeftI64 => WrappingShiftLeftI64 => 3),
            r!(SelectedInstructionKind::WrappingShiftRightI64 => WrappingShiftRightI64 => 3),
            r!(SelectedInstructionKind::WrappingShiftRightU64 => WrappingShiftRightU64 => 3),
            (
                SelectedInstructionKind::ExactShiftLeftI64 {
                    obligation,
                    accepted_fact,
                },
                MachineAlternativeFamily::ExactShiftLeftI64,
                3,
            ),
            (
                SelectedInstructionKind::ExactShiftRightI64 {
                    obligation,
                    accepted_fact,
                },
                MachineAlternativeFamily::ExactShiftRightI64,
                3,
            ),
            (
                SelectedInstructionKind::ExactShiftRightU64 {
                    obligation,
                    accepted_fact,
                },
                MachineAlternativeFamily::ExactShiftRightU64,
                3,
            ),
        ] {
            for names in [["x9", "x10", "x11"], ["x9", "x9", "x11"]] {
                let registers = names.map(|name| physical.model().view_named(name).unwrap().id);
                let operands = &registers[..count];
                let key = alternative(family);
                let encoded = encode_aarch64_selected_form(&physical, kind, key, operands).unwrap();
                let decoded = validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(decoded.footprint().register_reads, operands[..count - 1]);
                assert_eq!(decoded.footprint().register_writes, operands[count - 1..]);
                assert_eq!(
                    decoded.footprint().encoded,
                    MachineEncodedEffects::fallthrough_v1(
                        (0..count as u16 - 1).collect(),
                        vec![count as u16 - 1],
                    )
                );
                rejects_every_bit_flip(&physical, kind, key, operands, encoded.bytes());
            }
        }
    }

    #[test]
    fn wrapping_remainder_binds_signed_divide_msub_and_preserved_inputs() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::WrappingRemainderI64 {
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
        };
        let key = alternative(MachineAlternativeFamily::WrappingRemainderI64);
        for names in [["x9", "x10", "x11"], ["x9", "x9", "x11"]] {
            let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
            let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_eq!(encoded.bytes().len(), 8);
            assert_eq!(encoded.footprint().register_reads, operands[..2]);
            assert_eq!(encoded.footprint().register_writes, operands[2..]);
            assert_eq!(
                encoded.footprint().encoded,
                MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2])
            );
            assert!(!encoded.footprint().writes_nzcv);
            if names[1] == "x10" {
                assert_eq!(
                    encoded.bytes(),
                    [0x9aca_0d2b_u32, 0x9b0a_a56b]
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect::<Vec<_>>()
                );
            }
            for byte_position in 0..encoded.bytes().len() {
                let mut changed = encoded.bytes().to_vec();
                changed[byte_position] ^= 1;
                assert!(
                    validate_aarch64_selected_form_encoding(
                        &physical, kind, key, &operands, &changed,
                    )
                    .is_err(),
                    "mutated byte {byte_position}"
                );
            }
            for operand_position in 0..operands.len() {
                let mut changed = operands;
                changed[operand_position] = physical.model().view_named("x8").unwrap().id;
                assert!(
                    validate_aarch64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &changed,
                        encoded.bytes(),
                    )
                    .is_err()
                );
            }
        }
        for names in [["x9", "x10", "x9"], ["x9", "x10", "x10"]] {
            let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
            assert!(encode_aarch64_selected_form(&physical, kind, key, &operands).is_err());
            let destination = if names[2] == "x9" { 9 } else { 10 };
            let bytes = [
                0x9aca_0d20 | destination,
                0x9b0a_a400 | (destination << 5) | destination,
            ]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
            assert!(
                validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, &bytes)
                    .is_err()
            );
        }
    }

    #[test]
    fn wrapping_remainder_decoded_arithmetic_covers_minimum_and_dividend_sign() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::WrappingRemainderI64 {
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
        };
        let key = alternative(MachineAlternativeFamily::WrappingRemainderI64);
        let operands =
            ["x9", "x10", "x11"].map(|name| physical.model().view_named(name).unwrap().id);
        let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
        for (dividend, divisor, expected) in [
            (i64::MIN, -1, 0),
            (i64::MIN, 1, 0),
            (i64::MIN, 3, -2),
            (7, 2, 1),
            (-7, 2, -1),
            (7, -2, 1),
            (-7, -2, -1),
            (0, -1, 0),
        ] {
            let mut registers = [0_i64; 31];
            registers[9] = dividend;
            registers[10] = divisor;
            for instruction in decode_words(encoded.bytes()).unwrap() {
                match instruction {
                    DecodedWord::SignedDivide {
                        dividend,
                        divisor,
                        destination,
                    } => {
                        registers[destination as usize] =
                            registers[dividend as usize].wrapping_div(registers[divisor as usize]);
                    }
                    DecodedWord::MultiplySubtract {
                        left,
                        right,
                        minuend,
                        destination,
                    } => {
                        registers[destination as usize] = registers[minuend as usize].wrapping_sub(
                            registers[left as usize].wrapping_mul(registers[right as usize]),
                        );
                    }
                    _ => panic!("unexpected remainder instruction"),
                }
            }
            assert_eq!(registers[11], expected, "{dividend} % {divisor}");
            assert_eq!((registers[9], registers[10]), (dividend, divisor));
        }
    }

    #[test]
    fn exact_divide_binds_unsigned_opcode_and_registers() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::ExactDivideU64 {
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
        };
        let key = alternative(MachineAlternativeFamily::ExactDivideU64);
        let names = ["x9", "x10", "x11"];
        let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
        let expected = 0x9aca092bu32.to_le_bytes().to_vec();

        let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(encoded.bytes(), expected);
        for byte in 0..expected.len() {
            let mut changed = expected.clone();
            changed[byte] ^= 1;
            assert!(
                validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, &changed)
                    .is_err()
            );
        }
        for operand in 0..operands.len() {
            let mut changed = operands;
            changed[operand] = physical.model().view_named("x8").unwrap().id;
            assert!(
                validate_aarch64_selected_form_encoding(&physical, kind, key, &changed, &expected)
                    .is_err()
            );
        }
    }
}
pub use branch_forms::{
    AARCH64_CONDITIONAL_BRANCH_BYTE_REACH, Aarch64ConditionalBranchForm,
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_conditional_branch_form,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_conditional_branch_form,
};
pub(crate) use encoding_types::Aarch64SelectedFormFootprint;
pub use encoding_types::{Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding};
pub use hosted_write_byte::{
    encode_aarch64_selected_hosted_write_byte_form,
    validate_aarch64_selected_hosted_write_byte_form,
};
pub use jump::{encode_aarch64_selected_jump_form, validate_aarch64_selected_jump_form};
pub use memory::{
    encode_aarch64_selected_frame_form, encode_aarch64_selected_memory_form,
    validate_aarch64_selected_frame_form, validate_aarch64_selected_memory_form,
};

pub use normalized_foreign_call::{
    Aarch64CallbackAddressMaterialization, Aarch64CallbackMaterializationDestination,
    Aarch64NormalizedForeignCallFixup, Aarch64NormalizedForeignCallFixupKind,
    Aarch64NormalizedForeignCallFixupState, Aarch64NormalizedForeignCallTemplateError,
    encode_aarch64_selected_normalized_foreign_call_template,
    validate_aarch64_selected_normalized_foreign_call_template,
};
pub use scalar_call::{
    Aarch64ScalarCallFixup, Aarch64ScalarCallFixupKind, Aarch64ScalarCallFixupState,
    Aarch64ScalarCallTemplateError, encode_aarch64_selected_dynamic_call_template,
    encode_aarch64_selected_scalar_call_template, validate_aarch64_selected_scalar_call_template,
};
pub use table_call::{
    encode_aarch64_selected_table_call_form, validate_aarch64_selected_table_call_form,
};

pub use selected_forms::{
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET,
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET, encode_aarch64_selected_form,
    validate_aarch64_selected_form_encoding,
};

fn nzcv_flag_units(
    physical: &register_model::ValidatedPhysicalRegisterModel,
) -> Result<Vec<register_model::RegisterUnitId>, Aarch64SelectedFormEncodingError> {
    Ok(physical
        .model()
        .view_named("nzcv")
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
        .units
        .clone())
}

fn nzcv_effects_encoding(
    physical: &register_model::ValidatedPhysicalRegisterModel,
    operands: &[register_model::RegisterViewId],
    bytes: &[u8],
    read_count: usize,
    memory: selected_instructions::MachineEncodedMemoryEffect,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: operands[..read_count].to_vec(),
            register_writes: operands[read_count..].to_vec(),
            writes_nzcv: true,
            encoded: selected_instructions::MachineEncodedEffects {
                external_operand_reads: (0..read_count as u16).collect(),
                external_operand_writes: (read_count as u16..operands.len() as u16).collect(),
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: nzcv_flag_units(physical)?,
                memory,
                stack: selected_instructions::MachineEncodedStackEffect::UnchangedV1,
                trap: selected_instructions::MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: selected_instructions::MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

fn word_stream(bytes: &[u8]) -> Option<Vec<u32>> {
    let (words, tail) = bytes.as_chunks::<4>();
    tail.is_empty()
        .then(|| words.iter().map(|word| u32::from_le_bytes(*word)).collect())
}

fn decode_all<D>(
    bytes: &[u8],
    decode_word: impl Fn(u32, usize) -> Result<D, Aarch64SelectedFormEncodingError>,
) -> Result<Vec<D>, Aarch64SelectedFormEncodingError> {
    word_stream(bytes)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?
        .into_iter()
        .enumerate()
        .map(|(position, word)| decode_word(word, position))
        .collect()
}
