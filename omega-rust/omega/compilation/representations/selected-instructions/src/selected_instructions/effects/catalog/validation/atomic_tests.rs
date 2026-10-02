use super::validate_declaration;
use crate::{
    AtomicForm, AtomicOperation, AtomicWidth, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineCallEffect,
    MachineCleanupEffect, MachineEffectDeclaration, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
};

fn declaration(form: AtomicForm) -> (RegisterInstructionConstraint, MachineEffectDeclaration) {
    let key = RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: 80,
    };
    let constraint = RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key,
        operands: (0..form.operand_count())
            .map(|operand| {
                let output = Some(operand) == form.observed_operand();
                RegisterOperandConstraint {
                    operand,
                    access: if output {
                        RegisterOperandAccess::Def
                    } else {
                        RegisterOperandAccess::Use
                    },
                    class: RegisterClassId(0),
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: output,
                }
            })
            .collect(),
        implicit_uses: vec![],
        implicit_defs: vec![],
        clobbers: vec![],
    };
    let mut encoded = MachineEncodedEffects::fallthrough_v1(
        (0..form.operation.input_count()).collect(),
        form.observed_operand().into_iter().collect(),
    );
    encoded.memory = MachineEncodedMemoryEffect::AtomicPointerV1 {
        pointer_operand: 0,
        form,
    };
    encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    let declaration = MachineEffectDeclaration {
        semantic: MachineSemanticKind::AtomicAccess(form),
        constraint: key,
        memory: MachineMemoryEffect::AtomicPointerV1,
        trap: MachineTrapBehavior::MayArchitecturalFaultV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::AtomicAccess(form),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded,
        }],
    };
    (constraint, declaration)
}

#[test]
fn atomic_forms_require_indivisible_memory_and_exact_operand_custody() {
    for form in AtomicForm::ALL {
        let (constraint, original) = declaration(form);
        validate_declaration(target::Architecture::X86_64, &constraint, &original)
            .expect("complete atomic effects");
        for mutation in 0..10 {
            let mut changed = original.clone();
            let mut changed_constraint = constraint.clone();
            let encoded = &mut changed.alternatives[0].encoded;
            match mutation {
                0 => changed.memory = MachineMemoryEffect::ReadPointerV1,
                1 => changed.barrier = MachineBarrier::None,
                2 => {
                    encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: u16::from(form.width.bytes()),
                    }
                }
                3 => {
                    encoded.memory = MachineEncodedMemoryEffect::AtomicPointerV1 {
                        pointer_operand: 1,
                        form,
                    }
                }
                4 => {
                    encoded.memory = MachineEncodedMemoryEffect::AtomicPointerV1 {
                        pointer_operand: 0,
                        form: AtomicForm {
                            width: if form.width == AtomicWidth::Bits8 {
                                AtomicWidth::Bits16
                            } else {
                                AtomicWidth::Bits8
                            },
                            ..form
                        },
                    }
                }
                5 => encoded.external_operand_reads.clear(),
                6 => encoded.external_operand_writes.push(form.operand_count()),
                7 => encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                8 => changed_constraint.operands[0].early_clobber = true,
                9 => {
                    changed.alternatives[0].key.family =
                        MachineAlternativeFamily::AtomicAccess(AtomicForm {
                            operation: if form.operation == AtomicOperation::Load {
                                AtomicOperation::Store
                            } else {
                                AtomicOperation::Load
                            },
                            ..form
                        })
                }
                _ => unreachable!(),
            }
            assert!(
                validate_declaration(target::Architecture::X86_64, &changed_constraint, &changed)
                    .is_err(),
                "{form:?}, mutation {mutation}"
            );
        }
        if let Some(output) = form.observed_operand() {
            let mut changed = constraint.clone();
            changed.operands[usize::from(output)].early_clobber = false;
            assert!(
                validate_declaration(target::Architecture::X86_64, &changed, &original).is_err()
            );
            changed.operands[usize::from(output)].early_clobber = true;
            changed.operands[usize::from(output)].tied_to = Some(0);
            assert!(
                validate_declaration(target::Architecture::X86_64, &changed, &original).is_err()
            );
        }
    }
}
