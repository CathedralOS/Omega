use super::*;
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

#[test]
fn float_result_policy_trapping_bytes_bind_width_registers_and_branches() {
    let form = FloatResultPolicyForm {
        format: IeeeFloatFormat::Binary64,
        policy: FloatResultPolicyKind::Trapping,
    };
    // movq xmm3, rcx; movq r8, xmm3; shl r8, 1; shr r8, 53;
    // cmp r8, 2047; jne +2; ud2; movq r8, xmm3.
    assert_eq!(
        encoded(form, &[1, 8]),
        [
            0x66, 0x48, 0x0f, 0x6e, 0xd9, 0x66, 0x49, 0x0f, 0x7e, 0xd8, 0x49, 0xc1, 0xe0, 1, 0x49,
            0xc1, 0xe8, 53, 0x49, 0x81, 0xf8, 0xff, 7, 0, 0, 0x0f, 0x85, 2, 0, 0, 0, 0x0f, 0x0b,
            0x66, 0x49, 0x0f, 0x7e, 0xd8,
        ]
    );
    for form in FloatResultPolicyForm::ALL {
        let registers = if form.can_trap() {
            vec![1, 8]
        } else {
            vec![1, 8, 10, 15]
        };
        let bytes = encoded(form, &registers);
        for position in 0..bytes.len() {
            for bit in 0..8 {
                let mut corrupted = bytes.clone();
                corrupted[position] ^= 1 << bit;
                assert!(
                    validate_program(form, &registers, &corrupted).is_err(),
                    "{form:?} byte {position} bit {bit}"
                );
            }
        }
        assert!(validate_program(form, &registers, &bytes[..bytes.len() - 1]).is_err());
        let mut appended = bytes.clone();
        appended.push(0x90);
        assert!(validate_program(form, &registers, &appended).is_err());
        for other in FloatResultPolicyForm::ALL {
            if other != form && other.operand_count() == form.operand_count() {
                assert!(validate_program(other, &registers, &bytes).is_err());
            }
        }
        let mut other_registers = registers.clone();
        other_registers.swap(0, registers.len() - 1);
        assert!(validate_program(form, &other_registers, &bytes).is_err());
    }
}

