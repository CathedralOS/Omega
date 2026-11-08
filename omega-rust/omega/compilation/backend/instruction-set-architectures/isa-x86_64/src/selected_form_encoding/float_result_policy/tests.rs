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
    // The request check refuses a wrong-arity roster before reading its plan.
    registers.len() == form.operand_count() + 1 && validate_program(form, registers, bytes).is_ok()
}

fn roster(form: FloatResultPolicyForm) -> Vec<u8> {
    if form.can_trap() {
        vec![1, 8]
    } else {
        // Two inputs and the result; the old fourth slot was never read.
        vec![1, 8, 10]
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
fn float_result_policy_trapping_bytes_bind_width_registers_and_branches() {
    let form = FloatResultPolicyForm {
        format: IeeeFloatFormat::Binary64,
        policy: FloatResultPolicyKind::Trapping,
    };
    // movq xmm3, rcx; movq r8, xmm3; shl r8, 1; shr r8, 53;
    // cmp r8, 2047; jne +2; ud2; movq r8, xmm3.
    policy::exact_stream(
        form,
        &[1, 8],
        &[
            0x66, 0x48, 0x0f, 0x6e, 0xd9, 0x66, 0x49, 0x0f, 0x7e, 0xd8, 0x49, 0xc1, 0xe0, 1, 0x49,
            0xc1, 0xe8, 53, 0x49, 0x81, 0xf8, 0xff, 7, 0, 0, 0x0f, 0x85, 2, 0, 0, 0, 0x0f, 0x0b,
            0x66, 0x49, 0x0f, 0x7e, 0xd8,
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
            &["xmm3", "rflags"]
        } else {
            &["xmm3", "xmm4", "rflags"]
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
            crate::encode_x86_64_selected_form(&physical, kind, key(form), &operands[..1]).is_err()
        );
        let mut invalid = operands.to_vec();
        invalid[0] = physical.model().view_named("xmm3").unwrap().id;
        assert!(crate::encode_x86_64_selected_form(&physical, kind, key(form), &invalid).is_err());
    }
}
