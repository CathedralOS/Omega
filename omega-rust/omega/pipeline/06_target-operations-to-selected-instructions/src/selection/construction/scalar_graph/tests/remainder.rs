//! Raw projection controls; proof admission is exercised by legalization tests.

use super::super::{SelectedInstructionId, VirtualRegisterOrigin};
use super::{
    IntegerSign, IntegerType, IntegerValue, LegalizedScalarInstructionKind, ScalarType,
    SelectedFunction, SelectedInstructionKind, SelectedSelectionConstraints, ValueId, build,
    fixture,
};
#[test]
fn signed_remainder_preserves_operand_type_proof_and_scratch_custody() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        for bits in [8, 16, 32, 64] {
            let mut source = fixture(target, 0);
            let scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, bits).unwrap());
            source.blocks[0].instructions.truncate(3);
            source.provenance.operations.truncate(3);
            for instruction in &mut source.blocks[0].instructions {
                instruction.result.as_mut().unwrap().scalar_type = scalar_type;
            }
            source.blocks[0].instructions[0].kind =
                LegalizedScalarInstructionKind::Constant(IntegerValue::Signed(-7));
            source.blocks[0].instructions[1].kind =
                LegalizedScalarInstructionKind::Constant(IntegerValue::Signed(2));
            let obligation = semantic_vocabulary::ObligationId::new(1).unwrap();
            let accepted_fact =
                optimization_core::AcceptedObligationFactIdentity::from_bytes([41; 32]);
            source.blocks[0].instructions[2].kind =
                LegalizedScalarInstructionKind::WrappingRemainder {
                    left: ValueId::new(1).unwrap(),
                    right: ValueId::new(2).unwrap(),
                    obligation,
                    accepted_fact,
                };
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |candidate: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    &source,
                    candidate,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&selected).unwrap();
            assert_eq!(
                selected.blocks[0].instructions[2].kind,
                SelectedInstructionKind::WrappingRemainderI64 {
                    obligation,
                    accepted_fact
                }
            );
            for corruption in 0..6 {
                let mut changed = selected.clone();
                let instruction = &mut changed.blocks[0].instructions[2];
                match corruption {
                    0 => instruction.operands.swap(0, 1),
                    1 => {
                        instruction.kind = SelectedInstructionKind::ExactDivideU64 {
                            obligation,
                            accepted_fact,
                        }
                    }
                    2 => {
                        instruction.kind = SelectedInstructionKind::WrappingRemainderI64 {
                            obligation: semantic_vocabulary::ObligationId::new(2).unwrap(),
                            accepted_fact,
                        }
                    }
                    3 => {
                        instruction.kind = SelectedInstructionKind::WrappingRemainderI64 {
                            obligation,
                            accepted_fact:
                                optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                    [42; 32],
                                ),
                        }
                    }
                    4 => instruction.provenance.obligations.clear(),
                    _ => {
                        let output = instruction.operands[2].virtual_register;
                        changed.virtual_registers[output.0 as usize].scalar_type =
                            ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
                            );
                    }
                }
                assert!(
                    validate(&changed).is_err(),
                    "{target:?}/{bits}: corruption {corruption}"
                );
            }
            if target.architecture == target::Architecture::X86_64 {
                let mut changed = selected.clone();
                let scratch = changed.blocks[0].instructions[2].operands[3].virtual_register;
                changed.virtual_registers[scratch.0 as usize].origin =
                    VirtualRegisterOrigin::InstructionScratch {
                        instruction: SelectedInstructionId(2),
                        operand: 0,
                    };
                assert!(validate(&changed).is_err());
            }
            let mut same_operand_source = source.clone();
            if let LegalizedScalarInstructionKind::WrappingRemainder { left, right, .. } =
                &mut same_operand_source.blocks[0].instructions[2].kind
            {
                *right = *left;
            }
            let same_operand = build(
                0,
                &same_operand_source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            crate::selection::validation::scalar_graph::validate(
                0,
                &same_operand_source,
                &same_operand,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            if target.architecture == target::Architecture::X86_64 {
                // The divisor is pinned to RCX, so a shared dividend/divisor
                // register cannot satisfy RAX and RCX at once: the constructor
                // unshares it through a copy before the remainder.
                assert_eq!(
                    same_operand.blocks[0].instructions[2].kind,
                    SelectedInstructionKind::CopyI64
                );
                let remainder = &same_operand.blocks[0].instructions[3];
                assert_eq!(
                    remainder.kind,
                    SelectedInstructionKind::WrappingRemainderI64 {
                        obligation,
                        accepted_fact
                    }
                );
                assert_ne!(
                    remainder.operands[0].virtual_register,
                    remainder.operands[1].virtual_register
                );
                let mut changed = same_operand.clone();
                changed.blocks[0].instructions[2].kind = SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(0),
                };
                assert!(
                    crate::selection::validation::scalar_graph::validate(
                        0,
                        &same_operand_source,
                        &changed,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                    )
                    .is_err()
                );
            } else {
                assert_eq!(
                    same_operand.blocks[0].instructions[2].operands[0].virtual_register,
                    same_operand.blocks[0].instructions[2].operands[1].virtual_register
                );
            }
            source.blocks[0].instructions[1]
                .result
                .as_mut()
                .unwrap()
                .scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
            assert!(
                build(
                    0,
                    &source,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .is_err()
            );
        }
    }
}