/// Execute decoded bytes with real byte-relative branches, not the producer's
/// requested policy. Initial poison makes undeclared reads visible.
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
                    vectors[usize::from(vector)] = sized(general[usize::from(register)], wide);
                } else {
                    general[usize::from(register)] = sized(vectors[usize::from(vector)], wide);
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
            } => equal = sized(general[usize::from(register)], wide) == u64::from(immediate),
            Instruction::Conditional {
                equal: condition,
                displacement,
            } if equal == condition => {
                position = usize::try_from(position as i64 + i64::from(displacement)).unwrap();
            }
            Instruction::Conditional { .. } => {}
            Instruction::Jump { displacement } => {
                position = usize::try_from(position as i64 + i64::from(displacement)).unwrap()
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
fn float_result_policy_decoded_semantics_preserve_payloads_and_aliases() {
    for form in FloatResultPolicyForm::ALL {
        let wide = !form.is_binary32();
        let bits = |number: f64| {
            if wide {
                number.to_bits()
            } else {
                u64::from((number as f32).to_bits())
            }
        };
        let number = |payload| {
            if wide {
                f64::from_bits(payload)
            } else {
                f64::from(f32::from_bits(payload as u32))
            }
        };
        let maximum = if wide {
            f64::MAX.to_bits()
        } else {
            u64::from(f32::MAX.to_bits())
        };
        let nan_payloads = if wide {
            [0x7ff0_0000_0000_0001, 0xfff8_1234_5678_0000]
        } else {
            [0x7f80_0001, 0xffc1_2345]
        };
        let mut cases = vec![
            [maximum, maximum, bits(f64::INFINITY)],
            [bits(-1.0), maximum, bits(f64::NEG_INFINITY)],
            [bits(f64::INFINITY), bits(1.0), bits(f64::INFINITY)],
            [bits(1.0), bits(f64::NEG_INFINITY), bits(f64::NEG_INFINITY)],
            [bits(1.0), bits(0.0), bits(f64::INFINITY)],
            [bits(1.0), bits(-0.0), bits(f64::NEG_INFINITY)],
            [bits(1.0), 1, bits(f64::INFINITY)],
            [nan_payloads[0], bits(1.0), bits(f64::INFINITY)],
            [bits(1.0), nan_payloads[1], bits(f64::NEG_INFINITY)],
        ];
        for raw in [
            bits(0.0),
            bits(-0.0),
            1,
            maximum,
            bits(1.0),
            nan_payloads[0],
            nan_payloads[1],
        ] {
            cases.push([bits(1.0), bits(2.0), raw]);
        }
        for [left, right, raw] in cases {
            let expected = if form.can_trap() {
                number(raw).is_finite().then_some(raw)
            } else {
                let overflow = number(left).is_finite()
                    && number(right).is_finite()
                    && number(raw).is_infinite()
                    && (form.policy
                        != FloatResultPolicyKind::Saturating(
                            FloatSaturationRule::FiniteDivisionOverflow,
                        )
                        || number(right) != 0.0);
                Some(if overflow {
                    maximum | (raw & if wide { 1 << 63 } else { 1 << 31 })
                } else {
                    raw
                })
            };
            let mut inputs = if form.can_trap() {
                vec![raw]
            } else {
                vec![left, right, raw]
            };
            // A binary32 payload's unrelated upper half must not affect any predicate.
            if !wide {
                for input in &mut inputs {
                    *input |= 0xa5a5_5a5a_0000_0000;
                }
            }
            for first in [0, 8] {
                for output in first..=first + inputs.len() as u8 {
                    let mut registers = (first..first + inputs.len() as u8).collect::<Vec<_>>();
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
    let physical =
        register_model::validate_physical_register_model(crate::x86_64_physical_register_model())
            .unwrap();
    let constraints = crate::x86_64_register_constraint_catalog(&physical);
    let all = ["rcx", "r8", "r10", "r15"].map(|name| physical.model().view_named(name).unwrap().id);
    for form in FloatResultPolicyForm::ALL {
        let operands = &all[..form.operand_count() + 1];
        let kind = SelectedInstructionKind::FloatResultPolicy { form };
        let result =
            crate::encode_x86_64_selected_form(&physical, kind, key(form), operands).unwrap();
        crate::validate_x86_64_selected_form_encoding(
            &physical,
            kind,
            key(form),
            operands,
            result.bytes(),
        )
        .unwrap();
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
        assert!(result.footprint().encoded.implicit_unit_uses.is_empty());
        let scratch: &[&str] = if form.can_trap() {
            &["xmm3", "rflags"]
        } else {
            &["xmm3", "xmm4", "xmm5", "rflags"]
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
        assert_eq!(
            result.footprint().encoded.implicit_unit_clobbers,
            expected_clobbers
        );
        assert_eq!(
            constraint.clobbers,
            result.footprint().encoded.implicit_unit_clobbers
        );
        assert_eq!(
            declaration.alternatives[0].encoded,
            result.footprint().encoded
        );
        assert_eq!(
            declaration.alternatives[0].size,
            MachineSizeKnowledge::ExactBytes(result.bytes().len() as u16)
        );
        assert_eq!(
            result.footprint().encoded.control,
            if form.can_trap() {
                MachineEncodedControlEffect::FallThroughOrTrapV1
            } else {
                MachineEncodedControlEffect::FallThroughV1
            }
        );
        assert!(result.footprint().writes_rflags);
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
            crate::encode_x86_64_selected_form(&physical, kind, key(form), &operands[..1]).is_err()
        );
        let mut invalid = operands.to_vec();
        invalid[0] = physical.model().view_named("xmm3").unwrap().id;
        assert!(crate::encode_x86_64_selected_form(&physical, kind, key(form), &invalid).is_err());
    }
}
