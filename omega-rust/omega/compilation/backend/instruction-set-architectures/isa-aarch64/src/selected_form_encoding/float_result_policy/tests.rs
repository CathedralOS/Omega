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

/// Execute decoded fields, not the producer's requested form or plan. Branches
/// update the instruction position, and BRK has no returning scalar result.
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
        match decode_word(u32::from_le_bytes(instructions[position])).expect("decoded word") {
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
                general[usize::from(result)] =
                    (sized(general[usize::from(source)], wide) >> first) & ((1_u64 << count) - 1)
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
    // clang -target arm64-apple-macos: FMOV, UBFX, CMP, B.NE, BRK, FMOV.
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
            register_model::find_register_constraint(&constraints.constraints, key).unwrap();
        let declaration = declaration(form, key);
        let registers = (1..=form.operand_count() as u8 + 1).collect::<Vec<_>>();
        let operands = registers
            .iter()
            .map(|register| physical.view_named(&format!("x{register}")).unwrap().id)
            .collect::<Vec<_>>();
        let footprint = footprint(form, &operands);
        assert!(constraint.implicit_uses.is_empty());
        assert!(footprint.encoded.implicit_unit_uses.is_empty());
        // Trap touches one scratch vector; saturation saves eligibility and raw result.
        // Derive this roster independently, not from the producer's helper.
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
        let size = MachineSizeKnowledge::ExactBytes(encode(form, &registers).unwrap().len() as u16);
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
