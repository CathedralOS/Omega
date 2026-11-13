/*
We encode, structurally decode, and validate all x86-64 selected instruction
forms (`encode_x86_64_selected_form` and
`validate_x86_64_selected_form_encoding`), short and near conditional and
unconditional branches, scalar and dynamic call templates, normalized foreign-
call templates, atomic operations, IEEE floating-point arithmetic, hosted I/O
leaves, and installed interrupt/table sequences.
*/

pub(crate) mod atomic {
    use super::{X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint};
    use crate::register_model::units;
    use crate::selected_form_encoding::instruction_bytes::{append_boolean, modrm, rex};
    use register_model::RegisterViewId;
    use selected_instructions::{
        AtomicForm, AtomicOperation, AtomicWidth, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedTrapBehavior,
    };

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
                form.output_operands().collect(),
            )
        };
        if writes_rflags(form.operation) {
            effects
                .implicit_unit_clobbers
                .extend(units("condition_flags"));
        }
        if matches!(
            form.operation,
            AtomicOperation::CompareExchange | AtomicOperation::CompareExchangeOnce
        ) || form.operation.is_bitwise_fetch()
        {
            effects.implicit_unit_clobbers.extend(units("rax"));
        }
        if form.operation.is_bitwise_fetch() {
            effects.control = MachineEncodedControlEffect::AtomicRetryUntilCommitV1;
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
                .output_operands()
                .map(|output| operands[usize::from(output)])
                .collect(),
            writes_rflags: writes_rflags(form.operation),
            encoded: effects(form),
        }
    }

    const fn operand_bits(width: AtomicWidth) -> u16 {
        match width {
            AtomicWidth::Bits8 => 8,
            AtomicWidth::Bits16 => 16,
            AtomicWidth::Bits32 => 32,
            AtomicWidth::Bits64 => 64,
        }
    }

    fn append_memory_modrm(bytes: &mut Vec<u8>, register: u8, base: u8) {
        bytes.push(modrm(2, register & 7, base & 7));
        if base & 7 == 4 {
            bytes.push(0x24);
        }
        bytes.extend(0i32.to_le_bytes());
    }

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
        if byte != 0x40 || operand_bits(width) == 8 {
            bytes.push(byte);
        }
    }

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
        for output_operand in form.output_operands() {
            let output = registers[usize::from(output_operand)];
            if registers[..usize::from(output_operand)].contains(&output) {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
        }
        if matches!(
            form.operation,
            AtomicOperation::CompareExchange | AtomicOperation::CompareExchangeOnce
        ) && registers[1] != 0
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        if form.operation.is_bitwise_fetch() && registers.contains(&0) {
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
            AtomicOperation::CompareExchange | AtomicOperation::CompareExchangeOnce => {
                let replacement = registers[2];
                append_width_prefixes(&mut bytes, width, true, true, bits == 64, replacement, base);
                bytes.extend([0x0f, if bits == 8 { 0xb0 } else { 0xb1 }]);
                append_memory_modrm(&mut bytes, replacement, base);
                copy_into_observed(&mut bytes, registers[3], 0);
                if let Some(outcome) = form.outcome_operand() {
                    append_boolean(&mut bytes, registers[usize::from(outcome)], 4);
                }
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
                bytes.extend(encode(
                    AtomicForm {
                        width,
                        operation: AtomicOperation::Load,
                    },
                    &[base, 0],
                )?);
                let retry = bytes.len();
                let observed = registers[2];
                let mask = registers[1];
                copy_into_observed(&mut bytes, observed, 0);
                append_width_prefixes(&mut bytes, width, false, true, bits == 64, mask, observed);
                let opcode = match form.operation {
                    AtomicOperation::FetchAnd => 0x20,
                    AtomicOperation::FetchOr => 0x08,
                    AtomicOperation::FetchXor => 0x30,
                    _ => unreachable!("bitwise update arm"),
                };
                bytes.extend([
                    opcode + u8::from(bits != 8),
                    modrm(3, mask & 7, observed & 7),
                ]);
                append_width_prefixes(&mut bytes, width, true, true, bits == 64, observed, base);
                bytes.extend([0x0f, if bits == 8 { 0xb0 } else { 0xb1 }]);
                append_memory_modrm(&mut bytes, observed, base);
                let displacement = i8::try_from(retry as isize - (bytes.len() + 2) as isize)
                    .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
                bytes.extend([0x75, displacement as u8]);
                copy_into_observed(&mut bytes, observed, 0);
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

        const fn form_registers(form: AtomicForm) -> [u8; 5] {
            match form.operation {
                AtomicOperation::Load => [2, 3, 0, 0, 0],
                AtomicOperation::Store => [2, 5, 0, 0, 0],
                AtomicOperation::CompareExchange | AtomicOperation::CompareExchangeOnce => {
                    [2, 0, 6, 7, 8]
                }
                _ => [2, 5, 6, 0, 0],
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
                crate::validate_x86_64_machine_effect_catalog(target, &constraints, catalog)
                    .unwrap();
            }
        }

        #[test]
        fn atomic_words_pin_lock_encoding_and_reject_every_changed_bit() {
            let physical =
                validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
            for form in AtomicForm::ALL {
                let codes = form_registers(form);
                let registers = &codes[..usize::from(form.operand_count())];
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
                    AtomicOperation::Load | AtomicOperation::Store => vec![views[0], views[3]],
                    AtomicOperation::CompareExchange => {
                        vec![views[0], views[1], views[3], views[4]]
                    }
                    AtomicOperation::CompareExchangeOnce => {
                        vec![views[0], views[1], views[2], views[3], views[4]]
                    }
                    _ => vec![views[0], views[2], views[3]],
                };
                let kind = SelectedInstructionKind::AtomicAccess { form };
                let alternative = MachineAlternativeKey {
                    family: MachineAlternativeFamily::AtomicAccess(form),
                    variant: 0,
                };
                let encoded = crate::encode_x86_64_selected_form(
                    &physical,
                    kind,
                    alternative,
                    &operand_views,
                )
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
                for output in form.output_operands() {
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
        #[test]
        fn bitwise_retry_returns_committed_prior_and_preserves_inputs() {
            let form = AtomicForm {
                width: AtomicWidth::Bits32,
                operation: AtomicOperation::FetchOr,
            };
            assert_eq!(
                encode(form, &[2, 1, 8]).unwrap(),
                [
                    0x8b, 0x82, 0, 0, 0, 0, 0x49, 0x89, 0xc0, 0x41, 0x09, 0xc8, 0xf0, 0x44, 0x0f,
                    0xb1, 0x82, 0, 0, 0, 0, 0x75, 0xef, 0x49, 0x89, 0xc0,
                ]
            );
            for form in AtomicForm::ALL
                .into_iter()
                .filter(|form| form.operation.is_bitwise_fetch())
            {
                for operand in 0..3 {
                    let mut homes = [2, 1, 8];
                    homes[operand] = 0;
                    assert!(
                        encode(form, &homes).is_err(),
                        "RAX alias: {form:?} operand {operand}"
                    );
                }
                let bytes = encode(form, &[2, 1, 8]).unwrap();
                assert_eq!(
                    effects(form).control,
                    MachineEncodedControlEffect::AtomicRetryUntilCommitV1
                );
                assert_eq!(effects(form).external_operand_writes, [2]);
                let branch = bytes.len() - 5;
                assert_eq!(bytes[branch], 0x75);
                let retry = (branch as isize + 2 + bytes[branch + 1] as i8 as isize) as usize;
                assert_eq!(&bytes[retry..retry + 3], &[0x49, 0x89, 0xc0]);
                assert!(retry > 0 && retry < branch);
                println!(
                    "atomic-vector {:?} {:?} {}",
                    form.width,
                    form.operation,
                    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
                );
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
            MachineSizeKnowledge::ExactBytes(3),
            MachineEncodedEffects::atomic_fence_v1(),
        )
    }

    #[cfg(test)]
    mod tests {
        use selected_instructions::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
            SelectedInstructionKind,
        };

        #[test]
        fn atomic_fence_pins_full_barrier_bytes_and_rejects_substitutions() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let kind = SelectedInstructionKind::AtomicFence;
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::AtomicFence,
                variant: 0,
            };
            let encoded =
                crate::encode_x86_64_selected_form(&physical, kind, alternative, &[]).unwrap();
            assert_eq!(encoded.bytes(), &[0x0f, 0xae, 0xf0]);
            assert_eq!(
                encoded.footprint().encoded,
                MachineEncodedEffects::atomic_fence_v1()
            );
            crate::selected_form_encoding::assert_every_bit_flip_rejected(
                encoded.bytes(),
                |bytes| {
                    crate::validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        bytes,
                    )
                    .is_err()
                },
            );
            let mut extended = encoded.bytes().to_vec();
            extended.push(0);
            for bytes in [&extended[..], &encoded.bytes()[..encoded.bytes().len() - 1]] {
                assert!(
                    crate::validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        bytes
                    )
                    .is_err()
                );
            }
            let operand = physical.model().view_named("rax").unwrap().id;
            assert!(
                crate::encode_x86_64_selected_form(&physical, kind, alternative, &[operand])
                    .is_err()
            );
            assert!(
                crate::encode_x86_64_selected_form(
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
                crate::encode_x86_64_selected_form(
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
    use super::{encode_x86_64_selected_form, validate_x86_64_selected_form_encoding};
    use crate::x86_64_physical_register_model;
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
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        for name in ["rax", "rcx", "rbp", "rsi", "rdi", "r8", "r9", "r15"] {
            let destination = physical.model().view_named(name).unwrap().id;
            for (kind, family) in forms() {
                let key = MachineAlternativeKey { family, variant: 0 };
                let encoded =
                    encode_x86_64_selected_form(&physical, kind, key, &[destination]).unwrap();
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &[destination],
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), 8);
                let footprint = encoded.footprint();
                assert!(footprint.register_reads.is_empty());
                assert_eq!(footprint.register_writes, vec![destination]);
                assert!(!footprint.writes_rflags);
                let mut expected = MachineEncodedEffects::fallthrough_v1(vec![], vec![0]);
                expected.implicit_unit_uses = physical
                    .model()
                    .view_named("condition_flags")
                    .unwrap()
                    .units
                    .clone();
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
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            other_kind,
                            other_key,
                            &[destination],
                            encoded.bytes()
                        )
                        .is_err()
                    );
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            other_key,
                            &[destination],
                            encoded.bytes()
                        )
                        .is_err()
                    );
                }
                crate::selected_form_encoding::assert_every_bit_flip_rejected(
                    encoded.bytes(),
                    |changed| {
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            key,
                            &[destination],
                            changed,
                        )
                        .is_err()
                    },
                );
                for length in 0..encoded.bytes().len() {
                    assert!(
                        validate_x86_64_selected_form_encoding(
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
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &[destination],
                        &extra
                    )
                    .is_err()
                );
                for invalid in ["rsp", "eax", "ah", "xmm0", "rflags"] {
                    let view = physical.model().view_named(invalid).unwrap().id;
                    assert!(
                        encode_x86_64_selected_form(&physical, kind, key, &[view]).is_err(),
                        "invalid destination {invalid}"
                    );
                }
            }
        }
    }

    #[test]
    fn condition_opcodes_and_rex_low_byte_normalization_are_exact() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        for ((kind, family), opcode) in forms().into_iter().zip([0x94, 0x92, 0x9c, 0x96, 0x9e]) {
            let key = MachineAlternativeKey { family, variant: 0 };
            for (name, expected) in [
                ("rsi", [0x40, 0x0f, opcode, 0xc6, 0x40, 0x0f, 0xb6, 0xf6]),
                ("r15", [0x41, 0x0f, opcode, 0xc7, 0x45, 0x0f, 0xb6, 0xff]),
            ] {
                let destination = physical.model().view_named(name).unwrap().id;
                assert_eq!(
                    encode_x86_64_selected_form(&physical, kind, key, &[destination])
                        .unwrap()
                        .bytes(),
                    expected
                );
                let mut missing_rex = expected.to_vec();
                missing_rex.remove(0);
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &[destination],
                        &missing_rex
                    )
                    .is_err()
                );
            }
        }
    }
}
mod branch_forms {
    use crate::selected_form_encoding::decoding::footprint;
    use crate::selected_form_encoding::{
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
    };
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
    };

    #[derive(Clone, Copy)]
    struct BranchFamily {
        family: MachineAlternativeFamily,
        kind: SelectedInstructionKind,
        condition: u8,
        variant_count: u32,
    }

    const fn short_opcode(condition: u8) -> u8 {
        0x70 | condition
    }

    const fn near_opcode(condition: u8) -> [u8; 2] {
        [0x0f, 0x80 | condition]
    }

    const NONZERO: BranchFamily = BranchFamily {
        family: MachineAlternativeFamily::ConditionalBranchNonZero,
        kind: SelectedInstructionKind::ConditionalBranchNonZero,
        condition: 0x5,
        variant_count: 1,
    };

    const U64_LESS_THAN: BranchFamily = BranchFamily {
        family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
        kind: SelectedInstructionKind::ConditionalBranchU64LessThan,
        condition: 0x2,
        variant_count: 2,
    };

    const I64_LESS_THAN: BranchFamily = BranchFamily {
        family: MachineAlternativeFamily::ConditionalBranchI64LessThan,
        kind: SelectedInstructionKind::ConditionalBranchI64LessThan,
        condition: 0xc,
        variant_count: 2,
    };

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum WidthRule {
        Near,
        Short,
        Variant,
    }

    #[derive(Clone, Copy)]
    struct BranchRequest {
        row: BranchFamily,
        alternative: MachineAlternativeKey,
        near: bool,
    }

    impl BranchRequest {
        fn new(
            physical: &ValidatedPhysicalRegisterModel,
            alternative: MachineAlternativeKey,
            row: BranchFamily,
            rule: WidthRule,
        ) -> Result<Self, X86_64SelectedFormEncodingError> {
            super::request_validation::guard_canonical(physical)?;
            if alternative.family != row.family || alternative.variant >= row.variant_count {
                return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
            }
            let near = match rule {
                WidthRule::Near => true,
                WidthRule::Short => false,
                WidthRule::Variant => alternative.variant == 0,
            };
            Ok(Self {
                row,
                alternative,
                near,
            })
        }

        fn encode(
            self,
            byte_displacement_from_instruction_end: i64,
        ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
            let bytes = self.bytes(byte_displacement_from_instruction_end)?;
            self.validate(byte_displacement_from_instruction_end, &bytes)
        }

        fn validate(
            self,
            byte_displacement_from_instruction_end: i64,
            bytes: &[u8],
        ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
            self.validate_bytes(byte_displacement_from_instruction_end, bytes)?;
            Ok(ValidatedX86_64SelectedFormEncoding {
                bytes: bytes.to_vec(),
                footprint: footprint(self.row.kind, self.alternative, &[]),
            })
        }

        fn bytes(
            self,
            byte_displacement_from_instruction_end: i64,
        ) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
            if self.near {
                let displacement = i32::try_from(byte_displacement_from_instruction_end)
                    .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
                let mut bytes = near_opcode(self.row.condition).to_vec();
                bytes.extend(displacement.to_le_bytes());
                return Ok(bytes);
            }
            let displacement = i8::try_from(byte_displacement_from_instruction_end)
                .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
            Ok(vec![short_opcode(self.row.condition), displacement as u8])
        }

        fn validate_bytes(
            self,
            byte_displacement_from_instruction_end: i64,
            bytes: &[u8],
        ) -> Result<(), X86_64SelectedFormEncodingError> {
            let (expected, encoded) = if self.near {
                let expected = i32::try_from(byte_displacement_from_instruction_end)
                    .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)?;
                let encoded = bytes
                    .get(2..6)
                    .filter(|_| bytes.len() == 6 && bytes[..2] == near_opcode(self.row.condition))
                    .and_then(|displacement| displacement.try_into().ok())
                    .map(i32::from_le_bytes)
                    .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
                (i64::from(expected), i64::from(encoded))
            } else {
                let expected = i8::try_from(byte_displacement_from_instruction_end)
                    .map_err(|_| X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)?;
                let encoded = bytes
                    .get(1)
                    .copied()
                    .filter(|_| bytes.len() == 2 && bytes[0] == short_opcode(self.row.condition))
                    .map(|displacement| displacement as i8)
                    .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
                (i64::from(expected), i64::from(encoded))
            };
            if expected != encoded {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            Ok(())
        }
    }

    pub fn encode_x86_64_selected_nonzero_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        byte_displacement_from_instruction_end: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let request = BranchRequest::new(physical, alternative, NONZERO, WidthRule::Near)?;
        request.encode(byte_displacement_from_instruction_end)
    }

    pub fn validate_x86_64_selected_nonzero_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        byte_displacement_from_instruction_end: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let request = BranchRequest::new(physical, alternative, NONZERO, WidthRule::Near)?;
        request.validate(byte_displacement_from_instruction_end, bytes)
    }

    pub fn encode_x86_64_selected_short_nonzero_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        byte_displacement_from_instruction_end: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let request = BranchRequest::new(physical, alternative, NONZERO, WidthRule::Short)?;
        request.encode(byte_displacement_from_instruction_end)
    }

    pub fn validate_x86_64_selected_short_nonzero_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        byte_displacement_from_instruction_end: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let request = BranchRequest::new(physical, alternative, NONZERO, WidthRule::Short)?;
        request.validate(byte_displacement_from_instruction_end, bytes)
    }

    pub fn encode_x86_64_selected_u64_less_than_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        byte_displacement_from_instruction_end: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let request = BranchRequest::new(physical, alternative, U64_LESS_THAN, WidthRule::Variant)?;
        request.encode(byte_displacement_from_instruction_end)
    }

    pub fn validate_x86_64_selected_u64_less_than_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        byte_displacement_from_instruction_end: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let request = BranchRequest::new(physical, alternative, U64_LESS_THAN, WidthRule::Variant)?;
        request.validate(byte_displacement_from_instruction_end, bytes)
    }

    pub fn encode_x86_64_selected_i64_less_than_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        byte_displacement_from_instruction_end: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let request = BranchRequest::new(physical, alternative, I64_LESS_THAN, WidthRule::Variant)?;
        request.encode(byte_displacement_from_instruction_end)
    }

    pub fn validate_x86_64_selected_i64_less_than_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        byte_displacement_from_instruction_end: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let request = BranchRequest::new(physical, alternative, I64_LESS_THAN, WidthRule::Variant)?;
        request.validate(byte_displacement_from_instruction_end, bytes)
    }
}
#[cfg(test)]
mod byte_literal_address_tests {
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
        SelectedInstructionKind, X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET,
        encode_x86_64_selected_form, validate_x86_64_selected_form_encoding,
    };
    use register_model::validate_physical_register_model;
    use semantic_vocabulary::{OperationId, PlaceId};

    #[test]
    fn byte_literal_address_is_an_unresolved_rip_relative_lea() {
        let physical =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let views = ["rcx", "r9"].map(|name| physical.model().view_named(name).unwrap().id);
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
        for (view, expected) in [
            (views[0], [0x48, 0x8d, 0x0d, 0, 0, 0, 0]),
            (views[1], [0x4c, 0x8d, 0x0d, 0, 0, 0, 0]),
        ] {
            let encoded = encode_x86_64_selected_form(&physical, kind, key, &[view]).unwrap();
            assert_eq!(encoded.bytes(), expected);
            assert_eq!(
                &encoded.bytes()[usize::from(X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET)..],
                [0, 0, 0, 0]
            );
            validate_x86_64_selected_form_encoding(&physical, kind, key, &[view], encoded.bytes())
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
            crate::selected_form_encoding::assert_every_bit_flip_rejected(
                encoded.bytes(),
                |changed| {
                    validate_x86_64_selected_form_encoding(&physical, kind, key, &[view], changed)
                        .is_err()
                },
            );
        }
    }
}
#[cfg(test)]
mod byte_view_address_tests {
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
        SelectedInstructionKind, encode_x86_64_selected_form,
        validate_x86_64_selected_form_encoding,
    };
    use register_model::validate_physical_register_model;

    #[test]
    fn byte_view_address_has_distinct_family_and_exact_register_bytes() {
        let physical =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let views =
            ["rax", "rcx", "rdx", "rbx"].map(|name| physical.model().view_named(name).unwrap().id);
        let operands = &views[..3];
        let kind = SelectedInstructionKind::ByteViewAddress;
        let key = MachineAlternativeKey {
            family: MachineAlternativeFamily::ByteViewAddress,
            variant: 0,
        };
        let encoded = encode_x86_64_selected_form(&physical, kind, key, operands).unwrap();
        validate_x86_64_selected_form_encoding(&physical, kind, key, operands, encoded.bytes())
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
            validate_x86_64_selected_form_encoding(
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
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &changed,
                    encoded.bytes()
                )
                .is_err()
            );
        }
        crate::selected_form_encoding::assert_every_bit_flip_rejected(encoded.bytes(), |changed| {
            validate_x86_64_selected_form_encoding(&physical, kind, key, operands, changed).is_err()
        });
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
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
        RegisterViewId, ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError,
    };
    use crate::selected_form_encoding::instruction_bytes::{
        append_increment, append_indexed_byte_load, append_register_compare, append_self_clear,
        append_self_test, assemble_labeled_stream,
    };
    use crate::selected_form_encoding::request_validation::byte_loop_registers;

    const CC_E: u8 = 0x84;
    const CC_NE: u8 = 0x85;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Label {
        Loop,
        Equal,
        Done,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Instruction {
        XorSelf(u8),
        Inc(u8),
        TestSelf64(u8),
        CompareRegister64 {
            source: u8,
            operand: u8,
        },
        LoadByte {
            value: u8,
            base: u8,
            index: u8,
            offset: u32,
        },
        JumpConditional {
            condition: u8,
            target: Label,
        },
    }

    fn layout(registers: [u8; 8]) -> Vec<(Option<Label>, Instruction)> {
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
            (None, Instruction::XorSelf(cursor)),
            (None, Instruction::XorSelf(left_byte)),
            (None, Instruction::XorSelf(right_byte)),
            (None, Instruction::XorSelf(result)),
            (
                None,
                Instruction::CompareRegister64 {
                    source: right_count,
                    operand: left_count,
                },
            ),
            (
                None,
                Instruction::JumpConditional {
                    condition: CC_NE,
                    target: Label::Done,
                },
            ),
            (None, Instruction::TestSelf64(left_count)),
            (
                None,
                Instruction::JumpConditional {
                    condition: CC_E,
                    target: Label::Equal,
                },
            ),
            (
                Some(Label::Loop),
                Instruction::LoadByte {
                    value: left_byte,
                    base: left_base,
                    index: cursor,
                    offset: 0,
                },
            ),
            (
                None,
                Instruction::LoadByte {
                    value: right_byte,
                    base: right_base,
                    index: cursor,
                    offset: 0,
                },
            ),
            (
                None,
                Instruction::CompareRegister64 {
                    source: right_byte,
                    operand: left_byte,
                },
            ),
            (
                None,
                Instruction::JumpConditional {
                    condition: CC_NE,
                    target: Label::Done,
                },
            ),
            (None, Instruction::Inc(cursor)),
            (
                None,
                Instruction::CompareRegister64 {
                    source: left_count,
                    operand: cursor,
                },
            ),
            (
                None,
                Instruction::JumpConditional {
                    condition: CC_NE,
                    target: Label::Loop,
                },
            ),
            (Some(Label::Equal), Instruction::Inc(result)),
            (Some(Label::Done), Instruction::XorSelf(cursor)),
        ]
    }

    fn emit_instruction(
        bytes: &mut Vec<u8>,
        instruction: Instruction,
        patches: &mut Vec<(usize, Label)>,
    ) {
        match instruction {
            Instruction::XorSelf(register) => append_self_clear(bytes, register),
            Instruction::Inc(register) => append_increment(bytes, register),
            Instruction::TestSelf64(register) => append_self_test(bytes, register),
            Instruction::CompareRegister64 { source, operand } => {
                append_register_compare(bytes, source, operand)
            }
            Instruction::LoadByte {
                value,
                base,
                index,
                offset,
            } => append_indexed_byte_load(bytes, value, base, index, offset),
            Instruction::JumpConditional { condition, target } => {
                bytes.extend([0x0f, condition]);
                patches.push((bytes.len(), target));
                bytes.extend([0; 4]);
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Decoded {
        XorSelf(u8),
        Inc(u8),
        TestSelf64(u8),
        CompareRegister64 {
            source: u8,
            operand: u8,
        },
        LoadByte {
            value: u8,
            base: u8,
            index: u8,
            offset: u32,
        },
        JumpConditional {
            condition: u8,
            target: usize,
        },
    }

    fn decode_one(
        bytes: &[u8],
        position: usize,
    ) -> Result<(Decoded, usize), X86_64SelectedFormEncodingError> {
        let instruction = &bytes[position..];
        let fail = X86_64SelectedFormEncodingError::EncodedFormMismatch;
        let rex_fields = |prefix: u8, mode: u8| -> Option<(u8, u8)> {
            if prefix & 0xf8 != 0x48 {
                return None;
            }
            Some((
                ((mode >> 3) & 7) | ((prefix & 4) << 1),
                (mode & 7) | ((prefix & 1) << 3),
            ))
        };
        match *instruction {
            [prefix, 0x31, mode, ..] if prefix & 0xfa == 0x48 => {
                let Some((register, rm)) = rex_fields(prefix, mode) else {
                    return Err(fail);
                };
                if mode >> 6 == 3 && register == rm {
                    Ok((Decoded::XorSelf(register), 3))
                } else {
                    Err(fail)
                }
            }
            [prefix, 0xff, mode, ..]
                if prefix & 0xfe == 0x48 && mode >> 6 == 3 && ((mode >> 3) & 7) == 0 =>
            {
                Ok((Decoded::Inc((mode & 7) | ((prefix & 1) << 3)), 3))
            }
            [prefix, 0x85, mode, ..] if prefix & 0xfa == 0x48 => {
                let Some((register, rm)) = rex_fields(prefix, mode) else {
                    return Err(fail);
                };
                if mode >> 6 == 3 && register == rm {
                    Ok((Decoded::TestSelf64(register), 3))
                } else {
                    Err(fail)
                }
            }
            [prefix, 0x39, mode, ..] if prefix & 0xfa == 0x48 => {
                let Some((register, rm)) = rex_fields(prefix, mode) else {
                    return Err(fail);
                };
                if mode >> 6 == 3 {
                    Ok((
                        Decoded::CompareRegister64 {
                            source: register,
                            operand: rm,
                        },
                        3,
                    ))
                } else {
                    Err(fail)
                }
            }
            [prefix, 0x0f, 0xb6, mode, sib, d0, d1, d2, d3, ..]
                if prefix & 0xf8 == 0x48 && mode & 0xc7 == 0x84 && sib & 0xc0 == 0 =>
            {
                let value = ((mode >> 3) & 7) | ((prefix & 4) << 1);
                let base = (sib & 7) | ((prefix & 1) << 3);
                let index = ((sib >> 3) & 7) | ((prefix & 2) << 2);
                if index == 4 {
                    return Err(fail);
                }
                Ok((
                    Decoded::LoadByte {
                        value,
                        base,
                        index,
                        offset: u32::from_le_bytes([d0, d1, d2, d3]),
                    },
                    9,
                ))
            }
            [0x0f, condition, d0, d1, d2, d3, ..] if condition & 0xf0 == 0x80 => {
                let relative = i32::from_le_bytes([d0, d1, d2, d3]) as i64;
                let target = position as i64 + 6 + relative;
                if target < 0 || target > bytes.len() as i64 {
                    return Err(fail);
                }
                Ok((
                    Decoded::JumpConditional {
                        condition,
                        target: target as usize,
                    },
                    6,
                ))
            }
            _ => Err(fail),
        }
    }

    fn decode_all_stream(bytes: &[u8]) -> Result<Vec<Decoded>, X86_64SelectedFormEncodingError> {
        let mut decoded = Vec::new();
        let mut position = 0;
        while position < bytes.len() {
            let (instruction, length) = decode_one(bytes, position)?;
            decoded.push(instruction);
            position += length;
        }
        Ok(decoded)
    }

    fn expected_stream(
        registers: [u8; 8],
    ) -> Result<Vec<Decoded>, X86_64SelectedFormEncodingError> {
        let plan = layout(registers);
        let bytes = assemble_labeled_stream(&plan, emit_instruction)?;
        decode_all_stream(&bytes)
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<[u8; 8], X86_64SelectedFormEncodingError> {
        let registers = byte_loop_registers::<8>(
            physical,
            alternative,
            MachineAlternativeFamily::CompareByteSequences,
            operands,
            4,
        )?;
        if displacement != 0 || registers[4] == 4 {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(registers)
    }

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let registers = request(physical, alternative, operands, displacement)?;
        let bytes = assemble_labeled_stream(&layout(registers), emit_instruction)?;
        validate(physical, alternative, operands, displacement, &bytes)
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let registers = request(physical, alternative, operands, displacement)?;
        if decode_all_stream(bytes)? != expected_stream(registers)? {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        super::canonical_effects_encoding(
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
        use super::super::SelectedInstructionKind;
        use super::super::tests::{
            assert_byte_loop_catalog_contract,
            assert_loop_form_round_trips_and_rejects_every_changed_bit,
        };
        use super::{MachineAlternativeFamily, MachineAlternativeKey, encode, validate};

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
        fn compare_byte_sequences_catalog_requires_early_clobbers_and_memory_effect() {
            assert_byte_loop_catalog_contract(
                crate::X86_64_COMPARE_BYTE_SEQUENCES,
                &[4, 5, 6, 7],
                selected_instructions::MachineSemanticKind::CompareByteSequences,
                selected_instructions::MachineMemoryEffect::CompareByteSequencesV1,
                &[0, 1, 2, 3],
                &[4, 5, 6, 7],
            );
        }

        #[test]
        fn compare_byte_sequences_round_trips_and_rejects_every_changed_bit() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let alternative = alternative();
            assert_loop_form_round_trips_and_rejects_every_changed_bit(
                &physical,
                kind(),
                alternative,
                &[
                    &["r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15"],
                    &["rax", "rcx", "rdx", "rsi", "rdi", "rbx", "rbp", "r8"],
                ],
                |operands, displacement| encode(&physical, alternative, operands, displacement),
                |operands, bytes| validate(&physical, alternative, operands, 0, bytes).is_err(),
            );
        }
    }
}
mod copy_bytes {
    use super::{
        MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedMemoryEffect,
        RegisterViewId, ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError,
    };
    use crate::selected_form_encoding::instruction_bytes::{
        append_increment, append_indexed_byte_load, append_register_compare, append_self_clear,
        append_self_test, modrm, rex,
    };
    use crate::selected_form_encoding::request_validation::byte_loop_registers;
    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let [source, destination, count, cursor, byte] =
            request(physical, alternative, operands, displacement)?;
        let mut bytes = Vec::with_capacity(36);
        for register in [cursor, byte] {
            append_self_clear(&mut bytes, register);
        }
        append_self_test(&mut bytes, count);
        bytes.extend([0x74, 25]);
        append_indexed_byte_load(&mut bytes, byte, source, cursor, 0);
        bytes.extend_from_slice(&[
            rex(byte, cursor, destination) & !8,
            0x88,
            modrm(2, byte, 4),
            ((cursor & 7) << 3) | (destination & 7),
            0,
            0,
            0,
            0,
        ]);
        append_increment(&mut bytes, cursor);
        append_register_compare(&mut bytes, count, cursor);
        bytes.extend([0x72, 0xe7]);
        validate(physical, alternative, operands, displacement, &bytes)
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let [source, destination, count, cursor, byte] =
            request(physical, alternative, operands, displacement)?;
        if bytes.len() != 36
            || !register_instruction(&bytes[0..3], 0x31, cursor, cursor)
            || !register_instruction(&bytes[3..6], 0x31, byte, byte)
            || !register_instruction(&bytes[6..9], 0x85, count, count)
            || bytes[9] != 0x74
            || 11_i16 + i16::from(bytes[10] as i8) != 36
            || !indexed_access(&bytes[11..20], true, source, cursor, byte)
            || !indexed_access(&bytes[20..28], false, destination, cursor, byte)
            || bytes[28] & 0xfe != 0x48
            || bytes[29] != 0xff
            || bytes[30] & 0xf8 != 0xc0
            || (bytes[30] & 7) | ((bytes[28] & 1) << 3) != cursor
            || !register_instruction(&bytes[31..34], 0x39, count, cursor)
            || bytes[34] != 0x72
            || 36_i16 + i16::from(bytes[35] as i8) != 11
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        super::canonical_effects_encoding(
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

    pub(super) fn register_instruction(
        bytes: &[u8],
        opcode: u8,
        register: u8,
        destination: u8,
    ) -> bool {
        let [prefix, operation, mode] = bytes else {
            return false;
        };
        prefix & 0xfa == 0x48
            && *operation == opcode
            && mode >> 6 == 3
            && ((mode >> 3) & 7) | ((prefix & 4) << 1) == register
            && (mode & 7) | ((prefix & 1) << 3) == destination
    }

    fn indexed_access(bytes: &[u8], load: bool, base: u8, cursor: u8, byte: u8) -> bool {
        let (prefix, mode, sib, displacement) = if load {
            let [prefix, 0x0f, 0xb6, mode, sib, displacement @ ..] = bytes else {
                return false;
            };
            (*prefix, *mode, *sib, displacement)
        } else {
            let [prefix, 0x88, mode, sib, displacement @ ..] = bytes else {
                return false;
            };
            (*prefix, *mode, *sib, displacement)
        };
        prefix & 0xf8 == if load { 0x48 } else { 0x40 }
            && mode & 0xc7 == 0x84
            && sib >> 6 == 0
            && displacement == [0; 4]
            && ((mode >> 3) & 7) | ((prefix & 4) << 1) == byte
            && (sib & 7) | ((prefix & 1) << 3) == base
            && ((sib >> 3) & 7) | ((prefix & 2) << 2) == cursor
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<[u8; 5], X86_64SelectedFormEncodingError> {
        let registers =
            byte_loop_registers::<5>(physical, alternative, Family::CopyBytes, operands, 3)?;
        if displacement != 0 {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(registers)
    }

    #[cfg(test)]
    mod tests {
        use super::super::{
            SelectedInstructionKind as Kind, encode_x86_64_selected_form,
            validate_x86_64_selected_form_encoding,
        };
        use super::{Family, MachineAlternativeKey, encode, validate};

        #[test]
        fn copy_bytes_catalog_requires_both_early_clobbers_and_exact_dynamic_effect() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let catalog = crate::x86_64_register_constraint_catalog(&physical);
            let constraints =
                crate::validate_x86_64_register_constraint_catalog(catalog.clone(), &physical)
                    .unwrap();
            crate::selected_form_encoding::tests::assert_early_clobber_required(
                &catalog,
                &physical,
                crate::X86_64_COPY_BYTES,
                &[3, 4],
            );
            for target in [
                target::NativeTarget::linux_x64(),
                target::NativeTarget::windows_x64(),
            ] {
                let effects = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
                crate::validate_x86_64_machine_effect_catalog(
                    target,
                    &constraints,
                    effects.clone(),
                )
                .unwrap();
                let row = effects
                    .declaration_by_semantic(selected_instructions::MachineSemanticKind::CopyBytes)
                    .unwrap();
                assert_eq!(
                    row.memory,
                    selected_instructions::MachineMemoryEffect::CopyBytesV1
                );
                assert_eq!(
                    row.alternatives[0].encoded.external_operand_reads,
                    [0, 1, 2]
                );
                assert_eq!(row.alternatives[0].encoded.external_operand_writes, [3, 4]);
            }
        }

        #[test]
        fn copy_bytes_preserves_inputs_and_rejects_every_changed_encoding_bit() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let alternative = MachineAlternativeKey {
                family: Family::CopyBytes,
                variant: 0,
            };
            for names in [
                ["r13", "r14", "r15", "r12", "rbp"],
                ["rax", "rcx", "rdx", "rsi", "rdi"],
            ] {
                let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
                let encoded = encode(&physical, alternative, &operands, 0).unwrap();
                assert_eq!(
                    encode_x86_64_selected_form(&physical, Kind::CopyBytes, alternative, &operands)
                        .unwrap(),
                    encoded
                );
                validate_x86_64_selected_form_encoding(
                    &physical,
                    Kind::CopyBytes,
                    alternative,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), 36);
                assert_eq!(encoded.footprint().register_reads, operands[..3]);
                assert_eq!(encoded.footprint().register_writes, operands[3..]);
                crate::selected_form_encoding::assert_every_bit_flip_rejected(
                    encoded.bytes(),
                    |changed| validate(&physical, alternative, &operands, 0, changed).is_err(),
                );
                super::super::assert_byte_fragility(encoded.bytes(), |bytes| {
                    validate(&physical, alternative, &operands, 0, bytes).is_err()
                });
                assert!(encode(&physical, alternative, &operands, 1).is_err());
                for operand in 0..5 {
                    let mut changed = operands;
                    changed[operand] = physical.model().view_named("rsp").unwrap().id;
                    assert!(encode(&physical, alternative, &changed, 0).is_err());
                }
                for scratch in 3..5 {
                    for other in 0..5 {
                        if other == scratch {
                            continue;
                        }
                        let mut changed = operands;
                        changed[scratch] = operands[other];
                        assert!(encode(&physical, alternative, &changed, 0).is_err());
                    }
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
            MachineSizeKnowledge::ExactBytes(2),
            MachineEncodedEffects::crash_v1(&constraint.implicit_uses),
        )
    }

    pub(crate) fn privileged_declaration(
        constraint: &RegisterInstructionConstraint,
    ) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            MachineSemanticKind::PrivilegedCrash,
            constraint.key,
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            MachineBarrier::ControlFlow,
            MachineAlternativeFamily::PrivilegedCrash,
            MachineSizeKnowledge::ExactBytes(4),
            MachineEncodedEffects::privileged_stop_v1(
                &constraint.implicit_uses,
                &constraint.clobbers,
            ),
        )
    }

    #[cfg(test)]
    mod tests {
        use crate::selected_form_encoding::ValidatedX86_64SelectedFormEncoding;
        use selected_instructions::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, fixture,
        };

        impl fixture::EncodedWord for ValidatedX86_64SelectedFormEncoding {
            fn encoded_bytes(&self) -> &[u8] {
                self.bytes()
            }

            fn encoded_effects(&self) -> &MachineEncodedEffects {
                &self.footprint().encoded
            }
        }

        #[test]
        fn privileged_stop_has_exact_bytes_and_control_effects() {
            use selected_instructions::SelectedInstructionKind;
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let kind = SelectedInstructionKind::PrivilegedCrash;
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::PrivilegedCrash,
                variant: 0,
            };
            let encoded =
                crate::encode_x86_64_selected_form(&physical, kind, alternative, &[]).unwrap();
            assert_eq!(encoded.bytes(), [0xfa, 0xf4, 0xeb, 0xfd]);
            let units = |name| physical.model().view_named(name).unwrap().units.clone();
            assert_eq!(
                encoded.footprint().encoded,
                MachineEncodedEffects::privileged_stop_v1(&units("rip"), &units("control_flags"))
            );
            for bytes in [
                vec![0x0f, 0x0b],
                vec![0xfa, 0xf4],
                vec![0xfa, 0xf4, 0xeb, 0xfe],
                vec![0xfa, 0xf4, 0xeb, 0xfd, 0x90],
            ] {
                assert!(
                    crate::validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        &bytes
                    )
                    .is_err()
                );
            }
            assert!(
                crate::encode_x86_64_selected_form(
                    &physical,
                    kind,
                    alternative,
                    &[physical.model().view_named("rax").unwrap().id]
                )
                .is_err()
            );
            assert!(
                crate::validate_x86_64_selected_form_encoding(
                    &physical,
                    SelectedInstructionKind::Crash,
                    alternative,
                    &[],
                    encoded.bytes()
                )
                .is_err()
            );
        }

        #[test]
        fn crash_rejects_changed_bytes_operands_and_alternatives() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            fixture::assert_crash_refusals(
                fixture::CrashWord {
                    physical: &physical,
                    bytes: &[0x0f, 0x0b],
                    program_counter: "rip",
                    foreign_operand: "rax",
                },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::Crash,
                    variant: 0,
                },
                |kind, alternative, operands| {
                    crate::encode_x86_64_selected_form(&physical, kind, alternative, operands)
                },
                |kind, alternative, operands, bytes| {
                    crate::validate_x86_64_selected_form_encoding(
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
    use crate::register_model::units_of;
    use crate::selected_form_encoding::instruction_bytes::{integer_bits, u12};
    use crate::selected_form_encoding::saturating_forms::{SaturatingForm, saturating_operation};
    use crate::selected_form_encoding::trapping_forms::{
        DividendGuard, RangeCheck, TrappingArithmetic, TrappingShape, trapping_form,
    };
    use crate::selected_form_encoding::{
        X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
    };
    use register_model::RegisterViewId;
    use selected_instructions::{
        AtomicOperation, AtomicWidth, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, MachineSemanticKind, SaturatingCarrier, SaturatingOperation,
        SelectedInstructionKind as Kind, TrappingForm, dedicated_selected_forms,
        exact_add_or_sub_pair_pattern, exact_shift_i64_pair_pattern,
        exact_wrapping_divrem_set_pattern, layout_dependent_selected_transfers,
        saturating_add_or_subtract_pattern,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum DecodedInstruction {
        Crash,
        PrivilegedStop,
        SignedDivide {
            divisor: u8,
        },
        SignExtendDividend,
        JumpEqualShort {
            displacement: i8,
        },
        JumpNotEqualShort {
            displacement: i8,
        },
        JumpShort {
            displacement: i8,
        },
        JumpNoOverflowShort {
            displacement: i8,
        },
        JumpBelowShort {
            displacement: i8,
        },
        JumpAboveOrEqualShort {
            displacement: i8,
        },
        CompareSignedImmediate8 {
            register: u8,
            immediate: i8,
        },
        CompareRaxImmediate32 {
            immediate: i32,
        },
        ShiftRightLogicalImmediate {
            destination: u8,
            count: u8,
        },
        UnsignedDivide {
            divisor: u8,
        },
        UnsignedMultiply {
            multiplier: u8,
        },
        Complement {
            destination: u8,
        },
        MoveOnBorrow {
            source: u8,
            destination: u8,
        },
        MoveOnGreater {
            source: u8,
            destination: u8,
        },
        MoveOnLess {
            source: u8,
            destination: u8,
        },
        MoveOnAbove {
            source: u8,
            destination: u8,
        },
        MoveOnOverflow {
            source: u8,
            destination: u8,
        },
        ArithmeticShiftRight63 {
            destination: u8,
        },
        ComplementBit63 {
            destination: u8,
        },
        SubtractWithBorrow {
            source: u8,
            destination: u8,
        },
        Or {
            source: u8,
            destination: u8,
        },
        BitwiseAnd {
            source: u8,
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
        Materialize {
            destination: u8,
            value: u64,
        },
        Move {
            source: u8,
            destination: u8,
        },
        Test {
            register: u8,
        },
        Compare {
            left: u8,
            right: u8,
        },
        CompareImmediate {
            register: u8,
            immediate: u32,
        },
        Lea {
            destination: u8,
            base: u8,
            index: Option<u8>,
            displacement: i32,
        },
        Xor {
            source: u8,
            destination: u8,
        },
        Subtract {
            source: u8,
            destination: u8,
        },
        Multiply {
            source: u8,
            destination: u8,
        },
        ShiftLeftByCl {
            destination: u8,
        },
        ShiftRightLogicalByCl {
            destination: u8,
        },
        ShiftRightArithmeticByCl {
            destination: u8,
        },
        Negate {
            destination: u8,
        },
        AtomicLoad {
            destination: u8,
            base: u8,
            bits: u16,
        },
        AtomicStore {
            source: u8,
            base: u8,
            bits: u16,
        },
        MemoryFence,
        AtomicExchange {
            register: u8,
            base: u8,
            bits: u16,
        },
        AtomicLockedAdd {
            register: u8,
            base: u8,
            bits: u16,
        },
        AtomicBitwise {
            operation: AtomicOperation,
            source: u8,
            destination: u8,
            bits: u16,
        },
        AtomicLockedCompareExchange {
            register: u8,
            base: u8,
            bits: u16,
        },
        Add {
            source: u8,
            destination: u8,
        },
        Return,
        JumpIndirectRip {
            displacement: i32,
        },
        UnresolvedRipRelativeAddress {
            destination: u8,
        },
    }

    fn mov(source: u8, destination: u8) -> DecodedInstruction {
        DecodedInstruction::Move {
            source,
            destination,
        }
    }

    pub(crate) fn decode_all(
        bytes: &[u8],
    ) -> Result<Vec<DecodedInstruction>, X86_64SelectedFormEncodingError> {
        if bytes.is_empty() {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let mut decoded = Vec::new();
        let mut offset = 0;
        while offset < bytes.len() {
            let (instruction, length) = decode_one(&bytes[offset..])?;
            decoded.push(instruction);
            offset = offset
                .checked_add(length)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        }
        Ok(decoded)
    }

    fn modrm_source_destination(modrm: u8, rex: u8) -> (u8, u8) {
        (
            (modrm & 7) | ((rex & 1) << 3),
            ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
        )
    }

    pub(crate) fn decode_one(
        bytes: &[u8],
    ) -> Result<(DecodedInstruction, usize), X86_64SelectedFormEncodingError> {
        if let [0x48, 0x99, ..] = bytes {
            return Ok((DecodedInstruction::SignExtendDividend, 2));
        }
        if let [0x74, displacement, ..] = bytes {
            return Ok((
                DecodedInstruction::JumpEqualShort {
                    displacement: *displacement as i8,
                },
                2,
            ));
        }
        if let [0x75, displacement, ..] = bytes {
            return Ok((
                DecodedInstruction::JumpNotEqualShort {
                    displacement: *displacement as i8,
                },
                2,
            ));
        }
        if let [0xeb, displacement, ..] = bytes {
            return Ok((
                DecodedInstruction::JumpShort {
                    displacement: *displacement as i8,
                },
                2,
            ));
        }
        if let [opcode @ (0x71..=0x73), displacement, ..] = bytes {
            let displacement = *displacement as i8;
            return Ok((
                match *opcode {
                    0x71 => DecodedInstruction::JumpNoOverflowShort { displacement },
                    0x72 => DecodedInstruction::JumpBelowShort { displacement },
                    _ => DecodedInstruction::JumpAboveOrEqualShort { displacement },
                },
                2,
            ));
        }
        if let [0x48, 0x3d, i0, i1, i2, i3, ..] = bytes {
            return Ok((
                DecodedInstruction::CompareRaxImmediate32 {
                    immediate: i32::from_le_bytes([*i0, *i1, *i2, *i3]),
                },
                6,
            ));
        }
        if let [rex, 0xc1, modrm, count, ..] = bytes
            && rex & !1 == 0x48
            && modrm & 0xf8 == 0xe8
        {
            return Ok((
                DecodedInstruction::ShiftRightLogicalImmediate {
                    destination: (modrm & 7) | ((rex & 1) << 3),
                    count: *count,
                },
                4,
            ));
        }
        if let [rex, 0x83, modrm, immediate, ..] = bytes
            && rex & !1 == 0x48
            && modrm & 0xf8 == 0xf8
        {
            return Ok((
                DecodedInstruction::CompareSignedImmediate8 {
                    register: (modrm & 7) | ((rex & 1) << 3),
                    immediate: *immediate as i8,
                },
                4,
            ));
        }
        if let [rex, 0x0f, opcode, modrm, ..] = bytes
            && rex & !0x05 == 0x48
            && matches!(*opcode, 0x40 | 0x42 | 0x47 | 0x4c | 0x4f)
            && modrm & 0xc0 == 0xc0
        {
            let source = (modrm & 7) | ((rex & 1) << 3);
            let destination = ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3);
            return Ok((
                match opcode {
                    0x40 => DecodedInstruction::MoveOnOverflow {
                        source,
                        destination,
                    },
                    0x42 => DecodedInstruction::MoveOnBorrow {
                        source,
                        destination,
                    },
                    0x47 => DecodedInstruction::MoveOnAbove {
                        source,
                        destination,
                    },
                    0x4c => DecodedInstruction::MoveOnLess {
                        source,
                        destination,
                    },
                    _ => DecodedInstruction::MoveOnGreater {
                        source,
                        destination,
                    },
                },
                4,
            ));
        }
        if let [rex, 0x0f, 0xaf, modrm, ..] = bytes
            && rex & !0x05 == 0x48
            && modrm & 0xc0 == 0xc0
        {
            let (source, destination) = modrm_source_destination(*modrm, *rex);
            return Ok((
                DecodedInstruction::Multiply {
                    source,
                    destination,
                },
                4,
            ));
        }
        if let [rex, 0xc1, modrm, 63, ..] = bytes
            && rex & !1 == 0x48
            && modrm & 0xf8 == 0xf8
        {
            return Ok((
                DecodedInstruction::ArithmeticShiftRight63 {
                    destination: (modrm & 7) | ((rex & 1) << 3),
                },
                4,
            ));
        }
        if let [rex, 0x0f, 0xba, modrm, 63, ..] = bytes
            && rex & !1 == 0x48
            && modrm & 0xf8 == 0xf8
        {
            return Ok((
                DecodedInstruction::ComplementBit63 {
                    destination: (modrm & 7) | ((rex & 1) << 3),
                },
                5,
            ));
        }
        if let [rex, 0x0f, opcode, modrm, ..] = bytes
            && rex & !1 == 0x40
            && matches!(*opcode, 0x92 | 0x94 | 0x96 | 0x9c | 0x9e)
            && modrm & 0xf8 == 0xc0
        {
            return Ok((
                DecodedInstruction::SetBoolean {
                    condition: opcode & 0x0f,
                    destination: (modrm & 7) | ((rex & 1) << 3),
                },
                4,
            ));
        }
        if let [rex, 0x0f, 0xb6, modrm, ..] = bytes
            && rex & !0x05 == 0x40
            && modrm & 0xc0 == 0xc0
        {
            let (source, destination) = modrm_source_destination(*modrm, *rex);
            return Ok((
                DecodedInstruction::ZeroExtendU8 {
                    source,
                    destination,
                },
                4,
            ));
        }
        if let [rex, 0x0f, 0xb7, modrm, ..] = bytes
            && rex & !0x05 == 0x40
            && modrm & 0xc0 == 0xc0
        {
            let (source, destination) = modrm_source_destination(*modrm, *rex);
            return Ok((
                DecodedInstruction::ZeroExtendU16 {
                    source,
                    destination,
                },
                4,
            ));
        }
        if let [rex, 0x0f, 0xbe, modrm, ..] = bytes
            && rex & !0x05 == 0x48
            && modrm & 0xc0 == 0xc0
        {
            let (source, destination) = modrm_source_destination(*modrm, *rex);
            return Ok((
                DecodedInstruction::SignExtendI8 {
                    source,
                    destination,
                },
                4,
            ));
        }
        if let [rex, 0x0f, 0xbf, modrm, ..] = bytes
            && rex & !0x05 == 0x48
            && modrm & 0xc0 == 0xc0
        {
            let (source, destination) = modrm_source_destination(*modrm, *rex);
            return Ok((
                DecodedInstruction::SignExtendI16 {
                    source,
                    destination,
                },
                4,
            ));
        }
        if let [rex, 0x63, modrm, ..] = bytes
            && rex & !0x05 == 0x48
            && modrm & 0xc0 == 0xc0
        {
            let (source, destination) = modrm_source_destination(*modrm, *rex);
            return Ok((
                DecodedInstruction::SignExtendI32 {
                    source,
                    destination,
                },
                3,
            ));
        }
        if let [rex, 0x89, modrm, ..] = bytes
            && rex & !0x05 == 0x40
            && modrm & 0xc0 == 0xc0
        {
            return Ok((
                DecodedInstruction::ZeroExtendU32 {
                    source: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
                    destination: (modrm & 7) | ((rex & 1) << 3),
                },
                3,
            ));
        }
        if let [0xff, 0x25, d0, d1, d2, d3, ..] = bytes {
            return Ok((
                DecodedInstruction::JumpIndirectRip {
                    displacement: i32::from_le_bytes([*d0, *d1, *d2, *d3]),
                },
                6,
            ));
        }
        if bytes.starts_with(&crate::PRIVILEGED_STOP_BYTES) {
            return Ok((
                DecodedInstruction::PrivilegedStop,
                crate::PRIVILEGED_STOP_BYTES.len(),
            ));
        }
        if bytes.starts_with(&[0x0f, 0x0b]) {
            return Ok((DecodedInstruction::Crash, 2));
        }
        if bytes.first() == Some(&0xc3) {
            return Ok((DecodedInstruction::Return, 1));
        }
        if let Some(decoded) = decode_atomic(bytes) {
            return Ok(decoded);
        }
        let (&rex, rest) = bytes
            .split_first()
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        if !(0x48..=0x4f).contains(&rex) {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let (&opcode, _) = rest
            .split_first()
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        let rex_r = (rex >> 2) & 1;
        let rex_x = (rex >> 1) & 1;
        let rex_b = rex & 1;
        if opcode == 0x8d
            && rex_x == 0
            && rex_b == 0
            && let Some([modrm, 0, 0, 0, 0]) = bytes
                .get(2..7)
                .and_then(|field| <[u8; 5]>::try_from(field).ok())
            && modrm & 0xc7 == 0x05
        {
            return Ok((
                DecodedInstruction::UnresolvedRipRelativeAddress {
                    destination: ((modrm >> 3) & 7) | (rex_r << 3),
                },
                7,
            ));
        }
        if (0xb8..=0xbf).contains(&opcode) {
            let immediate = bytes
                .get(2..10)
                .and_then(|bytes| bytes.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            return Ok((
                DecodedInstruction::Materialize {
                    destination: (opcode & 7) | (rex_b << 3),
                    value: immediate,
                },
                10,
            ));
        }
        let modrm = *bytes
            .get(2)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        let mode = modrm >> 6;
        let reg = ((modrm >> 3) & 7) | (rex_r << 3);
        let rm_low = modrm & 7;
        let rm = rm_low | (rex_b << 3);
        if opcode == 0x81 {
            if mode != 3 || reg != 7 {
                return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
            }
            let immediate = bytes
                .get(3..7)
                .and_then(|bytes| bytes.try_into().ok())
                .map(u32::from_le_bytes)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            return Ok((
                DecodedInstruction::CompareImmediate {
                    register: rm,
                    immediate,
                },
                7,
            ));
        }
        if matches!(
            opcode,
            0x89 | 0x85 | 0x39 | 0x31 | 0x29 | 0x01 | 0x21 | 0x09 | 0x19 | 0xf7 | 0xd3
        ) {
            if mode != 3 || bytes.len() < 3 {
                return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
            }
            let decoded = match opcode {
                0x89 => mov(reg, rm),
                0x85 if reg == rm => DecodedInstruction::Test { register: reg },
                0x39 => DecodedInstruction::Compare {
                    left: rm,
                    right: reg,
                },
                0x31 => DecodedInstruction::Xor {
                    source: reg,
                    destination: rm,
                },
                0x29 => DecodedInstruction::Subtract {
                    source: reg,
                    destination: rm,
                },
                0x21 => DecodedInstruction::BitwiseAnd {
                    source: reg,
                    destination: rm,
                },
                0x09 => DecodedInstruction::Or {
                    source: reg,
                    destination: rm,
                },
                0x19 => DecodedInstruction::SubtractWithBorrow {
                    source: reg,
                    destination: rm,
                },
                0x01 => DecodedInstruction::Add {
                    source: reg,
                    destination: rm,
                },
                0xf7 if reg == 2 && rex_x == 0 => {
                    DecodedInstruction::Complement { destination: rm }
                }
                0xf7 if reg == 4 && rex_x == 0 => {
                    DecodedInstruction::UnsignedMultiply { multiplier: rm }
                }
                0xf7 if reg == 6 && rex_x == 0 => {
                    DecodedInstruction::UnsignedDivide { divisor: rm }
                }
                0xf7 if reg == 7 && rex_x == 0 => DecodedInstruction::SignedDivide { divisor: rm },
                0xf7 if (modrm >> 3) & 7 == 3 => DecodedInstruction::Negate { destination: rm },
                0xd3 if reg == 4 => DecodedInstruction::ShiftLeftByCl { destination: rm },
                0xd3 if reg == 5 => DecodedInstruction::ShiftRightLogicalByCl { destination: rm },
                0xd3 if reg == 7 => {
                    DecodedInstruction::ShiftRightArithmeticByCl { destination: rm }
                }
                _ => return Err(X86_64SelectedFormEncodingError::MalformedEncoding),
            };
            return Ok((decoded, 3));
        }
        if opcode != 0x8d || mode == 3 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let mut length = 3;
        let (base, index) = if rm_low == 4 {
            let sib = *bytes
                .get(length)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            length += 1;
            if sib >> 6 != 0 {
                return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
            }
            let index_low = (sib >> 3) & 7;
            let base_low = sib & 7;
            if mode == 0 && base_low == 5 {
                return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
            }
            (
                base_low | (rex_b << 3),
                if index_low == 4 && rex_x == 0 {
                    None
                } else {
                    Some(index_low | (rex_x << 3))
                },
            )
        } else {
            if mode == 0 && rm_low == 5 {
                return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
            }
            (rm, None)
        };
        let displacement = match mode {
            0 => 0,
            1 => {
                let value = *bytes
                    .get(length)
                    .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
                length += 1;
                i32::from(value as i8)
            }
            2 => {
                let value = bytes
                    .get(length..length + 4)
                    .and_then(|bytes| bytes.try_into().ok())
                    .map(i32::from_le_bytes)
                    .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
                length += 4;
                value
            }
            _ => return Err(X86_64SelectedFormEncodingError::MalformedEncoding),
        };
        Ok((
            DecodedInstruction::Lea {
                destination: reg,
                base,
                index,
                displacement,
            },
            length,
        ))
    }

    fn decode_atomic(bytes: &[u8]) -> Option<(DecodedInstruction, usize)> {
        let mut cursor = 0;
        let operand_sized = bytes.first() == Some(&0x66);
        if operand_sized {
            cursor += 1;
        }
        let locked = bytes.get(cursor) == Some(&0xf0);
        if locked {
            cursor += 1;
        }
        let rex = match bytes.get(cursor) {
            Some(&byte) if (0x40..=0x4f).contains(&byte) => {
                cursor += 1;
                byte
            }
            _ => 0x40,
        };
        let opcode = *bytes.get(cursor)?;
        if opcode == 0x0f {
            let second = *bytes.get(cursor + 1)?;
            if second == 0xae {
                return (bytes.get(cursor + 2) == Some(&0xf0))
                    .then_some((DecodedInstruction::MemoryFence, cursor + 3));
            }
            cursor += 2;
            let modrm = *bytes.get(cursor)?;
            cursor += 1;
            if modrm >> 6 != 2 {
                return None;
            }
            let register = ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3);
            let base = (modrm & 7) | ((rex & 1) << 3);
            if modrm & 7 == 4 {
                let sib = *bytes.get(cursor)?;
                cursor += 1;
                if sib != 0x24 {
                    return None;
                }
            }
            if !matches!(bytes.get(cursor..cursor + 4), Some(&[0, 0, 0, 0])) {
                return None;
            }
            cursor += 4;
            let wide = rex & 8 != 0;
            let bits = |eight: bool| -> u16 {
                if eight {
                    8
                } else if operand_sized {
                    16
                } else if wide {
                    64
                } else {
                    32
                }
            };
            let instruction = match second {
                0xb6 if !locked => DecodedInstruction::AtomicLoad {
                    destination: register,
                    base,
                    bits: 8,
                },
                0xb7 if !locked => DecodedInstruction::AtomicLoad {
                    destination: register,
                    base,
                    bits: 16,
                },
                0xb0 if locked => DecodedInstruction::AtomicLockedCompareExchange {
                    register,
                    base,
                    bits: 8,
                },
                0xb1 if locked => DecodedInstruction::AtomicLockedCompareExchange {
                    register,
                    base,
                    bits: bits(false),
                },
                0xc0 if locked => DecodedInstruction::AtomicLockedAdd {
                    register,
                    base,
                    bits: 8,
                },
                0xc1 if locked => DecodedInstruction::AtomicLockedAdd {
                    register,
                    base,
                    bits: bits(false),
                },
                _ => return None,
            };
            return Some((instruction, cursor));
        }
        let modrm = *bytes.get(cursor + 1)?;
        match opcode {
            0x20 | 0x21 | 0x08 | 0x09 | 0x30 | 0x31 if !locked && modrm >> 6 == 3 => {
                let operation = match opcode & !1 {
                    0x20 => AtomicOperation::FetchAnd,
                    0x08 => AtomicOperation::FetchOr,
                    0x30 => AtomicOperation::FetchXor,
                    _ => unreachable!(),
                };
                if rex & 8 != 0 && opcode & 1 != 0 && !operand_sized {
                    return None;
                }
                Some((
                    DecodedInstruction::AtomicBitwise {
                        operation,
                        source: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
                        destination: (modrm & 7) | ((rex & 1) << 3),
                        bits: if opcode & 1 == 0 {
                            8
                        } else if operand_sized {
                            16
                        } else {
                            32
                        },
                    },
                    cursor + 2,
                ))
            }
            0x8b | 0x88 | 0x89 | 0x86 | 0x87 if modrm >> 6 == 2 => {
                let register = ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3);
                let base = (modrm & 7) | ((rex & 1) << 3);
                let mut length = cursor + 2;
                if modrm & 7 == 4 {
                    if bytes.get(length) != Some(&0x24) {
                        return None;
                    }
                    length += 1;
                }
                if !matches!(bytes.get(length..length + 4), Some(&[0, 0, 0, 0])) {
                    return None;
                }
                length += 4;
                let wide = rex & 8 != 0;
                let bits = |eight: bool| -> u16 {
                    if eight {
                        8
                    } else if operand_sized {
                        16
                    } else if wide {
                        64
                    } else {
                        32
                    }
                };
                let instruction = match opcode {
                    0x8b => DecodedInstruction::AtomicLoad {
                        destination: register,
                        base,
                        bits: if wide { 64 } else { 32 },
                    },
                    0x88 => DecodedInstruction::AtomicStore {
                        source: register,
                        base,
                        bits: 8,
                    },
                    0x89 => DecodedInstruction::AtomicStore {
                        source: register,
                        base,
                        bits: bits(false),
                    },
                    0x86 => DecodedInstruction::AtomicExchange {
                        register,
                        base,
                        bits: 8,
                    },
                    _ => DecodedInstruction::AtomicExchange {
                        register,
                        base,
                        bits: bits(false),
                    },
                };
                Some((instruction, length))
            }
            0xf6 | 0xf7 if modrm >> 6 == 3 && (modrm >> 3) & 7 == 3 => Some((
                DecodedInstruction::Negate {
                    destination: (modrm & 7) | ((rex & 1) << 3),
                },
                cursor + 2,
            )),
            _ => None,
        }
    }

    fn divide_register_layout(registers: &[u8]) -> bool {
        registers[0] == 0 && registers[2] == 0 && registers[3] == 2 && registers[1] != 2
    }

    pub(crate) fn validate_decoded(
        kind: Kind,
        alternative: MachineAlternativeKey,
        registers: &[u8],
        decoded: &[DecodedInstruction],
    ) -> Result<(), X86_64SelectedFormEncodingError> {
        let valid = match kind {
            Kind::InstalledInterruptCpuCheck { .. }
            | Kind::InstalledInterruptTaskCommit { .. }
            | Kind::InstalledInterruptTablePublication { .. }
            | Kind::RegisterAccess { .. } => {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            Kind::PortWrite { .. } => {
                unreachable!("port envelopes use the dedicated exact decoder")
            }
            Kind::AccessorScalarAccess {
                form, byte_offset, ..
            } => {
                let ordinary = form
                    .ordinary_kind(byte_offset)
                    .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
                return validate_decoded(
                    ordinary,
                    selected_instructions::MachineAlternativeKey {
                        family: (if form.write {
                            MachineSemanticKind::Store
                        } else {
                            match form.bytes {
                                1 => MachineSemanticKind::Load8,
                                2 => MachineSemanticKind::Load16,
                                4 => MachineSemanticKind::Load32,
                                _ => MachineSemanticKind::Load64,
                            }
                        })
                        .into(),
                        variant: alternative.variant,
                    },
                    registers,
                    decoded,
                );
            }
            Kind::AtomicAccess { form } => {
                let base = registers[0];
                let bits = match form.width {
                    AtomicWidth::Bits8 => 8,
                    AtomicWidth::Bits16 => 16,
                    AtomicWidth::Bits32 => 32,
                    AtomicWidth::Bits64 => 64,
                };
                match form.operation {
                    AtomicOperation::Load => {
                        decoded
                            == [DecodedInstruction::AtomicLoad {
                                destination: registers[1],
                                base,
                                bits,
                            }]
                    }
                    AtomicOperation::Store => {
                        decoded
                            == [
                                DecodedInstruction::AtomicStore {
                                    source: registers[1],
                                    base,
                                    bits,
                                },
                                DecodedInstruction::MemoryFence,
                            ]
                    }
                    AtomicOperation::Swap => {
                        decoded
                            == [
                                mov(registers[1], registers[2]),
                                DecodedInstruction::AtomicExchange {
                                    register: registers[2],
                                    base,
                                    bits,
                                },
                            ]
                    }
                    AtomicOperation::FetchAdd => {
                        decoded
                            == [
                                mov(registers[1], registers[2]),
                                DecodedInstruction::AtomicLockedAdd {
                                    register: registers[2],
                                    base,
                                    bits,
                                },
                            ]
                    }
                    AtomicOperation::FetchSub => {
                        decoded
                            == [
                                mov(registers[1], registers[2]),
                                DecodedInstruction::Negate {
                                    destination: registers[2],
                                },
                                DecodedInstruction::AtomicLockedAdd {
                                    register: registers[2],
                                    base,
                                    bits,
                                },
                            ]
                    }
                    AtomicOperation::CompareExchange | AtomicOperation::CompareExchangeOnce => {
                        let mut expected = vec![
                            DecodedInstruction::AtomicLockedCompareExchange {
                                register: registers[2],
                                base,
                                bits,
                            },
                            mov(0, registers[3]),
                        ];
                        if let Some(outcome) = form.outcome_operand() {
                            let destination = registers[usize::from(outcome)];
                            expected.extend([
                                DecodedInstruction::SetBoolean {
                                    condition: 4,
                                    destination,
                                },
                                DecodedInstruction::ZeroExtendU8 {
                                    source: destination,
                                    destination,
                                },
                            ]);
                        }
                        decoded == expected
                    }
                    operation @ (AtomicOperation::FetchAnd
                    | AtomicOperation::FetchOr
                    | AtomicOperation::FetchXor) => {
                        let source = registers[1];
                        let destination = registers[2];
                        let bitwise = if bits == 64 {
                            match operation {
                                AtomicOperation::FetchAnd => DecodedInstruction::BitwiseAnd {
                                    source,
                                    destination,
                                },
                                AtomicOperation::FetchOr => DecodedInstruction::Or {
                                    source,
                                    destination,
                                },
                                AtomicOperation::FetchXor => DecodedInstruction::Xor {
                                    source,
                                    destination,
                                },
                                _ => unreachable!(),
                            }
                        } else {
                            DecodedInstruction::AtomicBitwise {
                                operation,
                                source,
                                destination,
                                bits,
                            }
                        };
                        let width_prefix = usize::from(bits == 16);
                        let bit_rex =
                            usize::from(bits == 8 || bits == 64 || source >= 8 || destination >= 8);
                        let cmp_rex =
                            usize::from(bits == 8 || bits == 64 || destination >= 8 || base >= 8);
                        let retry_length = 3
                            + width_prefix
                            + bit_rex
                            + 2
                            + 1
                            + width_prefix
                            + cmp_rex
                            + 3
                            + usize::from(base & 7 == 4)
                            + 4
                            + 2;
                        decoded
                            == [
                                DecodedInstruction::AtomicLoad {
                                    destination: 0,
                                    base,
                                    bits,
                                },
                                DecodedInstruction::Move {
                                    source: 0,
                                    destination,
                                },
                                bitwise,
                                DecodedInstruction::AtomicLockedCompareExchange {
                                    register: destination,
                                    base,
                                    bits,
                                },
                                DecodedInstruction::JumpNotEqualShort {
                                    displacement: -(retry_length as i8),
                                },
                                DecodedInstruction::Move {
                                    source: 0,
                                    destination,
                                },
                            ]
                    }
                }
            }
            Kind::NearestIeeeFloatArithmetic { .. } | Kind::FloatResultPolicy { .. } => false,
            Kind::AtomicFence => {
                registers.is_empty() && decoded == [DecodedInstruction::MemoryFence]
            }
            Kind::Crash => registers.is_empty() && decoded == [DecodedInstruction::Crash],
            Kind::PrivilegedCrash => {
                registers.is_empty() && decoded == [DecodedInstruction::PrivilegedStop]
            }
            Kind::MaterializeI64 { value } if alternative.variant == 1 => {
                integer_bits(value)? == 0
                    && decoded
                        == [DecodedInstruction::Xor {
                            source: registers[0],
                            destination: registers[0],
                        }]
            }
            Kind::MaterializeI64 { value } => {
                decoded
                    == [DecodedInstruction::Materialize {
                        destination: registers[0],
                        value: integer_bits(value)?,
                    }]
            }
            Kind::ImageAddress { .. } => {
                decoded
                    == [DecodedInstruction::UnresolvedRipRelativeAddress {
                        destination: registers[0],
                    }]
            }
            Kind::MaterializeBooleanEqual
            | Kind::MaterializeBooleanU64LessThan
            | Kind::MaterializeBooleanI64LessThan
            | Kind::MaterializeBooleanU64LessOrEqual
            | Kind::MaterializeBooleanI64LessOrEqual => {
                let condition = match kind {
                    Kind::MaterializeBooleanEqual => 4,
                    Kind::MaterializeBooleanU64LessThan => 2,
                    Kind::MaterializeBooleanI64LessThan => 12,
                    Kind::MaterializeBooleanU64LessOrEqual => 6,
                    Kind::MaterializeBooleanI64LessOrEqual => 14,
                    _ => unreachable!("Boolean condition arm"),
                };
                decoded
                    == [
                        DecodedInstruction::SetBoolean {
                            condition,
                            destination: registers[0],
                        },
                        DecodedInstruction::ZeroExtendU8 {
                            source: registers[0],
                            destination: registers[0],
                        },
                    ]
            }
            Kind::ZeroExtendU8 => {
                decoded
                    == [DecodedInstruction::ZeroExtendU8 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            Kind::ZeroExtendU16 => {
                decoded
                    == [DecodedInstruction::ZeroExtendU16 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            Kind::SignExtendI8 => {
                decoded
                    == [DecodedInstruction::SignExtendI8 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            Kind::SignExtendI16 => {
                decoded
                    == [DecodedInstruction::SignExtendI16 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            Kind::SignExtendI32 => {
                decoded
                    == [DecodedInstruction::SignExtendI32 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            Kind::ZeroExtendU32 => {
                decoded
                    == [DecodedInstruction::ZeroExtendU32 {
                        source: registers[0],
                        destination: registers[1],
                    }]
            }
            Kind::CopyI64 => decoded == [mov(registers[0], registers[1])],
            Kind::CompareI64Zero => {
                decoded
                    == [DecodedInstruction::Test {
                        register: registers[0],
                    }]
            }
            Kind::CompareI64 => {
                decoded
                    == [DecodedInstruction::Compare {
                        left: registers[0],
                        right: registers[1],
                    }]
            }
            Kind::CompareI64Immediate { immediate } => {
                decoded
                    == [DecodedInstruction::CompareImmediate {
                        register: registers[0],
                        immediate: u12(immediate)?,
                    }]
            }
            Kind::ByteViewAddress | Kind::WrappingAddI64 | Kind::ExactAddI64 { .. } => {
                matches!(decoded, [DecodedInstruction::Lea { destination, base, index: Some(index), displacement: 0 }]
                if *destination == registers[2]
                    && ((*base == registers[0] && *index == registers[1])
                        || (*base == registers[1] && *index == registers[0])))
            }
            Kind::ExactDivideU64 { .. } => {
                divide_register_layout(registers)
                    && decoded
                        == [DecodedInstruction::UnsignedDivide {
                            divisor: registers[1],
                        }]
            }
            Kind::ExactRemainderU64 { .. } => {
                divide_register_layout(registers)
                    && decoded
                        == [
                            DecodedInstruction::Xor {
                                source: 2,
                                destination: 2,
                            },
                            DecodedInstruction::UnsignedDivide {
                                divisor: registers[1],
                            },
                            mov(2, 0),
                        ]
            }
            Kind::WrappingDivideI64 { .. } => {
                divide_register_layout(registers)
                    && decoded
                        == [
                            DecodedInstruction::CompareSignedImmediate8 {
                                register: registers[1],
                                immediate: -1,
                            },
                            DecodedInstruction::JumpNotEqualShort { displacement: 5 },
                            DecodedInstruction::Negate { destination: 0 },
                            DecodedInstruction::JumpShort { displacement: 5 },
                            DecodedInstruction::SignExtendDividend,
                            DecodedInstruction::SignedDivide {
                                divisor: registers[1],
                            },
                        ]
            }
            Kind::ExactDivideI64 { .. } => {
                divide_register_layout(registers)
                    && decoded
                        == [
                            DecodedInstruction::SignExtendDividend,
                            DecodedInstruction::SignedDivide {
                                divisor: registers[1],
                            },
                        ]
            }
            Kind::ExactRemainderI64 { .. } => {
                divide_register_layout(registers)
                    && decoded
                        == [
                            DecodedInstruction::SignExtendDividend,
                            DecodedInstruction::SignedDivide {
                                divisor: registers[1],
                            },
                            mov(2, 0),
                        ]
            }
            Kind::WrappingRemainderI64 { .. } => {
                divide_register_layout(registers)
                    && decoded
                        == [
                            DecodedInstruction::Xor {
                                source: 2,
                                destination: 2,
                            },
                            DecodedInstruction::CompareSignedImmediate8 {
                                register: registers[1],
                                immediate: -1,
                            },
                            DecodedInstruction::JumpEqualShort { displacement: 5 },
                            DecodedInstruction::SignExtendDividend,
                            DecodedInstruction::SignedDivide {
                                divisor: registers[1],
                            },
                            mov(2, 0),
                        ]
            }
            Kind::SaturatingAdd { carrier } => {
                saturating_matches(SaturatingOperation::Add, carrier, registers, decoded)
            }
            Kind::SaturatingSubtract { carrier } => {
                saturating_matches(SaturatingOperation::Subtract, carrier, registers, decoded)
            }
            Kind::SaturatingDivide { carrier, .. } => {
                saturating_matches(SaturatingOperation::Divide, carrier, registers, decoded)
            }
            Kind::SaturatingRemainder { carrier, .. } => {
                saturating_matches(SaturatingOperation::Remainder, carrier, registers, decoded)
            }
            Kind::SaturatingMultiply { carrier } => {
                saturating_matches(SaturatingOperation::Multiply, carrier, registers, decoded)
            }
            Kind::TrappingInteger { form } => {
                TrappingShape::of(form).accepts_registers(registers)
                    && decoded == expected_trapping(form, registers)
            }
            Kind::BitwiseAndI64 | Kind::BitwiseOrI64 | Kind::BitwiseXorI64 => {
                let operation = |source, destination| match kind {
                    Kind::BitwiseXorI64 => DecodedInstruction::Xor {
                        source,
                        destination,
                    },
                    Kind::BitwiseOrI64 => DecodedInstruction::Or {
                        source,
                        destination,
                    },
                    _ => DecodedInstruction::BitwiseAnd {
                        source,
                        destination,
                    },
                };
                if registers[2] == registers[0] {
                    decoded == [operation(registers[1], registers[2])]
                } else if registers[2] == registers[1] {
                    decoded == [operation(registers[0], registers[2])]
                } else {
                    decoded
                        == [
                            mov(registers[0], registers[2]),
                            operation(registers[1], registers[2]),
                        ]
                }
            }
            Kind::ExactAddI64Immediate { immediate, .. } => {
                decoded
                    == [DecodedInstruction::Lea {
                        destination: registers[1],
                        base: registers[0],
                        index: None,
                        displacement: i32::try_from(u12(immediate)?).expect("u12 fits i32"),
                    }]
            }
            Kind::ExactSubtractI64Immediate { immediate, .. } => {
                decoded
                    == [DecodedInstruction::Lea {
                        destination: registers[1],
                        base: registers[0],
                        index: None,
                        displacement: -i32::try_from(u12(immediate)?).expect("u12 fits i32"),
                    }]
            }
            Kind::BitwiseNotI64 => {
                if registers[1] == registers[0] {
                    decoded
                        == [DecodedInstruction::Complement {
                            destination: registers[1],
                        }]
                } else {
                    decoded
                        == [
                            mov(registers[0], registers[1]),
                            DecodedInstruction::Complement {
                                destination: registers[1],
                            },
                        ]
                }
            }
            Kind::ExactSubtractI64 { .. } | Kind::WrappingSubtractI64 => {
                match alternative.variant {
                    0 => {
                        decoded
                            == [DecodedInstruction::Xor {
                                source: registers[2],
                                destination: registers[2],
                            }]
                    }
                    1 => {
                        decoded
                            == [DecodedInstruction::Subtract {
                                source: registers[1],
                                destination: registers[2],
                            }]
                    }
                    2 => {
                        decoded
                            == [
                                DecodedInstruction::Negate {
                                    destination: registers[2],
                                },
                                DecodedInstruction::Add {
                                    source: registers[0],
                                    destination: registers[2],
                                },
                            ]
                    }
                    3 => {
                        decoded
                            == [
                                mov(registers[0], registers[2]),
                                DecodedInstruction::Subtract {
                                    source: registers[1],
                                    destination: registers[2],
                                },
                            ]
                    }
                    _ => false,
                }
            }
            Kind::ExactMultiplyI64 { .. } | Kind::WrappingMultiplyI64 => {
                match alternative.variant {
                    0 => {
                        decoded
                            == [DecodedInstruction::Multiply {
                                source: registers[2],
                                destination: registers[2],
                            }]
                    }
                    1 => {
                        decoded
                            == [DecodedInstruction::Multiply {
                                source: registers[1],
                                destination: registers[2],
                            }]
                    }
                    2 => {
                        decoded
                            == [DecodedInstruction::Multiply {
                                source: registers[0],
                                destination: registers[2],
                            }]
                    }
                    3 => {
                        decoded
                            == [
                                mov(registers[0], registers[2]),
                                DecodedInstruction::Multiply {
                                    source: registers[1],
                                    destination: registers[2],
                                },
                            ]
                    }
                    _ => false,
                }
            }
            Kind::WrappingShiftLeftI64
            | Kind::WrappingShiftRightI64
            | Kind::WrappingShiftRightU64
            | exact_shift_i64_pair_pattern!()
            | Kind::ExactShiftRightU64 { .. } => {
                let shift = match kind {
                    Kind::WrappingShiftLeftI64 | Kind::ExactShiftLeftI64 { .. } => {
                        DecodedInstruction::ShiftLeftByCl {
                            destination: registers[2],
                        }
                    }
                    Kind::WrappingShiftRightU64 | Kind::ExactShiftRightU64 { .. } => {
                        DecodedInstruction::ShiftRightLogicalByCl {
                            destination: registers[2],
                        }
                    }
                    _ => DecodedInstruction::ShiftRightArithmeticByCl {
                        destination: registers[2],
                    },
                };
                registers[1] == 1 && decoded == [mov(registers[0], registers[2]), shift]
            }
            Kind::ReturnScalar | Kind::ReturnAggregate { .. } | Kind::ReturnUnit => {
                decoded == [DecodedInstruction::Return]
            }
            layout_dependent_selected_transfers!() | dedicated_selected_forms!() => false,
        };
        if valid {
            Ok(())
        } else {
            Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
        }
    }

    fn saturating_matches(
        operation: SaturatingOperation,
        carrier: SaturatingCarrier,
        registers: &[u8],
        decoded: &[DecodedInstruction],
    ) -> bool {
        let form = SaturatingForm::of(operation, carrier);
        form.accepts_registers(registers)
            && decoded == expected_saturating(operation, carrier, registers)
    }

    fn expected_saturating(
        operation: SaturatingOperation,
        carrier: SaturatingCarrier,
        registers: &[u8],
    ) -> Vec<DecodedInstruction> {
        let (left, right, result) = (registers[0], registers[1], registers[2]);
        let arithmetic = |source, destination| {
            if operation == SaturatingOperation::Add {
                DecodedInstruction::Add {
                    source,
                    destination,
                }
            } else {
                DecodedInstruction::Subtract {
                    source,
                    destination,
                }
            }
        };
        let clamp_with = |value, scratch, bound_bits, select: DecodedInstruction| {
            [
                DecodedInstruction::Materialize {
                    destination: scratch,
                    value: bound_bits,
                },
                DecodedInstruction::Compare {
                    left: value,
                    right: scratch,
                },
                select,
            ]
        };
        let clamp = |value, scratch, bound_bits, upper| {
            clamp_with(
                value,
                scratch,
                bound_bits,
                if upper {
                    DecodedInstruction::MoveOnGreater {
                        source: scratch,
                        destination: value,
                    }
                } else {
                    DecodedInstruction::MoveOnLess {
                        source: scratch,
                        destination: value,
                    }
                },
            )
        };
        match SaturatingForm::of(operation, carrier) {
            SaturatingForm::SubtractUnsigned => vec![
                DecodedInstruction::Compare { left, right },
                mov(left, result),
                DecodedInstruction::MoveOnBorrow {
                    source: right,
                    destination: result,
                },
                DecodedInstruction::Subtract {
                    source: right,
                    destination: result,
                },
            ],
            SaturatingForm::AddU64 => vec![
                mov(left, result),
                DecodedInstruction::Complement {
                    destination: result,
                },
                DecodedInstruction::Compare {
                    left: result,
                    right,
                },
                DecodedInstruction::MoveOnBorrow {
                    source: right,
                    destination: result,
                },
                DecodedInstruction::Subtract {
                    source: right,
                    destination: result,
                },
                DecodedInstruction::Complement {
                    destination: result,
                },
            ],
            form @ (SaturatingForm::ClampSignedNarrow | SaturatingForm::ClampUnsignedNarrow) => {
                let scratch = registers[3];
                let mut expected = vec![mov(left, result), arithmetic(right, result)];
                expected.extend(clamp(result, scratch, carrier.maximum_bits(), true));
                if form == SaturatingForm::ClampSignedNarrow {
                    expected.extend(clamp(result, scratch, carrier.minimum_bits(), false));
                }
                expected
            }
            SaturatingForm::OverflowSelectI64 => {
                let scratch = registers[3];
                vec![
                    mov(left, scratch),
                    DecodedInstruction::Complement {
                        destination: scratch,
                    },
                    DecodedInstruction::ArithmeticShiftRight63 {
                        destination: scratch,
                    },
                    DecodedInstruction::ComplementBit63 {
                        destination: scratch,
                    },
                    mov(left, result),
                    arithmetic(right, result),
                    DecodedInstruction::MoveOnOverflow {
                        source: scratch,
                        destination: result,
                    },
                ]
            }
            SaturatingForm::DivideUnsigned => {
                vec![DecodedInstruction::UnsignedDivide { divisor: right }]
            }
            SaturatingForm::DivideSignedNarrow => {
                let mut expected = vec![
                    DecodedInstruction::SignExtendDividend,
                    DecodedInstruction::SignedDivide { divisor: right },
                ];
                expected.extend(clamp(0, 2, carrier.maximum_bits(), true));
                expected
            }
            SaturatingForm::DivideI64 => vec![
                DecodedInstruction::CompareSignedImmediate8 {
                    register: right,
                    immediate: -1,
                },
                DecodedInstruction::SubtractWithBorrow {
                    source: 2,
                    destination: 2,
                },
                DecodedInstruction::Or {
                    source: 0,
                    destination: 2,
                },
                DecodedInstruction::Negate { destination: 2 },
                DecodedInstruction::Lea {
                    destination: 2,
                    base: 0,
                    index: None,
                    displacement: 1,
                },
                DecodedInstruction::MoveOnOverflow {
                    source: 2,
                    destination: 0,
                },
                DecodedInstruction::SignExtendDividend,
                DecodedInstruction::SignedDivide { divisor: right },
            ],
            SaturatingForm::RemainderUnsigned => vec![
                DecodedInstruction::Xor {
                    source: 2,
                    destination: 2,
                },
                DecodedInstruction::UnsignedDivide { divisor: right },
                mov(2, result),
            ],
            SaturatingForm::RemainderSigned => vec![
                DecodedInstruction::Xor {
                    source: 2,
                    destination: 2,
                },
                DecodedInstruction::CompareSignedImmediate8 {
                    register: right,
                    immediate: -1,
                },
                DecodedInstruction::JumpEqualShort { displacement: 5 },
                DecodedInstruction::SignExtendDividend,
                DecodedInstruction::SignedDivide { divisor: right },
                mov(2, result),
            ],
            form @ (SaturatingForm::MultiplySignedNarrow
            | SaturatingForm::MultiplyUnsignedNarrow) => {
                let scratch = registers[3];
                let mut expected = vec![
                    mov(left, result),
                    DecodedInstruction::Multiply {
                        source: right,
                        destination: result,
                    },
                ];
                if form == SaturatingForm::MultiplySignedNarrow {
                    expected.extend(clamp(result, scratch, carrier.maximum_bits(), true));
                    expected.extend(clamp(result, scratch, carrier.minimum_bits(), false));
                } else {
                    expected.extend(clamp_with(
                        result,
                        scratch,
                        carrier.maximum_bits(),
                        DecodedInstruction::MoveOnAbove {
                            source: scratch,
                            destination: result,
                        },
                    ));
                }
                expected
            }
            SaturatingForm::MultiplyI64 => {
                let scratch = registers[3];
                vec![
                    mov(left, scratch),
                    DecodedInstruction::Xor {
                        source: right,
                        destination: scratch,
                    },
                    DecodedInstruction::Complement {
                        destination: scratch,
                    },
                    DecodedInstruction::ArithmeticShiftRight63 {
                        destination: scratch,
                    },
                    DecodedInstruction::ComplementBit63 {
                        destination: scratch,
                    },
                    mov(left, result),
                    DecodedInstruction::Multiply {
                        source: right,
                        destination: result,
                    },
                    DecodedInstruction::MoveOnOverflow {
                        source: scratch,
                        destination: result,
                    },
                ]
            }
            SaturatingForm::MultiplyU64 => vec![
                DecodedInstruction::UnsignedMultiply { multiplier: right },
                DecodedInstruction::SubtractWithBorrow {
                    source: 2,
                    destination: 2,
                },
                DecodedInstruction::Or {
                    source: 2,
                    destination: 0,
                },
            ],
        }
    }

    fn expected_trapping(form: TrappingForm, registers: &[u8]) -> Vec<DecodedInstruction> {
        let trap_unless = |jump: DecodedInstruction| [jump, DecodedInstruction::Crash];
        let trap_unless_equal = trap_unless(DecodedInstruction::JumpEqualShort { displacement: 2 });
        let trap_on_overflow =
            trap_unless(DecodedInstruction::JumpNoOverflowShort { displacement: 2 });
        let range_check = |check: RangeCheck, value: u8, scratch: u8| {
            let mut expected = match check {
                RangeCheck::SignExtension { bits } => {
                    let (source, destination) = (value, scratch);
                    vec![
                        match bits {
                            8 => DecodedInstruction::SignExtendI8 {
                                source,
                                destination,
                            },
                            16 => DecodedInstruction::SignExtendI16 {
                                source,
                                destination,
                            },
                            _ => DecodedInstruction::SignExtendI32 {
                                source,
                                destination,
                            },
                        },
                        DecodedInstruction::Compare {
                            left: scratch,
                            right: value,
                        },
                    ]
                }
                RangeCheck::HighBits { shift } => vec![
                    mov(value, scratch),
                    DecodedInstruction::ShiftRightLogicalImmediate {
                        destination: scratch,
                        count: shift,
                    },
                ],
            };
            expected.extend(trap_unless_equal);
            expected
        };
        let arithmetic = |arithmetic: TrappingArithmetic| {
            let (left, right, result) = (registers[0], registers[1], registers[2]);
            vec![
                mov(left, result),
                match arithmetic {
                    TrappingArithmetic::Add => DecodedInstruction::Add {
                        source: right,
                        destination: result,
                    },
                    TrappingArithmetic::Subtract => DecodedInstruction::Subtract {
                        source: right,
                        destination: result,
                    },
                    TrappingArithmetic::Multiply => DecodedInstruction::Multiply {
                        source: right,
                        destination: result,
                    },
                },
            ]
        };
        let shift_prefix = || {
            let mut expected = vec![DecodedInstruction::CompareSignedImmediate8 {
                register: registers[1],
                immediate: form.carrier.bits() as i8,
            }];
            expected.extend(trap_unless(DecodedInstruction::JumpBelowShort {
                displacement: 2,
            }));
            expected.push(mov(registers[0], registers[2]));
            expected
        };
        let shift_right = |destination| {
            if form.carrier.is_signed() {
                DecodedInstruction::ShiftRightArithmeticByCl { destination }
            } else {
                DecodedInstruction::ShiftRightLogicalByCl { destination }
            }
        };
        match TrappingShape::of(form) {
            TrappingShape::OverflowFlag(operation) => {
                let mut expected = arithmetic(operation);
                expected.extend(trap_on_overflow);
                expected
            }
            TrappingShape::CarryFlag(operation) => {
                let mut expected = arithmetic(operation);
                expected.extend(trap_unless(DecodedInstruction::JumpAboveOrEqualShort {
                    displacement: 2,
                }));
                expected
            }
            TrappingShape::NarrowRange(operation, check) => {
                let mut expected = arithmetic(operation);
                expected.extend(range_check(check, registers[2], registers[3]));
                expected
            }
            TrappingShape::MultiplyHighHalf => {
                let mut expected = vec![DecodedInstruction::UnsignedMultiply {
                    multiplier: registers[1],
                }];
                expected.extend(trap_on_overflow);
                expected
            }
            TrappingShape::Division { guard, remainder } => {
                let divisor = registers[1];
                let mut expected = vec![DecodedInstruction::Test { register: divisor }];
                expected.extend(trap_unless(DecodedInstruction::JumpNotEqualShort {
                    displacement: 2,
                }));
                let minimum_trap = |compare, skip, jump| {
                    let mut guard = vec![
                        DecodedInstruction::CompareSignedImmediate8 {
                            register: divisor,
                            immediate: -1,
                        },
                        DecodedInstruction::JumpNotEqualShort { displacement: skip },
                        compare,
                    ];
                    guard.extend(trap_unless(jump));
                    guard.extend([
                        DecodedInstruction::SignExtendDividend,
                        DecodedInstruction::SignedDivide { divisor },
                    ]);
                    guard
                };
                expected.extend(match guard {
                    None => vec![
                        DecodedInstruction::Xor {
                            source: 2,
                            destination: 2,
                        },
                        DecodedInstruction::UnsignedDivide { divisor },
                    ],
                    Some(DividendGuard::Minimum8(minimum)) => minimum_trap(
                        DecodedInstruction::CompareSignedImmediate8 {
                            register: 0,
                            immediate: minimum,
                        },
                        8,
                        DecodedInstruction::JumpNotEqualShort { displacement: 2 },
                    ),
                    Some(DividendGuard::Minimum32(minimum)) => minimum_trap(
                        DecodedInstruction::CompareRaxImmediate32 { immediate: minimum },
                        10,
                        DecodedInstruction::JumpNotEqualShort { displacement: 2 },
                    ),
                    Some(DividendGuard::DecrementOverflows) => minimum_trap(
                        DecodedInstruction::CompareSignedImmediate8 {
                            register: 0,
                            immediate: 1,
                        },
                        8,
                        DecodedInstruction::JumpNoOverflowShort { displacement: 2 },
                    ),
                });
                if remainder {
                    expected.push(mov(2, 0));
                }
                expected
            }
            TrappingShape::ShiftLeftRoundTrip { .. } => {
                let (value, result, scratch) = (registers[0], registers[2], registers[3]);
                let mut expected = shift_prefix();
                expected.extend([
                    DecodedInstruction::ShiftLeftByCl {
                        destination: result,
                    },
                    mov(result, scratch),
                    shift_right(scratch),
                    DecodedInstruction::Compare {
                        left: scratch,
                        right: value,
                    },
                ]);
                expected.extend(trap_unless_equal);
                expected
            }
            TrappingShape::ShiftLeftRange { check, .. } => {
                let mut expected = shift_prefix();
                expected.push(DecodedInstruction::ShiftLeftByCl {
                    destination: registers[2],
                });
                expected.extend(range_check(check, registers[2], registers[3]));
                expected
            }
            TrappingShape::ShiftRight { .. } => {
                let mut expected = shift_prefix();
                expected.push(shift_right(registers[2]));
                expected
            }
            TrappingShape::Convert(check) => {
                let (operand, result, scratch) = (registers[0], registers[1], registers[2]);
                let mut expected = check
                    .map(|check| range_check(check, operand, scratch))
                    .unwrap_or_default();
                expected.push(mov(operand, result));
                expected
            }
        }
    }

    fn operand_views(positions: &[u16], operands: &[RegisterViewId]) -> Vec<RegisterViewId> {
        positions
            .iter()
            .map(|&position| operands[usize::from(position)])
            .collect()
    }

    fn trapping_footprint(
        form: TrappingForm,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        let shape = TrappingShape::of(form);
        let (reads, writes) = shape.operand_reads_and_writes();
        X86_64SelectedFormFootprint {
            register_reads: operand_views(&reads, operands),
            register_writes: operand_views(&writes, operands),
            writes_rflags: true,
            encoded: shape.encoded_effects(),
        }
    }

    fn saturating_footprint(
        form: SaturatingForm,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        let physical = crate::x86_64_physical_register_model_ref();
        let units = |name: &str| physical.view_named(name).unwrap().units.clone();
        let (reads, writes) = form.operand_reads_and_writes();
        let register_reads = operand_views(&reads, operands);
        let register_writes = operand_views(&writes, operands);
        let mut encoded = MachineEncodedEffects::fallthrough_v1(reads, writes);
        encoded.implicit_unit_clobbers = units("condition_flags");
        if form.is_division() {
            if !matches!(
                form,
                SaturatingForm::RemainderUnsigned | SaturatingForm::RemainderSigned
            ) {
                encoded.implicit_unit_clobbers.extend(units("rdx"));
                encoded.implicit_unit_clobbers.sort_unstable();
                encoded.implicit_unit_clobbers.dedup();
            }
            encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        }
        X86_64SelectedFormFootprint {
            register_reads,
            register_writes,
            writes_rflags: true,
            encoded,
        }
    }

    pub(crate) fn footprint(
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        if let Some((operation, carrier)) = saturating_operation(kind) {
            return saturating_footprint(SaturatingForm::of(operation, carrier), operands);
        }
        if let Some(form) = trapping_form(kind) {
            return trapping_footprint(form, operands);
        }
        if let Kind::NearestIeeeFloatArithmetic { form } = kind {
            return super::ieee_float_arithmetic::footprint(form, operands);
        }
        if let Kind::AtomicAccess { form } = kind {
            return super::atomic::footprint(form, operands);
        }
        let (reads, writes, writes_rflags): (Vec<u16>, Vec<u16>, bool) = match kind {
            Kind::InstalledInterruptCpuCheck { .. }
            | Kind::InstalledInterruptTaskCommit { .. }
            | Kind::InstalledInterruptTablePublication { .. } => {
                return super::installed_interrupt::footprint(
                    super::installed_interrupt::Form::from_kind(kind).expect("installed form"),
                    operands,
                );
            }
            Kind::RegisterAccess { operation } => {
                return super::register_access::footprint(operation, operands);
            }
            Kind::AccessorScalarAccess { form, .. } => {
                if form.write {
                    (vec![0, 1], vec![], false)
                } else {
                    (vec![0], vec![1], false)
                }
            }
            Kind::FloatResultPolicy { form } => {
                return super::float_result_policy::footprint(form, operands);
            }
            Kind::NearestIeeeFloatArithmetic { .. } => {
                unreachable!("float arithmetic footprint returned above")
            }
            Kind::AtomicAccess { .. } => {
                unreachable!("atomic footprint returned above")
            }
            Kind::PortWrite { .. } | Kind::AtomicFence | Kind::Crash => {
                (Vec::new(), Vec::new(), false)
            }
            Kind::PrivilegedCrash => (Vec::new(), Vec::new(), true),
            Kind::MaterializeBooleanEqual
            | Kind::MaterializeBooleanU64LessThan
            | Kind::MaterializeBooleanI64LessThan
            | Kind::MaterializeBooleanU64LessOrEqual
            | Kind::MaterializeBooleanI64LessOrEqual => (vec![], vec![0], false),
            Kind::MaterializeI64 { .. } => (vec![], vec![0], alternative.variant == 1),
            Kind::ImageAddress { .. } => (vec![], vec![0], false),
            Kind::CopyI64
            | Kind::ZeroExtendU8
            | Kind::ZeroExtendU16
            | Kind::SignExtendI8
            | Kind::SignExtendI16
            | Kind::SignExtendI32
            | Kind::ZeroExtendU32 => (vec![0], vec![1], false),
            Kind::CompareI64Zero | Kind::CompareI64Immediate { .. } => (vec![0], vec![], true),
            Kind::ExactDivideU64 { .. } => (vec![0, 1, 3], vec![2], true),
            exact_wrapping_divrem_set_pattern!(..) => (vec![0, 1], vec![2, 3], true),
            Kind::CompareI64 => (vec![0, 1], vec![], true),
            Kind::ByteViewAddress | Kind::WrappingAddI64 | Kind::ExactAddI64 { .. } => {
                (vec![0, 1], vec![2], false)
            }
            Kind::BitwiseNotI64 => (vec![0], vec![1], false),
            exact_add_or_sub_pair_pattern!(..) => (vec![0], vec![1], false),
            Kind::ExactSubtractI64 { .. } | Kind::WrappingSubtractI64
                if alternative.variant == 0 =>
            {
                (vec![], vec![2], true)
            }
            Kind::ExactSubtractI64 { .. } | Kind::WrappingSubtractI64 => {
                (vec![0, 1], vec![2], true)
            }
            Kind::ExactMultiplyI64 { .. } | Kind::WrappingMultiplyI64 => {
                (vec![0, 1], vec![2], true)
            }
            Kind::WrappingShiftLeftI64
            | Kind::WrappingShiftRightI64
            | Kind::WrappingShiftRightU64
            | exact_shift_i64_pair_pattern!()
            | Kind::ExactShiftRightU64 { .. } => (vec![0, 1], vec![2], true),
            Kind::BitwiseAndI64 | Kind::BitwiseOrI64 | Kind::BitwiseXorI64 => {
                (vec![0, 1], vec![2], true)
            }
            Kind::ReturnScalar | Kind::ReturnAggregate { .. } | Kind::ReturnUnit => {
                (vec![], vec![], false)
            }
            layout_dependent_selected_transfers!() | dedicated_selected_forms!() => {
                (vec![], vec![], false)
            }
            saturating_add_or_subtract_pattern!()
            | Kind::SaturatingDivide { .. }
            | Kind::SaturatingRemainder { .. }
            | Kind::SaturatingMultiply { .. } => {
                unreachable!("saturating forms handled above")
            }
            Kind::TrappingInteger { .. } => {
                unreachable!("trapping forms handled above")
            }
        };
        let register_reads = operand_views(&reads, operands);
        let register_writes = operand_views(&writes, operands);
        let physical = crate::x86_64_physical_register_model_ref();
        let units = |name: &str| physical.view_named(name).unwrap().units.clone();
        let encoded = if matches!(kind, Kind::PortWrite { .. }) {
            super::port_write::effects()
        } else if kind == Kind::AtomicFence {
            MachineEncodedEffects::atomic_fence_v1()
        } else if kind == Kind::PrivilegedCrash {
            MachineEncodedEffects::privileged_stop_v1(&units("rip"), &units("control_flags"))
        } else if kind == Kind::Crash {
            selected_instructions::MachineEncodedEffects::crash_v1(&units("rip"))
        } else if let Kind::AtomicAccess { .. } = kind {
            unreachable!("atomic footprint returned above");
        } else if matches!(
            kind,
            Kind::ReturnScalar | Kind::ReturnAggregate { .. } | Kind::ReturnUnit
        ) {
            let stack_pointer = physical.view_named("rsp").unwrap().id;
            let defs = units_of(&["rsp", "rip"]);
            MachineEncodedEffects {
                external_operand_reads: vec![],
                external_operand_writes: vec![],
                implicit_unit_uses: units("rsp"),
                implicit_unit_defs: defs,
                implicit_unit_clobbers: vec![],
                memory: MachineEncodedMemoryEffect::ReadActivationStackV1 {
                    stack_pointer,
                    byte_count: 8,
                },
                stack: MachineEncodedStackEffect::PopBytesV1 {
                    stack_pointer,
                    byte_count: 8,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::ReturnFromActivationStackV1,
            }
        } else if matches!(
            kind,
            Kind::ConditionalBranchNonZero
                | Kind::ConditionalBranchU64LessThan
                | Kind::ConditionalBranchI64LessThan
        ) {
            let uses = units_of(&["condition_flags", "rip"]);
            MachineEncodedEffects {
                external_operand_reads: vec![],
                external_operand_writes: vec![],
                implicit_unit_uses: uses,
                implicit_unit_defs: units("rip"),
                implicit_unit_clobbers: vec![],
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            }
        } else {
            let mut effects = MachineEncodedEffects::fallthrough_v1(reads, writes);
            if matches!(
                kind,
                Kind::CompareI64Zero | Kind::CompareI64 | Kind::CompareI64Immediate { .. }
            ) {
                effects.implicit_unit_defs = units("condition_flags");
            }
            if matches!(
                kind,
                Kind::ExactSubtractI64 { .. }
                    | Kind::WrappingSubtractI64
                    | Kind::ExactMultiplyI64 { .. }
                    | Kind::WrappingMultiplyI64
                    | Kind::WrappingShiftLeftI64
                    | Kind::WrappingShiftRightI64
                    | Kind::WrappingShiftRightU64
                    | exact_shift_i64_pair_pattern!()
                    | Kind::ExactShiftRightU64 { .. }
                    | Kind::BitwiseAndI64
                    | Kind::BitwiseOrI64
                    | Kind::BitwiseXorI64
            ) {
                effects.implicit_unit_clobbers = units("condition_flags");
            }
            if matches!(
                kind,
                Kind::MaterializeBooleanEqual
                    | Kind::MaterializeBooleanU64LessThan
                    | Kind::MaterializeBooleanI64LessThan
                    | Kind::MaterializeBooleanU64LessOrEqual
                    | Kind::MaterializeBooleanI64LessOrEqual
            ) {
                effects.implicit_unit_uses = units("condition_flags");
            }
            if matches!(kind, Kind::ExactDivideU64 { .. }) {
                effects.implicit_unit_clobbers = units("rdx");
                effects
                    .implicit_unit_clobbers
                    .extend(units("condition_flags"));
                effects.implicit_unit_clobbers.sort_unstable();
                effects.implicit_unit_clobbers.dedup();
                effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            }
            if matches!(kind, exact_wrapping_divrem_set_pattern!(..)) {
                effects.implicit_unit_clobbers = units("condition_flags");
                effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            }
            if writes_rflags && matches!(kind, Kind::MaterializeI64 { .. }) {
                effects.implicit_unit_clobbers = units("condition_flags");
            }
            effects
        };
        X86_64SelectedFormFootprint {
            register_reads,
            register_writes,
            writes_rflags,
            encoded,
        }
    }
}
mod encoded_scalar {
    use super::instruction_bytes::{modrm, rex};
    use super::request_validation::resolve_registers;
    use super::{
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
        X86_64SelectedFormFootprint,
    };
    use encoded_scalar_plan::{Step, plan};
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, SelectedInstructionKind as Kind,
    };

    fn codec_plan(load: bool, registers: [u8; 6]) -> Vec<Step> {
        let [_, _, cursor, _, _, _] = registers;
        plan(load, registers, Step::Zero(cursor))
    }

    fn instruction_size(instruction: Step) -> usize {
        match instruction {
            Step::Zero(_) | Step::Copy(..) | Step::Or(..) | Step::Compare(..) => 3,
            Step::ByteCount(..)
            | Step::SubOne(_)
            | Step::AddOne(_)
            | Step::SignByte(_)
            | Step::ShiftLeft8(_)
            | Step::ShiftRight8(_) => 4,
            Step::LoadByte(..) | Step::ZeroBranch(..) => 9,
            Step::StoreByte(..) => 8,
            Step::TestSigned(..) => 13,
            Step::LessBranch(_) => 6,
            Step::Branch(_) => 5,
        }
    }

    fn positions(plan: &[Step]) -> Vec<usize> {
        let mut offset = 0;
        plan.iter()
            .map(|instruction| {
                let position = offset;
                offset += instruction_size(*instruction);
                position
            })
            .collect()
    }

    fn branch_target(offsets: &[usize], target: usize) -> Result<usize, Error> {
        offsets.get(target).copied().ok_or(Error::MalformedEncoding)
    }

    fn assemble(plan: &[Step]) -> Result<Vec<u8>, Error> {
        let offsets = positions(plan);
        let mut bytes = Vec::new();
        for instruction in plan {
            match *instruction {
                Step::Zero(register) => bytes.extend([
                    rex(register, 0, register),
                    0x31,
                    modrm(3, register, register),
                ]),
                Step::Copy(destination, source) => bytes.extend([
                    rex(source, 0, destination),
                    0x89,
                    modrm(3, source, destination),
                ]),
                Step::ByteCount(destination, source) => bytes.extend([
                    rex(destination, 0, source),
                    0x0f,
                    0xb6,
                    modrm(3, destination, source),
                ]),
                Step::SubOne(register) => {
                    bytes.extend([rex(0, 0, register), 0x83, modrm(3, 5, register), 1])
                }
                Step::AddOne(register) => {
                    bytes.extend([rex(0, 0, register), 0x83, modrm(3, 0, register), 1])
                }
                Step::LoadByte(destination, base, index) => {
                    bytes.extend([
                        rex(destination, index, base),
                        0x0f,
                        0xb6,
                        modrm(2, destination, 4),
                        ((index & 7) << 3) | (base & 7),
                    ]);
                    bytes.extend(0u32.to_le_bytes());
                }
                Step::StoreByte(source, base, index) => {
                    bytes.extend([
                        rex(source, index, base) & !8,
                        0x88,
                        modrm(2, source, 4),
                        ((index & 7) << 3) | (base & 7),
                    ]);
                    bytes.extend(0u32.to_le_bytes());
                }
                Step::SignByte(register) => bytes.extend([
                    rex(register, 0, register),
                    0x0f,
                    0xbe,
                    modrm(3, register, register),
                ]),
                Step::ShiftLeft8(register) => {
                    bytes.extend([rex(0, 0, register), 0xc1, modrm(3, 4, register), 8])
                }
                Step::ShiftRight8(register) => {
                    bytes.extend([rex(0, 0, register), 0xc1, modrm(3, 5, register), 8])
                }
                Step::Or(destination, source) => bytes.extend([
                    rex(source, 0, destination),
                    0x09,
                    modrm(3, source, destination),
                ]),
                Step::Compare(left, right) => {
                    bytes.extend([rex(right, 0, left), 0x39, modrm(3, right, left)])
                }
                Step::TestSigned(register, target) => {
                    bytes.extend([rex(0, 0, register), 0xf7, modrm(3, 0, register)]);
                    bytes.extend(0x100u32.to_le_bytes());
                    append_jump(&mut bytes, &[0x0f, 0x84], branch_target(&offsets, target)?)?;
                }
                Step::ZeroBranch(register, target) => {
                    bytes.extend([
                        rex(register, 0, register),
                        0x85,
                        modrm(3, register, register),
                    ]);
                    append_jump(&mut bytes, &[0x0f, 0x84], branch_target(&offsets, target)?)?;
                }
                Step::LessBranch(target) => {
                    append_jump(&mut bytes, &[0x0f, 0x82], branch_target(&offsets, target)?)?
                }
                Step::Branch(target) => {
                    append_jump(&mut bytes, &[0xe9], branch_target(&offsets, target)?)?
                }
            }
        }
        Ok(bytes)
    }

    fn append_jump(bytes: &mut Vec<u8>, prefix: &[u8], target: usize) -> Result<(), Error> {
        bytes.extend(prefix);
        let displacement = i32::try_from(target as i64 - (bytes.len() as i64 + 4))
            .map_err(|_| Error::MalformedEncoding)?;
        bytes.extend(displacement.to_le_bytes());
        Ok(())
    }

    fn decode_one(bytes: &[u8], position: usize) -> Result<(Step, usize), Error> {
        let instruction = bytes.get(position..).ok_or(Error::MalformedEncoding)?;
        let fail = Error::EncodedFormMismatch;
        let fields = |prefix: u8, mode: u8| {
            (
                ((mode >> 3) & 7) | ((prefix & 4) << 1),
                (mode & 7) | ((prefix & 1) << 3),
            )
        };
        let target = |offset: usize, width: usize| -> Result<usize, Error> {
            let displacement: [u8; 4] = instruction
                .get(offset..offset + 4)
                .ok_or(Error::MalformedEncoding)?
                .try_into()
                .map_err(|_| Error::MalformedEncoding)?;
            usize::try_from(
                position as i64 + width as i64 + i64::from(i32::from_le_bytes(displacement)),
            )
            .map_err(|_| Error::MalformedEncoding)
        };
        match *instruction {
            [prefix, 0x31, mode, ..] if prefix & 0xfa == 0x48 && mode >> 6 == 3 => {
                let (source, destination) = fields(prefix, mode);
                if source == destination {
                    Ok((Step::Zero(source), 3))
                } else {
                    Err(fail)
                }
            }
            [prefix, opcode @ (0x89 | 0x09 | 0x39), mode, ..]
                if prefix & 0xfa == 0x48 && mode >> 6 == 3 =>
            {
                let (source, destination) = fields(prefix, mode);
                Ok((
                    match opcode {
                        0x89 => Step::Copy(destination, source),
                        0x09 => Step::Or(destination, source),
                        _ => Step::Compare(destination, source),
                    },
                    3,
                ))
            }
            [prefix, 0x0f, opcode @ (0xb6 | 0xbe), mode, ..]
                if prefix & 0xfa == 0x48 && mode >> 6 == 3 =>
            {
                let (destination, source) = fields(prefix, mode);
                if opcode == 0xb6 {
                    Ok((Step::ByteCount(destination, source), 4))
                } else if source == destination {
                    Ok((Step::SignByte(source), 4))
                } else {
                    Err(fail)
                }
            }
            [prefix, 0x83, mode, 1, ..] if prefix & 0xfe == 0x48 && mode >> 6 == 3 => {
                let register = (mode & 7) | ((prefix & 1) << 3);
                match (mode >> 3) & 7 {
                    0 => Ok((Step::AddOne(register), 4)),
                    5 => Ok((Step::SubOne(register), 4)),
                    _ => Err(fail),
                }
            }
            [prefix, 0xc1, mode, 8, ..] if prefix & 0xfe == 0x48 && mode >> 6 == 3 => {
                let register = (mode & 7) | ((prefix & 1) << 3);
                match (mode >> 3) & 7 {
                    4 => Ok((Step::ShiftLeft8(register), 4)),
                    5 => Ok((Step::ShiftRight8(register), 4)),
                    _ => Err(fail),
                }
            }
            [prefix, 0x0f, 0xb6, mode, sib, 0, 0, 0, 0, ..]
                if prefix & 0xf8 == 0x48 && mode & 0xc7 == 0x84 && sib >> 6 == 0 =>
            {
                let destination = ((mode >> 3) & 7) | ((prefix & 4) << 1);
                let base = (sib & 7) | ((prefix & 1) << 3);
                let index = ((sib >> 3) & 7) | ((prefix & 2) << 2);
                if index == 4 {
                    Err(fail)
                } else {
                    Ok((Step::LoadByte(destination, base, index), 9))
                }
            }
            [prefix, 0x88, mode, sib, 0, 0, 0, 0, ..]
                if prefix & 0xf8 == 0x40 && mode & 0xc7 == 0x84 && sib >> 6 == 0 =>
            {
                let source = ((mode >> 3) & 7) | ((prefix & 4) << 1);
                let base = (sib & 7) | ((prefix & 1) << 3);
                let index = ((sib >> 3) & 7) | ((prefix & 2) << 2);
                if index == 4 {
                    Err(fail)
                } else {
                    Ok((Step::StoreByte(source, base, index), 8))
                }
            }
            [prefix, 0xf7, mode, 0, 1, 0, 0, 0x0f, 0x84, _, _, _, _, ..]
                if prefix & 0xfe == 0x48 && mode & 0xf8 == 0xc0 =>
            {
                Ok((
                    Step::TestSigned((mode & 7) | ((prefix & 1) << 3), target(9, 13)?),
                    13,
                ))
            }
            [prefix, 0x85, mode, 0x0f, 0x84, _, _, _, _, ..]
                if prefix & 0xfa == 0x48 && mode >> 6 == 3 =>
            {
                let (source, destination) = fields(prefix, mode);
                if source == destination {
                    Ok((Step::ZeroBranch(source, target(5, 9)?), 9))
                } else {
                    Err(fail)
                }
            }
            [0x0f, 0x82, _, _, _, _, ..] => Ok((Step::LessBranch(target(2, 6)?), 6)),
            [0xe9, _, _, _, _, ..] => Ok((Step::Branch(target(1, 5)?), 5)),
            _ => Err(fail),
        }
    }

    fn resolved_plan(plan: &[Step]) -> Result<Vec<Step>, Error> {
        let offsets = positions(plan);
        plan.iter()
            .map(|instruction| {
                Ok(match *instruction {
                    Step::TestSigned(register, target) => {
                        Step::TestSigned(register, branch_target(&offsets, target)?)
                    }
                    Step::ZeroBranch(register, target) => {
                        Step::ZeroBranch(register, branch_target(&offsets, target)?)
                    }
                    Step::LessBranch(target) => Step::LessBranch(branch_target(&offsets, target)?),
                    Step::Branch(target) => Step::Branch(branch_target(&offsets, target)?),
                    other => other,
                })
            })
            .collect()
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<(bool, [u8; 6]), Error> {
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        let load = kind == Kind::LoadEncodedScalar;
        let family = if load {
            Family::LoadEncodedScalar
        } else {
            Family::StoreEncodedScalar
        };
        if !matches!(kind, Kind::LoadEncodedScalar | Kind::StoreEncodedScalar)
            || alternative != (MachineAlternativeKey { family, variant: 0 })
        {
            return Err(Error::EncodedFormMismatch);
        }
        let registers: [u8; 6] = resolve_registers(physical, operands)?
            .try_into()
            .map_err(|_| Error::OperandCountMismatch)?;
        let inputs = if load { 2 } else { 3 };
        for scratch in inputs..6 {
            if registers[..scratch].contains(&registers[scratch]) {
                return Err(Error::EncodedFormMismatch);
            }
        }
        Ok((load, registers))
    }

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (load, registers) = request(physical, kind, alternative, operands)?;
        let bytes = assemble(&codec_plan(load, registers))?;
        validate(physical, kind, alternative, operands, &bytes)
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (load, registers) = request(physical, kind, alternative, operands)?;
        let mut decoded = Vec::new();
        let mut position = 0;
        while position < bytes.len() {
            let (instruction, size) = decode_one(bytes, position)?;
            decoded.push(instruction);
            position += size;
        }
        if decoded != resolved_plan(&codec_plan(load, registers))? {
            return Err(Error::EncodedFormMismatch);
        }
        let inputs = if load { 2 } else { 3 };
        let flags = physical
            .model()
            .view_named("condition_flags")
            .ok_or(Error::NonCanonicalPhysicalModel)?
            .units
            .clone();
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: operands[..inputs].to_vec(),
                register_writes: operands[inputs..].to_vec(),
                writes_rflags: true,
                encoded: MachineEncodedEffects {
                    external_operand_reads: (0..inputs as u16).collect(),
                    external_operand_writes: (inputs as u16..6).collect(),
                    implicit_unit_uses: vec![],
                    implicit_unit_defs: vec![],
                    implicit_unit_clobbers: flags,
                    memory: if load {
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
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::FallThroughV1,
                },
            },
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use encoded_scalar_plan::{harness, replay};

        fn form() -> (ValidatedPhysicalRegisterModel, [RegisterViewId; 6]) {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let operands = ["r8", "r9", "r10", "r11", "r12", "r13"]
                .map(|name| physical.model().view_named(name).unwrap().id);
            (physical, operands)
        }

        fn execute(bytes: &[u8], state: &mut [u64; 32], memory: &mut [u8]) -> Vec<usize> {
            replay::execute(
                bytes,
                bytes.len(),
                |bytes, position| decode_one(bytes, position).unwrap(),
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
mod float_bits {
    use super::{
        MachineAlternativeKey, MachineEncodedEffects, RegisterViewId,
        SelectedInstructionKind as Kind, ValidatedPhysicalRegisterModel,
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
        X86_64SelectedFormFootprint,
    };
    use crate::selected_form_encoding::request_validation::resolve_registers;
    use selected_instructions::float_bit_transfer_properties;
    type Error = X86_64SelectedFormEncodingError;
    type Encoding = ValidatedX86_64SelectedFormEncoding;

    fn operands(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        values: &[RegisterViewId],
    ) -> Result<(u8, u8, bool, bool), Error> {
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
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
            .strip_prefix("xmm")
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .filter(|number| *number <= 15)
            .ok_or(Error::UnknownOrNonGpr64View(float))?;
        if float_view.bits != 128 || !float_view.allocatable {
            return Err(Error::UnknownOrNonGpr64View(float));
        }
        let bits_register = resolve_registers(physical, &[bits])?[0];
        Ok((float_register, bits_register, to_bits, wide))
    }
    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        values: &[RegisterViewId],
    ) -> Result<Encoding, Error> {
        let (float, bits, to_bits, wide) = operands(physical, kind, alternative, values)?;
        let mut bytes = vec![0x66];
        let rex = 0x40 | (u8::from(wide) << 3) | ((float >> 3) << 2) | (bits >> 3);
        if rex != 0x40 {
            bytes.push(rex);
        }
        bytes.extend([
            0x0f,
            if to_bits { 0x7e } else { 0x6e },
            0xc0 | ((float & 7) << 3) | (bits & 7),
        ]);
        validate(physical, kind, alternative, values, &bytes)
    }
    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        values: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<Encoding, Error> {
        let (float, bits, to_bits, wide) = operands(physical, kind, alternative, values)?;
        let [0x66, tail @ ..] = bytes else {
            return Err(Error::MalformedEncoding);
        };
        let (rex, tail) = match tail {
            [rex @ 0x41..=0x4f, rest @ ..] => (*rex, rest),
            _ => (0x40, tail),
        };
        let [0x0f, opcode, modrm] = tail else {
            return Err(Error::MalformedEncoding);
        };
        if modrm & 0xc0 != 0xc0 || rex & 2 != 0 {
            return Err(Error::MalformedEncoding);
        }
        let decoded_to_bits = match opcode {
            0x7e => true,
            0x6e => false,
            _ => return Err(Error::MalformedEncoding),
        };
        let decoded_float = ((modrm >> 3) & 7) | ((rex & 4) << 1);
        let decoded_bits = (modrm & 7) | ((rex & 1) << 3);
        if (decoded_float, decoded_bits, decoded_to_bits, rex & 8 != 0)
            != (float, bits, to_bits, wide)
        {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(Encoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: vec![values[0]],
                register_writes: vec![values[1]],
                writes_rflags: false,
                encoded: MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]),
            },
        })
    }
    #[cfg(test)]
    mod tests {
        use super::{
            Kind, MachineAlternativeKey, MachineEncodedEffects, encode,
            float_bit_transfer_properties, validate,
        };
        use selected_instructions::MachineAlternativeFamily as Family;
        #[test]
        fn raw_transfers_decode_exact_width_direction_and_registers() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            for (kind, family) in [
                (Kind::Float32ToBits, Family::Float32ToBits),
                (Kind::Float64ToBits, Family::Float64ToBits),
                (Kind::BitsToFloat32, Family::BitsToFloat32),
                (Kind::BitsToFloat64, Family::BitsToFloat64),
            ] {
                for float_name in ["xmm0", "xmm15"] {
                    for bits_name in ["rax", "r15"] {
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
    use super::{
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
        X86_64SelectedFormFootprint,
    };
    use crate::register_model::units_of;
    use register_model::{
        RegisterConstraintKey, RegisterUnitId, RegisterViewId, ValidatedPhysicalRegisterModel,
    };
    use selected_instructions::{
        FloatResultPolicyForm, MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier,
        MachineEffectDeclaration, MachineEncodedEffects, MachineMemoryEffect, MachineSemanticKind,
        MachineSizeKnowledge, MachineTrapBehavior, SelectedInstructionKind,
    };

    #[cfg(test)]
    use selected_instructions::{FloatResultPolicyKind, MachineEncodedControlEffect};

    pub(crate) fn clobbers(form: FloatResultPolicyForm) -> Vec<RegisterUnitId> {
        units_of(if form.can_trap() {
            &["xmm3", "condition_flags"]
        } else {
            &["xmm3", "xmm4", "condition_flags"]
        })
    }

    pub(crate) fn effects(form: FloatResultPolicyForm) -> MachineEncodedEffects {
        MachineEncodedEffects::float_result_policy_v1(
            form.operand_count() as u16,
            clobbers(form),
            form.can_trap(),
        )
    }

    pub(crate) fn footprint(
        form: FloatResultPolicyForm,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        let inputs = form.operand_count();
        X86_64SelectedFormFootprint {
            register_reads: operands[..inputs].to_vec(),
            register_writes: operands[inputs..inputs + 1].to_vec(),
            writes_rflags: true,
            encoded: effects(form),
        }
    }

    pub(crate) fn size(form: FloatResultPolicyForm) -> MachineSizeKnowledge {
        MachineSizeKnowledge::ExactBytes(
            program(form, &[0; 4])
                .iter()
                .map(|instruction| instruction.size())
                .sum::<usize>() as u16,
        )
    }

    pub(crate) fn declaration(
        form: FloatResultPolicyForm,
        constraint: RegisterConstraintKey,
    ) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            MachineSemanticKind::FloatResultPolicy(form),
            constraint,
            MachineMemoryEffect::NoneV1,
            if form.can_trap() {
                MachineTrapBehavior::TrappingIntegerV1
            } else {
                MachineTrapBehavior::NeverV1
            },
            if form.can_trap() {
                MachineBarrier::ExternalEffect
            } else {
                MachineBarrier::None
            },
            MachineAlternativeFamily::FloatResultPolicy(form),
            size(form),
            effects(form),
        )
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Instruction {
        Transfer {
            save: bool,
            wide: bool,
            vector: u8,
            general: u8,
        },
        Shift {
            left: bool,
            wide: bool,
            general: u8,
            count: u8,
        },
        Compare {
            wide: bool,
            general: u8,
            immediate: u32,
        },
        Conditional {
            equal: bool,
            displacement: i32,
        },
        Jump {
            displacement: i32,
        },
        SubtractOne {
            wide: bool,
            general: u8,
        },
        Crash,
    }

    impl Instruction {
        fn size(self) -> usize {
            match self {
                Self::Transfer { .. } | Self::Jump { .. } => 5,
                Self::Shift { .. } | Self::SubtractOne { .. } => 4,
                Self::Compare { .. } => 7,
                Self::Conditional { .. } => 6,
                Self::Crash => 2,
            }
        }
    }

    fn program(form: FloatResultPolicyForm, registers: &[u8]) -> Vec<Instruction> {
        let wide = !form.is_binary32();
        let fraction = if wide { 52 } else { 23 };
        let exponent_mask = if wide { 2047 } else { 255 };
        let inputs = form.operand_count();
        let general = registers[inputs];
        let restore = |vector| Instruction::Transfer {
            save: false,
            wide,
            vector,
            general,
        };
        let shift = |left, count| Instruction::Shift {
            left,
            wide,
            general,
            count,
        };
        let compare = |immediate| Instruction::Compare {
            wide,
            general,
            immediate,
        };
        let mut instructions = registers[..inputs]
            .iter()
            .enumerate()
            .map(|(position, &source)| Instruction::Transfer {
                save: true,
                wide,
                vector: 3 + position as u8,
                general: source,
            })
            .collect::<Vec<_>>();
        if form.can_trap() {
            instructions.extend([
                restore(3),
                shift(true, 1),
                shift(false, fraction + 1),
                compare(exponent_mask),
                Instruction::Conditional {
                    equal: false,
                    displacement: 2,
                },
                Instruction::Crash,
                restore(3),
            ]);
            return instructions;
        }
        let mut retain_raw_branches = Vec::new();
        instructions.extend([
            restore(3),
            shift(true, if wide { 56 } else { 24 }),
            shift(false, if wide { 56 } else { 24 }),
            compare(0),
        ]);
        retain_raw_branches.push(instructions.len());
        instructions.push(Instruction::Conditional {
            equal: true,
            displacement: 0,
        });
        instructions.extend([
            restore(4),
            shift(true, 1),
            shift(false, fraction + 1),
            compare(exponent_mask),
        ]);
        retain_raw_branches.push(instructions.len());
        instructions.push(Instruction::Conditional {
            equal: false,
            displacement: 0,
        });
        instructions.extend([
            restore(4),
            shift(true, if wide { 12 } else { 9 }),
            compare(0),
        ]);
        retain_raw_branches.push(instructions.len());
        instructions.push(Instruction::Conditional {
            equal: false,
            displacement: 0,
        });
        instructions.extend([
            restore(4),
            Instruction::SubtractOne { wide, general },
            Instruction::Jump { displacement: 5 },
        ]);
        let retain_raw_offset = instructions
            .iter()
            .map(|instruction| instruction.size())
            .sum::<usize>();
        instructions.push(restore(4));
        for position in retain_raw_branches {
            let next_offset = instructions[..=position]
                .iter()
                .map(|instruction| instruction.size())
                .sum::<usize>();
            if let Instruction::Conditional { displacement, .. } = &mut instructions[position] {
                *displacement = (retain_raw_offset - next_offset) as i32;
            }
        }
        instructions
    }

    fn encode_instruction(instruction: Instruction, bytes: &mut Vec<u8>) {
        let rex = |wide, general: u8| 0x40 | if wide { 8 } else { 0 } | (general >> 3);
        match instruction {
            Instruction::Transfer {
                save,
                wide,
                vector,
                general,
            } => bytes.extend([
                0x66,
                rex(wide, general),
                0x0f,
                if save { 0x6e } else { 0x7e },
                0xc0 | vector << 3 | general & 7,
            ]),
            Instruction::Shift {
                left,
                wide,
                general,
                count,
            } => bytes.extend([
                rex(wide, general),
                0xc1,
                0xc0 | (if left { 4 } else { 5 }) << 3 | general & 7,
                count,
            ]),
            Instruction::Compare {
                wide,
                general,
                immediate,
            } => {
                bytes.extend([rex(wide, general), 0x81, 0xf8 | general & 7]);
                bytes.extend(immediate.to_le_bytes());
            }
            Instruction::Conditional {
                equal,
                displacement,
            } => {
                bytes.extend([0x0f, if equal { 0x84 } else { 0x85 }]);
                bytes.extend(displacement.to_le_bytes());
            }
            Instruction::Jump { displacement } => {
                bytes.push(0xe9);
                bytes.extend(displacement.to_le_bytes());
            }
            Instruction::SubtractOne { wide, general } => {
                bytes.extend([rex(wide, general), 0x83, 0xe8 | general & 7, 1])
            }
            Instruction::Crash => bytes.extend([0x0f, 0x0b]),
        }
    }

    fn decode_instruction(bytes: &[u8]) -> Option<(Instruction, &[u8])> {
        let immediate = |offset| crate::codec_bytes::u32_at(bytes, offset);
        match bytes {
            [0x0f, 0x0b, tail @ ..] => Some((Instruction::Crash, tail)),
            [0x0f, condition @ (0x84 | 0x85), ..] => Some((
                Instruction::Conditional {
                    equal: *condition == 0x84,
                    displacement: immediate(2)? as i32,
                },
                bytes.get(6..)?,
            )),
            [0xe9, ..] => Some((
                Instruction::Jump {
                    displacement: immediate(1)? as i32,
                },
                bytes.get(5..)?,
            )),
            [0x66, rex, 0x0f, opcode @ (0x6e | 0x7e), operands, tail @ ..]
                if rex & 0xf6 == 0x40 && operands & 0xc0 == 0xc0 =>
            {
                Some((
                    Instruction::Transfer {
                        save: *opcode == 0x6e,
                        wide: rex & 8 != 0,
                        vector: (operands >> 3) & 7,
                        general: (operands & 7) | (rex & 1) << 3,
                    },
                    tail,
                ))
            }
            [rex, opcode, operands, ..] if rex & 0xf6 == 0x40 && operands & 0xc0 == 0xc0 => {
                let wide = rex & 8 != 0;
                let general = (operands & 7) | (rex & 1) << 3;
                let extension = (operands >> 3) & 7;
                match (*opcode, extension) {
                    (0xc1, 4 | 5) => {
                        let count = *bytes.get(3)?;
                        if count == 0 || count >= if wide { 64 } else { 32 } {
                            return None;
                        }
                        Some((
                            Instruction::Shift {
                                left: extension == 4,
                                wide,
                                general,
                                count,
                            },
                            bytes.get(4..)?,
                        ))
                    }
                    (0x81, 7) => Some((
                        Instruction::Compare {
                            wide,
                            general,
                            immediate: immediate(3)?,
                        },
                        bytes.get(7..)?,
                    )),
                    (0x83, 5) if bytes.get(3) == Some(&1) => {
                        Some((Instruction::SubtractOne { wide, general }, bytes.get(4..)?))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    struct Replay<'a> {
        bytes: &'a [u8],
        position: usize,
    }

    impl Replay<'_> {
        fn next(&mut self) -> Result<Instruction, Error> {
            let bytes = self
                .bytes
                .get(self.position..)
                .ok_or(Error::MalformedEncoding)?;
            let (instruction, tail) = decode_instruction(bytes).ok_or(Error::MalformedEncoding)?;
            self.position += bytes.len() - tail.len();
            Ok(instruction)
        }
        fn require(&mut self, expected: Instruction) -> Result<(), Error> {
            if self.next()? == expected {
                Ok(())
            } else {
                Err(Error::EncodedFormMismatch)
            }
        }
        fn branch(&mut self, equal: bool, destination: usize) -> Result<(), Error> {
            let Instruction::Conditional {
                equal: actual_equal,
                displacement,
            } = self.next()?
            else {
                return Err(Error::EncodedFormMismatch);
            };
            if actual_equal == equal
                && self.position as i64 + i64::from(displacement) == destination as i64
            {
                Ok(())
            } else {
                Err(Error::EncodedFormMismatch)
            }
        }
        fn exponent(&mut self, wide: bool, vector: u8, general: u8) -> Result<(), Error> {
            self.require(Instruction::Transfer {
                save: false,
                wide,
                vector,
                general,
            })?;
            self.require(Instruction::Shift {
                left: true,
                wide,
                general,
                count: 1,
            })?;
            self.require(Instruction::Shift {
                left: false,
                wide,
                general,
                count: if wide { 53 } else { 24 },
            })?;
            self.require(Instruction::Compare {
                wide,
                general,
                immediate: if wide { 2047 } else { 255 },
            })
        }
        fn shifted_zero(
            &mut self,
            wide: bool,
            vector: u8,
            general: u8,
            count: u8,
        ) -> Result<(), Error> {
            self.require(Instruction::Transfer {
                save: false,
                wide,
                vector,
                general,
            })?;
            self.require(Instruction::Shift {
                left: true,
                wide,
                general,
                count,
            })?;
            self.require(Instruction::Compare {
                wide,
                general,
                immediate: 0,
            })
        }
    }

    fn validate_program(
        form: FloatResultPolicyForm,
        registers: &[u8],
        bytes: &[u8],
    ) -> Result<(), Error> {
        let wide = !form.is_binary32();
        let inputs = form.operand_count();
        let general = registers[inputs];
        let mut replay = Replay { bytes, position: 0 };
        for (position, &source) in registers[..inputs].iter().enumerate() {
            replay.require(Instruction::Transfer {
                save: true,
                wide,
                vector: 3 + position as u8,
                general: source,
            })?;
        }
        if form.can_trap() {
            replay.exponent(wide, 3, general)?;
            replay.branch(false, replay.position + 8)?;
            replay.require(Instruction::Crash)?;
            replay.require(Instruction::Transfer {
                save: false,
                wide,
                vector: 3,
                general,
            })?;
        } else {
            let raw_destination = bytes.len().checked_sub(5).ok_or(Error::MalformedEncoding)?;
            replay.require(Instruction::Transfer {
                save: false,
                wide,
                vector: 3,
                general,
            })?;
            replay.require(Instruction::Shift {
                left: true,
                wide,
                general,
                count: if wide { 56 } else { 24 },
            })?;
            replay.require(Instruction::Shift {
                left: false,
                wide,
                general,
                count: if wide { 56 } else { 24 },
            })?;
            replay.require(Instruction::Compare {
                wide,
                general,
                immediate: 0,
            })?;
            replay.branch(true, raw_destination)?;
            replay.exponent(wide, 4, general)?;
            replay.branch(false, raw_destination)?;
            replay.shifted_zero(wide, 4, general, if wide { 12 } else { 9 })?;
            replay.branch(false, raw_destination)?;
            replay.require(Instruction::Transfer {
                save: false,
                wide,
                vector: 4,
                general,
            })?;
            replay.require(Instruction::SubtractOne { wide, general })?;
            let Instruction::Jump { displacement } = replay.next()? else {
                return Err(Error::EncodedFormMismatch);
            };
            if replay.position != raw_destination
                || replay.position as i64 + i64::from(displacement) != bytes.len() as i64
            {
                return Err(Error::EncodedFormMismatch);
            }
            replay.require(Instruction::Transfer {
                save: false,
                wide,
                vector: 4,
                general,
            })?;
        }
        if replay.position != bytes.len() {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(())
    }

    fn registers(
        physical: &ValidatedPhysicalRegisterModel,
        form: FloatResultPolicyForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, Error> {
        super::request_validation::validate_request(
            physical,
            SelectedInstructionKind::FloatResultPolicy { form },
            alternative,
            operands,
        )?;
        super::request_validation::resolve_registers(physical, operands)
    }

    pub(crate) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        form: FloatResultPolicyForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let registers = registers(physical, form, alternative, operands)?;
        let mut bytes = Vec::new();
        for instruction in program(form, &registers) {
            encode_instruction(instruction, &mut bytes);
        }
        validate(physical, form, alternative, operands, &bytes)
    }

    pub(crate) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        form: FloatResultPolicyForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let registers = registers(physical, form, alternative, operands)?;
        validate_program(form, &registers, bytes)?;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: footprint(form, operands),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use isa_conformance::float_result_policy as policy;
        use selected_instructions::IeeeFloatFormat;

        fn key(form: FloatResultPolicyForm) -> MachineAlternativeKey {
            MachineAlternativeKey {
                family: MachineAlternativeFamily::FloatResultPolicy(form),
                variant: 0,
            }
        }

        fn encoded(form: FloatResultPolicyForm, registers: &[u8]) -> Vec<u8> {
            let mut bytes = Vec::new();
            for instruction in program(form, registers) {
                encode_instruction(instruction, &mut bytes);
            }
            validate_program(form, registers, &bytes).unwrap();
            bytes
        }

        fn validated(form: FloatResultPolicyForm, registers: &[u8], bytes: &[u8]) -> bool {
            registers.len() == form.operand_count() + 1
                && validate_program(form, registers, bytes).is_ok()
        }

        fn roster(form: FloatResultPolicyForm) -> Vec<u8> {
            if form.can_trap() {
                vec![1, 8]
            } else {
                vec![1, 8, 10]
            }
        }

        fn replay(bytes: &[u8], registers: &[u8], payloads: &[u64]) -> Option<u64> {
            let mut general = [0xa5a5_a5a5_a5a5_a5a5_u64; 16];
            let mut vectors = [0x5a5a_5a5a_5a5a_5a5a_u64; 16];
            for (&register, &payload) in registers.iter().zip(payloads) {
                general[usize::from(register)] = payload;
            }
            let sized = |value, wide| if wide { value } else { value & 0xffff_ffff };
            let mut position = 0_usize;
            let mut equal = false;
            let mut steps = 0;
            while position < bytes.len() {
                steps += 1;
                assert!(steps <= bytes.len(), "policy must have no loop");
                let (instruction, tail) = decode_instruction(&bytes[position..]).unwrap();
                position = bytes.len() - tail.len();
                match instruction {
                    Instruction::Transfer {
                        save,
                        wide,
                        vector,
                        general: register,
                    } => {
                        if save {
                            vectors[usize::from(vector)] =
                                sized(general[usize::from(register)], wide);
                        } else {
                            general[usize::from(register)] =
                                sized(vectors[usize::from(vector)], wide);
                        }
                    }
                    Instruction::Shift {
                        left,
                        wide,
                        general: register,
                        count,
                    } => {
                        let payload = sized(general[usize::from(register)], wide);
                        general[usize::from(register)] = sized(
                            if left {
                                payload << count
                            } else {
                                payload >> count
                            },
                            wide,
                        );
                        equal = general[usize::from(register)] == 0;
                    }
                    Instruction::Compare {
                        wide,
                        general: register,
                        immediate,
                    } => {
                        equal = sized(general[usize::from(register)], wide) == u64::from(immediate)
                    }
                    Instruction::Conditional {
                        equal: condition,
                        displacement,
                    } if equal == condition => {
                        position =
                            usize::try_from(position as i64 + i64::from(displacement)).unwrap();
                    }
                    Instruction::Conditional { .. } => {}
                    Instruction::Jump { displacement } => {
                        position =
                            usize::try_from(position as i64 + i64::from(displacement)).unwrap()
                    }
                    Instruction::SubtractOne {
                        wide,
                        general: register,
                    } => {
                        general[usize::from(register)] =
                            sized(general[usize::from(register)].wrapping_sub(1), wide);
                        equal = general[usize::from(register)] == 0;
                    }
                    Instruction::Crash => return None,
                }
            }
            assert_eq!(position, bytes.len());
            Some(general[usize::from(*registers.last().unwrap())])
        }

        #[test]
        fn float_result_policy_trapping_bytes_bind_width_registers_and_branches() {
            let form = FloatResultPolicyForm {
                format: IeeeFloatFormat::Binary64,
                policy: FloatResultPolicyKind::Trapping,
            };
            policy::exact_stream(
                form,
                &[1, 8],
                &[
                    0x66, 0x48, 0x0f, 0x6e, 0xd9, 0x66, 0x49, 0x0f, 0x7e, 0xd8, 0x49, 0xc1, 0xe0,
                    1, 0x49, 0xc1, 0xe8, 53, 0x49, 0x81, 0xf8, 0xff, 7, 0, 0, 0x0f, 0x85, 2, 0, 0,
                    0, 0x0f, 0x0b, 0x66, 0x49, 0x0f, 0x7e, 0xd8,
                ],
                encoded,
                validated,
            );
            policy::rejects_corrupted_and_substituted_streams(roster, encoded, validated);
        }

        #[test]
        fn float_result_policy_decoded_semantics_preserve_payloads_and_aliases() {
            for form in FloatResultPolicyForm::ALL {
                for case in policy::cases(!form.is_binary32()) {
                    let expected = policy::expected(form, case);
                    let inputs = policy::inputs(form, case);
                    for first in [0, 8] {
                        for output in first..=first + inputs.len() as u8 {
                            let mut registers =
                                (first..first + inputs.len() as u8).collect::<Vec<_>>();
                            registers.push(output);
                            assert_eq!(
                                replay(&encoded(form, &registers), &registers, &inputs),
                                expected,
                                "{form:?} {inputs:x?} {registers:?}"
                            );
                        }
                    }
                }
            }
        }

        #[test]
        fn float_result_policy_public_route_binds_constraints_and_effects() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let constraints = crate::x86_64_register_constraint_catalog(&physical);
            let all = ["rcx", "r8", "r10", "r15"]
                .map(|name| physical.model().view_named(name).unwrap().id);
            for form in FloatResultPolicyForm::ALL {
                let operands = &all[..form.operand_count() + 1];
                let kind = SelectedInstructionKind::FloatResultPolicy { form };
                let result =
                    crate::encode_x86_64_selected_form(&physical, kind, key(form), operands)
                        .unwrap();
                crate::validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key(form),
                    operands,
                    result.bytes(),
                )
                .unwrap();
                let footprint = result.footprint();
                let constraint_key = if form.can_trap() {
                    crate::X86_64_FLOAT_RESULT_TRAPPING
                } else {
                    crate::X86_64_FLOAT_RESULT_SATURATING
                };
                let constraint = constraints
                    .constraints
                    .iter()
                    .find(|constraint| constraint.key == constraint_key)
                    .unwrap();
                let declaration = declaration(form, constraint_key);
                assert_eq!(constraint.operands.len(), form.operand_count() + 1);
                assert!(constraint.implicit_uses.is_empty());
                assert!(footprint.encoded.implicit_unit_uses.is_empty());
                let scratch: &[&str] = if form.can_trap() {
                    &["xmm3", "condition_flags"]
                } else {
                    &["xmm3", "xmm4", "condition_flags"]
                };
                let mut expected_clobbers = scratch
                    .iter()
                    .flat_map(|name| {
                        physical
                            .model()
                            .view_named(name)
                            .expect("policy scratch must exist in the canonical model")
                            .units
                            .iter()
                            .copied()
                    })
                    .collect::<Vec<_>>();
                expected_clobbers.sort_unstable();
                expected_clobbers.dedup();
                assert_eq!(constraint.clobbers, expected_clobbers);
                assert_eq!(footprint.encoded.implicit_unit_clobbers, expected_clobbers);
                assert_eq!(
                    constraint.clobbers,
                    footprint.encoded.implicit_unit_clobbers
                );
                assert_eq!(declaration.alternatives[0].encoded, footprint.encoded);
                assert_eq!(
                    declaration.alternatives[0].size,
                    MachineSizeKnowledge::ExactBytes(result.bytes().len() as u16)
                );
                assert_eq!(
                    footprint.encoded.control,
                    if form.can_trap() {
                        MachineEncodedControlEffect::FallThroughOrTrapV1
                    } else {
                        MachineEncodedControlEffect::FallThroughV1
                    }
                );
                assert!(footprint.writes_rflags);
                assert!(
                    crate::encode_x86_64_selected_form(
                        &physical,
                        kind,
                        MachineAlternativeKey {
                            variant: 1,
                            ..key(form)
                        },
                        operands
                    )
                    .is_err()
                );
                assert!(
                    crate::encode_x86_64_selected_form(&physical, kind, key(form), &operands[..1])
                        .is_err()
                );
                let mut invalid = operands.to_vec();
                invalid[0] = physical.model().view_named("xmm3").unwrap().id;
                assert!(
                    crate::encode_x86_64_selected_form(&physical, kind, key(form), &invalid)
                        .is_err()
                );
            }
        }
    }
}
pub(crate) mod floating_control {
    use super::{
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
        X86_64SelectedFormFootprint,
    };
    use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeKey, MachineBarrier, MachineEffectDeclaration,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
        MachineSemanticKind, MachineTrapBehavior, SelectedInstructionKind,
    };

    pub(crate) fn effects(restore: bool) -> MachineEncodedEffects {
        let physical = crate::x86_64_physical_register_model_ref();
        let view = |name| {
            physical
                .view_named(name)
                .expect("canonical floating-control register")
        };
        let mut uses = view("rsp").units.clone();
        if !restore {
            uses.extend(view("mxcsr").units.iter().copied());
        }
        uses.sort_unstable();
        MachineEncodedEffects {
            external_operand_reads: Vec::new(),
            external_operand_writes: Vec::new(),
            implicit_unit_uses: uses,
            implicit_unit_defs: if restore {
                view("mxcsr").units.clone()
            } else {
                Vec::new()
            },
            implicit_unit_clobbers: Vec::new(),
            memory: if restore {
                MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                    stack_pointer: view("rsp").id,
                    byte_count: 4,
                }
            } else {
                MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                    stack_pointer: view("rsp").id,
                    byte_count: 4,
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
            crate::encoder_resolved(4, Some(8)),
            effects(restore),
        )
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<bool, X86_64SelectedFormEncodingError> {
        let semantic = match kind {
            SelectedInstructionKind::SaveFloatingControl {
                slot: LocalStorageSlotId::Boundary { .. },
            } => MachineSemanticKind::SaveFloatingControl,
            SelectedInstructionKind::RestoreFloatingControl {
                slot: LocalStorageSlotId::Boundary { .. },
            } => MachineSemanticKind::RestoreFloatingControl,
            _ => return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch),
        };
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
            || !operands.is_empty()
            || alternative
                != (MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                })
            || !(0..=i64::from(i32::MAX)).contains(&displacement)
            || displacement % 8 != 0
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(semantic == MachineSemanticKind::RestoreFloatingControl)
    }

    pub(crate) fn encode_x86_64_selected_floating_control_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let restore = request(physical, kind, alternative, operands, displacement)?;
        let offset = u32::try_from(displacement)
            .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        let bytes = if restore {
            crate::encode_ldmxcsr_rsp_displacement(offset)
        } else {
            crate::encode_stmxcsr_rsp_displacement(offset)
        }
        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        validate_x86_64_selected_floating_control_form(
            physical,
            kind,
            alternative,
            operands,
            displacement,
            &bytes,
        )
    }

    pub(crate) fn validate_x86_64_selected_floating_control_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let restore = request(physical, kind, alternative, operands, displacement)?;
        if decode(bytes) != Some((restore, displacement)) {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: Vec::new(),
                register_writes: Vec::new(),
                writes_rflags: false,
                encoded: effects(restore),
            },
        })
    }

    fn decode(bytes: &[u8]) -> Option<(bool, i64)> {
        let [0x0f, 0xae, mode, 0x24, tail @ ..] = bytes else {
            return None;
        };
        let restore = match mode & 0x3f {
            0x14 => true,
            0x1c => false,
            _ => return None,
        };
        let displacement = match (mode >> 6, tail) {
            (0, []) => 0,
            (1, [offset]) if (1..=127).contains(offset) => i64::from(*offset),
            (2, encoded_displacement) if encoded_displacement.len() == 4 => {
                let offset = i32::from_le_bytes(encoded_displacement.try_into().ok()?);
                if offset < 128 {
                    return None;
                }
                i64::from(offset)
            }
            _ => return None,
        };
        Some((restore, displacement))
    }

    #[cfg(test)]
    mod tests {
        use super::{
            effects, encode_x86_64_selected_floating_control_form,
            validate_x86_64_selected_floating_control_form,
        };
        use selected_instructions::{
            LocalStorageSlotId, MachineAlternativeKey, MachineSemanticKind, SelectedInstructionKind,
        };
        #[test]
        fn floating_controls_bind_direction_slot_displacement_and_every_encoded_bit() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
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
                for displacement in [0, 8, 120, 128, 4096, 2147483640] {
                    let encoded = encode_x86_64_selected_floating_control_form(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        displacement,
                    )
                    .unwrap();
                    assert_eq!(encoded.footprint().encoded, effects(restore));
                    crate::selected_form_encoding::assert_every_bit_flip_rejected(
                        encoded.bytes(),
                        |changed| {
                            validate_x86_64_selected_floating_control_form(
                                &physical,
                                kind,
                                alternative,
                                &[],
                                displacement,
                                changed,
                            )
                            .is_err()
                        },
                    );
                    assert!(
                        validate_x86_64_selected_floating_control_form(
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
                        validate_x86_64_selected_floating_control_form(
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
                for displacement in [-8, 4, 2147483648] {
                    assert!(
                        encode_x86_64_selected_floating_control_form(
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
                    encode_x86_64_selected_floating_control_form(
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
pub(crate) mod ieee_float_arithmetic {
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, RegisterViewId,
        ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
    };
    use crate::register_model::units;
    use crate::selected_form_encoding::request_validation::resolve_registers;
    use register_model::{RegisterConstraintKey, RegisterUnitId, sorted_units};
    use selected_instructions::{
        IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation,
        IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain,
        MachineBarrier, MachineEffectDeclaration, MachineEncodedControlEffect,
        MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
        MachineTrapBehavior, ieee_float_arithmetic_is_trapping,
    };

    mod conversions {
        use super::{IeeeFloatIntegerCarrier, Instruction, LEFT_SCRATCH, RIGHT_SCRATCH};

        const ZERO: u8 = 4;
        const NOT_ZERO: u8 = 5;
        const NOT_SIGNED: u8 = 9;
        const LESS: u8 = 1;
        const LESS_EQUAL: u8 = 2;
        const TWO_TO_64: u64 = 0x43f0_0000_0000_0000;

        fn transfer(to_float: bool, wide: bool, xmm: u8, general: u8) -> Instruction {
            Instruction::Transfer {
                to_float,
                wide,
                xmm,
                general,
            }
        }

        fn bound(result: u8, bits: u64) -> [Instruction; 2] {
            [
                Instruction::MoveImmediate64 {
                    general: result,
                    value: bits,
                },
                transfer(true, true, RIGHT_SCRATCH, result),
            ]
        }

        fn observe_mask(result: u8) -> [Instruction; 2] {
            [
                transfer(false, true, RIGHT_SCRATCH, result),
                Instruction::Test64 { general: result },
            ]
        }

        fn scalar(opcode: u8) -> Instruction {
            Instruction::Scalar {
                double: true,
                opcode,
                destination: LEFT_SCRATCH,
                source: RIGHT_SCRATCH,
            }
        }

        fn compare_bound(predicate: u8) -> Instruction {
            Instruction::CompareScalar {
                double: true,
                predicate,
                destination: RIGHT_SCRATCH,
                source: LEFT_SCRATCH,
            }
        }

        fn encoded_length(instructions: &[Instruction]) -> i32 {
            let mut bytes = Vec::new();
            for instruction in instructions {
                super::encode_instruction(&mut bytes, *instruction);
            }
            bytes.len() as i32
        }

        fn choose(
            program: &mut Vec<Instruction>,
            condition: u8,
            taken: Vec<Instruction>,
            not_taken: Vec<Instruction>,
        ) {
            program.push(Instruction::ConditionalJump {
                condition,
                displacement: encoded_length(&not_taken) + 5,
            });
            program.extend(not_taken);
            program.push(Instruction::Jump {
                displacement: encoded_length(&taken),
            });
            program.extend(taken);
        }

        pub(super) fn from_unsigned64(double: bool, source: u8, result: u8) -> Vec<Instruction> {
            let (left, right) = (LEFT_SCRATCH, RIGHT_SCRATCH);
            let mut program = vec![
                transfer(true, true, left, source),
                Instruction::Test64 { general: source },
            ];
            let low = vec![Instruction::IntegerToFloat {
                double,
                wide: true,
                xmm: left,
                general: source,
            }];
            let high = vec![
                Instruction::MoveAligned {
                    destination: right,
                    source: left,
                },
                Instruction::ShiftLeft {
                    quad: true,
                    xmm: right,
                    count: 63,
                },
                Instruction::ShiftRight {
                    quad: true,
                    arithmetic: false,
                    xmm: right,
                    count: 63,
                },
                Instruction::ShiftRight {
                    quad: true,
                    arithmetic: false,
                    xmm: left,
                    count: 1,
                },
                Instruction::Or {
                    destination: left,
                    source: right,
                },
                transfer(false, true, left, result),
                Instruction::IntegerToFloat {
                    double,
                    wide: true,
                    xmm: left,
                    general: result,
                },
                Instruction::Scalar {
                    double,
                    opcode: 0x58,
                    destination: left,
                    source: left,
                },
            ];
            choose(&mut program, NOT_SIGNED, low, high);
            program.push(transfer(false, double, left, result));
            program
        }

        pub(super) fn to_unsigned64(result: u8) -> Vec<Instruction> {
            let (left, right) = (LEFT_SCRATCH, RIGHT_SCRATCH);
            let mut program = vec![
                Instruction::MoveAligned {
                    destination: right,
                    source: left,
                },
                Instruction::CompareScalar {
                    double: true,
                    predicate: super::ORDERED,
                    destination: right,
                    source: left,
                },
                Instruction::And {
                    destination: left,
                    source: right,
                },
                Instruction::Xor {
                    double: true,
                    destination: right,
                    source: right,
                },
                scalar(0x5f),
            ];
            program.extend(bound(result, TWO_TO_64));
            program.push(compare_bound(LESS_EQUAL));
            program.extend(observe_mask(result));

            let mut in_range = Vec::new();
            in_range.extend(bound(result, super::TWO_TO_63));
            in_range.push(compare_bound(LESS_EQUAL));
            in_range.extend(observe_mask(result));
            let low = vec![Instruction::FloatToInteger {
                wide: true,
                general: result,
                xmm: left,
            }];
            let mut high = Vec::from(bound(result, super::TWO_TO_63));
            high.extend([
                scalar(0x5c),
                Instruction::FloatToInteger {
                    wide: true,
                    general: result,
                    xmm: left,
                },
                transfer(true, true, left, result),
                Instruction::AllOnes { xmm: right },
                Instruction::ShiftLeft {
                    quad: true,
                    xmm: right,
                    count: 63,
                },
                Instruction::Or {
                    destination: left,
                    source: right,
                },
                transfer(false, true, left, result),
            ]);
            choose(&mut in_range, NOT_ZERO, high, low);
            choose(
                &mut program,
                NOT_ZERO,
                vec![Instruction::MoveImmediate64 {
                    general: result,
                    value: u64::MAX,
                }],
                in_range,
            );
            program
        }

        fn interval(target: IeeeFloatIntegerCarrier) -> (u64, u8, u64) {
            use IeeeFloatIntegerCarrier::*;
            match target {
                I8 => (0xc060_2000_0000_0000, LESS, 0x4060_0000_0000_0000),
                I16 => (0xc0e0_0020_0000_0000, LESS, 0x40e0_0000_0000_0000),
                I32 => (0xc1e0_0000_0020_0000, LESS, super::TWO_TO_31),
                I64 => (0xc3e0_0000_0000_0000, LESS_EQUAL, super::TWO_TO_63),
                U8 => (0xbff0_0000_0000_0000, LESS, 0x4070_0000_0000_0000),
                U16 => (0xbff0_0000_0000_0000, LESS, 0x40f0_0000_0000_0000),
                U32 => (0xbff0_0000_0000_0000, LESS, 0x41f0_0000_0000_0000),
                U64 => (0xbff0_0000_0000_0000, LESS, TWO_TO_64),
            }
        }

        pub(super) fn trapping(
            double: bool,
            target: IeeeFloatIntegerCarrier,
            source: u8,
            result: u8,
        ) -> Vec<Instruction> {
            let mut program = vec![transfer(true, double, LEFT_SCRATCH, source)];
            if !double {
                program.push(Instruction::Scalar {
                    double: false,
                    opcode: 0x5a,
                    destination: LEFT_SCRATCH,
                    source: LEFT_SCRATCH,
                });
            }
            let (minimum, lower_predicate, upper) = interval(target);
            program.extend(bound(result, minimum));
            program.push(compare_bound(lower_predicate));
            program.extend(observe_mask(result));
            program.extend([
                Instruction::ConditionalJump {
                    condition: NOT_ZERO,
                    displacement: 2,
                },
                Instruction::Crash,
            ]);
            program.extend(bound(result, upper));
            program.push(compare_bound(LESS_EQUAL));
            program.extend(observe_mask(result));
            program.extend([
                Instruction::ConditionalJump {
                    condition: ZERO,
                    displacement: 2,
                },
                Instruction::Crash,
            ]);
            program.extend(super::truncation_landed(target, result));
            program
        }

        #[cfg(test)]
        mod tests {
            use super::super::{decode_instruction, encode_instruction};
            use super::*;
            use selected_instructions::{
                IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation as Operation,
                IeeeFloatFormat, IeeeFloatToIntegerDomain as Domain, MachineAlternativeFamily,
                MachineAlternativeKey, MachineEncodedControlEffect, MachineEncodedTrapBehavior,
            };

            fn decoded(bytes: &[u8]) -> Vec<(usize, Instruction, usize)> {
                let mut instructions = Vec::new();
                let mut rest = bytes;
                while !rest.is_empty() {
                    let start = bytes.len() - rest.len();
                    let (instruction, tail) =
                        decode_instruction(rest).expect("independent instruction decode");
                    instructions.push((start, instruction, rest.len() - tail.len()));
                    rest = tail;
                }
                instructions
            }

            #[test]
            fn conversion_control_opcodes_have_architectural_bytes_and_independent_decode() {
                for (instruction, expected) in [
                    (Instruction::Test64 { general: 0 }, vec![0x48, 0x85, 0xc0]),
                    (Instruction::Test64 { general: 9 }, vec![0x4d, 0x85, 0xc9]),
                    (
                        Instruction::Or {
                            destination: 4,
                            source: 5,
                        },
                        vec![0x66, 0x0f, 0xeb, 0xe5],
                    ),
                    (
                        Instruction::ConditionalJump {
                            condition: NOT_ZERO,
                            displacement: 2,
                        },
                        vec![0x0f, 0x85, 2, 0, 0, 0],
                    ),
                    (
                        Instruction::ConditionalJump {
                            condition: NOT_SIGNED,
                            displacement: 17,
                        },
                        vec![0x0f, 0x89, 17, 0, 0, 0],
                    ),
                    (
                        Instruction::Jump { displacement: 10 },
                        vec![0xe9, 10, 0, 0, 0],
                    ),
                    (Instruction::Crash, vec![0x0f, 0x0b]),
                ] {
                    let mut bytes = Vec::new();
                    encode_instruction(&mut bytes, instruction);
                    assert_eq!(bytes, expected);
                    assert_eq!(decode_instruction(&expected), Some((instruction, &[][..])));
                }
            }

            #[test]
            fn unsigned64_from_integer_uses_one_format_rounding_and_sticky_half() {
                for double in [false, true] {
                    let program = from_unsigned64(double, 9, 9);
                    assert!(program.contains(&Instruction::ShiftRight {
                        quad: true,
                        arithmetic: false,
                        xmm: LEFT_SCRATCH,
                        count: 1,
                    }));
                    assert!(program.contains(&Instruction::ShiftLeft {
                        quad: true,
                        xmm: RIGHT_SCRATCH,
                        count: 63,
                    }));
                    assert!(program.contains(&Instruction::ShiftRight {
                        quad: true,
                        arithmetic: false,
                        xmm: RIGHT_SCRATCH,
                        count: 63,
                    }));
                    assert!(program.contains(&Instruction::Or {
                        destination: LEFT_SCRATCH,
                        source: RIGHT_SCRATCH,
                    }));
                    assert_eq!(program.iter().filter(|instruction| matches!(instruction,
            Instruction::IntegerToFloat { double: format, wide: true, .. } if *format == double
        )).count(), 2);
                    assert!(program.contains(&Instruction::Scalar {
                        double,
                        opcode: 0x58,
                        destination: LEFT_SCRATCH,
                        source: LEFT_SCRATCH,
                    }));
                    assert!(
                        !program.iter().any(|instruction| matches!(
                            instruction,
                            Instruction::Scalar { opcode: 0x5a, .. }
                        )),
                        "u64 to binary32 must not round through binary64"
                    );
                }
                for value in [
                    1u64 << 63,
                    (1u64 << 63) + 1,
                    (1u64 << 63) + 1023,
                    (1u64 << 63) + 1024,
                    (1u64 << 63) + 1025,
                    (1u64 << 63) + (1u64 << 39) - 1,
                    (1u64 << 63) + (1u64 << 39),
                    (1u64 << 63) + (1u64 << 39) + 1,
                    u64::MAX,
                ] {
                    let half = (value >> 1) | (value & 1);
                    assert_eq!(
                        ((half as i64 as f32) * 2.0).to_bits(),
                        (value as f32).to_bits()
                    );
                    assert_eq!(
                        ((half as i64 as f64) * 2.0).to_bits(),
                        (value as f64).to_bits()
                    );
                }
            }

            #[test]
            fn trapping_guards_replay_exact_toward_zero_intervals() {
                use IeeeFloatIntegerCarrier::*;
                for target in [I8, I16, I32, I64, U8, U16, U32, U64] {
                    for double in [false, true] {
                        let mut bytes = Vec::new();
                        for instruction in trapping(double, target, 9, 9) {
                            encode_instruction(&mut bytes, instruction);
                        }
                        let instructions = decoded(&bytes)
                            .into_iter()
                            .map(|(_, instruction, _)| instruction)
                            .collect::<Vec<_>>();
                        let guards = instructions
                            .windows(7)
                            .filter_map(|window| match window {
                                [
                                    Instruction::MoveImmediate64 { general: 9, value },
                                    Instruction::Transfer {
                                        to_float: true,
                                        wide: true,
                                        xmm: RIGHT_SCRATCH,
                                        general: 9,
                                    },
                                    Instruction::CompareScalar {
                                        double: true,
                                        predicate,
                                        destination: RIGHT_SCRATCH,
                                        source: LEFT_SCRATCH,
                                    },
                                    Instruction::Transfer {
                                        to_float: false,
                                        wide: true,
                                        xmm: RIGHT_SCRATCH,
                                        general: 9,
                                    },
                                    Instruction::Test64 { general: 9 },
                                    Instruction::ConditionalJump {
                                        condition,
                                        displacement: 2,
                                    },
                                    Instruction::Crash,
                                ] => Some((f64::from_bits(*value), *predicate, *condition)),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(guards.len(), 2);
                        assert_eq!(guards[0].2, NOT_ZERO);
                        assert_eq!((guards[1].1, guards[1].2), (LESS_EQUAL, ZERO));
                        let minimum = if target.is_signed() {
                            -(1i128 << (target.bits() - 1))
                        } else {
                            0
                        };
                        let maximum =
                            (1i128 << (target.bits() - u16::from(target.is_signed()))) - 1;
                        let lower = guards[0].0;
                        let upper = guards[1].0;
                        for operand in [
                            f64::NAN,
                            f64::NEG_INFINITY,
                            f64::INFINITY,
                            -0.0,
                            0.0,
                            -1.0,
                            -0.75,
                            1.25,
                            lower - 1.0,
                            lower,
                            lower + 0.5,
                            minimum as f64,
                            f64::from_bits(lower.to_bits() + 1),
                            f64::from_bits(lower.to_bits() - 1),
                            f64::from_bits(upper.to_bits() - 1),
                            upper,
                            f64::from_bits(upper.to_bits() + 1),
                        ] {
                            let operand = if double {
                                operand
                            } else {
                                f64::from(operand as f32)
                            };
                            let guards_allow =
                                guards.iter().all(|(bound, predicate, condition)| {
                                    let mask = match *predicate {
                                        LESS => *bound < operand,
                                        LESS_EQUAL => *bound <= operand,
                                        _ => panic!("unexpected comparison"),
                                    };
                                    if *condition == NOT_ZERO { mask } else { !mask }
                                });
                            let truncated = operand.trunc() as i128;
                            let expected =
                                operand.is_finite() && truncated >= minimum && truncated <= maximum;
                            assert_eq!(
                                guards_allow, expected,
                                "{target:?} double={double}, {operand:?}"
                            );
                        }
                    }
                }
            }

            #[test]
            fn unsigned64_saturation_replays_zero_split_and_overflow_bounds() {
                let instructions = to_unsigned64(0);
                assert!(instructions.contains(&Instruction::CompareScalar {
                    double: true,
                    predicate: super::super::ORDERED,
                    destination: RIGHT_SCRATCH,
                    source: LEFT_SCRATCH,
                }));
                assert!(
                    instructions.contains(&scalar(0x5f)),
                    "negative values clamp to zero"
                );
                let constants = instructions
                    .iter()
                    .filter_map(|instruction| match instruction {
                        Instruction::MoveImmediate64 { value, .. } => Some(*value),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    constants,
                    [
                        TWO_TO_64,
                        super::super::TWO_TO_63,
                        super::super::TWO_TO_63,
                        u64::MAX
                    ]
                );
                assert!(
                    instructions.contains(&scalar(0x5c)),
                    "high half subtracts 2^63 before signed truncation"
                );
                assert!(instructions.contains(&Instruction::ShiftLeft {
                    quad: true,
                    xmm: RIGHT_SCRATCH,
                    count: 63
                }));
                assert!(instructions.contains(&Instruction::Or {
                    destination: LEFT_SCRATCH,
                    source: RIGHT_SCRATCH
                }));
            }

            #[test]
            fn conversion_control_replay_binds_local_branches_aliasing_and_effects() {
                let physical = register_model::validate_physical_register_model(
                    crate::x86_64_physical_register_model(),
                )
                .unwrap();
                let flags = &physical
                    .model()
                    .view_named("condition_flags")
                    .unwrap()
                    .units;
                for operation in [
                    Operation::FromInteger(IeeeFloatIntegerCarrier::U64),
                    Operation::TruncateToInteger {
                        target: IeeeFloatIntegerCarrier::U64,
                        domain: Domain::Exact,
                    },
                    Operation::TruncateToInteger {
                        target: IeeeFloatIntegerCarrier::U64,
                        domain: Domain::Saturating,
                    },
                    Operation::TruncateToInteger {
                        target: IeeeFloatIntegerCarrier::I8,
                        domain: Domain::Trapping,
                    },
                    Operation::TruncateToInteger {
                        target: IeeeFloatIntegerCarrier::U64,
                        domain: Domain::Trapping,
                    },
                ] {
                    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                        for register in ["rax", "r9"] {
                            let form = IeeeFloatArithmeticForm { operation, format };
                            let key = MachineAlternativeKey {
                                family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
                                variant: 0,
                            };
                            let view = physical.model().view_named(register).unwrap().id;
                            let operands = [view, view];
                            let encoded =
                                super::super::encode(&physical, form, key, &operands).unwrap();
                            let bytes = encoded.bytes();
                            let instructions = decoded(bytes);
                            for (position, instruction, length) in &instructions {
                                if let Instruction::ConditionalJump { displacement, .. }
                                | Instruction::Jump { displacement } = instruction
                                {
                                    let destination = (*position + *length) as i32 + displacement;
                                    assert!(
                                        destination >= 0 && destination as usize <= bytes.len()
                                    );
                                    assert!(
                            destination as usize == bytes.len()
                                || instructions
                                    .iter()
                                    .any(|(start, _, _)| *start == destination as usize)
                        );
                                }
                            }
                            let footprint = encoded.footprint();
                            assert!(footprint.writes_rflags);
                            assert!(flags.iter().all(|flag| {
                                footprint.encoded.implicit_unit_clobbers.contains(flag)
                            }));
                            let traps = matches!(
                                operation,
                                Operation::TruncateToInteger {
                                    domain: Domain::Trapping,
                                    ..
                                }
                            );
                            assert_eq!(
                                footprint.encoded.trap,
                                if traps {
                                    MachineEncodedTrapBehavior::TrappingIntegerV1
                                } else {
                                    MachineEncodedTrapBehavior::NeverV1
                                }
                            );
                            assert_eq!(
                                footprint.encoded.control,
                                if traps {
                                    MachineEncodedControlEffect::FallThroughOrTrapV1
                                } else {
                                    MachineEncodedControlEffect::FallThroughV1
                                }
                            );
                            assert_eq!(
                    instructions
                        .iter()
                        .filter(|(_, instruction, _)| *instruction == Instruction::Crash)
                        .count(),
                    if traps { 2 } else { 0 }
                );
                            for (position, instruction, _) in instructions {
                                let displacement_position = match instruction {
                                    Instruction::ConditionalJump { .. } => position + 2,
                                    Instruction::Jump { .. } => position + 1,
                                    _ => continue,
                                };
                                let mut changed = bytes.to_vec();
                                changed[displacement_position] ^= 1;
                                assert!(
                                    super::super::validate(
                                        &physical, form, key, &operands, &changed
                                    )
                                    .is_err()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    type Error = X86_64SelectedFormEncodingError;

    const LEFT_SCRATCH: u8 = 4;
    const RIGHT_SCRATCH: u8 = 5;
    const ADDEND_SCRATCH: u8 = 3;

    pub(crate) fn implicit_uses() -> Vec<RegisterUnitId> {
        sorted_units(units("mxcsr"))
    }

    pub(crate) fn clobbers() -> Vec<RegisterUnitId> {
        sorted_units(units("xmm4").into_iter().chain(units("xmm5")))
    }

    fn conversion_control(form: IeeeFloatArithmeticForm) -> bool {
        matches!(
            form.operation,
            IeeeFloatArithmeticOperation::IsNan
                | IeeeFloatArithmeticOperation::IsInfinite
                | IeeeFloatArithmeticOperation::IsFinite
                | IeeeFloatArithmeticOperation::IsNormal
                | IeeeFloatArithmeticOperation::IsSubnormal
                | IeeeFloatArithmeticOperation::Classify
                | IeeeFloatArithmeticOperation::FromInteger(IeeeFloatIntegerCarrier::U64)
                | IeeeFloatArithmeticOperation::TruncateToInteger {
                    target: IeeeFloatIntegerCarrier::U64,
                    ..
                }
                | IeeeFloatArithmeticOperation::TruncateToInteger {
                    domain: IeeeFloatToIntegerDomain::Trapping,
                    ..
                }
        )
    }

    pub(crate) fn effects(form: IeeeFloatArithmeticForm) -> MachineEncodedEffects {
        let inputs = form.operand_count() as u16;
        let mut encoded = MachineEncodedEffects {
            implicit_unit_uses: implicit_uses(),
            implicit_unit_clobbers: clobbers(),
            ..MachineEncodedEffects::fallthrough_v1((0..inputs).collect(), vec![inputs])
        };
        if form.operation.rounding_direction().is_some() {
            let model = crate::x86_64_physical_register_model();
            let stack_pointer = model.view_named("rsp").expect("canonical stack pointer").id;
            encoded.implicit_unit_uses =
                sorted_units(encoded.implicit_unit_uses.into_iter().chain(units("rsp")));
            encoded.memory =
                selected_instructions::MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                    stack_pointer,
                    byte_count: 16,
                };
            encoded.stack = selected_instructions::MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: 16,
            };
            encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        }
        if conversion_control(form) {
            encoded.implicit_unit_clobbers =
                sorted_units(clobbers().into_iter().chain(units("condition_flags")));
        }
        if form.operand_count() == 3 {
            encoded.implicit_unit_clobbers =
                sorted_units(clobbers().into_iter().chain(units("xmm3")));
        }
        if ieee_float_arithmetic_is_trapping(form) {
            encoded.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
            encoded.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
        }
        encoded
    }

    pub(crate) fn footprint(
        form: IeeeFloatArithmeticForm,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        let inputs = form.operand_count();
        X86_64SelectedFormFootprint {
            register_reads: operands[..inputs].to_vec(),
            register_writes: operands[inputs..inputs + 1].to_vec(),
            writes_rflags: conversion_control(form),
            encoded: effects(form),
        }
    }

    fn size(form: IeeeFloatArithmeticForm) -> Result<MachineSizeKnowledge, Error> {
        Ok(crate::encoder_resolved(
            canonical(form, &[0, 0, 0, 0])?.len() as u16,
            Some(canonical(form, &[8, 8, 8, 8])?.len() as u16),
        ))
    }

    pub(crate) fn declaration(
        form: IeeeFloatArithmeticForm,
        constraint: RegisterConstraintKey,
    ) -> Result<MachineEffectDeclaration, Error> {
        Ok(MachineEffectDeclaration::single(
            MachineSemanticKind::NearestIeeeFloatArithmetic(form),
            constraint,
            if form.operation.rounding_direction().is_some() {
                MachineMemoryEffect::TemporaryStackStorageV1
            } else {
                MachineMemoryEffect::NoneV1
            },
            if form.operation.rounding_direction().is_some() {
                MachineTrapBehavior::MayArchitecturalFaultV1
            } else if ieee_float_arithmetic_is_trapping(form) {
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
            size(form)?,
            effects(form),
        ))
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Instruction {
        AdjustStack {
            displacement: i8,
        },
        ControlWord {
            load: bool,
            displacement: u8,
        },
        StoreControlMode {
            value: u32,
        },
        Transfer {
            to_float: bool,
            wide: bool,
            xmm: u8,
            general: u8,
        },
        Scalar {
            double: bool,
            opcode: u8,
            destination: u8,
            source: u8,
        },
        AllOnes {
            xmm: u8,
        },
        ShiftLeft {
            quad: bool,
            xmm: u8,
            count: u8,
        },
        ShiftRight {
            quad: bool,
            arithmetic: bool,
            xmm: u8,
            count: u8,
        },
        Xor {
            double: bool,
            destination: u8,
            source: u8,
        },
        IntegerToFloat {
            double: bool,
            wide: bool,
            xmm: u8,
            general: u8,
        },
        Move32 {
            destination: u8,
            source: u8,
        },
        MoveAligned {
            destination: u8,
            source: u8,
        },
        CompareScalar {
            double: bool,
            predicate: u8,
            destination: u8,
            source: u8,
        },
        And {
            destination: u8,
            source: u8,
        },
        MoveImmediate64 {
            general: u8,
            value: u64,
        },
        FloatToInteger {
            wide: bool,
            general: u8,
            xmm: u8,
        },
        SignExtend32 {
            destination: u8,
            source: u8,
        },
        Or {
            destination: u8,
            source: u8,
        },
        ShiftInteger64 {
            left: bool,
            general: u8,
            count: u8,
        },
        CompareImmediate64 {
            general: u8,
            value: u32,
        },
        Test64 {
            general: u8,
        },
        ConditionalJump {
            condition: u8,
            displacement: i32,
        },
        Jump {
            displacement: i32,
        },
        Crash,
        FusedMultiplyAdd {
            double: bool,
            destination: u8,
            multiplicand: u8,
            addend: u8,
        },
    }

    fn encode_instruction(bytes: &mut Vec<u8>, instruction: Instruction) {
        let modrm = |reg: u8, rm: u8| 0xc0 | ((reg & 7) << 3) | (rm & 7);
        let optional_rex = |bytes: &mut Vec<u8>, rex: u8| {
            if rex != 0x40 {
                bytes.push(rex);
            }
        };
        let scalar_prefix = |double: bool| if double { 0xf2 } else { 0xf3 };
        match instruction {
            Instruction::AdjustStack { displacement } => {
                bytes.extend_from_slice(&[0x48, 0x8d, 0x64, 0x24, displacement as u8])
            }
            Instruction::ControlWord { load, displacement } => {
                let extension = if load { 2 } else { 3 };
                bytes.extend_from_slice(&[
                    0x0f,
                    0xae,
                    (extension << 3) | 4 | if displacement == 0 { 0 } else { 0x40 },
                    0x24,
                ]);
                if displacement != 0 {
                    bytes.push(displacement);
                }
            }
            Instruction::StoreControlMode { value } => {
                bytes.extend_from_slice(&[0xc7, 0x44, 0x24, 4]);
                crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
            }
            Instruction::Transfer {
                to_float,
                wide,
                xmm,
                general,
            } => {
                bytes.push(0x66);
                optional_rex(bytes, 0x40 | (u8::from(wide) << 3) | (general >> 3));
                bytes.extend([
                    0x0f,
                    if to_float { 0x6e } else { 0x7e },
                    modrm(xmm, general),
                ]);
            }
            Instruction::Scalar {
                double,
                opcode,
                destination,
                source,
            } => bytes.extend([
                scalar_prefix(double),
                0x0f,
                opcode,
                modrm(destination, source),
            ]),
            Instruction::AllOnes { xmm } => bytes.extend([0x66, 0x0f, 0x76, modrm(xmm, xmm)]),
            Instruction::ShiftLeft { quad, xmm, count } => bytes.extend([
                0x66,
                0x0f,
                if quad { 0x73 } else { 0x72 },
                modrm(6, xmm),
                count,
            ]),
            Instruction::ShiftRight {
                quad,
                arithmetic,
                xmm,
                count,
            } => bytes.extend([
                0x66,
                0x0f,
                if quad { 0x73 } else { 0x72 },
                modrm(if arithmetic { 4 } else { 2 }, xmm),
                count,
            ]),
            Instruction::Xor {
                double,
                destination,
                source,
            } => {
                if double {
                    bytes.push(0x66);
                }
                bytes.extend([0x0f, 0x57, modrm(destination, source)]);
            }
            Instruction::IntegerToFloat {
                double,
                wide,
                xmm,
                general,
            } => {
                bytes.push(scalar_prefix(double));
                optional_rex(bytes, 0x40 | (u8::from(wide) << 3) | (general >> 3));
                bytes.extend([0x0f, 0x2a, modrm(xmm, general)]);
            }
            Instruction::Move32 {
                destination,
                source,
            } => {
                optional_rex(bytes, 0x40 | ((source >> 3) << 2) | (destination >> 3));
                bytes.extend([0x89, modrm(source, destination)]);
            }
            Instruction::MoveAligned {
                destination,
                source,
            } => bytes.extend([0x66, 0x0f, 0x28, modrm(destination, source)]),
            Instruction::CompareScalar {
                double,
                predicate,
                destination,
                source,
            } => bytes.extend([
                scalar_prefix(double),
                0x0f,
                0xc2,
                modrm(destination, source),
                predicate,
            ]),
            Instruction::And {
                destination,
                source,
            } => bytes.extend([0x66, 0x0f, 0x54, modrm(destination, source)]),
            Instruction::MoveImmediate64 { general, value } => {
                bytes.extend([0x48 | (general >> 3), 0xb8 | (general & 7)]);
                bytes.extend(value.to_le_bytes());
            }
            Instruction::FloatToInteger { wide, general, xmm } => {
                bytes.push(0xf2);
                optional_rex(bytes, 0x40 | (u8::from(wide) << 3) | ((general >> 3) << 2));
                bytes.extend([0x0f, 0x2c, modrm(general, xmm)]);
            }
            Instruction::SignExtend32 {
                destination,
                source,
            } => bytes.extend([
                0x48 | ((destination >> 3) << 2) | (source >> 3),
                0x63,
                modrm(destination, source),
            ]),
            Instruction::Or {
                destination,
                source,
            } => bytes.extend([0x66, 0x0f, 0xeb, modrm(destination, source)]),
            Instruction::ShiftInteger64 {
                left,
                general,
                count,
            } => bytes.extend([
                0x48 | (general >> 3),
                0xc1,
                modrm(if left { 4 } else { 5 }, general),
                count,
            ]),
            Instruction::CompareImmediate64 { general, value } => {
                bytes.extend([0x48 | (general >> 3), 0x81, modrm(7, general)]);
                bytes.extend(value.to_le_bytes());
            }
            Instruction::Test64 { general } => bytes.extend([
                0x48 | ((general >> 3) << 2) | (general >> 3),
                0x85,
                modrm(general, general),
            ]),
            Instruction::ConditionalJump {
                condition,
                displacement,
            } => {
                bytes.extend([0x0f, 0x80 | condition]);
                bytes.extend(displacement.to_le_bytes());
            }
            Instruction::Jump { displacement } => {
                bytes.push(0xe9);
                bytes.extend(displacement.to_le_bytes());
            }
            Instruction::Crash => bytes.extend([0x0f, 0x0b]),
            Instruction::FusedMultiplyAdd {
                double,
                destination,
                multiplicand,
                addend,
            } => bytes.extend([
                0xc4,
                0xe2,
                (u8::from(double) << 7) | ((!multiplicand & 0xf) << 3) | 0b01,
                0xa9,
                modrm(destination, addend),
            ]),
        }
    }

    fn decode_instruction(bytes: &[u8]) -> Option<(Instruction, &[u8])> {
        let register_pair =
            |modrm: u8| (modrm & 0xc0 == 0xc0).then_some(((modrm >> 3) & 7, modrm & 7));
        fn optional_rex(bytes: &[u8]) -> (u8, &[u8]) {
            match bytes {
                [rex @ 0x41..=0x4f, rest @ ..] => (*rex, rest),
                _ => (0x40, bytes),
            }
        }
        let float_prefix = |prefix: u8| match prefix {
            0xf2 => Some(true),
            0xf3 => Some(false),
            _ => None,
        };
        match bytes {
            [0x48, 0x8d, 0x64, 0x24, displacement, rest @ ..] => Some((
                Instruction::AdjustStack {
                    displacement: *displacement as i8,
                },
                rest,
            )),
            [0x0f, 0xae, modrm @ (0x14 | 0x1c), 0x24, rest @ ..] => Some((
                Instruction::ControlWord {
                    load: *modrm == 0x14,
                    displacement: 0,
                },
                rest,
            )),
            [
                0x0f,
                0xae,
                modrm @ (0x54 | 0x5c),
                0x24,
                displacement,
                rest @ ..,
            ] => Some((
                Instruction::ControlWord {
                    load: *modrm == 0x54,
                    displacement: *displacement,
                },
                rest,
            )),
            [0xc7, 0x44, 0x24, 4, tail @ ..] if tail.len() >= 4 => {
                let (value, rest) = tail.split_at(4);
                Some((
                    Instruction::StoreControlMode {
                        value: u32::from_le_bytes(value.try_into().ok()?),
                    },
                    rest,
                ))
            }
            [0x0f, 0x0b, rest @ ..] => Some((Instruction::Crash, rest)),
            [0xc4, 0xe2, payload, 0xa9, modrm, rest @ ..] if payload & 0x47 == 0x41 => {
                let (destination, addend) = register_pair(*modrm)?;
                Some((
                    Instruction::FusedMultiplyAdd {
                        double: payload & 0x80 != 0,
                        destination,
                        multiplicand: !(payload >> 3) & 7,
                        addend,
                    },
                    rest,
                ))
            }
            [0x0f, opcode @ 0x80..=0x8f, tail @ ..] if tail.len() >= 4 => {
                let (displacement, rest) = tail.split_at(4);
                Some((
                    Instruction::ConditionalJump {
                        condition: opcode & 15,
                        displacement: i32::from_le_bytes(displacement.try_into().ok()?),
                    },
                    rest,
                ))
            }
            [0xe9, tail @ ..] if tail.len() >= 4 => {
                let (displacement, rest) = tail.split_at(4);
                Some((
                    Instruction::Jump {
                        displacement: i32::from_le_bytes(displacement.try_into().ok()?),
                    },
                    rest,
                ))
            }
            [0x66, 0x0f, 0x76, modrm, rest @ ..] => {
                let (reg, rm) = register_pair(*modrm)?;
                (reg == rm).then_some((Instruction::AllOnes { xmm: reg }, rest))
            }
            [0x66, 0x0f, opcode @ (0x72 | 0x73), modrm, count, rest @ ..] => {
                let (reg, rm) = register_pair(*modrm)?;
                let instruction = match (*opcode, reg) {
                    (_, 6) => Instruction::ShiftLeft {
                        quad: *opcode == 0x73,
                        xmm: rm,
                        count: *count,
                    },
                    (0x72, 4 | 2) | (0x73, 2) => Instruction::ShiftRight {
                        quad: *opcode == 0x73,
                        arithmetic: reg == 4,
                        xmm: rm,
                        count: *count,
                    },
                    _ => return None,
                };
                Some((instruction, rest))
            }
            [0x66, 0x0f, 0x57, modrm, rest @ ..] | [0x0f, 0x57, modrm, rest @ ..] => {
                let (destination, source) = register_pair(*modrm)?;
                Some((
                    Instruction::Xor {
                        double: bytes[0] == 0x66,
                        destination,
                        source,
                    },
                    rest,
                ))
            }
            [0x66, 0x0f, opcode @ (0x28 | 0x54 | 0xeb), modrm, rest @ ..] => {
                let (destination, source) = register_pair(*modrm)?;
                Some((
                    if *opcode == 0x28 {
                        Instruction::MoveAligned {
                            destination,
                            source,
                        }
                    } else if *opcode == 0x54 {
                        Instruction::And {
                            destination,
                            source,
                        }
                    } else {
                        Instruction::Or {
                            destination,
                            source,
                        }
                    },
                    rest,
                ))
            }
            [
                prefix @ (0xf2 | 0xf3),
                0x0f,
                0xc2,
                modrm,
                predicate,
                rest @ ..,
            ] => {
                let (destination, source) = register_pair(*modrm)?;
                Some((
                    Instruction::CompareScalar {
                        double: *prefix == 0xf2,
                        predicate: *predicate,
                        destination,
                        source,
                    },
                    rest,
                ))
            }
            [0x66, tail @ ..] => {
                let (rex, tail) = optional_rex(tail);
                let [0x0f, opcode @ (0x6e | 0x7e), modrm, rest @ ..] = tail else {
                    return None;
                };
                let (xmm, general) = register_pair(*modrm)?;
                (rex & 6 == 0).then_some((
                    Instruction::Transfer {
                        to_float: *opcode == 0x6e,
                        wide: rex & 8 != 0,
                        xmm,
                        general: general | ((rex & 1) << 3),
                    },
                    rest,
                ))
            }
            [prefix @ (0xf2 | 0xf3), tail @ ..] => {
                let double = float_prefix(*prefix)?;
                let (rex, tail) = optional_rex(tail);
                match tail {
                    [0x0f, 0x2a, modrm, rest @ ..] if rex & 6 == 0 => {
                        let (xmm, general) = register_pair(*modrm)?;
                        Some((
                            Instruction::IntegerToFloat {
                                double,
                                wide: rex & 8 != 0,
                                xmm,
                                general: general | ((rex & 1) << 3),
                            },
                            rest,
                        ))
                    }
                    [0x0f, 0x2c, modrm, rest @ ..] if double && rex & 3 == 0 => {
                        let (general, xmm) = register_pair(*modrm)?;
                        Some((
                            Instruction::FloatToInteger {
                                wide: rex & 8 != 0,
                                general: general | (((rex >> 2) & 1) << 3),
                                xmm,
                            },
                            rest,
                        ))
                    }
                    [0x0f, opcode, modrm, rest @ ..] if rex == 0x40 => {
                        let (destination, source) = register_pair(*modrm)?;
                        Some((
                            Instruction::Scalar {
                                double,
                                opcode: *opcode,
                                destination,
                                source,
                            },
                            rest,
                        ))
                    }
                    _ => None,
                }
            }
            _ => {
                let (rex, tail) = optional_rex(bytes);
                let wide = rex & 0xa == 8;
                match tail {
                    [0xc1, modrm, count, rest @ ..]
                        if wide && matches!((modrm >> 3) & 7, 4 | 5) && modrm & 0xc0 == 0xc0 =>
                    {
                        Some((
                            Instruction::ShiftInteger64 {
                                left: (modrm >> 3) & 7 == 4,
                                general: (modrm & 7) | ((rex & 1) << 3),
                                count: *count,
                            },
                            rest,
                        ))
                    }
                    [0x81, modrm, rest @ ..] if wide && modrm & 0xf8 == 0xf8 && rest.len() >= 4 => {
                        let (immediate, rest) = rest.split_at(4);
                        Some((
                            Instruction::CompareImmediate64 {
                                general: (modrm & 7) | ((rex & 1) << 3),
                                value: u32::from_le_bytes(immediate.try_into().ok()?),
                            },
                            rest,
                        ))
                    }
                    [0x85, modrm, rest @ ..] if wide => {
                        let (left, right) = register_pair(*modrm)?;
                        let left = left | (((rex >> 2) & 1) << 3);
                        let right = right | ((rex & 1) << 3);
                        (left == right).then_some((Instruction::Test64 { general: left }, rest))
                    }
                    [0x89, modrm, rest @ ..] if rex & 0xa == 0 => {
                        let (source, destination) = register_pair(*modrm)?;
                        Some((
                            Instruction::Move32 {
                                destination: destination | ((rex & 1) << 3),
                                source: source | (((rex >> 2) & 1) << 3),
                            },
                            rest,
                        ))
                    }
                    [opcode @ 0xb8..=0xbf, rest @ ..]
                        if wide && rex & 4 == 0 && rest.len() >= 8 =>
                    {
                        let (value, rest) = rest.split_at(8);
                        Some((
                            Instruction::MoveImmediate64 {
                                general: (opcode & 7) | ((rex & 1) << 3),
                                value: u64::from_le_bytes(value.try_into().ok()?),
                            },
                            rest,
                        ))
                    }
                    [0x63, modrm, rest @ ..] if wide => {
                        let (destination, source) = register_pair(*modrm)?;
                        Some((
                            Instruction::SignExtend32 {
                                destination: destination | (((rex >> 2) & 1) << 3),
                                source: source | ((rex & 1) << 3),
                            },
                            rest,
                        ))
                    }
                    _ => None,
                }
            }
        }
    }

    fn classification_program(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Vec<Instruction> {
        let double = !form.is_binary32();
        let result = registers[1];
        let restore = Instruction::Transfer {
            to_float: false,
            wide: true,
            xmm: LEFT_SCRATCH,
            general: result,
        };
        let output = |class| Instruction::MoveImmediate64 {
            general: result,
            value: match form.operation {
                IeeeFloatArithmeticOperation::Classify => class,
                IeeeFloatArithmeticOperation::IsNan => u64::from(class == 0),
                IeeeFloatArithmeticOperation::IsInfinite => u64::from(class == 1),
                IeeeFloatArithmeticOperation::IsFinite => u64::from(class >= 2),
                IeeeFloatArithmeticOperation::IsNormal => u64::from(class == 2),
                IeeeFloatArithmeticOperation::IsSubnormal => u64::from(class == 3),
                _ => unreachable!("classification operation"),
            },
        };
        let jump = |condition| Instruction::ConditionalJump {
            condition,
            displacement: 0,
        };
        let mut instructions = vec![
            Instruction::Transfer {
                to_float: true,
                wide: double,
                xmm: LEFT_SCRATCH,
                general: registers[0],
            },
            restore,
            Instruction::ShiftInteger64 {
                left: true,
                general: result,
                count: if double { 1 } else { 33 },
            },
            Instruction::ShiftInteger64 {
                left: false,
                general: result,
                count: if double { 53 } else { 56 },
            },
            Instruction::CompareImmediate64 {
                general: result,
                value: 0,
            },
            jump(4),
            Instruction::CompareImmediate64 {
                general: result,
                value: if double { 2047 } else { 255 },
            },
            jump(4),
            output(2),
            Instruction::Jump { displacement: 0 },
        ];
        let subnormal = instructions.len();
        instructions.extend([
            restore,
            Instruction::ShiftInteger64 {
                left: true,
                general: result,
                count: if double { 1 } else { 33 },
            },
            Instruction::Test64 { general: result },
            jump(4),
            output(3),
            Instruction::Jump { displacement: 0 },
        ]);
        let zero = instructions.len();
        instructions.extend([output(4), Instruction::Jump { displacement: 0 }]);
        let nonfinite = instructions.len();
        instructions.extend([
            restore,
            Instruction::ShiftInteger64 {
                left: true,
                general: result,
                count: if double { 12 } else { 41 },
            },
            Instruction::Test64 { general: result },
            jump(4),
            output(0),
            Instruction::Jump { displacement: 0 },
        ]);
        let infinity = instructions.len();
        instructions.push(output(1));
        let end = instructions.len();
        if form.operation == IeeeFloatArithmeticOperation::Classify {
            instructions.extend([
                Instruction::CompareImmediate64 {
                    general: result,
                    value: 0,
                },
                jump(4),
                Instruction::Transfer {
                    to_float: true,
                    wide: true,
                    xmm: RIGHT_SCRATCH,
                    general: result,
                },
                Instruction::ShiftRight {
                    quad: double,
                    arithmetic: false,
                    xmm: LEFT_SCRATCH,
                    count: if double { 63 } else { 31 },
                },
                Instruction::ShiftLeft {
                    quad: true,
                    xmm: LEFT_SCRATCH,
                    count: 32,
                },
                Instruction::Or {
                    destination: LEFT_SCRATCH,
                    source: RIGHT_SCRATCH,
                },
                restore,
            ]);
        }
        let mut branches = vec![
            (5, subnormal),
            (7, nonfinite),
            (9, end),
            (subnormal + 3, zero),
            (subnormal + 5, end),
            (zero + 1, end),
            (nonfinite + 3, infinity),
            (nonfinite + 5, end),
        ];
        if form.operation == IeeeFloatArithmeticOperation::Classify {
            branches.push((end + 1, instructions.len()));
        }
        let mut offsets = vec![0usize];
        for instruction in &instructions {
            let mut bytes = Vec::new();
            encode_instruction(&mut bytes, *instruction);
            offsets.push(offsets.last().copied().unwrap() + bytes.len());
        }
        for (branch, destination) in branches {
            let displacement = (offsets[destination] as i64 - offsets[branch + 1] as i64) as i32;
            match &mut instructions[branch] {
                Instruction::Jump {
                    displacement: field,
                }
                | Instruction::ConditionalJump {
                    displacement: field,
                    ..
                } => *field = displacement,
                _ => unreachable!("classification branch"),
            }
        }
        instructions
    }

    const fn binary_opcode(operation: IeeeFloatArithmeticOperation) -> Option<u8> {
        match operation {
            IeeeFloatArithmeticOperation::Add => Some(0x58),
            IeeeFloatArithmeticOperation::Multiply => Some(0x59),
            IeeeFloatArithmeticOperation::Subtract => Some(0x5c),
            IeeeFloatArithmeticOperation::Minimum => Some(0x5d),
            IeeeFloatArithmeticOperation::Divide => Some(0x5e),
            IeeeFloatArithmeticOperation::Maximum => Some(0x5f),
            _ => None,
        }
    }

    const fn rounding_control(rounding: IeeeFloatRoundingDirection) -> u32 {
        match rounding {
            IeeeFloatRoundingDirection::TowardZero => 3,
            IeeeFloatRoundingDirection::TowardPositive => 2,
            IeeeFloatRoundingDirection::TowardNegative => 1,
        }
    }

    fn mxcsr_envelope(
        rounding: IeeeFloatRoundingDirection,
        arithmetic: Instruction,
    ) -> [Instruction; 7] {
        [
            Instruction::AdjustStack { displacement: -16 },
            Instruction::ControlWord {
                load: false,
                displacement: 0,
            },
            Instruction::StoreControlMode {
                value: crate::OMEGA_CANONICAL_MXCSR | (rounding_control(rounding) << 13),
            },
            Instruction::ControlWord {
                load: true,
                displacement: 4,
            },
            arithmetic,
            Instruction::ControlWord {
                load: true,
                displacement: 0,
            },
            Instruction::AdjustStack { displacement: 16 },
        ]
    }

    fn program(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Result<Vec<Instruction>, Error> {
        let double = !form.is_binary32();
        let result = registers[form.operand_count()];
        let move_in = |general: u8, xmm: u8, wide: bool| Instruction::Transfer {
            to_float: true,
            wide,
            xmm,
            general,
        };
        let move_out = Instruction::Transfer {
            to_float: false,
            wide: double,
            xmm: LEFT_SCRATCH,
            general: result,
        };
        let in_place = |double: bool, opcode: u8| Instruction::Scalar {
            double,
            opcode,
            destination: LEFT_SCRATCH,
            source: LEFT_SCRATCH,
        };
        Ok(match form.operation {
            IeeeFloatArithmeticOperation::DirectedBinary {
                operation,
                rounding,
            } => {
                let arithmetic = Instruction::Scalar {
                    double,
                    opcode: match operation {
                        IeeeFloatBinaryOperation::Add => 0x58,
                        IeeeFloatBinaryOperation::Subtract => 0x5c,
                        IeeeFloatBinaryOperation::Multiply => 0x59,
                        IeeeFloatBinaryOperation::Divide => 0x5e,
                    },
                    destination: LEFT_SCRATCH,
                    source: RIGHT_SCRATCH,
                };
                let mut program = vec![
                    move_in(registers[0], LEFT_SCRATCH, double),
                    move_in(registers[1], RIGHT_SCRATCH, double),
                ];
                program.extend(mxcsr_envelope(rounding, arithmetic));
                program.push(move_out);
                program
            }
            IeeeFloatArithmeticOperation::DirectedSquareRoot { rounding } => {
                let mut program = vec![move_in(registers[0], LEFT_SCRATCH, double)];
                program.extend(mxcsr_envelope(rounding, in_place(double, 0x51)));
                program.push(move_out);
                program
            }
            IeeeFloatArithmeticOperation::FusedMultiplyAdd => vec![
                move_in(registers[0], LEFT_SCRATCH, double),
                move_in(registers[1], RIGHT_SCRATCH, double),
                move_in(registers[2], ADDEND_SCRATCH, double),
                Instruction::FusedMultiplyAdd {
                    double,
                    destination: LEFT_SCRATCH,
                    multiplicand: RIGHT_SCRATCH,
                    addend: ADDEND_SCRATCH,
                },
                move_out,
            ],
            IeeeFloatArithmeticOperation::IsNan
            | IeeeFloatArithmeticOperation::IsInfinite
            | IeeeFloatArithmeticOperation::IsFinite
            | IeeeFloatArithmeticOperation::IsNormal
            | IeeeFloatArithmeticOperation::IsSubnormal
            | IeeeFloatArithmeticOperation::Classify => classification_program(form, registers),
            IeeeFloatArithmeticOperation::Compare(comparison) => {
                use semantic_vocabulary::IeeeFloatComparisonOperation as Relation;
                let (left, right, predicate) = match comparison {
                    Relation::Equal => (registers[0], registers[1], EQUAL),
                    Relation::NotEqual => (registers[0], registers[1], NOT_EQUAL),
                    Relation::Less => (registers[0], registers[1], LESS),
                    Relation::LessOrEqual => (registers[0], registers[1], LESS_OR_EQUAL),
                    Relation::Greater => (registers[1], registers[0], LESS),
                    Relation::GreaterOrEqual => (registers[1], registers[0], LESS_OR_EQUAL),
                };
                vec![
                    move_in(left, LEFT_SCRATCH, double),
                    move_in(right, RIGHT_SCRATCH, double),
                    Instruction::CompareScalar {
                        double,
                        predicate,
                        destination: LEFT_SCRATCH,
                        source: RIGHT_SCRATCH,
                    },
                    Instruction::Transfer {
                        to_float: false,
                        wide: double,
                        xmm: LEFT_SCRATCH,
                        general: result,
                    },
                    Instruction::ShiftInteger64 {
                        left: false,
                        general: result,
                        count: if double { 63 } else { 31 },
                    },
                ]
            }
            IeeeFloatArithmeticOperation::MultiplyThenAdd => vec![
                move_in(registers[0], LEFT_SCRATCH, double),
                move_in(registers[1], RIGHT_SCRATCH, double),
                Instruction::Scalar {
                    double,
                    opcode: 0x59,
                    destination: LEFT_SCRATCH,
                    source: RIGHT_SCRATCH,
                },
                move_in(registers[2], ADDEND_SCRATCH, double),
                Instruction::Scalar {
                    double,
                    opcode: 0x58,
                    destination: LEFT_SCRATCH,
                    source: ADDEND_SCRATCH,
                },
                move_out,
            ],
            IeeeFloatArithmeticOperation::Negate => vec![
                move_in(registers[0], LEFT_SCRATCH, double),
                Instruction::AllOnes { xmm: RIGHT_SCRATCH },
                Instruction::ShiftLeft {
                    quad: double,
                    xmm: RIGHT_SCRATCH,
                    count: if double { 63 } else { 31 },
                },
                Instruction::Xor {
                    double,
                    destination: LEFT_SCRATCH,
                    source: RIGHT_SCRATCH,
                },
                move_out,
            ],
            IeeeFloatArithmeticOperation::SquareRoot => vec![
                move_in(registers[0], LEFT_SCRATCH, double),
                in_place(double, 0x51),
                move_out,
            ],
            IeeeFloatArithmeticOperation::ConvertFormat => vec![
                move_in(registers[0], LEFT_SCRATCH, !double),
                in_place(!double, 0x5a),
                move_out,
            ],
            IeeeFloatArithmeticOperation::TruncateToInteger {
                target,
                domain: IeeeFloatToIntegerDomain::Trapping,
            } => conversions::trapping(double, target, registers[0], result),
            IeeeFloatArithmeticOperation::TruncateToInteger { target, .. } => {
                truncation(double, target, registers[0], result)
            }
            IeeeFloatArithmeticOperation::FromInteger(IeeeFloatIntegerCarrier::U64) => {
                conversions::from_unsigned64(double, registers[0], result)
            }
            IeeeFloatArithmeticOperation::FromInteger(source) if source.bits() < 32 => {
                let shift = (32 - source.bits()) as u8;
                let mut program = vec![
                    move_in(registers[0], LEFT_SCRATCH, false),
                    Instruction::ShiftLeft {
                        quad: false,
                        xmm: LEFT_SCRATCH,
                        count: shift,
                    },
                    Instruction::ShiftRight {
                        quad: false,
                        arithmetic: source.is_signed(),
                        xmm: LEFT_SCRATCH,
                        count: shift,
                    },
                    in_place(false, 0xe6),
                ];
                if !double {
                    program.push(in_place(true, 0x5a));
                }
                program.push(move_out);
                program
            }
            IeeeFloatArithmeticOperation::FromInteger(source) if !source.is_signed() => vec![
                Instruction::Move32 {
                    destination: result,
                    source: registers[0],
                },
                Instruction::IntegerToFloat {
                    double,
                    wide: true,
                    xmm: LEFT_SCRATCH,
                    general: result,
                },
                move_out,
            ],
            IeeeFloatArithmeticOperation::FromInteger(source) => vec![
                Instruction::IntegerToFloat {
                    double,
                    wide: source.bits() == 64,
                    xmm: LEFT_SCRATCH,
                    general: registers[0],
                },
                move_out,
            ],
            binary => vec![
                move_in(registers[0], LEFT_SCRATCH, double),
                move_in(registers[1], RIGHT_SCRATCH, double),
                Instruction::Scalar {
                    double,
                    opcode: binary_opcode(binary).ok_or(Error::EncodedFormMismatch)?,
                    destination: LEFT_SCRATCH,
                    source: RIGHT_SCRATCH,
                },
                move_out,
            ],
        })
    }

    const TWO_TO_31: u64 = 0x41e0_0000_0000_0000;
    const TWO_TO_63: u64 = 0x43e0_0000_0000_0000;
    const U32_MAXIMUM: u64 = 0x41ef_ffff_ffe0_0000;
    const EQUAL: u8 = 0;
    const LESS: u8 = 1;
    const LESS_OR_EQUAL: u8 = 2;
    const NOT_EQUAL: u8 = 4;
    const ORDERED: u8 = 7;

    fn truncation(
        double: bool,
        target: IeeeFloatIntegerCarrier,
        source: u8,
        result: u8,
    ) -> Vec<Instruction> {
        let left = LEFT_SCRATCH;
        let mut program = vec![Instruction::Transfer {
            to_float: true,
            wide: double,
            xmm: left,
            general: source,
        }];
        if !double {
            program.push(Instruction::Scalar {
                double: false,
                opcode: 0x5a,
                destination: left,
                source: left,
            });
        }
        program.extend(truncation_landed(target, result));
        program
    }

    fn truncation_landed(target: IeeeFloatIntegerCarrier, result: u8) -> Vec<Instruction> {
        if target == IeeeFloatIntegerCarrier::U64 {
            return conversions::to_unsigned64(result);
        }
        let (left, right) = (LEFT_SCRATCH, RIGHT_SCRATCH);
        let mut program = vec![
            Instruction::MoveAligned {
                destination: right,
                source: left,
            },
            Instruction::CompareScalar {
                double: true,
                predicate: ORDERED,
                destination: right,
                source: left,
            },
            Instruction::And {
                destination: left,
                source: right,
            },
        ];
        let bound = |value: u64| {
            [
                Instruction::MoveImmediate64 {
                    general: result,
                    value,
                },
                Instruction::Transfer {
                    to_float: true,
                    wide: true,
                    xmm: right,
                    general: result,
                },
            ]
        };
        if !target.is_signed() || target.bits() < 32 {
            let (minimum, maximum) = match target {
                IeeeFloatIntegerCarrier::I8 => (Some(0xc060_0000_0000_0000), 0x405f_c000_0000_0000),
                IeeeFloatIntegerCarrier::I16 => {
                    (Some(0xc0e0_0000_0000_0000), 0x40df_ffc0_0000_0000)
                }
                IeeeFloatIntegerCarrier::U8 => (None, 0x406f_e000_0000_0000),
                IeeeFloatIntegerCarrier::U16 => (None, 0x40ef_ffe0_0000_0000),
                _ => (None, U32_MAXIMUM),
            };
            match minimum {
                None => program.push(Instruction::Xor {
                    double: true,
                    destination: right,
                    source: right,
                }),
                Some(minimum) => program.extend(bound(minimum)),
            }
            program.push(Instruction::Scalar {
                double: true,
                opcode: 0x5f,
                destination: left,
                source: right,
            });
            program.extend(bound(maximum));
            program.extend([
                Instruction::Scalar {
                    double: true,
                    opcode: 0x5d,
                    destination: left,
                    source: right,
                },
                Instruction::FloatToInteger {
                    wide: target.bits() == 32,
                    general: result,
                    xmm: left,
                },
            ]);
            if target.is_signed() {
                program.push(Instruction::SignExtend32 {
                    destination: result,
                    source: result,
                });
            }
            return program;
        }
        let wide = target.bits() == 64;
        program.extend(bound(if wide { TWO_TO_63 } else { TWO_TO_31 }));
        program.extend([
            Instruction::CompareScalar {
                double: true,
                predicate: LESS_OR_EQUAL,
                destination: right,
                source: left,
            },
            Instruction::FloatToInteger {
                wide,
                general: result,
                xmm: left,
            },
            Instruction::Transfer {
                to_float: true,
                wide,
                xmm: left,
                general: result,
            },
            Instruction::Xor {
                double: true,
                destination: left,
                source: right,
            },
            Instruction::Transfer {
                to_float: false,
                wide,
                xmm: left,
                general: result,
            },
        ]);
        if !wide {
            program.push(Instruction::SignExtend32 {
                destination: result,
                source: result,
            });
        }
        program
    }

    fn canonical(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::with_capacity(24);
        for instruction in program(form, registers)? {
            encode_instruction(&mut bytes, instruction);
        }
        Ok(bytes)
    }

    fn registers(
        physical: &ValidatedPhysicalRegisterModel,
        form: IeeeFloatArithmeticForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, Error> {
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        if alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
                variant: 0,
            })
        {
            return Err(Error::AlternativeMismatch);
        }
        if operands.len() != form.operand_count() + 1 {
            return Err(Error::OperandCountMismatch);
        }
        resolve_registers(physical, operands)
    }

    pub(crate) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        form: IeeeFloatArithmeticForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let registers = registers(physical, form, alternative, operands)?;
        let bytes = canonical(form, &registers)?;
        validate(physical, form, alternative, operands, &bytes)
    }

    pub(crate) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        form: IeeeFloatArithmeticForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let registers = registers(physical, form, alternative, operands)?;
        let mut decoded = Vec::new();
        let mut rest = bytes;
        while !rest.is_empty() {
            let (instruction, tail) = decode_instruction(rest).ok_or(Error::MalformedEncoding)?;
            decoded.push(instruction);
            rest = tail;
        }
        if decoded != program(form, &registers)? || bytes != canonical(form, &registers)? {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: footprint(form, operands),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::{encode, validate};
        use selected_instructions::{
            IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatFormat,
            IeeeFloatIntegerCarrier, MachineAlternativeFamily, MachineAlternativeKey,
        };

        fn key(form: IeeeFloatArithmeticForm) -> MachineAlternativeKey {
            MachineAlternativeKey {
                family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
                variant: 0,
            }
        }

        fn bytes_of(
            operation: IeeeFloatArithmeticOperation,
            format: IeeeFloatFormat,
            registers: &[&str],
        ) -> Vec<u8> {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let operands = registers
                .iter()
                .map(|name| physical.model().view_named(name).unwrap().id)
                .collect::<Vec<_>>();
            let form = IeeeFloatArithmeticForm { operation, format };
            encode(&physical, form, key(form), &operands)
                .unwrap()
                .bytes()
                .to_vec()
        }

        #[test]
        fn directed_division_restores_controls_and_balances_private_storage() {
            for (operation, expected_mode) in [(54, 0x7f80u32), (55, 0x5f80), (56, 0x3f80)] {
                for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                    let form = IeeeFloatArithmeticForm {
                        operation: IeeeFloatArithmeticOperation::ALL[operation],
                        format,
                    };
                    let program = super::program(form, &[0, 1, 0]).unwrap();
                    assert_eq!(
                        program[2],
                        super::Instruction::AdjustStack { displacement: -16 }
                    );
                    assert_eq!(
                        program[3],
                        super::Instruction::ControlWord {
                            load: false,
                            displacement: 0
                        }
                    );
                    assert_eq!(
                        program[4],
                        super::Instruction::StoreControlMode {
                            value: expected_mode
                        }
                    );
                    assert_eq!(
                        program[7],
                        super::Instruction::ControlWord {
                            load: true,
                            displacement: 0
                        }
                    );
                    assert_eq!(
                        program[8],
                        super::Instruction::AdjustStack { displacement: 16 }
                    );
                    let encoded = super::effects(form);
                    assert!(matches!(
                        encoded.stack,
                        selected_instructions::MachineEncodedStackEffect::TemporaryBytesV1 {
                            byte_count: 16,
                            ..
                        }
                    ));
                    assert!(matches!(
                    encoded.memory,
                    selected_instructions::MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                        byte_count: 16,
                        ..
                    }
                ));
                }
            }
        }

        #[test]
        fn directed_square_root_uses_sqrt_opcode_under_the_private_rounding_envelope() {
            for (operation, expected_mode) in [(67, 0x7f80u32), (68, 0x5f80), (69, 0x3f80)] {
                for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                    let form = IeeeFloatArithmeticForm {
                        operation: IeeeFloatArithmeticOperation::ALL[operation],
                        format,
                    };
                    let program = super::program(form, &[0, 0]).unwrap();
                    assert_eq!(
                        program[1],
                        super::Instruction::AdjustStack { displacement: -16 }
                    );
                    assert_eq!(
                        program[3],
                        super::Instruction::StoreControlMode {
                            value: expected_mode
                        }
                    );
                    assert_eq!(
                        program[5],
                        super::Instruction::Scalar {
                            double: !form.is_binary32(),
                            opcode: 0x51,
                            destination: super::LEFT_SCRATCH,
                            source: super::LEFT_SCRATCH,
                        }
                    );
                    assert_eq!(
                        program[7],
                        super::Instruction::AdjustStack { displacement: 16 }
                    );
                    let encoded = super::effects(form);
                    assert!(matches!(
                        encoded.stack,
                        selected_instructions::MachineEncodedStackEffect::TemporaryBytesV1 {
                            byte_count: 16,
                            ..
                        }
                    ));
                }
            }
        }

        #[test]
        fn fused_multiply_add_encodes_one_vfmadd213_and_rejects_two_roundings() {
            use IeeeFloatFormat::{Binary32, Binary64};
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let operands = ["rax", "rcx", "rdx", "rax"]
                .map(|name| physical.model().view_named(name).unwrap().id);
            for (format, expected) in [
                (
                    Binary32,
                    &[
                        0x66, 0x0f, 0x6e, 0xe0, 0x66, 0x0f, 0x6e, 0xe9, 0x66, 0x0f, 0x6e, 0xda,
                        0xc4, 0xe2, 0x51, 0xa9, 0xe3, 0x66, 0x0f, 0x7e, 0xe0,
                    ][..],
                ),
                (
                    Binary64,
                    &[
                        0x66, 0x48, 0x0f, 0x6e, 0xe0, 0x66, 0x48, 0x0f, 0x6e, 0xe9, 0x66, 0x48,
                        0x0f, 0x6e, 0xda, 0xc4, 0xe2, 0xd1, 0xa9, 0xe3, 0x66, 0x48, 0x0f, 0x7e,
                        0xe0,
                    ][..],
                ),
            ] {
                let form = IeeeFloatArithmeticForm {
                    operation: IeeeFloatArithmeticOperation::FusedMultiplyAdd,
                    format,
                };
                assert_eq!(
                    bytes_of(form.operation, format, &["rax", "rcx", "rdx", "rax"]),
                    expected
                );
                let kind =
                    selected_instructions::SelectedInstructionKind::NearestIeeeFloatArithmetic {
                        form,
                    };
                let unfused = bytes_of(
                    IeeeFloatArithmeticOperation::MultiplyThenAdd,
                    format,
                    &["rax", "rcx", "rdx", "rax"],
                );
                let mut substituted_opcode = expected.to_vec();
                substituted_opcode[expected.len() - 8] = 0x99;
                let mut substituted_format = expected.to_vec();
                substituted_format[expected.len() - 9] ^= 0x80;
                for bytes in [
                    &[][..],
                    unfused.as_slice(),
                    substituted_opcode.as_slice(),
                    substituted_format.as_slice(),
                ] {
                    assert!(
                        crate::validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            key(form),
                            &operands,
                            bytes,
                        )
                        .is_err()
                    );
                }
                let xmm3 = crate::register_model::units("xmm3");
                assert!(
                    xmm3.iter()
                        .all(|unit| super::effects(form).implicit_unit_clobbers.contains(unit))
                );
                let binary_form = IeeeFloatArithmeticForm {
                    operation: IeeeFloatArithmeticOperation::Add,
                    format,
                };
                assert!(!xmm3.iter().any(|unit| {
                    super::effects(binary_form)
                        .implicit_unit_clobbers
                        .contains(unit)
                }));
            }
        }

        #[test]
        fn encodings_match_the_architectural_bytes() {
            use IeeeFloatArithmeticOperation as Operation;
            use IeeeFloatFormat::{Binary32, Binary64};
            assert_eq!(
                bytes_of(Operation::Add, Binary64, &["rcx", "r9", "rax"]),
                [
                    0x66, 0x48, 0x0f, 0x6e, 0xe1, 0x66, 0x49, 0x0f, 0x6e, 0xe9, 0xf2, 0x0f, 0x58,
                    0xe5, 0x66, 0x48, 0x0f, 0x7e, 0xe0,
                ]
            );
            assert_eq!(
                bytes_of(Operation::Divide, Binary32, &["rax", "rdx", "r10"]),
                [
                    0x66, 0x0f, 0x6e, 0xe0, 0x66, 0x0f, 0x6e, 0xea, 0xf3, 0x0f, 0x5e, 0xe5, 0x66,
                    0x41, 0x0f, 0x7e, 0xe2,
                ]
            );
            assert_eq!(
                bytes_of(Operation::Minimum, Binary64, &["rcx", "rdx", "rax"])[10..14],
                [0xf2, 0x0f, 0x5d, 0xe5]
            );
            assert_eq!(
                bytes_of(Operation::Maximum, Binary32, &["rcx", "rdx", "rax"])[8..12],
                [0xf3, 0x0f, 0x5f, 0xe5]
            );
            assert_eq!(
                bytes_of(Operation::Negate, Binary64, &["rsi", "rdi"]),
                [
                    0x66, 0x48, 0x0f, 0x6e, 0xe6, 0x66, 0x0f, 0x76, 0xed, 0x66, 0x0f, 0x73, 0xf5,
                    0x3f, 0x66, 0x0f, 0x57, 0xe5, 0x66, 0x48, 0x0f, 0x7e, 0xe7,
                ]
            );
            assert_eq!(
                bytes_of(Operation::Negate, Binary32, &["rsi", "rdi"]),
                [
                    0x66, 0x0f, 0x6e, 0xe6, 0x66, 0x0f, 0x76, 0xed, 0x66, 0x0f, 0x72, 0xf5, 0x1f,
                    0x0f, 0x57, 0xe5, 0x66, 0x0f, 0x7e, 0xe7,
                ]
            );
            assert_eq!(
                bytes_of(Operation::SquareRoot, Binary64, &["r8", "rax"]),
                [
                    0x66, 0x49, 0x0f, 0x6e, 0xe0, 0xf2, 0x0f, 0x51, 0xe4, 0x66, 0x48, 0x0f, 0x7e,
                    0xe0,
                ]
            );
            assert_eq!(
                bytes_of(Operation::ConvertFormat, Binary64, &["rcx", "rax"]),
                [
                    0x66, 0x0f, 0x6e, 0xe1, 0xf3, 0x0f, 0x5a, 0xe4, 0x66, 0x48, 0x0f, 0x7e, 0xe0,
                ]
            );
            assert_eq!(
                bytes_of(Operation::ConvertFormat, Binary32, &["rcx", "rax"]),
                [
                    0x66, 0x48, 0x0f, 0x6e, 0xe1, 0xf2, 0x0f, 0x5a, 0xe4, 0x66, 0x0f, 0x7e, 0xe0,
                ]
            );
            assert_eq!(
                bytes_of(
                    Operation::FromInteger(IeeeFloatIntegerCarrier::I32),
                    Binary64,
                    &["r9", "rax"]
                ),
                [0xf2, 0x41, 0x0f, 0x2a, 0xe1, 0x66, 0x48, 0x0f, 0x7e, 0xe0]
            );
            assert_eq!(
                bytes_of(
                    Operation::FromInteger(IeeeFloatIntegerCarrier::I64),
                    Binary32,
                    &["rcx", "rax"]
                ),
                [0xf3, 0x48, 0x0f, 0x2a, 0xe1, 0x66, 0x0f, 0x7e, 0xe0]
            );
            assert_eq!(
                bytes_of(
                    Operation::FromInteger(IeeeFloatIntegerCarrier::U32),
                    Binary64,
                    &["rcx", "r11"]
                ),
                [
                    0x41, 0x89, 0xcb, 0xf2, 0x49, 0x0f, 0x2a, 0xe3, 0x66, 0x49, 0x0f, 0x7e, 0xe3,
                ]
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
                        TruncateToInteger { target: left_target, domain: left_domain },
                        TruncateToInteger { target: right_target, domain: right_domain },
                    ) if left_target == right_target && left.format == right.format
                        && left_domain != selected_instructions::IeeeFloatToIntegerDomain::Trapping
                        && right_domain != selected_instructions::IeeeFloatToIntegerDomain::Trapping
                )
        }

        #[test]
        fn truncation_encodings_match_the_architectural_bytes() {
            use IeeeFloatArithmeticOperation::TruncateToInteger;
            use selected_instructions::IeeeFloatToIntegerDomain::Saturating;
            let truncate = |target| TruncateToInteger {
                target,
                domain: Saturating,
            };
            assert_eq!(
                bytes_of(
                    truncate(IeeeFloatIntegerCarrier::I32),
                    IeeeFloatFormat::Binary64,
                    &["rcx", "rax"]
                ),
                [
                    0x66, 0x48, 0x0f, 0x6e, 0xe1, 0x66, 0x0f, 0x28, 0xec, 0xf2, 0x0f, 0xc2, 0xec,
                    0x07, 0x66, 0x0f, 0x54, 0xe5, 0x48, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0xe0, 0x41, 0x66, 0x48, 0x0f, 0x6e, 0xe8, 0xf2, 0x0f, 0xc2, 0xec, 0x02, 0xf2,
                    0x0f, 0x2c, 0xc4, 0x66, 0x0f, 0x6e, 0xe0, 0x66, 0x0f, 0x57, 0xe5, 0x66, 0x0f,
                    0x7e, 0xe0, 0x48, 0x63, 0xc0,
                ]
            );
            let widened = bytes_of(
                truncate(IeeeFloatIntegerCarrier::I64),
                IeeeFloatFormat::Binary32,
                &["rcx", "r9"],
            );
            assert_eq!(
                widened[..8],
                [0x66, 0x0f, 0x6e, 0xe1, 0xf3, 0x0f, 0x5a, 0xe4]
            );
            assert!(
                widened
                    .windows(10)
                    .any(|window| window == [0x49, 0xb9, 0, 0, 0, 0, 0, 0, 0xe0, 0x43])
            );
            assert!(
                widened
                    .windows(5)
                    .any(|window| window == [0xf2, 0x4c, 0x0f, 0x2c, 0xcc])
            );
            let unsigned = bytes_of(
                truncate(IeeeFloatIntegerCarrier::U32),
                IeeeFloatFormat::Binary64,
                &["rcx", "r9"],
            );
            assert!(unsigned.windows(10).any(
                |window| window == [0x49, 0xb9, 0x00, 0x00, 0xe0, 0xff, 0xff, 0xff, 0xef, 0x41]
            ));
            assert!(unsigned.ends_with(&[0xf2, 0x0f, 0x5d, 0xe5, 0xf2, 0x4c, 0x0f, 0x2c, 0xcc]));
        }

        #[test]
        fn narrow_carrier_encodings_match_the_architectural_bytes() {
            use IeeeFloatArithmeticOperation::FromInteger;
            assert_eq!(
                bytes_of(
                    FromInteger(IeeeFloatIntegerCarrier::I8),
                    IeeeFloatFormat::Binary64,
                    &["rcx", "rax"]
                ),
                [
                    0x66, 0x0f, 0x6e, 0xe1, 0x66, 0x0f, 0x72, 0xf4, 0x18, 0x66, 0x0f, 0x72, 0xe4,
                    0x18, 0xf3, 0x0f, 0xe6, 0xe4, 0x66, 0x48, 0x0f, 0x7e, 0xe0,
                ]
            );
            let unsigned = bytes_of(
                FromInteger(IeeeFloatIntegerCarrier::U16),
                IeeeFloatFormat::Binary32,
                &["rcx", "rax"],
            );
            assert_eq!(unsigned[9..14], [0x66, 0x0f, 0x72, 0xd4, 0x10]);
            assert_eq!(unsigned[18..22], [0xf2, 0x0f, 0x5a, 0xe4]);
        }

        #[test]
        fn raw_bit_classification_encodings_match_independently_assembled_bytes() {
            for (operation, format, hexadecimal) in [
                (
                    IeeeFloatArithmeticOperation::IsNan,
                    IeeeFloatFormat::Binary32,
                    "660f6ee766480f7ee048c1e02148c1e8384881f8000000000f841c0000004881f8ff0000000f843f00000048b80000000000000000e95b00000066480f7ee048c1e0214885c00f840f00000048b80000000000000000e93a00000048b80000000000000000e92b00000066480f7ee048c1e0294885c00f840f00000048b80100000000000000e90a00000048b80000000000000000",
                ),
                (
                    IeeeFloatArithmeticOperation::IsNan,
                    IeeeFloatFormat::Binary64,
                    "66480f6ee766480f7ee048c1e00148c1e8354881f8000000000f841c0000004881f8ff0700000f843f00000048b80000000000000000e95b00000066480f7ee048c1e0014885c00f840f00000048b80000000000000000e93a00000048b80000000000000000e92b00000066480f7ee048c1e00c4885c00f840f00000048b80100000000000000e90a00000048b80000000000000000",
                ),
                (
                    IeeeFloatArithmeticOperation::IsInfinite,
                    IeeeFloatFormat::Binary32,
                    "660f6ee766480f7ee048c1e02148c1e8384881f8000000000f841c0000004881f8ff0000000f843f00000048b80000000000000000e95b00000066480f7ee048c1e0214885c00f840f00000048b80000000000000000e93a00000048b80000000000000000e92b00000066480f7ee048c1e0294885c00f840f00000048b80000000000000000e90a00000048b80100000000000000",
                ),
                (
                    IeeeFloatArithmeticOperation::IsInfinite,
                    IeeeFloatFormat::Binary64,
                    "66480f6ee766480f7ee048c1e00148c1e8354881f8000000000f841c0000004881f8ff0700000f843f00000048b80000000000000000e95b00000066480f7ee048c1e0014885c00f840f00000048b80000000000000000e93a00000048b80000000000000000e92b00000066480f7ee048c1e00c4885c00f840f00000048b80000000000000000e90a00000048b80100000000000000",
                ),
                (
                    IeeeFloatArithmeticOperation::IsFinite,
                    IeeeFloatFormat::Binary32,
                    "660f6ee766480f7ee048c1e02148c1e8384881f8000000000f841c0000004881f8ff0000000f843f00000048b80100000000000000e95b00000066480f7ee048c1e0214885c00f840f00000048b80100000000000000e93a00000048b80100000000000000e92b00000066480f7ee048c1e0294885c00f840f00000048b80000000000000000e90a00000048b80000000000000000",
                ),
                (
                    IeeeFloatArithmeticOperation::IsFinite,
                    IeeeFloatFormat::Binary64,
                    "66480f6ee766480f7ee048c1e00148c1e8354881f8000000000f841c0000004881f8ff0700000f843f00000048b80100000000000000e95b00000066480f7ee048c1e0014885c00f840f00000048b80100000000000000e93a00000048b80100000000000000e92b00000066480f7ee048c1e00c4885c00f840f00000048b80000000000000000e90a00000048b80000000000000000",
                ),
                (
                    IeeeFloatArithmeticOperation::IsNormal,
                    IeeeFloatFormat::Binary32,
                    "660f6ee766480f7ee048c1e02148c1e8384881f8000000000f841c0000004881f8ff0000000f843f00000048b80100000000000000e95b00000066480f7ee048c1e0214885c00f840f00000048b80000000000000000e93a00000048b80000000000000000e92b00000066480f7ee048c1e0294885c00f840f00000048b80000000000000000e90a00000048b80000000000000000",
                ),
                (
                    IeeeFloatArithmeticOperation::IsNormal,
                    IeeeFloatFormat::Binary64,
                    "66480f6ee766480f7ee048c1e00148c1e8354881f8000000000f841c0000004881f8ff0700000f843f00000048b80100000000000000e95b00000066480f7ee048c1e0014885c00f840f00000048b80000000000000000e93a00000048b80000000000000000e92b00000066480f7ee048c1e00c4885c00f840f00000048b80000000000000000e90a00000048b80000000000000000",
                ),
                (
                    IeeeFloatArithmeticOperation::IsSubnormal,
                    IeeeFloatFormat::Binary32,
                    "660f6ee766480f7ee048c1e02148c1e8384881f8000000000f841c0000004881f8ff0000000f843f00000048b80000000000000000e95b00000066480f7ee048c1e0214885c00f840f00000048b80100000000000000e93a00000048b80000000000000000e92b00000066480f7ee048c1e0294885c00f840f00000048b80000000000000000e90a00000048b80000000000000000",
                ),
                (
                    IeeeFloatArithmeticOperation::IsSubnormal,
                    IeeeFloatFormat::Binary64,
                    "66480f6ee766480f7ee048c1e00148c1e8354881f8000000000f841c0000004881f8ff0700000f843f00000048b80000000000000000e95b00000066480f7ee048c1e0014885c00f840f00000048b80100000000000000e93a00000048b80000000000000000e92b00000066480f7ee048c1e00c4885c00f840f00000048b80000000000000000e90a00000048b80000000000000000",
                ),
                (
                    IeeeFloatArithmeticOperation::Classify,
                    IeeeFloatFormat::Binary32,
                    "660f6ee766480f7ee048c1e02148c1e8384881f8000000000f841c0000004881f8ff0000000f843f00000048b80200000000000000e95b00000066480f7ee048c1e0214885c00f840f00000048b80300000000000000e93a00000048b80400000000000000e92b00000066480f7ee048c1e0294885c00f840f00000048b80000000000000000e90a00000048b801000000000000004881f8000000000f841800000066480f6ee8660f72d41f660f73f420660febe566480f7ee0",
                ),
                (
                    IeeeFloatArithmeticOperation::Classify,
                    IeeeFloatFormat::Binary64,
                    "66480f6ee766480f7ee048c1e00148c1e8354881f8000000000f841c0000004881f8ff0700000f843f00000048b80200000000000000e95b00000066480f7ee048c1e0014885c00f840f00000048b80300000000000000e93a00000048b80400000000000000e92b00000066480f7ee048c1e00c4885c00f840f00000048b80000000000000000e90a00000048b801000000000000004881f8000000000f841800000066480f6ee8660f73d43f660f73f420660febe566480f7ee0",
                ),
            ] {
                let expected = hexadecimal
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(bytes_of(operation, format, &["rdi", "rax"]), expected);
            }
        }

        #[test]
        fn validation_binds_every_bit_operation_format_and_register() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let view = |name: &str| physical.model().view_named(name).unwrap().id;
            let all = [view("rdx"), view("rsi"), view("r15"), view("r8")];
            for form in IeeeFloatArithmeticForm::ALL {
                let operands = &all[4 - form.operand_count() - 1..];
                let bytes = encode(&physical, form, key(form), operands)
                    .unwrap()
                    .bytes()
                    .to_vec();
                let super::MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes,
                    maximum_bytes: Some(maximum_bytes),
                } = super::size(form).unwrap()
                else {
                    panic!("scalar SSE2 forms declare bounded encoded sizes");
                };
                assert!(
                    (usize::from(minimum_bytes)..=usize::from(maximum_bytes))
                        .contains(&bytes.len())
                );
                for byte_index in 0..bytes.len() {
                    for bit in 0..8 {
                        let mut changed = bytes.clone();
                        changed[byte_index] ^= 1 << bit;
                        assert!(
                            validate(&physical, form, key(form), operands, &changed).is_err(),
                            "{form:?} accepted byte {byte_index} bit {bit}"
                        );
                    }
                }
                for other in IeeeFloatArithmeticForm::ALL {
                    if other != form && other.operand_count() == form.operand_count() {
                        if !shares_realization(other, form) {
                            assert!(
                                validate(&physical, other, key(other), operands, &bytes).is_err()
                            );
                        }
                        assert!(validate(&physical, form, key(other), operands, &bytes).is_err());
                    }
                }
                let mut swapped = operands.to_vec();
                swapped.swap(0, operands.len() - 1);
                assert!(validate(&physical, form, key(form), &swapped, &bytes).is_err());
                assert!(validate(&physical, form, key(form), operands, &bytes[1..]).is_err());
                assert!(validate(&physical, form, key(form), &all[..1], &bytes).is_err());
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
            MachineSizeKnowledge::ExactBytes(3),
            MachineEncodedEffects::catalog_fence_v1(),
        )
    }

    pub(crate) fn encode(kind: MemoryFenceKind) -> Option<Vec<u8>> {
        let opcode = match kind {
            MemoryFenceKind::Load => 0xe8,
            MemoryFenceKind::Store => 0xf8,
            MemoryFenceKind::Full => 0xf0,
            _ => return None,
        };
        Some(vec![0x0f, 0xae, opcode])
    }

    pub(crate) fn decode(bytes: &[u8]) -> Option<MemoryFenceKind> {
        match bytes {
            [0x0f, 0xae, 0xe8] => Some(MemoryFenceKind::Load),
            [0x0f, 0xae, 0xf8] => Some(MemoryFenceKind::Store),
            [0x0f, 0xae, 0xf0] => Some(MemoryFenceKind::Full),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use selected_instructions::{
            MachineAlternativeKey, RegisterOperation, SelectedInstructionKind,
        };
        #[test]
        fn catalog_fences_keep_exact_bytes_and_reject_other_effects() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::MemoryFence,
                variant: 0,
            };
            for (kind, expected) in [
                (MemoryFenceKind::Load, &[0x0f, 0xae, 0xe8][..]),
                (MemoryFenceKind::Store, &[0x0f, 0xae, 0xf8][..]),
                (MemoryFenceKind::Full, &[0x0f, 0xae, 0xf0][..]),
            ] {
                let instruction = SelectedInstructionKind::RegisterAccess {
                    operation: RegisterOperation::MemoryFence(kind),
                };
                let realized =
                    crate::encode_x86_64_selected_form(&physical, instruction, alternative, &[])
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
                assert!(!realized.footprint().writes_rflags);
                for bit in 0..expected.len() * 8 {
                    let mut changed = expected.to_vec();
                    changed[bit / 8] ^= 1 << (bit % 8);
                    assert!(
                        crate::validate_x86_64_selected_form_encoding(
                            &physical,
                            instruction,
                            alternative,
                            &[],
                            &changed
                        )
                        .is_err(),
                        "changed bit {bit}"
                    );
                }
                let mut doubled = expected.to_vec();
                doubled.extend(expected);
                for changed in [&doubled[..], &expected[..expected.len() - 1], &[]] {
                    assert!(
                        crate::validate_x86_64_selected_form_encoding(
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
                    operation: RegisterOperation::MemoryFence(MemoryFenceKind::Aarch64Full),
                };
                assert!(
                    crate::encode_x86_64_selected_form(&physical, wrong_kind, alternative, &[])
                        .is_err()
                );
                assert!(
                    crate::encode_x86_64_selected_form(
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
                let operand = physical.model().view_named("rax").unwrap().id;
                assert!(
                    crate::encode_x86_64_selected_form(
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
pub(crate) mod port_write {
    use super::{ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError};
    use crate::register_model::units_of;
    use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineEffectDeclaration,
        MachineEncodedEffects, MachineEncodedTrapBehavior, MachineMemoryEffect,
        MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior, SelectedInstructionKind,
    };

    pub(crate) fn effects() -> MachineEncodedEffects {
        MachineEncodedEffects {
            implicit_unit_clobbers: units_of(&["rax", "rdx", "r10", "r11"]),
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            ..MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new())
        }
    }

    pub(crate) fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            MachineSemanticKind::PortWrite,
            constraint,
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::MayArchitecturalFaultV1,
            MachineBarrier::ExternalEffect,
            MachineAlternativeFamily::PortWrite,
            MachineSizeKnowledge::ExactBytes(x86_encoding::IMMEDIATE_PORT_WRITE_WIDTH as u16),
            effects(),
        )
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        port: u16,
        value: u8,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let kind = SelectedInstructionKind::PortWrite { port, value };
        super::request_validation::validate_request(physical, kind, alternative, operands)?;
        if bytes != x86_encoding::encode_immediate_port_write(port, value) {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: super::decoding::footprint(kind, alternative, operands),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn port_write_retains_its_exact_operands_and_rejects_changed_bytes() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::PortWrite,
                variant: 0,
            };
            for (port, value) in [(0, 0), (0x3f8, 0x41), (u16::MAX, u8::MAX)] {
                let kind = SelectedInstructionKind::PortWrite { port, value };
                let encoded =
                    crate::encode_x86_64_selected_form(&physical, kind, alternative, &[]).unwrap();
                assert_eq!(
                    encoded.bytes(),
                    x86_encoding::encode_immediate_port_write(port, value)
                );
                assert_eq!(encoded.footprint().encoded, effects());
                crate::selected_form_encoding::assert_every_bit_flip_rejected(
                    encoded.bytes(),
                    |changed| validate(&physical, port, value, alternative, &[], changed).is_err(),
                );
                assert!(
                    validate(
                        &physical,
                        port,
                        value,
                        alternative,
                        &[],
                        &encoded.bytes()[1..]
                    )
                    .is_err()
                );
                let mut extended = encoded.bytes().to_vec();
                extended.push(0);
                assert!(validate(&physical, port, value, alternative, &[], &extended).is_err());
                let register = physical.model().view_named("rax").unwrap().id;
                assert!(
                    crate::encode_x86_64_selected_form(&physical, kind, alternative, &[register])
                        .is_err()
                );
                assert!(
                    crate::encode_x86_64_selected_form(
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
            }
        }
    }
}
pub(crate) mod register_access {
    mod descriptor_registers {
        use super::{Error, Operation};
        use crate::selected_form_encoding::instruction_bytes::assemble_labeled_stream;
        use selected_instructions::{SegmentRegister, SegmentReload};

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(super) enum Label {
            Refused,
            Resumed,
            ActivationRefused,
            ActivationDone,
            Done,
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(super) enum Step {
            CodeSelector,
            Flags,
            StoreGdtr,
            WriteDescriptorLimit,
            WriteDescriptorBase,
            LoadIdtr,
            ClearVerifiedTaskBusyBit,
            InvalidCommit,
            Selector(u8),
            Load {
                bits: u8,
                base: u8,
                offset: u8,
                result: u8,
            },
            Move(u8, u8),
            Immediate(u8, u32),
            Binary {
                opcode: u8,
                source: u8,
                target: u8,
            },
            ImmediateOperation {
                extension: u8,
                register: u8,
                value: u32,
            },
            Shift {
                extension: u8,
                register: u8,
                count: u8,
            },
            ReadCr4,
            ReadMsr,
            WriteMsr,
            LoadSegment {
                register: SegmentRegister,
                source: u8,
            },
            ReadSegment {
                register: SegmentRegister,
                result: u8,
            },
            Push(u8),
            Continuation(u8, Label),
            FarReturn,
            NoOperation,
            LoadTask,
            ReadTask(u8),
            Branch(u8, Label),
            End,
        }

        pub(super) fn activation() -> Vec<(Option<Label>, Step)> {
            use Step::*;
            let mut steps = Vec::new();
            let mut push = |step| steps.push((None, step));
            let fail = Branch(0x85, Label::Refused);
            let imm = |extension, register, value| ImmediateOperation {
                extension,
                register,
                value,
            };
            let binary = |opcode, source, target| Binary {
                opcode,
                source,
                target,
            };
            let load = |bits, base, offset, result| Load {
                bits,
                base,
                offset,
                result,
            };
            for (_, step) in table_preflight(16, false, false) {
                push(step);
            }
            push(load(8, 9, 5, 0));
            push(imm(4, 0, 0x9f));
            push(imm(7, 0, 0x89));
            push(fail);
            push(load(8, 9, 6, 0));
            push(imm(4, 0, 0xef));
            push(imm(7, 0, 0));
            push(fail);
            push(load(16, 9, 0, 0));
            push(imm(7, 0, 103));
            push(fail);
            push(load(32, 9, 12, 0));
            push(imm(7, 0, 0));
            push(fail);
            push(load(32, 9, 2, 10));
            push(imm(4, 10, 0x00ff_ffff));
            push(load(8, 9, 7, 11));
            push(Shift {
                extension: 4,
                register: 11,
                count: 24,
            });
            push(binary(0x09, 11, 10));
            push(load(32, 9, 8, 11));
            push(Shift {
                extension: 4,
                register: 11,
                count: 32,
            });
            push(binary(0x09, 11, 10));
            push(binary(0x39, 6, 10));
            push(fail);
            push(LoadTask);
            push(Immediate(0, 1));
            push(Branch(0xe9, Label::Done));
            steps.push((Some(Label::Refused), Immediate(0, 0)));
            steps.push((Some(Label::Done), End));
            steps
        }

        fn table_preflight(
            descriptor_bytes: u32,
            canonical_selector: bool,
            reject_cet: bool,
        ) -> Vec<(Option<Label>, Step)> {
            use Step::*;
            let mut steps = Vec::new();
            let mut push = |step| steps.push((None, step));
            let fail = Branch(0x85, Label::Refused);
            let imm = |extension, register, value| ImmediateOperation {
                extension,
                register,
                value,
            };
            let binary = |opcode, source, target| Binary {
                opcode,
                source,
                target,
            };
            let load = |bits, base, offset, result| Load {
                bits,
                base,
                offset,
                result,
            };
            push(CodeSelector);
            push(imm(4, 0, 3));
            push(imm(7, 0, 0));
            push(fail);
            push(Flags);
            push(imm(4, 0, 0x200));
            push(imm(7, 0, 0));
            push(fail);
            if reject_cet {
                push(ReadCr4);
                push(imm(4, 0, 1 << 23));
                push(imm(7, 0, 0));
                push(fail);
            }
            push(StoreGdtr);
            push(load(16, 1, 0, 0));
            push(imm(0, 0, 1));
            push(binary(0x39, 8, 0));
            push(fail);
            push(load(64, 1, 2, 0));
            push(binary(0x39, 7, 0));
            push(fail);
            push(Selector(9));
            push(Move(9, 0));
            push(imm(4, 0, if canonical_selector { 7 } else { 4 }));
            push(imm(7, 0, 0));
            push(fail);
            push(imm(4, 9, 0xffff_fff8));
            push(imm(7, 9, 0));
            push(Branch(0x84, Label::Refused));
            push(Move(9, 0));
            push(imm(0, 0, descriptor_bytes));
            push(binary(0x39, 8, 0));
            push(Branch(0x87, Label::Refused));
            push(binary(0x01, 7, 9));
            steps
        }

        fn reload(kind: SegmentReload) -> Vec<(Option<Label>, Step)> {
            use Step::*;
            let mut steps = table_preflight(8, true, kind == SegmentReload::Code);
            let mut push = |step| steps.push((None, step));
            let imm = |extension, register, value| ImmediateOperation {
                extension,
                register,
                value,
            };
            push(Load {
                bits: 8,
                base: 9,
                offset: 5,
                result: 0,
            });
            let (mask, value) = match kind {
                SegmentReload::Code => (0xf8, 0x98),
                SegmentReload::Data => (0xfa, 0x92),
            };
            push(imm(4, 0, mask));
            push(imm(7, 0, value));
            push(Branch(0x85, Label::Refused));
            push(Load {
                bits: 8,
                base: 9,
                offset: 6,
                result: 0,
            });
            let (mask, value) = match kind {
                SegmentReload::Code => (0x60, 0x20),
                SegmentReload::Data => (0x20, 0),
            };
            push(imm(4, 0, mask));
            push(imm(7, 0, value));
            push(Branch(0x85, Label::Refused));
            match kind {
                SegmentReload::Code => {
                    push(Selector(10));
                    push(Push(10));
                    push(Continuation(10, Label::Resumed));
                    push(Push(10));
                    push(FarReturn);
                }
                SegmentReload::Data => {
                    push(Move(2, 8));
                    for (msr, saved) in [(0xc0000100, 10), (0xc0000101, 11)] {
                        push(Immediate(1, msr));
                        push(ReadMsr);
                        push(Move(0, saved));
                        push(Shift {
                            extension: 4,
                            register: 2,
                            count: 32,
                        });
                        push(Binary {
                            opcode: 0x09,
                            source: 2,
                            target: saved,
                        });
                    }
                    for register in [
                        SegmentRegister::Data,
                        SegmentRegister::Extra,
                        SegmentRegister::Stack,
                    ] {
                        push(LoadSegment {
                            register,
                            source: 8,
                        });
                    }
                    push(NoOperation);
                    push(Immediate(0, 0));
                    for register in [SegmentRegister::Fs, SegmentRegister::Gs] {
                        push(LoadSegment {
                            register,
                            source: 0,
                        });
                    }
                    for (msr, saved) in [(0xc0000100, 10), (0xc0000101, 11)] {
                        push(Immediate(1, msr));
                        push(Move(saved, 0));
                        push(Move(saved, 2));
                        push(Shift {
                            extension: 5,
                            register: 2,
                            count: 32,
                        });
                        push(WriteMsr);
                    }
                }
            }
            steps.push((Some(Label::Resumed), Immediate(0, 1)));
            steps.push((None, Branch(0xe9, Label::Done)));
            steps.push((Some(Label::Refused), Immediate(0, 0)));
            steps.push((Some(Label::Done), End));
            steps
        }

        fn segment_encoding(register: SegmentRegister) -> u8 {
            match register {
                SegmentRegister::Extra => 0,
                SegmentRegister::Code => 1,
                SegmentRegister::Stack => 2,
                SegmentRegister::Data => 3,
                SegmentRegister::Fs => 4,
                SegmentRegister::Gs => 5,
            }
        }
        fn segment_from_encoding(code: u8) -> Option<SegmentRegister> {
            Some(match code {
                0 => SegmentRegister::Extra,
                1 => SegmentRegister::Code,
                2 => SegmentRegister::Stack,
                3 => SegmentRegister::Data,
                4 => SegmentRegister::Fs,
                5 => SegmentRegister::Gs,
                _ => return None,
            })
        }

        pub(super) fn emit(bytes: &mut Vec<u8>, step: Step, patches: &mut Vec<(usize, Label)>) {
            let rex = |wide: bool, reg: u8, base: u8| {
                0x40 | (u8::from(wide) << 3) | ((reg >> 3) << 2) | (base >> 3)
            };
            let rr = |reg: u8, base: u8| 0xc0 | ((reg & 7) << 3) | (base & 7);
            match step {
                Step::ReadCr4 => bytes.extend([0x0f, 0x20, 0xe0]),
                Step::ReadMsr => bytes.extend([0x0f, 0x32]),
                Step::WriteMsr => bytes.extend([0x0f, 0x30]),
                Step::NoOperation => bytes.push(0x90),
                Step::FarReturn => bytes.extend([0x48, 0xcb]),
                Step::Push(register) => {
                    if register >= 8 {
                        bytes.push(0x41);
                    }
                    bytes.push(0x50 | (register & 7));
                }
                Step::Continuation(register, label) => {
                    bytes.extend([rex(true, register, 0), 0x8d, ((register & 7) << 3) | 5]);
                    patches.push((bytes.len(), label));
                    bytes.extend([0; 4]);
                }
                Step::LoadSegment { register, source } => {
                    bytes.extend([
                        rex(false, 0, source),
                        0x8e,
                        rr(segment_encoding(register), source),
                    ]);
                }
                Step::ReadSegment { register, result } => {
                    bytes.extend([
                        rex(false, 0, result),
                        0x8c,
                        rr(segment_encoding(register), result),
                    ]);
                    bytes.extend([rex(false, result, result), 0x0f, 0xb7, rr(result, result)]);
                }
                Step::CodeSelector => bytes.extend([0x8c, 0xc8]),
                Step::Flags => bytes.extend([0x9c, 0x58]),
                Step::StoreGdtr => bytes.extend([0x0f, 0x01, 0x01]),
                Step::WriteDescriptorLimit => bytes.extend([0x66, 0x89, 0x01]),
                Step::WriteDescriptorBase => bytes.extend([0x48, 0x89, 0x79, 0x02]),
                Step::LoadIdtr => bytes.extend([0x0f, 0x01, 0x19]),
                Step::ClearVerifiedTaskBusyBit => bytes.extend([0x41, 0x80, 0x61, 0x05, 0xfd]),
                Step::InvalidCommit => bytes.extend(crate::PRIVILEGED_STOP_BYTES),
                Step::Selector(result) => {
                    bytes.extend([rex(false, result, 2), 0x0f, 0xb7, rr(result, 2)])
                }
                Step::Load {
                    bits,
                    base,
                    offset,
                    result,
                } => {
                    bytes.push(rex(bits == 64, result, base));
                    match bits {
                        8 => bytes.extend([0x0f, 0xb6]),
                        16 => bytes.extend([0x0f, 0xb7]),
                        _ => bytes.push(0x8b),
                    }
                    bytes.extend([0x40 | ((result & 7) << 3) | (base & 7), offset]);
                }
                Step::Move(source, target) => {
                    bytes.extend([rex(true, source, target), 0x89, rr(source, target)])
                }
                Step::Immediate(register, value) => {
                    bytes.extend([rex(false, 0, register), 0xb8 | (register & 7)]);
                    bytes.extend(value.to_le_bytes());
                }
                Step::Binary {
                    opcode,
                    source,
                    target,
                } => bytes.extend([rex(true, source, target), opcode, rr(source, target)]),
                Step::ImmediateOperation {
                    extension,
                    register,
                    value,
                } => {
                    bytes.extend([rex(true, 0, register), 0x81, rr(extension, register)]);
                    bytes.extend(value.to_le_bytes());
                }
                Step::Shift {
                    extension,
                    register,
                    count,
                } => bytes.extend([rex(true, 0, register), 0xc1, rr(extension, register), count]),
                Step::LoadTask => bytes.extend([0x0f, 0x00, 0xda]),
                Step::ReadTask(result) => {
                    bytes.extend([rex(false, 0, result), 0x0f, 0x00, rr(1, result)]);
                    bytes.extend([rex(false, result, result), 0x0f, 0xb7, rr(result, result)]);
                }
                Step::Branch(condition, label) => {
                    if condition == 0xe9 {
                        bytes.push(0xe9);
                    } else {
                        bytes.extend([0x0f, condition]);
                    }
                    patches.push((bytes.len(), label));
                    bytes.extend([0; 4]);
                }
                Step::End => {}
            }
        }

        pub(super) fn encode(operation: Operation, registers: &[u8]) -> Result<Vec<u8>, Error> {
            match (operation, registers) {
                (Operation::SegmentReload(kind), [7, 2, 1, 8, 0]) => {
                    assemble_labeled_stream(&reload(kind), emit)
                }
                (Operation::SegmentRead(register), [result]) if *result < 16 && *result != 4 => {
                    let mut bytes = Vec::new();
                    emit(
                        &mut bytes,
                        Step::ReadSegment {
                            register,
                            result: *result,
                        },
                        &mut Vec::new(),
                    );
                    Ok(bytes)
                }
                (Operation::TaskRegisterActivate, [7, 6, 2, 1, 8, 0]) => {
                    assemble_labeled_stream(&activation(), emit)
                }
                (Operation::TaskRegisterRead, [result]) if *result < 16 && *result != 4 => {
                    let mut bytes = Vec::new();
                    emit(&mut bytes, Step::ReadTask(*result), &mut Vec::new());
                    Ok(bytes)
                }
                _ => Err(Error::EncodedFormMismatch),
            }
        }

        fn decode(bytes: &[u8], cursor: &mut usize) -> Option<(Step, Option<usize>)> {
            let start = *cursor;
            let tail = bytes.get(start..)?;
            let (step, length, target) = if tail.starts_with(&[0x8c, 0xc8]) {
                (Step::CodeSelector, 2, None)
            } else if tail.starts_with(&[0x0f, 0x20, 0xe0]) {
                (Step::ReadCr4, 3, None)
            } else if tail.starts_with(&[0x0f, 0x32]) {
                (Step::ReadMsr, 2, None)
            } else if tail.starts_with(&[0x0f, 0x30]) {
                (Step::WriteMsr, 2, None)
            } else if tail.starts_with(&[0x48, 0xcb]) {
                (Step::FarReturn, 2, None)
            } else if tail.first() == Some(&0x90) {
                (Step::NoOperation, 1, None)
            } else if tail
                .first()
                .is_some_and(|byte| (0x50..=0x57).contains(byte))
            {
                (Step::Push(tail[0] & 7), 1, None)
            } else if tail.first() == Some(&0x41)
                && tail.get(1).is_some_and(|byte| (0x50..=0x57).contains(byte))
            {
                (Step::Push((tail[1] & 7) | 8), 2, None)
            } else if tail.starts_with(&[0x9c, 0x58]) {
                (Step::Flags, 2, None)
            } else if tail.starts_with(&[0x0f, 0x01, 0x01]) {
                (Step::StoreGdtr, 3, None)
            } else if tail.starts_with(&[0x66, 0x89, 0x01]) {
                (Step::WriteDescriptorLimit, 3, None)
            } else if tail.starts_with(&[0x48, 0x89, 0x79, 0x02]) {
                (Step::WriteDescriptorBase, 4, None)
            } else if tail.starts_with(&[0x0f, 0x01, 0x19]) {
                (Step::LoadIdtr, 3, None)
            } else if tail.starts_with(&[0x41, 0x80, 0x61, 0x05, 0xfd]) {
                (Step::ClearVerifiedTaskBusyBit, 5, None)
            } else if tail.starts_with(&crate::PRIVILEGED_STOP_BYTES) {
                (
                    Step::InvalidCommit,
                    crate::PRIVILEGED_STOP_BYTES.len(),
                    None,
                )
            } else if let Some(result) = decode_task_read(tail.get(..8).unwrap_or(&[])) {
                (Step::ReadTask(result), 8, None)
            } else if tail.starts_with(&[0x0f, 0x00, 0xda]) {
                (Step::LoadTask, 3, None)
            } else if tail.first() == Some(&0xe9)
                || (tail.first() == Some(&0x0f) && matches!(tail.get(1), Some(0x84 | 0x85 | 0x87)))
            {
                let offset = if tail[0] == 0xe9 { 1 } else { 2 };
                let displacement =
                    i32::from_le_bytes(tail.get(offset..offset + 4)?.try_into().ok()?);
                let target =
                    usize::try_from((start + offset + 4) as i64 + i64::from(displacement)).ok()?;
                (
                    Step::Branch(if offset == 1 { 0xe9 } else { tail[1] }, Label::Refused),
                    offset + 4,
                    Some(target),
                )
            } else {
                let prefix = *tail.first()?;
                if !(0x40..=0x4f).contains(&prefix) || prefix & 2 != 0 {
                    return None;
                }
                let wide = prefix & 8 != 0;
                let opcode = *tail.get(1)?;
                let modrm = *tail.get(2)?;
                let reg = ((modrm >> 3) & 7) | ((prefix & 4) << 1);
                let base = (modrm & 7) | ((prefix & 1) << 3);
                match opcode {
                    0x8d if wide && prefix & 1 == 0 && modrm & 0xc7 == 5 => {
                        let displacement = i32::from_le_bytes(tail.get(3..7)?.try_into().ok()?);
                        let target =
                            usize::try_from((start + 7) as i64 + i64::from(displacement)).ok()?;
                        (Step::Continuation(reg, Label::Resumed), 7, Some(target))
                    }
                    0x8e if !wide && prefix & 4 == 0 && modrm >> 6 == 3 => (
                        Step::LoadSegment {
                            register: segment_from_encoding(reg)?,
                            source: base,
                        },
                        3,
                        None,
                    ),
                    0xb8..=0xbf if !wide && prefix & 4 == 0 => (
                        Step::Immediate(
                            (opcode & 7) | ((prefix & 1) << 3),
                            u32::from_le_bytes(tail.get(2..6)?.try_into().ok()?),
                        ),
                        6,
                        None,
                    ),
                    0x89 if wide && modrm >> 6 == 3 => (Step::Move(reg, base), 3, None),
                    0x01 | 0x09 | 0x39 if wide && modrm >> 6 == 3 => (
                        Step::Binary {
                            opcode,
                            source: reg,
                            target: base,
                        },
                        3,
                        None,
                    ),
                    0x81 if wide && prefix & 4 == 0 && modrm >> 6 == 3 => (
                        Step::ImmediateOperation {
                            extension: reg,
                            register: base,
                            value: u32::from_le_bytes(tail.get(3..7)?.try_into().ok()?),
                        },
                        7,
                        None,
                    ),
                    0xc1 if wide && prefix & 4 == 0 && modrm >> 6 == 3 => (
                        Step::Shift {
                            extension: reg,
                            register: base,
                            count: *tail.get(3)?,
                        },
                        4,
                        None,
                    ),
                    0x8b if modrm >> 6 == 1 && base & 7 != 4 => (
                        Step::Load {
                            bits: if wide { 64 } else { 32 },
                            base,
                            offset: *tail.get(3)?,
                            result: reg,
                        },
                        4,
                        None,
                    ),
                    0x0f if !wide => {
                        let second = modrm;
                        let address = *tail.get(3)?;
                        let reg = ((address >> 3) & 7) | ((prefix & 4) << 1);
                        let base = (address & 7) | ((prefix & 1) << 3);
                        match (second, address >> 6) {
                            (0xb7, 3) if base == 2 => (Step::Selector(reg), 4, None),
                            (0xb6 | 0xb7, 1) if base & 7 != 4 => (
                                Step::Load {
                                    bits: if second == 0xb6 { 8 } else { 16 },
                                    base,
                                    offset: *tail.get(4)?,
                                    result: reg,
                                },
                                5,
                                None,
                            ),
                            _ => return None,
                        }
                    }
                    _ => return None,
                }
            };
            *cursor += length;
            Some((step, target))
        }

        fn decode_task_read(bytes: &[u8]) -> Option<u8> {
            let [rex, 0x0f, 0x00, modrm, mov_rex, 0x0f, 0xb7, mov_modrm] = bytes else {
                return None;
            };
            let result = (modrm & 7) | ((rex & 1) << 3);
            let source = (mov_modrm & 7) | ((mov_rex & 1) << 3);
            let target = ((mov_modrm >> 3) & 7) | ((mov_rex & 4) << 1);
            (result != 4
                && *rex == 0x40 | (result >> 3)
                && modrm & 0xf8 == 0xc8
                && *mov_rex == 0x40 | ((result >> 3) << 2) | (result >> 3)
                && mov_modrm >> 6 == 3
                && source == result
                && target == result)
                .then_some(result)
        }

        pub(super) fn validate(operation: Operation, registers: &[u8], bytes: &[u8]) -> Option<()> {
            if operation == Operation::TaskRegisterRead {
                let [result] = registers else {
                    return None;
                };
                return (decode_task_read(bytes)? == *result).then_some(());
            }
            if let Operation::SegmentRead(register) = operation {
                let [result] = registers else {
                    return None;
                };
                let [rex, 0x8c, modrm, mov_rex, 0x0f, 0xb7, mov_modrm] = bytes else {
                    return None;
                };
                return (*result < 16
                    && *result != 4
                    && *rex == 0x40 | (*result >> 3)
                    && *modrm == 0xc0 | (segment_encoding(register) << 3) | (*result & 7)
                    && *mov_rex == 0x40 | ((*result >> 3) << 2) | (*result >> 3)
                    && *mov_modrm == 0xc0 | ((*result & 7) << 3) | (*result & 7))
                    .then_some(());
            }
            let steps = match operation {
                Operation::TaskRegisterActivate if registers == [7, 6, 2, 1, 8, 0] => activation(),
                Operation::SegmentReload(kind) if registers == [7, 2, 1, 8, 0] => reload(kind),
                _ => return None,
            };
            validate_stream(&steps, bytes)
        }

        pub(super) fn validate_stream(steps: &[(Option<Label>, Step)], bytes: &[u8]) -> Option<()> {
            let mut cursor = 0;
            let mut labels = Vec::new();
            let mut branches = Vec::new();
            for &(label, expected) in steps {
                if let Some(label) = label {
                    labels.push((label, cursor));
                }
                if expected == Step::End {
                    continue;
                }
                let (actual, target) = decode(bytes, &mut cursor)?;
                match (expected, actual, target) {
                    (Step::Branch(condition, label), Step::Branch(actual, _), Some(target))
                        if condition == actual =>
                    {
                        branches.push((label, target))
                    }
                    (
                        Step::Continuation(register, label),
                        Step::Continuation(actual, _),
                        Some(target),
                    ) if register == actual => branches.push((label, target)),
                    (expected, actual, None) if expected == actual => {}
                    _ => return None,
                }
            }
            (cursor == bytes.len()
                && branches
                    .iter()
                    .all(|(label, target)| labels.contains(&(*label, *target))))
            .then_some(())
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn activation_checks_every_operand_and_refusal_branch_before_ltr() {
                let registers = [7, 6, 2, 1, 8, 0];
                let bytes = encode(Operation::TaskRegisterActivate, &registers).unwrap();
                assert!(validate(Operation::TaskRegisterActivate, &registers, &bytes).is_some());
                assert!(bytes.len() <= 512);
                for index in 0..bytes.len() {
                    assert!(
                        validate(Operation::TaskRegisterActivate, &registers, &bytes[..index])
                            .is_none()
                    );
                    for bit in 0..8 {
                        let mut changed = bytes.clone();
                        changed[index] ^= 1 << bit;
                        assert!(
                            validate(Operation::TaskRegisterActivate, &registers, &changed)
                                .is_none(),
                            "changed byte {index} bit {bit}"
                        );
                    }
                }
                for index in 0..registers.len() {
                    let mut changed = registers;
                    changed[index] ^= 1;
                    assert!(encode(Operation::TaskRegisterActivate, &changed).is_err());
                    assert!(validate(Operation::TaskRegisterActivate, &changed, &bytes).is_none());
                }
            }

            #[test]
            fn selector_read_defines_all_result_bits_without_changing_flags() {
                for result in 0..16 {
                    if result == 4 {
                        continue;
                    }
                    let bytes = encode(Operation::TaskRegisterRead, &[result]).unwrap();
                    assert!(validate(Operation::TaskRegisterRead, &[result], &bytes).is_some());
                    for index in 0..bytes.len() {
                        for bit in 0..8 {
                            let mut changed = bytes.clone();
                            changed[index] ^= 1 << bit;
                            assert!(
                                validate(Operation::TaskRegisterRead, &[result], &changed)
                                    .is_none()
                            );
                        }
                    }
                }
            }
            #[test]
            fn selector_reload_checks_every_byte_and_sealed_continuation() {
                let registers = [7, 2, 1, 8, 0];
                for kind in [SegmentReload::Code, SegmentReload::Data] {
                    let operation = Operation::SegmentReload(kind);
                    let bytes = encode(operation, &registers).unwrap();
                    assert!(bytes.len() <= 512);
                    assert!(validate(operation, &registers, &bytes).is_some());
                    for index in 0..bytes.len() {
                        assert!(validate(operation, &registers, &bytes[..index]).is_none());
                        for bit in 0..8 {
                            let mut changed = bytes.clone();
                            changed[index] ^= 1 << bit;
                            assert!(
                                validate(operation, &registers, &changed).is_none(),
                                "{kind:?}: byte {index}, bit {bit}"
                            );
                        }
                    }
                    for index in 0..registers.len() {
                        let mut changed = registers;
                        changed[index] ^= 1;
                        assert!(encode(operation, &changed).is_err());
                        assert!(validate(operation, &changed, &bytes).is_none());
                    }
                }
            }

            #[test]
            fn segment_reads_preserve_exact_selector_and_zero_extend_the_result() {
                for register in [
                    SegmentRegister::Code,
                    SegmentRegister::Data,
                    SegmentRegister::Extra,
                    SegmentRegister::Stack,
                    SegmentRegister::Fs,
                    SegmentRegister::Gs,
                ] {
                    for result in 0..16 {
                        if result == 4 {
                            continue;
                        }
                        let operation = Operation::SegmentRead(register);
                        let bytes = encode(operation, &[result]).unwrap();
                        assert!(validate(operation, &[result], &bytes).is_some());
                        for index in 0..bytes.len() {
                            for bit in 0..8 {
                                let mut changed = bytes.clone();
                                changed[index] ^= 1 << bit;
                                assert!(validate(operation, &[result], &changed).is_none());
                            }
                        }
                    }
                }
            }
        }
    }
    pub(crate) mod installed_table {
        use calling_conventions::MachineRegister;

        use super::descriptor_registers::{Label, Step, activation, emit, validate_stream};
        use crate::selected_form_encoding::X86_64SelectedFormEncodingError;
        use crate::selected_form_encoding::instruction_bytes::assemble_labeled_stream;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct X86InstalledCpuStateRegisters {
            pub gdt: MachineRegister,
            pub task: MachineRegister,
            pub descriptor_scratch: MachineRegister,
            pub result: MachineRegister,
            pub scratch: [MachineRegister; 3],
        }

        impl X86InstalledCpuStateRegisters {
            pub const FIXED: Self = Self {
                gdt: MachineRegister::X86Rdi,
                task: MachineRegister::X86Rsi,
                descriptor_scratch: MachineRegister::X86Rcx,
                result: MachineRegister::X86Rax,
                scratch: [
                    MachineRegister::X86R9,
                    MachineRegister::X86R10,
                    MachineRegister::X86R11,
                ],
            };
            pub const CLOBBERS: [MachineRegister; 4] = [
                MachineRegister::X86Rax,
                MachineRegister::X86R9,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
            ];
            pub const TEMPORARY_STACK_BYTES: u32 = 8;
            pub const DESCRIPTOR_SCRATCH_BYTES: u32 = 10;
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct X86InstalledCpuStateGeometry {
            pub gdt_bytes: u32,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct X86InstalledTaskCommitRegisters {
            pub cpu: X86InstalledCpuStateRegisters,
            pub selector_scratch: MachineRegister,
            pub extent_scratch: MachineRegister,
        }

        impl X86InstalledTaskCommitRegisters {
            pub const FIXED: Self = Self {
                cpu: X86InstalledCpuStateRegisters::FIXED,
                selector_scratch: MachineRegister::X86Rdx,
                extent_scratch: MachineRegister::X86R8,
            };
            pub const CLOBBERS: [MachineRegister; 6] = [
                MachineRegister::X86Rax,
                MachineRegister::X86Rdx,
                MachineRegister::X86R8,
                MachineRegister::X86R9,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
            ];
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct X86InstalledTablePublicationRegisters {
            pub table: MachineRegister,
            pub descriptor_scratch: MachineRegister,
            pub scratch: MachineRegister,
        }

        impl X86InstalledTablePublicationRegisters {
            pub const FIXED: Self = Self {
                table: MachineRegister::X86Rdi,
                descriptor_scratch: MachineRegister::X86Rcx,
                scratch: MachineRegister::X86Rax,
            };
            pub const CLOBBERS: [MachineRegister; 1] = [MachineRegister::X86Rax];
            pub const DESCRIPTOR_SCRATCH_BYTES: u32 = 10;
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum X86InstalledTableEncodingError {
            UnsupportedRegisters,
            InvalidGdtExtent,
            InvalidIdtExtent,
            Encoding(X86_64SelectedFormEncodingError),
            ReplayMismatch,
        }

        impl std::fmt::Display for X86InstalledTableEncodingError {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
            Self::UnsupportedRegisters => formatter.write_str("installed table operand registers differ from the explicit fixed-register contract"),
            Self::InvalidGdtExtent => formatter.write_str("GDT extent must contain whole eight-byte entries and fit its 16-bit limit"),
            Self::InvalidIdtExtent => formatter.write_str("IDT extent must contain 1 through 256 complete 16-byte gates"),
            Self::Encoding(error) => write!(formatter, "installed table instruction encoding failed: {error}"),
            Self::ReplayMismatch => formatter.write_str("installed table bytes differ from the retained operand and geometry checks"),
        }
            }
        }
        impl std::error::Error for X86InstalledTableEncodingError {}
        type Error = X86InstalledTableEncodingError;

        pub fn encode_x86_64_installed_cpu_state_check(
            registers: X86InstalledCpuStateRegisters,
            geometry: X86InstalledCpuStateGeometry,
        ) -> Result<Vec<u8>, Error> {
            let stream = cpu_state_stream(registers, geometry, false)?;
            assemble_labeled_stream(&stream, emit).map_err(Error::Encoding)
        }

        pub fn validate_x86_64_installed_cpu_state_check(
            registers: X86InstalledCpuStateRegisters,
            geometry: X86InstalledCpuStateGeometry,
            bytes: &[u8],
        ) -> Result<(), Error> {
            validate_stream(&cpu_state_stream(registers, geometry, false)?, bytes)
                .ok_or(Error::ReplayMismatch)
        }

        pub fn encode_x86_64_installed_task_commit(
            registers: X86InstalledTaskCommitRegisters,
            geometry: X86InstalledCpuStateGeometry,
        ) -> Result<Vec<u8>, Error> {
            assemble_labeled_stream(&task_commit_stream(registers, geometry)?, emit)
                .map_err(Error::Encoding)
        }

        pub fn validate_x86_64_installed_task_commit(
            registers: X86InstalledTaskCommitRegisters,
            geometry: X86InstalledCpuStateGeometry,
            bytes: &[u8],
        ) -> Result<(), Error> {
            validate_stream(&task_commit_stream(registers, geometry)?, bytes)
                .ok_or(Error::ReplayMismatch)
        }

        pub fn encode_x86_64_installed_table_publication(
            registers: X86InstalledTablePublicationRegisters,
            table_bytes: u32,
        ) -> Result<Vec<u8>, Error> {
            assemble_labeled_stream(&publication_stream(registers, table_bytes)?, emit)
                .map_err(Error::Encoding)
        }

        pub fn validate_x86_64_installed_table_publication(
            registers: X86InstalledTablePublicationRegisters,
            table_bytes: u32,
            bytes: &[u8],
        ) -> Result<(), Error> {
            validate_stream(&publication_stream(registers, table_bytes)?, bytes)
                .ok_or(Error::ReplayMismatch)
        }

        fn cpu_state_stream(
            registers: X86InstalledCpuStateRegisters,
            geometry: X86InstalledCpuStateGeometry,
            preserve_selector: bool,
        ) -> Result<Vec<(Option<Label>, Step)>, Error> {
            if registers != X86InstalledCpuStateRegisters::FIXED {
                return Err(Error::UnsupportedRegisters);
            }
            if !(8..=65_536).contains(&geometry.gdt_bytes) || !geometry.gdt_bytes.is_multiple_of(8)
            {
                return Err(Error::InvalidGdtExtent);
            }
            use Step::*;
            let mut steps = Vec::new();
            let mut push = |step| steps.push((None, step));
            let fail = Branch(0x85, Label::Refused);
            let imm = |extension, register, value| ImmediateOperation {
                extension,
                register,
                value,
            };
            let binary = |opcode, source, target| Binary {
                opcode,
                source,
                target,
            };
            let load = |bits, base, offset, result| Load {
                bits,
                base,
                offset,
                result,
            };
            push(CodeSelector);
            push(imm(4, 0, 3));
            push(imm(7, 0, 0));
            push(fail);
            push(Flags);
            push(imm(4, 0, 0x200));
            push(imm(7, 0, 0));
            push(fail);
            push(StoreGdtr);
            push(load(16, 1, 0, 0));
            push(imm(7, 0, geometry.gdt_bytes - 1));
            push(fail);
            push(load(64, 1, 2, 0));
            push(binary(0x39, 7, 0));
            push(fail);
            push(ReadTask(9));
            push(Move(9, 0));
            push(imm(4, 0, 7));
            push(imm(7, 0, 0));
            push(fail);
            push(imm(7, 9, 0));
            push(Branch(0x84, Label::Refused));
            push(Move(9, 0));
            push(imm(0, 0, 16));
            push(imm(7, 0, geometry.gdt_bytes));
            push(Branch(0x87, Label::Refused));
            if preserve_selector {
                push(Move(9, 2));
            }
            push(binary(0x01, 7, 9));
            push(load(8, 9, 5, 0));
            push(imm(7, 0, 0x8b));
            push(fail);
            push(load(8, 9, 6, 0));
            push(imm(7, 0, 0));
            push(fail);
            push(load(16, 9, 0, 0));
            push(imm(7, 0, 103));
            push(fail);
            push(load(32, 9, 12, 0));
            push(imm(7, 0, 0));
            push(fail);
            push(load(32, 9, 2, 10));
            push(imm(4, 10, 0x00ff_ffff));
            push(load(8, 9, 7, 11));
            push(Shift {
                extension: 4,
                register: 11,
                count: 24,
            });
            push(binary(0x09, 11, 10));
            push(load(32, 9, 8, 11));
            push(Shift {
                extension: 4,
                register: 11,
                count: 32,
            });
            push(binary(0x09, 11, 10));
            push(binary(0x39, 6, 10));
            push(fail);
            push(Immediate(0, 1));
            push(Branch(0xe9, Label::Done));
            steps.push((Some(Label::Refused), Immediate(0, 0)));
            steps.push((Some(Label::Done), End));
            Ok(steps)
        }

        fn task_commit_stream(
            registers: X86InstalledTaskCommitRegisters,
            geometry: X86InstalledCpuStateGeometry,
        ) -> Result<Vec<(Option<Label>, Step)>, Error> {
            if registers != X86InstalledTaskCommitRegisters::FIXED {
                return Err(Error::UnsupportedRegisters);
            }
            let mut steps = cpu_state_stream(registers.cpu, geometry, true)?;
            steps.truncate(steps.len() - 4);
            steps.extend([
                (None, Step::ClearVerifiedTaskBusyBit),
                (None, Step::Immediate(8, geometry.gdt_bytes)),
            ]);
            let activation_label = |label| match label {
                Label::Refused => Label::ActivationRefused,
                Label::Done => Label::ActivationDone,
                other => other,
            };
            for (label, step) in activation() {
                let step = match step {
                    Step::Branch(condition, label) => {
                        Step::Branch(condition, activation_label(label))
                    }
                    other => other,
                };
                steps.push((label.map(activation_label), step));
            }
            steps.extend([
                (
                    None,
                    Step::ImmediateOperation {
                        extension: 7,
                        register: 0,
                        value: 0,
                    },
                ),
                (None, Step::Branch(0x85, Label::Done)),
                (None, Step::InvalidCommit),
                (Some(Label::Refused), Step::InvalidCommit),
                (Some(Label::Done), Step::End),
            ]);
            Ok(steps)
        }

        fn publication_stream(
            registers: X86InstalledTablePublicationRegisters,
            table_bytes: u32,
        ) -> Result<Vec<(Option<Label>, Step)>, Error> {
            if registers != X86InstalledTablePublicationRegisters::FIXED {
                return Err(Error::UnsupportedRegisters);
            }
            if !(16..=4096).contains(&table_bytes) || !table_bytes.is_multiple_of(16) {
                return Err(Error::InvalidIdtExtent);
            }
            Ok(vec![
                (None, Step::Immediate(0, table_bytes - 1)),
                (None, Step::WriteDescriptorLimit),
                (None, Step::WriteDescriptorBase),
                (None, Step::LoadIdtr),
            ])
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn current_cpu_checks_bind_every_byte_and_refusal_branch() {
                let registers = X86InstalledCpuStateRegisters::FIXED;
                let geometry = X86InstalledCpuStateGeometry { gdt_bytes: 40 };
                let bytes = encode_x86_64_installed_cpu_state_check(registers, geometry).unwrap();
                validate_x86_64_installed_cpu_state_check(registers, geometry, &bytes).unwrap();
                for index in 0..bytes.len() {
                    assert!(
                        validate_x86_64_installed_cpu_state_check(
                            registers,
                            geometry,
                            &bytes[..index]
                        )
                        .is_err()
                    );
                    for bit in 0..8 {
                        let mut changed = bytes.clone();
                        changed[index] ^= 1 << bit;
                        assert!(
                            validate_x86_64_installed_cpu_state_check(
                                registers, geometry, &changed
                            )
                            .is_err(),
                            "byte {index} bit {bit}"
                        );
                    }
                }
                let different_table = X86InstalledCpuStateGeometry { gdt_bytes: 48 };
                assert!(
                    validate_x86_64_installed_cpu_state_check(registers, different_table, &bytes)
                        .is_err()
                );
                for extent in [0, 7, 9, 65_544] {
                    assert!(
                        encode_x86_64_installed_cpu_state_check(
                            registers,
                            X86InstalledCpuStateGeometry { gdt_bytes: extent }
                        )
                        .is_err()
                    );
                }
                let wrong_register = X86InstalledCpuStateRegisters {
                    task: MachineRegister::X86Rdx,
                    ..registers
                };
                assert!(encode_x86_64_installed_cpu_state_check(wrong_register, geometry).is_err());
                assert!(
                    cpu_state_stream(registers, geometry, false)
                        .unwrap()
                        .iter()
                        .all(|(_, step)| !matches!(
                            step,
                            Step::LoadTask
                                | Step::ClearVerifiedTaskBusyBit
                                | Step::LoadIdtr
                                | Step::WriteDescriptorBase
                                | Step::WriteDescriptorLimit
                        ))
                );
            }

            #[test]
            fn task_commit_rechecks_then_reloads_and_cannot_refuse_after_mutation() {
                let registers = X86InstalledTaskCommitRegisters::FIXED;
                let geometry = X86InstalledCpuStateGeometry { gdt_bytes: 40 };
                let bytes = encode_x86_64_installed_task_commit(registers, geometry).unwrap();
                validate_x86_64_installed_task_commit(registers, geometry, &bytes).unwrap();
                for index in 0..bytes.len() {
                    assert!(
                        validate_x86_64_installed_task_commit(registers, geometry, &bytes[..index])
                            .is_err()
                    );
                    for bit in 0..8 {
                        let mut changed = bytes.clone();
                        changed[index] ^= 1 << bit;
                        assert!(
                            validate_x86_64_installed_task_commit(registers, geometry, &changed)
                                .is_err(),
                            "byte {index} bit {bit}"
                        );
                    }
                }
                let stream = task_commit_stream(registers, geometry).unwrap();
                let mutation = stream
                    .iter()
                    .position(|(_, step)| *step == Step::ClearVerifiedTaskBusyBit)
                    .unwrap();
                let ltr = stream
                    .iter()
                    .position(|(_, step)| *step == Step::LoadTask)
                    .unwrap();
                assert!(mutation < ltr);
                assert!(
                    stream[..mutation]
                        .iter()
                        .any(|(_, step)| *step == Step::ReadTask(9))
                );
                assert!(
                    stream[mutation..]
                        .iter()
                        .all(|(_, step)| !matches!(step, Step::Branch(_, Label::Refused)))
                );
                let refused = stream
                    .iter()
                    .position(|(label, _)| *label == Some(Label::Refused))
                    .unwrap();
                assert_eq!(stream[refused - 1].1, Step::InvalidCommit);
                assert!(!stream.iter().any(|(_, step)| *step == Step::LoadIdtr));
                let preserved_selector = stream
                    .iter()
                    .position(|(_, step)| *step == Step::Move(9, 2))
                    .unwrap();
                assert!(preserved_selector < mutation);
                assert!(
                    !stream[preserved_selector + 1..ltr]
                        .iter()
                        .any(|(_, step)| matches!(
                            step,
                            Step::Immediate(2, _) | Step::Move(_, 2) | Step::ReadTask(2)
                        ))
                );
                let mut guessed = stream.clone();
                guessed[preserved_selector].1 = Step::Immediate(2, 24);
                let guessed_bytes = assemble_labeled_stream(&guessed, emit).unwrap();
                assert!(
                    validate_x86_64_installed_task_commit(registers, geometry, &guessed_bytes)
                        .is_err()
                );
            }

            #[test]
            fn publication_binds_real_base_exact_extent_and_final_lidt() {
                let registers = X86InstalledTablePublicationRegisters::FIXED;
                let bytes = encode_x86_64_installed_table_publication(registers, 1024).unwrap();
                validate_x86_64_installed_table_publication(registers, 1024, &bytes).unwrap();
                assert!(bytes.ends_with(&[0x0f, 0x01, 0x19]));
                for index in 0..bytes.len() {
                    assert!(
                        validate_x86_64_installed_table_publication(
                            registers,
                            1024,
                            &bytes[..index]
                        )
                        .is_err()
                    );
                    for bit in 0..8 {
                        let mut changed = bytes.clone();
                        changed[index] ^= 1 << bit;
                        assert!(
                            validate_x86_64_installed_table_publication(registers, 1024, &changed)
                                .is_err()
                        );
                    }
                }
                assert!(
                    validate_x86_64_installed_table_publication(registers, 512, &bytes).is_err()
                );
                for extent in [0, 15, 17, 4112] {
                    assert!(encode_x86_64_installed_table_publication(registers, extent).is_err());
                }
                let wrong_register = X86InstalledTablePublicationRegisters {
                    table: MachineRegister::X86Rsi,
                    ..registers
                };
                assert!(encode_x86_64_installed_table_publication(wrong_register, 1024).is_err());
            }
        }
    }
    use super::{
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
        X86_64SelectedFormFootprint,
    };
    use crate::register_model::units_of;
    use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::GlobalDescriptorTableOperation as DescriptorOperation;
    use selected_instructions::SystemRegister;
    use selected_instructions::{ControlRegister, SegmentRegister, SegmentReload};
    use selected_instructions::{
        InterruptControlKind, MachineAlternativeFamily as Family, MachineAlternativeKey,
        MachineBarrier, MachineEffectDeclaration, MachineEncodedEffects,
        MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
        MachineMemoryEffect, MachineSemanticKind as Semantic, MachineSizeKnowledge,
        MachineTrapBehavior, MemoryFenceKind, RegisterOperation as Operation,
        RegisterTargetApplicability, SelectedInstructionKind,
    };

    pub(crate) fn semantic(operation: Operation) -> Option<Semantic> {
        Some(match operation {
            Operation::SystemRead(SystemRegister::Control(_)) => Semantic::ControlRegisterRead,
            Operation::SystemWrite(SystemRegister::Control(_)) => Semantic::ControlRegisterWrite,
            Operation::GlobalDescriptorTable(DescriptorOperation::Load) => {
                Semantic::GlobalDescriptorTableLoad
            }
            Operation::GlobalDescriptorTable(DescriptorOperation::Store) => {
                Semantic::GlobalDescriptorTableStore
            }
            Operation::TaskRegisterActivate => Semantic::TaskRegisterActivate,
            Operation::TaskRegisterRead => Semantic::TaskRegisterRead,
            Operation::SegmentReload(SegmentReload::Code) => Semantic::CodeSegmentReload,
            Operation::SegmentReload(SegmentReload::Data) => Semantic::DataSegmentReload,
            Operation::SegmentRead(_) => Semantic::SegmentSelectorRead,
            Operation::PortRead => Semantic::PortRegisterRead,
            Operation::PortWrite => Semantic::PortRegisterWrite,
            Operation::ModelSpecificRead => Semantic::ModelSpecificRead,
            Operation::ModelSpecificWrite => Semantic::ModelSpecificWrite,
            Operation::MemoryFence(kind)
                if kind.target() == RegisterTargetApplicability::X86_64 =>
            {
                Semantic::MemoryFence
            }
            Operation::ProcessorHalt => Semantic::ProcessorHalt,
            Operation::InterruptControl(_) => Semantic::InterruptControl,
            Operation::SystemRead(SystemRegister::Rflags) => Semantic::SystemRegisterRead,
            Operation::SystemWrite(SystemRegister::Rflags) => Semantic::SystemRegisterWrite,
            _ => return None,
        })
    }

    pub(crate) fn operation(semantic: Semantic) -> Option<Operation> {
        Some(match semantic {
            Semantic::ControlRegisterRead => {
                Operation::SystemRead(SystemRegister::Control(ControlRegister::Cr0))
            }
            Semantic::ControlRegisterWrite => {
                Operation::SystemWrite(SystemRegister::Control(ControlRegister::Cr0))
            }
            Semantic::GlobalDescriptorTableLoad => {
                Operation::GlobalDescriptorTable(DescriptorOperation::Load)
            }
            Semantic::GlobalDescriptorTableStore => {
                Operation::GlobalDescriptorTable(DescriptorOperation::Store)
            }
            Semantic::TaskRegisterActivate => Operation::TaskRegisterActivate,
            Semantic::TaskRegisterRead => Operation::TaskRegisterRead,
            Semantic::CodeSegmentReload => Operation::SegmentReload(SegmentReload::Code),
            Semantic::DataSegmentReload => Operation::SegmentReload(SegmentReload::Data),
            Semantic::SegmentSelectorRead => Operation::SegmentRead(SegmentRegister::Code),
            Semantic::PortRegisterRead => Operation::PortRead,
            Semantic::PortRegisterWrite => Operation::PortWrite,
            Semantic::ModelSpecificRead => Operation::ModelSpecificRead,
            Semantic::ModelSpecificWrite => Operation::ModelSpecificWrite,
            Semantic::MemoryFence => Operation::MemoryFence(MemoryFenceKind::Full),
            Semantic::ProcessorHalt => Operation::ProcessorHalt,
            Semantic::InterruptControl => {
                Operation::InterruptControl(InterruptControlKind::Disable)
            }
            Semantic::SystemRegisterRead => Operation::SystemRead(SystemRegister::Rflags),
            Semantic::SystemRegisterWrite => Operation::SystemWrite(SystemRegister::Rflags),
            _ => return None,
        })
    }

    pub(crate) fn family(operation: Operation) -> Result<Family, Error> {
        Ok(Family::from(
            semantic(operation).ok_or(Error::AlternativeMismatch)?,
        ))
    }

    fn scratch(operation: Operation) -> &'static [&'static str] {
        match operation {
            Operation::SystemRead(SystemRegister::Control(_)) => &["r10", "r15"],
            Operation::SystemWrite(SystemRegister::Control(_)) => &["rax", "r10", "r11", "r15"],
            Operation::GlobalDescriptorTable(_) => &["r10"],
            Operation::TaskRegisterActivate => &["r9", "r10", "r11", "condition_flags"],
            Operation::SegmentReload(SegmentReload::Code) => &["r9", "r10", "condition_flags"],
            Operation::SegmentReload(SegmentReload::Data) => {
                &["rcx", "rdx", "r8", "r9", "r10", "r11", "condition_flags"]
            }
            Operation::PortRead => &["rax", "rdx", "r10"],
            Operation::SystemRead(SystemRegister::Rflags)
            | Operation::SystemWrite(SystemRegister::Rflags) => &["r10", "r15"],
            Operation::PortWrite => &["rax", "rdx", "r10", "r11"],
            Operation::ModelSpecificRead | Operation::ModelSpecificWrite => {
                &["rax", "rcx", "rdx", "r10", "r11"]
            }
            _ => &[],
        }
    }

    pub(crate) fn effects(operation: Operation) -> MachineEncodedEffects {
        if matches!(operation, Operation::MemoryFence(_)) {
            return MachineEncodedEffects::catalog_fence_v1();
        }
        let physical = crate::x86_64_physical_register_model_ref();
        let mut encoded = MachineEncodedEffects {
            implicit_unit_clobbers: units_of(scratch(operation)),
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            ..MachineEncodedEffects::fallthrough_v1(
                (0..operation.input_carriers().len() as u16).collect(),
                if operation.produces_value() {
                    vec![operation.input_carriers().len() as u16]
                } else {
                    vec![]
                },
            )
        };
        if operation == Operation::SystemRead(SystemRegister::Rflags)
            || operation == Operation::SystemWrite(SystemRegister::Rflags)
        {
            let stack_pointer = physical
                .view_named("rsp")
                .expect("canonical stack pointer")
                .id;
            let units = |name| {
                physical
                    .view_named(name)
                    .expect("canonical flags register")
                    .units
                    .iter()
                    .copied()
            };
            encoded.implicit_unit_uses = if operation.produces_value() {
                register_model::sorted_units(["rflags", "rsp"].into_iter().flat_map(units))
            } else {
                register_model::sorted_units(["rsp"].into_iter().flat_map(units))
            };
            if !operation.produces_value() {
                encoded.implicit_unit_defs = register_model::sorted_units(units("rflags"));
            }
            encoded.memory = MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                stack_pointer,
                byte_count: 8,
            };
            encoded.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: 8,
            };
        }
        if operation == Operation::TaskRegisterActivate {
            let stack_pointer = physical
                .view_named("rsp")
                .expect("canonical stack pointer")
                .id;
            encoded.implicit_unit_uses = units_of(&["rsp", "control_flags"]);
            encoded.memory = MachineEncodedMemoryEffect::TaskRegisterActivationV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                selector_operand: 2,
                scratch_pointer_operand: 3,
                table_byte_count_operand: 4,
                stack_pointer,
            };
            encoded.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: 8,
            };
        }
        if let Operation::SegmentReload(kind) = operation {
            let stack_pointer = physical
                .view_named("rsp")
                .expect("canonical stack pointer")
                .id;
            encoded.implicit_unit_uses = units_of(&["rsp", "control_flags"]);
            encoded.memory = MachineEncodedMemoryEffect::SegmentDescriptorReloadV1 {
                table_pointer_operand: 0,
                selector_operand: 1,
                scratch_pointer_operand: 2,
                table_byte_count_operand: 3,
                stack_pointer,
            };
            encoded.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: match kind {
                    SegmentReload::Code => 16,
                    SegmentReload::Data => 8,
                },
            };
        }
        match operation {
            Operation::GlobalDescriptorTable(DescriptorOperation::Load) => {
                encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: 10,
                };
            }
            Operation::GlobalDescriptorTable(DescriptorOperation::Store) => {
                encoded.memory = MachineEncodedMemoryEffect::WriteFixedPointerV1 {
                    pointer_operand: 0,
                    byte_count: 10,
                };
            }
            _ => {}
        }
        if matches!(operation, Operation::InterruptControl(_)) {
            let flags = physical
                .view_named("control_flags")
                .expect("canonical flags register");
            encoded.implicit_unit_uses = register_model::sorted_units(flags.units.iter().copied());
            encoded.implicit_unit_defs = encoded.implicit_unit_uses.clone();
        }
        encoded
    }

    pub(crate) fn declaration(
        operation: Operation,
        constraint: RegisterConstraintKey,
    ) -> MachineEffectDeclaration {
        if matches!(operation, Operation::MemoryFence(_)) {
            return super::memory_fence::declaration(constraint);
        }
        MachineEffectDeclaration::single(
            semantic(operation).expect("x86 register operation"),
            constraint,
            if matches!(
                operation,
                Operation::SystemRead(SystemRegister::Rflags)
                    | Operation::SystemWrite(SystemRegister::Rflags)
            ) {
                MachineMemoryEffect::TemporaryStackStorageV1
            } else if matches!(operation, Operation::SegmentReload(_)) {
                MachineMemoryEffect::SegmentDescriptorReloadV1
            } else if operation == Operation::TaskRegisterActivate {
                MachineMemoryEffect::TaskRegisterActivationV1
            } else if operation == Operation::GlobalDescriptorTable(DescriptorOperation::Load) {
                MachineMemoryEffect::ReadPointerV1
            } else if operation == Operation::GlobalDescriptorTable(DescriptorOperation::Store) {
                MachineMemoryEffect::WritePointerV1
            } else {
                MachineMemoryEffect::NoneV1
            },
            MachineTrapBehavior::MayArchitecturalFaultV1,
            MachineBarrier::ExternalEffect,
            family(operation).expect("x86 register family"),
            if matches!(
                operation,
                Operation::ProcessorHalt | Operation::InterruptControl(_)
            ) {
                MachineSizeKnowledge::ExactBytes(1)
            } else {
                crate::encoder_resolved(
                    1,
                    Some(
                        if matches!(
                            operation,
                            Operation::TaskRegisterActivate | Operation::SegmentReload(_)
                        ) {
                            512
                        } else {
                            40
                        },
                    ),
                )
            },
            effects(operation),
        )
    }

    fn move_register(bytes: &mut Vec<u8>, source: u8, destination: u8, wide: bool) {
        bytes.extend([
            0x40 | (u8::from(wide) << 3) | ((source >> 3) << 2) | (destination >> 3),
            0x89,
            0xc0 | ((source & 7) << 3) | (destination & 7),
        ]);
    }
    fn swap_bytes(bytes: &mut Vec<u8>, register: u8, wide: bool) {
        let rex = 0x40 | (u8::from(wide) << 3) | (register >> 3);
        if rex != 0x40 {
            bytes.push(rex);
        }
        bytes.extend([0x0f, 0xc8 | (register & 7)]);
    }

    pub(crate) fn encode(operation: Operation, registers: &[u8]) -> Result<Vec<u8>, Error> {
        if matches!(
            operation,
            Operation::TaskRegisterActivate
                | Operation::TaskRegisterRead
                | Operation::SegmentReload(_)
                | Operation::SegmentRead(_)
        ) {
            return descriptor_registers::encode(operation, registers);
        }
        if let Operation::MemoryFence(kind) = operation {
            if !registers.is_empty() {
                return Err(Error::OperandCountMismatch);
            }
            return super::memory_fence::encode(kind).ok_or(Error::AlternativeMismatch);
        }
        family(operation)?;
        if let Operation::GlobalDescriptorTable(direction) = operation {
            let [pointer] = registers else {
                return Err(Error::OperandCountMismatch);
            };
            if *pointer > 15 {
                return Err(Error::EncodedFormMismatch);
            }
            let mut bytes = Vec::new();
            move_register(&mut bytes, *pointer, 10, true);
            bytes.extend([
                0x41,
                0x0f,
                0x01,
                match direction {
                    DescriptorOperation::Store => 0x02,
                    DescriptorOperation::Load => 0x12,
                },
            ]);
            return Ok(bytes);
        }
        if let Operation::SystemRead(SystemRegister::Control(control))
        | Operation::SystemWrite(SystemRegister::Control(control)) = operation
        {
            let [operand] = registers else {
                return Err(Error::OperandCountMismatch);
            };
            if *operand > 15
                || (operation.produces_value() && *operand == 4)
                || !operation.is_admitted()
            {
                return Err(Error::EncodedFormMismatch);
            }
            let selector = match control {
                ControlRegister::Cr0 => 0,
                ControlRegister::Cr2 => 2,
                ControlRegister::Cr3 => 3,
                ControlRegister::Cr4 => 4,
            };
            let mut bytes = Vec::new();
            if operation.produces_value() {
                bytes.extend([0x41, 0x0f, 0x20, 0xc2 | (selector << 3)]);
                move_register(&mut bytes, 10, 15, true);
                move_register(&mut bytes, 15, *operand, true);
            } else {
                move_register(&mut bytes, *operand, 10, true);
                bytes.extend([0x41, 0x0f, 0x22, 0xc2 | (selector << 3)]);
            }
            return Ok(bytes);
        }
        if let Operation::InterruptControl(kind) = operation {
            return if registers.is_empty() {
                Ok(vec![match kind {
                    InterruptControlKind::Disable => 0xfa,
                    InterruptControlKind::Enable => 0xfb,
                }])
            } else {
                Err(Error::OperandCountMismatch)
            };
        }
        if operation == Operation::ProcessorHalt {
            return if registers.is_empty() {
                Ok(vec![0xf4])
            } else {
                Err(Error::OperandCountMismatch)
            };
        }
        if operation == Operation::SystemRead(SystemRegister::Rflags) {
            let [destination] = registers else {
                return Err(Error::OperandCountMismatch);
            };
            if *destination > 15 || *destination == 4 {
                return Err(Error::EncodedFormMismatch);
            }
            let mut bytes = vec![0x9c, 0x41, 0x5a];
            move_register(&mut bytes, 10, 15, true);
            move_register(&mut bytes, 15, *destination, true);
            return Ok(bytes);
        }
        if operation == Operation::SystemWrite(SystemRegister::Rflags) {
            let [source] = registers else {
                return Err(Error::OperandCountMismatch);
            };
            if *source > 15 || *source == 4 {
                return Err(Error::EncodedFormMismatch);
            }
            let mut bytes = Vec::new();
            move_register(&mut bytes, *source, 10, true);
            move_register(&mut bytes, 10, 15, true);
            bytes.extend([0x41, 0x57, 0x9d]);
            return Ok(bytes);
        }
        let [first, second] = registers else {
            return Err(Error::OperandCountMismatch);
        };
        if registers.iter().any(|register| *register > 15)
            || (operation.produces_value() && *second == 4)
        {
            return Err(Error::EncodedFormMismatch);
        }
        let mut bytes = Vec::new();
        if !operation.produces_value() && *second == 10 && *first != 10 {
            if *first == 11 {
                move_register(&mut bytes, 11, 0, true);
                move_register(&mut bytes, 10, 11, true);
                move_register(&mut bytes, 0, 10, true);
            } else {
                move_register(&mut bytes, 10, 11, true);
                move_register(&mut bytes, *first, 10, true);
            }
        } else {
            move_register(&mut bytes, *first, 10, true);
            if !operation.produces_value() {
                move_register(&mut bytes, *second, 11, true);
            }
        }
        match operation {
            Operation::PortRead => {
                move_register(&mut bytes, 10, 2, false);
                bytes.push(0xec);
                bytes.extend([
                    0x48 | ((*second >> 3) << 2),
                    0x0f,
                    0xb6,
                    0xc0 | ((*second & 7) << 3),
                ]);
            }
            Operation::PortWrite => {
                move_register(&mut bytes, 10, 2, false);
                move_register(&mut bytes, 11, 0, false);
                bytes.push(0xee);
            }
            Operation::ModelSpecificRead => {
                move_register(&mut bytes, 10, 1, false);
                bytes.extend([0x0f, 0x32]);
                swap_bytes(&mut bytes, 2, false);
                swap_bytes(&mut bytes, 2, true);
                bytes.extend([
                    0x48 | ((*second >> 3) << 2),
                    0x8d,
                    0x04 | ((*second & 7) << 3),
                    0x10,
                ]);
            }
            Operation::ModelSpecificWrite => {
                move_register(&mut bytes, 10, 1, false);
                move_register(&mut bytes, 11, 2, true);
                swap_bytes(&mut bytes, 2, true);
                swap_bytes(&mut bytes, 2, false);
                move_register(&mut bytes, 11, 0, false);
                bytes.extend([0x0f, 0x30]);
            }
            _ => return Err(Error::AlternativeMismatch),
        }
        Ok(bytes)
    }

    #[derive(Clone, Copy)]
    enum Decoded {
        Control {
            register: ControlRegister,
            operand: u8,
            read: bool,
        },
        PushFlags,
        PopFlags,
        Push {
            source: u8,
        },
        Pop {
            destination: u8,
        },
        Move {
            source: u8,
            destination: u8,
            bits: usize,
        },
        Swap {
            register: u8,
            bytes: usize,
        },
        Join {
            destination: u8,
        },
        Physical(Operation),
    }

    fn decode(bytes: &[u8]) -> Option<Vec<Decoded>> {
        let mut cursor = 0;
        let mut decoded = Vec::new();
        while cursor < bytes.len() {
            if bytes[cursor] == 0x41 && matches!(bytes.get(cursor + 1), Some(0x50..=0x57)) {
                decoded.push(Decoded::Push {
                    source: 8 | (bytes[cursor + 1] & 7),
                });
                cursor += 2;
                continue;
            }
            if bytes[cursor] == 0x41 && matches!(bytes.get(cursor + 1), Some(0x58..=0x5f)) {
                decoded.push(Decoded::Pop {
                    destination: 8 | (bytes[cursor + 1] & 7),
                });
                cursor += 2;
                continue;
            }
            match bytes[cursor] {
                0x9c => {
                    decoded.push(Decoded::PushFlags);
                    cursor += 1;
                    continue;
                }
                0x9d => {
                    decoded.push(Decoded::PopFlags);
                    cursor += 1;
                    continue;
                }
                0x50..=0x57 => {
                    decoded.push(Decoded::Push {
                        source: bytes[cursor] & 7,
                    });
                    cursor += 1;
                    continue;
                }
                0x58..=0x5f => {
                    decoded.push(Decoded::Pop {
                        destination: bytes[cursor] & 7,
                    });
                    cursor += 1;
                    continue;
                }
                0xf4 => {
                    decoded.push(Decoded::Physical(Operation::ProcessorHalt));
                    cursor += 1;
                    continue;
                }
                0xfa | 0xfb => {
                    decoded.push(Decoded::Physical(Operation::InterruptControl(
                        if bytes[cursor] == 0xfa {
                            InterruptControlKind::Disable
                        } else {
                            InterruptControlKind::Enable
                        },
                    )));
                    cursor += 1;
                    continue;
                }
                0xec | 0xee => {
                    decoded.push(Decoded::Physical(if bytes[cursor] == 0xec {
                        Operation::PortRead
                    } else {
                        Operation::PortWrite
                    }));
                    cursor += 1;
                    continue;
                }
                0x0f => {
                    decoded.push(match *bytes.get(cursor + 1)? {
                        0x32 => Decoded::Physical(Operation::ModelSpecificRead),
                        0x30 => Decoded::Physical(Operation::ModelSpecificWrite),
                        opcode @ 0xc8..=0xcf => Decoded::Swap {
                            register: opcode & 7,
                            bytes: 4,
                        },
                        _ => return None,
                    });
                    cursor += 2;
                    continue;
                }
                0x40..=0x4f => {}
                _ => return None,
            }
            let rex = bytes[cursor];
            let opcode = *bytes.get(cursor + 1)?;
            let operand = *bytes.get(cursor + 2)?;
            match opcode {
                0x0f if matches!(operand, 0x20 | 0x22) && rex == 0x41 => {
                    let addressing = *bytes.get(cursor + 3)?;
                    if addressing >> 6 != 3 {
                        return None;
                    }
                    let register = match (addressing >> 3) & 7 {
                        0 => ControlRegister::Cr0,
                        2 => ControlRegister::Cr2,
                        3 => ControlRegister::Cr3,
                        4 => ControlRegister::Cr4,
                        _ => return None,
                    };
                    decoded.push(Decoded::Control {
                        register,
                        operand: 8 | (addressing & 7),
                        read: operand == 0x20,
                    });
                    cursor += 4;
                }
                0x89 if operand >> 6 == 3 && rex & 2 == 0 => {
                    decoded.push(Decoded::Move {
                        source: ((operand >> 3) & 7) | ((rex & 4) << 1),
                        destination: (operand & 7) | ((rex & 1) << 3),
                        bits: if rex & 8 != 0 { 64 } else { 32 },
                    });
                    cursor += 3;
                }
                0x0f if operand & 0xf8 == 0xc8 && rex & 6 == 0 => {
                    decoded.push(Decoded::Swap {
                        register: (operand & 7) | ((rex & 1) << 3),
                        bytes: if rex & 8 != 0 { 8 } else { 4 },
                    });
                    cursor += 3;
                }
                0x0f if operand == 0xb6 && rex & 0xb == 8 => {
                    let addressing = *bytes.get(cursor + 3)?;
                    if addressing & 0xc7 != 0xc0 {
                        return None;
                    }
                    decoded.push(Decoded::Move {
                        source: 0,
                        destination: ((addressing >> 3) & 7) | ((rex & 4) << 1),
                        bits: 8,
                    });
                    cursor += 4;
                }
                0x8d if operand & 0xc7 == 4
                    && rex & 0xb == 8
                    && *bytes.get(cursor + 3)? == 0x10 =>
                {
                    decoded.push(Decoded::Join {
                        destination: ((operand >> 3) & 7) | ((rex & 4) << 1),
                    });
                    cursor += 4;
                }
                _ => return None,
            }
        }
        Some(decoded)
    }

    fn validate_bits(operation: Operation, registers: &[u8], bytes: &[u8]) -> Option<()> {
        if matches!(
            operation,
            Operation::TaskRegisterActivate
                | Operation::TaskRegisterRead
                | Operation::SegmentReload(_)
                | Operation::SegmentRead(_)
        ) {
            return descriptor_registers::validate(operation, registers, bytes);
        }

        if let Operation::GlobalDescriptorTable(direction) = operation {
            let [pointer] = registers else {
                return None;
            };
            let [rex, 0x89, register_move, 0x41, 0x0f, 0x01, descriptor] = bytes else {
                return None;
            };
            let source = ((register_move >> 3) & 7) | ((rex & 4) << 1);
            let destination = (register_move & 7) | ((rex & 1) << 3);
            let selector = match direction {
                DescriptorOperation::Store => 0x02,
                DescriptorOperation::Load => 0x12,
            };
            return (*rex & 0xfa == 0x48
                && register_move & 0xc0 == 0xc0
                && source == *pointer
                && destination == 10
                && *descriptor == selector)
                .then_some(());
        }

        if let Operation::MemoryFence(kind) = operation {
            return (registers.is_empty() && super::memory_fence::decode(bytes) == Some(kind))
                .then_some(());
        }

        if let Operation::SystemRead(SystemRegister::Control(control))
        | Operation::SystemWrite(SystemRegister::Control(control)) = operation
        {
            let [operand] = registers else {
                return None;
            };
            if *operand > 15
                || (operation.produces_value() && *operand == 4)
                || !operation.is_admitted()
            {
                return None;
            }
            return if operation.produces_value() {
                matches!(decode(bytes)?.as_slice(), [
                Decoded::Control { register, operand: 10, read: true },
                Decoded::Move { source: 10, destination: 15, bits: 64 },
                Decoded::Move { source: 15, destination, bits: 64 },
            ] if *register == control && destination == operand)
                .then_some(())
            } else {
                matches!(decode(bytes)?.as_slice(), [
                Decoded::Move { source, destination: 10, bits: 64 },
                Decoded::Control { register, operand: 10, read: false },
            ] if *register == control && source == operand)
                .then_some(())
            };
        }
        if matches!(
            operation,
            Operation::ProcessorHalt | Operation::InterruptControl(_)
        ) {
            return (registers.is_empty()
                && matches!(
                    decode(bytes)?.as_slice(),
                    [Decoded::Physical(actual)] if *actual == operation
                ))
            .then_some(());
        }
        if operation == Operation::SystemRead(SystemRegister::Rflags) {
            let [destination] = registers else {
                return None;
            };
            if *destination > 15 || *destination == 4 {
                return None;
            }
            return matches!(decode(bytes)?.as_slice(), [
            Decoded::PushFlags, Decoded::Pop { destination: 10 },
            Decoded::Move { source: 10, destination: 15, bits: 64 },
            Decoded::Move { source: 15, destination: actual, bits: 64 },
        ] if actual == destination)
            .then_some(());
        }
        if operation == Operation::SystemWrite(SystemRegister::Rflags) {
            let [source] = registers else {
                return None;
            };
            if *source > 15 || *source == 4 {
                return None;
            }
            return matches!(decode(bytes)?.as_slice(), [
            Decoded::Move { source: actual, destination: 10, bits: 64 },
            Decoded::Move { source: 10, destination: 15, bits: 64 },
            Decoded::Push { source: 15 },
            Decoded::PopFlags,
        ] if actual == source)
            .then_some(());
        }
        const ZERO: u16 = u16::MAX;
        const RESULT: u16 = 1024;
        let mut words = std::array::from_fn::<_, 16, _>(|register| {
            std::array::from_fn::<_, 64, _>(|bit| (register * 64 + bit) as u16)
        });
        let initial = words;
        let first = *registers.first()? as usize;
        let second = *registers.get(1)? as usize;
        if first >= 16 || second >= 16 || (operation.produces_value() && second == 4) {
            return None;
        }
        let mut physical_count = 0;
        for instruction in decode(bytes)? {
            match instruction {
                Decoded::Move {
                    source,
                    destination,
                    bits,
                } => {
                    let source = words[source as usize];
                    words[destination as usize] = [ZERO; 64];
                    words[destination as usize][..bits].copy_from_slice(&source[..bits]);
                }
                Decoded::Swap { register, bytes } => {
                    let source = words[register as usize];
                    words[register as usize] = [ZERO; 64];
                    for byte in 0..bytes {
                        words[register as usize][byte * 8..byte * 8 + 8]
                            .copy_from_slice(&source[(bytes - 1 - byte) * 8..(bytes - byte) * 8]);
                    }
                }
                Decoded::Join { destination } => {
                    if words[0][32..].iter().any(|bit| *bit != ZERO)
                        || words[2][..32].iter().any(|bit| *bit != ZERO)
                    {
                        return None;
                    }
                    let mut result = words[0];
                    result[32..].copy_from_slice(&words[2][32..]);
                    words[destination as usize] = result;
                }
                Decoded::PushFlags
                | Decoded::PopFlags
                | Decoded::Push { .. }
                | Decoded::Pop { .. }
                | Decoded::Control { .. } => return None,
                Decoded::Physical(actual) => {
                    if actual != operation {
                        return None;
                    }
                    physical_count += 1;
                    match actual {
                        Operation::PortRead | Operation::PortWrite => {
                            if words[2][..16] != initial[first][..16] {
                                return None;
                            }
                            if actual == Operation::PortWrite {
                                if words[0][..8] != initial[second][..8] {
                                    return None;
                                }
                            } else {
                                for (bit, origin) in words[0][..8].iter_mut().enumerate() {
                                    *origin = RESULT + bit as u16;
                                }
                            }
                        }
                        Operation::ModelSpecificRead | Operation::ModelSpecificWrite => {
                            if words[1][..32] != initial[first][..32] {
                                return None;
                            }
                            if actual == Operation::ModelSpecificWrite {
                                if words[0][..32] != initial[second][..32]
                                    || words[2][..32] != initial[second][32..]
                                {
                                    return None;
                                }
                            } else {
                                words[0] = [ZERO; 64];
                                words[2] = [ZERO; 64];
                                for bit in 0..32 {
                                    words[0][bit] = RESULT + bit as u16;
                                    words[2][bit] = RESULT + 32 + bit as u16;
                                }
                            }
                        }
                        _ => return None,
                    }
                }
            }
        }
        if physical_count != 1 {
            return None;
        }
        if let Some(carrier) = operation.result_carrier() {
            let bits = carrier.bits() as usize;
            for (bit, origin) in words[second].iter().enumerate() {
                if *origin
                    != if bit < bits {
                        RESULT + bit as u16
                    } else {
                        ZERO
                    }
                {
                    return None;
                }
            }
        }
        let clobbered = |register: usize| match operation {
            Operation::PortRead => [0, 2, 10].contains(&register),
            Operation::PortWrite => [0, 2, 10, 11].contains(&register),
            _ => [0, 1, 2, 10, 11].contains(&register),
        };
        for register in 0..16 {
            if !clobbered(register)
                && !(operation.produces_value() && register == second)
                && words[register] != initial[register]
            {
                return None;
            }
        }
        Some(())
    }

    pub(crate) fn footprint(
        operation: Operation,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        X86_64SelectedFormFootprint {
            register_reads: operands[..operation.input_carriers().len()].to_vec(),
            register_writes: if operation.produces_value() {
                vec![operands[operation.input_carriers().len()]]
            } else {
                vec![]
            },
            writes_rflags: matches!(
                operation,
                Operation::SystemWrite(SystemRegister::Rflags)
                    | Operation::InterruptControl(_)
                    | Operation::TaskRegisterActivate
                    | Operation::SegmentReload(_)
            ),
            encoded: effects(operation),
        }
    }

    pub(crate) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        operation: Operation,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let kind = SelectedInstructionKind::RegisterAccess { operation };
        super::request_validation::validate_request(physical, kind, alternative, operands)?;
        let registers =
            super::request_validation::resolve_scalar_registers(physical, kind, operands)?;
        validate_bits(operation, &registers, bytes).ok_or(Error::EncodedFormMismatch)?;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: footprint(operation, operands),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn control_register_transfers_preserve_selector_direction_and_scalar_operand() {
            let vectors = [
                (ControlRegister::Cr0, 0xc2),
                (ControlRegister::Cr2, 0xd2),
                (ControlRegister::Cr3, 0xda),
                (ControlRegister::Cr4, 0xe2),
            ];
            for (control, selector_byte) in vectors {
                for read in [true, false] {
                    let register = SystemRegister::Control(control);
                    let operation = if read {
                        Operation::SystemRead(register)
                    } else {
                        Operation::SystemWrite(register)
                    };
                    if !operation.is_admitted() {
                        assert!(encode(operation, &[3]).is_err());
                        continue;
                    }
                    for operand in 0..16 {
                        if read && operand == 4 {
                            assert!(encode(operation, &[operand]).is_err());
                            continue;
                        }
                        let bytes = encode(operation, &[operand]).unwrap();
                        let physical = [0x41, 0x0f, if read { 0x20 } else { 0x22 }, selector_byte];
                        assert_eq!(if read { &bytes[..4] } else { &bytes[3..] }, physical);
                        assert!(validate_bits(operation, &[operand], &bytes).is_some());
                        for byte_index in 0..bytes.len() {
                            for bit in 0..8 {
                                let mut changed = bytes.clone();
                                changed[byte_index] ^= 1 << bit;
                                assert!(
                                    validate_bits(operation, &[operand], &changed).is_none(),
                                    "{operation:?} operand {operand} byte {byte_index} bit {bit}"
                                );
                            }
                            assert!(
                                validate_bits(operation, &[operand], &bytes[..byte_index])
                                    .is_none()
                            );
                        }
                        let mut extended = bytes.clone();
                        extended.push(0x90);
                        assert!(validate_bits(operation, &[operand], &extended).is_none());
                        let encoded = effects(operation);
                        assert_eq!(
                            encoded.memory,
                            MachineEncodedEffects::fallthrough_v1(vec![], vec![]).memory
                        );
                        assert_eq!(
                            encoded.stack,
                            MachineEncodedEffects::fallthrough_v1(vec![], vec![]).stack
                        );
                        assert_eq!(
                            declaration(
                                operation,
                                if read {
                                    crate::X86_64_CONTROL_REGISTER_READ
                                } else {
                                    crate::X86_64_CONTROL_REGISTER_WRITE
                                }
                            )
                            .barrier,
                            MachineBarrier::ExternalEffect
                        );
                    }
                }
            }
        }

        #[test]
        fn flags_snapshot_has_one_balanced_full_width_observation() {
            let operation = Operation::SystemRead(SystemRegister::Rflags);
            for destination in 0..16 {
                if destination == 4 {
                    assert!(encode(operation, &[destination]).is_err());
                    continue;
                }
                let bytes = encode(operation, &[destination]).unwrap();
                assert_eq!(bytes.len(), 9);
                assert!(validate_bits(operation, &[destination], &bytes).is_some());
                for end in 0..bytes.len() {
                    assert!(validate_bits(operation, &[destination], &bytes[..end]).is_none());
                }
                for index in 0..bytes.len() {
                    for bit in 0..8 {
                        let mut changed = bytes.clone();
                        changed[index] ^= 1 << bit;
                        assert!(
                            validate_bits(operation, &[destination], &changed).is_none(),
                            "destination {destination}, byte {index}, bit {bit}"
                        );
                    }
                }
                let mut doubled = bytes.clone();
                doubled.extend(&bytes);
                assert!(validate_bits(operation, &[destination], &doubled).is_none());
            }
            let encoded = effects(operation);
            assert!(matches!(
                encoded.memory,
                MachineEncodedMemoryEffect::TemporaryStackStorageV1 { byte_count: 8, .. }
            ));
            assert!(matches!(
                encoded.stack,
                MachineEncodedStackEffect::TemporaryBytesV1 { byte_count: 8, .. }
            ));
        }

        #[test]
        fn flags_restore_reads_one_saved_place_and_balances_the_stack() {
            let operation = Operation::SystemWrite(SystemRegister::Rflags);
            for source in 0..16 {
                if source == 4 {
                    assert!(encode(operation, &[source]).is_err());
                    continue;
                }
                let bytes = encode(operation, &[source]).unwrap();
                assert_eq!(bytes.len(), 9);
                assert!(validate_bits(operation, &[source], &bytes).is_some());
                for end in 0..bytes.len() {
                    assert!(validate_bits(operation, &[source], &bytes[..end]).is_none());
                }
            }
            let encoded = effects(operation);
            assert!(matches!(
                encoded.memory,
                MachineEncodedMemoryEffect::TemporaryStackStorageV1 { byte_count: 8, .. }
            ));
            assert!(!encoded.implicit_unit_defs.is_empty());
        }

        #[test]
        fn processor_halt_is_one_operand_free_hardware_event() {
            let operation = Operation::ProcessorHalt;
            assert_eq!(encode(operation, &[]).unwrap(), [0xf4]);
            assert!(encode(operation, &[0]).is_err());
            assert!(validate_bits(operation, &[], &[0xf4]).is_some());
            for bytes in [&[][..], &[0x90], &[0xf4, 0xf4], &[0xf4, 0x90], &[0xee]] {
                assert!(validate_bits(operation, &[], bytes).is_none());
            }
            assert!(validate_bits(operation, &[0], &[0xf4]).is_none());
            let encoded = effects(operation);
            assert!(encoded.implicit_unit_clobbers.is_empty());
            let declaration = declaration(operation, crate::X86_64_PROCESSOR_HALT);
            assert_eq!(declaration.barrier, MachineBarrier::ExternalEffect);
            assert_eq!(
                declaration.trap,
                MachineTrapBehavior::MayArchitecturalFaultV1
            );
            assert_eq!(
                declaration.alternatives[0].size,
                MachineSizeKnowledge::ExactBytes(1)
            );
        }

        #[test]
        fn interrupt_control_retains_direction_without_operands_or_hidden_effects() {
            use InterruptControlKind::{Disable, Enable};
            for (kind, opcode, wrong) in [(Disable, 0xfa, 0xfb), (Enable, 0xfb, 0xfa)] {
                let operation = Operation::InterruptControl(kind);
                assert_eq!(encode(operation, &[]).unwrap(), [opcode]);
                assert!(encode(operation, &[0]).is_err());
                assert!(validate_bits(operation, &[], &[opcode]).is_some());
                for bytes in [
                    &[][..],
                    &[wrong],
                    &[0x90],
                    &[opcode, opcode],
                    &[opcode, 0x90],
                    &[0xf4],
                ] {
                    assert!(validate_bits(operation, &[], bytes).is_none());
                }
                assert!(validate_bits(operation, &[0], &[opcode]).is_none());
                let encoded = effects(operation);
                assert!(encoded.implicit_unit_clobbers.is_empty());
                assert!(!encoded.implicit_unit_defs.is_empty());
                assert_eq!(encoded.implicit_unit_uses, encoded.implicit_unit_defs);
                let declaration = declaration(operation, crate::X86_64_INTERRUPT_CONTROL);
                assert_eq!(declaration.barrier, MachineBarrier::ExternalEffect);
                assert_eq!(
                    declaration.alternatives[0].size,
                    MachineSizeKnowledge::ExactBytes(1)
                );
            }
        }

        #[test]
        fn every_operand_alias_preserves_port_and_model_specific_bits() {
            for operation in [
                Operation::PortRead,
                Operation::PortWrite,
                Operation::ModelSpecificRead,
                Operation::ModelSpecificWrite,
            ] {
                for first in 0..16 {
                    for second in 0..16 {
                        if operation.produces_value() && second == 4 {
                            assert!(encode(operation, &[first, second]).is_err());
                            continue;
                        }
                        let bytes = encode(operation, &[first, second]).unwrap();
                        assert!(
                            validate_bits(operation, &[first, second], &bytes).is_some(),
                            "{operation:?} {first} {second}: {bytes:02x?}"
                        );
                        assert!(bytes.len() <= 40);
                        for end in 0..bytes.len() {
                            assert!(
                                validate_bits(operation, &[first, second], &bytes[..end]).is_none(),
                                "accepted truncated {operation:?} at {end}"
                            );
                        }
                        let reversed = match operation {
                            Operation::PortRead => Operation::PortWrite,
                            Operation::PortWrite => Operation::PortRead,
                            Operation::ModelSpecificRead => Operation::ModelSpecificWrite,
                            Operation::ModelSpecificWrite => Operation::ModelSpecificRead,
                            _ => unreachable!(),
                        };
                        assert!(validate_bits(reversed, &[first, second], &bytes).is_none());
                    }
                }
            }
        }

        #[test]
        fn decoder_rejects_lost_indices_values_and_result_halves() {
            let registers = [11, 10];
            for operation in [
                Operation::PortRead,
                Operation::PortWrite,
                Operation::ModelSpecificRead,
                Operation::ModelSpecificWrite,
            ] {
                let bytes = encode(operation, &registers).unwrap();
                let mut changed = bytes.clone();
                changed[2] ^= 8;
                assert!(validate_bits(operation, &registers, &changed).is_none());
                let mut doubled = bytes.clone();
                doubled.extend(&bytes);
                assert!(validate_bits(operation, &registers, &doubled).is_none());
            }
            let mut read = encode(Operation::ModelSpecificRead, &[3, 12]).unwrap();
            let swap = read
                .windows(2)
                .position(|window| window == [0x0f, 0xca])
                .unwrap();
            read.drain(swap..swap + 2);
            assert!(validate_bits(Operation::ModelSpecificRead, &[3, 12], &read).is_none());
        }
    }

    #[cfg(test)]
    mod encoding_vectors {
        use super::*;

        #[test]
        fn matches_independent_llvm_mc_vectors() {
            assert_eq!(
                encode(Operation::PortRead, &[3, 12]).unwrap(),
                [
                    0x49, 0x89, 0xda, 0x44, 0x89, 0xd2, 0xec, 0x4c, 0x0f, 0xb6, 0xe0
                ]
            );
            assert_eq!(
                encode(Operation::ModelSpecificWrite, &[11, 10]).unwrap(),
                [
                    0x4c, 0x89, 0xd8, 0x4d, 0x89, 0xd3, 0x49, 0x89, 0xc2, 0x44, 0x89, 0xd1, 0x4c,
                    0x89, 0xda, 0x48, 0x0f, 0xca, 0x0f, 0xca, 0x44, 0x89, 0xd8, 0x0f, 0x30
                ]
            );
            assert_eq!(
                encode(Operation::ModelSpecificRead, &[3, 12]).unwrap(),
                [
                    0x49, 0x89, 0xda, 0x44, 0x89, 0xd1, 0x0f, 0x32, 0x0f, 0xca, 0x48, 0x0f, 0xca,
                    0x4c, 0x8d, 0x24, 0x10
                ]
            );
        }
    }
}
mod scan_byte_sequence {
    use super::{
        MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedMemoryEffect,
        RegisterViewId, ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError,
    };
    use crate::selected_form_encoding::instruction_bytes::{
        append_increment, append_indexed_byte_load, append_register_compare, append_self_clear,
        append_self_test, assemble_labeled_stream, modrm, rex,
    };
    use crate::selected_form_encoding::request_validation::byte_loop_registers;
    use selected_instructions::ByteSequencePredicate;

    const CC_B: u8 = 0x82;
    const CC_AE: u8 = 0x83;
    const CC_E: u8 = 0x84;
    const CC_NE: u8 = 0x85;
    const CC_A: u8 = 0x87;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Label {
        Loop,
        Done,
        Fail,
        Advance1,
        Cont1,
        Cont2Any,
        E0Path,
        EdPath,
        Lead3,
        Cont3Any,
        F0Path,
        F4Path,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Instruction<T> {
        XorSelf(u8),
        Inc(u8),
        AddImmediate(u8, u8),
        TestSelf64(u8),
        TestSelf8(u8),
        TestImmediate8(u8, u8),
        CompareImmediate8(u8, u8),
        CompareRegister64 {
            source: u8,
            operand: u8,
        },
        LoadByte {
            value: u8,
            base: u8,
            index: u8,
            offset: u32,
        },
        SetNotEqual8(u8),
        LoadAddress {
            value: u8,
            base: u8,
            offset: u8,
        },
        JumpConditional {
            condition: u8,
            target: T,
        },
        Jump {
            target: T,
        },
    }

    fn emit_compare_immediate_branch(
        emit: &mut (impl FnMut(Option<Label>, Instruction<Label>) + ?Sized),
        label: Option<Label>,
        register: u8,
        immediate: u8,
        condition: u8,
        target: Label,
    ) {
        emit(label, Instruction::CompareImmediate8(register, immediate));
        emit(None, Instruction::JumpConditional { condition, target });
    }

    macro_rules! t {
        ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr => $f:expr => $g:expr => $h:expr => $i:expr) => {
            emit_cont_tail($a, $b, $c, $d, $e, $f, $g, $h, $i)
        };
    }

    fn layout(
        predicate: ByteSequencePredicate,
        base: u8,
        count: u8,
        cursor: u8,
        byte: u8,
        result: u8,
    ) -> Vec<(Option<Label>, Instruction<Label>)> {
        use Instruction::*;
        let mut stream: Vec<(Option<Label>, Instruction<Label>)> = Vec::new();
        let mut emit = |label: Option<Label>, instruction: Instruction<Label>| {
            stream.push((label, instruction));
        };
        emit(None, XorSelf(cursor));
        emit(None, XorSelf(byte));
        emit(None, XorSelf(result));
        if predicate == ByteSequencePredicate::NonEmpty {
            emit(None, TestSelf64(count));
            emit(None, SetNotEqual8(result));
            return stream;
        }
        emit(None, Inc(result));
        emit(None, TestSelf64(count));
        emit(
            None,
            JumpConditional {
                condition: CC_E,
                target: Label::Done,
            },
        );
        if predicate == ByteSequencePredicate::ValidUtf8 {
            emit(
                Some(Label::Loop),
                CompareRegister64 {
                    source: count,
                    operand: cursor,
                },
            );
            emit(
                None,
                JumpConditional {
                    condition: CC_AE,
                    target: Label::Done,
                },
            );
            emit(
                None,
                LoadByte {
                    value: byte,
                    base,
                    index: cursor,
                    offset: 0,
                },
            );
            emit_compare_immediate_branch(&mut emit, None, byte, 0x80, CC_B, Label::Advance1);
            emit_compare_immediate_branch(&mut emit, None, byte, 0xC2, CC_B, Label::Fail);
            emit_compare_immediate_branch(&mut emit, None, byte, 0xE0, CC_B, Label::Cont1);
            emit_compare_immediate_branch(&mut emit, None, byte, 0xF0, CC_AE, Label::Lead3);
            emit_compare_immediate_branch(&mut emit, None, byte, 0xE0, CC_E, Label::E0Path);
            emit_compare_immediate_branch(&mut emit, None, byte, 0xED, CC_E, Label::EdPath);
            emit(
                None,
                Jump {
                    target: Label::Cont2Any,
                },
            );
            emit_compare_immediate_branch(
                &mut emit,
                Some(Label::Lead3),
                byte,
                0xF5,
                CC_AE,
                Label::Fail,
            );
            emit_compare_immediate_branch(&mut emit, None, byte, 0xF0, CC_E, Label::F0Path);
            emit_compare_immediate_branch(&mut emit, None, byte, 0xF4, CC_E, Label::F4Path);
            emit(
                None,
                Jump {
                    target: Label::Cont3Any,
                },
            );
            t!(&mut emit => Label::Cont1 => 1 => 0x80 => 0xBF => base => count => cursor => byte);
            t!(&mut emit => Label::Cont2Any => 2 => 0x80 => 0xBF => base => count => cursor => byte);
            t!(&mut emit => Label::E0Path => 2 => 0xA0 => 0xBF => base => count => cursor => byte);
            t!(&mut emit => Label::EdPath => 2 => 0x80 => 0x9F => base => count => cursor => byte);
            t!(&mut emit => Label::Cont3Any => 3 => 0x80 => 0xBF => base => count => cursor => byte);
            t!(&mut emit => Label::F0Path => 3 => 0x90 => 0xBF => base => count => cursor => byte);
            t!(&mut emit => Label::F4Path => 3 => 0x80 => 0x8F => base => count => cursor => byte);
            emit(Some(Label::Advance1), Inc(cursor));
            emit(
                None,
                Jump {
                    target: Label::Loop,
                },
            );
        } else {
            emit(
                Some(Label::Loop),
                LoadByte {
                    value: byte,
                    base,
                    index: cursor,
                    offset: 0,
                },
            );
            match predicate {
                ByteSequencePredicate::NoNul => {
                    emit(None, TestSelf8(byte));
                    emit(
                        None,
                        JumpConditional {
                            condition: CC_E,
                            target: Label::Fail,
                        },
                    );
                }
                ByteSequencePredicate::AsciiOnly => {
                    emit(None, TestImmediate8(byte, 0x80));
                    emit(
                        None,
                        JumpConditional {
                            condition: CC_NE,
                            target: Label::Fail,
                        },
                    );
                }
                _ => unreachable!(),
            }
            emit(None, Inc(cursor));
            emit(
                None,
                CompareRegister64 {
                    source: count,
                    operand: cursor,
                },
            );
            emit(
                None,
                JumpConditional {
                    condition: CC_B,
                    target: Label::Loop,
                },
            );
            emit(
                None,
                Jump {
                    target: Label::Done,
                },
            );
        }
        emit(Some(Label::Fail), XorSelf(result));
        emit(Some(Label::Done), XorSelf(cursor));
        stream
    }

    fn emit_cont_tail(
        emit: &mut dyn FnMut(Option<Label>, Instruction<Label>),
        label: Label,
        continuations: u8,
        lower: u8,
        upper: u8,
        base: u8,
        count: u8,
        cursor: u8,
        byte: u8,
    ) {
        use Instruction::*;
        emit(
            Some(label),
            LoadAddress {
                value: byte,
                base: cursor,
                offset: continuations,
            },
        );
        emit(
            None,
            CompareRegister64 {
                source: count,
                operand: byte,
            },
        );
        emit(
            None,
            JumpConditional {
                condition: CC_AE,
                target: Label::Fail,
            },
        );
        for offset in 1..=u32::from(continuations) {
            emit(
                None,
                LoadByte {
                    value: byte,
                    base,
                    index: cursor,
                    offset,
                },
            );
            let (cont_lower, cont_upper) = if offset == 1 {
                (lower, upper)
            } else {
                (0x80, 0xBF)
            };
            emit_compare_immediate_branch(emit, None, byte, cont_lower, CC_B, Label::Fail);
            emit_compare_immediate_branch(emit, None, byte, cont_upper, CC_A, Label::Fail);
        }
        emit(None, AddImmediate(cursor, continuations + 1));
        emit(
            None,
            Jump {
                target: Label::Loop,
            },
        );
    }

    fn rex8(register: u8) -> u8 {
        0x40 | (register >> 3)
    }

    fn rex8_pair(register: u8) -> u8 {
        0x40 | ((register >> 3) << 2) | (register >> 3)
    }

    fn emit_instruction(
        bytes: &mut Vec<u8>,
        instruction: Instruction<Label>,
        patches: &mut Vec<(usize, Label)>,
    ) {
        match instruction {
            Instruction::XorSelf(register) => append_self_clear(bytes, register),
            Instruction::Inc(register) => append_increment(bytes, register),
            Instruction::AddImmediate(register, immediate) => {
                bytes.extend([rex(0, 0, register), 0x83, modrm(3, 0, register), immediate])
            }
            Instruction::TestSelf64(register) => append_self_test(bytes, register),
            Instruction::TestSelf8(register) => {
                bytes.extend([rex8_pair(register), 0x84, modrm(3, register, register)])
            }
            Instruction::TestImmediate8(register, immediate) => {
                bytes.extend([rex8(register), 0xf6, modrm(3, 0, register), immediate])
            }
            Instruction::CompareImmediate8(register, immediate) => {
                bytes.extend([rex8(register), 0x80, modrm(3, 7, register), immediate])
            }
            Instruction::CompareRegister64 { source, operand } => {
                append_register_compare(bytes, source, operand)
            }
            Instruction::LoadByte {
                value,
                base,
                index,
                offset,
            } => append_indexed_byte_load(bytes, value, base, index, offset),
            Instruction::SetNotEqual8(register) => {
                bytes.extend([rex8(register), 0x0f, 0x95, modrm(3, 0, register)])
            }
            Instruction::LoadAddress {
                value,
                base,
                offset,
            } => {
                bytes.extend([rex(value, 0, base), 0x8d]);
                if base & 7 == 4 {
                    bytes.extend([modrm(1, value, 4), 0x24 | (base & 7), offset]);
                } else {
                    bytes.extend([modrm(1, value, base), offset]);
                }
            }
            Instruction::JumpConditional { condition, target } => {
                bytes.extend([0x0f, condition]);
                patches.push((bytes.len(), target));
                bytes.extend([0; 4]);
            }
            Instruction::Jump { target } => {
                bytes.push(0xe9);
                patches.push((bytes.len(), target));
                bytes.extend([0; 4]);
            }
        }
    }

    type Decoded = Instruction<usize>;

    fn decode_one(
        bytes: &[u8],
        position: usize,
    ) -> Result<(Decoded, usize), X86_64SelectedFormEncodingError> {
        let instruction = &bytes[position..];
        let fail = X86_64SelectedFormEncodingError::EncodedFormMismatch;
        let rex_fields = |prefix: u8, mode: u8, want_64: bool| -> Option<(u8, u8, u8)> {
            if want_64 {
                if prefix & 0xf8 != 0x48 {
                    return None;
                }
            } else if prefix & 0xf8 != 0x40 {
                return None;
            }
            Some((
                ((mode >> 3) & 7) | ((prefix & 4) << 1),
                (mode & 7) | ((prefix & 1) << 3),
                ((prefix & 2) << 2) | ((mode >> 3) & 7),
            ))
        };
        match *instruction {
            [prefix, 0x31, mode, ..] if prefix & 0xfa == 0x48 => {
                let (register, rm, _) = rex_fields(prefix, mode, true).unwrap();
                if mode >> 6 == 3 && register == rm {
                    Ok((Decoded::XorSelf(register), 3))
                } else {
                    Err(fail)
                }
            }
            [prefix, 0xff, mode, ..]
                if prefix & 0xfe == 0x48 && mode >> 6 == 3 && ((mode >> 3) & 7) == 0 =>
            {
                Ok((Decoded::Inc((mode & 7) | ((prefix & 1) << 3)), 3))
            }
            [prefix, 0x83, mode, immediate, ..]
                if prefix & 0xfe == 0x48 && mode >> 6 == 3 && ((mode >> 3) & 7) == 0 =>
            {
                Ok((
                    Decoded::AddImmediate((mode & 7) | ((prefix & 1) << 3), immediate),
                    4,
                ))
            }
            [prefix, 0x85, mode, ..] if prefix & 0xfa == 0x48 => {
                let (register, rm, _) = rex_fields(prefix, mode, true).unwrap();
                if mode >> 6 == 3 && register == rm {
                    Ok((Decoded::TestSelf64(register), 3))
                } else {
                    Err(fail)
                }
            }
            [prefix, 0x84, mode, ..] if prefix & 0xfa == 0x40 => {
                let (register, rm, _) = rex_fields(prefix, mode, false).unwrap();
                if mode >> 6 == 3 && register == rm {
                    Ok((Decoded::TestSelf8(register), 3))
                } else {
                    Err(fail)
                }
            }
            [prefix, 0xf6, mode, immediate, ..]
                if prefix & 0xfe == 0x40 && mode >> 6 == 3 && ((mode >> 3) & 7) == 0 =>
            {
                let (_, rm, _) = rex_fields(prefix, mode, false).unwrap();
                Ok((Decoded::TestImmediate8(rm, immediate), 4))
            }
            [prefix, 0x80, mode, immediate, ..]
                if prefix & 0xfe == 0x40 && mode >> 6 == 3 && ((mode >> 3) & 7) == 7 =>
            {
                let (_, rm, _) = rex_fields(prefix, mode, false).unwrap();
                Ok((Decoded::CompareImmediate8(rm, immediate), 4))
            }
            [prefix, 0x39, mode, ..] if prefix & 0xfa == 0x48 => {
                let (register, rm, _) = rex_fields(prefix, mode, true).unwrap();
                if mode >> 6 == 3 {
                    Ok((
                        Decoded::CompareRegister64 {
                            source: register,
                            operand: rm,
                        },
                        3,
                    ))
                } else {
                    Err(fail)
                }
            }
            [prefix, 0x0f, 0xb6, mode, sib, d0, d1, d2, d3, ..]
                if prefix & 0xf8 == 0x48 && mode & 0xc7 == 0x84 && sib & 0xc0 == 0 =>
            {
                let value = ((mode >> 3) & 7) | ((prefix & 4) << 1);
                let base = (sib & 7) | ((prefix & 1) << 3);
                let index = ((sib >> 3) & 7) | ((prefix & 2) << 2);
                if index == 4 {
                    return Err(fail);
                }
                Ok((
                    Decoded::LoadByte {
                        value,
                        base,
                        index,
                        offset: u32::from_le_bytes([d0, d1, d2, d3]),
                    },
                    9,
                ))
            }
            [prefix, 0x0f, 0x95, mode, ..]
                if prefix & 0xfe == 0x40 && mode >> 6 == 3 && ((mode >> 3) & 7) == 0 =>
            {
                let (_, rm, _) = rex_fields(prefix, mode, false).unwrap();
                Ok((Decoded::SetNotEqual8(rm), 4))
            }
            [prefix, 0x8d, mode, offset, ..]
                if prefix & 0xfa == 0x48 && mode >> 6 == 1 && mode & 7 != 4 =>
            {
                let value = ((mode >> 3) & 7) | ((prefix & 4) << 1);
                let base = (mode & 7) | ((prefix & 1) << 3);
                Ok((
                    Decoded::LoadAddress {
                        value,
                        base,
                        offset,
                    },
                    4,
                ))
            }
            [prefix, 0x8d, mode, sib, offset, ..]
                if prefix & 0xfa == 0x48
                    && mode >> 6 == 1
                    && mode & 7 == 4
                    && sib & 0xf8 == 0x20 =>
            {
                let value = ((mode >> 3) & 7) | ((prefix & 4) << 1);
                let base = (sib & 7) | ((prefix & 1) << 3);
                Ok((
                    Decoded::LoadAddress {
                        value,
                        base,
                        offset,
                    },
                    5,
                ))
            }
            [
                0x0f,
                condition @ (0x82 | 0x83 | 0x84 | 0x85 | 0x87),
                d0,
                d1,
                d2,
                d3,
                ..,
            ] => {
                let relative = i32::from_le_bytes([d0, d1, d2, d3]);
                let target = position as i64 + 6 + i64::from(relative);
                let target = usize::try_from(target)
                    .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
                if target > bytes.len() {
                    return Err(fail);
                }
                Ok((Decoded::JumpConditional { condition, target }, 6))
            }
            [0xe9, d0, d1, d2, d3, ..] => {
                let relative = i32::from_le_bytes([d0, d1, d2, d3]);
                let target = position as i64 + 5 + i64::from(relative);
                let target = usize::try_from(target)
                    .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
                if target > bytes.len() {
                    return Err(fail);
                }
                Ok((Decoded::Jump { target }, 5))
            }
            _ => Err(fail),
        }
    }

    fn decode_all_stream(bytes: &[u8]) -> Result<Vec<Decoded>, X86_64SelectedFormEncodingError> {
        if bytes.is_empty() {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let mut decoded = Vec::new();
        let mut position = 0;
        while position < bytes.len() {
            let (instruction, length) = decode_one(bytes, position)?;
            decoded.push(instruction);
            position += length;
        }
        Ok(decoded)
    }

    fn expected_stream(
        stream: &[(Option<Label>, Instruction<Label>)],
    ) -> Result<Vec<Decoded>, X86_64SelectedFormEncodingError> {
        let mut offsets: Vec<(Label, usize)> = Vec::new();
        let mut position = 0;
        for (label, instruction) in stream {
            if let Some(label) = label {
                offsets.push((*label, position));
            }
            position += instruction_size(*instruction);
        }
        let position_of = |label: Label| {
            offsets
                .iter()
                .find(|(candidate, _)| *candidate == label)
                .map(|(_, position)| *position)
                .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)
        };
        stream
            .iter()
            .map(|(_, instruction)| match *instruction {
                Instruction::XorSelf(r) => Ok(Decoded::XorSelf(r)),
                Instruction::Inc(r) => Ok(Decoded::Inc(r)),
                Instruction::AddImmediate(r, i) => Ok(Decoded::AddImmediate(r, i)),
                Instruction::TestSelf64(r) => Ok(Decoded::TestSelf64(r)),
                Instruction::TestSelf8(r) => Ok(Decoded::TestSelf8(r)),
                Instruction::TestImmediate8(r, i) => Ok(Decoded::TestImmediate8(r, i)),
                Instruction::CompareImmediate8(r, i) => Ok(Decoded::CompareImmediate8(r, i)),
                Instruction::CompareRegister64 { source, operand } => {
                    Ok(Decoded::CompareRegister64 { source, operand })
                }
                Instruction::LoadByte {
                    value,
                    base,
                    index,
                    offset,
                } => Ok(Decoded::LoadByte {
                    value,
                    base,
                    index,
                    offset,
                }),
                Instruction::SetNotEqual8(r) => Ok(Decoded::SetNotEqual8(r)),
                Instruction::LoadAddress {
                    value,
                    base,
                    offset,
                } => Ok(Decoded::LoadAddress {
                    value,
                    base,
                    offset,
                }),
                Instruction::JumpConditional { condition, target } => {
                    Ok(Decoded::JumpConditional {
                        condition,
                        target: position_of(target)?,
                    })
                }
                Instruction::Jump { target } => Ok(Decoded::Jump {
                    target: position_of(target)?,
                }),
            })
            .collect()
    }

    fn instruction_size(instruction: Instruction<Label>) -> usize {
        match instruction {
            Instruction::XorSelf(_)
            | Instruction::Inc(_)
            | Instruction::TestSelf64(_)
            | Instruction::TestSelf8(_)
            | Instruction::CompareRegister64 { .. } => 3,
            Instruction::AddImmediate(_, _)
            | Instruction::TestImmediate8(_, _)
            | Instruction::CompareImmediate8(_, _)
            | Instruction::SetNotEqual8(_)
            | Instruction::LoadAddress { .. } => 4,
            Instruction::JumpConditional { .. } => 6,
            Instruction::Jump { .. } => 5,
            Instruction::LoadByte { .. } => 9,
        }
    }

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        predicate: ByteSequencePredicate,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let [base, count, cursor, byte, result] =
            request(physical, alternative, operands, displacement)?;
        let bytes = assemble_labeled_stream(
            &layout(predicate, base, count, cursor, byte, result),
            emit_instruction,
        )?;
        validate(
            physical,
            predicate,
            alternative,
            operands,
            displacement,
            &bytes,
        )
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        predicate: ByteSequencePredicate,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let [base, count, cursor, byte, result] =
            request(physical, alternative, operands, displacement)?;
        let expected = expected_stream(&layout(predicate, base, count, cursor, byte, result))?;
        if decode_all_stream(bytes)? != expected {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        super::canonical_effects_encoding(
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

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<[u8; 5], X86_64SelectedFormEncodingError> {
        let registers =
            byte_loop_registers::<5>(physical, alternative, Family::ScanByteSequence, operands, 2)?;
        if displacement != 0 || registers[2] == 4 || registers[3] == 4 {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(registers)
    }

    #[cfg(test)]
    mod tests {
        use super::super::SelectedInstructionKind as Kind;
        use super::super::tests::{
            assert_byte_loop_catalog_contract,
            assert_loop_form_round_trips_and_rejects_every_changed_bit,
        };
        use super::{Family, MachineAlternativeKey, encode, validate};
        use selected_instructions::ByteSequencePredicate;

        fn kind(predicate: ByteSequencePredicate) -> Kind {
            Kind::ScanByteSequence { predicate }
        }

        #[test]
        fn scan_byte_sequence_catalog_requires_early_clobbers_and_memory_effect() {
            assert_byte_loop_catalog_contract(
                crate::X86_64_SCAN_BYTE_SEQUENCE,
                &[2, 3],
                selected_instructions::MachineSemanticKind::ScanByteSequence,
                selected_instructions::MachineMemoryEffect::ScanByteSequenceV1,
                &[0, 1],
                &[2, 3, 4],
            );
        }

        #[test]
        fn scan_byte_sequence_round_trips_and_rejects_every_changed_bit() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let alternative = MachineAlternativeKey {
                family: Family::ScanByteSequence,
                variant: 0,
            };
            for predicate in ByteSequencePredicate::ALL {
                assert_loop_form_round_trips_and_rejects_every_changed_bit(
                    &physical,
                    kind(predicate),
                    alternative,
                    &[
                        &["r13", "r14", "r15", "r12", "rbp"],
                        &["rax", "rcx", "rdx", "rsi", "rdi"],
                    ],
                    |operands, displacement| {
                        encode(&physical, predicate, alternative, operands, displacement)
                    },
                    |operands, bytes| {
                        validate(&physical, predicate, alternative, operands, 0, bytes).is_err()
                    },
                );
            }
        }
    }
}
mod zero_bytes {
    use super::{
        MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedMemoryEffect,
        RegisterViewId, ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError,
    };
    use crate::selected_form_encoding::copy_bytes::register_instruction;
    use crate::selected_form_encoding::instruction_bytes::{
        append_increment, append_register_compare, append_self_clear, append_self_test, modrm, rex,
    };
    use crate::selected_form_encoding::request_validation::byte_loop_registers;

    const ENCODED_BYTES: usize = 25;
    const STORE: usize = 8;

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let [destination, count, cursor] = request(physical, alternative, operands)?;
        let mut bytes = Vec::with_capacity(ENCODED_BYTES);
        append_self_clear(&mut bytes, cursor);
        append_self_test(&mut bytes, count);
        bytes.extend([0x74, (ENCODED_BYTES - STORE) as u8]);
        bytes.extend_from_slice(&[
            rex(0, cursor, destination) & !8,
            0xc6,
            modrm(2, 0, 4),
            ((cursor & 7) << 3) | (destination & 7),
            0,
            0,
            0,
            0,
            0,
        ]);
        append_increment(&mut bytes, cursor);
        append_register_compare(&mut bytes, count, cursor);
        bytes.extend([0x72, (STORE as i8 - ENCODED_BYTES as i8) as u8]);
        validate(physical, alternative, operands, &bytes)
    }

    pub(super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let [destination, count, cursor] = request(physical, alternative, operands)?;
        if bytes.len() != ENCODED_BYTES
            || !register_instruction(&bytes[0..3], 0x31, cursor, cursor)
            || !register_instruction(&bytes[3..6], 0x85, count, count)
            || bytes[6] != 0x74
            || 8_i16 + i16::from(bytes[7] as i8) != ENCODED_BYTES as i16
            || !zero_store(&bytes[STORE..17], destination, cursor)
            || bytes[17] & 0xfe != 0x48
            || bytes[18] != 0xff
            || bytes[19] & 0xf8 != 0xc0
            || (bytes[19] & 7) | ((bytes[17] & 1) << 3) != cursor
            || !register_instruction(&bytes[20..23], 0x39, count, cursor)
            || bytes[23] != 0x72
            || ENCODED_BYTES as i16 + i16::from(bytes[24] as i8) != STORE as i16
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        super::canonical_effects_encoding(
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

    fn zero_store(bytes: &[u8], base: u8, cursor: u8) -> bool {
        let [prefix, 0xc6, mode, sib, displacement @ .., 0] = bytes else {
            return false;
        };
        *prefix & 0xf8 == 0x40
            && *prefix & 4 == 0
            && *mode == 0x84
            && sib >> 6 == 0
            && displacement == [0; 4]
            && (sib & 7) | ((prefix & 1) << 3) == base
            && ((sib >> 3) & 7) | ((prefix & 2) << 2) == cursor
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<[u8; 3], X86_64SelectedFormEncodingError> {
        byte_loop_registers::<3>(physical, alternative, Family::ZeroBytes, operands, 2)
    }

    #[cfg(test)]
    mod tests {
        use super::super::{
            SelectedInstructionKind as Kind, encode_x86_64_selected_form,
            validate_x86_64_selected_form_encoding,
        };
        use super::{Family, MachineAlternativeKey, encode, validate};

        #[test]
        fn zero_bytes_catalog_requires_the_early_clobber_cursor_and_exact_dynamic_effect() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let catalog = crate::x86_64_register_constraint_catalog(&physical);
            let constraints =
                crate::validate_x86_64_register_constraint_catalog(catalog.clone(), &physical)
                    .unwrap();
            crate::selected_form_encoding::tests::assert_early_clobber_required(
                &catalog,
                &physical,
                crate::X86_64_ZERO_BYTES,
                &[2],
            );
            for target in [
                target::NativeTarget::linux_x64(),
                target::NativeTarget::windows_x64(),
            ] {
                let effects = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
                crate::validate_x86_64_machine_effect_catalog(
                    target,
                    &constraints,
                    effects.clone(),
                )
                .unwrap();
                let row = effects
                    .declaration_by_semantic(selected_instructions::MachineSemanticKind::ZeroBytes)
                    .unwrap();
                assert_eq!(
                    row.memory,
                    selected_instructions::MachineMemoryEffect::ZeroBytesV1
                );
                assert_eq!(row.alternatives[0].encoded.external_operand_reads, [0, 1]);
                assert_eq!(row.alternatives[0].encoded.external_operand_writes, [2]);
            }
        }

        #[test]
        fn zero_bytes_preserves_inputs_and_rejects_every_changed_encoding_bit() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let alternative = MachineAlternativeKey {
                family: Family::ZeroBytes,
                variant: 0,
            };
            let operands =
                ["rdi", "rcx", "rax"].map(|name| physical.model().view_named(name).unwrap().id);
            assert_eq!(
                encode(&physical, alternative, &operands).unwrap().bytes(),
                [
                    0x48, 0x31, 0xc0, 0x48, 0x85, 0xc9, 0x74, 0x11, 0x40, 0xc6, 0x84, 0x07, 0, 0,
                    0, 0, 0, 0x48, 0xff, 0xc0, 0x48, 0x39, 0xc8, 0x72, 0xef,
                ]
            );
            for names in [
                ["r13", "r12", "r15"],
                ["rax", "rcx", "rdx"],
                ["rbp", "r8", "r12"],
            ] {
                let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
                let encoded = encode(&physical, alternative, &operands).unwrap();
                assert_eq!(
                    encode_x86_64_selected_form(&physical, Kind::ZeroBytes, alternative, &operands)
                        .unwrap(),
                    encoded
                );
                validate_x86_64_selected_form_encoding(
                    &physical,
                    Kind::ZeroBytes,
                    alternative,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), 25);
                assert_eq!(encoded.footprint().register_reads, operands[..2]);
                assert_eq!(encoded.footprint().register_writes, operands[2..]);
                crate::selected_form_encoding::assert_every_bit_flip_rejected(
                    encoded.bytes(),
                    |changed| validate(&physical, alternative, &operands, changed).is_err(),
                );
                super::super::assert_byte_fragility(encoded.bytes(), |bytes| {
                    validate(&physical, alternative, &operands, bytes).is_err()
                });
                for operand in 0..3 {
                    let mut changed = operands;
                    changed[operand] = physical.model().view_named("rsp").unwrap().id;
                    assert!(encode(&physical, alternative, &changed).is_err());
                }
                for input in 0..2 {
                    let mut changed = operands;
                    changed[2] = operands[input];
                    assert!(encode(&physical, alternative, &changed).is_err());
                }
            }
        }
    }
}
pub(crate) mod hosted_exit_process {
    use super::{
        RegisterViewId, ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
    };
    use crate::selected_form_encoding::request_validation::resolve_registers;
    use register_model::RegisterConstraintKey;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineEffectDeclaration,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
        MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior, SelectedInstructionKind,
    };
    use target::NativeTarget;

    pub(crate) fn effects(
        _target: NativeTarget,
    ) -> Result<MachineEncodedEffects, X86_64SelectedFormEncodingError> {
        let physical = crate::x86_64_physical_register_model();
        let view = |name: &str| {
            physical
                .view_named(name)
                .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)
        };
        let mut clobbers = Vec::new();
        for name in ["rax", "rdi", "rcx", "r11", "rflags"] {
            clobbers.extend(view(name)?.units.iter().copied());
        }
        clobbers.sort_unstable();
        clobbers.dedup();
        Ok(MachineEncodedEffects {
            external_operand_reads: vec![0],
            external_operand_writes: Vec::new(),
            implicit_unit_uses: view("rip")?.units.clone(),
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
    ) -> Result<MachineEffectDeclaration, X86_64SelectedFormEncodingError> {
        Ok(MachineEffectDeclaration::single(
            MachineSemanticKind::HostedExitProcessI32,
            constraint,
            MachineMemoryEffect::NoneV1,
            MachineTrapBehavior::HostedExitReturnedV1,
            MachineBarrier::ExternalEffect,
            MachineAlternativeFamily::HostedExitProcessI32,
            MachineSizeKnowledge::ExactBytes(12),
            effects(target)?,
        ))
    }

    fn request(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<u8, X86_64SelectedFormEncodingError> {
        if crate::hosted_exit_syscall_number(target).is_none()
            || physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
            || kind != SelectedInstructionKind::HostedExitProcessI32
            || alternative
                != (MachineAlternativeKey {
                    family: MachineAlternativeFamily::HostedExitProcessI32,
                    variant: 0,
                })
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let registers = resolve_registers(physical, operands)?;
        let [register] = registers.as_slice() else {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        };
        Ok(*register)
    }

    pub fn encode_x86_64_selected_hosted_exit_process_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let register = request(target, physical, kind, alternative, operands)?;
        let syscall = crate::hosted_exit_syscall_number(target)
            .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        let mut bytes = vec![
            0x40 | ((register >> 3) << 2),
            0x89,
            0xc7 | ((register & 7) << 3),
            0xb8,
        ];
        crate::codec_bytes::push_encoded(&mut bytes, syscall.to_le_bytes());
        bytes.extend_from_slice(&[0x0f, 0x05, 0x0f, 0x0b]);
        validate_x86_64_selected_hosted_exit_process_form(
            target,
            physical,
            kind,
            alternative,
            operands,
            &bytes,
        )
    }

    pub fn validate_x86_64_selected_hosted_exit_process_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let register = request(target, physical, kind, alternative, operands)?;
        if decode(target, bytes) != Some(register) {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                writes_rflags: true,
                encoded: effects(target)?,
            },
        })
    }

    fn decode(target: NativeTarget, bytes: &[u8]) -> Option<u8> {
        let syscall = crate::hosted_exit_syscall_number(target)?;
        if bytes.len() != 12 {
            return None;
        }
        let register = ((bytes[2] >> 3) & 7) | ((bytes[0] & 4) << 1);
        if bytes[0] & 0xfb != 0x40
            || bytes[1] != 0x89
            || bytes[2] & 0xc7 != 0xc7
            || register == 4
            || bytes[3] != 0xb8
            || bytes[4..8] != syscall.to_le_bytes()
            || bytes[8..] != [0x0f, 0x05, 0x0f, 0x0b]
        {
            return None;
        }
        Some(register)
    }

    pub fn decode_x86_64_selected_hosted_exit_process_i32(
        target: NativeTarget,
        bytes: &[u8],
    ) -> Option<calling_conventions::MachineRegister> {
        let register = decode(target, bytes)?;
        use calling_conventions::MachineRegister::*;
        let registers = [
            X86Rax, X86Rcx, X86Rdx, X86Rbx, X86Rsp, X86Rbp, X86Rsi, X86Rdi, X86R8, X86R9, X86R10,
            X86R11, X86R12, X86R13, X86R14, X86R15,
        ];
        Some(registers[usize::from(register)])
    }

    #[cfg(test)]
    mod tests {
        use super::{
            NativeTarget, decode_x86_64_selected_hosted_exit_process_i32, effects,
            encode_x86_64_selected_hosted_exit_process_form,
            validate_x86_64_selected_hosted_exit_process_form,
        };
        use register_model::RegisterViewId;
        use selected_instructions::{
            MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
        };
        #[test]
        fn hosted_exit_replays_target_input_and_every_encoded_bit() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let kind = SelectedInstructionKind::HostedExitProcessI32;
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedExitProcessI32,
                variant: 0,
            };
            let encode = |target, kind, operands: &[RegisterViewId]| {
                encode_x86_64_selected_hosted_exit_process_form(
                    target,
                    &physical,
                    kind,
                    alternative,
                    operands,
                )
            };
            let validate = |target, operands: &[RegisterViewId], bytes: &[u8]| {
                validate_x86_64_selected_hosted_exit_process_form(
                    target,
                    &physical,
                    kind,
                    alternative,
                    operands,
                    bytes,
                )
            };
            for target in [NativeTarget::linux_x64(), NativeTarget::macos_x64()] {
                for name in ["rax", "rdi", "rcx", "r8", "r11", "r15"] {
                    let operands = [physical.model().view_named(name).unwrap().id];
                    let encoded = encode(target, kind, &operands).unwrap();
                    assert_eq!(encoded.footprint().encoded, effects(target).unwrap());
                    crate::selected_form_encoding::assert_every_bit_flip_rejected(
                        encoded.bytes(),
                        |corrupted| validate(target, &operands, corrupted).is_err(),
                    );
                    let different = [physical
                        .model()
                        .view_named(if name == "rax" { "rdi" } else { "rax" })
                        .unwrap()
                        .id];
                    assert!(validate(target, &different, encoded.bytes()).is_err());
                    assert!(encode(target, kind, &[]).is_err());
                    assert!(encode(target, kind, &[operands[0], operands[0]]).is_err());
                    assert!(
                        encode(target, SelectedInstructionKind::ReturnUnit, &operands).is_err()
                    );
                    let other_kernel = if target == NativeTarget::linux_x64() {
                        NativeTarget::macos_x64()
                    } else {
                        NativeTarget::linux_x64()
                    };
                    for wrong_target in [
                        NativeTarget::windows_x64(),
                        NativeTarget {
                            pointer_size: 4,
                            ..target
                        },
                        NativeTarget::macos_arm64(),
                        other_kernel,
                    ] {
                        assert!(validate(wrong_target, &operands, encoded.bytes()).is_err());
                    }
                    let mut extra = encoded.bytes().to_vec();
                    extra.push(0);
                    assert!(
                        decode_x86_64_selected_hosted_exit_process_i32(target, &extra).is_none()
                    );
                }
            }
        }
    }
}
pub(crate) mod hosted_read_byte {
    use super::{
        RegisterViewId, ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
    };
    use crate::register_model::{units_of, view_of};
    use register_model::RegisterConstraintKey;
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier,
        MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
        MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
        SelectedInstructionKind,
    };
    use target::NativeTarget;

    fn leaf_bytes(target: NativeTarget) -> u16 {
        if target == NativeTarget::macos_x64() {
            crate::hosted_kernel::DARWIN_READ_BYTE_LEAF_BYTES as u16
        } else {
            62
        }
    }

    pub(crate) fn effects() -> MachineEncodedEffects {
        MachineEncodedEffects {
            external_operand_reads: Vec::new(),
            external_operand_writes: Vec::new(),
            implicit_unit_uses: units_of(&["rsp", "rip"]),
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: units_of(&["rax", "rdi", "rsi", "rdx", "rcx", "r11", "rflags"]),
            memory: MachineEncodedMemoryEffect::HostedReadByteV1 {
                stack_pointer: view_of("rsp").id,
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
            MachineSizeKnowledge::ExactBytes(leaf_bytes(target)),
            effects(),
        )
    }

    fn request(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<(), X86_64SelectedFormEncodingError> {
        if !crate::hosted_syscall_target(target)
            || physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
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
            || displacement > 2147483640
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(())
    }

    pub fn encode_x86_64_selected_hosted_read_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        request(target, physical, kind, alternative, operands, displacement)?;
        let displacement = u32::try_from(displacement)
            .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        let bytes = if target == NativeTarget::macos_x64() {
            crate::encode_darwin_read_byte_to_stack(displacement, displacement + 4)
        } else {
            crate::encode_linux_read_byte_to_stack(displacement, displacement + 4)
        }
        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        validate_x86_64_selected_hosted_read_byte_form(
            target,
            physical,
            kind,
            alternative,
            operands,
            i64::from(displacement),
            &bytes,
        )
    }

    pub fn validate_x86_64_selected_hosted_read_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        request(target, physical, kind, alternative, operands, displacement)?;
        if decode_x86_64_selected_hosted_read_byte(target, bytes).map(i64::from)
            != Some(displacement)
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: Vec::new(),
                register_writes: Vec::new(),
                writes_rflags: true,
                encoded: effects(),
            },
        })
    }

    pub fn decode_x86_64_selected_hosted_read_byte(
        target: NativeTarget,
        bytes: &[u8],
    ) -> Option<u32> {
        if target == NativeTarget::macos_x64() {
            return crate::hosted_kernel::decode_darwin_read_byte(bytes);
        }
        if target != NativeTarget::linux_x64() {
            return None;
        }
        decode_linux_read_byte(bytes)
    }

    fn decode_linux_read_byte(bytes: &[u8]) -> Option<u32> {
        if bytes.len() != 62 {
            return None;
        }
        let home = crate::codec_bytes::u32_at(bytes, 5)?;
        let payload = home.checked_add(4)?;
        if !home.is_multiple_of(4)
            || home > 2147483640
            || bytes[..5] != [0x31, 0xc0, 0x89, 0x84, 0x24]
            || bytes[9..12] != [0x89, 0x84, 0x24]
            || crate::codec_bytes::u32_at(bytes, 12)? != payload
            || bytes[16..22] != [0x31, 0xff, 0x48, 0x8d, 0xb4, 0x24]
            || crate::codec_bytes::u32_at(bytes, 22)? != payload
            || bytes[26..50]
                != [
                    0xba, 1, 0, 0, 0, 0x31, 0xc0, 0x0f, 0x05, 0x48, 0x83, 0xf8, 0, 0x74, 21, 0x48,
                    0x83, 0xf8, 1, 0x75, 13, 0xc7, 0x84, 0x24,
                ]
            || crate::codec_bytes::u32_at(bytes, 50)? != home
            || bytes[54..] != [1, 0, 0, 0, 0xeb, 2, 0x0f, 0x0b]
        {
            return None;
        }
        Some(home)
    }

    #[cfg(test)]
    mod tests {
        use super::{
            NativeTarget, decode_x86_64_selected_hosted_read_byte, effects,
            encode_x86_64_selected_hosted_read_byte_form,
            validate_x86_64_selected_hosted_read_byte_form,
        };
        use register_model::RegisterViewId;
        use selected_instructions::{
            LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey,
            MachineEncodedStackEffect, MachineSemanticKind, SelectedInstructionKind,
        };
        use semantic_vocabulary::{OperationId, PlaceId};

        #[test]
        fn read_byte_replays_every_bit_and_rejects_wrong_home_source_slot_and_alternative() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
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
                    encode_x86_64_selected_hosted_read_byte_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        source,
                        i64::from(displacement),
                    )
                };
            let validate = |target, displacement: u32, bytes: &[u8]| {
                validate_x86_64_selected_hosted_read_byte_form(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &[],
                    i64::from(displacement),
                    bytes,
                )
            };
            for (target, other_kernel, width) in [
                (NativeTarget::linux_x64(), NativeTarget::macos_x64(), 62),
                (NativeTarget::macos_x64(), NativeTarget::linux_x64(), 67),
            ] {
                for displacement in [0, 32, 2147483640] {
                    let encoded = encode(target, kind, alternative, &[], displacement).unwrap();
                    assert_eq!(
                        decode_x86_64_selected_hosted_read_byte(target, encoded.bytes()),
                        Some(displacement)
                    );
                    assert_eq!(encoded.footprint().encoded, effects());
                    assert_eq!(encoded.bytes().len(), width);
                    assert_eq!(
                        decode_x86_64_selected_hosted_read_byte(other_kernel, encoded.bytes()),
                        None
                    );
                    assert!(encoded.footprint().register_reads.is_empty());
                    assert!(encoded.footprint().register_writes.is_empty());
                    assert_eq!(
                        encoded.footprint().encoded.stack,
                        MachineEncodedStackEffect::UnchangedV1
                    );
                    for bit in 0..encoded.bytes().len() * 8 {
                        let mut changed = encoded.bytes().to_vec();
                        changed[bit / 8] ^= 1 << (bit % 8);
                        assert!(
                            validate(target, displacement, &changed).is_err(),
                            "bit {bit}"
                        );
                        assert_eq!(
                            decode_x86_64_selected_hosted_read_byte(target, &changed),
                            None,
                            "decoder bit {bit}"
                        );
                    }
                    for invalid_offset in [1, 3, 2147483644, u32::MAX] {
                        assert!(encode(target, kind, alternative, &[], invalid_offset).is_err());
                    }
                    assert!(validate(target, displacement ^ 4, encoded.bytes()).is_err());
                    let boundary = SelectedInstructionKind::HostedReadByte {
                        slot: LocalStorageSlotId::Boundary { operation },
                    };
                    assert!(encode(target, boundary, alternative, &[], displacement).is_err());
                    let source = [physical.model().view_named("rax").unwrap().id];
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
                        decode_x86_64_selected_hosted_read_byte(target, &padded),
                        None
                    );
                }
            }
        }

        #[test]
        fn read_catalog_retains_complete_zero_operand_constraint_and_effect_clobbers() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let constraints = crate::validate_x86_64_register_constraint_catalog(
                crate::x86_64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap();
            for (target, width) in [
                (target::NativeTarget::linux_x64(), 62),
                (target::NativeTarget::macos_x64(), 67),
            ] {
                let catalog = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
                let validated = crate::validate_x86_64_machine_effect_catalog(
                    target,
                    &constraints,
                    catalog.clone(),
                )
                .unwrap();
                let declaration = validated
                    .catalog()
                    .declaration_by_semantic(MachineSemanticKind::HostedReadByte)
                    .unwrap();
                assert_eq!(declaration.constraint, crate::X86_64_HOSTED_READ_BYTE);
                assert_eq!(declaration.alternatives[0].encoded, effects());
                assert_eq!(
                    declaration.alternatives[0].size,
                    selected_instructions::MachineSizeKnowledge::ExactBytes(width)
                );
                let mut damaged = catalog;
                let row = damaged
                    .declaration_by_semantic_mut(MachineSemanticKind::HostedReadByte)
                    .unwrap();
                row.alternatives[0].encoded.implicit_unit_clobbers.pop();
                assert!(
                    crate::validate_x86_64_machine_effect_catalog(target, &constraints, damaged)
                        .is_err()
                );
            }
        }
    }
}
pub(crate) mod hosted_write_byte {
    use super::{
        RegisterViewId, ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
    };
    use crate::register_model::{units_of, view_of};
    use crate::selected_form_encoding::request_validation::resolve_registers;
    use register_model::RegisterConstraintKey;
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier,
        MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
        MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
        SelectedInstructionKind,
    };
    use target::NativeTarget;

    fn leaf_bytes(target: NativeTarget) -> u16 {
        if crate::hosted_kernel::hosted_kernel(target)
            .is_some_and(|kernel| kernel.carry_reports_failure)
        {
            42
        } else {
            40
        }
    }

    pub(crate) fn effects() -> MachineEncodedEffects {
        MachineEncodedEffects {
            external_operand_reads: vec![0],
            external_operand_writes: Vec::new(),
            implicit_unit_uses: units_of(&["rsp", "rip"]),
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: units_of(&["rax", "rdi", "rsi", "rdx", "rcx", "r11", "rflags"]),
            memory: MachineEncodedMemoryEffect::HostedWriteByteV1 {
                stack_pointer: view_of("rsp").id,
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
            MachineSizeKnowledge::ExactBytes(leaf_bytes(target)),
            effects(),
        )
    }

    fn request(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<u8, X86_64SelectedFormEncodingError> {
        if !crate::hosted_syscall_target(target)
            || physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
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
            || !(0..=i64::from(i32::MAX)).contains(&displacement)
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let registers = resolve_registers(physical, operands)?;
        let [register] = registers.as_slice() else {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        };
        Ok(*register)
    }

    pub fn encode_x86_64_selected_hosted_write_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let register = request(target, physical, kind, alternative, operands, displacement)?;
        let kernel = crate::hosted_kernel::hosted_kernel(target)
            .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        let displacement = i32::try_from(displacement)
            .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        let mut bytes = vec![
            0x40 | ((register >> 3) << 2),
            0x88,
            0x84 | ((register & 7) << 3),
            0x24,
        ];
        crate::codec_bytes::push_encoded(&mut bytes, displacement.to_le_bytes());
        bytes.extend_from_slice(&[0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0xb4, 0x24]);
        crate::codec_bytes::push_encoded(&mut bytes, displacement.to_le_bytes());
        bytes.extend_from_slice(&[0xba, 1, 0, 0, 0, 0xb8]);
        crate::codec_bytes::push_encoded(&mut bytes, kernel.write.to_le_bytes());
        bytes.extend_from_slice(&[0x0f, 0x05]);
        if kernel.carry_reports_failure {
            bytes.extend_from_slice(&[0x72, 0x05]);
        }
        bytes.extend_from_slice(&[0x48, 0x85, 0xc0, 0x7f, 0x02, 0x0f, 0x0b]);
        validate_x86_64_selected_hosted_write_byte_form(
            target,
            physical,
            kind,
            alternative,
            operands,
            i64::from(displacement),
            &bytes,
        )
    }

    pub fn validate_x86_64_selected_hosted_write_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let register = request(target, physical, kind, alternative, operands, displacement)?;
        if decode(target, bytes).map(|(register, offset)| (register, i64::from(offset)))
            != Some((register, displacement))
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                writes_rflags: true,
                encoded: effects(),
            },
        })
    }

    fn decode(target: NativeTarget, bytes: &[u8]) -> Option<(u8, u32)> {
        let tail: &[u8] = if target == NativeTarget::linux_x64() {
            &[
                0xba, 1, 0, 0, 0, 0xb8, 1, 0, 0, 0, 0x0f, 0x05, 0x48, 0x85, 0xc0, 0x7f, 0x02, 0x0f,
                0x0b,
            ]
        } else if target == NativeTarget::macos_x64() {
            &[
                0xba, 1, 0, 0, 0, 0xb8, 4, 0, 0, 2, 0x0f, 0x05, 0x72, 0x05, 0x48, 0x85, 0xc0, 0x7f,
                0x02, 0x0f, 0x0b,
            ]
        } else {
            return None;
        };
        if bytes.len() != 21 + tail.len() {
            return None;
        }
        let stored_offset = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let addressed_offset = u32::from_le_bytes([bytes[17], bytes[18], bytes[19], bytes[20]]);
        let decoded_register = ((bytes[2] >> 3) & 7) | ((bytes[0] & 4) << 1);
        if bytes[0] & 0xfb != 0x40
            || bytes[1] != 0x88
            || bytes[2] & 0xc7 != 0x84
            || bytes[3] != 0x24
            || decoded_register == 4
            || stored_offset > i32::MAX as u32
            || addressed_offset != stored_offset
            || bytes[8..17] != [0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0xb4, 0x24]
            || bytes[21..] != *tail
        {
            return None;
        }
        Some((decoded_register, stored_offset))
    }

    pub fn decode_x86_64_selected_hosted_write_byte_i32(
        target: NativeTarget,
        bytes: &[u8],
    ) -> Option<(calling_conventions::MachineRegister, u32)> {
        let (register, displacement) = decode(target, bytes)?;
        use calling_conventions::MachineRegister::*;
        let registers = [
            X86Rax, X86Rcx, X86Rdx, X86Rbx, X86Rsp, X86Rbp, X86Rsi, X86Rdi, X86R8, X86R9, X86R10,
            X86R11, X86R12, X86R13, X86R14, X86R15,
        ];
        Some((registers[usize::from(register)], displacement))
    }

    #[cfg(test)]
    mod tests {
        use super::{
            NativeTarget, decode_x86_64_selected_hosted_write_byte_i32, effects,
            encode_x86_64_selected_hosted_write_byte_form,
            validate_x86_64_selected_hosted_write_byte_form,
        };
        use register_model::RegisterViewId;
        use selected_instructions::{
            LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey,
            MachineEncodedStackEffect, SelectedInstructionKind,
        };
        use semantic_vocabulary::{OperationId, PlaceId};

        #[test]
        fn hosted_write_byte_replays_source_frame_offset_syscall_failure_branch_and_every_bit() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
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
                encode_x86_64_selected_hosted_write_byte_form(
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
                    validate_x86_64_selected_hosted_write_byte_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        operands,
                        displacement,
                        bytes,
                    )
                };
            for (target, other_kernel, width) in [
                (NativeTarget::linux_x64(), NativeTarget::macos_x64(), 40),
                (NativeTarget::macos_x64(), NativeTarget::linux_x64(), 42),
            ] {
                for name in ["rax", "rbp", "rsi", "rdx", "r11", "r15"] {
                    let operands = [physical.model().view_named(name).unwrap().id];
                    for displacement in [0, 31, 2147483647] {
                        let encoded = encode(target, &operands, displacement).unwrap();
                        assert_eq!(encoded.footprint().encoded, effects());
                        assert_eq!(encoded.bytes().len(), width);
                        assert_eq!(
                            decode_x86_64_selected_hosted_write_byte_i32(target, encoded.bytes())
                                .map(|(_, offset)| i64::from(offset)),
                            Some(displacement)
                        );
                        assert!(
                            decode_x86_64_selected_hosted_write_byte_i32(
                                other_kernel,
                                encoded.bytes()
                            )
                            .is_none()
                        );
                        assert!(encoded.footprint().register_writes.is_empty());
                        assert_eq!(
                            encoded.footprint().encoded.stack,
                            MachineEncodedStackEffect::UnchangedV1
                        );
                        crate::selected_form_encoding::assert_every_bit_flip_rejected(
                            encoded.bytes(),
                            |changed| {
                                validate(target, kind, &operands, displacement, changed).is_err()
                            },
                        );
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
                        assert!(encode(target, &operands, 2147483648).is_err());
                        assert!(encode(target, &[], displacement).is_err());
                    }
                }
            }
            assert!(
                decode_x86_64_selected_hosted_write_byte_i32(NativeTarget::windows_x64(), &[0; 40])
                    .is_none()
            );
        }
    }
}
pub(crate) mod installed_interrupt {
    use super::{
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
        X86_64SelectedFormFootprint,
    };
    use crate::register_model::units_of;
    use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeKey, MachineBarrier, MachineEffectDeclaration,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
        MachineSemanticKind, MachineTrapBehavior, SelectedInstructionKind,
    };

    impl From<crate::X86InstalledTableEncodingError> for Error {
        fn from(error: crate::X86InstalledTableEncodingError) -> Self {
            Self::InstalledTable(Box::new(error))
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Form {
        CpuCheck,
        TaskCommit,
        TablePublication,
    }
    impl Form {
        pub(crate) fn from_kind(kind: SelectedInstructionKind) -> Option<Self> {
            Some(match kind {
                SelectedInstructionKind::InstalledInterruptCpuCheck { .. } => Self::CpuCheck,
                SelectedInstructionKind::InstalledInterruptTaskCommit { .. } => Self::TaskCommit,
                SelectedInstructionKind::InstalledInterruptTablePublication { .. } => {
                    Self::TablePublication
                }
                _ => return None,
            })
        }
        pub(crate) fn from_semantic(semantic: MachineSemanticKind) -> Option<Self> {
            Some(match semantic {
                MachineSemanticKind::InstalledInterruptCpuCheck => Self::CpuCheck,
                MachineSemanticKind::InstalledInterruptTaskCommit => Self::TaskCommit,
                MachineSemanticKind::InstalledInterruptTablePublication => Self::TablePublication,
                _ => return None,
            })
        }
        fn semantic(self) -> MachineSemanticKind {
            match self {
                Self::CpuCheck => MachineSemanticKind::InstalledInterruptCpuCheck,
                Self::TaskCommit => MachineSemanticKind::InstalledInterruptTaskCommit,
                Self::TablePublication => MachineSemanticKind::InstalledInterruptTablePublication,
            }
        }
        fn memory(self) -> MachineMemoryEffect {
            match self {
                Self::CpuCheck => MachineMemoryEffect::InstalledInterruptCpuCheckV1,
                Self::TaskCommit => MachineMemoryEffect::InstalledInterruptTaskCommitV1,
                Self::TablePublication => MachineMemoryEffect::InstalledInterruptTablePublicationV1,
            }
        }
        fn registers(self) -> &'static [u8] {
            match self {
                Self::CpuCheck => &[7, 6, 1, 0],
                Self::TaskCommit => &[7, 6, 1],
                Self::TablePublication => &[7, 1],
            }
        }
        fn input_count(self) -> usize {
            if self == Self::TablePublication { 2 } else { 3 }
        }
    }

    pub(crate) fn effects(form: Form) -> MachineEncodedEffects {
        let mut effects = MachineEncodedEffects::fallthrough_v1(
            (0..form.input_count() as u16).collect(),
            if form == Form::CpuCheck {
                vec![3]
            } else {
                Vec::new()
            },
        );
        effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        effects.implicit_unit_clobbers = units_of(match form {
            Form::CpuCheck => &["r9", "r10", "r11", "condition_flags"],
            Form::TaskCommit => &[
                "rax",
                "rdx",
                "r8",
                "r9",
                "r10",
                "r11",
                "condition_flags",
                "control_flags",
            ],
            Form::TablePublication => &["rax"],
        });
        if form == Form::TablePublication {
            effects.memory = MachineEncodedMemoryEffect::InstalledInterruptTablePublicationV1 {
                table_pointer_operand: 0,
                scratch_pointer_operand: 1,
            };
        } else {
            let stack_pointer = crate::x86_64_physical_register_model_ref()
                .view_named("rsp")
                .expect("canonical stack pointer")
                .id;
            effects.implicit_unit_uses = units_of(&["rsp", "control_flags"]);
            effects.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: 8,
            };
            effects.memory = if form == Form::CpuCheck {
                MachineEncodedMemoryEffect::InstalledInterruptCpuCheckV1 {
                    table_pointer_operand: 0,
                    task_pointer_operand: 1,
                    scratch_pointer_operand: 2,
                    stack_pointer,
                }
            } else {
                effects.control = MachineEncodedControlEffect::FallThroughOrStopV1;
                MachineEncodedMemoryEffect::InstalledInterruptTaskCommitV1 {
                    table_pointer_operand: 0,
                    task_pointer_operand: 1,
                    scratch_pointer_operand: 2,
                    stack_pointer,
                }
            };
        }
        effects
    }

    pub(crate) fn declaration(
        form: Form,
        constraint: RegisterConstraintKey,
    ) -> MachineEffectDeclaration {
        MachineEffectDeclaration::single(
            form.semantic(),
            constraint,
            form.memory(),
            MachineTrapBehavior::MayArchitecturalFaultV1,
            MachineBarrier::ExternalEffect,
            form.semantic().into(),
            crate::encoder_resolved(1, Some(2048)),
            effects(form),
        )
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<Form, Error> {
        let form = Form::from_kind(kind).ok_or(Error::AlternativeMismatch)?;
        super::request_validation::validate_request(physical, kind, alternative, operands)?;
        let registers =
            super::request_validation::resolve_scalar_registers(physical, kind, operands)?;
        if registers != form.registers() {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(form)
    }

    pub(crate) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        request(physical, kind, alternative, operands)?;
        let bytes = match kind {
            SelectedInstructionKind::InstalledInterruptCpuCheck { gdt_bytes } => {
                crate::encode_x86_64_installed_cpu_state_check(
                    crate::X86InstalledCpuStateRegisters::FIXED,
                    crate::X86InstalledCpuStateGeometry { gdt_bytes },
                )?
            }
            SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes } => {
                crate::encode_x86_64_installed_task_commit(
                    crate::X86InstalledTaskCommitRegisters::FIXED,
                    crate::X86InstalledCpuStateGeometry { gdt_bytes },
                )?
            }
            SelectedInstructionKind::InstalledInterruptTablePublication { table_bytes } => {
                crate::encode_x86_64_installed_table_publication(
                    crate::X86InstalledTablePublicationRegisters::FIXED,
                    table_bytes,
                )?
            }
            _ => return Err(Error::AlternativeMismatch),
        };
        validate(physical, kind, alternative, operands, &bytes)
    }

    pub(crate) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let form = request(physical, kind, alternative, operands)?;
        match kind {
            SelectedInstructionKind::InstalledInterruptCpuCheck { gdt_bytes } => {
                crate::validate_x86_64_installed_cpu_state_check(
                    crate::X86InstalledCpuStateRegisters::FIXED,
                    crate::X86InstalledCpuStateGeometry { gdt_bytes },
                    bytes,
                )?
            }
            SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes } => {
                crate::validate_x86_64_installed_task_commit(
                    crate::X86InstalledTaskCommitRegisters::FIXED,
                    crate::X86InstalledCpuStateGeometry { gdt_bytes },
                    bytes,
                )?;
            }
            SelectedInstructionKind::InstalledInterruptTablePublication { table_bytes } => {
                crate::validate_x86_64_installed_table_publication(
                    crate::X86InstalledTablePublicationRegisters::FIXED,
                    table_bytes,
                    bytes,
                )?
            }
            _ => return Err(Error::AlternativeMismatch),
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: footprint(form, operands),
        })
    }

    pub(crate) fn footprint(
        form: Form,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        X86_64SelectedFormFootprint {
            register_reads: operands[..form.input_count()].to_vec(),
            register_writes: if form == Form::CpuCheck {
                vec![operands[3]]
            } else {
                Vec::new()
            },
            writes_rflags: form != Form::TablePublication,
            encoded: effects(form),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn operands(form: Form) -> Vec<RegisterViewId> {
            let names: &[&str] = match form {
                Form::CpuCheck => &["rdi", "rsi", "rcx", "rax"],
                Form::TaskCommit => &["rdi", "rsi", "rcx"],
                Form::TablePublication => &["rdi", "rcx"],
            };
            let model = crate::validated_x86_64_physical_register_model();
            names
                .iter()
                .map(|name| model.model().view_named(name).unwrap().id)
                .collect()
        }

        #[test]
        fn installed_selected_forms_replay_exact_geometry_and_registers() {
            let model = crate::validated_x86_64_physical_register_model();
            let kinds = [
                SelectedInstructionKind::InstalledInterruptCpuCheck { gdt_bytes: 40 },
                SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes: 40 },
                SelectedInstructionKind::InstalledInterruptTablePublication { table_bytes: 1024 },
            ];
            for kind in kinds {
                let form = Form::from_kind(kind).unwrap();
                let registers = operands(form);
                let alternative = MachineAlternativeKey {
                    family: form.semantic().into(),
                    variant: 0,
                };
                let encoded =
                    crate::encode_x86_64_selected_form(model, kind, alternative, &registers)
                        .unwrap();
                assert!(
                    crate::validate_x86_64_selected_form_encoding(
                        model,
                        kind,
                        alternative,
                        &registers,
                        encoded.bytes()
                    )
                    .is_ok()
                );
                let changed = match kind {
                    SelectedInstructionKind::InstalledInterruptCpuCheck { .. } => {
                        SelectedInstructionKind::InstalledInterruptCpuCheck { gdt_bytes: 48 }
                    }
                    SelectedInstructionKind::InstalledInterruptTaskCommit { .. } => {
                        SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes: 48 }
                    }
                    _ => SelectedInstructionKind::InstalledInterruptTablePublication {
                        table_bytes: 1040,
                    },
                };
                assert!(
                    crate::validate_x86_64_selected_form_encoding(
                        model,
                        changed,
                        alternative,
                        &registers,
                        encoded.bytes()
                    )
                    .is_err()
                );
                let mut wrong_registers = registers.clone();
                wrong_registers[0] = model.model().view_named("rbx").unwrap().id;
                assert!(
                    crate::validate_x86_64_selected_form_encoding(
                        model,
                        kind,
                        alternative,
                        &wrong_registers,
                        encoded.bytes()
                    )
                    .is_err()
                );
            }
        }

        #[test]
        fn selected_commit_cannot_drop_its_refusal_stop() {
            let model = crate::validated_x86_64_physical_register_model();
            let kind = SelectedInstructionKind::InstalledInterruptTaskCommit { gdt_bytes: 40 };
            let alternative = MachineAlternativeKey {
                family: MachineSemanticKind::InstalledInterruptTaskCommit.into(),
                variant: 0,
            };
            let operands = operands(Form::TaskCommit);
            let encoded =
                crate::encode_x86_64_selected_form(model, kind, alternative, &operands).unwrap();
            let bytes = encoded.bytes();
            assert!(
                crate::validate_x86_64_selected_form_encoding(
                    model,
                    kind,
                    alternative,
                    &operands,
                    &bytes[..bytes.len() - crate::PRIVILEGED_STOP_BYTES.len()]
                )
                .is_err()
            );
            let mut reversed = bytes.to_vec();
            let stop = reversed.len() - crate::PRIVILEGED_STOP_BYTES.len();
            reversed[stop..].fill(0x90);
            assert!(
                crate::validate_x86_64_selected_form_encoding(
                    model,
                    kind,
                    alternative,
                    &operands,
                    &reversed
                )
                .is_err()
            );
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::FallThroughOrStopV1
            );
        }
    }
}
mod instruction_bytes {
    use crate::selected_form_encoding::X86_64SelectedFormEncodingError;
    use crate::selected_form_encoding::saturating_forms::SaturatingForm;
    use crate::selected_form_encoding::trapping_forms::{
        DividendGuard, RangeCheck, TrappingArithmetic, TrappingShape,
    };
    use selected_instructions::{
        MachineAlternativeKey, SaturatingCarrier, SaturatingOperation,
        SelectedInstructionKind as Kind, TrappingForm,
    };
    use semantic_vocabulary::IntegerValue;

    pub(crate) fn integer_bits(
        value: IntegerValue,
    ) -> Result<u64, X86_64SelectedFormEncodingError> {
        match value {
            IntegerValue::Signed(value) => i64::try_from(value)
                .map(|value| value as u64)
                .map_err(|_| X86_64SelectedFormEncodingError::IntegerOutsideI64Bits),
            IntegerValue::Unsigned(value) => u64::try_from(value)
                .map_err(|_| X86_64SelectedFormEncodingError::IntegerOutsideI64Bits),
        }
    }

    pub(crate) fn u12(value: IntegerValue) -> Result<u32, X86_64SelectedFormEncodingError> {
        match value {
            IntegerValue::Unsigned(value) if value <= 4095 => Ok(value as u32),
            _ => Err(X86_64SelectedFormEncodingError::ImmediateOutsideU12),
        }
    }

    pub(crate) fn rex(register: u8, index: u8, base: u8) -> u8 {
        0x48 | ((register >> 3) << 2) | ((index >> 3) << 1) | (base >> 3)
    }

    pub(crate) fn modrm(mode: u8, register: u8, rm: u8) -> u8 {
        (mode << 6) | ((register & 7) << 3) | (rm & 7)
    }

    pub(crate) fn assemble_labeled_stream<L: Copy + Eq, I: Copy>(
        stream: &[(Option<L>, I)],
        emit: impl Fn(&mut Vec<u8>, I, &mut Vec<(usize, L)>),
    ) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
        let mut bytes = Vec::new();
        let mut labels: Vec<(L, usize)> = Vec::new();
        let mut patches = Vec::new();
        for (label, instruction) in stream {
            if let Some(label) = label {
                labels.push((*label, bytes.len()));
            }
            emit(&mut bytes, *instruction, &mut patches);
        }
        for (offset, target) in &patches {
            let Some((_, position)) = labels.iter().find(|(label, _)| label == target) else {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            };
            let relative = *position as i64 - (*offset as i64 + 4);
            let relative = i32::try_from(relative)
                .map_err(|_| X86_64SelectedFormEncodingError::MalformedEncoding)?;
            crate::codec_bytes::patch_encoded(&mut bytes, *offset, relative.to_le_bytes());
        }
        Ok(bytes)
    }

    pub(crate) fn append_boolean(bytes: &mut Vec<u8>, destination: u8, condition: u8) {
        bytes.extend([
            0x40 | (destination >> 3),
            0x0f,
            0x90 | condition,
            0xc0 | (destination & 7),
            0x40 | ((destination >> 3) << 2) | (destination >> 3),
            0x0f,
            0xb6,
            0xc0 | ((destination & 7) << 3) | (destination & 7),
        ]);
    }

    fn append_register_binary(bytes: &mut Vec<u8>, opcode: u8, source: u8, destination: u8) {
        bytes.extend([
            rex(source, 0, destination),
            opcode,
            modrm(3, source, destination),
        ]);
    }

    pub(crate) fn append_self_clear(bytes: &mut Vec<u8>, register: u8) {
        append_register_binary(bytes, 0x31, register, register);
    }

    pub(crate) fn append_increment(bytes: &mut Vec<u8>, register: u8) {
        bytes.extend([rex(0, 0, register), 0xff, modrm(3, 0, register)]);
    }

    pub(crate) fn append_self_test(bytes: &mut Vec<u8>, register: u8) {
        append_register_binary(bytes, 0x85, register, register);
    }

    pub(crate) fn append_register_compare(bytes: &mut Vec<u8>, source: u8, operand: u8) {
        append_register_binary(bytes, 0x39, source, operand);
    }

    pub(crate) fn append_indexed_byte_load(
        bytes: &mut Vec<u8>,
        value: u8,
        base: u8,
        index: u8,
        offset: u32,
    ) {
        bytes.extend([
            rex(value, index, base),
            0x0f,
            0xb6,
            modrm(2, value, 4),
            ((index & 7) << 3) | (base & 7),
        ]);
        bytes.extend(offset.to_le_bytes());
    }

    const CMOVG: u8 = 0x4f;
    const CMOVL: u8 = 0x4c;
    const CMOVA: u8 = 0x47;

    fn append_clamp(bytes: &mut Vec<u8>, value: u8, scratch: u8, bound_bits: u64, condition: u8) {
        bytes.extend([0x48 | (scratch >> 3), 0xb8 | (scratch & 7)]);
        bytes.extend(bound_bits.to_le_bytes());
        append_register_binary(bytes, 0x39, scratch, value);
        bytes.extend([
            rex(value, 0, scratch),
            0x0f,
            condition,
            modrm(3, value, scratch),
        ]);
    }

    fn append_signed_multiply(bytes: &mut Vec<u8>, source: u8, destination: u8) {
        bytes.extend([
            rex(destination, 0, source),
            0x0f,
            0xaf,
            modrm(3, destination, source),
        ]);
    }

    fn append_move_on_overflow(bytes: &mut Vec<u8>, source: u8, destination: u8) {
        bytes.extend([
            rex(destination, 0, source),
            0x0f,
            0x40,
            modrm(3, destination, source),
        ]);
    }

    fn append_signed_divide(bytes: &mut Vec<u8>, divisor: u8) {
        bytes.extend([0x48, 0x99]);
        bytes.extend([rex(0, 0, divisor), 0xf7, modrm(3, 7, divisor)]);
    }

    fn append_saturating(
        bytes: &mut Vec<u8>,
        operation: SaturatingOperation,
        carrier: SaturatingCarrier,
        registers: &[u8],
    ) -> Result<(), X86_64SelectedFormEncodingError> {
        let form = SaturatingForm::of(operation, carrier);
        if !form.accepts_registers(registers) {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let (left, right, result) = (registers[0], registers[1], registers[2]);
        let arithmetic = if operation == SaturatingOperation::Add {
            0x01
        } else {
            0x29
        };
        match form {
            SaturatingForm::SubtractUnsigned => {
                append_register_binary(bytes, 0x39, right, left);
                append_register_binary(bytes, 0x89, left, result);
                bytes.extend([rex(result, 0, right), 0x0f, 0x42, modrm(3, result, right)]);
                append_register_binary(bytes, 0x29, right, result);
            }
            SaturatingForm::AddU64 => {
                append_register_binary(bytes, 0x89, left, result);
                bytes.extend([rex(0, 0, result), 0xf7, modrm(3, 2, result)]);
                append_register_binary(bytes, 0x39, right, result);
                bytes.extend([rex(result, 0, right), 0x0f, 0x42, modrm(3, result, right)]);
                append_register_binary(bytes, 0x29, right, result);
                bytes.extend([rex(0, 0, result), 0xf7, modrm(3, 2, result)]);
            }
            SaturatingForm::ClampSignedNarrow | SaturatingForm::ClampUnsignedNarrow => {
                let scratch = registers[3];
                append_register_binary(bytes, 0x89, left, result);
                append_register_binary(bytes, arithmetic, right, result);
                append_clamp(bytes, result, scratch, carrier.maximum_bits(), CMOVG);
                if form == SaturatingForm::ClampSignedNarrow {
                    append_clamp(bytes, result, scratch, carrier.minimum_bits(), CMOVL);
                }
            }
            SaturatingForm::OverflowSelectI64 => {
                let scratch = registers[3];
                append_register_binary(bytes, 0x89, left, scratch);
                bytes.extend([rex(0, 0, scratch), 0xf7, modrm(3, 2, scratch)]);
                bytes.extend([rex(0, 0, scratch), 0xc1, modrm(3, 7, scratch), 63]);
                bytes.extend([rex(0, 0, scratch), 0x0f, 0xba, modrm(3, 7, scratch), 63]);
                append_register_binary(bytes, 0x89, left, result);
                append_register_binary(bytes, arithmetic, right, result);
                append_move_on_overflow(bytes, scratch, result);
            }
            SaturatingForm::DivideUnsigned => {
                bytes.extend([rex(0, 0, right), 0xf7, modrm(3, 6, right)]);
            }
            SaturatingForm::DivideSignedNarrow => {
                append_signed_divide(bytes, right);
                append_clamp(bytes, 0, 2, carrier.maximum_bits(), CMOVG);
            }
            SaturatingForm::DivideI64 => {
                bytes.extend([rex(0, 0, right), 0x83, modrm(3, 7, right), 0xff]);
                append_register_binary(bytes, 0x19, 2, 2);
                append_register_binary(bytes, 0x09, 0, 2);
                bytes.extend([rex(0, 0, 2), 0xf7, modrm(3, 3, 2)]);
                append_lea_immediate(bytes, 0, 2, 1);
                append_move_on_overflow(bytes, 2, 0);
                append_signed_divide(bytes, right);
            }
            SaturatingForm::RemainderUnsigned => {
                append_register_binary(bytes, 0x31, registers[3], registers[3]);
                bytes.extend([rex(0, 0, right), 0xf7, modrm(3, 6, right)]);
                append_register_binary(bytes, 0x89, registers[3], result);
            }
            SaturatingForm::RemainderSigned => {
                append_register_binary(bytes, 0x31, registers[3], registers[3]);
                bytes.extend([rex(0, 0, right), 0x83, modrm(3, 7, right), 0xff]);
                bytes.extend([0x74, 5, 0x48, 0x99]);
                bytes.extend([rex(0, 0, right), 0xf7, modrm(3, 7, right)]);
                append_register_binary(bytes, 0x89, registers[3], result);
            }
            SaturatingForm::MultiplySignedNarrow | SaturatingForm::MultiplyUnsignedNarrow => {
                let scratch = registers[3];
                append_register_binary(bytes, 0x89, left, result);
                append_signed_multiply(bytes, right, result);
                if form == SaturatingForm::MultiplySignedNarrow {
                    append_clamp(bytes, result, scratch, carrier.maximum_bits(), CMOVG);
                    append_clamp(bytes, result, scratch, carrier.minimum_bits(), CMOVL);
                } else {
                    append_clamp(bytes, result, scratch, carrier.maximum_bits(), CMOVA);
                }
            }
            SaturatingForm::MultiplyI64 => {
                let scratch = registers[3];
                append_register_binary(bytes, 0x89, left, scratch);
                append_register_binary(bytes, 0x31, right, scratch);
                bytes.extend([rex(0, 0, scratch), 0xf7, modrm(3, 2, scratch)]);
                bytes.extend([rex(0, 0, scratch), 0xc1, modrm(3, 7, scratch), 63]);
                bytes.extend([rex(0, 0, scratch), 0x0f, 0xba, modrm(3, 7, scratch), 63]);
                append_register_binary(bytes, 0x89, left, result);
                append_signed_multiply(bytes, right, result);
                append_move_on_overflow(bytes, scratch, result);
            }
            SaturatingForm::MultiplyU64 => {
                bytes.extend([rex(0, 0, right), 0xf7, modrm(3, 4, right)]);
                append_register_binary(bytes, 0x19, 2, 2);
                append_register_binary(bytes, 0x09, 2, 0);
            }
        }
        Ok(())
    }

    const JNO: u8 = 0x71;
    const JB: u8 = 0x72;
    const JAE: u8 = 0x73;
    const JE: u8 = 0x74;
    const JNE: u8 = 0x75;

    fn append_trap_unless(bytes: &mut Vec<u8>, condition: u8) {
        bytes.extend([condition, 2, 0x0f, 0x0b]);
    }

    fn append_sign_extension(bytes: &mut Vec<u8>, bits: u16, source: u8, destination: u8) {
        let opcode: &[u8] = match bits {
            8 => &[0x0f, 0xbe],
            16 => &[0x0f, 0xbf],
            _ => &[0x63],
        };
        bytes.push(rex(destination, 0, source));
        bytes.extend_from_slice(opcode);
        bytes.push(modrm(3, destination, source));
    }

    fn append_range_check(bytes: &mut Vec<u8>, check: RangeCheck, value: u8, scratch: u8) {
        match check {
            RangeCheck::SignExtension { bits } => {
                append_sign_extension(bytes, bits, value, scratch);
                append_register_binary(bytes, 0x39, value, scratch);
            }
            RangeCheck::HighBits { shift } => {
                append_register_binary(bytes, 0x89, value, scratch);
                bytes.extend([rex(0, 0, scratch), 0xc1, modrm(3, 5, scratch), shift]);
            }
        }
        append_trap_unless(bytes, JE);
    }

    fn append_shift_by_cl(bytes: &mut Vec<u8>, digit: u8, destination: u8) {
        bytes.extend([rex(0, 0, destination), 0xd3, modrm(3, digit, destination)]);
    }

    const fn right_shift_digit(signed: bool) -> u8 {
        if signed { 7 } else { 5 }
    }

    fn append_trapping_arithmetic(
        bytes: &mut Vec<u8>,
        arithmetic: TrappingArithmetic,
        registers: &[u8],
    ) {
        let (left, right, result) = (registers[0], registers[1], registers[2]);
        append_register_binary(bytes, 0x89, left, result);
        match arithmetic {
            TrappingArithmetic::Add => append_register_binary(bytes, 0x01, right, result),
            TrappingArithmetic::Subtract => append_register_binary(bytes, 0x29, right, result),
            TrappingArithmetic::Multiply => append_signed_multiply(bytes, right, result),
        }
    }

    fn append_shift_count_check(bytes: &mut Vec<u8>, width: u8, registers: &[u8]) {
        let (value, count, result) = (registers[0], registers[1], registers[2]);
        bytes.extend([rex(0, 0, count), 0x83, modrm(3, 7, count), width]);
        append_trap_unless(bytes, JB);
        append_register_binary(bytes, 0x89, value, result);
    }

    fn append_trapping_divide(bytes: &mut Vec<u8>, guard: Option<DividendGuard>, divisor: u8) {
        append_register_binary(bytes, 0x85, divisor, divisor);
        append_trap_unless(bytes, JNE);
        let Some(guard) = guard else {
            append_register_binary(bytes, 0x31, 2, 2);
            bytes.extend([rex(0, 0, divisor), 0xf7, modrm(3, 6, divisor)]);
            return;
        };
        bytes.extend([rex(0, 0, divisor), 0x83, modrm(3, 7, divisor), 0xff]);
        bytes.extend([JNE, guard.byte_count() as u8]);
        match guard {
            DividendGuard::Minimum8(minimum) => {
                bytes.extend([0x48, 0x83, 0xf8, minimum as u8]);
                append_trap_unless(bytes, JNE);
            }
            DividendGuard::Minimum32(minimum) => {
                bytes.extend([0x48, 0x3d]);
                bytes.extend(minimum.to_le_bytes());
                append_trap_unless(bytes, JNE);
            }
            DividendGuard::DecrementOverflows => {
                bytes.extend([0x48, 0x83, 0xf8, 1]);
                append_trap_unless(bytes, JNO);
            }
        }
        append_signed_divide(bytes, divisor);
    }

    fn append_trapping(
        bytes: &mut Vec<u8>,
        form: TrappingForm,
        registers: &[u8],
    ) -> Result<(), X86_64SelectedFormEncodingError> {
        let shape = TrappingShape::of(form);
        if !shape.accepts_registers(registers) {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        match shape {
            TrappingShape::OverflowFlag(arithmetic) => {
                append_trapping_arithmetic(bytes, arithmetic, registers);
                append_trap_unless(bytes, JNO);
            }
            TrappingShape::CarryFlag(arithmetic) => {
                append_trapping_arithmetic(bytes, arithmetic, registers);
                append_trap_unless(bytes, JAE);
            }
            TrappingShape::NarrowRange(arithmetic, check) => {
                append_trapping_arithmetic(bytes, arithmetic, registers);
                append_range_check(bytes, check, registers[2], registers[3]);
            }
            TrappingShape::MultiplyHighHalf => {
                bytes.extend([rex(0, 0, registers[1]), 0xf7, modrm(3, 4, registers[1])]);
                append_trap_unless(bytes, JNO);
            }
            TrappingShape::Division { guard, remainder } => {
                append_trapping_divide(bytes, guard, registers[1]);
                if remainder {
                    append_register_binary(bytes, 0x89, 2, 0);
                }
            }
            TrappingShape::ShiftLeftRoundTrip { signed } => {
                let (value, result, scratch) = (registers[0], registers[2], registers[3]);
                append_shift_count_check(bytes, 64, registers);
                append_shift_by_cl(bytes, 4, result);
                append_register_binary(bytes, 0x89, result, scratch);
                append_shift_by_cl(bytes, right_shift_digit(signed), scratch);
                append_register_binary(bytes, 0x39, value, scratch);
                append_trap_unless(bytes, JE);
            }
            TrappingShape::ShiftLeftRange { width, check } => {
                append_shift_count_check(bytes, width, registers);
                append_shift_by_cl(bytes, 4, registers[2]);
                append_range_check(bytes, check, registers[2], registers[3]);
            }
            TrappingShape::ShiftRight { width, signed } => {
                append_shift_count_check(bytes, width, registers);
                append_shift_by_cl(bytes, right_shift_digit(signed), registers[2]);
            }
            TrappingShape::Convert(check) => {
                let (operand, result, scratch) = (registers[0], registers[1], registers[2]);
                if let Some(check) = check {
                    append_range_check(bytes, check, operand, scratch);
                }
                append_register_binary(bytes, 0x89, operand, result);
            }
        }
        Ok(())
    }

    fn append_lea_register(bytes: &mut Vec<u8>, left: u8, right: u8, destination: u8) {
        let (base, index) = if left & 7 == 5 && right & 7 != 5 {
            (right, left)
        } else {
            (left, right)
        };
        let needs_zero_displacement = base & 7 == 5;
        bytes.extend([
            rex(destination, index, base),
            0x8d,
            modrm(u8::from(needs_zero_displacement), destination, 4),
            ((index & 7) << 3) | (base & 7),
        ]);
        if needs_zero_displacement {
            bytes.push(0);
        }
    }

    fn append_lea_immediate(bytes: &mut Vec<u8>, base: u8, destination: u8, displacement: i32) {
        let use_disp8 = i8::try_from(displacement).is_ok();
        let uses_sib = base & 7 == 4;
        bytes.extend([
            rex(destination, 0, base),
            0x8d,
            modrm(
                if use_disp8 { 1 } else { 2 },
                destination,
                if uses_sib { 4 } else { base },
            ),
        ]);
        if uses_sib {
            bytes.push(0x20 | (base & 7));
        }
        if use_disp8 {
            bytes.push(displacement as i8 as u8);
        } else {
            bytes.extend(displacement.to_le_bytes());
        }
    }

    pub(crate) fn encode_unchecked(
        kind: Kind,
        alternative: MachineAlternativeKey,
        registers: &[u8],
    ) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
        let mut bytes = Vec::new();
        match kind {
            Kind::InstalledInterruptCpuCheck { .. }
            | Kind::InstalledInterruptTaskCommit { .. }
            | Kind::InstalledInterruptTablePublication { .. } => {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            Kind::RegisterAccess { operation } => {
                return super::register_access::encode(operation, registers);
            }
            Kind::AtomicAccess { form } => bytes.extend(
                crate::selected_form_encoding::atomic::encode(form, registers)?,
            ),
            Kind::AccessorScalarAccess { .. }
            | Kind::NearestIeeeFloatArithmetic { .. }
            | Kind::FloatResultPolicy { .. } => {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            Kind::PortWrite { port, value } => {
                bytes.extend_from_slice(&x86_encoding::encode_immediate_port_write(port, value))
            }
            Kind::AtomicFence => bytes.extend_from_slice(&[0x0f, 0xae, 0xf0]),
            Kind::Crash => bytes.extend_from_slice(&[0x0f, 0x0b]),
            Kind::PrivilegedCrash => bytes.extend_from_slice(&crate::PRIVILEGED_STOP_BYTES),
            Kind::MaterializeI64 { value } if alternative.variant == 1 => {
                if integer_bits(value)? != 0 {
                    return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
                }
                append_self_clear(&mut bytes, registers[0]);
            }
            Kind::MaterializeI64 { value } => {
                bytes.extend([0x48 | (registers[0] >> 3), 0xb8 | (registers[0] & 7)]);
                bytes.extend(integer_bits(value)?.to_le_bytes());
            }
            Kind::ImageAddress { .. } => {
                bytes.extend(byte_literal_address_bytes(registers[0]));
            }
            Kind::MaterializeBooleanEqual
            | Kind::MaterializeBooleanU64LessThan
            | Kind::MaterializeBooleanI64LessThan
            | Kind::MaterializeBooleanU64LessOrEqual
            | Kind::MaterializeBooleanI64LessOrEqual => {
                let condition = match kind {
                    Kind::MaterializeBooleanEqual => 4,
                    Kind::MaterializeBooleanU64LessThan => 2,
                    Kind::MaterializeBooleanI64LessThan => 12,
                    Kind::MaterializeBooleanU64LessOrEqual => 6,
                    Kind::MaterializeBooleanI64LessOrEqual => 14,
                    _ => unreachable!("Boolean condition arm"),
                };
                append_boolean(&mut bytes, registers[0], condition);
            }
            Kind::ZeroExtendU8 => {
                bytes.extend([
                    0x40 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                    0x0f,
                    0xb6,
                    0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
                ]);
            }
            Kind::ZeroExtendU16 => {
                bytes.extend([
                    0x40 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                    0x0f,
                    0xb7,
                    0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
                ]);
            }
            Kind::SignExtendI8 => {
                bytes.extend([
                    0x48 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                    0x0f,
                    0xbe,
                    0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
                ]);
            }
            Kind::SignExtendI16 => {
                bytes.extend([
                    0x48 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                    0x0f,
                    0xbf,
                    0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
                ]);
            }
            Kind::SignExtendI32 => {
                bytes.extend([
                    0x48 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                    0x63,
                    0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
                ]);
            }
            Kind::ZeroExtendU32 => {
                bytes.extend([
                    0x40 | ((registers[0] >> 3) << 2) | (registers[1] >> 3),
                    0x89,
                    0xc0 | ((registers[0] & 7) << 3) | (registers[1] & 7),
                ]);
            }
            Kind::CopyI64 => {
                append_register_binary(&mut bytes, 0x89, registers[0], registers[1]);
            }
            Kind::CompareI64Zero => {
                append_self_test(&mut bytes, registers[0]);
            }
            Kind::CompareI64 => {
                append_register_compare(&mut bytes, registers[1], registers[0]);
            }
            Kind::CompareI64Immediate { immediate } => {
                bytes.extend([rex(0, 0, registers[0]), 0x81, modrm(3, 7, registers[0])]);
                bytes.extend(u12(immediate)?.to_le_bytes());
            }
            Kind::ByteViewAddress | Kind::WrappingAddI64 | Kind::ExactAddI64 { .. } => {
                append_lea_register(&mut bytes, registers[0], registers[1], registers[2]);
            }
            Kind::ExactDivideU64 { .. } => {
                if registers[0] != 0 || registers[2] != 0 || registers[3] != 2 || registers[1] == 2
                {
                    return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
                }
                bytes.extend([rex(0, 0, registers[1]), 0xf7, modrm(3, 6, registers[1])]);
            }
            Kind::ExactRemainderU64 { .. } => {
                if registers[0] != 0 || registers[2] != 0 || registers[3] != 2 || registers[1] == 2
                {
                    return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
                }
                append_register_binary(&mut bytes, 0x31, registers[3], registers[3]);
                bytes.extend([rex(0, 0, registers[1]), 0xf7, modrm(3, 6, registers[1])]);
                append_register_binary(&mut bytes, 0x89, registers[3], registers[2]);
            }
            Kind::WrappingDivideI64 { .. } => {
                if registers[0] != 0 || registers[2] != 0 || registers[3] != 2 || registers[1] == 2
                {
                    return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
                }
                bytes.extend([
                    rex(0, 0, registers[1]),
                    0x83,
                    modrm(3, 7, registers[1]),
                    0xff,
                ]);
                bytes.extend([0x75, 5]);
                bytes.extend([rex(0, 0, registers[2]), 0xf7, modrm(3, 3, registers[2])]);
                bytes.extend([0xeb, 5]);
                append_signed_divide(&mut bytes, registers[1]);
            }
            Kind::ExactDivideI64 { .. } => {
                if registers[0] != 0 || registers[2] != 0 || registers[3] != 2 || registers[1] == 2
                {
                    return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
                }
                append_signed_divide(&mut bytes, registers[1]);
            }
            Kind::ExactRemainderI64 { .. } => {
                if registers[0] != 0 || registers[2] != 0 || registers[3] != 2 || registers[1] == 2
                {
                    return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
                }
                append_signed_divide(&mut bytes, registers[1]);
                append_register_binary(&mut bytes, 0x89, registers[3], registers[2]);
            }
            Kind::WrappingRemainderI64 { .. } => {
                if registers[0] != 0 || registers[2] != 0 || registers[3] != 2 || registers[1] == 2
                {
                    return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
                }
                append_register_binary(&mut bytes, 0x31, registers[3], registers[3]);
                bytes.extend([
                    rex(0, 0, registers[1]),
                    0x83,
                    modrm(3, 7, registers[1]),
                    0xff,
                ]);
                bytes.extend([0x74, 5, 0x48, 0x99]);
                bytes.extend([rex(0, 0, registers[1]), 0xf7, modrm(3, 7, registers[1])]);
                append_register_binary(&mut bytes, 0x89, registers[3], registers[2]);
            }
            Kind::SaturatingAdd { carrier } => {
                append_saturating(&mut bytes, SaturatingOperation::Add, carrier, registers)?;
            }
            Kind::SaturatingSubtract { carrier } => {
                append_saturating(
                    &mut bytes,
                    SaturatingOperation::Subtract,
                    carrier,
                    registers,
                )?;
            }
            Kind::SaturatingDivide { carrier, .. } => {
                append_saturating(&mut bytes, SaturatingOperation::Divide, carrier, registers)?;
            }
            Kind::SaturatingRemainder { carrier, .. } => {
                append_saturating(
                    &mut bytes,
                    SaturatingOperation::Remainder,
                    carrier,
                    registers,
                )?;
            }
            Kind::SaturatingMultiply { carrier } => {
                append_saturating(
                    &mut bytes,
                    SaturatingOperation::Multiply,
                    carrier,
                    registers,
                )?;
            }
            Kind::TrappingInteger { form } => {
                append_trapping(&mut bytes, form, registers)?;
            }
            Kind::BitwiseAndI64 | Kind::BitwiseOrI64 | Kind::BitwiseXorI64 => {
                let opcode = match kind {
                    Kind::BitwiseXorI64 => 0x31,
                    Kind::BitwiseOrI64 => 0x09,
                    _ => 0x21,
                };
                if registers[2] == registers[0] {
                    append_register_binary(&mut bytes, opcode, registers[1], registers[2]);
                } else if registers[2] == registers[1] {
                    append_register_binary(&mut bytes, opcode, registers[0], registers[2]);
                } else {
                    append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
                    append_register_binary(&mut bytes, opcode, registers[1], registers[2]);
                }
            }
            Kind::BitwiseNotI64 => {
                if registers[1] != registers[0] {
                    append_register_binary(&mut bytes, 0x89, registers[0], registers[1]);
                }
                bytes.extend([rex(0, 0, registers[1]), 0xf7, modrm(3, 2, registers[1])]);
            }
            Kind::ExactAddI64Immediate { immediate, .. } => {
                append_lea_immediate(
                    &mut bytes,
                    registers[0],
                    registers[1],
                    i32::try_from(u12(immediate)?).expect("u12 fits i32"),
                );
            }
            Kind::ExactSubtractI64Immediate { immediate, .. } => {
                append_lea_immediate(
                    &mut bytes,
                    registers[0],
                    registers[1],
                    -i32::try_from(u12(immediate)?).expect("u12 fits i32"),
                );
            }
            Kind::ExactSubtractI64 { .. } | Kind::WrappingSubtractI64 => {
                match alternative.variant {
                    0 => append_self_clear(&mut bytes, registers[2]),
                    1 => append_register_binary(&mut bytes, 0x29, registers[1], registers[2]),
                    2 => {
                        bytes.extend([rex(0, 0, registers[2]), 0xf7, modrm(3, 3, registers[2])]);
                        append_register_binary(&mut bytes, 0x01, registers[0], registers[2]);
                    }
                    3 => {
                        append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
                        append_register_binary(&mut bytes, 0x29, registers[1], registers[2]);
                    }
                    _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
                }
            }
            Kind::ExactMultiplyI64 { .. } | Kind::WrappingMultiplyI64 => {
                let append_imul = |bytes: &mut Vec<u8>, source: u8, destination: u8| {
                    bytes.extend([
                        rex(destination, 0, source),
                        0x0f,
                        0xaf,
                        modrm(3, destination, source),
                    ]);
                };
                match alternative.variant {
                    0 => append_imul(&mut bytes, registers[2], registers[2]),
                    1 => append_imul(&mut bytes, registers[1], registers[2]),
                    2 => append_imul(&mut bytes, registers[0], registers[2]),
                    3 => {
                        append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
                        append_imul(&mut bytes, registers[1], registers[2]);
                    }
                    _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
                }
            }
            Kind::WrappingShiftLeftI64
            | Kind::WrappingShiftRightI64
            | Kind::WrappingShiftRightU64
            | selected_instructions::exact_shift_i64_pair_pattern!()
            | Kind::ExactShiftRightU64 { .. } => {
                if registers[1] != 1 {
                    return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
                }
                let digit = match kind {
                    Kind::WrappingShiftLeftI64 | Kind::ExactShiftLeftI64 { .. } => 4,
                    Kind::WrappingShiftRightU64 | Kind::ExactShiftRightU64 { .. } => 5,
                    _ => 7,
                };
                append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
                bytes.extend([rex(0, 0, registers[2]), 0xd3, modrm(3, digit, registers[2])]);
            }
            Kind::ReturnScalar | Kind::ReturnAggregate { .. } | Kind::ReturnUnit => {
                bytes.extend_from_slice(&crate::ORDINARY_RETURN_BYTES)
            }
            selected_instructions::layout_dependent_selected_transfers!()
            | selected_instructions::dedicated_selected_forms!() => {
                return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
            }
        }
        Ok(bytes)
    }

    pub const X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET: u16 = 3;

    pub(crate) const fn byte_literal_address_bytes(register: u8) -> [u8; 7] {
        [
            0x48 | ((register >> 3) << 2),
            0x8d,
            0x05 | ((register & 7) << 3),
            0,
            0,
            0,
            0,
        ]
    }
}
#[cfg(test)]
mod integer_normalization_tests {
    use register_model::validate_physical_register_model;
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
        SelectedInstructionKind,
    };

    use super::{encode_x86_64_selected_form, validate_x86_64_selected_form_encoding};
    use crate::x86_64_physical_register_model;

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
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        for (source, destination) in [
            ("rax", "rax"),
            ("rax", "rcx"),
            ("rcx", "rdx"),
            ("rdx", "rbx"),
            ("rbx", "rsi"),
            ("rsi", "rdi"),
            ("rdi", "rbp"),
            ("rbp", "r8"),
            ("r8", "r9"),
            ("r9", "r15"),
            ("r15", "rax"),
            ("r15", "r15"),
        ] {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            for (kind, family) in forms() {
                let key = MachineAlternativeKey { family, variant: 0 };
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                let footprint = encoded.footprint();
                assert_eq!(footprint.register_reads, vec![operands[0]]);
                assert_eq!(footprint.register_writes, vec![operands[1]]);
                assert!(!footprint.writes_rflags);
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
                        validate_x86_64_selected_form_encoding(
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
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            other_key,
                            &operands,
                            encoded.bytes()
                        )
                        .is_err()
                    );
                }
                for bit_offset in 0..encoded.bytes().len() * 8 {
                    let mut changed = encoded.bytes().to_vec();
                    changed[bit_offset / 8] ^= 1 << (bit_offset % 8);
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical, kind, key, &operands, &changed
                        )
                        .is_err(),
                        "{kind:?} accepted changed bit {bit_offset}"
                    );
                }
            }
        }
    }

    #[test]
    fn normalization_matches_clang_assembler_oracle() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let fixtures: [(
            SelectedInstructionKind,
            MachineAlternativeFamily,
            &str,
            &str,
            &[u8],
        ); 4] = [
            r!(ZeroExtendU16 => ZeroExtendU16 => "r15" => "r9" => &[0x45, 0x0f, 0xb7, 0xcf]),
            r!(SignExtendI8 => SignExtendI8 => "rsi" => "r9" => &[0x4c, 0x0f, 0xbe, 0xce]),
            r!(SignExtendI16 => SignExtendI16 => "r15" => "r9" => &[0x4d, 0x0f, 0xbf, 0xcf]),
            r!(SignExtendI32 => SignExtendI32 => "r15" => "r9" => &[0x4d, 0x63, 0xcf]),
        ];
        for (kind, family, source, destination, expected) in fixtures {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            let key = MachineAlternativeKey { family, variant: 0 };
            assert_eq!(
                encode_x86_64_selected_form(&physical, kind, key, &operands)
                    .unwrap()
                    .bytes(),
                expected
            );
            validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, expected)
                .unwrap();
        }
    }
}
mod jump {
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, ValidatedPhysicalRegisterModel,
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
        X86_64SelectedFormFootprint,
    };
    pub fn encode_x86_64_selected_jump_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let displacement = i32::try_from(displacement)
            .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        let mut bytes = vec![0xe9];
        crate::codec_bytes::push_encoded(&mut bytes, displacement.to_le_bytes());
        validate_x86_64_selected_jump_form(physical, alternative, i64::from(displacement), &bytes)
    }

    pub fn validate_x86_64_selected_jump_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        super::request_validation::guard_canonical(physical)?;
        if alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::Jump,
                variant: 0,
            })
        {
            return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
        }
        let [0xe9, a, b, c, d] = bytes else {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        };
        if i64::from(i32::from_le_bytes([*a, *b, *c, *d])) != displacement {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let pc = physical
            .model()
            .view_named("rip")
            .expect("canonical model has RIP")
            .units
            .clone();
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: vec![],
                register_writes: vec![],
                writes_rflags: false,
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
        use super::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
            encode_x86_64_selected_jump_form, validate_x86_64_selected_jump_form,
        };
        use crate::x86_64_physical_register_model;
        #[test]
        fn jump_round_trip_rejects_wrong_target_and_opcode() {
            let physical =
                register_model::validate_physical_register_model(x86_64_physical_register_model())
                    .unwrap();
            let key = MachineAlternativeKey {
                family: MachineAlternativeFamily::Jump,
                variant: 0,
            };
            for displacement in [i64::from(i32::MIN), -5, 0, 7, i64::from(i32::MAX)] {
                let encoded =
                    encode_x86_64_selected_jump_form(&physical, key, displacement).unwrap();
                assert_eq!(encoded.bytes().len(), 5);
                assert!(
                    validate_x86_64_selected_jump_form(
                        &physical,
                        key,
                        displacement + 1,
                        encoded.bytes()
                    )
                    .is_err()
                );
                let mut corrupt = encoded.bytes().to_vec();
                corrupt[0] ^= 1;
                assert!(
                    validate_x86_64_selected_jump_form(&physical, key, displacement, &corrupt)
                        .is_err()
                );
                assert_eq!(
                    encoded.footprint().encoded.control,
                    MachineEncodedControlEffect::UnconditionalRelativeBranchV1
                );
            }
            assert!(
                encode_x86_64_selected_jump_form(&physical, key, i64::from(i32::MAX) + 1).is_err()
            );
        }
    }
}
pub(crate) mod materialization {
    pub(crate) mod mov_r32_imm32 {
        use register_model::RegisterViewId;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum X86_64MovR32Imm32I64MaterializationError {
            NonCanonicalPhysicalModel,
            UnknownOrNonGpr64View(RegisterViewId),
            IntegerOutsideZeroExtendedU32Bits,
            MalformedEncoding,
            EncodedFormMismatch,
        }

        optimization_core::invalid_error_display!(
            X86_64MovR32Imm32I64MaterializationError,
            "x86-64 MOV-r32-imm32 i64 materialization"
        );
    }
    pub(crate) mod mov_r64_imm32_sign_extended {
        use register_model::RegisterViewId;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum X86_64MovR64Imm32SignExtendedI64MaterializationError {
            NonCanonicalPhysicalModel,
            UnknownOrNonGpr64View(RegisterViewId),
            IntegerOutsideSignExtendedI32Bits,
            MalformedEncoding,
            EncodedFormMismatch,
        }

        optimization_core::invalid_error_display!(
            X86_64MovR64Imm32SignExtendedI64MaterializationError,
            "x86-64 MOV-r64-imm32 sign-extended i64 materialization"
        );
    }
}
mod memory {
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, RegisterViewId, SelectedInstructionKind,
        ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
    };
    use crate::selected_form_encoding::instruction_bytes::{modrm, rex};
    use crate::selected_form_encoding::request_validation::resolve_registers;
    pub(super) mod frame {
        use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
        use selected_instructions::{
            MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedControlEffect,
            MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
            MachineEncodedTrapBehavior, SelectedInstructionKind as Kind,
        };

        use super::super::{
            ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
            X86_64SelectedFormFootprint,
        };

        fn request(
            physical: &ValidatedPhysicalRegisterModel,
            kind: Kind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
        ) -> Result<(RegisterViewId, Option<RegisterViewId>), X86_64SelectedFormEncodingError>
        {
            if matches!(
                kind,
                Kind::Store64 {
                    slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                    ..
                }
            ) || matches!(kind, Kind::FrameAddress { slot: selected_instructions::FrameStorageSlotId::ActivationBase, byte_offset } if byte_offset != 0)
            {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            let (family, store) = match kind {
                Kind::Store64 { .. } => (Family::Store64, true),
                Kind::FrameAddress { .. } => (Family::FrameAddress, false),
                _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
            };
            if physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
                || alternative != (MachineAlternativeKey { family, variant: 0 })
            {
                return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
            }
            super::super::request_validation::resolve_registers(physical, operands)?;
            match operands {
                [value, scratch] if store && value != scratch => Ok((*scratch, Some(*value))),
                [address] if !store => Ok((*address, None)),
                _ => Err(X86_64SelectedFormEncodingError::OperandCountMismatch),
            }
        }

        pub fn encode(
            physical: &ValidatedPhysicalRegisterModel,
            kind: Kind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: i64,
            alignment_bytes: u16,
        ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
            if matches!(
                kind,
                Kind::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                    ..
                }
            ) && (displacement != 0 || alignment_bytes != 1)
            {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            let (address, value) = request(physical, kind, alternative, operands)?;
            let displacement = i32::try_from(displacement)
                .map_err(|_| X86_64SelectedFormEncodingError::ImmediateOutsideU12)?;
            let member = match kind {
                Kind::Store64 { byte_offset, .. } | Kind::FrameAddress { byte_offset, .. } => {
                    byte_offset
                }
                _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
            };
            let mut bytes = if alignment_bytes == 1 {
                crate::frame_protocol::encode_frame_address_signed(physical, address, displacement)
                    .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?
            } else {
                let offset = u32::try_from(displacement)
                    .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
                crate::encode_x86_64_aligned_frame_address(
                    physical,
                    address,
                    offset,
                    alignment_bytes,
                    member,
                )
                .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?
            };
            if let Some(value) = value {
                bytes.extend_from_slice(
                    &crate::encode_x86_64_pointer_memory(physical, value, address, 0, 8, false)
                        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?,
                );
            }
            validate(
                physical,
                kind,
                alternative,
                operands,
                i64::from(displacement),
                alignment_bytes,
                &bytes,
            )
        }

        pub fn validate(
            physical: &ValidatedPhysicalRegisterModel,
            kind: Kind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: i64,
            alignment_bytes: u16,
            bytes: &[u8],
        ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
            if matches!(
                kind,
                Kind::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                    ..
                }
            ) && (displacement != 0 || alignment_bytes != 1)
            {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            let (address, value) = request(physical, kind, alternative, operands)?;
            let displacement = i32::try_from(displacement)
                .map_err(|_| X86_64SelectedFormEncodingError::ImmediateOutsideU12)?;
            let member = match kind {
                Kind::Store64 { byte_offset, .. } | Kind::FrameAddress { byte_offset, .. } => {
                    byte_offset
                }
                _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
            };
            let address_bytes = if let Some(value) = value {
                crate::frame_protocol::decode_pointer_store_suffix(physical, value, address, bytes)
                    .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?
            } else {
                bytes
            };
            if !crate::frame_protocol::validate_frame_address(
                physical,
                address,
                displacement,
                alignment_bytes,
                member,
                address_bytes,
            ) {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            let store = value.is_some();
            let catalog = crate::x86_64_register_constraint_catalog_for(physical);
            let key = if store {
                crate::X86_64_STORE64
            } else {
                crate::X86_64_FRAME_ADDRESS
            };
            let constraint = catalog
                .constraint(key)
                .ok_or(X86_64SelectedFormEncodingError::AlternativeMismatch)?;
            let stack_pointer = physical
                .model()
                .view_named("rsp")
                .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?
                .id;
            Ok(ValidatedX86_64SelectedFormEncoding {
                bytes: bytes.to_vec(),
                footprint: X86_64SelectedFormFootprint {
                    register_reads: value.into_iter().collect(),
                    register_writes: vec![address],
                    writes_rflags: alignment_bytes != 1,
                    encoded: MachineEncodedEffects {
                        external_operand_reads: if store { vec![0] } else { Vec::new() },
                        external_operand_writes: vec![u16::from(store)],
                        implicit_unit_uses: constraint.implicit_uses.clone(),
                        implicit_unit_defs: constraint.implicit_defs.clone(),
                        implicit_unit_clobbers: constraint.clobbers.clone(),
                        memory: if store {
                            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                                stack_pointer,
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
            use super::super::replay_battery;
            use super::*;
            use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};
            use semantic_vocabulary::{OperationId, PlaceId};

            #[test]
            fn aligned_frame_forms_decode_members_and_declared_clobbers() {
                let physical = register_model::validate_physical_register_model(
                    crate::x86_64_physical_register_model(),
                )
                .unwrap();
                let value = physical.model().view_named("r9").unwrap().id;
                let address = physical.model().view_named("r12").unwrap().id;
                let slot = FrameStorageSlotId::Local(LocalStorageSlotId::Structural {
                    operation: OperationId::new(7).unwrap(),
                    place: PlaceId::new(1).unwrap(),
                });
                for (kind, family, operands) in [
                    (
                        Kind::FrameAddress {
                            slot,
                            byte_offset: 24,
                        },
                        Family::FrameAddress,
                        vec![address],
                    ),
                    (
                        Kind::Store64 {
                            slot,
                            byte_offset: 24,
                        },
                        Family::Store64,
                        vec![value, address],
                    ),
                ] {
                    let alternative = MachineAlternativeKey { family, variant: 0 };
                    let encoded = encode(&physical, kind, alternative, &operands, 48, 64).unwrap();
                    assert!(encoded.footprint.writes_rflags);
                    assert_eq!(encoded.footprint.register_writes, vec![address]);
                    assert_eq!(
                        encoded.footprint.encoded.implicit_unit_clobbers,
                        physical
                            .model()
                            .view_named("condition_flags")
                            .unwrap()
                            .units
                    );
                    assert_eq!(
                        &encoded.bytes[..15],
                        &[
                            0x4c, 0x8d, 0xa4, 0x24, 111, 0, 0, 0, 0x49, 0x81, 0xe4, 0xc0, 0xff,
                            0xff, 0xff
                        ]
                    );
                    assert_eq!(
                        &encoded.bytes[15..23],
                        &[0x4d, 0x8d, 0xa4, 0x24, 24, 0, 0, 0]
                    );
                    replay_battery::replay_rejects_byte_mutations(&encoded.bytes, |bytes| {
                        validate(&physical, kind, alternative, &operands, 48, 64, bytes)
                    });
                    assert!(
                        validate(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            64,
                            64,
                            &encoded.bytes
                        )
                        .is_err()
                    );
                    assert!(
                        validate(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            48,
                            32,
                            &encoded.bytes
                        )
                        .is_err()
                    );
                    assert!(encode(&physical, kind, alternative, &operands, 49, 64).is_err());
                    assert!(encode(&physical, kind, alternative, &operands, 48, 48).is_err());
                }
                assert!(
                    encode(
                        &physical,
                        Kind::Store64 {
                            slot,
                            byte_offset: 0
                        },
                        MachineAlternativeKey {
                            family: Family::Store64,
                            variant: 0
                        },
                        &[value, value],
                        48,
                        32
                    )
                    .is_err()
                );
            }
        }
    }
    mod indexed {
        use super::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
            MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
            MachineEncodedTrapBehavior, RegisterViewId, ValidatedPhysicalRegisterModel,
            ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
            X86_64SelectedFormFootprint, modrm, resolve_registers, rex,
        };
        pub(super) fn encode(
            physical: &ValidatedPhysicalRegisterModel,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: i64,
        ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
            let [base, index, destination] =
                request(physical, alternative, operands, displacement)?;
            let bytes = [
                rex(destination, index, base),
                0x0f,
                0xb6,
                modrm(2, destination, 4),
                ((index & 7) << 3) | (base & 7),
                0,
                0,
                0,
                0,
            ];
            validate(physical, alternative, operands, displacement, &bytes)
        }

        pub(super) fn validate(
            physical: &ValidatedPhysicalRegisterModel,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: i64,
            bytes: &[u8],
        ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
            let [base, index, destination] =
                request(physical, alternative, operands, displacement)?;
            let [
                prefix,
                escape,
                opcode,
                mode,
                sib,
                offset0,
                offset1,
                offset2,
                offset3,
            ] = bytes
            else {
                return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
            };
            let decoded_destination = ((mode >> 3) & 7) | ((prefix & 4) << 1);
            let decoded_base = (sib & 7) | ((prefix & 1) << 3);
            let decoded_index = ((sib >> 3) & 7) | ((prefix & 2) << 2);
            if prefix & 0xf8 != 0x48
                || *escape != 0x0f
                || *opcode != 0xb6
                || mode & 0xc7 != 0x84
                || sib >> 6 != 0
                || decoded_destination != destination
                || decoded_base != base
                || decoded_index != index
                || [*offset0, *offset1, *offset2, *offset3] != [0; 4]
            {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            Ok(ValidatedX86_64SelectedFormEncoding {
                bytes: bytes.to_vec(),
                footprint: X86_64SelectedFormFootprint {
                    register_reads: vec![operands[0], operands[1]],
                    register_writes: vec![operands[2]],
                    writes_rflags: false,
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
            displacement: i64,
        ) -> Result<[u8; 3], X86_64SelectedFormEncodingError> {
            super::super::request_validation::guard_canonical(physical)?;
            if displacement != 0
                || alternative
                    != (MachineAlternativeKey {
                        family: MachineAlternativeFamily::Load8Indexed,
                        variant: 0,
                    })
            {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            resolve_registers(physical, operands)?
                .try_into()
                .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)
        }

        #[cfg(test)]
        mod tests {
            use super::super::{
                SelectedInstructionKind, encode_x86_64_selected_memory_form, replay_battery,
                validate_x86_64_selected_memory_form,
            };
            use super::{
                MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
            };

            #[test]
            fn indexed_byte_load_decodes_every_bit_and_operand_role() {
                let physical = register_model::validate_physical_register_model(
                    crate::x86_64_physical_register_model(),
                )
                .unwrap();
                let operands =
                    ["r13", "r12", "r15"].map(|name| physical.model().view_named(name).unwrap().id);
                let kind = SelectedInstructionKind::Load8Indexed;
                let alternative = MachineAlternativeKey {
                    family: MachineAlternativeFamily::Load8Indexed,
                    variant: 0,
                };
                let encoded =
                    encode_x86_64_selected_memory_form(&physical, kind, alternative, &operands, 0)
                        .unwrap();
                assert_eq!(encoded.bytes(), &[0x4f, 0x0f, 0xb6, 0xbc, 0x25, 0, 0, 0, 0]);
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
                replay_battery::replay_rejects_byte_mutations(encoded.bytes(), |bytes| {
                    validate_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        0,
                        bytes,
                    )
                });
                replay_battery::replay_rejects_substituted_requests(
                    encoded.bytes(),
                    kind,
                    alternative,
                    &operands,
                    0,
                    |kind, alternative, operands, displacement, bytes| {
                        validate_x86_64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            operands,
                            displacement,
                            bytes,
                        )
                    },
                );
                for role in 0..3 {
                    let mut corrupt = operands;
                    corrupt[role] = operands[(role + 1) % 3];
                    assert!(
                        validate_x86_64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &corrupt,
                            0,
                            encoded.bytes(),
                        )
                        .is_err()
                    );
                }
                assert!(
                    validate_x86_64_selected_memory_form(
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
                let stack = physical.model().view_named("rsp").unwrap().id;
                for role in 0..3 {
                    let mut invalid = operands;
                    invalid[role] = stack;
                    assert!(
                        encode_x86_64_selected_memory_form(
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
                        encode_x86_64_selected_memory_form(
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
            SelectedInstructionKind, encode_x86_64_selected_memory_form, replay_battery,
            validate_x86_64_selected_memory_form,
        };
        #[test]
        fn load32_preserves_exact_four_byte_extent_and_rejects_a_widened_load() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let operands = [
                physical.model().view_named("rax").unwrap().id,
                physical.model().view_named("rcx").unwrap().id,
            ];
            let kind = SelectedInstructionKind::Load32 { byte_offset: 4 };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::Load32,
                variant: 0,
            };
            let encoded =
                encode_x86_64_selected_memory_form(&physical, kind, alternative, &operands, 4)
                    .unwrap();
            assert_eq!(encoded.bytes(), &[0x40, 0x8b, 0x88, 4, 0, 0, 0]);
            assert_eq!(
                encoded.footprint().encoded.memory,
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: 4
                }
            );
            replay_battery::replay_rejects_byte_mutations(encoded.bytes(), |bytes| {
                validate_x86_64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    4,
                    bytes,
                )
            });
            replay_battery::replay_rejects_substituted_requests(
                encoded.bytes(),
                kind,
                alternative,
                &operands,
                4,
                |kind, alternative, operands, displacement, bytes| {
                    validate_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        operands,
                        displacement,
                        bytes,
                    )
                },
            );
            assert!(
                validate_x86_64_selected_memory_form(
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
        use super::{encode_x86_64_selected_memory_form, validate_x86_64_selected_memory_form};
        use crate::selected_form_encoding::ValidatedX86_64SelectedFormEncoding;
        use isa_conformance::narrow_load::{self, Exact, Footprint, Roster};

        fn observed(encoded: &ValidatedX86_64SelectedFormEncoding) -> Footprint {
            Footprint {
                bytes: encoded.bytes().to_vec(),
                register_reads: encoded.footprint().register_reads.clone(),
                register_writes: encoded.footprint().register_writes.clone(),
                memory: encoded.footprint().encoded.memory,
                stack: encoded.footprint().encoded.stack,
                trap: encoded.footprint().encoded.trap,
                flags_untouched: !encoded.footprint().writes_rflags,
            }
        }

        #[test]
        fn narrow_loads_bind_widths_footprints_opcodes_and_refuse_substituted_requests() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            narrow_load::conformance(
                &physical,
                &Roster {
                    bases: &["rax", "rbp", "r12", "r13", "r15"],
                    destination: "r11",
                    offsets: |_width| vec![0, 2, 4096],
                },
                &Exact {
                    operands: ["rax", "rcx"],
                    expected: [
                        (1, &[0x48, 0x0f, 0xb6, 0x88, 0, 0, 0, 0]),
                        (2, &[0x48, 0x0f, 0xb7, 0x88, 0, 0, 0, 0]),
                    ],
                    signed_opcode: (2, [0xbe, 0xbf]),
                    invalid_views: &["rsp", "eax", "xmm0"],
                    invalid_offsets: &[(1, u32::MAX)],
                },
                |kind, alternative, operands, displacement| {
                    encode_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        operands,
                        displacement,
                    )
                    .map(|encoded| observed(&encoded))
                },
                |kind, alternative, operands, displacement, bytes| {
                    validate_x86_64_selected_memory_form(
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
            MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedControlEffect,
            MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
            MachineEncodedTrapBehavior, RegisterViewId, SelectedInstructionKind as Kind,
            ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
            X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint, modrm, resolve_registers,
            rex,
        };
        mod replay {
            use super::X86_64SelectedFormEncodingError as Error;

            pub(super) fn validate(
                bytes: &[u8],
                load: bool,
                width: u8,
                [base, value, scratch]: [u8; 3],
                offset: u32,
            ) -> Result<(), Error> {
                let mut remaining = bytes;
                if !load {
                    register_operation(&mut remaining, 0x89, value, scratch)?;
                }
                for byte_index in 0..width {
                    let destination = if load && byte_index == 0 {
                        value
                    } else {
                        scratch
                    };
                    access(
                        &mut remaining,
                        load,
                        destination,
                        base,
                        offset + u32::from(byte_index),
                    )?;
                    if load && byte_index != 0 {
                        shift(&mut remaining, scratch, 4, byte_index * 8)?;
                        register_operation(&mut remaining, 0x09, scratch, value)?;
                    } else if !load && byte_index + 1 != width {
                        shift(&mut remaining, scratch, 5, 8)?;
                    }
                }
                if remaining.is_empty() {
                    Ok(())
                } else {
                    Err(Error::MalformedEncoding)
                }
            }

            fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Result<&'a [u8], Error> {
                let (head, tail) = bytes
                    .split_at_checked(count)
                    .ok_or(Error::MalformedEncoding)?;
                *bytes = tail;
                Ok(head)
            }

            fn register_operation(
                bytes: &mut &[u8],
                opcode: u8,
                source: u8,
                destination: u8,
            ) -> Result<(), Error> {
                let encoded = take(bytes, 3)?;
                let prefix = encoded[0];
                let mode = encoded[2];
                if prefix & 0xfa != 0x48
                    || encoded[1] != opcode
                    || mode >> 6 != 3
                    || ((mode >> 3) & 7) | ((prefix & 4) << 1) != source
                    || (mode & 7) | ((prefix & 1) << 3) != destination
                {
                    return Err(Error::EncodedFormMismatch);
                }
                Ok(())
            }

            fn shift(
                bytes: &mut &[u8],
                register: u8,
                operation: u8,
                amount: u8,
            ) -> Result<(), Error> {
                let encoded = take(bytes, 4)?;
                if encoded[0] & 0xfe != 0x48
                    || encoded[1] != 0xc1
                    || encoded[2] >> 6 != 3
                    || (encoded[2] >> 3) & 7 != operation
                    || (encoded[2] & 7) | ((encoded[0] & 1) << 3) != register
                    || encoded[3] != amount
                {
                    return Err(Error::EncodedFormMismatch);
                }
                Ok(())
            }

            fn access(
                bytes: &mut &[u8],
                load: bool,
                register: u8,
                base: u8,
                offset: u32,
            ) -> Result<(), Error> {
                let prefix = take(bytes, 1)?[0];
                if prefix & 0xfa != if load { 0x48 } else { 0x40 } {
                    return Err(Error::EncodedFormMismatch);
                }
                if load && take(bytes, 1)?[0] != 0x0f {
                    return Err(Error::EncodedFormMismatch);
                }
                let header = take(bytes, 2)?;
                let mode = header[1];
                if header[0] != if load { 0xb6 } else { 0x88 }
                    || mode >> 6 != 2
                    || ((mode >> 3) & 7) | ((prefix & 4) << 1) != register
                    || (mode & 7) | ((prefix & 1) << 3) != base
                {
                    return Err(Error::EncodedFormMismatch);
                }
                if mode & 7 == 4 && take(bytes, 1)?[0] != 0x24 {
                    return Err(Error::EncodedFormMismatch);
                }
                let displacement =
                    <[u8; 4]>::try_from(take(bytes, 4)?).map_err(|_| Error::MalformedEncoding)?;
                if i32::from_le_bytes(displacement) < 0
                    || u32::from_le_bytes(displacement) != offset
                {
                    return Err(Error::EncodedFormMismatch);
                }
                Ok(())
            }
        }
        type Request = (bool, u8, [u8; 3], X86_64SelectedFormFootprint);

        fn request(
            physical: &ValidatedPhysicalRegisterModel,
            kind: Kind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: u32,
        ) -> Result<Request, X86_64SelectedFormEncodingError> {
            super::super::request_validation::guard_canonical(physical)?;
            let invalid = X86_64SelectedFormEncodingError::EncodedFormMismatch;
            let (load, width, offset) = match kind {
                Kind::LoadPacked { byte_offset, width } => (true, width.byte_size(), byte_offset),
                Kind::StorePacked { byte_offset, width } => (false, width.byte_size(), byte_offset),
                _ => return Err(invalid),
            };
            let family = if load {
                match width {
                    3 => Family::LoadPacked3,
                    5 => Family::LoadPacked5,
                    6 => Family::LoadPacked6,
                    7 => Family::LoadPacked7,
                    _ => return Err(invalid),
                }
            } else {
                Family::StorePacked
            };
            if alternative != (MachineAlternativeKey { family, variant: 0 }) {
                return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
            }
            if offset != displacement
                || displacement
                    .checked_add(u32::from(width) - 1)
                    .is_none_or(|end| end > i32::MAX as u32)
            {
                return Err(invalid);
            }
            let registers: [u8; 3] = resolve_registers(physical, operands)?
                .try_into()
                .map_err(|_| X86_64SelectedFormEncodingError::OperandCountMismatch)?;
            if registers[2] == registers[0]
                || registers[2] == registers[1]
                || (load && registers[0] == registers[1])
            {
                return Err(invalid);
            }
            let reads = if load { vec![0] } else { vec![0, 1] };
            let writes = if load { vec![1, 2] } else { vec![2] };
            let flags = physical
                .model()
                .view_named("condition_flags")
                .ok_or(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)?;
            let footprint = X86_64SelectedFormFootprint {
                register_reads: reads
                    .iter()
                    .map(|position| operands[*position as usize])
                    .collect(),
                register_writes: writes
                    .iter()
                    .map(|position| operands[*position as usize])
                    .collect(),
                writes_rflags: true,
                encoded: MachineEncodedEffects {
                    external_operand_reads: reads,
                    external_operand_writes: writes,
                    implicit_unit_uses: Vec::new(),
                    implicit_unit_defs: Vec::new(),
                    implicit_unit_clobbers: flags.units.clone(),
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
            };
            Ok((load, width, registers, footprint))
        }

        pub(super) fn encode(
            physical: &ValidatedPhysicalRegisterModel,
            kind: Kind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: i64,
        ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
            let displacement = u32::try_from(displacement)
                .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
            let (load, width, [base, value, scratch], _) =
                request(physical, kind, alternative, operands, displacement)?;
            let mut bytes = Vec::new();
            if !load {
                bytes.extend([rex(value, 0, scratch), 0x89, modrm(3, value, scratch)]);
            }
            for byte_index in 0..width {
                let register = if load && byte_index == 0 {
                    value
                } else {
                    scratch
                };
                bytes.push(rex(register, 0, base) & if load { 0xff } else { 0xf7 });
                if load {
                    bytes.push(0x0f);
                }
                bytes.extend([if load { 0xb6 } else { 0x88 }, modrm(2, register, base)]);
                if base & 7 == 4 {
                    bytes.push(0x24);
                }
                bytes.extend((displacement + u32::from(byte_index)).to_le_bytes());
                if load && byte_index != 0 {
                    bytes.extend([
                        rex(0, 0, scratch),
                        0xc1,
                        modrm(3, 4, scratch),
                        byte_index * 8,
                    ]);
                    bytes.extend([rex(scratch, 0, value), 0x09, modrm(3, scratch, value)]);
                } else if !load && byte_index + 1 != width {
                    bytes.extend([rex(0, 0, scratch), 0xc1, modrm(3, 5, scratch), 8]);
                }
            }
            validate(
                physical,
                kind,
                alternative,
                operands,
                i64::from(displacement),
                &bytes,
            )
        }

        pub(super) fn validate(
            physical: &ValidatedPhysicalRegisterModel,
            kind: Kind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: i64,
            bytes: &[u8],
        ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
            let displacement = u32::try_from(displacement)
                .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
            let (load, width, registers, footprint) =
                request(physical, kind, alternative, operands, displacement)?;
            replay::validate(bytes, load, width, registers, displacement)?;
            Ok(ValidatedX86_64SelectedFormEncoding {
                bytes: bytes.to_vec(),
                footprint,
            })
        }

        #[cfg(test)]
        mod tests {
            use super::super::replay_battery;
            use super::{
                Family, Kind, MachineAlternativeKey, MachineEncodedMemoryEffect, encode, validate,
            };
            use register_model::{RegisterOperandAccess, validate_physical_register_model};
            use selected_instructions::{MachineSemanticKind, PackedByteWidth};

            fn kind(load: bool, width: PackedByteWidth, byte_offset: u32) -> Kind {
                if load {
                    Kind::LoadPacked { byte_offset, width }
                } else {
                    Kind::StorePacked { byte_offset, width }
                }
            }

            fn alternative(load: bool, width: PackedByteWidth) -> MachineAlternativeKey {
                MachineAlternativeKey {
                    family: if load {
                        match width {
                            PackedByteWidth::Three => Family::LoadPacked3,
                            PackedByteWidth::Five => Family::LoadPacked5,
                            PackedByteWidth::Six => Family::LoadPacked6,
                            PackedByteWidth::Seven => Family::LoadPacked7,
                        }
                    } else {
                        Family::StorePacked
                    },
                    variant: 0,
                }
            }

            #[test]
            fn packed_memory_replays_exact_bytes_widths_offsets_and_aliases() {
                let model =
                    validate_physical_register_model(crate::x86_64_physical_register_model())
                        .unwrap();
                for names in [
                    ["rdi", "rax", "r10"],
                    ["r12", "r9", "r11"],
                    ["r13", "r8", "rbx"],
                ] {
                    let operands = names.map(|name| model.model().view_named(name).unwrap().id);
                    for width in [
                        PackedByteWidth::Three,
                        PackedByteWidth::Five,
                        PackedByteWidth::Six,
                        PackedByteWidth::Seven,
                    ] {
                        for load in [false, true] {
                            let kind = kind(load, width, 17);
                            let alternative = alternative(load, width);
                            let encoded = encode(&model, kind, alternative, &operands, 17).unwrap();
                            validate(&model, kind, alternative, &operands, 17, encoded.bytes())
                                .unwrap();
                            assert!(encoded.footprint().writes_rflags);
                            assert_eq!(
                                encoded.footprint().register_writes,
                                if load {
                                    vec![operands[1], operands[2]]
                                } else {
                                    vec![operands[2]]
                                }
                            );
                            if load {
                                assert_eq!(
                                    encoded.footprint().encoded.memory,
                                    MachineEncodedMemoryEffect::ReadPointerV1 {
                                        pointer_operand: 0,
                                        byte_count: u16::from(width.byte_size()),
                                    }
                                );
                            }
                            replay_battery::replay_rejects_byte_mutations(
                                encoded.bytes(),
                                |bytes| validate(&model, kind, alternative, &operands, 17, bytes),
                            );
                            replay_battery::replay_rejects_substituted_requests(
                                encoded.bytes(),
                                kind,
                                alternative,
                                &operands,
                                17,
                                |kind, alternative, operands, displacement, bytes| {
                                    validate(
                                        &model,
                                        kind,
                                        alternative,
                                        operands,
                                        displacement,
                                        bytes,
                                    )
                                },
                            );
                            for pair in [(0, 2), (1, 2), (0, 1)] {
                                let mut changed = operands;
                                changed[pair.1] = changed[pair.0];
                                assert_eq!(
                                    encode(&model, kind, alternative, &changed, 17).is_err(),
                                    load || pair != (0, 1)
                                );
                            }
                        }
                    }
                }
                let operands =
                    ["rdi", "rax", "r10"].map(|name| model.model().view_named(name).unwrap().id);
                let width = PackedByteWidth::Three;
                let load = encode(
                    &model,
                    kind(true, width, 0),
                    alternative(true, width),
                    &operands,
                    0,
                )
                .unwrap();
                assert_eq!(
                    load.bytes(),
                    &[
                        0x48, 0x0f, 0xb6, 0x87, 0, 0, 0, 0, 0x4c, 0x0f, 0xb6, 0x97, 1, 0, 0, 0,
                        0x49, 0xc1, 0xe2, 8, 0x4c, 0x09, 0xd0, 0x4c, 0x0f, 0xb6, 0x97, 2, 0, 0, 0,
                        0x49, 0xc1, 0xe2, 16, 0x4c, 0x09, 0xd0,
                    ]
                );
                for load in [false, true] {
                    let last = i32::MAX as u32 - 2;
                    assert!(
                        encode(
                            &model,
                            kind(load, width, last),
                            alternative(load, width),
                            &operands,
                            i64::from(last)
                        )
                        .is_ok()
                    );
                    for offset in [last + 1, u32::MAX] {
                        assert!(
                            encode(
                                &model,
                                kind(load, width, offset),
                                alternative(load, width),
                                &operands,
                                i64::from(offset)
                            )
                            .is_err()
                        );
                    }
                    let three = encode(
                        &model,
                        kind(load, width, 0),
                        alternative(load, width),
                        &operands,
                        0,
                    )
                    .unwrap();
                    assert!(
                        validate(
                            &model,
                            kind(load, PackedByteWidth::Five, 0),
                            alternative(load, PackedByteWidth::Five),
                            &operands,
                            0,
                            three.bytes()
                        )
                        .is_err()
                    );
                }
            }

            #[test]
            fn packed_memory_constraints_and_effects_retain_early_writes_and_flags() {
                let model =
                    validate_physical_register_model(crate::x86_64_physical_register_model())
                        .unwrap();
                let catalog = crate::x86_64_register_constraint_catalog(&model);
                let constraints =
                    crate::validate_x86_64_register_constraint_catalog(catalog.clone(), &model)
                        .unwrap();
                let effects = crate::x86_64_machine_effect_catalog(
                    target::NativeTarget::linux_x64(),
                    &constraints,
                )
                .unwrap();
                for (key, load) in [
                    (crate::X86_64_LOAD_PACKED, true),
                    (crate::X86_64_STORE_PACKED, false),
                ] {
                    let row = catalog.constraint(key).unwrap();
                    assert_eq!(row.operands.len(), 3);
                    assert_eq!(
                        row.clobbers,
                        model.model().view_named("condition_flags").unwrap().units
                    );
                    for (position, operand) in row.operands.iter().enumerate() {
                        let writes = position == 2 || (load && position == 1);
                        assert_eq!(
                            operand.access,
                            if writes {
                                RegisterOperandAccess::Def
                            } else {
                                RegisterOperandAccess::Use
                            }
                        );
                        assert_eq!(operand.early_clobber, writes);
                        assert!(operand.fixed_view.is_none());
                    }
                    for mutation in 0..3 {
                        let mut changed = catalog.clone();
                        let row = register_model::find_register_constraint_mut(
                            &mut changed.constraints,
                            key,
                        )
                        .unwrap();
                        match mutation {
                            0 => row.operands[2].early_clobber = false,
                            1 => row.clobbers.clear(),
                            _ => row.operands[2].access = RegisterOperandAccess::Use,
                        }
                        assert!(
                            crate::validate_x86_64_register_constraint_catalog(changed, &model)
                                .is_err()
                        );
                    }
                }
                for (semantic, width) in [
                    (MachineSemanticKind::LoadPacked3, 3),
                    (MachineSemanticKind::LoadPacked5, 5),
                    (MachineSemanticKind::LoadPacked6, 6),
                    (MachineSemanticKind::LoadPacked7, 7),
                ] {
                    let declaration = effects.declaration_by_semantic(semantic).unwrap();
                    assert_eq!(
                        declaration.alternatives[0].encoded.memory,
                        MachineEncodedMemoryEffect::ReadPointerV1 {
                            pointer_operand: 0,
                            byte_count: width
                        }
                    );
                    assert_eq!(
                        declaration.alternatives[0].encoded.external_operand_writes,
                        [1, 2]
                    );
                    for mutation in 0..3 {
                        let mut changed = effects.clone();
                        let encoded = &mut changed
                            .declaration_by_semantic_mut(semantic)
                            .unwrap()
                            .alternatives[0]
                            .encoded;
                        match mutation {
                            0 => encoded.implicit_unit_clobbers.clear(),
                            1 => encoded.external_operand_writes.truncate(1),
                            _ => {
                                encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                                    pointer_operand: 0,
                                    byte_count: 8,
                                }
                            }
                        }
                        assert!(
                            crate::validate_x86_64_machine_effect_catalog(
                                target::NativeTarget::linux_x64(),
                                &constraints,
                                changed
                            )
                            .is_err()
                        );
                    }
                }
            }
        }
    }
    #[cfg(test)]
    mod placed_scalar_tests {
        use super::replay_battery;
        use selected_instructions::{
            AccessorScalarForm, MachineAlternativeFamily as Family, MachineAlternativeKey,
            MachineBarrier, MachineEncodedMemoryEffect, MachineSemanticKind,
            SelectedInstructionKind as Kind,
        };

        fn accessor_scalar_request(form: AccessorScalarForm) -> (Kind, MachineAlternativeKey) {
            (
                Kind::AccessorScalarAccess {
                    form,
                    contract: [3; 32],
                    byte_offset: 8,
                },
                MachineAlternativeKey {
                    family: Family::AccessorScalarAccess(form),
                    variant: 0,
                },
            )
        }

        #[test]
        fn accessor_scalar_forms_decode_one_exact_width_transfer() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let constraints = crate::validate_x86_64_register_constraint_catalog(
                crate::x86_64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap();
            let target = target::NativeTarget::linux_x64();
            let catalog = crate::validate_x86_64_machine_effect_catalog(
                target,
                &constraints,
                crate::x86_64_machine_effect_catalog(target, &constraints).unwrap(),
            )
            .unwrap();
            let base = physical.model().view_named("r12").unwrap().id;
            let data = physical.model().view_named("r11").unwrap().id;
            for form in AccessorScalarForm::ALL {
                let (kind, alternative) = accessor_scalar_request(form);
                let declaration = catalog
                    .catalog()
                    .declaration_by_semantic(MachineSemanticKind::AccessorScalarAccess(form))
                    .unwrap();
                assert_eq!(declaration.barrier, MachineBarrier::ExternalEffect);
                let encoded = super::encode_x86_64_selected_memory_form(
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
                    super::validate_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        operands,
                        displacement,
                        bytes,
                    )
                };
                replay_battery::replay_rejects_byte_mutations(encoded.bytes(), |bytes| {
                    verify(kind, alternative, &[base, data], 8, bytes)
                });
                replay_battery::replay_rejects_substituted_requests(
                    encoded.bytes(),
                    kind,
                    alternative,
                    &[base, data],
                    8,
                    verify,
                );
                let verify_request = |kind: Kind, alternative: MachineAlternativeKey| {
                    verify(kind, alternative, &[base, data], 8, encoded.bytes())
                };
                assert!(verify(kind, alternative, &[data, base], 8, encoded.bytes()).is_err());
                assert!(verify(kind, alternative, &[data, data], 8, encoded.bytes()).is_err());
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
            }
        }
    }
    #[cfg(test)]
    mod pointer_tests {
        use super::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
            RegisterViewId, SelectedInstructionKind, ValidatedPhysicalRegisterModel,
            encode_x86_64_selected_memory_form, replay_battery,
            validate_x86_64_selected_memory_form,
        };
        #[test]
        fn pointer_store_known_bytes_preserve_the_requested_width() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let operands = ["r12", "r9"].map(|name| physical.model().view_named(name).unwrap().id);
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::Store,
                variant: 0,
            };
            for (byte_size, expected) in [
                (1, vec![0x45, 0x88, 0x8c, 0x24, 8, 0, 0, 0]),
                (2, vec![0x66, 0x45, 0x89, 0x8c, 0x24, 8, 0, 0, 0]),
                (4, vec![0x45, 0x89, 0x8c, 0x24, 8, 0, 0, 0]),
                (8, vec![0x4d, 0x89, 0x8c, 0x24, 8, 0, 0, 0]),
            ] {
                let kind = SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size,
                };
                let encoded =
                    encode_x86_64_selected_memory_form(&physical, kind, alternative, &operands, 8)
                        .unwrap();
                assert_eq!(encoded.bytes(), expected);
                assert!(
                    validate_x86_64_selected_memory_form(
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
                            validate_x86_64_selected_memory_form(
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
            let address = encode_x86_64_selected_memory_form(
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
            assert_eq!(address.bytes(), [0x4d, 0x8d, 0x8c, 0x24, 2, 0, 0, 0]);
        }

        #[test]
        fn pointer_stores_and_offsets_replay_every_bit_and_operand() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            for base_name in ["rax", "rbp", "rsi", "r8", "r12", "r15"] {
                for value_name in ["rax", "rbp", "rsi", "r8", "r12", "r15"] {
                    let operands = [base_name, value_name]
                        .map(|name| physical.model().view_named(name).unwrap().id);
                    for byte_size in [1, 2, 4, 8] {
                        for byte_offset in [0, u32::from(byte_size), i32::MAX as u32] {
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
                    for byte_offset in [0, 2, 4095] {
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
            let encoded = encode_x86_64_selected_memory_form(
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
            replay_battery::replay_rejects_byte_mutations(encoded.bytes(), |bytes| {
                validate_x86_64_selected_memory_form(
                    physical,
                    kind,
                    alternative,
                    operands,
                    displacement,
                    bytes,
                )
            });
            replay_battery::replay_rejects_substituted_requests(
                encoded.bytes(),
                kind,
                alternative,
                operands,
                displacement,
                |kind, alternative, operands, displacement, bytes| {
                    validate_x86_64_selected_memory_form(
                        physical,
                        kind,
                        alternative,
                        operands,
                        displacement,
                        bytes,
                    )
                },
            );
        }

        #[test]
        fn pointer_stores_reject_unsupported_widths_and_offsets() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let operands = ["rax", "r9"].map(|name| physical.model().view_named(name).unwrap().id);
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::Store,
                variant: 0,
            };
            for (byte_size, byte_offset) in [(0, 0), (3, 0), (16, 0), (8, u32::MAX)] {
                assert!(
                    encode_x86_64_selected_memory_form(
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
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let address_view = physical.model().view_named("r9").unwrap().id;
            let pointer_view = physical.model().view_named("r10").unwrap().id;
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
            let encoded = encode_x86_64_selected_memory_form(
                &physical,
                address_kind,
                address_key,
                &[address_view],
                48,
            )
            .unwrap();
            validate_x86_64_selected_memory_form(
                &physical,
                address_kind,
                address_key,
                &[address_view],
                48,
                encoded.bytes(),
            )
            .unwrap();
            assert!(
                validate_x86_64_selected_memory_form(
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
            let load = encode_x86_64_selected_memory_form(
                &physical,
                load_kind,
                load_key,
                &[address_view, pointer_view],
                0,
            )
            .unwrap();
            validate_x86_64_selected_memory_form(
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
                encode_x86_64_selected_memory_form(
                    &physical,
                    store_kind,
                    store_key,
                    &[pointer_view],
                    48
                )
                .is_err()
            );
            assert!(
                validate_x86_64_selected_memory_form(
                    &physical,
                    store_kind,
                    store_key,
                    &[pointer_view],
                    48,
                    encoded.bytes()
                )
                .is_err()
            );
        }

        #[test]
        fn activation_base_encodes_only_the_unshifted_stack_pointer() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            let address = physical.model().view_named("r9").unwrap().id;
            let kind = SelectedInstructionKind::FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                byte_offset: 0,
            };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::FrameAddress,
                variant: 0,
            };
            let encoded =
                encode_x86_64_selected_memory_form(&physical, kind, alternative, &[address], 0)
                    .unwrap();
            assert_eq!(encoded.bytes(), [0x4c, 0x8d, 0x8c, 0x24, 0, 0, 0, 0]);
            validate_x86_64_selected_memory_form(
                &physical,
                kind,
                alternative,
                &[address],
                0,
                encoded.bytes(),
            )
            .unwrap();
            assert!(
                encode_x86_64_selected_memory_form(&physical, kind, alternative, &[address], 8)
                    .is_err()
            );
            assert!(
                validate_x86_64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &[address],
                    8,
                    encoded.bytes()
                )
                .is_err()
            );
            let shifted = SelectedInstructionKind::FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::ActivationBase,
                byte_offset: 8,
            };
            assert!(
                encode_x86_64_selected_memory_form(&physical, shifted, alternative, &[address], 0)
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
            let value = physical.model().view_named("r10").unwrap().id;
            assert!(
                encode_x86_64_selected_memory_form(
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
                    validate_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &[address],
                        0,
                        &changed
                    )
                    .is_err()
                );
            }
        }
    }
    #[cfg(test)]
    mod replay_battery {
        use super::{ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError};
        use register_model::RegisterViewId;
        use selected_instructions::{MachineAlternativeKey, SelectedInstructionKind};

        pub(super) fn replay_rejects_byte_mutations(
            encoded: &[u8],
            replay: impl Fn(
                &[u8],
            ) -> Result<
                ValidatedX86_64SelectedFormEncoding,
                X86_64SelectedFormEncodingError,
            >,
        ) {
            assert!(replay(encoded).is_ok(), "the exact stream must replay");
            for bit in 0..encoded.len() * 8 {
                let mut changed = encoded.to_vec();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    replay(&changed).is_err(),
                    "accepted changed encoding bit {bit}"
                );
            }
            for end in 0..encoded.len() {
                assert!(
                    replay(&encoded[..end]).is_err(),
                    "accepted truncated prefix {end}"
                );
            }
            let mut extended = encoded.to_vec();
            extended.push(0);
            assert!(replay(&extended).is_err(), "accepted an appended byte");
        }

        pub(super) fn replay_rejects_substituted_requests(
            encoded: &[u8],
            kind: SelectedInstructionKind,
            alternative: MachineAlternativeKey,
            operands: &[RegisterViewId],
            displacement: i64,
            replay: impl Fn(
                SelectedInstructionKind,
                MachineAlternativeKey,
                &[RegisterViewId],
                i64,
                &[u8],
            ) -> Result<
                ValidatedX86_64SelectedFormEncoding,
                X86_64SelectedFormEncodingError,
            >,
        ) {
            let undeclared = MachineAlternativeKey {
                family: alternative.family,
                variant: alternative.variant + 1,
            };
            assert!(
                replay(kind, undeclared, operands, displacement, encoded).is_err(),
                "accepted an undeclared alternative variant"
            );
            assert!(
                replay(kind, alternative, operands, displacement + 1, encoded).is_err(),
                "accepted a neighbouring displacement"
            );
            let shorter = &operands[..operands.len().saturating_sub(1)];
            assert!(
                replay(kind, alternative, shorter, displacement, encoded).is_err(),
                "accepted a shorter operand list"
            );
        }
    }
    fn accessor_scalar_target(
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
    ) -> Result<
        Option<(SelectedInstructionKind, MachineAlternativeKey)>,
        X86_64SelectedFormEncodingError,
    > {
        let SelectedInstructionKind::AccessorScalarAccess {
            form, byte_offset, ..
        } = kind
        else {
            return Ok(None);
        };
        if alternative.family != MachineAlternativeFamily::AccessorScalarAccess(form)
            || alternative.variant != 0
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let ordinary = form
            .ordinary_kind(byte_offset)
            .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        let family = form
            .ordinary_semantic()
            .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?
            .into();
        Ok(Some((
            ordinary,
            MachineAlternativeKey { family, variant: 0 },
        )))
    }

    pub fn encode_x86_64_selected_memory_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        if let Some((ordinary, alternative)) = accessor_scalar_target(kind, alternative)? {
            return encode_x86_64_selected_memory_form(
                physical,
                ordinary,
                alternative,
                operands,
                displacement,
            );
        }
        if matches!(
            kind,
            SelectedInstructionKind::Store64 { .. } | SelectedInstructionKind::FrameAddress { .. }
        ) {
            return frame::encode(physical, kind, alternative, operands, displacement, 1);
        }
        if matches!(
            kind,
            SelectedInstructionKind::SaveFloatingControl { .. }
                | SelectedInstructionKind::RestoreFloatingControl { .. }
        ) {
            return super::floating_control::encode_x86_64_selected_floating_control_form(
                physical,
                kind,
                alternative,
                operands,
                displacement,
            );
        }
        if matches!(
            kind,
            SelectedInstructionKind::LoadPacked { .. }
                | SelectedInstructionKind::StorePacked { .. }
        ) {
            return packed::encode(physical, kind, alternative, operands, displacement);
        }
        if kind == SelectedInstructionKind::Load8Indexed {
            return indexed::encode(physical, alternative, operands, displacement);
        }
        let (opcode, register, base, _) = request(physical, kind, alternative, operands)?;
        let displacement = i32::try_from(displacement)
            .map_err(|_| X86_64SelectedFormEncodingError::ImmediateOutsideU12)?;
        let width = store_width(kind)?;
        let narrow_load = matches!(
            kind,
            SelectedInstructionKind::Load8 { .. } | SelectedInstructionKind::Load16 { .. }
        );
        let mut bytes = Vec::new();
        if width == 2 && !narrow_load {
            bytes.push(0x66);
        }
        let prefix = rex(register, 0, base)
            & if width == 8 || narrow_load {
                0xff
            } else {
                0xf7
            };
        bytes.push(prefix);
        if narrow_load {
            bytes.push(0x0f);
        }
        bytes.extend_from_slice(&[opcode, modrm(2, register, base)]);
        if base & 7 == 4 {
            bytes.push(0x24);
        }
        crate::codec_bytes::push_encoded(&mut bytes, displacement.to_le_bytes());
        validate_x86_64_selected_memory_form(
            physical,
            kind,
            alternative,
            operands,
            i64::from(displacement),
            &bytes,
        )
    }

    pub fn validate_x86_64_selected_memory_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        if let Some((ordinary, alternative)) = accessor_scalar_target(kind, alternative)? {
            return validate_x86_64_selected_memory_form(
                physical,
                ordinary,
                alternative,
                operands,
                displacement,
                bytes,
            );
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
        if matches!(
            kind,
            SelectedInstructionKind::SaveFloatingControl { .. }
                | SelectedInstructionKind::RestoreFloatingControl { .. }
        ) {
            return super::floating_control::validate_x86_64_selected_floating_control_form(
                physical,
                kind,
                alternative,
                operands,
                displacement,
                bytes,
            );
        }
        if matches!(
            kind,
            SelectedInstructionKind::LoadPacked { .. }
                | SelectedInstructionKind::StorePacked { .. }
        ) {
            return packed::validate(physical, kind, alternative, operands, displacement, bytes);
        }
        if kind == SelectedInstructionKind::Load8Indexed {
            return indexed::validate(physical, alternative, operands, displacement, bytes);
        }
        let (opcode, register, base, footprint) = request(physical, kind, alternative, operands)?;
        let width = store_width(kind)?;
        let narrow_load = matches!(
            kind,
            SelectedInstructionKind::Load8 { .. } | SelectedInstructionKind::Load16 { .. }
        );
        if matches!(kind, selected_instructions::load_width_triple_pattern!(byte_offset, ..) | SelectedInstructionKind::Store { byte_offset, .. } | SelectedInstructionKind::AddressOffset { byte_offset } if i64::from(byte_offset) != displacement)
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let instruction = if width == 2 && !narrow_load {
            bytes
                .strip_prefix(&[0x66])
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?
        } else {
            bytes
        };
        let prefix = *instruction
            .first()
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        let opcode_position = if narrow_load { 2 } else { 1 };
        let mode_position = opcode_position + 1;
        let mode = *instruction
            .get(mode_position)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        let has_sib = mode & 7 == 4;
        let displacement_start = mode_position + 1 + usize::from(has_sib);
        let actual = instruction
            .get(displacement_start..)
            .filter(|tail| tail.len() == 4)
            .and_then(|tail| <[u8; 4]>::try_from(tail).ok())
            .map(i32::from_le_bytes)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        let decoded_register = ((mode >> 3) & 7) | ((prefix & 4) << 1);
        let decoded_base = (mode & 7) | ((prefix & 1) << 3);
        if prefix & 0xfa
            != (if width == 8 || narrow_load {
                0x48
            } else {
                0x40
            })
            || (narrow_load && instruction.get(1) != Some(&0x0f))
            || instruction[opcode_position] != opcode
            || mode >> 6 != 2
            || decoded_register != register
            || decoded_base != base
            || (has_sib && instruction.get(mode_position + 1) != Some(&0x24))
            || i64::from(actual) != displacement
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint,
        })
    }

    fn store_width(kind: SelectedInstructionKind) -> Result<u8, X86_64SelectedFormEncodingError> {
        match kind {
            SelectedInstructionKind::Store {
                byte_size: width @ (1 | 2 | 4 | 8),
                ..
            } => Ok(width),
            SelectedInstructionKind::Store { .. } => {
                Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
            }
            SelectedInstructionKind::Load8 { .. } => Ok(1),
            SelectedInstructionKind::Load16 { .. } => Ok(2),
            SelectedInstructionKind::Load32 { .. } => Ok(4),
            _ => Ok(8),
        }
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<(u8, u8, u8, X86_64SelectedFormFootprint), X86_64SelectedFormEncodingError> {
        if matches!(
            kind,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Incoming { .. },
                ..
            }
        ) {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        super::request_validation::guard_canonical(physical)?;
        let (family, count, opcode, key) = match kind {
            SelectedInstructionKind::Store { .. } => (
                MachineAlternativeFamily::Store,
                2,
                if store_width(kind)? == 1 { 0x88 } else { 0x89 },
                crate::X86_64_STORE,
            ),
            SelectedInstructionKind::AddressOffset { .. } => (
                MachineAlternativeFamily::AddressOffset,
                2,
                0x8d,
                crate::X86_64_ADDRESS_OFFSET,
            ),
            SelectedInstructionKind::Load8 { .. } => (
                MachineAlternativeFamily::Load8,
                2,
                0xb6,
                crate::X86_64_LOAD8,
            ),
            SelectedInstructionKind::Load16 { .. } => (
                MachineAlternativeFamily::Load16,
                2,
                0xb7,
                crate::X86_64_LOAD16,
            ),
            SelectedInstructionKind::Load32 { .. } => (
                MachineAlternativeFamily::Load32,
                2,
                0x8b,
                crate::X86_64_LOAD32,
            ),
            SelectedInstructionKind::Load64 { .. } => (
                MachineAlternativeFamily::Load64,
                2,
                0x8b,
                crate::X86_64_LOAD64,
            ),
            _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
        };
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
        }
        if operands.len() != count {
            return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
        }
        let registers = resolve_registers(physical, operands)?;
        let (register, base) = if count == 2 {
            (registers[1], registers[0])
        } else {
            (registers[0], 4)
        };
        let catalog = crate::x86_64_register_constraint_catalog_for(physical);
        let constraint = catalog
            .constraint(key)
            .expect("canonical memory constraint");
        let (reads, writes, memory, trap) = match kind {
            SelectedInstructionKind::Store { .. } => (
                vec![0, 1],
                vec![],
                MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            ),
            SelectedInstructionKind::AddressOffset { .. } => (
                vec![0],
                vec![1],
                MachineEncodedMemoryEffect::NoneV1,
                MachineEncodedTrapBehavior::NeverV1,
            ),
            selected_instructions::load_width_triple_pattern!(..)
            | SelectedInstructionKind::Load64 { .. } => (
                vec![0],
                vec![1],
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: u16::from(store_width(kind)?),
                },
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            ),
            _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
        };
        let footprint = X86_64SelectedFormFootprint {
            register_reads: reads
                .iter()
                .map(|index| operands[*index as usize])
                .collect(),
            register_writes: writes
                .iter()
                .map(|index| operands[*index as usize])
                .collect(),
            writes_rflags: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: reads,
                external_operand_writes: writes,
                implicit_unit_uses: constraint.implicit_uses.clone(),
                implicit_unit_defs: constraint.implicit_defs.clone(),
                implicit_unit_clobbers: constraint.clobbers.clone(),
                memory,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        };
        Ok((opcode, register, base, footprint))
    }

    #[cfg(test)]
    mod tests {
        use super::{
            MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
            X86_64SelectedFormEncodingError, encode_x86_64_selected_memory_form, replay_battery,
            validate_x86_64_selected_memory_form,
        };
        use register_model::validate_physical_register_model;
        use selected_instructions::OutgoingArgumentSlotId;
        use semantic_vocabulary::OperationId;

        #[test]
        fn local_frame_forms_reject_corrupt_stack_base_and_address_bytes() {
            let physical =
                validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
            let slot = selected_instructions::FrameStorageSlotId::Local(
                selected_instructions::LocalStorageSlotId::Structural {
                    operation: OperationId::new(104).unwrap(),
                    place: semantic_vocabulary::PlaceId::new(102).unwrap(),
                },
            );
            let value = physical.model().view_named("r9").unwrap().id;
            let scratch = physical.model().view_named("r10").unwrap().id;
            for (kind, family) in [
                (
                    SelectedInstructionKind::Store64 {
                        slot,
                        byte_offset: 0,
                    },
                    MachineAlternativeFamily::Store64,
                ),
                (
                    SelectedInstructionKind::FrameAddress {
                        slot,
                        byte_offset: 0,
                    },
                    MachineAlternativeFamily::FrameAddress,
                ),
            ] {
                let operands = if family == MachineAlternativeFamily::Store64 {
                    vec![value, scratch]
                } else {
                    vec![value]
                };
                let alternative = MachineAlternativeKey { family, variant: 0 };
                let encoded =
                    encode_x86_64_selected_memory_form(&physical, kind, alternative, &operands, 24)
                        .unwrap();
                crate::selected_form_encoding::assert_every_bit_flip_rejected(
                    encoded.bytes(),
                    |corrupt| {
                        validate_x86_64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            24,
                            corrupt,
                        )
                        .is_err()
                    },
                );
            }
        }

        #[test]
        fn stack_pointer_is_not_an_allocatable_pointer_operand() {
            let physical =
                validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
            let stack_pointer = physical.model().view_named("rsp").unwrap().id;
            let result = physical.model().view_named("r11").unwrap().id;
            assert_eq!(
                encode_x86_64_selected_memory_form(
                    &physical,
                    SelectedInstructionKind::Load64 { byte_offset: 0 },
                    MachineAlternativeKey {
                        family: MachineAlternativeFamily::Load64,
                        variant: 0
                    },
                    &[stack_pointer, result],
                    0,
                ),
                Err(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(
                    stack_pointer
                )),
            );
        }

        #[test]
        fn frame_forms_encode_signed_displacements_below_the_stack_pointer() {
            let physical =
                validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
            let slot = selected_instructions::FrameStorageSlotId::Local(
                selected_instructions::LocalStorageSlotId::Spill {
                    register: selected_instructions::VirtualRegisterId(7),
                },
            );
            let value = physical.model().view_named("r11").unwrap().id;
            let scratch = physical.model().view_named("r10").unwrap().id;
            for (kind, family, expected) in [
                (
                    SelectedInstructionKind::Store64 {
                        slot,
                        byte_offset: 0,
                    },
                    MachineAlternativeFamily::Store64,
                    vec![
                        0x4c, 0x8d, 0x94, 0x24, 0xf0, 0xff, 0xff, 0xff, 0x4d, 0x89, 0x9a, 0, 0, 0,
                        0,
                    ],
                ),
                (
                    SelectedInstructionKind::FrameAddress {
                        slot,
                        byte_offset: 8,
                    },
                    MachineAlternativeFamily::FrameAddress,
                    vec![0x4c, 0x8d, 0x9c, 0x24, 0xf0, 0xff, 0xff, 0xff],
                ),
            ] {
                let operands = if family == MachineAlternativeFamily::Store64 {
                    vec![value, scratch]
                } else {
                    vec![value]
                };
                let alternative = MachineAlternativeKey { family, variant: 0 };
                let encoded = encode_x86_64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    -16,
                )
                .unwrap();
                assert_eq!(encoded.bytes, expected, "{kind:?}");
                assert_eq!(
                    validate_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        -16,
                        &encoded.bytes
                    ),
                    Ok(encoded.clone())
                );
                for displacement in [-24, -8, 0, i64::from(i32::MAX)] {
                    assert!(
                        validate_x86_64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            displacement,
                            &encoded.bytes
                        )
                        .is_err(),
                        "{kind:?} {displacement}"
                    );
                }
                for byte_index in 0..encoded.bytes.len() {
                    let mut changed = encoded.bytes.clone();
                    changed[byte_index] ^= 1;
                    assert!(
                        validate_x86_64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            -16,
                            &changed
                        )
                        .is_err(),
                        "{kind:?} byte {byte_index}"
                    );
                }
            }
            let kind = SelectedInstructionKind::Store64 {
                slot,
                byte_offset: 0,
            };
            for displacement in [i64::from(i32::MIN) - 1, i64::from(i32::MAX) + 1] {
                assert_eq!(
                    encode_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        MachineAlternativeKey {
                            family: MachineAlternativeFamily::Store64,
                            variant: 0
                        },
                        &[value, scratch],
                        displacement,
                    ),
                    Err(X86_64SelectedFormEncodingError::ImmediateOutsideU12)
                );
            }
            let encoded = encode_x86_64_selected_memory_form(
                &physical,
                kind,
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::Store64,
                    variant: 0,
                },
                &[value, scratch],
                i64::from(i32::MIN),
            )
            .unwrap();
            assert_eq!(
                validate_x86_64_selected_memory_form(
                    &physical,
                    kind,
                    MachineAlternativeKey {
                        family: MachineAlternativeFamily::Store64,
                        variant: 0
                    },
                    &[value, scratch],
                    i64::from(i32::MIN),
                    &encoded.bytes
                ),
                Ok(encoded)
            );
        }

        #[test]
        fn ordinary_memory_forms_replay_registers_displacements_and_every_encoded_byte() {
            let physical =
                validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
            let slot =
                selected_instructions::FrameStorageSlotId::Outgoing(OutgoingArgumentSlotId {
                    role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                    invocation_site: OperationId::new(7).unwrap().into(),
                    argument_index: 1,
                });
            for name in ["rax", "rbp", "r8", "r12", "r15"] {
                let pointer = physical.model().view_named(name).unwrap().id;
                let value = physical.model().view_named("r11").unwrap().id;
                for (kind, family, operands) in [
                    (
                        SelectedInstructionKind::Load64 { byte_offset: 8 },
                        MachineAlternativeFamily::Load64,
                        vec![pointer, value],
                    ),
                    (
                        SelectedInstructionKind::Store64 {
                            slot,
                            byte_offset: 8,
                        },
                        MachineAlternativeFamily::Store64,
                        vec![value, pointer],
                    ),
                    (
                        SelectedInstructionKind::FrameAddress {
                            slot,
                            byte_offset: 0,
                        },
                        MachineAlternativeFamily::FrameAddress,
                        vec![value],
                    ),
                ] {
                    let alternative = MachineAlternativeKey { family, variant: 0 };
                    let encoded = encode_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        56,
                    )
                    .unwrap();
                    if name == "r12" {
                        let expected = match kind {
                            SelectedInstructionKind::Load64 { .. } => {
                                vec![0x4d, 0x8b, 0x9c, 0x24, 56, 0, 0, 0]
                            }
                            SelectedInstructionKind::Store64 { .. } => {
                                vec![
                                    0x4c, 0x8d, 0xa4, 0x24, 56, 0, 0, 0, 0x4d, 0x89, 0x9c, 0x24, 0,
                                    0, 0, 0,
                                ]
                            }
                            SelectedInstructionKind::FrameAddress { .. } => {
                                vec![0x4c, 0x8d, 0x9c, 0x24, 56, 0, 0, 0]
                            }
                            _ => unreachable!(),
                        };
                        assert_eq!(encoded.bytes, expected);
                    }
                    replay_battery::replay_rejects_byte_mutations(&encoded.bytes, |bytes| {
                        validate_x86_64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            56,
                            bytes,
                        )
                    });
                    replay_battery::replay_rejects_substituted_requests(
                        &encoded.bytes,
                        kind,
                        alternative,
                        &operands,
                        56,
                        |kind, alternative, operands, displacement, bytes| {
                            validate_x86_64_selected_memory_form(
                                &physical,
                                kind,
                                alternative,
                                operands,
                                displacement,
                                bytes,
                            )
                        },
                    );
                }
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

    use crate::machine_effects::x86_64_selected_abi;

    pub(crate) const X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH: u8 = 4;
    pub(crate) const X86_64_CALLBACK_MATERIALIZATION_RELOCATION_OFFSET: u16 = 3;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum X86_64CallbackMaterializationDestination {
        Register(calling_conventions::MachineRegister),
        OutgoingStack {
            byte_offset: u32,
        },
        RecordField {
            root_register: calling_conventions::MachineRegister,
            placements: calling_conventions::PrivateCallbackPlacements,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum X86_64NormalizedForeignCallFixupKind {
        Relative32FromNextInstructionToNormalizedForeignImportV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum X86_64NormalizedForeignCallFixupState {
        UnresolvedImportFieldV1,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct X86_64CallbackAddressMaterialization {
        pub destination: X86_64CallbackMaterializationDestination,
        pub materialization_byte_offset: u16,
        pub materialization_byte_count: u16,
        pub relocation_byte_offset: u16,
    }

    pub type X86_64NormalizedForeignCallFixup = Fixup<
        X86_64NormalizedForeignCallFixupKind,
        X86_64NormalizedForeignCallFixupState,
        X86_64CallbackAddressMaterialization,
    >;

    pub type ValidatedX86_64SelectedNormalizedForeignCallTemplate =
        Template<X86_64NormalizedForeignCallFixup>;

    pub type X86_64NormalizedForeignCallTemplateError = normalized_foreign_call_plan::Error;

    pub fn encode_x86_64_selected_normalized_foreign_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        effects: &MachineEncodedEffects,
        callback_destinations: &[X86_64CallbackMaterializationDestination],
        record: &SelectedNormalizedForeignCall,
    ) -> Result<
        ValidatedX86_64SelectedNormalizedForeignCallTemplate,
        X86_64NormalizedForeignCallTemplateError,
    > {
        let (boundary, ordinal) = match kind {
            SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => {
                (boundary, ordinal)
            }
            _ => return Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch),
        };
        let mut bytes = Vec::new();
        for destination in callback_destinations {
            bytes.extend_from_slice(
                &callback_group_bytes(destination.clone())
                    .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
            );
        }
        bytes.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
        let fixup = callback_fixup(boundary, ordinal, callback_destinations)?;
        validate_x86_64_selected_normalized_foreign_call_template(
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

    pub fn validate_x86_64_selected_normalized_foreign_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        effects: &MachineEncodedEffects,
        bytes: &[u8],
        fixup: X86_64NormalizedForeignCallFixup,
        callback_destinations: &[X86_64CallbackMaterializationDestination],
        record: &SelectedNormalizedForeignCall,
    ) -> Result<
        ValidatedX86_64SelectedNormalizedForeignCallTemplate,
        X86_64NormalizedForeignCallTemplateError,
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
            |native| x86_64_selected_abi(native).is_ok(),
            crate::canonical_x86_64_physical_register_model_identity,
            crate::machine_effects::normalized_foreign_constraint,
            crate::x86_64_normalized_foreign_call_declaration,
        )?;
        let destinations = record
            .call
            .callbacks
            .iter()
            .map(|callback| {
                match (
                    &callback.destination,
                    callback.application.placement.locations.as_slice(),
                ) {
                    (
                        calling_conventions::NativePlace::Parameter(_),
                        [calling_conventions::ValueLocation::Register { register, .. }],
                    ) => Some(X86_64CallbackMaterializationDestination::Register(
                        *register,
                    )),
                    (
                        calling_conventions::NativePlace::Parameter(_),
                        [
                            calling_conventions::ValueLocation::Stack {
                                stack_byte_offset, ..
                            },
                        ],
                    ) => Some(X86_64CallbackMaterializationDestination::OutgoingStack {
                        byte_offset: *stack_byte_offset,
                    }),
                    (
                        calling_conventions::NativePlace::Field { .. },
                        [calling_conventions::ValueLocation::Register { register, .. }],
                    ) => Some(X86_64CallbackMaterializationDestination::RecordField {
                        root_register: *register,
                        placements: callback.field_placements.clone()?,
                    }),
                    _ => None,
                }
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
        if destinations != callback_destinations {
            return Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch);
        }
        let mut expected_bytes = Vec::new();
        for destination in callback_destinations {
            expected_bytes.extend_from_slice(
                &callback_group_bytes(destination.clone())
                    .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
            );
        }
        expected_bytes.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
        if bytes != expected_bytes.as_slice() {
            return Err(X86_64NormalizedForeignCallTemplateError::MalformedTemplate);
        }
        let expected_fixup = callback_fixup(boundary, ordinal, callback_destinations)?;
        if fixup != expected_fixup {
            return Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch);
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

    pub fn callback_group_bytes(
        destination: X86_64CallbackMaterializationDestination,
    ) -> Option<Vec<u8>> {
        match destination {
            X86_64CallbackMaterializationDestination::Register(register) => {
                let index = x86_64_machine_register_encoding(register)?;
                let rex = 0x48 | (u8::from(index >= 8) << 2);
                let modrm = 0x05 | ((index & 7) << 3);
                Some(vec![rex, 0x8d, modrm, 0, 0, 0, 0])
            }
            X86_64CallbackMaterializationDestination::OutgoingStack { byte_offset } => {
                if byte_offset > i32::MAX as u32 {
                    return None;
                }
                let mut group = vec![0x4c, 0x8d, 0x1d, 0, 0, 0, 0, 0x4c, 0x89];
                if byte_offset <= i8::MAX as u32 {
                    group.extend_from_slice(&[0x5c, 0x24, byte_offset as u8]);
                } else {
                    group.extend_from_slice(&[0x9c, 0x24]);
                    crate::codec_bytes::push_encoded(&mut group, byte_offset.to_le_bytes());
                }
                Some(group)
            }
            X86_64CallbackMaterializationDestination::RecordField {
                root_register,
                placements,
            } => {
                let mut group = vec![0x4c, 0x8d, 0x1d, 0, 0, 0, 0];
                for placement in placements.as_slice() {
                    crate::post_handoff_writer::append_fragment(
                        &mut group,
                        calling_conventions::MachineRegister::X86R11,
                        root_register,
                        calling_conventions::MachineRegister::X86R10,
                        calling_conventions::MachineRegister::X86Rax,
                        *placement,
                    )?;
                }
                (!placements.as_slice().is_empty()).then_some(group)
            }
        }
    }

    pub(super) fn x86_64_machine_register_encoding(
        register: calling_conventions::MachineRegister,
    ) -> Option<u8> {
        Some(match register {
            calling_conventions::MachineRegister::X86Rax => 0,
            calling_conventions::MachineRegister::X86Rcx => 1,
            calling_conventions::MachineRegister::X86Rdx => 2,
            calling_conventions::MachineRegister::X86Rbx => 3,
            calling_conventions::MachineRegister::X86Rsi => 6,
            calling_conventions::MachineRegister::X86Rdi => 7,
            calling_conventions::MachineRegister::X86R8 => 8,
            calling_conventions::MachineRegister::X86R9 => 9,
            calling_conventions::MachineRegister::X86R10 => 10,
            calling_conventions::MachineRegister::X86R11 => 11,
            calling_conventions::MachineRegister::X86R12 => 12,
            calling_conventions::MachineRegister::X86R13 => 13,
            calling_conventions::MachineRegister::X86R14 => 14,
            calling_conventions::MachineRegister::X86R15 => 15,
            _ => return None,
        })
    }

    fn callback_fixup(
        boundary: BoundaryMachineId,
        ordinal: u32,
        destinations: &[X86_64CallbackMaterializationDestination],
    ) -> Result<X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallTemplateError> {
        let mut callbacks = Vec::with_capacity(destinations.len());
        let mut offset = 0_u16;
        for destination in destinations {
            let byte_count = u16::try_from(
                callback_group_bytes(destination.clone())
                    .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?
                    .len(),
            )
            .map_err(|_| X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
            callbacks.push(X86_64CallbackAddressMaterialization {
                destination: destination.clone(),
                materialization_byte_offset: offset,
                materialization_byte_count: byte_count,
                relocation_byte_offset: offset
                    .checked_add(X86_64_CALLBACK_MATERIALIZATION_RELOCATION_OFFSET)
                    .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
            });
            offset = offset
                .checked_add(byte_count)
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
        }
        let patch_byte_offset = offset
            .checked_add(1)
            .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
        let reference_byte_offset = offset
            .checked_add(5)
            .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
        Ok(X86_64NormalizedForeignCallFixup {
        kind: X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1,
        state: X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_byte_offset: offset,
        patch_byte_offset,
        reference_byte_offset,
        patch_byte_width: X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH,
        callbacks,
    })
    }

    #[cfg(test)]
    mod tests {
        use super::{
            X86_64NormalizedForeignCallTemplateError, callback_fixup,
            encode_x86_64_selected_normalized_foreign_call_template,
            validate_x86_64_selected_normalized_foreign_call_template,
        };
        use normalized_foreign_call_plan::fixture;
        use register_model::{RegisterOperandAccess, validate_physical_register_model};
        use selected_instructions::SelectedNormalizedForeignCall;
        use semantic_vocabulary::BoundaryMachineId;
        use target::NativeTarget;

        use crate::x86_64_physical_register_model;

        fn callback_destination(
            placement: fixture::CallbackPlacement,
        ) -> super::X86_64CallbackMaterializationDestination {
            match placement.field_byte_offset {
                None => {
                    super::X86_64CallbackMaterializationDestination::Register(placement.register)
                }
                Some(byte_offset) => super::X86_64CallbackMaterializationDestination::RecordField {
                    root_register: placement.register,
                    placements: calling_conventions::PrivateCallbackPlacements::new(&[
                        layout_plans::LayoutPlacementReport::At {
                            offset: u64::try_from(byte_offset).unwrap(),
                        },
                    ]),
                },
            }
        }

        const CALLBACK_BYTE_COUNTS: fixture::CallbackByteCounts = fixture::CallbackByteCounts {
            register: 7,
            field: 17,
        };

        fn inputs(windows: bool, input_layout: usize, result: usize) -> fixture::Case {
            fixture::case(
                validate_physical_register_model(x86_64_physical_register_model()).unwrap(),
                record(windows, input_layout, result),
                crate::machine_effects::normalized_foreign_constraint,
                |row, physical| {
                    crate::machine_effects::user_direct_call_effects(
                        row,
                        physical.model().view_named("rsp").unwrap().id,
                    )
                },
            )
        }

        fn record(
            windows: bool,
            input_layout: usize,
            result: usize,
        ) -> SelectedNormalizedForeignCall {
            use calling_conventions::ValueShape;
            use target::TargetProfile;

            fixture::record(
                if windows {
                    NativeTarget::windows_x64()
                } else {
                    NativeTarget::linux_x64()
                },
                7,
                if windows {
                    ValueShape::integer(16, 8)
                } else {
                    ValueShape::system_v_aggregate(
                        16,
                        8,
                        calling_conventions::SystemVEightbyteClass::Integer,
                        calling_conventions::SystemVEightbyteClass::Sse,
                    )
                },
                if windows {
                    fixture::pe_locator()
                } else {
                    fixture::elf_locator(TargetProfile::LinuxX64)
                },
                input_layout,
                result,
            )
        }

        type Entrances = fixture::Entrances<
            super::X86_64CallbackMaterializationDestination,
            super::X86_64NormalizedForeignCallFixupKind,
            super::X86_64NormalizedForeignCallFixupState,
            super::X86_64CallbackAddressMaterialization,
        >;

        fn entrances() -> Entrances {
            fixture::Entrances {
                call: fixture::CallBytes {
                    bytes: vec![0xe8, 0, 0, 0, 0],
                    patch_byte_offset: 1,
                    reference_byte_offset: 5,
                },
                constraint: crate::machine_effects::normalized_foreign_constraint,
                declaration: crate::x86_64_normalized_foreign_call_declaration,
                fixed_view: crate::x86_64_fixed_register_view,
                encode: encode_x86_64_selected_normalized_foreign_call_template,
                validate: validate_x86_64_selected_normalized_foreign_call_template,
                fixup: |boundary, ordinal, destinations| {
                    callback_fixup(boundary, ordinal, destinations).ok()
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
        fn table_calls_encode_the_retained_field_and_reject_scratch_aliases() {
            use super::super::table_call::{
                encode_x86_64_selected_table_call_form, validate_x86_64_selected_table_call_form,
            };
            for windows in [false, true] {
                for byte_offset in [0, 8, 65_536] {
                    let (_, _, alternative, operands, accesses, _, mut record) =
                        inputs(windows, 1, 1);
                    let same_stack_contribution = record
                        .call
                        .binding
                        .same_stack_contribution()
                        .unwrap()
                        .clone();
                    record.call.binding.mechanism =
                        target_operations::NormalizedForeignCallMechanism::Table {
                            target: if windows {
                                target::TargetProfile::WindowsX64
                            } else {
                                target::TargetProfile::LinuxX64
                            },
                            byte_offset,
                            dispatch_only: false,
                            same_stack_contribution,
                        };
                    let encoded = encode_x86_64_selected_table_call_form(
                        &record,
                        alternative,
                        &operands,
                        &accesses,
                    )
                    .unwrap();
                    assert_eq!(encoded.bytes.len(), 10);
                    assert_eq!(&encoded.bytes[3..7], &byte_offset.to_le_bytes());
                    assert_eq!(&encoded.bytes[7..], &[0x41, 0xff, 0xd3]);
                    for position in 0..encoded.bytes.len() {
                        let mut corrupted = encoded.bytes.clone();
                        corrupted[position] ^= 1;
                        assert!(
                            validate_x86_64_selected_table_call_form(
                                &record,
                                alternative,
                                &operands,
                                &accesses,
                                &corrupted,
                            )
                            .is_err(),
                            "changed byte {position}"
                        );
                    }
                    let calling_conventions::ValueLocation::Register { register, .. } =
                        &mut record.call.binding.boundary_entry_plan.call.parameters[0].locations
                            [0]
                    else {
                        panic!("one pointer argument must be in a register");
                    };
                    *register = calling_conventions::MachineRegister::X86R11;
                    assert!(
                        encode_x86_64_selected_table_call_form(
                            &record,
                            alternative,
                            &operands,
                            &accesses,
                        )
                        .is_err()
                    );
                }
            }
        }

        #[test]
        fn dispatch_only_table_is_loaded_from_scratch_without_shifting_wire_arguments() {
            use super::super::table_call::{
                encode_x86_64_selected_table_call_form, validate_x86_64_selected_table_call_form,
            };
            for windows in [false, true] {
                for wire_count in [0, 1] {
                    let (_, _, alternative, _, _, _, mut record) = inputs(windows, wire_count, 1);
                    let same_stack_contribution = record
                        .call
                        .binding
                        .same_stack_contribution()
                        .unwrap()
                        .clone();
                    let original_plan = record.call.binding.boundary_entry_plan.clone();
                    record.call.binding.mechanism =
                        target_operations::NormalizedForeignCallMechanism::Table {
                            target: if windows {
                                target::TargetProfile::WindowsX64
                            } else {
                                target::TargetProfile::LinuxX64
                            },
                            byte_offset: 16,
                            dispatch_only: true,
                            same_stack_contribution,
                        };
                    let constraint =
                        crate::machine_effects::normalized_foreign_constraint(&record).unwrap();
                    let views: Vec<_> = constraint
                        .operands
                        .iter()
                        .map(|operand| operand.fixed_view.unwrap())
                        .collect();
                    let accesses: Vec<_> = constraint
                        .operands
                        .iter()
                        .map(|operand| operand.access)
                        .collect();
                    let encoded = encode_x86_64_selected_table_call_form(
                        &record,
                        alternative,
                        &views,
                        &accesses,
                    )
                    .unwrap();
                    assert_eq!(
                        encoded.bytes,
                        [0x4d, 0x8b, 0x9b, 16, 0, 0, 0, 0x41, 0xff, 0xd3]
                    );
                    assert_eq!(record.call.binding.boundary_entry_plan, original_plan);
                    assert_eq!(
                        selected_instructions::calls::constraints::foreign_argument_registers(
                            &record.call
                        )[0],
                        (0, calling_conventions::MachineRegister::X86R11)
                    );
                    for position in 0..encoded.bytes.len() {
                        let mut corrupted = encoded.bytes.clone();
                        corrupted[position] ^= 1;
                        assert!(
                            validate_x86_64_selected_table_call_form(
                                &record,
                                alternative,
                                &views,
                                &accesses,
                                &corrupted
                            )
                            .is_err()
                        );
                    }
                }
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
        fn callback_stack_bytes_preserve_outgoing_offsets_and_fixup_extent() {
            use super::{X86_64CallbackMaterializationDestination, callback_group_bytes};
            for (offset, store) in [
                (0, vec![0x4c, 0x89, 0x5c, 0x24, 0]),
                (32, vec![0x4c, 0x89, 0x5c, 0x24, 32]),
                (127, vec![0x4c, 0x89, 0x5c, 0x24, 127]),
                (128, vec![0x4c, 0x89, 0x9c, 0x24, 128, 0, 0, 0]),
                (256, vec![0x4c, 0x89, 0x9c, 0x24, 0, 1, 0, 0]),
            ] {
                let destination = X86_64CallbackMaterializationDestination::OutgoingStack {
                    byte_offset: offset,
                };
                let bytes = callback_group_bytes(destination.clone()).unwrap();
                assert_eq!(&bytes[..7], &[0x4c, 0x8d, 0x1d, 0, 0, 0, 0]);
                assert_eq!(&bytes[7..], store);
                let fixup =
                    callback_fixup(BoundaryMachineId::new(3).unwrap(), 0, &[destination]).unwrap();
                assert_eq!(usize::from(fixup.opcode_byte_offset), bytes.len());
                assert_eq!(
                    usize::from(fixup.callbacks[0].materialization_byte_count),
                    bytes.len()
                );
                assert_eq!(fixup.callbacks[0].relocation_byte_offset, 3);
            }
            assert!(
                callback_group_bytes(X86_64CallbackMaterializationDestination::OutgoingStack {
                    byte_offset: i32::MAX as u32 + 1,
                })
                .is_none()
            );
        }

        #[test]
        fn callback_field_bytes_preserve_root_addressing_and_fixup_extent() {
            use super::{X86_64CallbackMaterializationDestination, callback_group_bytes};
            use calling_conventions::MachineRegister;
            for (root, store) in [
                (MachineRegister::X86Rdi, vec![0x4c, 0x89, 0x97, 24, 0, 0, 0]),
                (
                    MachineRegister::X86R12,
                    vec![0x4d, 0x89, 0x94, 0x24, 24, 0, 0, 0],
                ),
            ] {
                let destination = X86_64CallbackMaterializationDestination::RecordField {
                    root_register: root,
                    placements: calling_conventions::PrivateCallbackPlacements::new(&[
                        layout_plans::LayoutPlacementReport::At { offset: 24 },
                    ]),
                };
                let bytes = callback_group_bytes(destination.clone()).unwrap();
                let length = 7 + 3 + store.len();
                assert_eq!(bytes.len(), length);
                assert_eq!(&bytes[..7], &[0x4c, 0x8d, 0x1d, 0, 0, 0, 0]);
                assert_eq!(&bytes[7..10], &[0x4d, 0x89, 0xda]);
                assert_eq!(&bytes[10..], store.as_slice());
                let fixup =
                    callback_fixup(BoundaryMachineId::new(3).unwrap(), 0, &[destination]).unwrap();
                assert_eq!(usize::from(fixup.opcode_byte_offset), length);
                assert_eq!(
                    usize::from(fixup.callbacks[0].materialization_byte_count),
                    length
                );
            }
            for (root, byte_offset) in [
                (MachineRegister::X86R11, 0),
                (MachineRegister::X86R10, 0),
                (MachineRegister::X86Rax, 0),
                (MachineRegister::X86Rdi, u32::MAX),
            ] {
                assert!(
                    callback_group_bytes(X86_64CallbackMaterializationDestination::RecordField {
                        root_register: root,
                        placements: calling_conventions::PrivateCallbackPlacements::new(&[
                            layout_plans::LayoutPlacementReport::At {
                                offset: u64::from(byte_offset)
                            },
                        ]),
                    })
                    .is_none()
                );
            }
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
                0..63,
                "r11",
            );
        }

        #[test]
        fn microsoft_calls_select_their_exact_operands() {
            let (physical, kind, alternative, operands, accesses, effects, record) =
                inputs(true, 2, 1);
            assert!(
                encode_x86_64_selected_normalized_foreign_call_template(
                    NativeTarget::windows_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &accesses,
                    &effects,
                    &[],
                    &record
                )
                .is_ok()
            );
            assert_eq!(
                encode_x86_64_selected_normalized_foreign_call_template(
                    NativeTarget::windows_x64(),
                    &physical,
                    kind,
                    alternative,
                    &[physical.model().view_named("rdi").unwrap().id],
                    &[RegisterOperandAccess::Use],
                    &effects,
                    &[],
                    &record
                ),
                Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)
            );
        }

        #[test]
        fn rejects_wrong_kind_target_alternative_and_fixup() {
            fixture::rejects_wrong_kind_target_alternative_and_fixup(
                &entrances(),
                inputs,
                &[NativeTarget::macos_arm64(), NativeTarget::linux_arm64()],
                &[0xe8, 0, 0, 0, 1],
            );
        }
    }
}
pub use normalized_foreign_call::callback_group_bytes;
mod request_validation {
    use crate::selected_form_encoding::X86_64SelectedFormEncodingError;
    use crate::selected_form_encoding::saturating_forms::SaturatingForm;
    use crate::selected_form_encoding::trapping_forms::TrappingShape;
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily as Family, MachineAlternativeKey, SaturatingOperation,
        SelectedInstructionKind as Kind,
    };

    pub(crate) fn guard_canonical(
        physical: &ValidatedPhysicalRegisterModel,
    ) -> Result<(), X86_64SelectedFormEncodingError> {
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
            return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
        }
        Ok(())
    }

    pub(super) fn byte_loop_registers<const N: usize>(
        physical: &ValidatedPhysicalRegisterModel,
        requested: MachineAlternativeKey,
        family: Family,
        operands: &[RegisterViewId],
        preserved: usize,
    ) -> Result<[u8; N], X86_64SelectedFormEncodingError> {
        guard_canonical(physical)?;
        if requested != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let registers: [u8; N] = resolve_registers(physical, operands)?
            .try_into()
            .map_err(|_| X86_64SelectedFormEncodingError::OperandCountMismatch)?;
        for position in preserved..N {
            if registers[..position].contains(&registers[position]) {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
        }
        Ok(registers)
    }

    pub(crate) fn validate_request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<(), X86_64SelectedFormEncodingError> {
        guard_canonical(physical)?;
        let (family, count, variants) = family_and_operand_count(kind)?;
        if alternative.family != family || !variants.contains(&alternative.variant) {
            return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
        }
        if operands.len() != count {
            return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
        }
        Ok(())
    }

    fn family_and_operand_count(
        kind: Kind,
    ) -> Result<(Family, usize, std::ops::RangeInclusive<u32>), X86_64SelectedFormEncodingError>
    {
        Ok(match kind {
            Kind::InstalledInterruptCpuCheck { .. } => {
                (Family::InstalledInterruptCpuCheck, 4, 0..=0)
            }
            Kind::InstalledInterruptTaskCommit { .. } => {
                (Family::InstalledInterruptTaskCommit, 3, 0..=0)
            }
            Kind::InstalledInterruptTablePublication { .. } => {
                (Family::InstalledInterruptTablePublication, 2, 0..=0)
            }

            Kind::RegisterAccess { operation } => (
                super::register_access::family(operation)?,
                operation.input_carriers().len() + usize::from(operation.produces_value()),
                0..=0,
            ),
            Kind::AccessorScalarAccess { form, .. } => {
                (Family::AccessorScalarAccess(form), 2, 0..=0)
            }
            Kind::FloatResultPolicy { form } => (
                Family::FloatResultPolicy(form),
                form.operand_count() + 1,
                0..=0,
            ),
            Kind::AtomicAccess { form } => (
                Family::AtomicAccess(form),
                usize::from(form.operand_count()),
                0..=0,
            ),
            Kind::NearestIeeeFloatArithmetic { form } => (
                Family::NearestIeeeFloatArithmetic(form),
                form.operand_count() + 1,
                0..=0,
            ),
            Kind::PortWrite { .. } => (Family::PortWrite, 0, 0..=0),
            Kind::AtomicFence => (Family::AtomicFence, 0, 0..=0),
            Kind::Crash => (Family::Crash, 0, 0..=0),
            Kind::PrivilegedCrash => (Family::PrivilegedCrash, 0, 0..=0),
            Kind::CompareI64Zero => (Family::CompareI64Zero, 1, 0..=0),
            Kind::CompareI64 => (Family::CompareI64, 2, 0..=0),
            Kind::CompareI64Immediate { .. } => (Family::CompareI64Immediate, 1, 0..=0),
            Kind::MaterializeI64 { .. } => (Family::MaterializeI64, 1, 0..=1),
            Kind::ImageAddress { .. } => (Family::ImageAddress, 1, 0..=0),
            Kind::CopyI64 => (Family::CopyI64, 2, 0..=0),
            Kind::MaterializeBooleanEqual => (Family::MaterializeBooleanEqual, 1, 0..=0),
            Kind::MaterializeBooleanU64LessThan => {
                (Family::MaterializeBooleanU64LessThan, 1, 0..=0)
            }
            Kind::MaterializeBooleanI64LessThan => {
                (Family::MaterializeBooleanI64LessThan, 1, 0..=0)
            }
            Kind::MaterializeBooleanU64LessOrEqual => {
                (Family::MaterializeBooleanU64LessOrEqual, 1, 0..=0)
            }
            Kind::MaterializeBooleanI64LessOrEqual => {
                (Family::MaterializeBooleanI64LessOrEqual, 1, 0..=0)
            }
            Kind::ZeroExtendU8 => (Family::ZeroExtendU8, 2, 0..=0),
            Kind::ZeroExtendU16 => (Family::ZeroExtendU16, 2, 0..=0),
            Kind::SignExtendI8 => (Family::SignExtendI8, 2, 0..=0),
            Kind::SignExtendI16 => (Family::SignExtendI16, 2, 0..=0),
            Kind::SignExtendI32 => (Family::SignExtendI32, 2, 0..=0),
            Kind::ZeroExtendU32 => (Family::ZeroExtendU32, 2, 0..=0),
            Kind::ByteViewAddress => (Family::ByteViewAddress, 3, 0..=0),
            Kind::ExactAddI64 { .. } => (Family::ExactAddI64, 3, 0..=0),
            Kind::WrappingAddI64 => (Family::WrappingAddI64, 3, 0..=0),
            Kind::BitwiseAndI64 => (Family::BitwiseAndI64, 3, 0..=0),
            Kind::BitwiseOrI64 => (Family::BitwiseOrI64, 3, 0..=0),
            Kind::BitwiseNotI64 => (Family::BitwiseNotI64, 2, 0..=0),
            Kind::ExactDivideU64 { .. } => (Family::ExactDivideU64, 4, 0..=0),
            Kind::ExactRemainderU64 { .. } => (Family::ExactRemainderU64, 4, 0..=0),
            Kind::WrappingRemainderI64 { .. } => (Family::WrappingRemainderI64, 4, 0..=0),
            Kind::WrappingDivideI64 { .. } => (Family::WrappingDivideI64, 4, 0..=0),
            Kind::ExactDivideI64 { .. } => (Family::ExactDivideI64, 4, 0..=0),
            Kind::ExactRemainderI64 { .. } => (Family::ExactRemainderI64, 4, 0..=0),
            Kind::SaturatingAdd { carrier } => (
                Family::SaturatingAdd(carrier),
                SaturatingForm::of(SaturatingOperation::Add, carrier).operand_count(),
                0..=0,
            ),
            Kind::SaturatingSubtract { carrier } => (
                Family::SaturatingSubtract(carrier),
                SaturatingForm::of(SaturatingOperation::Subtract, carrier).operand_count(),
                0..=0,
            ),
            Kind::SaturatingDivide { carrier, .. } => (
                Family::SaturatingDivide(carrier),
                SaturatingForm::of(SaturatingOperation::Divide, carrier).operand_count(),
                0..=0,
            ),
            Kind::SaturatingRemainder { carrier, .. } => (
                Family::SaturatingRemainder(carrier),
                SaturatingForm::of(SaturatingOperation::Remainder, carrier).operand_count(),
                0..=0,
            ),
            Kind::SaturatingMultiply { carrier } => (
                Family::SaturatingMultiply(carrier),
                SaturatingForm::of(SaturatingOperation::Multiply, carrier).operand_count(),
                0..=0,
            ),
            Kind::TrappingInteger { form } => (
                Family::TrappingInteger(form),
                TrappingShape::of(form).operand_count(),
                0..=0,
            ),
            Kind::BitwiseXorI64 => (Family::BitwiseXorI64, 3, 0..=0),
            Kind::ExactSubtractI64 { .. } => (Family::ExactSubtractI64, 3, 0..=3),
            Kind::WrappingSubtractI64 => (Family::WrappingSubtractI64, 3, 0..=3),
            Kind::ExactMultiplyI64 { .. } => (Family::ExactMultiplyI64, 3, 0..=3),
            Kind::WrappingMultiplyI64 => (Family::WrappingMultiplyI64, 3, 0..=3),
            Kind::WrappingShiftLeftI64 => (Family::WrappingShiftLeftI64, 3, 0..=0),
            Kind::WrappingShiftRightI64 => (Family::WrappingShiftRightI64, 3, 0..=0),
            Kind::WrappingShiftRightU64 => (Family::WrappingShiftRightU64, 3, 0..=0),
            Kind::ExactShiftLeftI64 { .. } => (Family::ExactShiftLeftI64, 3, 0..=0),
            Kind::ExactShiftRightI64 { .. } => (Family::ExactShiftRightI64, 3, 0..=0),
            Kind::ExactShiftRightU64 { .. } => (Family::ExactShiftRightU64, 3, 0..=0),
            Kind::ExactAddI64Immediate { .. } => (Family::ExactAddI64Immediate, 2, 0..=0),
            Kind::ExactSubtractI64Immediate { .. } => (Family::ExactSubtractI64Immediate, 2, 0..=0),
            Kind::ReturnScalar => (Family::ReturnScalar, 1, 0..=0),
            Kind::ReturnAggregate { fragment_count } => {
                if !(1..=2).contains(&fragment_count) {
                    return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
                }
                (Family::ReturnAggregate, usize::from(fragment_count), 0..=0)
            }
            Kind::ReturnUnit => (Family::ReturnUnit, 0, 0..=0),
            selected_instructions::layout_dependent_selected_transfers!()
            | selected_instructions::dedicated_selected_forms!() => {
                return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
            }
        })
    }

    pub(crate) fn validate_return_home(
        kind: Kind,
        registers: &[u8],
    ) -> Result<(), X86_64SelectedFormEncodingError> {
        if matches!(kind, Kind::ReturnScalar) && registers != [0] {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        if let Kind::ReturnAggregate { fragment_count } = kind
            && (!(1..=2).contains(&fragment_count)
                || registers != &[0, 2][..usize::from(fragment_count)])
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        Ok(())
    }

    pub(crate) fn resolve_scalar_registers(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
        if kind == Kind::ReturnScalar
            && physical
                .model()
                .view_named("xmm0")
                .is_some_and(|view| operands == [view.id])
        {
            return Ok(vec![0]);
        }
        resolve_registers(physical, operands)
    }

    pub(crate) fn resolve_registers(
        physical: &ValidatedPhysicalRegisterModel,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
        const NAMES: [&str; 16] = [
            "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15",
        ];
        operands
            .iter()
            .map(|id| {
                let (code, view) = NAMES
                    .iter()
                    .enumerate()
                    .find_map(|(code, name)| {
                        physical
                            .model()
                            .view_named(name)
                            .filter(|view| view.id == *id)
                            .map(|view| (code as u8, view))
                    })
                    .ok_or(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
                if code == 4 || view.bits != 64 || !view.allocatable {
                    return Err(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
                }
                Ok(code)
            })
            .collect()
    }

    pub(crate) fn validate_alias_partition(
        kind: Kind,
        alternative: MachineAlternativeKey,
        registers: &[u8],
    ) -> Result<(), X86_64SelectedFormEncodingError> {
        if !matches!(
            kind,
            Kind::ExactSubtractI64 { .. }
                | Kind::WrappingSubtractI64
                | Kind::ExactMultiplyI64 { .. }
                | Kind::WrappingMultiplyI64
        ) {
            return Ok(());
        }
        let [left, right, result] = registers else {
            return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
        };
        let expected = match (result == left, result == right) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        };
        if alternative.variant != expected {
            return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
        }
        Ok(())
    }
}
pub(crate) mod saturating_forms {
    use selected_instructions::{SaturatingCarrier, SaturatingOperation, SelectedInstructionKind};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum SaturatingForm {
        AddU64,
        SubtractUnsigned,
        DivideUnsigned,
        ClampSignedNarrow,
        ClampUnsignedNarrow,
        OverflowSelectI64,
        DivideSignedNarrow,
        DivideI64,
        RemainderUnsigned,
        RemainderSigned,
        MultiplySignedNarrow,
        MultiplyUnsignedNarrow,
        MultiplyI64,
        MultiplyU64,
    }

    impl SaturatingForm {
        pub(crate) const fn of(operation: SaturatingOperation, carrier: SaturatingCarrier) -> Self {
            match (operation, carrier.is_signed(), carrier.is_narrow()) {
                (SaturatingOperation::Add, false, false) => Self::AddU64,
                (SaturatingOperation::Add, false, true) => Self::ClampUnsignedNarrow,
                (SaturatingOperation::Add | SaturatingOperation::Subtract, true, true) => {
                    Self::ClampSignedNarrow
                }
                (SaturatingOperation::Add | SaturatingOperation::Subtract, true, false) => {
                    Self::OverflowSelectI64
                }
                (SaturatingOperation::Subtract, false, _) => Self::SubtractUnsigned,
                (SaturatingOperation::Divide, false, _) => Self::DivideUnsigned,
                (SaturatingOperation::Divide, true, true) => Self::DivideSignedNarrow,
                (SaturatingOperation::Divide, true, false) => Self::DivideI64,
                (SaturatingOperation::Remainder, false, _) => Self::RemainderUnsigned,
                (SaturatingOperation::Remainder, true, _) => Self::RemainderSigned,
                (SaturatingOperation::Multiply, true, true) => Self::MultiplySignedNarrow,
                (SaturatingOperation::Multiply, false, true) => Self::MultiplyUnsignedNarrow,
                (SaturatingOperation::Multiply, true, false) => Self::MultiplyI64,
                (SaturatingOperation::Multiply, false, false) => Self::MultiplyU64,
            }
        }

        pub(crate) const fn byte_count(self) -> u16 {
            match self {
                Self::AddU64 => 19,
                Self::SubtractUnsigned => 13,
                Self::DivideUnsigned => 3,
                Self::ClampSignedNarrow => 40,
                Self::ClampUnsignedNarrow => 23,
                Self::OverflowSelectI64 => 25,
                Self::DivideSignedNarrow => 22,
                Self::DivideI64 => 26,
                Self::RemainderUnsigned => 9,
                Self::RemainderSigned => 17,
                Self::MultiplySignedNarrow => 41,
                Self::MultiplyUnsignedNarrow => 24,
                Self::MultiplyI64 => 29,
                Self::MultiplyU64 => 9,
            }
        }

        pub(crate) const fn operand_count(self) -> usize {
            match self {
                Self::AddU64 | Self::SubtractUnsigned => 3,
                Self::DivideUnsigned
                | Self::ClampSignedNarrow
                | Self::ClampUnsignedNarrow
                | Self::OverflowSelectI64
                | Self::DivideSignedNarrow
                | Self::DivideI64
                | Self::RemainderUnsigned
                | Self::RemainderSigned
                | Self::MultiplySignedNarrow
                | Self::MultiplyUnsignedNarrow
                | Self::MultiplyI64
                | Self::MultiplyU64 => 4,
            }
        }

        pub(crate) const fn is_division(self) -> bool {
            matches!(
                self,
                Self::DivideUnsigned
                    | Self::DivideSignedNarrow
                    | Self::DivideI64
                    | Self::RemainderUnsigned
                    | Self::RemainderSigned
            )
        }

        pub(crate) const fn is_fixed_rax_rdx(self) -> bool {
            self.is_division() || matches!(self, Self::MultiplyU64)
        }

        pub(crate) fn accepts_registers(self, registers: &[u8]) -> bool {
            if registers.len() != self.operand_count() {
                return false;
            }
            let (inputs, outputs) = registers.split_at(2);
            if self.is_fixed_rax_rdx() {
                return inputs[0] == 0 && outputs[0] == 0 && outputs[1] == 2 && inputs[1] != 2;
            }
            outputs.iter().all(|output| !inputs.contains(output))
                && (outputs.len() == 1 || outputs[0] != outputs[1])
        }

        pub(crate) fn operand_reads_and_writes(self) -> (Vec<u16>, Vec<u16>) {
            match self {
                Self::AddU64 | Self::SubtractUnsigned => (vec![0, 1], vec![2]),
                Self::DivideUnsigned | Self::DivideSignedNarrow | Self::DivideI64 => {
                    (vec![0, 1, 3], vec![2])
                }
                Self::RemainderUnsigned | Self::RemainderSigned => (vec![0, 1], vec![2, 3]),
                Self::ClampSignedNarrow
                | Self::ClampUnsignedNarrow
                | Self::OverflowSelectI64
                | Self::MultiplySignedNarrow
                | Self::MultiplyUnsignedNarrow
                | Self::MultiplyI64
                | Self::MultiplyU64 => (vec![0, 1], vec![2, 3]),
            }
        }
    }

    pub(crate) const fn saturating_operation(
        kind: SelectedInstructionKind,
    ) -> Option<(SaturatingOperation, SaturatingCarrier)> {
        match kind {
            SelectedInstructionKind::SaturatingAdd { carrier } => {
                Some((SaturatingOperation::Add, carrier))
            }
            SelectedInstructionKind::SaturatingSubtract { carrier } => {
                Some((SaturatingOperation::Subtract, carrier))
            }
            SelectedInstructionKind::SaturatingDivide { carrier, .. } => {
                Some((SaturatingOperation::Divide, carrier))
            }
            SelectedInstructionKind::SaturatingRemainder { carrier, .. } => {
                Some((SaturatingOperation::Remainder, carrier))
            }
            SelectedInstructionKind::SaturatingMultiply { carrier } => {
                Some((SaturatingOperation::Multiply, carrier))
            }
            _ => None,
        }
    }
}
mod scalar_call {
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
        SelectedInstructionKind,
    };
    use semantic_vocabulary::MachineId;
    use target::NativeTarget;

    use crate::machine_effects::{X86_64SelectedAbi, x86_64_selected_abi};
    use crate::{x86_64_register_constraint_catalog_for, x86_64_system_v_register_call_keys};

    pub(crate) const X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT: usize = 5;
    pub(crate) const X86_64_SCALAR_CALL_OPCODE_OFFSET: u16 = 0;
    pub(crate) const X86_64_SCALAR_CALL_PATCH_OFFSET: u16 = 1;
    pub(crate) const X86_64_SCALAR_CALL_REFERENCE_OFFSET: u16 = 5;
    pub(crate) const X86_64_SCALAR_CALL_PATCH_WIDTH: u8 = 4;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum X86_64ScalarCallFixupKind {
        Relative32FromNextInstructionToInternalMachineV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum X86_64ScalarCallFixupState {
        UnresolvedZeroFieldV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct X86_64ScalarCallFixup {
        pub kind: X86_64ScalarCallFixupKind,
        pub state: X86_64ScalarCallFixupState,
        pub callee: MachineId,
        pub opcode_byte_offset: u16,
        pub patch_byte_offset: u16,
        pub reference_byte_offset: u16,
        pub patch_byte_width: u8,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedX86_64SelectedScalarCallTemplate {
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: Vec<RegisterViewId>,
        effects: MachineEncodedEffects,
        bytes: [u8; X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT],
        fixup: X86_64ScalarCallFixup,
    }

    impl ValidatedX86_64SelectedScalarCallTemplate {
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

        pub const fn bytes(&self) -> &[u8; X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT] {
            &self.bytes
        }

        pub const fn fixup(&self) -> X86_64ScalarCallFixup {
            self.fixup
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum X86_64ScalarCallTemplateError {
        UnsupportedTarget,
        NonCanonicalPhysicalModel,
        InstructionKindMismatch,
        AlternativeMismatch,
        OperandViewMismatch,
        EffectMismatch,
        MalformedTemplate,
        FixupMismatch,
    }

    optimization_core::invalid_error_display!(
        X86_64ScalarCallTemplateError,
        "x86-64 scalar-call template"
    );

    pub fn encode_x86_64_selected_scalar_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
    ) -> Result<ValidatedX86_64SelectedScalarCallTemplate, X86_64ScalarCallTemplateError> {
        let callee = kind
            .machine_callee()
            .ok_or(X86_64ScalarCallTemplateError::InstructionKindMismatch)?;
        let bytes = crate::ORDINARY_DIRECT_CALL_BYTES;
        let fixup = canonical_fixup(callee);
        validate_x86_64_selected_scalar_call_template(
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

    pub fn validate_x86_64_selected_scalar_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
        bytes: &[u8],
        fixup: X86_64ScalarCallFixup,
    ) -> Result<ValidatedX86_64SelectedScalarCallTemplate, X86_64ScalarCallTemplateError> {
        let abi = x86_64_selected_abi(target)
            .map_err(|_| X86_64ScalarCallTemplateError::UnsupportedTarget)?;
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
            return Err(X86_64ScalarCallTemplateError::NonCanonicalPhysicalModel);
        }
        let callee = kind
            .machine_callee()
            .ok_or(X86_64ScalarCallTemplateError::InstructionKindMismatch)?;
        if alternative != kind.call_alternative_key() {
            return Err(X86_64ScalarCallTemplateError::AlternativeMismatch);
        }
        let (expected_operand_views, expected) =
            if matches!(kind, SelectedInstructionKind::InvokeNoNormal { .. }) {
                expected_no_normal_mechanics(abi, physical, operand_views, effects)?
            } else {
                expected_call_mechanics(abi, physical, kind, operand_views)?
            };
        if effects != &expected {
            return Err(X86_64ScalarCallTemplateError::EffectMismatch);
        }
        let bytes: [u8; X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT] = bytes
            .try_into()
            .map_err(|_| X86_64ScalarCallTemplateError::MalformedTemplate)?;
        if bytes != [0xe8, 0, 0, 0, 0] {
            return Err(X86_64ScalarCallTemplateError::MalformedTemplate);
        }
        if fixup != canonical_fixup(callee) {
            return Err(X86_64ScalarCallTemplateError::FixupMismatch);
        }
        Ok(ValidatedX86_64SelectedScalarCallTemplate {
            kind,
            alternative,
            operand_views: expected_operand_views,
            effects: effects.clone(),
            bytes,
            fixup,
        })
    }

    fn canonical_fixup(callee: MachineId) -> X86_64ScalarCallFixup {
        X86_64ScalarCallFixup {
            kind: X86_64ScalarCallFixupKind::Relative32FromNextInstructionToInternalMachineV1,
            state: X86_64ScalarCallFixupState::UnresolvedZeroFieldV1,
            callee,
            opcode_byte_offset: X86_64_SCALAR_CALL_OPCODE_OFFSET,
            patch_byte_offset: X86_64_SCALAR_CALL_PATCH_OFFSET,
            reference_byte_offset: X86_64_SCALAR_CALL_REFERENCE_OFFSET,
            patch_byte_width: X86_64_SCALAR_CALL_PATCH_WIDTH,
        }
    }

    fn expected_operand_views(
        abi: X86_64SelectedAbi,
        physical: &ValidatedPhysicalRegisterModel,
        arity: usize,
    ) -> Vec<RegisterViewId> {
        let arguments: &[&str] = match abi {
            X86_64SelectedAbi::SystemV => &["rdi", "rsi", "rdx", "rcx", "r8", "r9"],
            X86_64SelectedAbi::Microsoft => &["rcx", "rdx", "r8", "r9"],
        };
        arguments
            .iter()
            .copied()
            .take(arity)
            .chain(["rax"])
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
        abi: X86_64SelectedAbi,
        physical: &ValidatedPhysicalRegisterModel,
        arity: usize,
    ) -> MachineEncodedEffects {
        let catalog = x86_64_register_constraint_catalog_for(physical);
        let row = catalog
            .constraints
            .iter()
            .find(|row| {
                row.key
                    == match abi {
                        X86_64SelectedAbi::SystemV => x86_64_system_v_register_call_keys()[arity],
                        X86_64SelectedAbi::Microsoft => {
                            crate::x86_64_microsoft_register_call_keys()[arity]
                        }
                    }
            })
            .expect("canonical x86-64 catalog contains scalar-call constraint");
        let stack_pointer = physical
            .model()
            .view_named("rsp")
            .expect("canonical x86-64 model contains rsp")
            .id;
        let mut effects = crate::machine_effects::user_direct_call_effects(row, stack_pointer);
        effects.external_operand_reads = (0..arity as u16).collect();
        effects.external_operand_writes = vec![arity as u16];
        effects
    }

    #[cfg(test)]
    mod mixed_calls {
        use super::{
            MachineId, NativeTarget, ValidatedX86_64SelectedScalarCallTemplate, canonical_fixup,
            encode_x86_64_selected_scalar_call_template, expected_effects, x86_64_selected_abi,
        };
        use crate::{x86_64_physical_register_model, x86_64_register_constraint_catalog};
        use register_model::{RegisterViewId, validate_physical_register_model};
        use selected_instructions::fixture;

        impl fixture::CallTemplate for ValidatedX86_64SelectedScalarCallTemplate {
            fn bound_operands(&self) -> &[RegisterViewId] {
                self.operand_views()
            }

            fn names_callee(&self, callee: MachineId) -> bool {
                self.fixup() == canonical_fixup(callee)
            }
        }

        #[test]
        fn microsoft_mixed_calls_preserve_positional_register_holes() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let catalog = x86_64_register_constraint_catalog(&physical);
            for (variant, names) in [
                (901, vec!["rdx", "xmm0"]),
                (902, vec!["rcx", "xmm1"]),
                (903, vec!["xmm0", "xmm1"]),
                (911, vec!["rdx", "r8", "r9", "xmm0"]),
                (925, vec!["xmm0", "xmm1", "xmm2", "xmm3"]),
            ] {
                let row = catalog
                    .constraints
                    .iter()
                    .find(|row| {
                        row.key.family == register_model::RegisterConstraintFamily::Call
                            && row.key.variant == variant
                    })
                    .unwrap();
                assert_eq!(
                    row.operands
                        .iter()
                        .map(|operand| operand.fixed_view.unwrap())
                        .collect::<Vec<_>>(),
                    names
                        .iter()
                        .map(|name| physical.model().view_named(name).unwrap().id)
                        .collect::<Vec<_>>()
                );
            }
        }

        #[test]
        fn every_mixed_unit_call_row_has_exact_template_operands_and_effects() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let catalog = x86_64_register_constraint_catalog(&physical);
            let callee = MachineId::new(7).unwrap();
            let stack_pointer = physical.model().view_named("rsp").unwrap().id;
            for (target, keys) in [
                (
                    NativeTarget::linux_x64(),
                    crate::x86_64_system_v_mixed_unit_call_keys(),
                ),
                (
                    NativeTarget::windows_x64(),
                    crate::x86_64_microsoft_mixed_unit_call_keys(),
                ),
            ] {
                let base = expected_effects(x86_64_selected_abi(target).unwrap(), &physical, 0);
                for key in keys {
                    fixture::assert_mixed_unit_call_row(
                        catalog.constraint(key).unwrap(),
                        base.clone(),
                        callee,
                        stack_pointer,
                        |kind, alternative, operands, effects| {
                            encode_x86_64_selected_scalar_call_template(
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
            MachineAlternativeFamily, MachineAlternativeKey, MachineId, NativeTarget,
            SelectedInstructionKind, X86_64ScalarCallTemplateError,
            encode_x86_64_selected_scalar_call_template, expected_effects,
            validate_x86_64_selected_scalar_call_template, x86_64_selected_abi,
        };
        use crate::{x86_64_physical_register_model, x86_64_register_constraint_catalog};
        use register_model::validate_physical_register_model;

        #[test]
        fn target_register_arities_encode_and_reject_opposite_abi_effects() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let constraints = crate::validate_x86_64_register_constraint_catalog(
                x86_64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap();
            let required = crate::x86_64_required_register_constraints();
            for pair in required.windows(2) {
                assert!(
                    pair[0] < pair[1],
                    "constraint inventory out of order: {:?} >= {:?}",
                    pair[0],
                    pair[1]
                );
            }
            let cases: [(NativeTarget, &[&str]); 2] = [
                (
                    NativeTarget::linux_x64(),
                    &["rdi", "rsi", "rdx", "rcx", "r8", "r9"],
                ),
                (NativeTarget::windows_x64(), &["rcx", "rdx", "r8", "r9"]),
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
                let catalog = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
                for arity in 0..=arguments.len() {
                    let operands = arguments[..arity]
                        .iter()
                        .copied()
                        .chain(["rax"])
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
                    let template = encode_x86_64_selected_scalar_call_template(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        effects,
                    )
                    .unwrap();
                    assert_eq!(
                        validate_x86_64_selected_scalar_call_template(
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
                    let opposite = if target == NativeTarget::linux_x64() {
                        NativeTarget::windows_x64()
                    } else {
                        NativeTarget::linux_x64()
                    };
                    let mut changed = effects.clone();
                    changed.implicit_unit_clobbers =
                        expected_effects(x86_64_selected_abi(opposite).unwrap(), &physical, 0)
                            .implicit_unit_clobbers;
                    assert_eq!(
                        validate_x86_64_selected_scalar_call_template(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &changed,
                            template.bytes(),
                            template.fixup(),
                        ),
                        Err(X86_64ScalarCallTemplateError::EffectMismatch)
                    );
                    let mut changed = operands.clone();
                    *changed.last_mut().unwrap() = physical.model().view_named("rsp").unwrap().id;
                    assert_eq!(
                        validate_x86_64_selected_scalar_call_template(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &changed,
                            effects,
                            template.bytes(),
                            template.fixup(),
                        ),
                        Err(X86_64ScalarCallTemplateError::OperandViewMismatch)
                    );
                }
            }
        }
    }
    #[cfg(test)]
    mod unit_calls {
        use super::{
            MachineAlternativeFamily, MachineAlternativeKey, MachineId, NativeTarget,
            SelectedInstructionKind, encode_x86_64_selected_scalar_call_template, expected_effects,
            expected_operand_views, validate_x86_64_selected_scalar_call_template,
            x86_64_selected_abi,
        };
        use crate::{x86_64_physical_register_model, x86_64_register_constraint_catalog};
        use register_model::{RegisterOperandAccess, validate_physical_register_model};

        #[test]
        fn unit_call_templates_cover_native_register_arities_and_reject_forged_results() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let catalog = x86_64_register_constraint_catalog(&physical);
            for (target, keys) in [
                (
                    NativeTarget::linux_x64(),
                    crate::x86_64_system_v_register_unit_call_keys(),
                ),
                (
                    NativeTarget::windows_x64(),
                    crate::x86_64_microsoft_register_unit_call_keys(),
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
                        crate::validate_x86_64_register_constraint_catalog(
                            missing_clobber,
                            &physical
                        )
                        .is_err()
                    );
                    let abi = x86_64_selected_abi(target).unwrap();
                    let mut operands = expected_operand_views(abi, &physical, arity);
                    operands.pop();
                    let mut effects = expected_effects(abi, &physical, arity);
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
                    let template = encode_x86_64_selected_scalar_call_template(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &effects,
                    )
                    .unwrap();
                    assert_eq!(template.operand_views(), operands);
                    validate_x86_64_selected_scalar_call_template(
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
                        encode_x86_64_selected_scalar_call_template(
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
                        encode_x86_64_selected_scalar_call_template(
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
                        wrong[0] = physical.model().view_named("rsp").unwrap().id;
                        assert!(
                            encode_x86_64_selected_scalar_call_template(
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
            MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, MachineId,
            NativeTarget, RegisterViewId, SelectedInstructionKind, ValidatedPhysicalRegisterModel,
            X86_64ScalarCallFixup, X86_64ScalarCallTemplateError, X86_64SelectedAbi,
            canonical_fixup, encode_x86_64_selected_scalar_call_template, expected_effects,
            expected_operand_views, validate_x86_64_selected_scalar_call_template,
            x86_64_selected_abi, x86_64_system_v_register_call_keys,
        };
        use crate::x86_64_physical_register_model;
        mod mixed_aggregates {
            use super::{
                MachineId, NativeTarget, SelectedInstructionKind, canonical_fixup,
                encode_x86_64_selected_scalar_call_template, inputs,
                validate_x86_64_selected_scalar_call_template,
            };
            #[test]
            fn mixed_aggregate_templates_replay_inputs_results_and_clobbers() {
                let (physical, _, _, _, _) = inputs();
                let constraints = crate::validate_x86_64_register_constraint_catalog(
                    crate::x86_64_register_constraint_catalog(&physical),
                    &physical,
                )
                .unwrap();
                let kind = SelectedInstructionKind::CallAggregate {
                    target: selected_instructions::SelectedCallTarget::Machine(
                        MachineId::new(7).unwrap(),
                    ),
                };
                for (native, keys) in [
                    (
                        NativeTarget::linux_x64(),
                        crate::x86_64_system_v_mixed_aggregate_call_keys(),
                    ),
                    (
                        NativeTarget::windows_x64(),
                        crate::x86_64_microsoft_mixed_aggregate_call_keys(),
                    ),
                ] {
                    let catalog =
                        crate::x86_64_machine_effect_catalog(native, &constraints).unwrap();
                    for key in keys {
                        let declaration = catalog
                            .declarations
                            .iter()
                            .find(|declaration| {
                                declaration.constraint == key
                                    && declaration.semantic
                                        == selected_instructions::MachineSemanticKind::CallAggregate
                            })
                            .unwrap();
                        let alternative = &declaration.alternatives[0];
                        let row = constraints.catalog().constraint(key).unwrap();
                        let operands = row
                            .operands
                            .iter()
                            .map(|operand| operand.fixed_view.unwrap())
                            .collect::<Vec<_>>();
                        let template = encode_x86_64_selected_scalar_call_template(
                            native,
                            &physical,
                            kind,
                            alternative.key,
                            &operands,
                            &alternative.encoded,
                        )
                        .unwrap();
                        assert_eq!(template.bytes(), &[0xe8, 0, 0, 0, 0]);
                        assert_eq!(
                            template.fixup(),
                            canonical_fixup(MachineId::new(7).unwrap())
                        );
                        validate_x86_64_selected_scalar_call_template(
                            native,
                            &physical,
                            kind,
                            alternative.key,
                            &operands,
                            &alternative.encoded,
                            template.bytes(),
                            template.fixup(),
                        )
                        .unwrap();
                        let mut changed = alternative.encoded.clone();
                        changed
                            .implicit_unit_clobbers
                            .extend(&physical.model().view_named("rax").unwrap().write_units);
                        changed.implicit_unit_clobbers.sort_unstable();
                        assert!(
                            encode_x86_64_selected_scalar_call_template(
                                native,
                                &physical,
                                kind,
                                alternative.key,
                                &operands,
                                &changed,
                            )
                            .is_err()
                        );
                        let mut changed = operands.clone();
                        *changed.last_mut().unwrap() =
                            physical.model().view_named("r11").unwrap().id;
                        assert!(
                            encode_x86_64_selected_scalar_call_template(
                                native,
                                &physical,
                                kind,
                                alternative.key,
                                &changed,
                                &alternative.encoded,
                            )
                            .is_err()
                        );
                        let mut changed = operands.clone();
                        changed[0] = physical.model().view_named("xmm15").unwrap().id;
                        assert!(
                            encode_x86_64_selected_scalar_call_template(
                                native,
                                &physical,
                                kind,
                                alternative.key,
                                &changed,
                                &alternative.encoded,
                            )
                            .is_err()
                        );
                    }
                }
            }
        }
        use register_model::validate_physical_register_model;

        fn inputs() -> (
            ValidatedPhysicalRegisterModel,
            SelectedInstructionKind,
            MachineAlternativeKey,
            Vec<RegisterViewId>,
            MachineEncodedEffects,
        ) {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let kind = SelectedInstructionKind::CallScalar {
                target: selected_instructions::SelectedCallTarget::Machine(
                    MachineId::new(7).unwrap(),
                ),
            };
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::CallScalar,
                variant: 0,
            };
            let operands = expected_operand_views(X86_64SelectedAbi::SystemV, &physical, 2);
            let effects = expected_effects(X86_64SelectedAbi::SystemV, &physical, 2);
            (physical, kind, alternative, operands, effects)
        }

        #[test]
        fn every_register_arity_has_exact_operands_and_effects() {
            let (physical, kind, alternative, _, _) = inputs();
            let catalog = crate::x86_64_register_constraint_catalog(&physical);
            for (arity, key) in x86_64_system_v_register_call_keys().into_iter().enumerate() {
                let row = catalog.constraint(key).unwrap();
                let operands = expected_operand_views(X86_64SelectedAbi::SystemV, &physical, arity);
                assert_eq!(row.operands.len(), arity + 1);
                assert_eq!(
                    row.operands
                        .iter()
                        .map(|operand| operand.fixed_view.unwrap())
                        .collect::<Vec<_>>(),
                    operands
                );
                let effects = expected_effects(X86_64SelectedAbi::SystemV, &physical, arity);
                assert_eq!(
                    effects.external_operand_reads,
                    (0..arity as u16).collect::<Vec<_>>()
                );
                assert_eq!(effects.external_operand_writes, vec![arity as u16]);
                let template = encode_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
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
                    encode_x86_64_selected_scalar_call_template(
                        NativeTarget::linux_x64(),
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &wrong_effects,
                    )
                    .is_err()
                );
                let mut wrong_views = operands.clone();
                wrong_views[arity] = physical.model().view_named("rsp").unwrap().id;
                assert!(
                    encode_x86_64_selected_scalar_call_template(
                        NativeTarget::linux_x64(),
                        &physical,
                        kind,
                        alternative,
                        &wrong_views,
                        &effects,
                    )
                    .is_err()
                );
            }
            let oversized = vec![physical.model().view_named("rax").unwrap().id; 8];
            assert_eq!(
                encode_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &oversized,
                    &expected_effects(X86_64SelectedAbi::SystemV, &physical, 0),
                ),
                Err(X86_64ScalarCallTemplateError::OperandViewMismatch)
            );
        }

        #[test]
        fn scalar_call_abi_matrix_resolves_declared_pairs_and_fails_closed() {
            for declared_system_v in [NativeTarget::linux_x64(), NativeTarget::macos_x64()] {
                assert_eq!(
                    x86_64_selected_abi(declared_system_v),
                    Ok(X86_64SelectedAbi::SystemV)
                );
            }
            for declared_coff in [NativeTarget::windows_x64(), NativeTarget::uefi_x64()] {
                assert_eq!(
                    x86_64_selected_abi(declared_coff),
                    Ok(X86_64SelectedAbi::Microsoft)
                );
            }
            for undeclared in [
                NativeTarget {
                    object_format: target::ObjectFormat::Coff,
                    ..NativeTarget::linux_arm64()
                },
                NativeTarget::linux_arm64(),
                NativeTarget::macos_arm64(),
            ] {
                assert_eq!(
                x86_64_selected_abi(undeclared),
                Err(
                    crate::machine_effects::X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi,
                )
            );
            }
        }

        #[test]
        fn scalar_call_template_has_exact_bytes_and_fixup() {
            let (physical, kind, alternative, operands, effects) = inputs();
            let template = encode_x86_64_selected_scalar_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
            )
            .unwrap();
            assert_eq!(template.bytes(), &[0xe8, 0, 0, 0, 0]);
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
        fn microsoft_direct_aggregate_templates_replay_registers_and_clobbers() {
            let (physical, _, _, _, _) = inputs();
            let constraints = crate::validate_x86_64_register_constraint_catalog(
                crate::x86_64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap();
            let native = NativeTarget::windows_x64();
            let catalog = crate::x86_64_machine_effect_catalog(native, &constraints).unwrap();
            let kind = SelectedInstructionKind::CallAggregate {
                target: selected_instructions::SelectedCallTarget::Machine(
                    MachineId::new(7).unwrap(),
                ),
            };
            for (arity, key) in crate::register_model::x86_64_microsoft_aggregate_call_keys()
                .into_iter()
                .enumerate()
            {
                let declaration = catalog
                    .declarations
                    .iter()
                    .find(|declaration| {
                        declaration.constraint == key
                            && declaration.semantic
                                == selected_instructions::MachineSemanticKind::CallAggregate
                    })
                    .unwrap();
                let alternative = &declaration.alternatives[0];
                let operands =
                    expected_operand_views(x86_64_selected_abi(native).unwrap(), &physical, arity);
                assert_eq!(
                    alternative.encoded.external_operand_reads,
                    (0..arity as u16).collect::<Vec<_>>()
                );
                assert_eq!(
                    alternative.encoded.external_operand_writes,
                    vec![arity as u16]
                );
                let template = encode_x86_64_selected_scalar_call_template(
                    native,
                    &physical,
                    kind,
                    alternative.key,
                    &operands,
                    &alternative.encoded,
                )
                .unwrap();
                assert_eq!(template.bytes(), &[0xe8, 0, 0, 0, 0]);
                let mut changed = alternative.encoded.clone();
                changed.implicit_unit_clobbers.clear();
                assert!(
                    encode_x86_64_selected_scalar_call_template(
                        native,
                        &physical,
                        kind,
                        alternative.key,
                        &operands,
                        &changed,
                    )
                    .is_err()
                );
                let mut wrong_registers = operands.clone();
                wrong_registers[arity] = physical.model().view_named("rdx").unwrap().id;
                assert!(
                    encode_x86_64_selected_scalar_call_template(
                        native,
                        &physical,
                        kind,
                        alternative.key,
                        &wrong_registers,
                        &alternative.encoded,
                    )
                    .is_err()
                );
            }
        }

        #[test]
        fn scalar_call_validation_rejects_malformed_selected_inputs() {
            let (physical, kind, alternative, operands, effects) = inputs();
            let fixup = canonical_fixup(MachineId::new(7).unwrap());
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                    &[0xe8, 0, 0, 0, 0],
                    fixup,
                ),
                Err(X86_64ScalarCallTemplateError::UnsupportedTarget)
            );
            assert!(
                validate_x86_64_selected_scalar_call_template(
                    NativeTarget::macos_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                    &[0xe8, 0, 0, 0, 0],
                    fixup,
                )
                .is_ok()
            );
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    SelectedInstructionKind::ReturnUnit,
                    alternative,
                    &operands,
                    &effects,
                    &[0xe8, 0, 0, 0, 0],
                    fixup,
                ),
                Err(X86_64ScalarCallTemplateError::InstructionKindMismatch)
            );
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    MachineAlternativeKey {
                        variant: 1,
                        ..alternative
                    },
                    &operands,
                    &effects,
                    &[0xe8, 0, 0, 0, 0],
                    fixup,
                ),
                Err(X86_64ScalarCallTemplateError::AlternativeMismatch)
            );
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands[..2],
                    &effects,
                    &[0xe8, 0, 0, 0, 0],
                    fixup,
                ),
                Err(X86_64ScalarCallTemplateError::OperandViewMismatch)
            );
            let mut malformed_effects = effects.clone();
            malformed_effects.external_operand_reads.clear();
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &malformed_effects,
                    &[0xe8, 0, 0, 0, 0],
                    fixup,
                ),
                Err(X86_64ScalarCallTemplateError::EffectMismatch)
            );
        }

        #[test]
        fn scalar_call_validation_rejects_nonzero_or_trailing_bytes_and_bad_fixup() {
            let (physical, kind, alternative, operands, effects) = inputs();
            let fixup = canonical_fixup(MachineId::new(7).unwrap());
            for malformed in [
                &[0xe9, 0, 0, 0, 0][..],
                &[0xe8, 1, 0, 0, 0][..],
                &[0xe8, 0, 0, 0][..],
                &[0xe8, 0, 0, 0, 0, 0][..],
            ] {
                assert_eq!(
                    validate_x86_64_selected_scalar_call_template(
                        NativeTarget::linux_x64(),
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &effects,
                        malformed,
                        fixup,
                    ),
                    Err(X86_64ScalarCallTemplateError::MalformedTemplate)
                );
            }
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                    &[0xe8, 0, 0, 0, 0],
                    X86_64ScalarCallFixup {
                        callee: MachineId::new(8).unwrap(),
                        ..fixup
                    },
                ),
                Err(X86_64ScalarCallTemplateError::FixupMismatch)
            );
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                    &[0xe8, 0, 0, 0, 0],
                    X86_64ScalarCallFixup {
                        patch_byte_offset: 0,
                        ..fixup
                    },
                ),
                Err(X86_64ScalarCallTemplateError::FixupMismatch)
            );
        }
    }

    fn expected_no_normal_mechanics(
        abi: X86_64SelectedAbi,
        physical: &ValidatedPhysicalRegisterModel,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
    ) -> Result<(Vec<RegisterViewId>, MachineEncodedEffects), X86_64ScalarCallTemplateError> {
        let microsoft = abi == X86_64SelectedAbi::Microsoft;
        let keys = if microsoft {
            crate::x86_64_microsoft_register_call_keys()
                .into_iter()
                .chain(crate::x86_64_microsoft_register_unit_call_keys())
                .chain(crate::x86_64_microsoft_mixed_unit_call_keys())
                .chain(crate::register_model::x86_64_microsoft_aggregate_call_keys())
                .chain(crate::x86_64_microsoft_mixed_aggregate_call_keys())
                .collect::<Vec<_>>()
        } else {
            crate::x86_64_system_v_register_call_keys()
                .into_iter()
                .chain(crate::x86_64_system_v_register_unit_call_keys())
                .chain(crate::x86_64_system_v_mixed_unit_call_keys())
                .chain(crate::register_model::x86_64_system_v_aggregate_call_keys())
                .chain(crate::x86_64_system_v_mixed_aggregate_call_keys())
                .collect::<Vec<_>>()
        };
        let keys = keys
            .into_iter()
            .chain(crate::x86_64_float_scalar_call_keys(microsoft))
            .chain(crate::x86_64_indirect_aggregate_call_keys(microsoft))
            .collect::<Vec<_>>();
        let catalog = x86_64_register_constraint_catalog_for(physical);
        let stack = physical
            .model()
            .view_named("rsp")
            .ok_or(X86_64ScalarCallTemplateError::NonCanonicalPhysicalModel)?
            .id;
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
            let expected = crate::machine_effects::user_direct_call_effects(row, stack);
            if &expected == effects {
                return Ok((operand_views.to_vec(), expected));
            }
        }
        Err(X86_64ScalarCallTemplateError::EffectMismatch)
    }

    fn expected_call_mechanics(
        abi: X86_64SelectedAbi,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        operand_views: &[RegisterViewId],
    ) -> Result<(Vec<RegisterViewId>, MachineEncodedEffects), X86_64ScalarCallTemplateError> {
        let unit = matches!(kind, SelectedInstructionKind::CallUnit { .. });
        let aggregate = matches!(kind, SelectedInstructionKind::CallAggregate { .. });
        let floating_scalar = !unit
            && !aggregate
            && operand_views.iter().any(|id| {
                physical
                    .model()
                    .views
                    .iter()
                    .any(|view| view.id == *id && view.name.starts_with("xmm"))
            });
        let (expected_operand_views, expected) = if unit || aggregate || floating_scalar {
            let keys = if aggregate {
                match abi {
                    X86_64SelectedAbi::SystemV => {
                        crate::register_model::x86_64_system_v_aggregate_call_keys()
                            .into_iter()
                            .chain(crate::x86_64_system_v_mixed_aggregate_call_keys())
                            .chain(crate::x86_64_indirect_aggregate_call_keys(false))
                            .collect()
                    }
                    X86_64SelectedAbi::Microsoft => {
                        crate::register_model::x86_64_microsoft_aggregate_call_keys()
                            .into_iter()
                            .chain(crate::x86_64_microsoft_mixed_aggregate_call_keys())
                            .chain(crate::x86_64_indirect_aggregate_call_keys(true))
                            .collect()
                    }
                }
            } else if floating_scalar {
                crate::x86_64_float_scalar_call_keys(abi == X86_64SelectedAbi::Microsoft)
            } else {
                match abi {
                    X86_64SelectedAbi::SystemV => crate::x86_64_system_v_register_unit_call_keys()
                        .into_iter()
                        .chain(crate::x86_64_system_v_mixed_unit_call_keys())
                        .collect::<Vec<_>>(),
                    X86_64SelectedAbi::Microsoft => {
                        crate::x86_64_microsoft_register_unit_call_keys()
                            .into_iter()
                            .chain(crate::x86_64_microsoft_mixed_unit_call_keys())
                            .collect::<Vec<_>>()
                    }
                }
            };
            let catalog = x86_64_register_constraint_catalog_for(physical);
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
                .ok_or(X86_64ScalarCallTemplateError::OperandViewMismatch)?;
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
                .filter(|arity| {
                    *arity
                        <= match abi {
                            X86_64SelectedAbi::SystemV => 6,
                            X86_64SelectedAbi::Microsoft => 4,
                        }
                })
                .ok_or(X86_64ScalarCallTemplateError::OperandViewMismatch)?;
            let expected_operand_views = expected_operand_views(abi, physical, arity);
            if operand_views != expected_operand_views {
                return Err(X86_64ScalarCallTemplateError::OperandViewMismatch);
            }
            (
                expected_operand_views,
                expected_effects(abi, physical, arity),
            )
        };

        Ok((expected_operand_views, expected))
    }

    pub fn encode_x86_64_selected_dynamic_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
    ) -> Result<(Vec<u8>, MachineEncodedEffects), X86_64ScalarCallTemplateError> {
        let abi = x86_64_selected_abi(target)
            .map_err(|_| X86_64ScalarCallTemplateError::UnsupportedTarget)?;
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
            return Err(X86_64ScalarCallTemplateError::NonCanonicalPhysicalModel);
        }
        let Some(offset) = kind.call_target().and_then(|target| target.indirect_code_offset())
        else {
            return Err(X86_64ScalarCallTemplateError::InstructionKindMismatch);
        };
        let family = match kind {
            SelectedInstructionKind::CallUnit { .. } => MachineAlternativeFamily::CallUnit,
            SelectedInstructionKind::CallScalar { .. } => MachineAlternativeFamily::CallScalar,
            SelectedInstructionKind::CallAggregate { .. } => {
                MachineAlternativeFamily::CallAggregate
            }
            _ => return Err(X86_64ScalarCallTemplateError::InstructionKindMismatch),
        };
        if alternative != (MachineAlternativeKey { family, variant: 1 }) {
            return Err(X86_64ScalarCallTemplateError::AlternativeMismatch);
        }
        let selector = physical
            .model()
            .view_named("r11")
            .ok_or(X86_64ScalarCallTemplateError::OperandViewMismatch)?
            .id;
        let mut ordinary = operand_views.to_vec();
        let position = ordinary
            .iter()
            .position(|view| *view == selector)
            .ok_or(X86_64ScalarCallTemplateError::OperandViewMismatch)?;
        ordinary.remove(position);
        let (_, baseline) = expected_call_mechanics(abi, physical, kind, &ordinary)?;
        if position != baseline.external_operand_reads.len() {
            return Err(X86_64ScalarCallTemplateError::OperandViewMismatch);
        }
        let expected = selected_instructions::calls::dynamic::encoded_effects(
            &baseline,
            physical.model().architecture,
        )
        .ok_or(X86_64ScalarCallTemplateError::EffectMismatch)?;
        if *effects != expected {
            return Err(X86_64ScalarCallTemplateError::EffectMismatch);
        }

        let mut bytes = Vec::with_capacity(24);
        bytes.extend_from_slice(&[0x49, 0xba]);
        crate::codec_bytes::push_encoded(&mut bytes, offset.to_le_bytes());
        bytes.extend_from_slice(&[
            0x4f, 0x8b, 0x1c, 0x13, 0x4d, 0x85, 0xdb, 0x75, 0x02, 0x0f, 0x0b, 0x41, 0xff, 0xd3,
        ]);
        Ok((bytes, expected))
    }

    #[cfg(test)]
    mod dynamic_call_tests {
        use super::*;
        use selected_instructions::MachineEncodedControlEffect;
        #[test]
        fn dispatch_register_is_separate_from_each_supported_data_arity_and_result() {
            let physical = register_model::validate_physical_register_model(
                crate::x86_64_physical_register_model(),
            )
            .unwrap();
            for target in [
                NativeTarget::linux_x64(),
                NativeTarget::macos_x64(),
                NativeTarget::windows_x64(),
            ] {
                let abi = x86_64_selected_abi(target).unwrap();
                for arity in [0, 1, 4] {
                    let mut operands = expected_operand_views(abi, &physical, arity);
                    let selector = physical.model().view_named("r11").unwrap().id;
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
                    let (bytes, actual) = encode_x86_64_selected_dynamic_call_template(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &effects,
                    )
                    .unwrap();
                    assert_eq!(bytes.len(), 24);
                    assert_eq!(
                        actual.external_operand_reads,
                        (0..=arity as u16).collect::<Vec<_>>()
                    );
                    assert_eq!(actual.external_operand_writes, vec![arity as u16 + 1]);
                    assert_eq!(
                        actual.control,
                        MachineEncodedControlEffect::IndirectTableCallOrTrapV1
                    );
                    assert_eq!(
                        &bytes[2..10],
                        &(u64::from(0x1234_5678_u32) * 8).to_le_bytes()
                    );
                    assert_eq!(&bytes[19..21], &[0x0f, 0x0b]);
                    assert_eq!(&bytes[21..24], &[0x41, 0xff, 0xd3]);
                    let mut missing_dispatch = operands.clone();
                    missing_dispatch.remove(arity);
                    assert!(
                        encode_x86_64_selected_dynamic_call_template(
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
                        encode_x86_64_selected_dynamic_call_template(
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
pub(crate) mod system_call {
    use super::{
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
        X86_64SelectedFormFootprint,
    };
    use register_model::{
        RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel, views_with_access,
    };
    use selected_instructions::{
        MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
    };
    use target::NativeTarget;

    pub fn encode_x86_64_selected_system_call_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        number: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let mut bytes = Vec::with_capacity(12);
        bytes.extend_from_slice(&[0x48, 0xb8]);
        crate::codec_bytes::push_encoded(&mut bytes, number.to_le_bytes());
        bytes.extend_from_slice(&[0x0f, 0x05]);
        validate_x86_64_selected_system_call_form(
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

    pub fn validate_x86_64_selected_system_call_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        number: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        if target != NativeTarget::linux_x64()
            || number < 0
            || physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
            || !matches!(kind, SelectedInstructionKind::NormalizedForeignCall { .. })
            || alternative != kind.call_alternative_key()
        {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        if bytes.len() != 12 || bytes[..2] != [0x48, 0xb8] || bytes[10..] != [0x0f, 0x05] {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let immediate: [u8; 8] = bytes[2..10]
            .try_into()
            .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        if i64::from_le_bytes(immediate) != number {
            return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let catalog = crate::x86_64_register_constraint_catalog_for(physical);
        let keys = crate::x86_64_linux_system_call_keys();
        let row = catalog
            .row_matching(&keys, operand_views, operand_accesses)
            .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        let effects = MachineEncodedEffects::supervisor_call_v1(row);
        let views_for = |access| views_with_access(operand_views, operand_accesses, access);
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: views_for(RegisterOperandAccess::Use),
                register_writes: views_for(RegisterOperandAccess::Def),
                writes_rflags: true,
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
            let physical = crate::validated_x86_64_physical_register_model();
            let target = NativeTarget::linux_x64();
            let catalog = crate::x86_64_register_constraint_catalog_for(physical);
            let kind = SelectedInstructionKind::NormalizedForeignCall {
                boundary: BoundaryMachineId::new(3).unwrap(),
                ordinal: 0,
            };
            let alternative = MachineAlternativeKey {
                family: selected_instructions::MachineAlternativeFamily::NormalizedForeignCall,
                variant: 0,
            };
            let summarize = |encoded: ValidatedX86_64SelectedFormEncoding| Encoding {
                bytes: encoded.bytes().to_vec(),
                effects: encoded.footprint().encoded.clone(),
            };
            let rows: Vec<_> = crate::x86_64_linux_system_call_keys()
                .into_iter()
                .map(|key| catalog.constraint(key).unwrap())
                .collect();
            conformance(
                &rows,
                physical.model().view_named("rsp").unwrap().id,
                |views, accesses, number| {
                    encode_x86_64_selected_system_call_form(
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
                    validate_x86_64_selected_system_call_form(
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
                    validate_x86_64_selected_system_call_form(
                        NativeTarget::windows_x64(),
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
        ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
        X86_64SelectedFormFootprint,
    };
    use register_model::{RegisterOperandAccess, RegisterViewId, views_with_access};
    use selected_instructions::{MachineAlternativeKey, SelectedNormalizedForeignCall};

    pub fn encode_x86_64_selected_table_call_form(
        record: &SelectedNormalizedForeignCall,
        alternative: MachineAlternativeKey,
        views: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let (register, offset) =
            table_address(record).ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        let mut bytes = vec![0x4c | (register >> 3), 0x8b, 0x98 | (register & 7)];
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(&[0x41, 0xff, 0xd3]);
        validate_x86_64_selected_table_call_form(record, alternative, views, accesses, &bytes)
    }

    pub fn validate_x86_64_selected_table_call_form(
        record: &SelectedNormalizedForeignCall,
        alternative: MachineAlternativeKey,
        views: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
        let invalid = || X86_64SelectedFormEncodingError::EncodedFormMismatch;
        let (register, offset) = table_address(record).ok_or_else(invalid)?;
        if bytes.len() != 10
            || bytes[0] != 0x4c | (register >> 3)
            || bytes[1] != 0x8b
            || bytes[2] != 0x98 | (register & 7)
            || bytes[7..] != [0x41, 0xff, 0xd3]
            || u32::from_le_bytes(bytes[3..7].try_into().map_err(|_| invalid())?) != offset
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
            crate::x86_64_normalized_foreign_call_declaration(record, &row).ok_or_else(invalid)?;
        let [form] = declaration.alternatives.as_slice() else {
            return Err(invalid());
        };
        if form.key != alternative {
            return Err(invalid());
        }
        let registers = |access| views_with_access(views, accesses, access);
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: X86_64SelectedFormFootprint {
                register_reads: registers(RegisterOperandAccess::Use),
                register_writes: registers(RegisterOperandAccess::Def),
                writes_rflags: true,
                encoded: form.encoded.clone(),
            },
        })
    }

    pub(crate) fn table_address(record: &SelectedNormalizedForeignCall) -> Option<(u8, u32)> {
        let (byte_offset, _) = record.call.binding.table_dispatch()?;
        if !record.call.callbacks.is_empty() || byte_offset > i32::MAX as u32 {
            return None;
        }
        let plan = &record.call.binding.boundary_entry_plan.call;
        let is_scratch = |location: &calling_conventions::ValueLocation| {
            location
                .direct_register()
                .and_then(super::normalized_foreign_call::x86_64_machine_register_encoding)
                == Some(11)
        };
        if plan
            .parameters
            .iter()
            .flat_map(|parameter| &parameter.locations)
            .any(is_scratch)
            || plan
                .result
                .iter()
                .flat_map(|result| &result.locations)
                .any(|location| {
                    matches!(
                        location,
                        calling_conventions::ValueLocation::Indirect { .. }
                    ) && is_scratch(location)
                })
        {
            return None;
        }
        let operands = record.call.binding.operand_placements();
        let parameter = operands.get(0)?;
        let [
            calling_conventions::ValueLocation::Register {
                register,
                byte_size: 8,
                value_byte_offset: 0,
            },
        ] = parameter.locations.as_slice()
        else {
            return None;
        };
        let register = super::normalized_foreign_call::x86_64_machine_register_encoding(*register)?;
        (register & 7 != 4).then_some((register, byte_offset))
    }
}
pub use table_call::{
    encode_x86_64_selected_table_call_form, validate_x86_64_selected_table_call_form,
};
#[cfg(test)]
mod tests {
    use super::{
        MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
        SelectedInstructionKind as Kind, ValidatedX86_64SelectedFormEncoding,
        X86_64SelectedFormEncodingError, encode_x86_64_selected_form,
        encode_x86_64_selected_i64_less_than_branch_form,
        encode_x86_64_selected_nonzero_branch_form,
        encode_x86_64_selected_short_nonzero_branch_form,
        encode_x86_64_selected_u64_less_than_branch_form, validate_x86_64_selected_form_encoding,
        validate_x86_64_selected_i64_less_than_branch_form,
        validate_x86_64_selected_nonzero_branch_form,
        validate_x86_64_selected_short_nonzero_branch_form,
        validate_x86_64_selected_u64_less_than_branch_form,
    };
    use crate::selected_form_encoding::decoding::{DecodedInstruction as Decoded, decode_one};
    use crate::selected_form_encoding::saturating_forms::SaturatingForm;
    use crate::selected_form_encoding::trapping_forms::{TrappingRow, TrappingShape};
    use crate::x86_64_physical_register_model;
    use optimization_core::AcceptedObligationFactIdentity;
    use register_model::{
        RegisterViewId, ValidatedPhysicalRegisterModel, validate_physical_register_model,
    };
    use selected_instructions::{
        MachineMemoryEffect, MachineSemanticKind, SaturatingCarrier, SaturatingOperation,
        TrappingForm, TrappingOperation,
    };
    use semantic_vocabulary::{IntegerSign, IntegerValue};
    use semantic_vocabulary::{MachineId, ObligationId};

    mod control {
        use super::*;
        use crate::selected_form_encoding::{
            MachineAlternativeFamily as Family, SelectedInstructionKind as Kind,
        };

        #[test]
        fn near_return_is_exact_and_separates_abi_result_custody_from_encoded_effects() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let rax = physical.model().view_named("rax").unwrap().id;
            let rbx = physical.model().view_named("rbx").unwrap().id;
            let rsp = physical.model().view_named("rsp").unwrap();
            let rip = physical.model().view_named("rip").unwrap();
            let kind = Kind::ReturnScalar;
            let alternative = alternative(Family::ReturnScalar, 0);
            let encoded =
                encode_x86_64_selected_form(&physical, kind, alternative, &[rax]).unwrap();

            assert_eq!(encoded.bytes(), [0xc3]);
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
            assert_eq!(encoded.footprint().encoded.implicit_unit_uses, rsp.units);
            let mut expected_defs = rsp.units.clone();
            expected_defs.extend(&rip.units);
            expected_defs.sort_unstable();
            expected_defs.dedup();
            assert_eq!(
                encoded.footprint().encoded.implicit_unit_defs,
                expected_defs
            );
            assert_eq!(
                encoded.footprint().encoded.memory,
                MachineEncodedMemoryEffect::ReadActivationStackV1 {
                    stack_pointer: rsp.id,
                    byte_count: 8,
                }
            );
            assert_eq!(
                encoded.footprint().encoded.stack,
                MachineEncodedStackEffect::PopBytesV1 {
                    stack_pointer: rsp.id,
                    byte_count: 8,
                }
            );
            assert_eq!(
                encoded.footprint().encoded.trap,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            );
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ReturnFromActivationStackV1
            );
            assert!(encode_x86_64_selected_form(&physical, kind, alternative, &[rbx]).is_err());
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &[rax],
                    &[0xc2, 0, 0]
                )
                .is_err()
            );
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &[rax],
                    &[0xc3, 0xc3]
                )
                .is_err()
            );
        }

        #[test]
        fn unit_return_is_a_distinct_zero_operand_near_return() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let kind = Kind::ReturnUnit;
            let return_alternative = alternative(Family::ReturnUnit, 0);
            let encoded =
                encode_x86_64_selected_form(&physical, kind, return_alternative, &[]).unwrap();

            assert_eq!(encoded.bytes(), [0xc3]);
            assert!(encoded.footprint().register_reads.is_empty());
            assert!(encoded.footprint().register_writes.is_empty());
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ReturnFromActivationStackV1
            );
            assert!(
                encode_x86_64_selected_form(
                    &physical,
                    kind,
                    alternative(Family::ReturnScalar, 0),
                    &[]
                )
                .is_err()
            );
        }

        #[test]
        fn near_nonzero_branch_has_exact_end_relative_displacement_and_effects() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let alternative = alternative(Family::ConditionalBranchNonZero, 0);
            for displacement in [i64::from(i32::MIN), -6, 0, 6, i64::from(i32::MAX)] {
                let encoded = encode_x86_64_selected_nonzero_branch_form(
                    &physical,
                    alternative,
                    displacement,
                )
                .unwrap();
                assert_eq!(&encoded.bytes()[..2], [0x0f, 0x85]);
                assert_eq!(
                    i32::from_le_bytes(encoded.bytes()[2..].try_into().unwrap()),
                    displacement as i32
                );
                assert!(encoded.footprint().register_reads.is_empty());
                assert!(encoded.footprint().register_writes.is_empty());
                assert_eq!(
                    encoded.footprint().encoded.control,
                    MachineEncodedControlEffect::ConditionalRelativeBranchV1
                );
            }
            assert!(
                encode_x86_64_selected_nonzero_branch_form(
                    &physical,
                    alternative,
                    i64::from(i32::MAX) + 1
                )
                .is_err()
            );
            assert!(
                validate_x86_64_selected_nonzero_branch_form(
                    &physical,
                    alternative,
                    0,
                    &[0x0f, 0x84, 0, 0, 0, 0]
                )
                .is_err()
            );
            assert!(
                validate_x86_64_selected_nonzero_branch_form(
                    &physical,
                    alternative,
                    0,
                    &[0x0f, 0x85, 0, 0, 0, 0, 0]
                )
                .is_err()
            );
        }

        #[test]
        fn short_nonzero_branch_has_exact_signed_rel8_bounds_and_near_footprint() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let alternative = alternative(Family::ConditionalBranchNonZero, 0);
            let near =
                encode_x86_64_selected_nonzero_branch_form(&physical, alternative, 0).unwrap();

            for (displacement, encoded_displacement) in [(-128, 0x80), (127, 0x7f)] {
                let encoded = encode_x86_64_selected_short_nonzero_branch_form(
                    &physical,
                    alternative,
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.bytes(), [0x75, encoded_displacement]);
                assert_eq!(encoded.footprint(), near.footprint());
            }

            for displacement in [-129, 128] {
                assert_eq!(
                    encode_x86_64_selected_short_nonzero_branch_form(
                        &physical,
                        alternative,
                        displacement,
                    ),
                    Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
                );
                assert_eq!(
                    validate_x86_64_selected_short_nonzero_branch_form(
                        &physical,
                        alternative,
                        displacement,
                        &[0x75, 0],
                    ),
                    Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
                );
            }
        }

        #[test]
        fn short_nonzero_branch_validation_rejects_every_noncanonical_form() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let canonical_alternative = alternative(Family::ConditionalBranchNonZero, 0);

            for bytes in [&[0x74, 0][..], &[0x75][..], &[0x75, 0, 0][..]] {
                assert_eq!(
                    validate_x86_64_selected_short_nonzero_branch_form(
                        &physical,
                        canonical_alternative,
                        0,
                        bytes,
                    ),
                    Err(X86_64SelectedFormEncodingError::MalformedEncoding)
                );
            }
            assert_eq!(
                validate_x86_64_selected_short_nonzero_branch_form(
                    &physical,
                    canonical_alternative,
                    0,
                    &[0x75, 1],
                ),
                Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
            );

            let wrong_alternative = alternative(Family::ConditionalBranchNonZero, 1);
            assert_eq!(
                encode_x86_64_selected_short_nonzero_branch_form(&physical, wrong_alternative, 0,),
                Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
            );
            assert_eq!(
                validate_x86_64_selected_short_nonzero_branch_form(
                    &physical,
                    alternative(Family::ReturnScalar, 0),
                    0,
                    &[0x75, 0],
                ),
                Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
            );

            let mut forged = x86_64_physical_register_model();
            forged.views[0].name = "forged.rax".into();
            let forged = validate_physical_register_model(forged).unwrap();
            assert_eq!(
                encode_x86_64_selected_short_nonzero_branch_form(&forged, canonical_alternative, 0,),
                Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)
            );
        }

        #[test]
        fn near_u64_less_than_branch_is_exact_jb_rel32_with_flag_control_effects() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let condition_flags = physical.model().view_named("condition_flags").unwrap();
            let rip = physical.model().view_named("rip").unwrap();
            let alternative = alternative(Family::ConditionalBranchU64LessThan, 0);
            for displacement in [i64::from(i32::MIN), -6, 0, 6, i64::from(i32::MAX)] {
                let encoded = encode_x86_64_selected_u64_less_than_branch_form(
                    &physical,
                    alternative,
                    displacement,
                )
                .unwrap();
                assert_eq!(&encoded.bytes()[..2], [0x0f, 0x82]);
                assert_eq!(
                    i32::from_le_bytes(encoded.bytes()[2..].try_into().unwrap()),
                    displacement as i32
                );
                assert!(encoded.footprint().register_reads.is_empty());
                assert!(encoded.footprint().register_writes.is_empty());
                assert_eq!(
                    encoded.footprint().encoded.control,
                    MachineEncodedControlEffect::ConditionalRelativeBranchV1
                );
                assert!(condition_flags.units.iter().all(|unit| {
                    encoded
                        .footprint()
                        .encoded
                        .implicit_unit_uses
                        .contains(unit)
                }));
                assert!(rip.units.iter().all(|unit| {
                    encoded
                        .footprint()
                        .encoded
                        .implicit_unit_uses
                        .contains(unit)
                }));
                assert_eq!(encoded.footprint().encoded.implicit_unit_defs, rip.units);
            }
            assert_eq!(
                encode_x86_64_selected_u64_less_than_branch_form(
                    &physical,
                    alternative,
                    i64::from(i32::MAX) + 1,
                ),
                Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)
            );
            for bytes in [
                &[0x0f, 0x83, 0, 0, 0, 0][..],
                &[0x0f, 0x82, 0, 0, 0][..],
                &[0x0f, 0x82, 0, 0, 0, 0, 0][..],
            ] {
                assert_eq!(
                    validate_x86_64_selected_u64_less_than_branch_form(
                        &physical,
                        alternative,
                        0,
                        bytes,
                    ),
                    Err(X86_64SelectedFormEncodingError::MalformedEncoding)
                );
            }
        }

        #[test]
        fn short_u64_less_than_branch_is_exact_jb_rel8_and_rejects_near_forms() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let near_alternative = alternative(Family::ConditionalBranchU64LessThan, 0);
            let short_alternative = alternative(Family::ConditionalBranchU64LessThan, 1);
            let near =
                encode_x86_64_selected_u64_less_than_branch_form(&physical, near_alternative, 0)
                    .unwrap();
            for (displacement, encoded_displacement) in [(-128, 0x80), (127, 0x7f)] {
                let encoded = encode_x86_64_selected_u64_less_than_branch_form(
                    &physical,
                    short_alternative,
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.bytes(), [0x72, encoded_displacement]);
                assert_eq!(encoded.footprint(), near.footprint());
            }
            for displacement in [-129, 128] {
                assert_eq!(
                    encode_x86_64_selected_u64_less_than_branch_form(
                        &physical,
                        short_alternative,
                        displacement,
                    ),
                    Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
                );
            }
            for bytes in [
                &[0x73, 0][..],
                &[0x72][..],
                &[0x72, 0, 0][..],
                &[0x0f, 0x82, 0, 0, 0, 0][..],
            ] {
                assert_eq!(
                    validate_x86_64_selected_u64_less_than_branch_form(
                        &physical,
                        short_alternative,
                        0,
                        bytes,
                    ),
                    Err(X86_64SelectedFormEncodingError::MalformedEncoding)
                );
            }
            assert_eq!(
                validate_x86_64_selected_u64_less_than_branch_form(
                    &physical,
                    short_alternative,
                    0,
                    &[0x72, 1],
                ),
                Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
            );
        }

        #[test]
        fn signed_less_than_branch_uses_exact_near_and_short_jl_forms() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let near = alternative(Family::ConditionalBranchI64LessThan, 0);
            let short = alternative(Family::ConditionalBranchI64LessThan, 1);

            let encoded =
                encode_x86_64_selected_i64_less_than_branch_form(&physical, near, -6).unwrap();
            assert_eq!(encoded.bytes(), [0x0f, 0x8c, 0xfa, 0xff, 0xff, 0xff]);
            assert_eq!(
                encoded.footprint().encoded.control,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            );
            let encoded =
                encode_x86_64_selected_i64_less_than_branch_form(&physical, short, -2).unwrap();
            assert_eq!(encoded.bytes(), [0x7c, 0xfe]);
            assert_eq!(
                validate_x86_64_selected_i64_less_than_branch_form(&physical, short, 0, &[0x72, 0],),
                Err(X86_64SelectedFormEncodingError::MalformedEncoding)
            );
            assert_eq!(
                encode_x86_64_selected_i64_less_than_branch_form(&physical, short, 128),
                Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
            );
        }
    }
    mod saturating {
        use super::*;

        macro_rules! row {
            ($a:expr => $b:expr => $c:expr => $d:expr) => {
                ($a, $b, $c, $d)
            };
        }

        const SATURATING_OPERATIONS: [SaturatingOperation; 5] = [
            SaturatingOperation::Add,
            SaturatingOperation::Subtract,
            SaturatingOperation::Divide,
            SaturatingOperation::Remainder,
            SaturatingOperation::Multiply,
        ];

        fn independently_assembled_saturating_forms() -> Vec<(
            SaturatingOperation,
            SaturatingCarrier,
            Vec<&'static str>,
            Vec<u8>,
        )> {
            let signed_narrow_low = |maximum: [u8; 8], minimum: [u8; 8], arithmetic: u8| {
                let mut bytes = vec![0x48, 0x89, 0xf8, 0x48, arithmetic, 0xf0, 0x48, 0xb9];
                bytes.extend(maximum);
                bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4f, 0xc1, 0x48, 0xb9]);
                bytes.extend(minimum);
                bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4c, 0xc1]);
                bytes
            };
            let signed_narrow_high = |maximum: [u8; 8], minimum: [u8; 8], arithmetic: u8| {
                let mut bytes = vec![0x4d, 0x89, 0xc2, 0x4d, arithmetic, 0xda, 0x49, 0xb9];
                bytes.extend(maximum);
                bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4f, 0xd1, 0x49, 0xb9]);
                bytes.extend(minimum);
                bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4c, 0xd1]);
                bytes
            };
            let unsigned_narrow_low = |maximum: [u8; 8]| {
                let mut bytes = vec![0x48, 0x89, 0xf8, 0x48, 0x01, 0xf0, 0x48, 0xb9];
                bytes.extend(maximum);
                bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4f, 0xc1]);
                bytes
            };
            let signed_narrow_divide_low = |maximum: [u8; 8]| {
                let mut bytes = vec![0x48, 0x99, 0x48, 0xf7, 0xfe, 0x48, 0xba];
                bytes.extend(maximum);
                bytes.extend([0x48, 0x39, 0xd0, 0x48, 0x0f, 0x4f, 0xc2]);
                bytes
            };
            let i8_maximum = [0x7f, 0, 0, 0, 0, 0, 0, 0];
            let i8_minimum = [0x80, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
            let i16_maximum = [0xff, 0x7f, 0, 0, 0, 0, 0, 0];
            let i16_minimum = [0x00, 0x80, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
            let i32_maximum = [0xff, 0xff, 0xff, 0x7f, 0, 0, 0, 0];
            let i32_minimum = [0x00, 0x00, 0x00, 0x80, 0xff, 0xff, 0xff, 0xff];
            let low4 = vec!["rdi", "rsi", "rax", "rcx"];
            let high4 = vec!["r8", "r11", "r10", "r9"];
            let low3 = vec!["rdi", "rsi", "rax"];
            let high3 = vec!["r8", "r9", "r10"];
            let low_divide = vec!["rax", "rsi", "rax", "rdx"];
            let high_divide = vec!["rax", "r9", "rax", "rdx"];
            let i64_overflow_select_low = |arithmetic: u8| {
                vec![
                    0x48, 0x89, 0xf9, 0x48, 0xf7, 0xd1, 0x48, 0xc1, 0xf9, 0x3f, 0x48, 0x0f, 0xba,
                    0xf9, 0x3f, 0x48, 0x89, 0xf8, 0x48, arithmetic, 0xf0, 0x48, 0x0f, 0x40, 0xc1,
                ]
            };
            let i64_overflow_select_high = |arithmetic: u8| {
                vec![
                    0x4d, 0x89, 0xc1, 0x49, 0xf7, 0xd1, 0x49, 0xc1, 0xf9, 0x3f, 0x49, 0x0f, 0xba,
                    0xf9, 0x3f, 0x4d, 0x89, 0xc2, 0x4d, arithmetic, 0xda, 0x4d, 0x0f, 0x40, 0xd1,
                ]
            };
            let unsigned_subtract_low = vec![
                0x48, 0x39, 0xf7, 0x48, 0x89, 0xf8, 0x48, 0x0f, 0x42, 0xc6, 0x48, 0x29, 0xf0,
            ];
            let unsigned_subtract_high = vec![
                0x4d, 0x39, 0xc8, 0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0x42, 0xd1, 0x4d, 0x29, 0xca,
            ];
            let signed_narrow_multiply_low = |maximum: [u8; 8], minimum: [u8; 8]| {
                let mut bytes = vec![0x48, 0x89, 0xf8, 0x48, 0x0f, 0xaf, 0xc6, 0x48, 0xb9];
                bytes.extend(maximum);
                bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4f, 0xc1, 0x48, 0xb9]);
                bytes.extend(minimum);
                bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4c, 0xc1]);
                bytes
            };
            let unsigned_narrow_multiply_low = |maximum: [u8; 8]| {
                let mut bytes = vec![0x48, 0x89, 0xf8, 0x48, 0x0f, 0xaf, 0xc6, 0x48, 0xb9];
                bytes.extend(maximum);
                bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x47, 0xc1]);
                bytes
            };
            use SaturatingCarrier::*;
            use SaturatingOperation::*;
            vec![
                row!(Multiply => I8 => low4.clone() => signed_narrow_multiply_low(i8_maximum, i8_minimum)),
                (
                    Multiply,
                    I16,
                    low4.clone(),
                    signed_narrow_multiply_low(i16_maximum, i16_minimum),
                ),
                (Multiply, I32, high4.clone(), {
                    let mut bytes = vec![0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0xaf, 0xd3, 0x49, 0xb9];
                    bytes.extend(i32_maximum);
                    bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4f, 0xd1, 0x49, 0xb9]);
                    bytes.extend(i32_minimum);
                    bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4c, 0xd1]);
                    bytes
                }),
                (
                    Multiply,
                    U32,
                    low4.clone(),
                    unsigned_narrow_multiply_low([0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]),
                ),
                (
                    Multiply,
                    U16,
                    low4.clone(),
                    unsigned_narrow_multiply_low([0xff, 0xff, 0, 0, 0, 0, 0, 0]),
                ),
                (Multiply, U8, high4.clone(), {
                    let mut bytes = vec![0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0xaf, 0xd3, 0x49, 0xb9];
                    bytes.extend([0xff, 0, 0, 0, 0, 0, 0, 0]);
                    bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x47, 0xd1]);
                    bytes
                }),
                (
                    Multiply,
                    I64,
                    low4.clone(),
                    vec![
                        0x48, 0x89, 0xf9, 0x48, 0x31, 0xf1, 0x48, 0xf7, 0xd1, 0x48, 0xc1, 0xf9,
                        0x3f, 0x48, 0x0f, 0xba, 0xf9, 0x3f, 0x48, 0x89, 0xf8, 0x48, 0x0f, 0xaf,
                        0xc6, 0x48, 0x0f, 0x40, 0xc1,
                    ],
                ),
                (
                    Multiply,
                    I64,
                    high4.clone(),
                    vec![
                        0x4d, 0x89, 0xc1, 0x4d, 0x31, 0xd9, 0x49, 0xf7, 0xd1, 0x49, 0xc1, 0xf9,
                        0x3f, 0x49, 0x0f, 0xba, 0xf9, 0x3f, 0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0xaf,
                        0xd3, 0x4d, 0x0f, 0x40, 0xd1,
                    ],
                ),
                (
                    Multiply,
                    U64,
                    low_divide.clone(),
                    vec![0x48, 0xf7, 0xe6, 0x48, 0x19, 0xd2, 0x48, 0x09, 0xd0],
                ),
                (
                    Multiply,
                    U64,
                    high_divide.clone(),
                    vec![0x49, 0xf7, 0xe1, 0x48, 0x19, 0xd2, 0x48, 0x09, 0xd0],
                ),
                row!(Add => I8 => low4.clone() => signed_narrow_low(i8_maximum, i8_minimum, 0x01)),
                row!(Add => I16 => low4.clone() => signed_narrow_low(i16_maximum, i16_minimum, 0x01)),
                row!(Add => I32 => low4.clone() => signed_narrow_low(i32_maximum, i32_minimum, 0x01)),
                row!(Subtract => I8 => low4.clone() => signed_narrow_low(i8_maximum, i8_minimum, 0x29)),
                row!(Subtract => I16 => low4.clone() => signed_narrow_low(i16_maximum, i16_minimum, 0x29)),
                row!(Subtract => I32 => low4.clone() => signed_narrow_low(i32_maximum, i32_minimum, 0x29)),
                row!(Add => I8 => high4.clone() => signed_narrow_high(i8_maximum, i8_minimum, 0x01)),
                (
                    Subtract,
                    I16,
                    high4.clone(),
                    signed_narrow_high(i16_maximum, i16_minimum, 0x29),
                ),
                (
                    Add,
                    U32,
                    low4.clone(),
                    unsigned_narrow_low([0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]),
                ),
                row!(Add => U8 => low4.clone() => unsigned_narrow_low([0xff, 0, 0, 0, 0, 0, 0, 0])),
                row!(Add => U16 => low4.clone() => unsigned_narrow_low([0xff, 0xff, 0, 0, 0, 0, 0, 0])),
                (Add, U16, high4.clone(), {
                    let mut bytes = vec![0x4d, 0x89, 0xc2, 0x4d, 0x01, 0xda, 0x49, 0xb9];
                    bytes.extend([0xff, 0xff, 0, 0, 0, 0, 0, 0]);
                    bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4f, 0xd1]);
                    bytes
                }),
                (Add, I64, low4.clone(), i64_overflow_select_low(0x01)),
                (Add, I64, high4.clone(), i64_overflow_select_high(0x01)),
                (Subtract, I64, low4.clone(), i64_overflow_select_low(0x29)),
                (Subtract, I64, high4.clone(), i64_overflow_select_high(0x29)),
                (Subtract, U8, low3.clone(), unsigned_subtract_low.clone()),
                (Subtract, U16, low3.clone(), unsigned_subtract_low.clone()),
                (Subtract, U32, low3.clone(), unsigned_subtract_low.clone()),
                (Subtract, U64, low3.clone(), unsigned_subtract_low.clone()),
                (Subtract, U32, high3.clone(), unsigned_subtract_high.clone()),
                (Subtract, U64, high3.clone(), unsigned_subtract_high),
                (
                    Add,
                    U64,
                    high3,
                    vec![
                        0x4d, 0x89, 0xc2, 0x49, 0xf7, 0xd2, 0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x42,
                        0xd1, 0x4d, 0x29, 0xca, 0x49, 0xf7, 0xd2,
                    ],
                ),
                (
                    Add,
                    U64,
                    low3,
                    vec![
                        0x48, 0x89, 0xf8, 0x48, 0xf7, 0xd0, 0x48, 0x39, 0xf0, 0x48, 0x0f, 0x42,
                        0xc6, 0x48, 0x29, 0xf0, 0x48, 0xf7, 0xd0,
                    ],
                ),
                (Divide, U8, low_divide.clone(), vec![0x48, 0xf7, 0xf6]),
                (Divide, U64, low_divide.clone(), vec![0x48, 0xf7, 0xf6]),
                (Divide, U16, high_divide.clone(), vec![0x49, 0xf7, 0xf1]),
                (Divide, U32, high_divide.clone(), vec![0x49, 0xf7, 0xf1]),
                row!(Divide => I16 => low_divide.clone() => signed_narrow_divide_low(i16_maximum)),
                row!(Divide => I8 => low_divide.clone() => signed_narrow_divide_low(i8_maximum)),
                row!(Divide => I32 => low_divide.clone() => signed_narrow_divide_low(i32_maximum)),
                (
                    Divide,
                    I32,
                    high_divide.clone(),
                    vec![
                        0x48, 0x99, 0x49, 0xf7, 0xf9, 0x48, 0xba, 0xff, 0xff, 0xff, 0x7f, 0x00,
                        0x00, 0x00, 0x00, 0x48, 0x39, 0xd0, 0x48, 0x0f, 0x4f, 0xc2,
                    ],
                ),
                (
                    Divide,
                    I64,
                    low_divide.clone(),
                    vec![
                        0x48, 0x83, 0xfe, 0xff, 0x48, 0x19, 0xd2, 0x48, 0x09, 0xc2, 0x48, 0xf7,
                        0xda, 0x48, 0x8d, 0x50, 0x01, 0x48, 0x0f, 0x40, 0xc2, 0x48, 0x99, 0x48,
                        0xf7, 0xfe,
                    ],
                ),
                (
                    Divide,
                    I64,
                    high_divide.clone(),
                    vec![
                        0x49, 0x83, 0xf9, 0xff, 0x48, 0x19, 0xd2, 0x48, 0x09, 0xc2, 0x48, 0xf7,
                        0xda, 0x48, 0x8d, 0x50, 0x01, 0x48, 0x0f, 0x40, 0xc2, 0x48, 0x99, 0x49,
                        0xf7, 0xf9,
                    ],
                ),
                (
                    Remainder,
                    U8,
                    low_divide.clone(),
                    vec![0x48, 0x31, 0xd2, 0x48, 0xf7, 0xf6, 0x48, 0x89, 0xd0],
                ),
                (
                    Remainder,
                    U32,
                    low_divide.clone(),
                    vec![0x48, 0x31, 0xd2, 0x48, 0xf7, 0xf6, 0x48, 0x89, 0xd0],
                ),
                (
                    Remainder,
                    U16,
                    high_divide.clone(),
                    vec![0x48, 0x31, 0xd2, 0x49, 0xf7, 0xf1, 0x48, 0x89, 0xd0],
                ),
                (
                    Remainder,
                    U64,
                    high_divide.clone(),
                    vec![0x48, 0x31, 0xd2, 0x49, 0xf7, 0xf1, 0x48, 0x89, 0xd0],
                ),
                (
                    Remainder,
                    I8,
                    low_divide.clone(),
                    vec![
                        0x48, 0x31, 0xd2, 0x48, 0x83, 0xfe, 0xff, 0x74, 0x05, 0x48, 0x99, 0x48,
                        0xf7, 0xfe, 0x48, 0x89, 0xd0,
                    ],
                ),
                (
                    Remainder,
                    I32,
                    low_divide.clone(),
                    vec![
                        0x48, 0x31, 0xd2, 0x48, 0x83, 0xfe, 0xff, 0x74, 0x05, 0x48, 0x99, 0x48,
                        0xf7, 0xfe, 0x48, 0x89, 0xd0,
                    ],
                ),
                (
                    Remainder,
                    I16,
                    high_divide.clone(),
                    vec![
                        0x48, 0x31, 0xd2, 0x49, 0x83, 0xf9, 0xff, 0x74, 0x05, 0x48, 0x99, 0x49,
                        0xf7, 0xf9, 0x48, 0x89, 0xd0,
                    ],
                ),
                (
                    Remainder,
                    I64,
                    high_divide,
                    vec![
                        0x48, 0x31, 0xd2, 0x49, 0x83, 0xf9, 0xff, 0x74, 0x05, 0x48, 0x99, 0x49,
                        0xf7, 0xf9, 0x48, 0x89, 0xd0,
                    ],
                ),
            ]
        }

        #[test]
        fn saturating_forms_match_independent_assembler_for_every_carrier() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let mut covered = Vec::new();
            for (operation, carrier, names, expected) in independently_assembled_saturating_forms()
            {
                let kind = saturating_kind(operation, carrier);
                let key = saturating_key(operation, carrier);
                let operands = saturating_operands(&physical, &names);
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                assert_eq!(
                    encoded.bytes(),
                    expected,
                    "{operation:?} {carrier:?} {names:?}"
                );
                assert_eq!(
                    expected.len(),
                    usize::from(SaturatingForm::of(operation, carrier).byte_count()),
                    "{operation:?} {carrier:?}"
                );
                validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &expected)
                    .unwrap();
                let high = names.iter().any(|name| {
                    name[1..]
                        .chars()
                        .all(|character| character.is_ascii_digit())
                });
                covered.push((
                    operation,
                    carrier,
                    SaturatingForm::of(operation, carrier),
                    high,
                ));
            }
            for operation in SATURATING_OPERATIONS {
                for carrier in SaturatingCarrier::ALL {
                    assert!(
                        covered
                            .iter()
                            .any(|(o, c, ..)| (*o, *c) == (operation, carrier)),
                        "{operation:?} {carrier:?} has no independently assembled bytes"
                    );
                }
            }
            for form in [
                SaturatingForm::AddU64,
                SaturatingForm::SubtractUnsigned,
                SaturatingForm::DivideUnsigned,
                SaturatingForm::ClampSignedNarrow,
                SaturatingForm::ClampUnsignedNarrow,
                SaturatingForm::OverflowSelectI64,
                SaturatingForm::DivideSignedNarrow,
                SaturatingForm::DivideI64,
                SaturatingForm::RemainderUnsigned,
                SaturatingForm::RemainderSigned,
                SaturatingForm::MultiplySignedNarrow,
                SaturatingForm::MultiplyUnsignedNarrow,
                SaturatingForm::MultiplyI64,
                SaturatingForm::MultiplyU64,
            ] {
                for high in [false, true] {
                    assert!(
                        covered.iter().any(|(_, _, f, h)| (*f, *h) == (form, high)),
                        "{form:?} high={high} has no independently assembled bytes"
                    );
                }
            }
        }

        #[test]
        fn saturating_forms_reject_every_mutation_substitution_and_sibling_carrier() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let substitute = physical.model().view_named("r14").unwrap().id;
            let mut canonical = Vec::new();
            for operation in SATURATING_OPERATIONS {
                for carrier in SaturatingCarrier::ALL {
                    for high in [false, true] {
                        let kind = saturating_kind(operation, carrier);
                        let key = saturating_key(operation, carrier);
                        let operands = saturating_operands(
                            &physical,
                            &saturating_operand_names(operation, carrier, high),
                        );
                        let encoded =
                            encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                        let form = SaturatingForm::of(operation, carrier);
                        assert_eq!(encoded.bytes().len(), usize::from(form.byte_count()));
                        let (reads, writes) = form.operand_reads_and_writes();
                        assert!(encoded.footprint().writes_rflags);
                        assert_eq!(encoded.footprint().encoded.external_operand_reads, reads);
                        assert_eq!(encoded.footprint().encoded.external_operand_writes, writes);
                        assert_eq!(
                            encoded.footprint().register_reads,
                            reads
                                .iter()
                                .map(|&position| operands[usize::from(position)])
                                .collect::<Vec<_>>()
                        );
                        assert_eq!(
                            encoded.footprint().register_writes,
                            writes
                                .iter()
                                .map(|&position| operands[usize::from(position)])
                                .collect::<Vec<_>>()
                        );
                        let rdx = physical.model().view_named("rdx").unwrap().units.clone();
                        let clobbers = &encoded.footprint().encoded.implicit_unit_clobbers;
                        assert!(!clobbers.is_empty());
                        let rdx_is_implicit_clobber = matches!(
                            form,
                            SaturatingForm::DivideUnsigned
                                | SaturatingForm::DivideSignedNarrow
                                | SaturatingForm::DivideI64
                        );
                        assert_eq!(
                            rdx.iter().all(|unit| clobbers.contains(unit)),
                            rdx_is_implicit_clobber,
                            "{operation:?} {carrier:?} RDX clobber"
                        );
                        assert_eq!(
                            encoded.footprint().encoded.trap
                                == MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                            form.is_division()
                        );
                        assert_every_bit_change_rejected(
                            &physical,
                            kind,
                            key,
                            &operands,
                            encoded.bytes(),
                            &format!("{operation:?} {carrier:?} high={high}"),
                        );
                        assert_every_operand_substitution_rejected(
                            &physical,
                            kind,
                            key,
                            &operands,
                            substitute,
                            encoded.bytes(),
                            &format!("{operation:?} {carrier:?}"),
                        );
                        if !high {
                            canonical.push((operation, carrier, kind, key, operands, encoded));
                        }
                    }
                }
            }
            for (operation, carrier, _, _, _, encoded) in &canonical {
                for (
                    other_operation,
                    other_carrier,
                    other_kind,
                    other_key,
                    other_operands,
                    other,
                ) in &canonical
                {
                    if (operation, carrier) == (other_operation, other_carrier) {
                        continue;
                    }
                    let accepted = validate_x86_64_selected_form_encoding(
                        &physical,
                        *other_kind,
                        *other_key,
                        other_operands,
                        encoded.bytes(),
                    )
                    .is_ok();
                    let shared = operation == other_operation
                        && encoded.bytes() == other.bytes()
                        && matches!(
                            SaturatingForm::of(*operation, *carrier),
                            SaturatingForm::SubtractUnsigned
                                | SaturatingForm::DivideUnsigned
                                | SaturatingForm::RemainderUnsigned
                                | SaturatingForm::RemainderSigned
                        );
                    assert_eq!(
                        accepted, shared,
                        "{operation:?} {carrier:?} bytes replayed as {other_operation:?} {other_carrier:?}"
                    );
                }
            }
        }

        #[test]
        fn saturating_forms_pin_outputs_away_from_inputs_and_division_to_rax_rdx() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            for operation in SATURATING_OPERATIONS {
                for carrier in SaturatingCarrier::ALL {
                    let kind = saturating_kind(operation, carrier);
                    let key = saturating_key(operation, carrier);
                    let form = SaturatingForm::of(operation, carrier);
                    let invalid: Vec<Vec<&str>> = if form.is_fixed_rax_rdx() {
                        vec![
                            vec!["rax", "rdx", "rax", "rdx"],
                            vec!["rcx", "r9", "rax", "rdx"],
                            vec!["rax", "r9", "rcx", "rdx"],
                            vec!["rax", "r9", "rax", "rcx"],
                        ]
                    } else if form.operand_count() == 4 {
                        vec![
                            vec!["r9", "r10", "r9", "r12"],
                            vec!["r9", "r10", "r10", "r12"],
                            vec!["r9", "r10", "r11", "r9"],
                            vec!["r9", "r10", "r11", "r10"],
                            vec!["r9", "r10", "r11", "r11"],
                        ]
                    } else {
                        vec![vec!["r9", "r10", "r9"], vec!["r9", "r10", "r10"]]
                    };
                    for names in invalid {
                        let operands = saturating_operands(&physical, &names);
                        assert!(
                            encode_x86_64_selected_form(&physical, kind, key, &operands).is_err(),
                            "{operation:?} {carrier:?} {names:?}"
                        );
                    }
                    let wrong_count = saturating_operand_names(operation, carrier, false);
                    let mut too_few = saturating_operands(&physical, &wrong_count);
                    too_few.pop();
                    assert_eq!(
                        encode_x86_64_selected_form(&physical, kind, key, &too_few),
                        Err(X86_64SelectedFormEncodingError::OperandCountMismatch)
                    );
                }
            }
        }

        fn execute_saturating(bytes: &[u8], registers: [u64; 16]) -> [u64; 16] {
            match execute_decoded(bytes, registers) {
                DecodedOutcome::Completed(registers) => registers,
                DecodedOutcome::Trapped => panic!("a saturating form never traps"),
            }
        }

        fn saturating_reference(
            operation: SaturatingOperation,
            carrier: SaturatingCarrier,
            left: i128,
            right: i128,
        ) -> u64 {
            macro_rules! reference {
                ($type:ty) => {{
                    let left = <$type>::try_from(left).unwrap();
                    let right = <$type>::try_from(right).unwrap();
                    let result = match operation {
                        SaturatingOperation::Add => left.saturating_add(right),
                        SaturatingOperation::Subtract => left.saturating_sub(right),
                        SaturatingOperation::Divide => left.saturating_div(right),
                        SaturatingOperation::Remainder => left.wrapping_rem(right),
                        SaturatingOperation::Multiply => left.saturating_mul(right),
                    };
                    i128::from(result) as u64
                }};
            }
            match carrier {
                SaturatingCarrier::I8 => reference!(i8),
                SaturatingCarrier::I16 => reference!(i16),
                SaturatingCarrier::I32 => reference!(i32),
                SaturatingCarrier::I64 => reference!(i64),
                SaturatingCarrier::U8 => reference!(u8),
                SaturatingCarrier::U16 => reference!(u16),
                SaturatingCarrier::U32 => reference!(u32),
                SaturatingCarrier::U64 => reference!(u64),
            }
        }

        #[test]
        fn saturating_decoded_arithmetic_clamps_every_carrier_edge() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            for operation in SATURATING_OPERATIONS {
                for carrier in SaturatingCarrier::ALL {
                    let minimum = i128::from(carrier.minimum_bits() as i64);
                    let maximum = i128::from(carrier.maximum_bits());
                    let in_range = |value: i128| (minimum..=maximum).contains(&value);
                    let inputs = [
                        (maximum, 1),
                        (minimum, -1),
                        (maximum, maximum),
                        (minimum, minimum),
                        (minimum, 1),
                        (maximum, -1),
                        (minimum, maximum),
                        (maximum, minimum),
                        (0, 1),
                        (0, -1),
                        (1, 2),
                        (2, 7),
                        (7, 2),
                        (-7, 2),
                        (7, -2),
                        (maximum, 0),
                        (maximum, 2),
                        (minimum, 2),
                        (1, maximum),
                        (maximum / 2, maximum / 2 + 1),
                    ];
                    let mut checked = 0;
                    for high in [false, true] {
                        let names = saturating_operand_names(operation, carrier, high);
                        let operands = saturating_operands(&physical, &names);
                        let encoded = encode_x86_64_selected_form(
                            &physical,
                            saturating_kind(operation, carrier),
                            saturating_key(operation, carrier),
                            &operands,
                        )
                        .unwrap();
                        let registers =
                            crate::selected_form_encoding::request_validation::resolve_registers(
                                &physical, &operands,
                            )
                            .unwrap();
                        let (left_home, right_home, result_home) = (
                            usize::from(registers[0]),
                            usize::from(registers[1]),
                            usize::from(registers[2]),
                        );
                        for (left, right) in inputs {
                            if !in_range(left)
                                || !in_range(right)
                                || (matches!(
                                    operation,
                                    SaturatingOperation::Divide | SaturatingOperation::Remainder
                                ) && right == 0)
                            {
                                continue;
                            }
                            let mut file = [0_u64; 16];
                            file[left_home] = left as u64;
                            file[right_home] = right as u64;
                            let file = execute_saturating(encoded.bytes(), file);
                            assert_eq!(
                                file[result_home],
                                saturating_reference(operation, carrier, left, right),
                                "{operation:?} {carrier:?} high={high} {left} {right}"
                            );
                            assert_eq!(file[right_home], right as u64, "{operation:?} {carrier:?}");
                            checked += 1;
                        }
                    }
                    assert!(
                        checked >= 16,
                        "{operation:?} {carrier:?} checked only {checked} pairs"
                    );
                }
            }
        }

        #[test]
        fn saturating_i64_divide_guard_only_rewrites_the_faulting_dividend() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let operands = saturating_operands(&physical, &["rax", "rsi", "rax", "rdx"]);
            let encoded = encode_x86_64_selected_form(
                &physical,
                saturating_kind(SaturatingOperation::Divide, SaturatingCarrier::I64),
                saturating_key(SaturatingOperation::Divide, SaturatingCarrier::I64),
                &operands,
            )
            .unwrap();
            for (dividend, divisor, quotient, remainder) in [
                (i64::MIN, -1, i64::MAX, 0),
                (i64::MIN, 1, i64::MIN, 0),
                (i64::MIN, 2, i64::MIN / 2, 0),
                (i64::MAX, -1, -i64::MAX, 0),
                (i64::MIN + 1, -1, i64::MAX, 0),
                (-1, -1, 1, 0),
                (7, -2, -3, 1),
                (-7, 2, -3, -1),
            ] {
                let mut file = [0_u64; 16];
                file[0] = dividend as u64;
                file[6] = divisor as u64;
                let file = execute_saturating(encoded.bytes(), file);
                assert_eq!(file[0] as i64, quotient, "{dividend} / {divisor}");
                assert_eq!(file[2] as i64, remainder, "{dividend} % {divisor}");
            }
        }
    }
    pub(crate) fn assert_early_clobber_required(
        catalog: &register_model::RegisterConstraintCatalog,
        physical: &ValidatedPhysicalRegisterModel,
        key: register_model::RegisterConstraintKey,
        operands: &[usize],
    ) {
        for &operand in operands {
            let mut changed = catalog.clone();
            register_model::find_register_constraint_mut(&mut changed.constraints, key)
                .unwrap()
                .operands[operand]
                .early_clobber = false;
            assert!(crate::validate_x86_64_register_constraint_catalog(changed, physical).is_err());
        }
    }

    mod scalar {
        use super::*;
        use crate::selected_form_encoding::{
            MachineAlternativeFamily as Family, SelectedInstructionKind as Kind,
        };

        #[test]
        fn scalar_call_is_explicitly_refused_before_encoding() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            assert_eq!(
                encode_x86_64_selected_form(
                    &physical,
                    Kind::CallScalar {
                        target: selected_instructions::SelectedCallTarget::Machine(
                            MachineId::new(1).unwrap()
                        ),
                    },
                    alternative(Family::ReturnUnit, 0),
                    &[],
                ),
                Err(X86_64SelectedFormEncodingError::LayoutDependentForm),
            );
        }

        fn for_zero_extend(
            kind: Kind,
            family: Family,
            byte_length: usize,
            mut inspect: impl FnMut(&ValidatedPhysicalRegisterModel, &[RegisterViewId], &[u8]),
        ) {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            for source in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
                for destination in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
                    let operands = [
                        physical.model().view_named(source).unwrap().id,
                        physical.model().view_named(destination).unwrap().id,
                    ];
                    let key = alternative(family, 0);
                    let encoded =
                        encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                    assert_eq!(encoded.bytes().len(), byte_length);
                    inspect(&physical, &operands, encoded.bytes());
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &operands,
                        encoded.bytes(),
                    )
                    .unwrap();
                    let copy = encode_x86_64_selected_form(
                        &physical,
                        Kind::CopyI64,
                        alternative(Family::CopyI64, 0),
                        &operands,
                    )
                    .unwrap();
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            key,
                            &operands,
                            copy.bytes(),
                        )
                        .is_err()
                    );
                    assert_every_byte_change_rejected(
                        &physical,
                        kind,
                        key,
                        &operands,
                        encoded.bytes(),
                        "changed",
                    );
                }
            }
        }

        #[test]
        fn zero_extend_u8_binds_width_registers_and_is_not_a_copy() {
            for_zero_extend(Kind::ZeroExtendU8, Family::ZeroExtendU8, 4, |_, _, _| {});
        }

        #[test]
        fn zero_extend_u32_binds_width_registers_and_is_not_a_copy() {
            let kind = Kind::ZeroExtendU32;
            let key = alternative(Family::ZeroExtendU32, 0);
            for_zero_extend(
                kind,
                Family::ZeroExtendU32,
                3,
                |physical, operands, bytes| {
                    assert_eq!(bytes[0] & 0xf8, 0x40);
                    assert_eq!(bytes[1], 0x89);
                    let narrow = encode_x86_64_selected_form(
                        physical,
                        Kind::ZeroExtendU8,
                        alternative(Family::ZeroExtendU8, 0),
                        operands,
                    )
                    .unwrap();
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            physical,
                            kind,
                            key,
                            operands,
                            narrow.bytes(),
                        )
                        .is_err()
                    );
                },
            );
        }

        #[test]
        fn r12_is_a_valid_rex_extended_sib_index() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let r12 = physical.model().view_named("r12").unwrap().id;
            let rax = physical.model().view_named("rax").unwrap().id;
            let encoded = encode_x86_64_selected_form(
                &physical,
                exact_add(),
                alternative(Family::ExactAddI64, 0),
                &[r12, r12, rax],
            )
            .unwrap();
            assert_eq!(encoded.bytes(), [0x4b, 0x8d, 0x04, 0x24]);
        }

        #[test]
        fn compare_i64_is_canonical_register_cmp_with_exact_footprint() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let rax = physical.model().view_named("rax").unwrap().id;
            let rbx = physical.model().view_named("rbx").unwrap().id;
            let r8 = physical.model().view_named("r8").unwrap().id;
            let r9 = physical.model().view_named("r9").unwrap().id;
            let alternative = alternative(Family::CompareI64, 0);
            for (operands, expected) in [
                ([rax, rbx], [0x48, 0x39, 0xd8]),
                ([r8, r9], [0x4d, 0x39, 0xc8]),
                ([rax, rax], [0x48, 0x39, 0xc0]),
            ] {
                let encoded = encode_x86_64_selected_form(
                    &physical,
                    Kind::CompareI64,
                    alternative,
                    &operands,
                )
                .unwrap();
                assert_eq!(encoded.bytes(), expected);
                assert_eq!(encoded.footprint().register_reads, operands);
                assert!(encoded.footprint().register_writes.is_empty());
                assert!(encoded.footprint().writes_rflags);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
            }
        }

        #[test]
        fn compare_i64_immediate_is_canonical_cmp_imm32_with_exact_footprint() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let rax = physical.model().view_named("rax").unwrap().id;
            let r12 = physical.model().view_named("r12").unwrap().id;
            let alternative = alternative(Family::CompareI64Immediate, 0);
            for (operand, immediate, expected) in [
                (rax, 0, [0x48, 0x81, 0xf8, 0x00, 0x00, 0x00, 0x00]),
                (rax, 4095, [0x48, 0x81, 0xf8, 0xff, 0x0f, 0x00, 0x00]),
                (r12, 4095, [0x49, 0x81, 0xfc, 0xff, 0x0f, 0x00, 0x00]),
            ] {
                let kind = Kind::CompareI64Immediate {
                    immediate: IntegerValue::Unsigned(immediate),
                };
                let encoded =
                    encode_x86_64_selected_form(&physical, kind, alternative, &[operand]).unwrap();
                assert_eq!(encoded.bytes(), expected);
                assert_eq!(encoded.footprint().register_reads, [operand]);
                assert!(encoded.footprint().register_writes.is_empty());
                assert!(encoded.footprint().writes_rflags);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, [0]);
                assert!(
                    encoded
                        .footprint()
                        .encoded
                        .external_operand_writes
                        .is_empty()
                );
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative,
                        &[operand],
                        encoded.bytes()
                    )
                    .is_ok()
                );
                assert_every_byte_change_rejected(
                    &physical,
                    kind,
                    alternative,
                    &[operand],
                    encoded.bytes(),
                    "corrupted",
                );
            }
            for immediate in [IntegerValue::Unsigned(4096), IntegerValue::Signed(-1)] {
                let kind = Kind::CompareI64Immediate { immediate };
                assert!(encode_x86_64_selected_form(&physical, kind, alternative, &[rax]).is_err());
            }
        }

        #[test]
        fn scalar_sizes_and_subtraction_alias_partitions_are_exact() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let views =
                ["rax", "rbx", "rcx"].map(|name| physical.model().view_named(name).unwrap().id);
            let materialize = encode_x86_64_selected_form(
                &physical,
                Kind::MaterializeI64 {
                    value: IntegerValue::Signed(-1),
                },
                alternative(Family::MaterializeI64, 0),
                &views[..1],
            )
            .unwrap();
            assert_eq!(materialize.bytes().len(), 10);
            for (homes, variant, size) in [
                ([views[0], views[0], views[0]], 0, 3),
                ([views[0], views[1], views[0]], 1, 3),
                ([views[0], views[1], views[1]], 2, 6),
                ([views[0], views[1], views[2]], 3, 6),
            ] {
                let kind = Kind::ExactSubtractI64 {
                    obligation: ObligationId::new(2).unwrap(),
                    accepted_fact: AcceptedObligationFactIdentity::from_bytes([4; 32]),
                };
                let encoded = encode_x86_64_selected_form(
                    &physical,
                    kind,
                    alternative(Family::ExactSubtractI64, variant),
                    &homes,
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), size);
                let mut corrupted = encoded.bytes().to_vec();
                corrupted[1] ^= 1;
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative(Family::ExactSubtractI64, variant),
                        &homes,
                        &corrupted,
                    )
                    .is_err()
                );
            }
        }

        #[test]
        fn exact_multiply_alias_partitions_encode_and_decode() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let views =
                ["rax", "rbx", "rcx"].map(|name| physical.model().view_named(name).unwrap().id);
            let kind = Kind::ExactMultiplyI64 {
                obligation: ObligationId::new(2).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([4; 32]),
            };
            for (homes, variant, size) in [
                ([views[0], views[0], views[0]], 0, 4),
                ([views[0], views[1], views[0]], 1, 4),
                ([views[0], views[1], views[1]], 2, 4),
                ([views[0], views[1], views[2]], 3, 7),
            ] {
                let key = alternative(Family::ExactMultiplyI64, variant);
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &homes).unwrap();
                assert_eq!(encoded.bytes().len(), size);
                let decoded = validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &homes,
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
                assert!(
                    !decoded
                        .footprint()
                        .encoded
                        .implicit_unit_clobbers
                        .is_empty(),
                    "imul clobbers condition flags"
                );
                assert_every_byte_change_rejected(
                    &physical,
                    kind,
                    key,
                    &homes,
                    encoded.bytes(),
                    "changed",
                );
            }
            let homes = [views[0], views[1], views[2]];
            let encoded = encode_x86_64_selected_form(
                &physical,
                kind,
                alternative(Family::ExactMultiplyI64, 3),
                &homes,
            )
            .unwrap();
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative(Family::ExactMultiplyI64, 0),
                    &homes,
                    encoded.bytes(),
                )
                .is_err()
            );
        }

        #[test]
        fn wrapping_add_preserves_aliases_and_rejects_exact_family_or_changed_bytes() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let views =
                ["rax", "r9", "r12"].map(|name| physical.model().view_named(name).unwrap().id);
            let kind = Kind::WrappingAddI64;
            let key = alternative(Family::WrappingAddI64, 0);
            for left in views {
                for right in views {
                    for output in views {
                        let operands = [left, right, output];
                        let encoded =
                            encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            key,
                            &operands,
                            encoded.bytes(),
                        )
                        .unwrap();
                        assert!(!encoded.footprint().writes_rflags);
                        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                        assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                        let mut changed = encoded.bytes().to_vec();
                        changed[1] ^= 1;
                        assert!(
                            validate_x86_64_selected_form_encoding(
                                &physical, kind, key, &operands, &changed
                            )
                            .is_err()
                        );
                        assert!(
                            encode_x86_64_selected_form(
                                &physical,
                                kind,
                                alternative(Family::ExactAddI64, 0),
                                &operands
                            )
                            .is_err()
                        );
                        assert!(
                            validate_x86_64_selected_form_encoding(
                                &physical,
                                kind,
                                alternative(Family::ExactAddI64, 0),
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
        fn bitwise_operations_preserve_aliases_and_reject_opcode_corruption() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let views =
                ["rax", "r9", "r12"].map(|name| physical.model().view_named(name).unwrap().id);
            for (kind, family, expected_opcode) in [
                (Kind::BitwiseAndI64, Family::BitwiseAndI64, 0x21),
                (Kind::BitwiseXorI64, Family::BitwiseXorI64, 0x31),
            ] {
                let alternative = alternative(family, 0);
                for left in views {
                    for right in views {
                        for output in views {
                            let operands = [left, right, output];
                            let encoded = encode_x86_64_selected_form(
                                &physical,
                                kind,
                                alternative,
                                &operands,
                            )
                            .unwrap();
                            assert_eq!(
                                encoded.bytes().len(),
                                if output == left || output == right {
                                    3
                                } else {
                                    6
                                }
                            );
                            assert!(encoded.footprint().writes_rflags);
                            assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                            assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                            let mut corrupted = encoded.bytes().to_vec();
                            let opcode = corrupted.len() - 2;
                            assert_eq!(corrupted[opcode], expected_opcode);
                            corrupted[opcode] = if expected_opcode == 0x21 { 0x31 } else { 0x21 };
                            assert!(
                                validate_x86_64_selected_form_encoding(
                                    &physical,
                                    kind,
                                    alternative,
                                    &operands,
                                    &corrupted
                                )
                                .is_err()
                            );
                            corrupted = encoded.bytes().to_vec();
                            let register_byte = corrupted.len() - 1;
                            corrupted[register_byte] ^= 1;
                            assert!(
                                validate_x86_64_selected_form_encoding(
                                    &physical,
                                    kind,
                                    alternative,
                                    &operands,
                                    &corrupted
                                )
                                .is_err()
                            );
                        }
                    }
                }
            }
        }

        #[test]
        fn immediate_lea_uses_declared_size_extremes() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let rax = physical.model().view_named("rax").unwrap().id;
            let r12 = physical.model().view_named("r12").unwrap().id;
            let fact = AcceptedObligationFactIdentity::from_bytes([5; 32]);
            for (base, immediate, size) in
                [(rax, 0, 4), (rax, 4095, 7), (r12, 0, 5), (r12, 4095, 8)]
            {
                let kind = Kind::ExactAddI64Immediate {
                    immediate: IntegerValue::Unsigned(immediate),
                    obligation: ObligationId::new(3).unwrap(),
                    accepted_fact: fact,
                };
                let encoded = encode_x86_64_selected_form(
                    &physical,
                    kind,
                    alternative(Family::ExactAddI64Immediate, 0),
                    &[base, rax],
                )
                .unwrap();
                assert_eq!(encoded.bytes().len(), size);
            }
        }

        #[test]
        fn subtract_immediate_lea_is_negative_canonical_and_flag_transparent() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let rax = physical.model().view_named("rax").unwrap().id;
            let r12 = physical.model().view_named("r12").unwrap().id;
            let fact = AcceptedObligationFactIdentity::from_bytes([6; 32]);
            for (base, immediate, expected) in [
                (rax, 0, vec![0x48, 0x8d, 0x40, 0x00]),
                (rax, 128, vec![0x48, 0x8d, 0x40, 0x80]),
                (rax, 129, vec![0x48, 0x8d, 0x80, 0x7f, 0xff, 0xff, 0xff]),
                (
                    r12,
                    4095,
                    vec![0x49, 0x8d, 0x84, 0x24, 0x01, 0xf0, 0xff, 0xff],
                ),
            ] {
                let kind = Kind::ExactSubtractI64Immediate {
                    immediate: IntegerValue::Unsigned(immediate),
                    obligation: ObligationId::new(4).unwrap(),
                    accepted_fact: fact,
                };
                let alternative = alternative(Family::ExactSubtractI64Immediate, 0);
                let encoded =
                    encode_x86_64_selected_form(&physical, kind, alternative, &[base, rax])
                        .unwrap();
                assert_eq!(encoded.bytes(), expected);
                assert!(!encoded.footprint().writes_rflags);
                assert!(
                    encoded
                        .footprint()
                        .encoded
                        .implicit_unit_clobbers
                        .is_empty()
                );
                let mut wrong_sign = expected;
                *wrong_sign.last_mut().unwrap() ^= 0x80;
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative,
                        &[base, rax],
                        &wrong_sign,
                    )
                    .is_err()
                );
            }
        }

        #[test]
        fn materialize_i64_variant_one_is_the_optimizer_selected_xor_zero_idiom() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let zero = Kind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            };
            let xor_zero = alternative(Family::MaterializeI64, 1);
            for (name, bytes) in [
                ("rax", [0x48, 0x31, 0xc0]),
                ("rdi", [0x48, 0x31, 0xff]),
                ("r8", [0x4d, 0x31, 0xc0]),
                ("r15", [0x4d, 0x31, 0xff]),
            ] {
                let destination = [physical.model().view_named(name).unwrap().id];
                let encoded =
                    encode_x86_64_selected_form(&physical, zero, xor_zero, &destination).unwrap();
                assert_eq!(encoded.bytes(), bytes);
                assert!(encoded.footprint().writes_rflags);
                assert_eq!(
                    encoded.footprint().encoded.implicit_unit_clobbers,
                    physical
                        .model()
                        .view_named("condition_flags")
                        .unwrap()
                        .units
                );
                assert_eq!(encoded.footprint().register_writes, destination);
                assert!(encoded.footprint().register_reads.is_empty());
                validate_x86_64_selected_form_encoding(
                    &physical,
                    zero,
                    xor_zero,
                    &destination,
                    &bytes,
                )
                .unwrap();
                let baseline = alternative(Family::MaterializeI64, 0);
                let moved =
                    encode_x86_64_selected_form(&physical, zero, baseline, &destination).unwrap();
                assert_eq!(moved.bytes().len(), 10);
                assert!(!moved.footprint().writes_rflags);
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        zero,
                        baseline,
                        &destination,
                        &bytes
                    )
                    .is_err()
                );
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        zero,
                        xor_zero,
                        &destination,
                        moved.bytes()
                    )
                    .is_err()
                );
                crate::selected_form_encoding::assert_byte_fragility(&bytes, |candidate| {
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        zero,
                        xor_zero,
                        &destination,
                        candidate,
                    )
                    .is_err()
                });
            }
            let destination = [physical.model().view_named("rax").unwrap().id];
            for value in [IntegerValue::Signed(-1), IntegerValue::Unsigned(1)] {
                assert_eq!(
                    encode_x86_64_selected_form(
                        &physical,
                        Kind::MaterializeI64 { value },
                        xor_zero,
                        &destination,
                    ),
                    Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
                );
            }
            let constraints = crate::validate_x86_64_register_constraint_catalog(
                crate::x86_64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap();
            let catalog = crate::x86_64_machine_effect_catalog(
                target::NativeTarget::linux_x64(),
                &constraints,
            )
            .unwrap();
            let declaration = catalog
                .declarations
                .iter()
                .find(|declaration| {
                    declaration.semantic
                        == selected_instructions::MachineSemanticKind::MaterializeI64
                })
                .unwrap();
            assert_eq!(declaration.alternatives.len(), 2);
            assert_eq!(
                declaration.alternatives[1].applicability,
                selected_instructions::MachineAlternativeApplicability::OptimizationSelected {
                    optimization:
                        optimization_core::Optimization::X86SelectXorZeroI64MaterializationV1,
                }
            );
            assert_eq!(
                declaration.alternatives[1].size,
                selected_instructions::MachineSizeKnowledge::ExactBytes(3)
            );
            assert_eq!(
                declaration.alternatives[1].encoded.implicit_unit_clobbers,
                physical
                    .model()
                    .view_named("condition_flags")
                    .unwrap()
                    .units
            );
        }
    }
    mod trapping {
        use super::*;

        macro_rules! row {
            ($a:expr => $b:expr => $c:expr => $d:expr) => {
                ($a, $b, $c, $d)
            };
        }

        const CLANG_TRAPPING_LOW: [&str; 72] = [
            "4889f84801f0480fbec84839c174020f0b",
            "4889f84801f0480fbfc84839c174020f0b",
            "4889f84801f04863c84839c174020f0b",
            "4889f84801f071020f0b",
            "4889f84801f04889c148c1e90874020f0b",
            "4889f84801f04889c148c1e91074020f0b",
            "4889f84801f04889c148c1e92074020f0b",
            "4889f84801f073020f0b",
            "4889f84829f0480fbec84839c174020f0b",
            "4889f84829f0480fbfc84839c174020f0b",
            "4889f84829f04863c84839c174020f0b",
            "4889f84829f071020f0b",
            "4889f84829f04889c148c1e90874020f0b",
            "4889f84829f04889c148c1e91074020f0b",
            "4889f84829f04889c148c1e92074020f0b",
            "4889f84829f073020f0b",
            "4889f8480fafc6480fbec84839c174020f0b",
            "4889f8480fafc6480fbfc84839c174020f0b",
            "4889f8480fafc64863c84839c174020f0b",
            "4889f8480fafc671020f0b",
            "4889f8480fafc64889c148c1e90874020f0b",
            "4889f8480fafc64889c148c1e91074020f0b",
            "4889f8480fafc64889c148c1e92074020f0b",
            "48f7e171020f0b",
            "4885c975020f0b4883f9ff75084883f88075020f0b489948f7f9",
            "4885c975020f0b4883f9ff750a483d0080ffff75020f0b489948f7f9",
            "4885c975020f0b4883f9ff750a483d0000008075020f0b489948f7f9",
            "4885c975020f0b4883f9ff75084883f80171020f0b489948f7f9",
            "4885c975020f0b4831d248f7f1",
            "4885c975020f0b4831d248f7f1",
            "4885c975020f0b4831d248f7f1",
            "4885c975020f0b4831d248f7f1",
            "4885c975020f0b4883f9ff75084883f88075020f0b489948f7f94889d0",
            "4885c975020f0b4883f9ff750a483d0080ffff75020f0b489948f7f94889d0",
            "4885c975020f0b4883f9ff750a483d0000008075020f0b489948f7f94889d0",
            "4885c975020f0b4883f9ff75084883f80171020f0b489948f7f94889d0",
            "4885c975020f0b4831d248f7f14889d0",
            "4885c975020f0b4831d248f7f14889d0",
            "4885c975020f0b4831d248f7f14889d0",
            "4885c975020f0b4831d248f7f14889d0",
            "4883f90872020f0b4889f848d3e0480fbed04839c274020f0b",
            "4883f91072020f0b4889f848d3e0480fbfd04839c274020f0b",
            "4883f92072020f0b4889f848d3e04863d04839c274020f0b",
            "4883f94072020f0b4889f848d3e04889c248d3fa4839fa74020f0b",
            "4883f90872020f0b4889f848d3e04889c248c1ea0874020f0b",
            "4883f91072020f0b4889f848d3e04889c248c1ea1074020f0b",
            "4883f92072020f0b4889f848d3e04889c248c1ea2074020f0b",
            "4883f94072020f0b4889f848d3e04889c248d3ea4839fa74020f0b",
            "4883f90872020f0b4889f848d3f8",
            "4883f91072020f0b4889f848d3f8",
            "4883f92072020f0b4889f848d3f8",
            "4883f94072020f0b4889f848d3f8",
            "4883f90872020f0b4889f848d3e8",
            "4883f91072020f0b4889f848d3e8",
            "4883f92072020f0b4889f848d3e8",
            "4883f94072020f0b4889f848d3e8",
            "480fbecf4839f974020f0b4889f8",
            "480fbfcf4839f974020f0b4889f8",
            "4863cf4839f974020f0b4889f8",
            "4889f8",
            "4889f948c1e90874020f0b4889f8",
            "4889f948c1e91074020f0b4889f8",
            "4889f948c1e92074020f0b4889f8",
            "4889f948c1e93f74020f0b4889f8",
            "4889f948c1e90774020f0b4889f8",
            "4889f948c1e90f74020f0b4889f8",
            "4889f948c1e91f74020f0b4889f8",
            "4889f948c1e93f74020f0b4889f8",
            "4889f948c1e90874020f0b4889f8",
            "4889f948c1e91074020f0b4889f8",
            "4889f948c1e92074020f0b4889f8",
            "4889f8",
        ];

        const CLANG_TRAPPING_HIGH: [&str; 72] = [
            "4d89c24d01da4d0fbeca4d39d174020f0b",
            "4d89c24d01da4d0fbfca4d39d174020f0b",
            "4d89c24d01da4d63ca4d39d174020f0b",
            "4d89c24d01da71020f0b",
            "4d89c24d01da4d89d149c1e90874020f0b",
            "4d89c24d01da4d89d149c1e91074020f0b",
            "4d89c24d01da4d89d149c1e92074020f0b",
            "4d89c24d01da73020f0b",
            "4d89c24d29da4d0fbeca4d39d174020f0b",
            "4d89c24d29da4d0fbfca4d39d174020f0b",
            "4d89c24d29da4d63ca4d39d174020f0b",
            "4d89c24d29da71020f0b",
            "4d89c24d29da4d89d149c1e90874020f0b",
            "4d89c24d29da4d89d149c1e91074020f0b",
            "4d89c24d29da4d89d149c1e92074020f0b",
            "4d89c24d29da73020f0b",
            "4d89c24d0fafd34d0fbeca4d39d174020f0b",
            "4d89c24d0fafd34d0fbfca4d39d174020f0b",
            "4d89c24d0fafd34d63ca4d39d174020f0b",
            "4d89c24d0fafd371020f0b",
            "4d89c24d0fafd34d89d149c1e90874020f0b",
            "4d89c24d0fafd34d89d149c1e91074020f0b",
            "4d89c24d0fafd34d89d149c1e92074020f0b",
            "49f7e171020f0b",
            "4d85c975020f0b4983f9ff75084883f88075020f0b489949f7f9",
            "4d85c975020f0b4983f9ff750a483d0080ffff75020f0b489949f7f9",
            "4d85c975020f0b4983f9ff750a483d0000008075020f0b489949f7f9",
            "4d85c975020f0b4983f9ff75084883f80171020f0b489949f7f9",
            "4d85c975020f0b4831d249f7f1",
            "4d85c975020f0b4831d249f7f1",
            "4d85c975020f0b4831d249f7f1",
            "4d85c975020f0b4831d249f7f1",
            "4d85c975020f0b4983f9ff75084883f88075020f0b489949f7f94889d0",
            "4d85c975020f0b4983f9ff750a483d0080ffff75020f0b489949f7f94889d0",
            "4d85c975020f0b4983f9ff750a483d0000008075020f0b489949f7f94889d0",
            "4d85c975020f0b4983f9ff75084883f80171020f0b489949f7f94889d0",
            "4d85c975020f0b4831d249f7f14889d0",
            "4d85c975020f0b4831d249f7f14889d0",
            "4d85c975020f0b4831d249f7f14889d0",
            "4d85c975020f0b4831d249f7f14889d0",
            "4883f90872020f0b4d89c249d3e24d0fbeca4d39d174020f0b",
            "4883f91072020f0b4d89c249d3e24d0fbfca4d39d174020f0b",
            "4883f92072020f0b4d89c249d3e24d63ca4d39d174020f0b",
            "4883f94072020f0b4d89c249d3e24d89d149d3f94d39c174020f0b",
            "4883f90872020f0b4d89c249d3e24d89d149c1e90874020f0b",
            "4883f91072020f0b4d89c249d3e24d89d149c1e91074020f0b",
            "4883f92072020f0b4d89c249d3e24d89d149c1e92074020f0b",
            "4883f94072020f0b4d89c249d3e24d89d149d3e94d39c174020f0b",
            "4883f90872020f0b4d89c249d3fa",
            "4883f91072020f0b4d89c249d3fa",
            "4883f92072020f0b4d89c249d3fa",
            "4883f94072020f0b4d89c249d3fa",
            "4883f90872020f0b4d89c249d3ea",
            "4883f91072020f0b4d89c249d3ea",
            "4883f92072020f0b4d89c249d3ea",
            "4883f94072020f0b4d89c249d3ea",
            "4d0fbec84d39c174020f0b4d89c2",
            "4d0fbfc84d39c174020f0b4d89c2",
            "4d63c84d39c174020f0b4d89c2",
            "4d89c2",
            "4d89c149c1e90874020f0b4d89c2",
            "4d89c149c1e91074020f0b4d89c2",
            "4d89c149c1e92074020f0b4d89c2",
            "4d89c149c1e93f74020f0b4d89c2",
            "4d89c149c1e90774020f0b4d89c2",
            "4d89c149c1e90f74020f0b4d89c2",
            "4d89c149c1e91f74020f0b4d89c2",
            "4d89c149c1e93f74020f0b4d89c2",
            "4d89c149c1e90874020f0b4d89c2",
            "4d89c149c1e91074020f0b4d89c2",
            "4d89c149c1e92074020f0b4d89c2",
            "4d89c2",
        ];

        fn trapping_kind(form: TrappingForm) -> Kind {
            Kind::TrappingInteger { form }
        }

        fn trapping_key(form: TrappingForm) -> MachineAlternativeKey {
            alternative(Family::TrappingInteger(form), 0)
        }

        fn trapping_operand_names(form: TrappingForm, high: bool) -> Vec<&'static str> {
            match (TrappingShape::of(form).row(), high) {
                (TrappingRow::FixedPair, _) => {
                    vec!["rax", if high { "r9" } else { "rcx" }, "rax", "rdx"]
                }
                (TrappingRow::Binary, false) => vec!["rdi", "rsi", "rax", "rcx"],
                (TrappingRow::Binary, true) => vec!["r8", "r11", "r10", "r9"],
                (TrappingRow::Shift, false) => vec!["rdi", "rcx", "rax", "rdx"],
                (TrappingRow::Shift, true) => vec!["r8", "rcx", "r10", "r9"],
                (TrappingRow::Convert, false) => vec!["rdi", "rax", "rcx"],
                (TrappingRow::Convert, true) => vec!["r8", "r10", "r9"],
            }
        }

        fn hex_bytes(hex: &str) -> Vec<u8> {
            (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect()
        }

        fn trapping_scratch_unused(form: TrappingForm) -> bool {
            matches!(
                TrappingShape::of(form),
                TrappingShape::OverflowFlag(_)
                    | TrappingShape::CarryFlag(_)
                    | TrappingShape::ShiftRight { .. }
                    | TrappingShape::Convert(None)
            )
        }

        #[test]
        fn trapping_forms_match_independent_assembler_for_every_form() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let constraints = crate::validate_x86_64_register_constraint_catalog(
                crate::x86_64_register_constraint_catalog(&physical),
                &physical,
            )
            .unwrap();
            let catalog = crate::x86_64_machine_effect_catalog(
                target::NativeTarget::linux_x64(),
                &constraints,
            )
            .unwrap();
            let condition_flags = physical
                .model()
                .view_named("condition_flags")
                .unwrap()
                .units
                .clone();
            for (index, form) in TrappingForm::ALL.into_iter().enumerate() {
                let shape = TrappingShape::of(form);
                let declared = &catalog
                    .declaration_by_semantic(MachineSemanticKind::TrappingInteger(form))
                    .unwrap()
                    .alternatives[0];
                for (high, table) in [(false, &CLANG_TRAPPING_LOW), (true, &CLANG_TRAPPING_HIGH)] {
                    let names = trapping_operand_names(form, high);
                    let operands = saturating_operands(&physical, &names);
                    let expected = hex_bytes(table[index]);
                    let encoded = encode_x86_64_selected_form(
                        &physical,
                        trapping_kind(form),
                        trapping_key(form),
                        &operands,
                    )
                    .unwrap();
                    assert_eq!(encoded.bytes(), expected, "{form:?} {names:?}");
                    assert_eq!(expected.len(), usize::from(shape.byte_count()), "{form:?}");
                    assert_eq!(
                        declared.size,
                        selected_instructions::MachineSizeKnowledge::ExactBytes(shape.byte_count())
                    );
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        trapping_kind(form),
                        trapping_key(form),
                        &operands,
                        &expected,
                    )
                    .unwrap();
                    let footprint = encoded.footprint();
                    let (reads, writes) = shape.operand_reads_and_writes();
                    assert_eq!(
                        footprint.register_reads,
                        reads
                            .iter()
                            .map(|&position| operands[usize::from(position)])
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(
                        footprint.register_writes,
                        writes
                            .iter()
                            .map(|&position| operands[usize::from(position)])
                            .collect::<Vec<_>>()
                    );
                    assert!(footprint.writes_rflags);
                    assert_eq!(footprint.encoded, declared.encoded, "{form:?}");
                    assert_eq!(footprint.encoded.implicit_unit_clobbers, condition_flags);
                    assert_eq!(
                        footprint.encoded.trap,
                        MachineEncodedTrapBehavior::TrappingIntegerV1
                    );
                    assert_eq!(
                        footprint.encoded.control,
                        MachineEncodedControlEffect::FallThroughOrTrapV1
                    );
                    assert_eq!(footprint.encoded.memory, MachineEncodedMemoryEffect::NoneV1);
                    assert_eq!(
                        footprint.encoded.stack,
                        MachineEncodedStackEffect::UnchangedV1
                    );
                    let ud2 = expected.windows(2).any(|pair| *pair == [0x0f, 0x0b]);
                    assert_eq!(ud2, shape != TrappingShape::Convert(None), "{form:?}");
                }
            }
        }

        #[test]
        fn trapping_forms_reject_every_mutation_substitution_and_wrong_request() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let view = |name: &str| physical.model().view_named(name).unwrap().id;
            let substitute = view("r14");
            for form in TrappingForm::ALL {
                let kind = trapping_kind(form);
                let key = trapping_key(form);
                for high in [false, true] {
                    let operands =
                        saturating_operands(&physical, &trapping_operand_names(form, high));
                    let encoded =
                        encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                    assert_every_bit_change_rejected(
                        &physical,
                        kind,
                        key,
                        &operands,
                        encoded.bytes(),
                        &format!("{form:?} high={high}"),
                    );
                    let mut truncated = encoded.bytes().to_vec();
                    truncated.pop();
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical, kind, key, &operands, &truncated
                        )
                        .is_err()
                    );
                    let scratch = operands.len() - 1;
                    for operand_position in 0..operands.len() {
                        let mut changed = operands.clone();
                        changed[operand_position] = substitute;
                        assert_eq!(
                            validate_x86_64_selected_form_encoding(
                                &physical,
                                kind,
                                key,
                                &changed,
                                encoded.bytes()
                            )
                            .is_ok(),
                            operand_position == scratch && trapping_scratch_unused(form),
                            "{form:?} high={high} substituted operand {operand_position}"
                        );
                    }
                }
                let operands = saturating_operands(&physical, &trapping_operand_names(form, false));
                let bytes = encode_x86_64_selected_form(&physical, kind, key, &operands)
                    .unwrap()
                    .bytes()
                    .to_vec();
                let sibling =
                    TrappingForm::ALL[(usize::from(form.ordinal()) + 1) % TrappingForm::COUNT];
                for wrong in [
                    alternative(Family::TrappingInteger(form), 1),
                    trapping_key(sibling),
                    saturating_key(SaturatingOperation::Add, form.carrier),
                ] {
                    assert_eq!(
                        encode_x86_64_selected_form(&physical, kind, wrong, &operands),
                        Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
                        "{form:?} {wrong:?}"
                    );
                    assert_eq!(
                        validate_x86_64_selected_form_encoding(
                            &physical, kind, wrong, &operands, &bytes
                        ),
                        Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
                    );
                }
                let mut too_few = operands.clone();
                too_few.pop();
                let mut too_many = operands.clone();
                too_many.push(view("r12"));
                for wrong_count in [too_few, too_many] {
                    assert_eq!(
                        encode_x86_64_selected_form(&physical, kind, key, &wrong_count),
                        Err(X86_64SelectedFormEncodingError::OperandCountMismatch)
                    );
                    assert_eq!(
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            key,
                            &wrong_count,
                            &bytes
                        ),
                        Err(X86_64SelectedFormEncodingError::OperandCountMismatch)
                    );
                }
            }
        }

        #[test]
        fn trapping_forms_pin_rows_and_keep_early_clobbers_off_inputs() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            for form in TrappingForm::ALL {
                let kind = trapping_kind(form);
                let key = trapping_key(form);
                let bytes = encode_x86_64_selected_form(
                    &physical,
                    kind,
                    key,
                    &saturating_operands(&physical, &trapping_operand_names(form, false)),
                )
                .unwrap()
                .bytes()
                .to_vec();
                let (invalid, valid): (Vec<Vec<&str>>, Vec<&str>) =
                    match TrappingShape::of(form).row() {
                        TrappingRow::Binary => (
                            vec![
                                vec!["r9", "r10", "r9", "r12"],
                                vec!["r9", "r10", "r10", "r12"],
                                vec!["r9", "r10", "r11", "r9"],
                                vec!["r9", "r10", "r11", "r10"],
                                vec!["r9", "r10", "r11", "r11"],
                            ],
                            vec!["r9", "r9", "r10", "r11"],
                        ),
                        TrappingRow::Shift => (
                            vec![
                                vec!["r9", "r10", "r11", "r12"],
                                vec!["r9", "rcx", "r9", "r12"],
                                vec!["r9", "rcx", "rcx", "r12"],
                                vec!["r9", "rcx", "r11", "r9"],
                                vec!["r9", "rcx", "r11", "rcx"],
                                vec!["r9", "rcx", "r11", "r11"],
                            ],
                            vec!["rcx", "rcx", "r10", "r11"],
                        ),
                        TrappingRow::Convert => (
                            vec![
                                vec!["r9", "r9", "r12"],
                                vec!["r9", "r10", "r9"],
                                vec!["r9", "r10", "r10"],
                            ],
                            vec!["r9", "r10", "r11"],
                        ),
                        TrappingRow::FixedPair => (
                            vec![
                                vec!["rax", "rdx", "rax", "rdx"],
                                vec!["rcx", "r9", "rax", "rdx"],
                                vec!["rax", "r9", "rcx", "rdx"],
                                vec!["rax", "r9", "rax", "rcx"],
                            ],
                            vec!["rax", "rax", "rax", "rdx"],
                        ),
                    };
                for names in invalid {
                    let operands = saturating_operands(&physical, &names);
                    assert_eq!(
                        encode_x86_64_selected_form(&physical, kind, key, &operands),
                        Err(X86_64SelectedFormEncodingError::EncodedFormMismatch),
                        "{form:?} {names:?}"
                    );
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical, kind, key, &operands, &bytes
                        )
                        .is_err()
                    );
                }
                let operands = saturating_operands(&physical, &valid);
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                assert_eq!(
                    encoded.bytes().len(),
                    usize::from(TrappingShape::of(form).byte_count())
                );
            }
        }

        #[test]
        fn trapping_bytes_replay_only_under_a_form_of_the_same_shape() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let canonical = TrappingForm::ALL.map(|form| {
                let operands = saturating_operands(&physical, &trapping_operand_names(form, false));
                let bytes = encode_x86_64_selected_form(
                    &physical,
                    trapping_kind(form),
                    trapping_key(form),
                    &operands,
                )
                .unwrap()
                .bytes()
                .to_vec();
                (form, operands, bytes)
            });
            let mut accepted = Vec::new();
            for (form, _, bytes) in &canonical {
                for (other, operands, _) in &canonical {
                    if form != other
                        && validate_x86_64_selected_form_encoding(
                            &physical,
                            trapping_kind(*other),
                            trapping_key(*other),
                            operands,
                            bytes,
                        )
                        .is_ok()
                    {
                        accepted.push((*form, *other));
                    }
                }
            }
            let convert = |source, carrier| TrappingForm {
                operation: TrappingOperation::Convert { source },
                carrier,
            };
            let mut groups: Vec<Vec<TrappingForm>> =
                [TrappingOperation::Divide, TrappingOperation::Remainder]
                    .map(|operation| {
                        [
                            SaturatingCarrier::U8,
                            SaturatingCarrier::U16,
                            SaturatingCarrier::U32,
                            SaturatingCarrier::U64,
                        ]
                        .map(|carrier| TrappingForm { operation, carrier })
                        .to_vec()
                    })
                    .to_vec();
            for carrier in [
                SaturatingCarrier::U8,
                SaturatingCarrier::U16,
                SaturatingCarrier::U32,
            ] {
                groups.push(vec![
                    convert(IntegerSign::Signed, carrier),
                    convert(IntegerSign::Unsigned, carrier),
                ]);
            }
            groups.push(vec![
                convert(IntegerSign::Signed, SaturatingCarrier::U64),
                convert(IntegerSign::Unsigned, SaturatingCarrier::I64),
            ]);
            groups.push(vec![
                convert(IntegerSign::Signed, SaturatingCarrier::I64),
                convert(IntegerSign::Unsigned, SaturatingCarrier::U64),
            ]);
            let mut expected = Vec::new();
            for group in &groups {
                for form in group {
                    for other in group {
                        if form != other {
                            expected.push((*form, *other));
                        }
                    }
                }
            }
            accepted.sort();
            expected.sort();
            assert_eq!(accepted, expected);
            for (form, other) in &accepted {
                assert_eq!(TrappingShape::of(*form), TrappingShape::of(*other));
            }
        }

        fn trapping_reference(form: TrappingForm, left: i128, right: i128) -> Option<u64> {
            let carrier = form.carrier;
            let minimum = i128::from(carrier.minimum_bits() as i64);
            let maximum = i128::from(carrier.maximum_bits());
            let width = i128::from(carrier.bits());
            let signed_overflow = carrier.is_signed() && left == minimum && right == -1;
            let exact = match form.operation {
                TrappingOperation::Add => left + right,
                TrappingOperation::Subtract => left - right,
                TrappingOperation::Multiply => left.checked_mul(right)?,
                TrappingOperation::Divide if right == 0 || signed_overflow => return None,
                TrappingOperation::Divide => left / right,
                TrappingOperation::Remainder if right == 0 || signed_overflow => return None,
                TrappingOperation::Remainder => left % right,
                TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight
                    if !(0..width).contains(&right) =>
                {
                    return None;
                }
                TrappingOperation::ShiftLeft => left * (1_i128 << right),
                TrappingOperation::ShiftRight => left >> right,
                TrappingOperation::Convert { .. } => left,
            };
            (minimum..=maximum).contains(&exact).then_some(exact as u64)
        }

        fn trapping_edge_values(carrier: SaturatingCarrier) -> Vec<i128> {
            let minimum = i128::from(carrier.minimum_bits() as i64);
            let maximum = i128::from(carrier.maximum_bits());
            let mut root = (maximum as f64).sqrt() as i128;
            while root * root > maximum {
                root -= 1;
            }
            while (root + 1) * (root + 1) <= maximum {
                root += 1;
            }
            let mut values = vec![
                minimum,
                minimum + 1,
                minimum / 2,
                minimum / 2 - 1,
                -2,
                -1,
                0,
                1,
                2,
                3,
                root,
                root + 1,
                -root,
                -root - 1,
                maximum / 2,
                maximum / 2 + 1,
                maximum - 1,
                maximum,
            ];
            values.retain(|value| (minimum..=maximum).contains(value));
            values.sort_unstable();
            values.dedup();
            values
        }

        fn trapping_inputs(form: TrappingForm) -> Vec<(i128, i128)> {
            let values = trapping_edge_values(form.carrier);
            match form.operation {
                TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => {
                    let width = i128::from(form.carrier.bits());
                    let counts = [
                        0,
                        1,
                        width - 1,
                        width,
                        width + 1,
                        63,
                        64,
                        -1,
                        i128::from(i64::MIN),
                        i128::from(u64::MAX),
                    ];
                    values
                        .iter()
                        .flat_map(|value| counts.map(|count| (*value, count)))
                        .collect()
                }
                TrappingOperation::Convert { source } => {
                    let sources: Vec<i128> = if source == IntegerSign::Signed {
                        [
                            i128::from(i64::MIN),
                            i128::from(i64::MIN) + 1,
                            i128::from(i32::MIN) - 1,
                            i128::from(i32::MIN),
                            i128::from(i16::MIN) - 1,
                            i128::from(i16::MIN),
                            -129,
                            -128,
                            -1,
                            0,
                            1,
                            127,
                            128,
                            255,
                            256,
                            32767,
                            32768,
                            65535,
                            65536,
                            i128::from(i32::MAX),
                            i128::from(i32::MAX) + 1,
                            i128::from(u32::MAX),
                            i128::from(u32::MAX) + 1,
                            i128::from(i64::MAX),
                        ]
                        .to_vec()
                    } else {
                        [
                            0,
                            1,
                            127,
                            128,
                            255,
                            256,
                            32767,
                            32768,
                            65535,
                            65536,
                            i128::from(i32::MAX),
                            i128::from(i32::MAX) + 1,
                            i128::from(u32::MAX),
                            i128::from(u32::MAX) + 1,
                            i128::from(i64::MAX),
                            i128::from(i64::MAX) + 1,
                            i128::from(u64::MAX),
                        ]
                        .to_vec()
                    };
                    sources.into_iter().map(|source| (source, 0)).collect()
                }
                _ => values
                    .iter()
                    .flat_map(|left| values.iter().map(|right| (*left, *right)))
                    .collect(),
            }
        }

        struct TrappingProgram {
            form: TrappingForm,
            bytes: Vec<u8>,
            homes: Vec<usize>,
        }

        impl TrappingProgram {
            fn encode(
                physical: &register_model::ValidatedPhysicalRegisterModel,
                form: TrappingForm,
                high: bool,
            ) -> Self {
                let operands = saturating_operands(physical, &trapping_operand_names(form, high));
                let bytes = encode_x86_64_selected_form(
                    physical,
                    trapping_kind(form),
                    trapping_key(form),
                    &operands,
                )
                .unwrap()
                .bytes()
                .to_vec();
                let homes = crate::selected_form_encoding::request_validation::resolve_registers(
                    physical, &operands,
                )
                .unwrap()
                .into_iter()
                .map(usize::from)
                .collect();
                Self { form, bytes, homes }
            }

            fn execute(&self, left: i128, right: i128) -> Option<u64> {
                let sources = self.form.source_count();
                let mut file = [0xa5a5_5a5a_a5a5_5a5a_u64; 16];
                file[self.homes[0]] = left as u64;
                if sources == 2 {
                    file[self.homes[1]] = right as u64;
                }
                let initial = file;
                match execute_decoded(&self.bytes, file) {
                    DecodedOutcome::Trapped => None,
                    DecodedOutcome::Completed(file) => {
                        for input in &self.homes[..sources] {
                            if !self.homes[sources..].contains(input) {
                                assert_eq!(file[*input], initial[*input], "{:?} input", self.form);
                            }
                        }
                        Some(file[self.homes[sources]])
                    }
                }
            }
        }

        #[test]
        fn trapping_decoded_forms_trap_exactly_when_the_policy_predicate_holds() {
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            for form in TrappingForm::ALL {
                let programs =
                    [false, true].map(|high| TrappingProgram::encode(&physical, form, high));
                let (mut trapped, mut completed) = (0, 0);
                for (left, right) in trapping_inputs(form) {
                    let expected = trapping_reference(form, left, right);
                    for program in &programs {
                        assert_eq!(
                            program.execute(left, right),
                            expected,
                            "{form:?} {:?} {left} {right}",
                            program.homes
                        );
                    }
                    if expected.is_some() {
                        completed += 1;
                    } else {
                        trapped += 1;
                    }
                }
                let identity = TrappingShape::of(form) == TrappingShape::Convert(None);
                assert_eq!(trapped > 0, !identity, "{form:?} trapped {trapped}");
                assert!(completed > 0, "{form:?} completed {completed}");
            }
        }

        #[test]
        fn trapping_decoded_forms_decide_the_named_boundary_cases() {
            use SaturatingCarrier::{I8, I16, I32, I64, U8, U16, U32, U64};
            use TrappingOperation::{
                Add, Divide, Multiply, Remainder, ShiftLeft, ShiftRight, Subtract,
            };
            let physical =
                validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let form = |operation, carrier| TrappingForm { operation, carrier };
            let convert = |source, carrier| TrappingForm {
                operation: TrappingOperation::Convert { source },
                carrier,
            };
            let (signed, unsigned) = (IntegerSign::Signed, IntegerSign::Unsigned);
            let cases: Vec<(TrappingForm, i128, i128, Option<i128>)> = vec![
                (form(Add, I8), 127, 0, Some(127)),
                (form(Add, I8), 127, 1, None),
                (form(Add, I8), -128, -1, None),
                (form(Subtract, I16), -32768, 0, Some(-32768)),
                (form(Subtract, I16), -32768, 1, None),
                (form(Add, I32), i128::from(i32::MAX), 1, None),
                (form(Add, I64), i128::from(i64::MAX), 1, None),
                (form(Add, I64), i128::from(i64::MIN), -1, None),
                row!(form(Add, I64) => i128::from(i64::MIN) => i128::from(i64::MAX) => Some(-1)),
                (form(Subtract, I64), i128::from(i64::MIN), 1, None),
                row!(form(Subtract, I64) => -1 => i128::from(i64::MIN) => Some(i128::from(i64::MAX))),
                (form(Add, U8), 255, 0, Some(255)),
                (form(Add, U8), 255, 1, None),
                (form(Subtract, U32), 0, 1, None),
                (form(Subtract, U32), 1, 1, Some(0)),
                (form(Add, U64), i128::from(u64::MAX), 1, None),
                row!(form(Add, U64) => i128::from(u64::MAX) - 1 => 1 => Some(i128::from(u64::MAX))),
                (form(Subtract, U64), 0, 1, None),
                (form(Multiply, I8), -128, 1, Some(-128)),
                (form(Multiply, I8), -128, -1, None),
                (form(Multiply, I32), 46341, 46341, None),
                (form(Multiply, I32), 46340, 46340, Some(46340 * 46340)),
                (form(Multiply, U32), 65536, 65536, None),
                row!(form(Multiply, U32) => 65535 => 65537 => Some(i128::from(u32::MAX))),
                (form(Multiply, I64), i128::from(i64::MIN), -1, None),
                row!(form(Multiply, I64) => i128::from(i64::MIN) => 1 => Some(i128::from(i64::MIN))),
                (form(Multiply, U64), 1 << 32, 1 << 32, None),
                row!(form(Multiply, U64) => (1 << 32) - 1 => (1 << 32) + 1 => Some(i128::from(u64::MAX))),
                (form(Divide, U8), 7, 0, None),
                (form(Remainder, U64), 7, 0, None),
                (form(Divide, I64), 0, 0, None),
                (form(Divide, I8), -128, -1, None),
                (form(Divide, I8), -128, 1, Some(-128)),
                (form(Divide, I8), -1, -1, Some(1)),
                (form(Remainder, I8), -128, -1, None),
                (form(Remainder, I8), -128, 1, Some(0)),
                (form(Divide, I16), -32768, -1, None),
                (form(Divide, I32), i128::from(i32::MIN), -1, None),
                row!(form(Divide, I32) => i128::from(i32::MIN) => 1 => Some(i128::from(i32::MIN))),
                (form(Divide, I64), i128::from(i64::MIN), -1, None),
                row!(form(Divide, I64) => i128::from(i64::MIN) => 1 => Some(i128::from(i64::MIN))),
                (form(Divide, I64), -1, -1, Some(1)),
                (form(Remainder, I64), i128::from(i64::MIN), -1, None),
                (form(Remainder, I64), i128::from(i64::MIN) + 1, -1, Some(0)),
                (form(Remainder, I32), -7, 2, Some(-1)),
                row!(form(Divide, U64) => i128::from(u64::MAX) => 1 => Some(i128::from(u64::MAX))),
                (form(ShiftLeft, I8), -1, 7, Some(-128)),
                (form(ShiftLeft, I8), 1, 7, None),
                (form(ShiftLeft, I8), 1, 8, None),
                (form(ShiftLeft, I8), 1, -1, None),
                (form(ShiftLeft, U8), 1, 7, Some(128)),
                (form(ShiftLeft, U8), 2, 7, None),
                (form(ShiftLeft, U16), 1, 16, None),
                (form(ShiftLeft, I32), 1, 30, Some(1 << 30)),
                (form(ShiftLeft, I32), 1, 31, None),
                (form(ShiftLeft, U32), 1, 31, Some(1 << 31)),
                (form(ShiftLeft, U32), 3, 31, None),
                (form(ShiftLeft, I64), -1, 63, Some(i128::from(i64::MIN))),
                (form(ShiftLeft, I64), 1, 63, None),
                (form(ShiftLeft, I64), 1, 64, None),
                (form(ShiftLeft, I64), 1, i128::from(i64::MIN), None),
                (form(ShiftLeft, U64), 1, 63, Some(1 << 63)),
                (form(ShiftLeft, U64), 2, 63, None),
                (form(ShiftRight, I32), -1, 31, Some(-1)),
                (form(ShiftRight, I32), -1, 32, None),
                (form(ShiftRight, I16), -32768, 15, Some(-1)),
                (form(ShiftRight, U8), 255, 7, Some(1)),
                (form(ShiftRight, U8), 255, -1, None),
                (form(ShiftRight, U64), i128::from(u64::MAX), 63, Some(1)),
                (form(ShiftRight, I64), i128::from(i64::MIN), 63, Some(-1)),
                (form(ShiftRight, I64), 1, 64, None),
                (convert(unsigned, I8), i128::from(u64::MAX), 0, None),
                (convert(unsigned, I8), 127, 0, Some(127)),
                (convert(unsigned, I8), 128, 0, None),
                (convert(signed, I8), -128, 0, Some(-128)),
                (convert(signed, I8), -129, 0, None),
                (convert(signed, I8), 128, 0, None),
                (convert(signed, I16), -32769, 0, None),
                row!(convert(signed, I32) => i128::from(i32::MIN) => 0 => Some(i128::from(i32::MIN))),
                (convert(signed, I32), i128::from(i32::MAX) + 1, 0, None),
                (convert(signed, U64), -1, 0, None),
                row!(convert(signed, U64) => i128::from(i64::MAX) => 0 => Some(i128::from(i64::MAX))),
                (convert(signed, U8), -1, 0, None),
                (convert(signed, U8), 255, 0, Some(255)),
                (convert(signed, U8), 256, 0, None),
                (convert(unsigned, U16), 65536, 0, None),
                row!(convert(unsigned, U32) => i128::from(u32::MAX) => 0 => Some(i128::from(u32::MAX))),
                (convert(unsigned, U32), 1 << 32, 0, None),
                (convert(unsigned, I64), 1 << 63, 0, None),
                row!(convert(unsigned, I64) => (1 << 63) - 1 => 0 => Some(i128::from(i64::MAX))),
                row!(convert(signed, I64) => i128::from(i64::MIN) => 0 => Some(i128::from(i64::MIN))),
                row!(convert(unsigned, U64) => i128::from(u64::MAX) => 0 => Some(i128::from(u64::MAX))),
            ];
            for (form, left, right, expected) in cases {
                let expected = expected.map(|value| value as u64);
                assert_eq!(
                    trapping_reference(form, left, right),
                    expected,
                    "reference {form:?} {left} {right}"
                );
                for high in [false, true] {
                    assert_eq!(
                        TrappingProgram::encode(&physical, form, high).execute(left, right),
                        expected,
                        "{form:?} high={high} {left} {right}"
                    );
                }
            }
        }
    }
    pub(crate) fn assert_byte_loop_catalog_contract(
        key: register_model::RegisterConstraintKey,
        scratch: &[usize],
        semantic: MachineSemanticKind,
        memory: MachineMemoryEffect,
        reads: &[u16],
        writes: &[u16],
    ) {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = crate::x86_64_register_constraint_catalog(&physical);
        let constraints =
            crate::validate_x86_64_register_constraint_catalog(catalog.clone(), &physical).unwrap();
        assert_early_clobber_required(&catalog, &physical, key, scratch);
        for target in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::windows_x64(),
        ] {
            let effects = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
            crate::validate_x86_64_machine_effect_catalog(target, &constraints, effects.clone())
                .unwrap();
            let row = effects
                .declarations
                .iter()
                .find(|row| row.semantic == semantic)
                .unwrap();
            assert_eq!(row.memory, memory);
            assert_eq!(row.alternatives[0].encoded.external_operand_reads, reads);
            assert_eq!(row.alternatives[0].encoded.external_operand_writes, writes);
        }
    }

    pub(crate) fn assert_loop_form_round_trips_and_rejects_every_changed_bit(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        rosters: &[&[&str]],
        encode: impl Fn(
            &[RegisterViewId],
            u32,
        )
            -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError>,
        validate: impl Fn(&[RegisterViewId], &[u8]) -> bool,
    ) {
        for names in rosters {
            let operands = names
                .iter()
                .map(|name| physical.model().view_named(name).unwrap().id)
                .collect::<Vec<_>>();
            let encoded = encode(&operands, 0).unwrap();
            assert_eq!(
                encode_x86_64_selected_form(physical, kind, alternative, &operands).unwrap(),
                encoded
            );
            validate_x86_64_selected_form_encoding(
                physical,
                kind,
                alternative,
                &operands,
                encoded.bytes(),
            )
            .unwrap();
            super::assert_every_bit_flip_rejected(encoded.bytes(), |changed| {
                validate(&operands, changed)
            });
            super::assert_byte_fragility(encoded.bytes(), |bytes| validate(&operands, bytes));
            assert!(encode(&operands, 1).is_err());
        }
    }

    fn alternative(family: Family, variant: u32) -> MachineAlternativeKey {
        MachineAlternativeKey { family, variant }
    }

    fn exact_add() -> Kind {
        Kind::ExactAddI64 {
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
        }
    }

    fn assert_every_byte_change_rejected(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        key: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
        context: &str,
    ) {
        for byte_position in 0..bytes.len() {
            let mut changed = bytes.to_vec();
            changed[byte_position] ^= 1;
            assert!(
                validate_x86_64_selected_form_encoding(physical, kind, key, operands, &changed)
                    .is_err(),
                "{context} byte {byte_position}"
            );
        }
    }

    fn assert_every_bit_change_rejected(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        key: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
        context: &str,
    ) {
        for byte_position in 0..bytes.len() {
            for bit in 0..8 {
                let mut changed = bytes.to_vec();
                changed[byte_position] ^= 1 << bit;
                assert!(
                    validate_x86_64_selected_form_encoding(physical, kind, key, operands, &changed)
                        .is_err(),
                    "{context} byte {byte_position} bit {bit}"
                );
            }
        }
    }

    fn assert_every_operand_substitution_rejected(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        key: MachineAlternativeKey,
        operands: &[RegisterViewId],
        substitute: RegisterViewId,
        bytes: &[u8],
        context: &str,
    ) {
        for operand_position in 0..operands.len() {
            let mut changed = operands.to_vec();
            changed[operand_position] = substitute;
            assert!(
                validate_x86_64_selected_form_encoding(physical, kind, key, &changed, bytes)
                    .is_err(),
                "{context} substituted operand {operand_position}"
            );
        }
    }

    #[test]
    fn saturating_subtract_binds_registers_condition_and_flag_effects() {
        assert_saturating_operation_binds_registers_condition_and_flag_effects(
            Kind::SaturatingSubtract {
                carrier: SaturatingCarrier::U64,
            },
            alternative(Family::SaturatingSubtract(SaturatingCarrier::U64), 0),
            13,
        );
    }

    #[test]
    fn saturating_add_binds_registers_condition_and_flag_effects() {
        assert_saturating_operation_binds_registers_condition_and_flag_effects(
            Kind::SaturatingAdd {
                carrier: SaturatingCarrier::U64,
            },
            alternative(Family::SaturatingAdd(SaturatingCarrier::U64), 0),
            19,
        );
    }

    fn assert_saturating_operation_binds_registers_condition_and_flag_effects(
        kind: Kind,
        key: MachineAlternativeKey,
        encoded_length: usize,
    ) {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        for left in ["rax", "rcx", "r8", "r15"] {
            for right in ["rax", "rcx", "r8", "r15"] {
                for destination in ["rax", "rcx", "r8", "r15"] {
                    let operands = [left, right, destination]
                        .map(|name| physical.model().view_named(name).unwrap().id);
                    let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands);
                    if destination == left || destination == right {
                        assert!(
                            encoded.is_err(),
                            "early-clobber output cannot overlap either input"
                        );
                        continue;
                    }
                    let encoded = encoded.unwrap();
                    assert_eq!(encoded.bytes().len(), encoded_length);
                    let decoded = validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &operands,
                        encoded.bytes(),
                    )
                    .unwrap();
                    assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
                    assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
                    assert!(
                        !decoded
                            .footprint()
                            .encoded
                            .implicit_unit_clobbers
                            .is_empty()
                    );
                    assert_every_byte_change_rejected(
                        &physical,
                        kind,
                        key,
                        &operands,
                        encoded.bytes(),
                        "changed",
                    );
                }
            }
        }
    }

    #[test]
    fn saturation_high_register_bytes_match_independent_assembler() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let operands =
            ["r8", "r9", "r10"].map(|name| physical.model().view_named(name).unwrap().id);
        for (kind, family, bytes) in [
            (
                Kind::SaturatingSubtract {
                    carrier: SaturatingCarrier::U64,
                },
                Family::SaturatingSubtract(SaturatingCarrier::U64),
                vec![
                    0x4d, 0x39, 0xc8, 0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0x42, 0xd1, 0x4d, 0x29, 0xca,
                ],
            ),
            (
                Kind::SaturatingAdd {
                    carrier: SaturatingCarrier::U64,
                },
                Family::SaturatingAdd(SaturatingCarrier::U64),
                vec![
                    0x4d, 0x89, 0xc2, 0x49, 0xf7, 0xd2, 0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x42, 0xd1,
                    0x4d, 0x29, 0xca, 0x49, 0xf7, 0xd2,
                ],
            ),
        ] {
            let key = alternative(family, 0);
            let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_eq!(encoded.bytes(), bytes);
            validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &bytes)
                .unwrap();
        }
    }

    #[test]
    fn variable_shift_bytes_match_independent_assembler() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let operands =
            ["r8", "rcx", "r10"].map(|name| physical.model().view_named(name).unwrap().id);
        for (kind, family, tail) in [
            (
                Kind::WrappingShiftLeftI64,
                Family::WrappingShiftLeftI64,
                0xe2u8,
            ),
            (
                Kind::WrappingShiftRightU64,
                Family::WrappingShiftRightU64,
                0xea,
            ),
            (
                Kind::WrappingShiftRightI64,
                Family::WrappingShiftRightI64,
                0xfa,
            ),
        ] {
            let bytes = vec![0x4d, 0x89, 0xc2, 0x49, 0xd3, tail];
            let key = alternative(family, 0);
            let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_eq!(encoded.bytes(), bytes, "{kind:?} encodes its own D3 digit");
            validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &bytes)
                .unwrap();
        }
    }

    #[test]
    fn wrapping_remainder_binds_guard_signed_division_scratch_and_aliases() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let kind = Kind::WrappingRemainderI64 {
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
        };
        let key = alternative(Family::WrappingRemainderI64, 0);
        for divisor in ["rax", "rcx", "r9", "r15"] {
            let operands = ["rax", divisor, "rax", "rdx"]
                .map(|name| physical.model().view_named(name).unwrap().id);
            let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_eq!(encoded.bytes().len(), 17);
            assert_eq!(encoded.footprint().register_reads, operands[..2]);
            assert_eq!(encoded.footprint().register_writes, operands[2..]);
            assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
            assert_eq!(encoded.footprint().encoded.external_operand_writes, [2, 3]);
            assert!(encoded.footprint().writes_rflags);
            if divisor == "r9" {
                assert_eq!(
                    encoded.bytes(),
                    [
                        0x48, 0x31, 0xd2, 0x49, 0x83, 0xf9, 0xff, 0x74, 0x05, 0x48, 0x99, 0x49,
                        0xf7, 0xf9, 0x48, 0x89, 0xd0,
                    ]
                );
            }
            assert_every_byte_change_rejected(
                &physical,
                kind,
                key,
                &operands,
                encoded.bytes(),
                "mutated",
            );
            assert_every_operand_substitution_rejected(
                &physical,
                kind,
                key,
                &operands,
                physical.model().view_named("r8").unwrap().id,
                encoded.bytes(),
                "mutated",
            );
        }
        let invalid =
            ["rax", "rdx", "rax", "rdx"].map(|name| physical.model().view_named(name).unwrap().id);
        assert!(encode_x86_64_selected_form(&physical, kind, key, &invalid).is_err());
    }

    #[test]
    fn wrapping_remainder_guard_skips_overflowing_quotient_and_keeps_dividend_sign() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let kind = Kind::WrappingRemainderI64 {
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
        };
        let key = alternative(Family::WrappingRemainderI64, 0);
        let operands =
            ["rax", "r9", "rax", "rdx"].map(|name| physical.model().view_named(name).unwrap().id);
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
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
            let mut registers = [0_i64; 16];
            registers[0] = dividend;
            registers[9] = divisor;
            registers[2] = 123;
            let mut equal = false;
            let mut byte_position = 0;
            while byte_position < encoded.bytes().len() {
                let (instruction, length) = decode_one(&encoded.bytes()[byte_position..]).unwrap();
                byte_position += length;
                match instruction {
                    Decoded::Xor {
                        source,
                        destination,
                    } => {
                        registers[destination as usize] ^= registers[source as usize];
                    }
                    Decoded::CompareSignedImmediate8 {
                        register,
                        immediate,
                    } => {
                        equal = registers[register as usize] == i64::from(immediate);
                    }
                    Decoded::JumpEqualShort { displacement } => {
                        if equal {
                            byte_position = byte_position
                                .checked_add_signed(displacement as isize)
                                .unwrap();
                        }
                    }
                    Decoded::SignExtendDividend => {
                        registers[2] = registers[0] >> 63;
                    }
                    Decoded::SignedDivide { divisor } => {
                        let dividend =
                            (i128::from(registers[2]) << 64) | i128::from(registers[0] as u64);
                        let divisor = i128::from(registers[divisor as usize]);
                        let quotient =
                            i64::try_from(dividend / divisor).expect("IDIV quotient must fit");
                        registers[2] = (dividend % divisor) as i64;
                        registers[0] = quotient;
                    }
                    Decoded::Move {
                        source,
                        destination,
                    } => {
                        registers[destination as usize] = registers[source as usize];
                    }
                    _ => panic!("unexpected remainder instruction"),
                }
            }
            assert_eq!(registers[0], expected, "{dividend} % {divisor}");
            assert_eq!(registers[9], divisor);
        }
    }

    #[test]
    fn wrapping_divide_then_sign_extension_wraps_narrow_min_by_minus_one() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let view = |name: &str| physical.model().view_named(name).unwrap().id;
        let divide = encode_x86_64_selected_form(
            &physical,
            Kind::WrappingDivideI64 {
                obligation: ObligationId::new(1).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
            },
            alternative(Family::WrappingDivideI64, 0),
            &["rax", "r9", "rax", "rdx"].map(view),
        )
        .unwrap();
        for (extension, family, minimum, maximum) in [
            (
                Some(Kind::SignExtendI8),
                Family::SignExtendI8,
                i64::from(i8::MIN),
                i64::from(i8::MAX),
            ),
            (
                Some(Kind::SignExtendI16),
                Family::SignExtendI16,
                i64::from(i16::MIN),
                i64::from(i16::MAX),
            ),
            (
                Some(Kind::SignExtendI32),
                Family::SignExtendI32,
                i64::from(i32::MIN),
                i64::from(i32::MAX),
            ),
            (None, Family::SignExtendI32, i64::MIN, i64::MAX),
        ] {
            let mut bytes = divide.bytes().to_vec();
            if let Some(extension) = extension {
                bytes.extend(
                    encode_x86_64_selected_form(
                        &physical,
                        extension,
                        alternative(family, 0),
                        &["rax", "rax"].map(view),
                    )
                    .unwrap()
                    .bytes(),
                );
            }
            for (dividend, divisor, quotient) in [
                (minimum, -1, minimum),
                (minimum, 1, minimum),
                (maximum, -1, -maximum),
                (minimum, 2, minimum / 2),
                (-7, 2, -3),
                (7, -2, -3),
                (0, -1, 0),
            ] {
                let mut registers = [0_i64; 16];
                registers[0] = dividend;
                registers[9] = divisor;
                let mut equal = false;
                let mut byte_position = 0;
                while byte_position < bytes.len() {
                    let (instruction, length) = decode_one(&bytes[byte_position..]).unwrap();
                    byte_position += length;
                    match instruction {
                        Decoded::CompareSignedImmediate8 {
                            register,
                            immediate,
                        } => equal = registers[register as usize] == i64::from(immediate),
                        Decoded::JumpNotEqualShort { displacement } => {
                            if !equal {
                                byte_position = byte_position
                                    .checked_add_signed(displacement as isize)
                                    .unwrap();
                            }
                        }
                        Decoded::JumpShort { displacement } => {
                            byte_position = byte_position
                                .checked_add_signed(displacement as isize)
                                .unwrap();
                        }
                        Decoded::Negate { destination } => {
                            registers[destination as usize] =
                                registers[destination as usize].wrapping_neg();
                        }
                        Decoded::SignExtendDividend => registers[2] = registers[0] >> 63,
                        Decoded::SignedDivide { divisor } => {
                            let dividend =
                                (i128::from(registers[2]) << 64) | i128::from(registers[0] as u64);
                            let divisor = i128::from(registers[divisor as usize]);
                            registers[0] = i64::try_from(dividend / divisor)
                                .expect("a guarded IDIV never sees the faulting pair");
                            registers[2] = (dividend % divisor) as i64;
                        }
                        Decoded::SignExtendI8 {
                            source,
                            destination,
                        } => {
                            registers[destination as usize] =
                                i64::from(registers[source as usize] as i8)
                        }
                        Decoded::SignExtendI16 {
                            source,
                            destination,
                        } => {
                            registers[destination as usize] =
                                i64::from(registers[source as usize] as i16)
                        }
                        Decoded::SignExtendI32 {
                            source,
                            destination,
                        } => {
                            registers[destination as usize] =
                                i64::from(registers[source as usize] as i32)
                        }
                        other => panic!("unexpected wrapping divide instruction {other:?}"),
                    }
                }
                assert_eq!(registers[0], quotient, "{dividend} / {divisor}");
                assert_eq!(registers[9], divisor);
            }
        }
    }

    #[test]
    fn exact_divide_binds_unsigned_opcode_and_registers() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let kind = Kind::ExactDivideU64 {
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
        };
        let key = alternative(Family::ExactDivideU64, 0);

        let names = ["rax", "r9", "rax", "rdx"];
        let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
        let expected = vec![0x49, 0xf7, 0xf1];

        let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert_every_byte_change_rejected(&physical, kind, key, &operands, &expected, "changed");
        assert_every_operand_substitution_rejected(
            &physical,
            kind,
            key,
            &operands,
            physical.model().view_named("r8").unwrap().id,
            &expected,
            "changed",
        );
    }

    fn saturating_kind(operation: SaturatingOperation, carrier: SaturatingCarrier) -> Kind {
        match operation {
            SaturatingOperation::Add => Kind::SaturatingAdd { carrier },
            SaturatingOperation::Subtract => Kind::SaturatingSubtract { carrier },
            SaturatingOperation::Divide => Kind::SaturatingDivide {
                carrier,
                obligation: ObligationId::new(1).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
            },
            SaturatingOperation::Remainder => Kind::SaturatingRemainder {
                carrier,
                obligation: ObligationId::new(1).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
            },
            SaturatingOperation::Multiply => Kind::SaturatingMultiply { carrier },
        }
    }

    fn saturating_key(
        operation: SaturatingOperation,
        carrier: SaturatingCarrier,
    ) -> MachineAlternativeKey {
        alternative(
            match operation {
                SaturatingOperation::Add => Family::SaturatingAdd(carrier),
                SaturatingOperation::Subtract => Family::SaturatingSubtract(carrier),
                SaturatingOperation::Divide => Family::SaturatingDivide(carrier),
                SaturatingOperation::Remainder => Family::SaturatingRemainder(carrier),
                SaturatingOperation::Multiply => Family::SaturatingMultiply(carrier),
            },
            0,
        )
    }

    fn saturating_operand_names(
        operation: SaturatingOperation,
        carrier: SaturatingCarrier,
        high: bool,
    ) -> Vec<&'static str> {
        let form = SaturatingForm::of(operation, carrier);
        if form.is_fixed_rax_rdx() {
            vec!["rax", if high { "r9" } else { "rsi" }, "rax", "rdx"]
        } else if form.operand_count() == 4 {
            if high {
                vec!["r8", "r11", "r10", "r9"]
            } else {
                vec!["rdi", "rsi", "rax", "rcx"]
            }
        } else if high {
            vec!["r8", "r9", "r10"]
        } else {
            vec!["rdi", "rsi", "rax"]
        }
    }

    fn saturating_operands(
        physical: &register_model::ValidatedPhysicalRegisterModel,
        names: &[&str],
    ) -> Vec<register_model::RegisterViewId> {
        names
            .iter()
            .map(|name| physical.model().view_named(name).unwrap().id)
            .collect()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DecodedOutcome {
        Completed([u64; 16]),
        Trapped,
    }

    #[derive(Default, Clone, Copy)]
    struct SaturatingFlags {
        overflow: Option<bool>,
        carry: Option<bool>,
        greater: Option<bool>,
        less: Option<bool>,
        zero: Option<bool>,
    }

    impl SaturatingFlags {
        fn arithmetic(exact: i128, overflow: bool, carry: bool) -> Self {
            Self {
                overflow: Some(overflow),
                carry: Some(carry),
                greater: Some(exact > 0),
                less: Some(exact < 0),
                zero: Some(exact as u64 == 0),
            }
        }

        fn undefined() -> Self {
            Self::default()
        }
    }

    fn execute_decoded(bytes: &[u8], mut registers: [u64; 16]) -> DecodedOutcome {
        let mut flags = SaturatingFlags::default();
        let mut boundaries = vec![0];
        let signed = |value: u64| i128::from(value as i64);
        let subtract_flags = |left: u64, right: u64| {
            let exact = signed(left) - signed(right);
            SaturatingFlags::arithmetic(exact, i64::try_from(exact).is_err(), left < right)
        };
        let mut position = 0;
        while position < bytes.len() {
            position += decode_one(&bytes[position..]).unwrap().1;
            boundaries.push(position);
        }
        let mut byte_position = 0;
        let jump = |byte_position: &mut usize, displacement: i8, taken: bool| {
            if taken {
                *byte_position = byte_position
                    .checked_add_signed(isize::from(displacement))
                    .unwrap();
                assert!(
                    boundaries.contains(byte_position),
                    "a jump lands on an instruction boundary of the form"
                );
            }
        };
        while byte_position < bytes.len() {
            let (instruction, length) = decode_one(&bytes[byte_position..]).unwrap();
            byte_position += length;
            match instruction {
                Decoded::Crash => return DecodedOutcome::Trapped,
                Decoded::Move {
                    source,
                    destination,
                } => registers[usize::from(destination)] = registers[usize::from(source)],
                Decoded::Materialize { destination, value } => {
                    registers[usize::from(destination)] = value;
                }
                Decoded::Add {
                    source,
                    destination,
                } => {
                    let (left, right) = (
                        registers[usize::from(destination)],
                        registers[usize::from(source)],
                    );
                    let exact = signed(left) + signed(right);
                    registers[usize::from(destination)] = left.wrapping_add(right);
                    flags = SaturatingFlags::arithmetic(
                        exact,
                        i64::try_from(exact).is_err(),
                        left.checked_add(right).is_none(),
                    );
                }
                Decoded::Subtract {
                    source,
                    destination,
                } => {
                    let (left, right) = (
                        registers[usize::from(destination)],
                        registers[usize::from(source)],
                    );
                    registers[usize::from(destination)] = left.wrapping_sub(right);
                    flags = subtract_flags(left, right);
                }
                Decoded::Compare { left, right } => {
                    flags =
                        subtract_flags(registers[usize::from(left)], registers[usize::from(right)]);
                }
                Decoded::CompareSignedImmediate8 {
                    register,
                    immediate,
                } => {
                    flags =
                        subtract_flags(registers[usize::from(register)], immediate as i64 as u64);
                }
                Decoded::CompareRaxImmediate32 { immediate } => {
                    flags = subtract_flags(registers[0], immediate as i64 as u64);
                }
                Decoded::Test { register } => {
                    flags = SaturatingFlags::arithmetic(
                        signed(registers[usize::from(register)]),
                        false,
                        false,
                    );
                }
                Decoded::SignExtendI8 {
                    source,
                    destination,
                } => {
                    registers[usize::from(destination)] =
                        i64::from(registers[usize::from(source)] as i8) as u64;
                }
                Decoded::SignExtendI16 {
                    source,
                    destination,
                } => {
                    registers[usize::from(destination)] =
                        i64::from(registers[usize::from(source)] as i16) as u64;
                }
                Decoded::SignExtendI32 {
                    source,
                    destination,
                } => {
                    registers[usize::from(destination)] =
                        i64::from(registers[usize::from(source)] as i32) as u64;
                }
                Decoded::ShiftRightLogicalImmediate { destination, count } => {
                    let value = registers[usize::from(destination)];
                    assert!((1..64).contains(&count));
                    registers[usize::from(destination)] = value >> count;
                    flags = SaturatingFlags {
                        carry: Some((value >> (count - 1)) & 1 == 1),
                        zero: Some(value >> count == 0),
                        ..SaturatingFlags::undefined()
                    };
                }
                Decoded::ShiftLeftByCl { destination }
                | Decoded::ShiftRightLogicalByCl { destination }
                | Decoded::ShiftRightArithmeticByCl { destination } => {
                    let count = registers[1] & 63;
                    let value = registers[usize::from(destination)];
                    registers[usize::from(destination)] = match instruction {
                        Decoded::ShiftLeftByCl { .. } => value << count,
                        Decoded::ShiftRightLogicalByCl { .. } => value >> count,
                        _ => ((value as i64) >> count) as u64,
                    };
                    if count != 0 {
                        flags = SaturatingFlags::undefined();
                    }
                }
                Decoded::JumpNotEqualShort { displacement } => {
                    let taken = !flags.zero.expect("JNE reads a defined ZF");
                    jump(&mut byte_position, displacement, taken);
                }
                Decoded::JumpNoOverflowShort { displacement } => {
                    let taken = !flags.overflow.expect("JNO reads a defined OF");
                    jump(&mut byte_position, displacement, taken);
                }
                Decoded::JumpBelowShort { displacement } => {
                    let taken = flags.carry.expect("JB reads a defined CF");
                    jump(&mut byte_position, displacement, taken);
                }
                Decoded::JumpAboveOrEqualShort { displacement } => {
                    let taken = !flags.carry.expect("JAE reads a defined CF");
                    jump(&mut byte_position, displacement, taken);
                }
                Decoded::Complement { destination } => {
                    registers[usize::from(destination)] = !registers[usize::from(destination)];
                }
                Decoded::Negate { destination } => {
                    let value = registers[usize::from(destination)];
                    registers[usize::from(destination)] = value.wrapping_neg();
                    flags = subtract_flags(0, value);
                }
                Decoded::SubtractWithBorrow {
                    source,
                    destination,
                } => {
                    let (left, right) = (
                        registers[usize::from(destination)],
                        registers[usize::from(source)],
                    );
                    let borrow = u64::from(flags.carry.expect("SBB reads a defined CF"));
                    let exact = signed(left) - signed(right) - i128::from(borrow);
                    registers[usize::from(destination)] =
                        left.wrapping_sub(right).wrapping_sub(borrow);
                    flags = SaturatingFlags::arithmetic(
                        exact,
                        i64::try_from(exact).is_err(),
                        u128::from(left) < u128::from(right) + u128::from(borrow),
                    );
                }
                Decoded::Or {
                    source,
                    destination,
                } => {
                    registers[usize::from(destination)] |= registers[usize::from(source)];
                    flags = SaturatingFlags::arithmetic(
                        signed(registers[usize::from(destination)]),
                        false,
                        false,
                    );
                }
                Decoded::ArithmeticShiftRight63 { destination } => {
                    let value = registers[usize::from(destination)];
                    registers[usize::from(destination)] = ((value as i64) >> 63) as u64;
                    flags = SaturatingFlags {
                        carry: Some((value >> 62) & 1 == 1),
                        ..SaturatingFlags::undefined()
                    };
                }
                Decoded::ComplementBit63 { destination } => {
                    let value = registers[usize::from(destination)];
                    registers[usize::from(destination)] = value ^ (1 << 63);
                    flags = SaturatingFlags {
                        carry: Some(value >> 63 == 1),
                        ..SaturatingFlags::undefined()
                    };
                }
                Decoded::Lea {
                    destination,
                    base,
                    index: None,
                    displacement,
                } => {
                    registers[usize::from(destination)] =
                        registers[usize::from(base)].wrapping_add(displacement as i64 as u64);
                }
                Decoded::MoveOnOverflow {
                    source,
                    destination,
                } => {
                    if flags.overflow.expect("CMOVO reads a defined OF") {
                        registers[usize::from(destination)] = registers[usize::from(source)];
                    }
                }
                Decoded::MoveOnBorrow {
                    source,
                    destination,
                } => {
                    if flags.carry.expect("CMOVB reads a defined CF") {
                        registers[usize::from(destination)] = registers[usize::from(source)];
                    }
                }
                Decoded::MoveOnGreater {
                    source,
                    destination,
                } => {
                    if flags.greater.expect("CMOVG reads defined ZF/SF/OF") {
                        registers[usize::from(destination)] = registers[usize::from(source)];
                    }
                }
                Decoded::MoveOnLess {
                    source,
                    destination,
                } => {
                    if flags.less.expect("CMOVL reads defined SF/OF") {
                        registers[usize::from(destination)] = registers[usize::from(source)];
                    }
                }
                Decoded::MoveOnAbove {
                    source,
                    destination,
                } => {
                    let carry = flags.carry.expect("CMOVA reads a defined CF");
                    let nonzero = flags.greater.expect("CMOVA reads a defined ZF")
                        || flags.less.expect("CMOVA reads a defined ZF");
                    if !carry && nonzero {
                        registers[usize::from(destination)] = registers[usize::from(source)];
                    }
                }
                Decoded::Multiply {
                    source,
                    destination,
                } => {
                    let (left, right) = (
                        registers[usize::from(destination)],
                        registers[usize::from(source)],
                    );
                    let exact = signed(left) * signed(right);
                    registers[usize::from(destination)] = left.wrapping_mul(right);
                    let truncated = i64::try_from(exact).is_err();
                    flags = SaturatingFlags {
                        overflow: Some(truncated),
                        carry: Some(truncated),
                        ..SaturatingFlags::undefined()
                    };
                }
                Decoded::UnsignedMultiply { multiplier } => {
                    let product =
                        u128::from(registers[0]) * u128::from(registers[usize::from(multiplier)]);
                    registers[0] = product as u64;
                    registers[2] = (product >> 64) as u64;
                    let high = registers[2] != 0;
                    flags = SaturatingFlags {
                        overflow: Some(high),
                        carry: Some(high),
                        ..SaturatingFlags::undefined()
                    };
                }
                Decoded::SignExtendDividend => {
                    registers[2] = ((registers[0] as i64) >> 63) as u64;
                }
                Decoded::SignedDivide { divisor } => {
                    let dividend =
                        (i128::from(registers[2] as i64) << 64) | i128::from(registers[0]);
                    let divisor = i128::from(registers[usize::from(divisor)] as i64);
                    assert_ne!(divisor, 0, "IDIV by zero faults");
                    let quotient = i64::try_from(dividend / divisor)
                        .expect("IDIV faults when the quotient does not fit");
                    registers[2] = (dividend % divisor) as i64 as u64;
                    registers[0] = quotient as u64;
                    flags = SaturatingFlags::undefined();
                }
                Decoded::Xor {
                    source,
                    destination,
                } => {
                    registers[usize::from(destination)] ^= registers[usize::from(source)];
                    flags = SaturatingFlags::arithmetic(
                        signed(registers[usize::from(destination)]),
                        false,
                        false,
                    );
                }
                Decoded::JumpEqualShort { displacement } => {
                    let taken = flags.zero.expect("JE reads a defined ZF");
                    jump(&mut byte_position, displacement, taken);
                }
                Decoded::UnsignedDivide { divisor } => {
                    let dividend = (u128::from(registers[2]) << 64) | u128::from(registers[0]);
                    let divisor = u128::from(registers[usize::from(divisor)]);
                    assert_ne!(divisor, 0, "DIV by zero faults");
                    let quotient = u64::try_from(dividend / divisor)
                        .expect("DIV faults when the quotient does not fit");
                    registers[2] = (dividend % divisor) as u64;
                    registers[0] = quotient;
                    flags = SaturatingFlags::undefined();
                }
                other => panic!("unexpected decoded instruction {other:?}"),
            }
        }
        DecodedOutcome::Completed(registers)
    }
}
pub(crate) mod trapping_forms {
    use selected_instructions::{
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedTrapBehavior,
        SaturatingCarrier, SelectedInstructionKind, TrappingForm, TrappingOperation,
    };
    use semantic_vocabulary::IntegerSign;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum TrappingRow {
        Binary,
        FixedPair,
        Shift,
        Convert,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum TrappingArithmetic {
        Add,
        Subtract,
        Multiply,
    }

    impl TrappingArithmetic {
        const fn byte_count(self) -> u16 {
            match self {
                Self::Add | Self::Subtract => 3,
                Self::Multiply => 4,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum RangeCheck {
        SignExtension { bits: u16 },
        HighBits { shift: u8 },
    }

    impl RangeCheck {
        const fn of_narrow_carrier(carrier: SaturatingCarrier) -> Self {
            if carrier.is_signed() {
                Self::SignExtension {
                    bits: carrier.bits(),
                }
            } else {
                Self::HighBits {
                    shift: carrier.bits() as u8,
                }
            }
        }

        const fn of_conversion(source: IntegerSign, carrier: SaturatingCarrier) -> Option<Self> {
            match (source, carrier) {
                (IntegerSign::Signed, SaturatingCarrier::I64)
                | (IntegerSign::Unsigned, SaturatingCarrier::U64) => None,
                (IntegerSign::Signed, SaturatingCarrier::U64) => Some(Self::HighBits { shift: 63 }),
                (IntegerSign::Signed, _) if carrier.is_signed() => Some(Self::SignExtension {
                    bits: carrier.bits(),
                }),
                _ if carrier.is_signed() => Some(Self::HighBits {
                    shift: carrier.bits() as u8 - 1,
                }),
                _ => Some(Self::HighBits {
                    shift: carrier.bits() as u8,
                }),
            }
        }

        pub(crate) const fn byte_count(self) -> u16 {
            match self {
                Self::SignExtension { bits: 32 } => 10,
                Self::SignExtension { .. } | Self::HighBits { .. } => 11,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum DividendGuard {
        Minimum8(i8),
        Minimum32(i32),
        DecrementOverflows,
    }

    impl DividendGuard {
        const fn of(carrier: SaturatingCarrier) -> Option<Self> {
            match carrier {
                SaturatingCarrier::I8 => Some(Self::Minimum8(i8::MIN)),
                SaturatingCarrier::I16 => Some(Self::Minimum32(i16::MIN as i32)),
                SaturatingCarrier::I32 => Some(Self::Minimum32(i32::MIN)),
                SaturatingCarrier::I64 => Some(Self::DecrementOverflows),
                SaturatingCarrier::U8
                | SaturatingCarrier::U16
                | SaturatingCarrier::U32
                | SaturatingCarrier::U64 => None,
            }
        }

        pub(crate) const fn byte_count(self) -> u16 {
            match self {
                Self::Minimum8(_) | Self::DecrementOverflows => 8,
                Self::Minimum32(_) => 10,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum TrappingShape {
        OverflowFlag(TrappingArithmetic),
        CarryFlag(TrappingArithmetic),
        NarrowRange(TrappingArithmetic, RangeCheck),
        MultiplyHighHalf,
        Division {
            guard: Option<DividendGuard>,
            remainder: bool,
        },
        ShiftLeftRoundTrip {
            signed: bool,
        },
        ShiftLeftRange {
            width: u8,
            check: RangeCheck,
        },
        ShiftRight {
            width: u8,
            signed: bool,
        },
        Convert(Option<RangeCheck>),
    }

    impl TrappingShape {
        pub(crate) const fn of(form: TrappingForm) -> Self {
            let carrier = form.carrier;
            let arithmetic = match form.operation {
                TrappingOperation::Add => TrappingArithmetic::Add,
                TrappingOperation::Subtract => TrappingArithmetic::Subtract,
                _ => TrappingArithmetic::Multiply,
            };
            match form.operation {
                TrappingOperation::Multiply if matches!(carrier, SaturatingCarrier::U64) => {
                    Self::MultiplyHighHalf
                }
                TrappingOperation::Add
                | TrappingOperation::Subtract
                | TrappingOperation::Multiply => match carrier {
                    SaturatingCarrier::I64 => Self::OverflowFlag(arithmetic),
                    SaturatingCarrier::U64 => Self::CarryFlag(arithmetic),
                    _ => Self::NarrowRange(arithmetic, RangeCheck::of_narrow_carrier(carrier)),
                },
                TrappingOperation::Divide | TrappingOperation::Remainder => Self::Division {
                    guard: DividendGuard::of(carrier),
                    remainder: matches!(form.operation, TrappingOperation::Remainder),
                },
                TrappingOperation::ShiftLeft if carrier.is_narrow() => Self::ShiftLeftRange {
                    width: carrier.bits() as u8,
                    check: RangeCheck::of_narrow_carrier(carrier),
                },
                TrappingOperation::ShiftLeft => Self::ShiftLeftRoundTrip {
                    signed: carrier.is_signed(),
                },
                TrappingOperation::ShiftRight => Self::ShiftRight {
                    width: carrier.bits() as u8,
                    signed: carrier.is_signed(),
                },
                TrappingOperation::Convert { source } => {
                    Self::Convert(RangeCheck::of_conversion(source, carrier))
                }
            }
        }

        pub(crate) const fn row(self) -> TrappingRow {
            match self {
                Self::OverflowFlag(_) | Self::CarryFlag(_) | Self::NarrowRange(..) => {
                    TrappingRow::Binary
                }
                Self::MultiplyHighHalf | Self::Division { .. } => TrappingRow::FixedPair,
                Self::ShiftLeftRoundTrip { .. }
                | Self::ShiftLeftRange { .. }
                | Self::ShiftRight { .. } => TrappingRow::Shift,
                Self::Convert(_) => TrappingRow::Convert,
            }
        }

        pub(crate) const fn byte_count(self) -> u16 {
            const TRAP_UNLESS: u16 = 4;
            const COUNT_CHECK: u16 = 4 + TRAP_UNLESS;
            const SHIFT: u16 = 6;
            match self {
                Self::OverflowFlag(arithmetic) | Self::CarryFlag(arithmetic) => {
                    3 + arithmetic.byte_count() + TRAP_UNLESS
                }
                Self::NarrowRange(arithmetic, check) => {
                    3 + arithmetic.byte_count() + check.byte_count()
                }
                Self::MultiplyHighHalf => 3 + TRAP_UNLESS,
                Self::Division { guard, remainder } => {
                    let zero = 3 + TRAP_UNLESS;
                    let divide = match guard {
                        None => 6,
                        Some(guard) => 4 + 2 + guard.byte_count() + 5,
                    };
                    zero + divide + if remainder { 3 } else { 0 }
                }
                Self::ShiftLeftRoundTrip { .. } => COUNT_CHECK + SHIFT + 9 + TRAP_UNLESS,
                Self::ShiftLeftRange { check, .. } => COUNT_CHECK + SHIFT + check.byte_count(),
                Self::ShiftRight { .. } => COUNT_CHECK + SHIFT,
                Self::Convert(None) => 3,
                Self::Convert(Some(check)) => check.byte_count() + 3,
            }
        }

        pub(crate) const fn operand_count(self) -> usize {
            match self.row() {
                TrappingRow::Binary | TrappingRow::FixedPair | TrappingRow::Shift => 4,
                TrappingRow::Convert => 3,
            }
        }

        pub(crate) fn accepts_registers(self, registers: &[u8]) -> bool {
            if registers.len() != self.operand_count() {
                return false;
            }
            let (inputs, outputs) = registers.split_at(registers.len() - 2);
            match self.row() {
                TrappingRow::FixedPair => {
                    inputs[0] == 0 && outputs[0] == 0 && outputs[1] == 2 && inputs[1] != 2
                }
                TrappingRow::Shift if inputs[1] != 1 => false,
                TrappingRow::Binary | TrappingRow::Shift | TrappingRow::Convert => {
                    outputs.iter().all(|output| !inputs.contains(output))
                        && outputs[0] != outputs[1]
                }
            }
        }

        pub(crate) fn operand_reads_and_writes(self) -> (Vec<u16>, Vec<u16>) {
            match self.row() {
                TrappingRow::Binary | TrappingRow::FixedPair | TrappingRow::Shift => {
                    (vec![0, 1], vec![2, 3])
                }
                TrappingRow::Convert => (vec![0], vec![1, 2]),
            }
        }

        pub(crate) fn encoded_effects(self) -> MachineEncodedEffects {
            let (reads, writes) = self.operand_reads_and_writes();
            let mut encoded = MachineEncodedEffects::fallthrough_v1(reads, writes);
            encoded.implicit_unit_clobbers = crate::x86_64_physical_register_model()
                .view_named("condition_flags")
                .expect("canonical x86-64 model declares condition flags")
                .units
                .clone();
            encoded.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
            encoded.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
            encoded
        }
    }

    pub(crate) const fn trapping_form(kind: SelectedInstructionKind) -> Option<TrappingForm> {
        match kind {
            SelectedInstructionKind::TrappingInteger { form } => Some(form),
            _ => None,
        }
    }
}
pub use branch_forms::{
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
pub use hosted_write_byte::{
    encode_x86_64_selected_hosted_write_byte_form, validate_x86_64_selected_hosted_write_byte_form,
};
pub use jump::{encode_x86_64_selected_jump_form, validate_x86_64_selected_jump_form};
pub use memory::frame::{
    encode as encode_x86_64_selected_frame_form, validate as validate_x86_64_selected_frame_form,
};
pub use memory::{encode_x86_64_selected_memory_form, validate_x86_64_selected_memory_form};
pub use normalized_foreign_call::{
    X86_64CallbackAddressMaterialization, X86_64CallbackMaterializationDestination,
    X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState, X86_64NormalizedForeignCallTemplateError,
    encode_x86_64_selected_normalized_foreign_call_template,
    validate_x86_64_selected_normalized_foreign_call_template,
};
pub use scalar_call::{
    X86_64ScalarCallFixup, X86_64ScalarCallFixupKind, X86_64ScalarCallFixupState,
    X86_64ScalarCallTemplateError, encode_x86_64_selected_dynamic_call_template,
    encode_x86_64_selected_scalar_call_template, validate_x86_64_selected_scalar_call_template,
};

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind, is_float_bit_transfer,
};

use crate::selected_form_encoding::decoding::{decode_all, footprint, validate_decoded};
pub use crate::selected_form_encoding::instruction_bytes::X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET;
use crate::selected_form_encoding::instruction_bytes::encode_unchecked;
use crate::selected_form_encoding::request_validation::{
    resolve_scalar_registers, validate_return_home,
};
use crate::selected_form_encoding::request_validation::{
    validate_alias_partition, validate_request,
};

pub(super) fn canonical_effects_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    operands: &[RegisterViewId],
    bytes: &[u8],
    read_count: usize,
    memory: MachineEncodedMemoryEffect,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let flags = physical
        .model()
        .view_named("condition_flags")
        .ok_or(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)?;
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: operands[..read_count].to_vec(),
            register_writes: operands[read_count..].to_vec(),
            writes_rflags: true,
            encoded: MachineEncodedEffects {
                external_operand_reads: (0..read_count as u16).collect(),
                external_operand_writes: (read_count as u16..operands.len() as u16).collect(),
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: flags.units.clone(),
                memory,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64SelectedFormFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub writes_rflags: bool,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64SelectedFormEncoding {
    bytes: Vec<u8>,
    footprint: X86_64SelectedFormFootprint,
}

impl ValidatedX86_64SelectedFormEncoding {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &X86_64SelectedFormFootprint {
        &self.footprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64SelectedFormEncodingError {
    InstalledTable(Box<crate::X86InstalledTableEncodingError>),
    UnsupportedAtomicForm,
    NonCanonicalPhysicalModel,
    LayoutDependentForm,
    AlternativeMismatch,
    OperandCountMismatch,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideI64Bits,
    ImmediateOutsideU12,
    BranchDisplacementOutsideI32,
    MalformedEncoding,
    EncodedFormMismatch,
    BranchDisplacementOutsideI8,
}

optimization_core::invalid_error_display!(
    X86_64SelectedFormEncodingError,
    "x86-64 selected-form encoding"
);

pub fn encode_x86_64_selected_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if installed_interrupt::Form::from_kind(kind).is_some() {
        return installed_interrupt::encode(physical, kind, alternative, operands);
    }
    if kind == SelectedInstructionKind::CopyBytes {
        return copy_bytes::encode(physical, alternative, operands, 0);
    }
    if kind == SelectedInstructionKind::ZeroBytes {
        return zero_bytes::encode(physical, alternative, operands);
    }
    if matches!(
        kind,
        SelectedInstructionKind::LoadEncodedScalar | SelectedInstructionKind::StoreEncodedScalar
    ) {
        return encoded_scalar::encode(physical, kind, alternative, operands);
    }
    if let SelectedInstructionKind::ScanByteSequence { predicate } = kind {
        return scan_byte_sequence::encode(physical, predicate, alternative, operands, 0);
    }
    if kind == SelectedInstructionKind::CompareByteSequences {
        return compare_byte_sequences::encode(physical, alternative, operands, 0);
    }
    if is_float_bit_transfer(kind) {
        return float_bits::encode(physical, kind, alternative, operands);
    }
    if let SelectedInstructionKind::NearestIeeeFloatArithmetic { form } = kind {
        return ieee_float_arithmetic::encode(physical, form, alternative, operands);
    }
    if let SelectedInstructionKind::FloatResultPolicy { form } = kind {
        return float_result_policy::encode(physical, form, alternative, operands);
    }
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_scalar_registers(physical, kind, operands)?;
    validate_return_home(kind, &registers)?;
    validate_alias_partition(kind, alternative, &registers)?;
    let bytes = encode_unchecked(kind, alternative, &registers)?;
    validate_x86_64_selected_form_encoding(physical, kind, alternative, operands, &bytes)
}

pub fn validate_x86_64_selected_form_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    if installed_interrupt::Form::from_kind(kind).is_some() {
        return installed_interrupt::validate(physical, kind, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::RegisterAccess { operation } = kind {
        return register_access::validate(physical, operation, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::PortWrite { port, value } = kind {
        return port_write::validate(physical, port, value, alternative, operands, bytes);
    }
    if kind == SelectedInstructionKind::CopyBytes {
        return copy_bytes::validate(physical, alternative, operands, 0, bytes);
    }
    if kind == SelectedInstructionKind::ZeroBytes {
        return zero_bytes::validate(physical, alternative, operands, bytes);
    }
    if matches!(
        kind,
        SelectedInstructionKind::LoadEncodedScalar | SelectedInstructionKind::StoreEncodedScalar
    ) {
        return encoded_scalar::validate(physical, kind, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::ScanByteSequence { predicate } = kind {
        return scan_byte_sequence::validate(physical, predicate, alternative, operands, 0, bytes);
    }
    if kind == SelectedInstructionKind::CompareByteSequences {
        return compare_byte_sequences::validate(physical, alternative, operands, 0, bytes);
    }
    if is_float_bit_transfer(kind) {
        return float_bits::validate(physical, kind, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::NearestIeeeFloatArithmetic { form } = kind {
        return ieee_float_arithmetic::validate(physical, form, alternative, operands, bytes);
    }
    if let SelectedInstructionKind::FloatResultPolicy { form } = kind {
        return float_result_policy::validate(physical, form, alternative, operands, bytes);
    }
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_scalar_registers(physical, kind, operands)?;
    validate_return_home(kind, &registers)?;
    validate_alias_partition(kind, alternative, &registers)?;
    let decoded = decode_all(bytes)?;
    validate_decoded(kind, alternative, &registers, &decoded)?;
    let canonical = encode_unchecked(kind, alternative, &registers)?;
    if bytes != canonical {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(kind, alternative, operands),
    })
}

#[cfg(test)]
pub(super) fn assert_byte_fragility(encoded: &[u8], rejects: impl Fn(&[u8]) -> bool) {
    for length in 0..encoded.len() {
        assert!(rejects(&encoded[..length]));
    }
    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert!(rejects(&trailing));
}

#[cfg(test)]
pub(crate) fn assert_every_bit_flip_rejected(encoded: &[u8], rejects: impl Fn(&[u8]) -> bool) {
    for bit in 0..encoded.len() * 8 {
        let mut changed = encoded.to_vec();
        changed[bit / 8] ^= 1 << (bit % 8);
        assert!(rejects(&changed), "changed encoding bit {bit}");
    }
}
