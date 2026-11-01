use super::validate_declaration;
use crate::{
    AtomicForm, AtomicOperation, AtomicWidth, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineCallEffect,
    MachineCleanupEffect, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior,
};
use register_model::RegisterUnitId;
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
    RegisterViewId,
};
use target::Architecture::Aarch64;

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
    if matches!(
        form.operation,
        AtomicOperation::FetchAnd | AtomicOperation::FetchOr | AtomicOperation::FetchXor
    ) {
        encoded.control = MachineEncodedControlEffect::AtomicRetryUntilCommitV1;
    }
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

#[test]
fn retry_control_is_required_only_for_x86_bitwise_atomic_updates() {
    for form in AtomicForm::ALL {
        let (constraint, mut declared) = declaration(form);
        let bitwise = matches!(
            form.operation,
            AtomicOperation::FetchAnd | AtomicOperation::FetchOr | AtomicOperation::FetchXor
        );
        declared.alternatives[0].encoded.control = MachineEncodedControlEffect::FallThroughV1;
        assert_eq!(
            validate_declaration(target::Architecture::X86_64, &constraint, &declared).is_ok(),
            !bitwise
        );
        validate_declaration(Aarch64, &constraint, &declared)
            .expect("single LSE instruction falls through");
        declared.alternatives[0].encoded.control =
            MachineEncodedControlEffect::AtomicRetryUntilCommitV1;
        assert_eq!(
            validate_declaration(target::Architecture::X86_64, &constraint, &declared).is_ok(),
            bitwise
        );
        assert!(validate_declaration(Aarch64, &constraint, &declared).is_err());
    }
}

#[test]
fn retry_fixed_output_requires_every_input_to_have_a_distinct_fixed_home() {
    let form = AtomicForm {
        width: AtomicWidth::Bits32,
        operation: AtomicOperation::FetchOr,
    };
    let (mut constraint, declaration) = declaration(form);
    for (i, operand) in constraint.operands.iter_mut().enumerate() {
        operand.fixed_view = Some(RegisterViewId(i as u16));
        operand.early_clobber = false;
    }
    validate_declaration(target::Architecture::X86_64, &constraint, &declaration)
        .expect("fixed homes keep the replacement separate from inputs");
    for input in 0..2 {
        let mut missing = constraint.clone();
        missing.operands[input].fixed_view = None;
        assert!(
            validate_declaration(target::Architecture::X86_64, &missing, &declaration).is_err()
        );
        let mut aliased = constraint.clone();
        aliased.operands[input].fixed_view = constraint.operands[2].fixed_view;
        assert!(
            validate_declaration(target::Architecture::X86_64, &aliased, &declaration).is_err()
        );
    }
}

#[test]
fn atomic_fence_requires_ordering_without_register_or_storage_effects() {
    let (mut constraint, mut original) = declaration(AtomicForm::ALL[0]);
    constraint.operands.clear();
    original.semantic = MachineSemanticKind::AtomicFence;
    original.memory = MachineMemoryEffect::AtomicFenceV1;
    original.trap = MachineTrapBehavior::NeverV1;
    original.alternatives[0].key.family = MachineAlternativeFamily::AtomicFence;
    original.alternatives[0].encoded = MachineEncodedEffects::atomic_fence_v1();
    for architecture in [Aarch64, target::Architecture::X86_64] {
        validate_declaration(architecture, &constraint, &original).unwrap();
        for mutation in 0..10 {
            let mut changed = original.clone();
            let mut changed_constraint = constraint.clone();
            match mutation {
                0 => changed.barrier = MachineBarrier::None,
                1 => changed.memory = MachineMemoryEffect::NoneV1,
                2 => changed.alternatives[0].encoded.memory = MachineEncodedMemoryEffect::NoneV1,
                3 => {
                    changed.alternatives[0].encoded.memory =
                        MachineEncodedMemoryEffect::ReadPointerV1 {
                            pointer_operand: 0,
                            byte_count: 8,
                        }
                }
                4 => changed.alternatives[0]
                    .encoded
                    .external_operand_reads
                    .push(0),
                5 => changed.alternatives[0]
                    .encoded
                    .external_operand_writes
                    .push(0),
                6 => {
                    changed.alternatives[0].encoded.trap =
                        MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                }
                7 => changed_constraint.implicit_uses.push(RegisterUnitId(0)),
                8 => changed_constraint.implicit_defs.push(RegisterUnitId(0)),
                9 => changed_constraint.clobbers.push(RegisterUnitId(0)),
                _ => unreachable!(),
            }
            assert!(
                validate_declaration(architecture, &changed_constraint, &changed).is_err(),
                "{architecture:?}, mutation {mutation}"
            );
        }
    }
}
