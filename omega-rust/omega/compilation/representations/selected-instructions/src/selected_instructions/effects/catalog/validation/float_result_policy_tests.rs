//! Policy forms retain their exact input/output and conditional-trap surfaces.
use super::validate_declaration;
use crate::{
    FloatResultPolicyForm, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior,
};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
    RegisterUnitId,
};

fn fixture(
    form: FloatResultPolicyForm,
) -> (RegisterInstructionConstraint, MachineEffectDeclaration) {
    let input_count = form.operand_count() as u16;
    let constraint = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 42,
        },
        operands: (0..=input_count)
            .map(|operand| RegisterOperandConstraint {
                operand,
                access: if operand < input_count {
                    RegisterOperandAccess::Use
                } else {
                    RegisterOperandAccess::Def
                },
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            })
            .collect(),
        implicit_uses: vec![],
        implicit_defs: vec![],
        clobbers: vec![RegisterUnitId(1)],
    };
    let mut encoded =
        MachineEncodedEffects::fallthrough_v1((0..input_count).collect(), vec![input_count]);
    encoded.implicit_unit_clobbers = constraint.clobbers.clone();
    if form.can_trap() {
        encoded.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
        encoded.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
    }
    let semantic = MachineSemanticKind::FloatResultPolicy(form);
    let declaration = MachineEffectDeclaration {
        semantic,
        constraint: constraint.key,
        memory: MachineMemoryEffect::NoneV1,
        trap: if form.can_trap() {
            MachineTrapBehavior::TrappingIntegerV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        barrier: if form.can_trap() {
            MachineBarrier::ExternalEffect
        } else {
            MachineBarrier::None
        },
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(32),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded,
        }],
    };
    (constraint, declaration)
}

#[test]
fn float_result_policy_catalog_rejects_changed_dependencies_and_trap_surfaces() {
    for form in FloatResultPolicyForm::ALL {
        let (constraint, source) = fixture(form);
        validate_declaration(target::Architecture::X86_64, &constraint, &source).unwrap();
        for mutation in 0..10 {
            let mut changed = source.clone();
            let alternative = &mut changed.alternatives[0];
            match mutation {
                0 => {
                    changed.barrier = if form.can_trap() {
                        MachineBarrier::None
                    } else {
                        MachineBarrier::ExternalEffect
                    }
                }
                1 => {
                    changed.trap = if form.can_trap() {
                        MachineTrapBehavior::NeverV1
                    } else {
                        MachineTrapBehavior::TrappingIntegerV1
                    }
                }
                2 => {
                    alternative.encoded.trap = if form.can_trap() {
                        MachineEncodedTrapBehavior::NeverV1
                    } else {
                        MachineEncodedTrapBehavior::TrappingIntegerV1
                    }
                }
                3 => {
                    alternative.encoded.control = if form.can_trap() {
                        MachineEncodedControlEffect::FallThroughV1
                    } else {
                        MachineEncodedControlEffect::FallThroughOrTrapV1
                    }
                }
                4 => alternative.encoded.external_operand_reads.clear(),
                5 => alternative.encoded.external_operand_writes.clear(),
                6 => alternative.encoded.implicit_unit_clobbers.clear(),
                7 => {
                    alternative.key.family = MachineSemanticKind::FloatResultPolicy(
                        FloatResultPolicyForm::ALL
                            [(form.ordinal() as usize + 1) % FloatResultPolicyForm::COUNT],
                    )
                    .into()
                }
                8 => alternative
                    .encoded
                    .external_operand_reads
                    .push(form.operand_count() as u16),
                _ => {
                    alternative.encoded.external_operand_reads.reverse();
                    if form.can_trap() {
                        alternative.encoded.external_operand_reads[0] = 1;
                    }
                }
            }
            assert!(
                validate_declaration(target::Architecture::X86_64, &constraint, &changed).is_err(),
                "{form:?}, mutation {mutation}"
            );
        }
        // Even matching forged constraint/effect rows cannot change arity.
        let mut wrong_constraint = constraint.clone();
        let mut wrong_declaration = source.clone();
        wrong_constraint
            .operands
            .insert(0, wrong_constraint.operands[0].clone());
        wrong_declaration.alternatives[0]
            .encoded
            .external_operand_reads
            .insert(0, 0);
        assert!(
            validate_declaration(
                target::Architecture::X86_64,
                &wrong_constraint,
                &wrong_declaration
            )
            .is_err()
        );
    }
}
