/*
  Admission validation (`validate_declaration`, `validate_isa_machine_effect_catalog`)
  for `MachineEffectCatalog` declarations against target register constraints
  and ISA encodings.
*/

use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
};

#[cfg(test)]
mod atomic_tests {
    use super::validate_declaration;
    use crate::{
        AtomicForm, AtomicOperation, AtomicWidth, MachineAlternative,
        MachineAlternativeApplicability, MachineAlternativeFamily, MachineAlternativeKey,
        MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
        MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect,
        MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
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
                    let output = form.output_operands().contains(&operand);
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
            form.output_operands().collect(),
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
                    validate_declaration(
                        target::Architecture::X86_64,
                        &changed_constraint,
                        &changed
                    )
                    .is_err(),
                    "{form:?}, mutation {mutation}"
                );
            }
            if let Some(output) = form.observed_operand() {
                let mut changed = constraint.clone();
                changed.operands[usize::from(output)].early_clobber = false;
                assert!(
                    validate_declaration(target::Architecture::X86_64, &changed, &original)
                        .is_err()
                );
                changed.operands[usize::from(output)].early_clobber = true;
                changed.operands[usize::from(output)].tied_to = Some(0);
                assert!(
                    validate_declaration(target::Architecture::X86_64, &changed, &original)
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn retry_control_matches_the_target_atomic_realization() {
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
            assert_eq!(
                validate_declaration(Aarch64, &constraint, &declared).is_ok(),
                !form.operation.retries_until_commit(Aarch64)
            );
            declared.alternatives[0].encoded.control =
                MachineEncodedControlEffect::AtomicRetryUntilCommitV1;
            assert_eq!(
                validate_declaration(target::Architecture::X86_64, &constraint, &declared).is_ok(),
                bitwise
            );
            assert_eq!(
                validate_declaration(Aarch64, &constraint, &declared).is_ok(),
                form.operation.retries_until_commit(Aarch64)
            );
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
                    2 => {
                        changed.alternatives[0].encoded.memory = MachineEncodedMemoryEffect::NoneV1
                    }
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
}
#[cfg(test)]
mod float_result_policy_tests {
    use super::validate_declaration;
    use crate::{
        FloatResultPolicyForm, MachineAlternative, MachineAlternativeApplicability,
        MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
        MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect,
        MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
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
                    validate_declaration(target::Architecture::X86_64, &constraint, &changed)
                        .is_err(),
                    "{form:?}, mutation {mutation}"
                );
            }
            let mut wrong_constraint = constraint.clone();
            let mut wrong_declaration = source.clone();
            wrong_constraint
                .operands
                .insert(0, wrong_constraint.operands[0]);
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
}
#[cfg(test)]
mod tests {
    use super::{
        MachineAlternativeApplicability, MachineBarrier, MachineEffectCatalogValidationError,
        MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
        MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
        MachineSemanticKind, MachineSizeKnowledge, RegisterInstructionConstraint,
        RegisterOperandAccess, validate_declaration,
    };
    use crate::MachineMemoryEffect::NoneV1;
    use crate::MachineTrapBehavior::NeverV1;
    use crate::{MachineAlternative, MachineAlternativeKey, MachineLatencyKnowledge};
    use register_model::RegisterUnitId;
    use register_model::{
        RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
        RegisterOperandConstraint, RegisterViewId,
    };

    fn instruction_constraint(
        variant: u32,
        accesses: &[RegisterOperandAccess],
    ) -> RegisterInstructionConstraint {
        RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: RegisterConstraintKey::instruction(variant),
            operands: accesses
                .iter()
                .enumerate()
                .map(|(operand, access)| operand_row(operand as u16, *access))
                .collect(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        }
    }

    fn operand_row(operand: u16, access: RegisterOperandAccess) -> RegisterOperandConstraint {
        RegisterOperandConstraint {
            operand,
            access,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }
    }

    fn always_alternative(
        family: impl Into<crate::MachineAlternativeFamily>,
        bytes: u16,
        encoded: MachineEncodedEffects,
    ) -> MachineAlternative {
        MachineAlternative {
            key: MachineAlternativeKey {
                family: family.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(bytes),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded,
        }
    }

    #[test]
    fn float_conversion_policy_traps_require_the_exact_control_surface() {
        use crate::{
            IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatFormat,
            IeeeFloatIntegerCarrier, IeeeFloatToIntegerDomain,
        };
        let mut constraint = instruction_constraint(
            42,
            &[RegisterOperandAccess::Use, RegisterOperandAccess::Def],
        );
        constraint.clobbers = vec![RegisterUnitId(1)];
        let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]);
        encoded.implicit_unit_clobbers = constraint.clobbers.clone();
        encoded.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
        encoded.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
        let semantic = |domain| {
            MachineSemanticKind::NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm {
                operation: IeeeFloatArithmeticOperation::TruncateToInteger {
                    target: IeeeFloatIntegerCarrier::I32,
                    domain,
                },
                format: IeeeFloatFormat::Binary64,
            })
        };
        let source = MachineEffectDeclaration {
            semantic: semantic(IeeeFloatToIntegerDomain::Trapping),
            constraint: constraint.key,
            memory: NoneV1,
            trap: crate::MachineTrapBehavior::TrappingIntegerV1,
            barrier: MachineBarrier::ExternalEffect,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![always_alternative(
                semantic(IeeeFloatToIntegerDomain::Trapping),
                32,
                encoded,
            )],
        };
        validate_declaration(target::Architecture::X86_64, &constraint, &source).unwrap();
        for mutation in 0..7 {
            let mut changed = source.clone();
            match mutation {
                0 => changed.barrier = MachineBarrier::None,
                1 => changed.trap = NeverV1,
                2 => changed.alternatives[0].encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                3 => {
                    changed.alternatives[0].encoded.control =
                        MachineEncodedControlEffect::FallThroughV1
                }
                4 => changed.alternatives[0]
                    .encoded
                    .implicit_unit_clobbers
                    .clear(),
                _ => {
                    changed.semantic = semantic(if mutation == 5 {
                        IeeeFloatToIntegerDomain::Exact
                    } else {
                        IeeeFloatToIntegerDomain::Saturating
                    });
                    changed.alternatives[0].key.family = changed.semantic.into();
                }
            }
            assert!(
                validate_declaration(target::Architecture::X86_64, &constraint, &changed).is_err(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn byte_copy_catalog_requires_dynamic_operands_and_early_clobbers() {
        let mut constraint = instruction_constraint(
            42,
            &[
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
                RegisterOperandAccess::Def,
            ],
        );
        for row in constraint.operands.iter_mut().skip(3) {
            row.early_clobber = true;
        }
        constraint.clobbers = vec![RegisterUnitId(0)];
        let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0, 1, 2], vec![3, 4]);
        encoded.memory = MachineEncodedMemoryEffect::CopyBytesV1 {
            source_pointer_operand: 0,
            destination_pointer_operand: 1,
            count_operand: 2,
        };
        encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        encoded.implicit_unit_clobbers = constraint.clobbers.clone();
        let source = MachineEffectDeclaration {
            semantic: MachineSemanticKind::CopyBytes,
            constraint: constraint.key,
            memory: crate::MachineMemoryEffect::CopyBytesV1,
            trap: crate::MachineTrapBehavior::MayArchitecturalFaultV1,
            barrier: MachineBarrier::None,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![always_alternative(
                MachineSemanticKind::CopyBytes,
                28,
                encoded,
            )],
        };
        validate_declaration(target::Architecture::X86_64, &constraint, &source).unwrap();
        for mutation in 0..8 {
            let mut changed = source.clone();
            let encoded = &mut changed.alternatives[0].encoded;
            match mutation {
                0 => {
                    changed.memory = NoneV1;
                    encoded.memory = MachineEncodedMemoryEffect::NoneV1;
                }
                1 => {
                    encoded.memory = MachineEncodedMemoryEffect::CopyBytesV1 {
                        source_pointer_operand: 1,
                        destination_pointer_operand: 0,
                        count_operand: 2,
                    }
                }
                2 => {
                    encoded.memory = MachineEncodedMemoryEffect::CopyBytesV1 {
                        source_pointer_operand: 0,
                        destination_pointer_operand: 1,
                        count_operand: 3,
                    }
                }
                3 => encoded.implicit_unit_clobbers.clear(),
                4 => encoded.external_operand_reads.truncate(2),
                5 => encoded.external_operand_writes.truncate(1),
                6 => encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                _ => encoded.external_operand_writes.push(0),
            }
            assert!(
                validate_declaration(target::Architecture::X86_64, &constraint, &changed).is_err(),
                "mutation {mutation}"
            );
        }
        for operand in 0..5 {
            let mut changed = constraint.clone();
            changed.operands[operand].early_clobber = !changed.operands[operand].early_clobber;
            assert!(validate_declaration(target::Architecture::X86_64, &changed, &source).is_err());
        }
    }

    #[test]
    fn zero_fill_catalog_requires_dynamic_operands_and_early_clobber_cursor() {
        let mut constraint = instruction_constraint(
            43,
            &[
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
            ],
        );
        for row in constraint.operands.iter_mut().skip(2) {
            row.early_clobber = true;
        }
        constraint.clobbers = vec![RegisterUnitId(0)];
        let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2]);
        encoded.memory = MachineEncodedMemoryEffect::ZeroBytesV1 {
            destination_pointer_operand: 0,
            count_operand: 1,
        };
        encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        encoded.implicit_unit_clobbers = constraint.clobbers.clone();
        let source = MachineEffectDeclaration {
            semantic: MachineSemanticKind::ZeroBytes,
            constraint: constraint.key,
            memory: crate::MachineMemoryEffect::ZeroBytesV1,
            trap: crate::MachineTrapBehavior::MayArchitecturalFaultV1,
            barrier: MachineBarrier::None,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![always_alternative(
                MachineSemanticKind::ZeroBytes,
                24,
                encoded,
            )],
        };
        validate_declaration(target::Architecture::Aarch64, &constraint, &source).unwrap();
        for mutation in 0..9 {
            let mut changed = source.clone();
            let encoded = &mut changed.alternatives[0].encoded;
            match mutation {
                0 => {
                    changed.memory = NoneV1;
                    encoded.memory = MachineEncodedMemoryEffect::NoneV1;
                }
                1 => {
                    encoded.memory = MachineEncodedMemoryEffect::ZeroBytesV1 {
                        destination_pointer_operand: 1,
                        count_operand: 0,
                    }
                }
                2 => {
                    encoded.memory = MachineEncodedMemoryEffect::ZeroBytesV1 {
                        destination_pointer_operand: 0,
                        count_operand: 2,
                    }
                }
                3 => encoded.implicit_unit_clobbers.clear(),
                4 => encoded.external_operand_reads.truncate(1),
                5 => encoded.external_operand_writes.clear(),
                6 => encoded.trap = MachineEncodedTrapBehavior::NeverV1,
                7 => {
                    changed.memory = crate::MachineMemoryEffect::CopyBytesV1;
                }
                _ => encoded.external_operand_writes.push(0),
            }
            assert!(
                validate_declaration(target::Architecture::Aarch64, &constraint, &changed).is_err(),
                "mutation {mutation}"
            );
        }
        for operand in 0..3 {
            let mut changed = constraint.clone();
            changed.operands[operand].early_clobber = !changed.operands[operand].early_clobber;
            assert!(
                validate_declaration(target::Architecture::Aarch64, &changed, &source).is_err()
            );
        }
    }

    #[test]
    fn pointer_load_catalog_requires_the_semantic_exact_width() {
        let constraint = instruction_constraint(
            32,
            &[RegisterOperandAccess::Use, RegisterOperandAccess::Def],
        );
        for (semantic, byte_count) in [
            (MachineSemanticKind::LoadPacked3, 3),
            (MachineSemanticKind::LoadPacked5, 5),
            (MachineSemanticKind::LoadPacked6, 6),
            (MachineSemanticKind::LoadPacked7, 7),
            (MachineSemanticKind::Load8, 1),
            (MachineSemanticKind::Load16, 2),
            (MachineSemanticKind::Load32, 4),
            (MachineSemanticKind::Load64, 8),
        ] {
            let mut constraint = constraint.clone();
            let packed = matches!(byte_count, 3 | 5 | 6 | 7);
            if packed {
                constraint.operands[1].early_clobber = true;
                let mut scratch = constraint.operands[1];
                scratch.operand = 2;
                constraint.operands.push(scratch);
            }
            let mut encoded = MachineEncodedEffects::fallthrough_v1(
                vec![0],
                if packed { vec![1, 2] } else { vec![1] },
            );
            encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count,
            };
            encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            let mut declaration = MachineEffectDeclaration {
                semantic,
                constraint: constraint.key,
                memory: crate::MachineMemoryEffect::ReadPointerV1,
                trap: crate::MachineTrapBehavior::MayArchitecturalFaultV1,
                barrier: MachineBarrier::None,
                call: crate::MachineCallEffect::NoneV1,
                cleanup: crate::MachineCleanupEffect::NoneV1,
                alternatives: vec![always_alternative(semantic, 4, encoded)],
            };
            validate_declaration(target::Architecture::X86_64, &constraint, &declaration).unwrap();
            if packed {
                for operand in [1, 2] {
                    let mut weakened = constraint.clone();
                    weakened.operands[operand].early_clobber = false;
                    assert!(
                        validate_declaration(target::Architecture::X86_64, &weakened, &declaration)
                            .is_err()
                    );
                }
                let mut stripped = declaration.clone();
                stripped.alternatives[0]
                    .encoded
                    .external_operand_writes
                    .pop();
                assert!(
                    validate_declaration(target::Architecture::X86_64, &constraint, &stripped)
                        .is_err()
                );
            }
            declaration.alternatives[0].encoded.memory =
                MachineEncodedMemoryEffect::ReadPointerV1 {
                    pointer_operand: 0,
                    byte_count: if byte_count == 4 { 8 } else { 4 },
                };
            assert!(
                validate_declaration(target::Architecture::X86_64, &constraint, &declaration)
                    .is_err()
            );
        }
    }

    #[test]
    fn linux_write_catalog_rejects_stripped_memory_trap_and_operand_effects() {
        let constraint = instruction_constraint(31, &[RegisterOperandAccess::Use]);
        let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0], Vec::new());
        encoded.memory = MachineEncodedMemoryEffect::HostedWriteByteV1 {
            stack_pointer: RegisterViewId(7),
        };
        encoded.control = MachineEncodedControlEffect::HostedWriteReturnOrTrapV1;
        encoded.trap = MachineEncodedTrapBehavior::HostedWriteFailureV1;
        let source = MachineEffectDeclaration {
            semantic: MachineSemanticKind::HostedWriteByteI32,
            constraint: constraint.key,
            memory: crate::MachineMemoryEffect::HostedWriteByteV1,
            trap: crate::MachineTrapBehavior::HostedWriteFailureV1,
            barrier: MachineBarrier::ExternalEffect,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![always_alternative(
                MachineSemanticKind::HostedWriteByteI32,
                40,
                encoded,
            )],
        };
        validate_declaration(target::Architecture::X86_64, &constraint, &source).unwrap();
        for mutation in 0..5 {
            let mut changed = source.clone();
            match mutation {
                0 => {
                    changed.memory = NoneV1;
                    changed.alternatives[0].encoded.memory = MachineEncodedMemoryEffect::NoneV1;
                }
                1 => {
                    changed.trap = NeverV1;
                    changed.alternatives[0].encoded.trap = MachineEncodedTrapBehavior::NeverV1;
                }
                2 => changed.alternatives[0]
                    .encoded
                    .external_operand_reads
                    .clear(),
                3 => changed.alternatives[0]
                    .encoded
                    .external_operand_writes
                    .push(0),
                _ => {
                    changed.alternatives[0].encoded.control =
                        MachineEncodedControlEffect::FallThroughV1
                }
            }
            assert!(
                validate_declaration(target::Architecture::X86_64, &constraint, &changed).is_err()
            );
        }
    }

    #[test]
    fn control_flow_rows_pin_their_encoded_control_shape() {
        let constraint = instruction_constraint(7, &[]);
        let declaration = |semantic, control| {
            let mut encoded = MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
            encoded.control = control;
            encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            MachineEffectDeclaration {
                semantic,
                constraint: constraint.key,
                memory: NoneV1,
                trap: NeverV1,
                barrier: MachineBarrier::ControlFlow,
                call: crate::MachineCallEffect::NoneV1,
                cleanup: crate::MachineCleanupEffect::NoneV1,
                alternatives: vec![always_alternative(semantic, 5, encoded)],
            }
        };
        validate_declaration(
            target::Architecture::X86_64,
            &constraint,
            &declaration(
                MachineSemanticKind::Jump,
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
            ),
        )
        .unwrap();
        validate_declaration(
            target::Architecture::X86_64,
            &constraint,
            &declaration(
                MachineSemanticKind::ConditionalBranchNonZero,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            ),
        )
        .unwrap();
        for (semantic, control) in [
            (
                MachineSemanticKind::Jump,
                MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            ),
            (
                MachineSemanticKind::Jump,
                MachineEncodedControlEffect::ReturnFromActivationStackV1,
            ),
            (
                MachineSemanticKind::ConditionalBranchNonZero,
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
            ),
            (
                MachineSemanticKind::ConditionalBranchU64LessThan,
                MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                    target: RegisterViewId(9),
                },
            ),
        ] {
            assert!(
                validate_declaration(
                    target::Architecture::X86_64,
                    &constraint,
                    &declaration(semantic, control)
                )
                .is_err(),
                "{semantic:?} borrowed control shape {control:?}"
            );
        }
        let mut hosted_trap = declaration(
            MachineSemanticKind::Jump,
            MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
        );
        hosted_trap.alternatives[0].encoded.trap = MachineEncodedTrapBehavior::HostedExitReturnedV1;
        assert!(
            validate_declaration(target::Architecture::X86_64, &constraint, &hosted_trap).is_err()
        );
    }

    #[test]
    fn returns_and_calls_reject_each_others_stack_effects() {
        let stack_pointer = RegisterViewId(5);
        let constraint =
            |variant, access: RegisterOperandAccess| instruction_constraint(variant, &[access]);
        let declaration = |semantic, key, barrier, reads, writes, memory, stack, control| {
            let mut encoded = MachineEncodedEffects::fallthrough_v1(reads, writes);
            encoded.memory = memory;
            encoded.stack = stack;
            encoded.control = control;
            encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            MachineEffectDeclaration {
                semantic,
                constraint: key,
                memory: NoneV1,
                trap: NeverV1,
                barrier,
                call: if barrier == MachineBarrier::Call {
                    crate::MachineCallEffect::InternalNormalReturnV1 {
                        pre_call_stack_alignment: 16,
                    }
                } else {
                    crate::MachineCallEffect::NoneV1
                },
                cleanup: crate::MachineCleanupEffect::NoneV1,
                alternatives: vec![always_alternative(semantic, 5, encoded)],
            }
        };
        let activation_stack = (
            MachineEncodedMemoryEffect::ReadActivationStackV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer,
                byte_count: 8,
            },
        );
        let call_return_address = (
            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                stack_pointer,
                return_address_byte_count: 8,
            },
        );
        let return_constraint = constraint(11, RegisterOperandAccess::Use);
        validate_declaration(
            target::Architecture::X86_64,
            &return_constraint,
            &declaration(
                MachineSemanticKind::ReturnScalar,
                return_constraint.key,
                MachineBarrier::ControlFlow,
                Vec::new(),
                Vec::new(),
                activation_stack.0,
                activation_stack.1,
                MachineEncodedControlEffect::ReturnFromActivationStackV1,
            ),
        )
        .unwrap();
        assert!(
            validate_declaration(
                target::Architecture::X86_64,
                &return_constraint,
                &declaration(
                    MachineSemanticKind::ReturnScalar,
                    return_constraint.key,
                    MachineBarrier::ControlFlow,
                    Vec::new(),
                    Vec::new(),
                    call_return_address.0,
                    call_return_address.1,
                    MachineEncodedControlEffect::ReturnFromActivationStackV1,
                ),
            )
            .is_err()
        );
        let call_constraint = constraint(13, RegisterOperandAccess::Def);
        validate_declaration(
            target::Architecture::X86_64,
            &call_constraint,
            &declaration(
                MachineSemanticKind::CallScalar,
                call_constraint.key,
                MachineBarrier::Call,
                Vec::new(),
                vec![0],
                call_return_address.0,
                call_return_address.1,
                MachineEncodedControlEffect::DirectRelativeCallV1,
            ),
        )
        .unwrap();
        assert!(
            validate_declaration(
                target::Architecture::X86_64,
                &call_constraint,
                &declaration(
                    MachineSemanticKind::CallScalar,
                    call_constraint.key,
                    MachineBarrier::Call,
                    Vec::new(),
                    vec![0],
                    activation_stack.0,
                    activation_stack.1,
                    MachineEncodedControlEffect::DirectRelativeCallV1,
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn explicit_crash_requires_operand_free_unconditional_no_cleanup_effects() {
        let constraint = RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: RegisterConstraintKey {
                family: RegisterConstraintFamily::Instruction,
                variant: 780,
            },
            operands: Vec::new(),
            implicit_uses: vec![RegisterUnitId(7)],
            implicit_defs: vec![RegisterUnitId(7)],
            clobbers: Vec::new(),
        };
        let source = MachineEffectDeclaration {
            semantic: MachineSemanticKind::Crash,
            constraint: constraint.key,
            memory: NoneV1,
            trap: crate::MachineTrapBehavior::ExplicitCrashV1,
            barrier: MachineBarrier::ControlFlow,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![always_alternative(
                crate::MachineAlternativeFamily::Crash,
                2,
                MachineEncodedEffects {
                    trap: MachineEncodedTrapBehavior::ExplicitCrashV1,
                    control: MachineEncodedControlEffect::CrashV1,
                    implicit_unit_uses: constraint.implicit_uses.clone(),
                    implicit_unit_defs: constraint.implicit_defs.clone(),
                    ..MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new())
                },
            )],
        };
        validate_declaration(target::Architecture::X86_64, &constraint, &source).unwrap();
        for mutation in 0..13 {
            let mut candidate = source.clone();
            let effects = &mut candidate.alternatives[0].encoded;
            match mutation {
                0 => candidate.trap = NeverV1,
                1 => candidate.barrier = MachineBarrier::None,
                2 => {
                    candidate.call = crate::MachineCallEffect::InternalNormalReturnV1 {
                        pre_call_stack_alignment: 16,
                    }
                }
                3 => effects.trap = MachineEncodedTrapBehavior::HostedExitReturnedV1,
                4 => effects.control = MachineEncodedControlEffect::FallThroughV1,
                5 => effects.control = MachineEncodedControlEffect::ReturnFromActivationStackV1,
                6 => effects.external_operand_reads.push(0),
                7 => effects.external_operand_writes.push(0),
                8 => effects.implicit_unit_clobbers.push(RegisterUnitId(0)),
                9 => {
                    effects.stack = MachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer: RegisterViewId(0),
                        byte_count: 8,
                    }
                }
                10 => candidate.semantic = MachineSemanticKind::ReturnUnit,
                11 => effects.implicit_unit_uses.clear(),
                12 => effects.implicit_unit_defs.clear(),
                _ => unreachable!(),
            }
            assert!(
                validate_declaration(target::Architecture::X86_64, &constraint, &candidate)
                    .is_err(),
                "mutation {mutation}"
            );
        }
        let mut changed_constraint = constraint;
        changed_constraint.operands.push(RegisterOperandConstraint {
            operand: 0,
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
        assert!(
            validate_declaration(target::Architecture::X86_64, &changed_constraint, &source)
                .is_err()
        );
    }

    #[test]
    fn indexed_and_frame_memory_rows_reject_foreign_semantics() {
        let constraint = instruction_constraint(
            13,
            &[
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
            ],
        );
        let mut indexed = MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2]);
        indexed.memory = MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
            pointer_operand: 0,
            index_operand: 1,
            byte_count: 1,
        };
        indexed.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        let declaration =
            |semantic, memory, trap, encoded: &MachineEncodedEffects| MachineEffectDeclaration {
                semantic,
                constraint: constraint.key,
                memory,
                trap,
                barrier: MachineBarrier::None,
                call: crate::MachineCallEffect::NoneV1,
                cleanup: crate::MachineCleanupEffect::NoneV1,
                alternatives: vec![always_alternative(semantic, 9, encoded.clone())],
            };
        validate_declaration(
            target::Architecture::X86_64,
            &constraint,
            &declaration(
                MachineSemanticKind::Load8Indexed,
                crate::MachineMemoryEffect::ReadPointerV1,
                crate::MachineTrapBehavior::MayArchitecturalFaultV1,
                &indexed,
            ),
        )
        .unwrap();
        for semantic in [
            MachineSemanticKind::Load64,
            MachineSemanticKind::ExactAddI64,
        ] {
            assert!(
                validate_declaration(
                    target::Architecture::X86_64,
                    &constraint,
                    &declaration(
                        semantic,
                        crate::MachineMemoryEffect::ReadPointerV1,
                        crate::MachineTrapBehavior::MayArchitecturalFaultV1,
                        &indexed,
                    ),
                )
                .is_err(),
                "{semantic:?} borrowed the indexed-load footprint"
            );
        }
        let mut pointer_read = MachineEncodedEffects::fallthrough_v1(vec![0], vec![2]);
        pointer_read.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand: 0,
            byte_count: 8,
        };
        pointer_read.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        assert!(
            validate_declaration(
                target::Architecture::X86_64,
                &constraint,
                &declaration(
                    MachineSemanticKind::Load64,
                    crate::MachineMemoryEffect::ReadPointerV1,
                    NeverV1,
                    &pointer_read,
                ),
            )
            .is_err()
        );
        assert!(
            validate_declaration(
                target::Architecture::X86_64,
                &constraint,
                &declaration(
                    MachineSemanticKind::ExactAddI64,
                    NoneV1,
                    crate::MachineTrapBehavior::HostedReadFailureV1,
                    &MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2]),
                ),
            )
            .is_err()
        );
        let mut store_constraint = instruction_constraint(29, &[RegisterOperandAccess::Use]);
        store_constraint.implicit_uses = vec![RegisterUnitId(9)];
        let mut frame_store = MachineEncodedEffects::fallthrough_v1(vec![0], Vec::new());
        frame_store.memory = MachineEncodedMemoryEffect::WriteFrameStorageV1 {
            stack_pointer: RegisterViewId(5),
            byte_count: 8,
        };
        frame_store.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        frame_store.implicit_unit_uses = store_constraint.implicit_uses.clone();
        validate_declaration(
            target::Architecture::X86_64,
            &store_constraint,
            &declaration(
                MachineSemanticKind::Store64,
                crate::MachineMemoryEffect::WriteFrameStorageV1,
                crate::MachineTrapBehavior::MayArchitecturalFaultV1,
                &frame_store,
            ),
        )
        .unwrap();
        let offset_constraint = instruction_constraint(
            31,
            &[RegisterOperandAccess::Use, RegisterOperandAccess::Def],
        );
        let mut borrowed = MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]);
        borrowed.memory = frame_store.memory;
        borrowed.trap = frame_store.trap;
        assert!(
            validate_declaration(
                target::Architecture::X86_64,
                &offset_constraint,
                &declaration(
                    MachineSemanticKind::AddressOffset,
                    crate::MachineMemoryEffect::WriteFrameStorageV1,
                    crate::MachineTrapBehavior::MayArchitecturalFaultV1,
                    &borrowed,
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn encoded_rows_restate_the_contracted_operand_custody() {
        let constraint = instruction_constraint;
        let declaration = |semantic, key, applicability, reads: Vec<u16>, writes: Vec<u16>| {
            MachineEffectDeclaration {
                semantic,
                constraint: key,
                memory: NoneV1,
                trap: NeverV1,
                barrier: MachineBarrier::None,
                call: crate::MachineCallEffect::NoneV1,
                cleanup: crate::MachineCleanupEffect::NoneV1,
                alternatives: vec![MachineAlternative {
                    key: MachineAlternativeKey {
                        family: semantic.into(),
                        variant: 0,
                    },
                    applicability,
                    size: MachineSizeKnowledge::ExactBytes(4),
                    latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                    encoded: MachineEncodedEffects::fallthrough_v1(reads, writes),
                }],
            }
        };
        let arithmetic = constraint(
            41,
            &[
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
            ],
        );
        validate_declaration(
            target::Architecture::X86_64,
            &arithmetic,
            &declaration(
                MachineSemanticKind::ExactAddI64,
                arithmetic.key,
                MachineAlternativeApplicability::Always,
                vec![0, 1],
                vec![2],
            ),
        )
        .unwrap();
        for (reads, writes) in [
            (vec![0], vec![2]),
            (vec![1], vec![2]),
            (vec![0, 1], vec![]),
            (vec![0, 1], vec![0]),
            (vec![0, 1, 2], vec![2]),
        ] {
            assert!(
                validate_declaration(
                    target::Architecture::X86_64,
                    &arithmetic,
                    &declaration(
                        MachineSemanticKind::ExactAddI64,
                        arithmetic.key,
                        MachineAlternativeApplicability::Always,
                        reads.clone(),
                        writes.clone(),
                    ),
                )
                .is_err(),
                "reads {reads:?} writes {writes:?}"
            );
        }
        let read_write = constraint(
            43,
            &[RegisterOperandAccess::UseDef, RegisterOperandAccess::Use],
        );
        validate_declaration(
            target::Architecture::X86_64,
            &read_write,
            &declaration(
                MachineSemanticKind::CopyI64,
                read_write.key,
                MachineAlternativeApplicability::Always,
                vec![0, 1],
                vec![0],
            ),
        )
        .unwrap();
        for (reads, writes) in [(vec![1], vec![0]), (vec![0, 1], vec![])] {
            assert!(
                validate_declaration(
                    target::Architecture::X86_64,
                    &read_write,
                    &declaration(
                        MachineSemanticKind::CopyI64,
                        read_write.key,
                        MachineAlternativeApplicability::Always,
                        reads.clone(),
                        writes.clone(),
                    ),
                )
                .is_err(),
                "reads {reads:?} writes {writes:?}"
            );
        }
        let returned = constraint(45, &[RegisterOperandAccess::Use]);
        let mut return_row = declaration(
            MachineSemanticKind::ReturnScalar,
            returned.key,
            MachineAlternativeApplicability::Always,
            Vec::new(),
            Vec::new(),
        );
        return_row.barrier = MachineBarrier::ControlFlow;
        return_row.alternatives[0].encoded.control =
            MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                target: RegisterViewId(9),
            };
        return_row.alternatives[0].encoded.trap =
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        validate_declaration(target::Architecture::X86_64, &returned, &return_row).unwrap();
        return_row.alternatives[0].encoded.external_operand_reads = vec![0];
        assert!(
            validate_declaration(target::Architecture::X86_64, &returned, &return_row).is_err()
        );
        let subtract = constraint(
            47,
            &[
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
            ],
        );
        let all_aliased = MachineAlternativeApplicability::ResultAliasesOperands {
            result: 2,
            left: 0,
            right: 1,
        };
        validate_declaration(
            target::Architecture::X86_64,
            &subtract,
            &declaration(
                MachineSemanticKind::ExactSubtractI64,
                subtract.key,
                all_aliased,
                Vec::new(),
                vec![2],
            ),
        )
        .unwrap();
        for (applicability, reads) in [
            (all_aliased, vec![0]),
            (
                MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                    result: 2,
                    aliased_operand: 0,
                    distinct_operand: 1,
                },
                vec![0],
            ),
            (
                MachineAlternativeApplicability::ResultDistinctFromOperands {
                    result: 2,
                    left: 0,
                    right: 1,
                },
                vec![0],
            ),
        ] {
            assert!(
                validate_declaration(
                    target::Architecture::X86_64,
                    &subtract,
                    &declaration(
                        MachineSemanticKind::ExactSubtractI64,
                        subtract.key,
                        applicability,
                        reads.clone(),
                        vec![2],
                    ),
                )
                .is_err(),
                "subtract applicability {applicability:?} reads {reads:?}"
            );
        }
        assert!(
            validate_declaration(
                target::Architecture::X86_64,
                &subtract,
                &declaration(
                    MachineSemanticKind::ExactAddI64,
                    subtract.key,
                    all_aliased,
                    Vec::new(),
                    vec![2],
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn encoded_rows_restate_the_contracted_implicit_custody() {
        let unit = |id| RegisterUnitId(id);
        let constraint = |variant,
                          accesses: &[RegisterOperandAccess],
                          uses: &[u16],
                          defs: &[u16],
                          clobbers: &[u16]| {
            let mut row = instruction_constraint(variant, accesses);
            row.implicit_uses = uses.iter().map(|id| unit(*id)).collect();
            row.implicit_defs = defs.iter().map(|id| unit(*id)).collect();
            row.clobbers = clobbers.iter().map(|id| unit(*id)).collect();
            row
        };
        let declaration = |semantic, key, encoded| MachineEffectDeclaration {
            semantic,
            constraint: key,
            memory: NoneV1,
            trap: NeverV1,
            barrier: MachineBarrier::None,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![always_alternative(semantic, 4, encoded)],
        };
        let row = constraint(
            53,
            &[RegisterOperandAccess::Use, RegisterOperandAccess::Def],
            &[7, 9],
            &[11],
            &[13],
        );
        let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]);
        encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        encoded.implicit_unit_uses = row.implicit_uses.clone();
        encoded.implicit_unit_defs = row.implicit_defs.clone();
        encoded.implicit_unit_clobbers = row.clobbers.clone();
        validate_declaration(
            target::Architecture::X86_64,
            &row,
            &declaration(MachineSemanticKind::ExactAddI64, row.key, encoded.clone()),
        )
        .unwrap();
        for mutation in 0..6 {
            let mut changed = encoded.clone();
            let (list, foreign) = match mutation {
                0 | 1 => (&mut changed.implicit_unit_uses, unit(8)),
                2 | 3 => (&mut changed.implicit_unit_defs, unit(12)),
                _ => (&mut changed.implicit_unit_clobbers, unit(14)),
            };
            if mutation % 2 == 0 {
                list.pop();
            } else {
                list.push(foreign);
                list.sort_unstable();
            }
            assert!(
                validate_declaration(
                    target::Architecture::X86_64,
                    &row,
                    &declaration(MachineSemanticKind::ExactAddI64, row.key, changed),
                )
                .is_err(),
                "mutation {mutation}"
            );
        }
        let returned = constraint(55, &[RegisterOperandAccess::Use], &[5, 9], &[11], &[]);
        let mut return_encoded = MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
        return_encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        return_encoded.control = MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
            target: RegisterViewId(9),
        };
        return_encoded.implicit_unit_uses = vec![unit(9)];
        return_encoded.implicit_unit_defs = returned.implicit_defs.clone();
        let mut return_row = declaration(
            MachineSemanticKind::ReturnScalar,
            returned.key,
            return_encoded,
        );
        return_row.barrier = MachineBarrier::ControlFlow;
        validate_declaration(target::Architecture::X86_64, &returned, &return_row).unwrap();
        return_row.alternatives[0].encoded.implicit_unit_uses = returned.implicit_uses.clone();
        validate_declaration(target::Architecture::X86_64, &returned, &return_row).unwrap();
        for mutation in 0..4 {
            let mut changed = return_row.clone();
            let encoded = &mut changed.alternatives[0].encoded;
            match mutation {
                0 => encoded.implicit_unit_uses.clear(),
                1 => encoded.implicit_unit_uses = vec![unit(4)],
                2 => encoded.implicit_unit_defs.clear(),
                _ => encoded.implicit_unit_clobbers.push(unit(15)),
            }
            assert!(
                validate_declaration(target::Architecture::X86_64, &returned, &changed).is_err(),
                "mutation {mutation}"
            );
        }
        let mut stack_return = return_row.clone();
        stack_return.alternatives[0].encoded.control =
            MachineEncodedControlEffect::ReturnFromActivationStackV1;
        validate_declaration(target::Architecture::X86_64, &returned, &stack_return).unwrap();
        stack_return.alternatives[0].encoded.implicit_unit_uses = vec![unit(9)];
        assert!(
            validate_declaration(target::Architecture::X86_64, &returned, &stack_return).is_err()
        );
    }

    #[test]
    fn declared_memory_and_trap_surfaces_bind_the_selected_semantic() {
        let constraint = instruction_constraint;
        let declaration =
            |semantic, key, memory, trap, barrier, call, encoded| MachineEffectDeclaration {
                semantic,
                constraint: key,
                memory,
                trap,
                barrier,
                call,
                cleanup: crate::MachineCleanupEffect::NoneV1,
                alternatives: vec![always_alternative(semantic, 4, encoded)],
            };
        let arithmetic = constraint(
            61,
            &[
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
            ],
        );
        let add = declaration(
            MachineSemanticKind::ExactAddI64,
            arithmetic.key,
            NoneV1,
            NeverV1,
            MachineBarrier::None,
            crate::MachineCallEffect::NoneV1,
            MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2]),
        );
        validate_declaration(target::Architecture::X86_64, &arithmetic, &add).unwrap();
        for (memory, trap) in [
            (NoneV1, crate::MachineTrapBehavior::MayArchitecturalFaultV1),
            (NoneV1, crate::MachineTrapBehavior::HostedReadFailureV1),
            (crate::MachineMemoryEffect::ReadPointerV1, NeverV1),
            (
                crate::MachineMemoryEffect::WriteFrameStorageV1,
                crate::MachineTrapBehavior::MayArchitecturalFaultV1,
            ),
        ] {
            let mut forged = add.clone();
            forged.memory = memory;
            forged.trap = trap;
            assert!(
                validate_declaration(target::Architecture::X86_64, &arithmetic, &forged).is_err(),
                "memory {memory:?} trap {trap:?}"
            );
        }
        let load = constraint(
            63,
            &[RegisterOperandAccess::Use, RegisterOperandAccess::Def],
        );
        let mut load_encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]);
        load_encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand: 0,
            byte_count: 8,
        };
        load_encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        let load64 = declaration(
            MachineSemanticKind::Load64,
            load.key,
            crate::MachineMemoryEffect::ReadPointerV1,
            crate::MachineTrapBehavior::MayArchitecturalFaultV1,
            MachineBarrier::None,
            crate::MachineCallEffect::NoneV1,
            load_encoded,
        );
        validate_declaration(target::Architecture::X86_64, &load, &load64).unwrap();
        for (memory, trap) in [
            (crate::MachineMemoryEffect::ReadPointerV1, NeverV1),
            (
                crate::MachineMemoryEffect::ReadPointerV1,
                crate::MachineTrapBehavior::HostedWriteFailureV1,
            ),
            (
                crate::MachineMemoryEffect::WritePointerV1,
                crate::MachineTrapBehavior::MayArchitecturalFaultV1,
            ),
            (NoneV1, crate::MachineTrapBehavior::MayArchitecturalFaultV1),
        ] {
            let mut forged = load64.clone();
            forged.memory = memory;
            forged.trap = trap;
            assert!(
                validate_declaration(target::Architecture::X86_64, &load, &forged).is_err(),
                "memory {memory:?} trap {trap:?}"
            );
        }
        let returned = constraint(65, &[RegisterOperandAccess::Use]);
        let stack_pointer = RegisterViewId(5);
        let mut return_encoded = MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
        return_encoded.memory = MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count: 8,
        };
        return_encoded.stack = MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count: 8,
        };
        return_encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        return_encoded.control = MachineEncodedControlEffect::ReturnFromActivationStackV1;
        let returned_row = declaration(
            MachineSemanticKind::ReturnScalar,
            returned.key,
            NoneV1,
            NeverV1,
            MachineBarrier::ControlFlow,
            crate::MachineCallEffect::NoneV1,
            return_encoded,
        );
        validate_declaration(target::Architecture::X86_64, &returned, &returned_row).unwrap();
        for (memory, trap) in [
            (crate::MachineMemoryEffect::ReadPointerV1, NeverV1),
            (NoneV1, crate::MachineTrapBehavior::MayArchitecturalFaultV1),
        ] {
            let mut forged = returned_row.clone();
            forged.memory = memory;
            forged.trap = trap;
            assert!(
                validate_declaration(target::Architecture::X86_64, &returned, &forged).is_err(),
                "memory {memory:?} trap {trap:?}"
            );
        }
        let called = constraint(
            67,
            &[RegisterOperandAccess::Use, RegisterOperandAccess::Def],
        );
        let mut call_encoded = MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]);
        call_encoded.memory = MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer,
            byte_count: 8,
        };
        call_encoded.stack = MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer,
            return_address_byte_count: 8,
        };
        call_encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        call_encoded.control = MachineEncodedControlEffect::DirectRelativeCallV1;
        let call_row = declaration(
            MachineSemanticKind::CallScalar,
            called.key,
            NoneV1,
            NeverV1,
            MachineBarrier::Call,
            crate::MachineCallEffect::InternalNormalReturnV1 {
                pre_call_stack_alignment: 16,
            },
            call_encoded,
        );
        validate_declaration(target::Architecture::X86_64, &called, &call_row).unwrap();
        for (memory, trap) in [
            (crate::MachineMemoryEffect::WritePointerV1, NeverV1),
            (NoneV1, crate::MachineTrapBehavior::MayArchitecturalFaultV1),
        ] {
            let mut forged = call_row.clone();
            forged.memory = memory;
            forged.trap = trap;
            assert!(
                validate_declaration(target::Architecture::X86_64, &called, &forged).is_err(),
                "memory {memory:?} trap {trap:?}"
            );
        }
    }

    #[test]
    fn hosted_exit_cannot_fall_through_to_the_plain_surface_row() {
        let constraint = instruction_constraint(
            17,
            &[RegisterOperandAccess::Use, RegisterOperandAccess::Use],
        );
        let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0, 1], Vec::new());
        encoded.control = MachineEncodedControlEffect::HostedExitOrTrapV1;
        encoded.trap = MachineEncodedTrapBehavior::HostedExitReturnedV1;
        let declaration = MachineEffectDeclaration {
            semantic: MachineSemanticKind::HostedExitProcessI32,
            constraint: constraint.key,
            memory: NoneV1,
            trap: crate::MachineTrapBehavior::HostedExitReturnedV1,
            barrier: MachineBarrier::ExternalEffect,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![always_alternative(
                MachineSemanticKind::HostedExitProcessI32,
                5,
                encoded,
            )],
        };
        assert!(
            validate_declaration(target::Architecture::X86_64, &constraint, &declaration).is_err()
        );
    }

    #[test]
    fn normalized_foreign_call_declarations_bind_external_call_effects() {
        let unit = |id| RegisterUnitId(id);
        let foreign_row = |variant: u32, arity: u16, result: bool| RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant,
            },
            operands: (0..arity)
                .map(|operand| RegisterOperandConstraint {
                    operand,
                    access: RegisterOperandAccess::Use,
                    class: RegisterClassId(0),
                    fixed_view: Some(RegisterViewId(operand)),
                    tied_to: None,
                    early_clobber: false,
                })
                .chain(result.then_some(RegisterOperandConstraint {
                    operand: arity,
                    access: RegisterOperandAccess::Def,
                    class: RegisterClassId(0),
                    fixed_view: Some(RegisterViewId(arity)),
                    tied_to: None,
                    early_clobber: false,
                }))
                .collect(),
            implicit_uses: vec![unit(4)],
            implicit_defs: Vec::new(),
            clobbers: vec![unit(0), unit(1)],
        };
        let declaration = |key, encoded: MachineEncodedEffects| MachineEffectDeclaration {
            semantic: MachineSemanticKind::NormalizedForeignCall,
            constraint: key,
            memory: NoneV1,
            trap: NeverV1,
            barrier: MachineBarrier::Call,
            call: crate::MachineCallEffect::ExternalNormalReturnV1 {
                pre_call_stack_alignment: 16,
            },
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![always_alternative(
                MachineSemanticKind::NormalizedForeignCall,
                5,
                encoded,
            )],
        };
        let encoded = |row: &RegisterInstructionConstraint| {
            let mut encoded = MachineEncodedEffects::fallthrough_v1(
                row.operands
                    .iter()
                    .filter(|operand| operand.access == RegisterOperandAccess::Use)
                    .map(|operand| operand.operand)
                    .collect(),
                row.operands
                    .iter()
                    .filter(|operand| operand.access == RegisterOperandAccess::Def)
                    .map(|operand| operand.operand)
                    .collect(),
            );
            encoded.implicit_unit_uses = row.implicit_uses.clone();
            encoded.implicit_unit_defs = row.implicit_defs.clone();
            encoded.implicit_unit_clobbers = row.clobbers.clone();
            encoded.memory = MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer: RegisterViewId(9),
                byte_count: 8,
            };
            encoded.stack = MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                stack_pointer: RegisterViewId(9),
                return_address_byte_count: 8,
            };
            encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
            encoded.control = MachineEncodedControlEffect::DirectRelativeCallV1;
            encoded
        };
        let row = foreign_row(3000, 2, true);
        let source = declaration(row.key, encoded(&row));
        validate_declaration(target::Architecture::X86_64, &row, &source).unwrap();
        let mut table = source.clone();
        table.memory = crate::MachineMemoryEffect::ReadPointerV1;
        table.trap = crate::MachineTrapBehavior::MayArchitecturalFaultV1;
        table.alternatives[0].size = MachineSizeKnowledge::ExactBytes(20);
        table.alternatives[0].encoded.memory = MachineEncodedMemoryEffect::ReadTableFieldV1 {
            pointer_operand: 0,
            byte_offset: 8,
        };
        table.alternatives[0].encoded.stack = MachineEncodedStackEffect::UnchangedV1;
        table.alternatives[0].encoded.control = MachineEncodedControlEffect::IndirectExternalCallV1;
        validate_declaration(target::Architecture::Aarch64, &row, &table).unwrap();
        assert!(validate_declaration(target::Architecture::X86_64, &row, &table).is_err());
        let mut wrong_stack = table.clone();
        wrong_stack.alternatives[0].encoded.stack = source.alternatives[0].encoded.stack;
        assert!(validate_declaration(target::Architecture::Aarch64, &row, &wrong_stack).is_err());
        let mut wrong_pointer = table.clone();
        wrong_pointer.alternatives[0].encoded.memory =
            MachineEncodedMemoryEffect::ReadTableFieldV1 {
                pointer_operand: 2,
                byte_offset: 8,
            };
        assert!(validate_declaration(target::Architecture::Aarch64, &row, &wrong_pointer).is_err());
        let mut missing_fault = table;
        missing_fault.trap = NeverV1;
        assert!(validate_declaration(target::Architecture::Aarch64, &row, &missing_fault).is_err());
        let unit_row = foreign_row(3010, 1, false);
        validate_declaration(
            target::Architecture::X86_64,
            &unit_row,
            &declaration(unit_row.key, encoded(&unit_row)),
        )
        .unwrap();

        for call in [
            crate::MachineCallEffect::NoneV1,
            crate::MachineCallEffect::InternalNormalReturnV1 {
                pre_call_stack_alignment: 16,
            },
        ] {
            let mut changed = source.clone();
            changed.call = call;
            assert_eq!(
                validate_declaration(target::Architecture::X86_64, &row, &changed),
                Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                    MachineSemanticKind::NormalizedForeignCall
                )),
                "{call:?} must reject on a foreign declaration"
            );
        }
        let mut changed = source.clone();
        changed.barrier = MachineBarrier::None;
        assert_eq!(
            validate_declaration(target::Architecture::X86_64, &row, &changed),
            Err(MachineEffectCatalogValidationError::BarrierMismatch(
                MachineSemanticKind::NormalizedForeignCall
            ))
        );
        let mut unpinned = row.clone();
        unpinned.operands[0].fixed_view = None;
        assert_eq!(
            validate_declaration(target::Architecture::X86_64, &unpinned, &source),
            Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                MachineSemanticKind::NormalizedForeignCall
            ))
        );
        let mut doubled = row.clone();
        doubled.operands.push(RegisterOperandConstraint {
            operand: 3,
            access: RegisterOperandAccess::Def,
            class: RegisterClassId(0),
            fixed_view: Some(RegisterViewId(3)),
            tied_to: None,
            early_clobber: false,
        });
        assert_eq!(
            validate_declaration(target::Architecture::X86_64, &doubled, &source),
            Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                MachineSemanticKind::NormalizedForeignCall
            ))
        );
        validate_declaration(
            target::Architecture::X86_64,
            &doubled,
            &declaration(doubled.key, encoded(&doubled)),
        )
        .unwrap();
        let mut reordered = declaration(doubled.key, encoded(&doubled));
        reordered.alternatives[0]
            .encoded
            .external_operand_writes
            .reverse();
        assert!(validate_declaration(target::Architecture::X86_64, &doubled, &reordered).is_err());
        let mut changed = source.clone();
        changed.alternatives[0].encoded.control = MachineEncodedControlEffect::FallThroughV1;
        assert_eq!(
            validate_declaration(target::Architecture::X86_64, &row, &changed),
            Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                MachineSemanticKind::NormalizedForeignCall
            ))
        );
        let mut changed = source;
        changed.alternatives[0].encoded.implicit_unit_clobbers.pop();
        assert_eq!(
            validate_declaration(target::Architecture::X86_64, &row, &changed),
            Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                MachineSemanticKind::NormalizedForeignCall
            ))
        );
    }

    #[test]
    fn directed_stack_effect_cannot_borrow_unbalanced_or_nearest_custody() {
        let key = register_model::RegisterConstraintKey {
            family: register_model::RegisterConstraintFamily::Instruction,
            variant: 1,
        };
        let constraint = register_model::RegisterInstructionConstraint {
            id: register_model::RegisterConstraintId(0),
            key,
            operands: (0..3)
                .map(|operand| register_model::RegisterOperandConstraint {
                    operand,
                    access: if operand == 2 {
                        RegisterOperandAccess::Def
                    } else {
                        RegisterOperandAccess::Use
                    },
                    class: register_model::RegisterClassId(1),
                    fixed_view: None,
                    tied_to: None,
                    early_clobber: false,
                })
                .collect(),
            implicit_uses: vec![RegisterUnitId(1)],
            implicit_defs: vec![],
            clobbers: vec![],
        };
        let form = crate::IeeeFloatArithmeticForm {
            operation: crate::IeeeFloatArithmeticOperation::ALL[54],
            format: crate::IeeeFloatFormat::Binary64,
        };
        let semantic = MachineSemanticKind::NearestIeeeFloatArithmetic(form);
        let pointer = register_model::RegisterViewId(1);
        let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2]);
        encoded.implicit_unit_uses = constraint.implicit_uses.clone();
        encoded.memory = MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
            stack_pointer: pointer,
            byte_count: 16,
        };
        encoded.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
            stack_pointer: pointer,
            byte_count: 16,
        };
        encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        let source = MachineEffectDeclaration {
            semantic,
            constraint: key,
            memory: crate::MachineMemoryEffect::TemporaryStackStorageV1,
            trap: crate::MachineTrapBehavior::MayArchitecturalFaultV1,
            barrier: MachineBarrier::None,
            call: crate::MachineCallEffect::NoneV1,
            cleanup: crate::MachineCleanupEffect::NoneV1,
            alternatives: vec![always_alternative(semantic, 64, encoded)],
        };
        validate_declaration(target::Architecture::X86_64, &constraint, &source).unwrap();
        assert!(validate_declaration(target::Architecture::Aarch64, &constraint, &source).is_err());
        for mutation in 0..5 {
            let mut changed = source.clone();
            let effects = &mut changed.alternatives[0].encoded;
            match mutation {
                0 => effects.stack = MachineEncodedStackEffect::UnchangedV1,
                1 => effects.memory = MachineEncodedMemoryEffect::NoneV1,
                2 => {
                    effects.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
                        stack_pointer: pointer,
                        byte_count: 32,
                    }
                }
                3 => {
                    effects.memory = MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                        stack_pointer: register_model::RegisterViewId(2),
                        byte_count: 16,
                    }
                }
                4 => {
                    changed.semantic = MachineSemanticKind::NearestIeeeFloatArithmetic(
                        crate::IeeeFloatArithmeticForm {
                            operation: crate::IeeeFloatArithmeticOperation::Divide,
                            ..form
                        },
                    );
                    changed.alternatives[0].key.family = changed.semantic.into();
                }
                _ => unreachable!(),
            }
            assert!(
                validate_declaration(target::Architecture::X86_64, &constraint, &changed).is_err()
            );
        }
    }
}

use super::{
    MachineAlternativeApplicability, MachineBarrier, MachineEffectCatalogValidationError,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineSemanticKind, MachineSizeKnowledge,
};
use crate::MachineCallEffect::SupervisorNormalReturnV1;
use crate::MachineMemoryEffect::{CompareByteSequencesV1, ReadFrameStorageV1, WritePointerV1};
use crate::MachineTrapBehavior::{HostedExitReturnedV1, MayArchitecturalFaultV1};
use register_model::RegisterConstraintFamily::Call;

fn has_policy_trap(semantic: MachineSemanticKind) -> bool {
    matches!(semantic, MachineSemanticKind::TrappingInteger(_))
        || matches!(semantic, MachineSemanticKind::FloatResultPolicy(form) if form.can_trap())
        || matches!(
            semantic,
            MachineSemanticKind::NearestIeeeFloatArithmetic(crate::IeeeFloatArithmeticForm {
                operation: crate::IeeeFloatArithmeticOperation::TruncateToInteger {
                    domain: crate::IeeeFloatToIntegerDomain::Trapping,
                    ..
                },
                ..
            })
        )
}

fn touches_external_state(semantic: MachineSemanticKind) -> bool {
    matches!(
        semantic,
        MachineSemanticKind::SaveFloatingControl
            | MachineSemanticKind::RestoreFloatingControl
            | MachineSemanticKind::PortWrite
            | MachineSemanticKind::SystemRegisterRead
            | MachineSemanticKind::SystemRegisterWrite
            | MachineSemanticKind::GlobalDescriptorTableLoad
            | MachineSemanticKind::GlobalDescriptorTableStore
            | MachineSemanticKind::TaskRegisterActivate
            | MachineSemanticKind::InstalledInterruptCpuCheck
            | MachineSemanticKind::InstalledInterruptTaskCommit
            | MachineSemanticKind::InstalledInterruptTablePublication
            | MachineSemanticKind::TaskRegisterRead
            | MachineSemanticKind::CodeSegmentReload
            | MachineSemanticKind::DataSegmentReload
            | MachineSemanticKind::SegmentSelectorRead
            | MachineSemanticKind::ControlRegisterRead
            | MachineSemanticKind::ControlRegisterWrite
            | MachineSemanticKind::PortRegisterRead
            | MachineSemanticKind::PortRegisterWrite
            | MachineSemanticKind::ModelSpecificRead
            | MachineSemanticKind::ModelSpecificWrite
            | MachineSemanticKind::ProcessorHalt
            | MachineSemanticKind::InterruptControl
            | MachineSemanticKind::MemoryFence
            | MachineSemanticKind::AtomicFence
            | MachineSemanticKind::AtomicAccess(_)
            | MachineSemanticKind::AccessorScalarAccess(_)
    )
}

fn positional_roster(
    constraint: &RegisterInstructionConstraint,
    inputs: usize,
    total: usize,
) -> bool {
    constraint.operands.len() == total
        && constraint
            .operands
            .iter()
            .enumerate()
            .all(|(position, operand)| {
                operand.operand as usize == position
                    && operand.access
                        == if position < inputs {
                            register_model::RegisterOperandAccess::Use
                        } else {
                            register_model::RegisterOperandAccess::Def
                        }
                    && operand.early_clobber == (position >= inputs)
                    && operand.fixed_view.is_none()
                    && operand.tied_to.is_none()
            })
}

fn encoded_scalar_constraint(constraint: &RegisterInstructionConstraint, inputs: usize) -> bool {
    constraint.implicit_uses.is_empty()
        && constraint.implicit_defs.is_empty()
        && !constraint.clobbers.is_empty()
        && positional_roster(constraint, inputs, 6)
}

fn implicit_roster_matches(
    encoded: &MachineEncodedEffects,
    constraint: &RegisterInstructionConstraint,
) -> bool {
    encoded.implicit_unit_uses == constraint.implicit_uses
        && encoded.implicit_unit_defs == constraint.implicit_defs
        && encoded.implicit_unit_clobbers == constraint.clobbers
}

fn call_roster_matches(
    encoded: &MachineEncodedEffects,
    constraint: &RegisterInstructionConstraint,
    arguments: &[RegisterOperandConstraint],
    results: &[RegisterOperandConstraint],
) -> bool {
    encoded
        .external_operand_reads
        .iter()
        .copied()
        .eq(arguments.iter().map(|operand| operand.operand))
        && encoded
            .external_operand_writes
            .iter()
            .copied()
            .eq(results.iter().map(|operand| operand.operand))
        && implicit_roster_matches(encoded, constraint)
}

pub(super) fn validate_declaration(
    architecture: target::Architecture,
    constraint: &RegisterInstructionConstraint,
    declaration: &MachineEffectDeclaration,
) -> Result<(), MachineEffectCatalogValidationError> {
    let semantic = declaration.semantic;
    let expected_barrier = if has_policy_trap(semantic)
        || matches!(
            semantic,
            MachineSemanticKind::HostedReadByte
                | MachineSemanticKind::HostedWriteByteI32
                | MachineSemanticKind::HostedExitProcessI32
        )
        || touches_external_state(semantic)
    {
        MachineBarrier::ExternalEffect
    } else if matches!(
        semantic,
        MachineSemanticKind::Crash
            | MachineSemanticKind::PrivilegedCrash
            | MachineSemanticKind::ConditionalBranchNonZero
            | MachineSemanticKind::Jump
            | MachineSemanticKind::ConditionalBranchU64LessThan
            | MachineSemanticKind::ConditionalBranchI64LessThan
            | MachineSemanticKind::ReturnScalar
            | MachineSemanticKind::ReturnAggregate
            | MachineSemanticKind::ReturnUnit
    ) {
        MachineBarrier::ControlFlow
    } else if matches!(
        semantic,
        MachineSemanticKind::CallScalar
            | MachineSemanticKind::InvokeNoNormal
            | MachineSemanticKind::CallUnit
            | MachineSemanticKind::CallAggregate
            | MachineSemanticKind::NormalizedForeignCall
    ) {
        MachineBarrier::Call
    } else {
        MachineBarrier::None
    };
    if declaration.barrier != expected_barrier {
        return Err(MachineEffectCatalogValidationError::BarrierMismatch(
            semantic,
        ));
    }
    let temporary_stack = architecture == target::Architecture::X86_64
        && (matches!(
            semantic,
            MachineSemanticKind::SystemRegisterRead | MachineSemanticKind::SystemRegisterWrite
        ) || matches!(semantic, MachineSemanticKind::NearestIeeeFloatArithmetic(form) if form.operation.rounding_direction().is_some()));
    if semantic == MachineSemanticKind::PortWrite
        && (architecture != target::Architecture::X86_64
            || !constraint.operands.is_empty()
            || !constraint.implicit_uses.is_empty()
            || !constraint.implicit_defs.is_empty())
    {
        return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            semantic,
        ));
    }
    if semantic == MachineSemanticKind::AtomicFence
        && (!constraint.operands.is_empty()
            || !constraint.implicit_uses.is_empty()
            || !constraint.implicit_defs.is_empty()
            || !constraint.clobbers.is_empty()
            || declaration.alternatives.iter().any(|alternative| {
                alternative.encoded != crate::MachineEncodedEffects::atomic_fence_v1()
            }))
    {
        return Err(MachineEffectCatalogValidationError::BarrierMismatch(
            semantic,
        ));
    }
    if semantic == MachineSemanticKind::MemoryFence
        && (!constraint.operands.is_empty()
            || !constraint.implicit_uses.is_empty()
            || !constraint.implicit_defs.is_empty()
            || !constraint.clobbers.is_empty()
            || declaration.alternatives.iter().any(|alternative| {
                alternative.encoded != crate::MachineEncodedEffects::catalog_fence_v1()
            }))
    {
        return Err(MachineEffectCatalogValidationError::BarrierMismatch(
            semantic,
        ));
    }
    let table_call = semantic == MachineSemanticKind::NormalizedForeignCall
        && !declaration.alternatives.is_empty()
        && declaration.alternatives.iter().all(|alternative| {
            alternative.encoded.control == MachineEncodedControlEffect::IndirectExternalCallV1
                && matches!(
                    alternative.encoded.memory,
                    MachineEncodedMemoryEffect::ReadTableFieldV1 { .. }
                )
        });
    let callback_writes = semantic == MachineSemanticKind::NormalizedForeignCall
        && !declaration.alternatives.is_empty()
        && declaration.alternatives.iter().all(|alternative| {
            alternative.encoded.memory
                == MachineEncodedMemoryEffect::PrivateCallbackMaterializationV1
        });
    let expected_memory = match semantic {
        _ if callback_writes => crate::MachineMemoryEffect::PrivateCallbackMaterializationV1,
        _ if table_call => crate::MachineMemoryEffect::ReadPointerV1,
        _ if temporary_stack => crate::MachineMemoryEffect::TemporaryStackStorageV1,
        MachineSemanticKind::GlobalDescriptorTableLoad => crate::MachineMemoryEffect::ReadPointerV1,
        MachineSemanticKind::GlobalDescriptorTableStore => WritePointerV1,
        MachineSemanticKind::InstalledInterruptCpuCheck => {
            crate::MachineMemoryEffect::InstalledInterruptCpuCheckV1
        }
        MachineSemanticKind::InstalledInterruptTaskCommit => {
            crate::MachineMemoryEffect::InstalledInterruptTaskCommitV1
        }
        MachineSemanticKind::InstalledInterruptTablePublication => {
            crate::MachineMemoryEffect::InstalledInterruptTablePublicationV1
        }
        MachineSemanticKind::TaskRegisterActivate => {
            crate::MachineMemoryEffect::TaskRegisterActivationV1
        }
        MachineSemanticKind::CodeSegmentReload | MachineSemanticKind::DataSegmentReload => {
            crate::MachineMemoryEffect::SegmentDescriptorReloadV1
        }
        MachineSemanticKind::MemoryFence => crate::MachineMemoryEffect::CatalogFenceV1,
        MachineSemanticKind::AtomicFence => crate::MachineMemoryEffect::AtomicFenceV1,
        MachineSemanticKind::AtomicAccess(_) => crate::MachineMemoryEffect::AtomicPointerV1,
        MachineSemanticKind::AccessorScalarAccess(form) => {
            if form.write {
                WritePointerV1
            } else {
                crate::MachineMemoryEffect::ReadPointerV1
            }
        }
        MachineSemanticKind::CopyBytes => crate::MachineMemoryEffect::CopyBytesV1,
        MachineSemanticKind::ZeroBytes => crate::MachineMemoryEffect::ZeroBytesV1,
        MachineSemanticKind::ScanByteSequence => crate::MachineMemoryEffect::ScanByteSequenceV1,
        MachineSemanticKind::LoadEncodedScalar => crate::MachineMemoryEffect::ReadEncodedScalarV1,
        MachineSemanticKind::StoreEncodedScalar => crate::MachineMemoryEffect::WriteEncodedScalarV1,
        MachineSemanticKind::CompareByteSequences => CompareByteSequencesV1,
        MachineSemanticKind::HostedReadByte => crate::MachineMemoryEffect::HostedReadByteV1,
        MachineSemanticKind::HostedWriteByteI32 => crate::MachineMemoryEffect::HostedWriteByteV1,
        MachineSemanticKind::Load8
        | MachineSemanticKind::Load16
        | MachineSemanticKind::Load32
        | MachineSemanticKind::Load64
        | MachineSemanticKind::Load8Indexed
        | MachineSemanticKind::LoadPacked3
        | MachineSemanticKind::LoadPacked5
        | MachineSemanticKind::LoadPacked6
        | MachineSemanticKind::LoadPacked7 => crate::MachineMemoryEffect::ReadPointerV1,
        MachineSemanticKind::Store64 | MachineSemanticKind::SaveFloatingControl => {
            crate::MachineMemoryEffect::WriteFrameStorageV1
        }
        MachineSemanticKind::RestoreFloatingControl => ReadFrameStorageV1,
        MachineSemanticKind::Store | MachineSemanticKind::StorePacked => WritePointerV1,
        _ => crate::MachineMemoryEffect::NoneV1,
    };
    if declaration.memory != expected_memory {
        return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            semantic,
        ));
    }
    let expected_trap = match semantic {
        _ if callback_writes => MayArchitecturalFaultV1,
        _ if table_call => MayArchitecturalFaultV1,
        _ if temporary_stack => MayArchitecturalFaultV1,
        MachineSemanticKind::PortWrite
        | MachineSemanticKind::SystemRegisterRead
        | MachineSemanticKind::SystemRegisterWrite
        | MachineSemanticKind::GlobalDescriptorTableLoad
        | MachineSemanticKind::GlobalDescriptorTableStore
        | MachineSemanticKind::TaskRegisterActivate
        | MachineSemanticKind::InstalledInterruptCpuCheck
        | MachineSemanticKind::InstalledInterruptTaskCommit
        | MachineSemanticKind::InstalledInterruptTablePublication
        | MachineSemanticKind::TaskRegisterRead
        | MachineSemanticKind::CodeSegmentReload
        | MachineSemanticKind::DataSegmentReload
        | MachineSemanticKind::SegmentSelectorRead
        | MachineSemanticKind::ControlRegisterRead
        | MachineSemanticKind::ControlRegisterWrite
        | MachineSemanticKind::PortRegisterRead
        | MachineSemanticKind::PortRegisterWrite
        | MachineSemanticKind::ModelSpecificRead
        | MachineSemanticKind::ModelSpecificWrite
        | MachineSemanticKind::ProcessorHalt
        | MachineSemanticKind::InterruptControl
        | MachineSemanticKind::AtomicAccess(_)
        | MachineSemanticKind::AccessorScalarAccess(_) => MayArchitecturalFaultV1,
        MachineSemanticKind::Crash => crate::MachineTrapBehavior::ExplicitCrashV1,
        MachineSemanticKind::PrivilegedCrash => MayArchitecturalFaultV1,
        semantic if has_policy_trap(semantic) => crate::MachineTrapBehavior::TrappingIntegerV1,
        MachineSemanticKind::HostedExitProcessI32 => HostedExitReturnedV1,
        MachineSemanticKind::HostedReadByte => crate::MachineTrapBehavior::HostedReadFailureV1,
        MachineSemanticKind::HostedWriteByteI32 => crate::MachineTrapBehavior::HostedWriteFailureV1,
        MachineSemanticKind::CopyBytes
        | MachineSemanticKind::ZeroBytes
        | MachineSemanticKind::ScanByteSequence
        | MachineSemanticKind::LoadEncodedScalar
        | MachineSemanticKind::StoreEncodedScalar
        | MachineSemanticKind::CompareByteSequences
        | MachineSemanticKind::Load8
        | MachineSemanticKind::Load16
        | MachineSemanticKind::Load32
        | MachineSemanticKind::Load64
        | MachineSemanticKind::Load8Indexed
        | MachineSemanticKind::LoadPacked3
        | MachineSemanticKind::LoadPacked5
        | MachineSemanticKind::LoadPacked6
        | MachineSemanticKind::LoadPacked7
        | MachineSemanticKind::Store
        | MachineSemanticKind::StorePacked
        | MachineSemanticKind::Store64
        | MachineSemanticKind::SaveFloatingControl
        | MachineSemanticKind::RestoreFloatingControl => MayArchitecturalFaultV1,
        _ => crate::MachineTrapBehavior::NeverV1,
    };
    if declaration.trap != expected_trap {
        return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            semantic,
        ));
    }
    if declaration.cleanup != crate::MachineCleanupEffect::NoneV1 {
        return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
            semantic,
        ));
    }
    match (semantic, declaration.call) {
        (
            MachineSemanticKind::InvokeNoNormal,
            crate::MachineCallEffect::InternalNoNormalV1 {
                pre_call_stack_alignment,
            },
        ) if pre_call_stack_alignment.is_power_of_two() && constraint.key.family == Call => {}
        (MachineSemanticKind::InvokeNoNormal, _) => {
            return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                semantic,
            ));
        }
        (
            MachineSemanticKind::CallScalar
            | MachineSemanticKind::CallUnit
            | MachineSemanticKind::CallAggregate,
            crate::MachineCallEffect::InternalNormalReturnV1 {
                pre_call_stack_alignment,
            },
        ) if pre_call_stack_alignment.is_power_of_two() => {}
        (
            MachineSemanticKind::CallScalar
            | MachineSemanticKind::CallUnit
            | MachineSemanticKind::CallAggregate,
            _,
        ) => {
            return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                semantic,
            ));
        }
        (
            MachineSemanticKind::NormalizedForeignCall,
            crate::MachineCallEffect::ExternalNormalReturnV1 {
                pre_call_stack_alignment,
            },
        ) if pre_call_stack_alignment.is_power_of_two() && constraint.key.family == Call => {}
        (MachineSemanticKind::NormalizedForeignCall, SupervisorNormalReturnV1)
            if constraint.key.family == register_model::RegisterConstraintFamily::SystemCall => {}
        (MachineSemanticKind::NormalizedForeignCall, _) => {
            return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                semantic,
            ));
        }
        (_, crate::MachineCallEffect::NoneV1) => {}
        _ => {
            return Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                semantic,
            ));
        }
    }
    if declaration.alternatives.is_empty() {
        return Err(MachineEffectCatalogValidationError::EmptyAlternatives(
            semantic,
        ));
    }
    if declaration
        .alternatives
        .windows(2)
        .any(|pair| pair[0].key >= pair[1].key)
    {
        return Err(MachineEffectCatalogValidationError::NonCanonicalAlternatives(semantic));
    }
    let expected_family = semantic.into();
    for alternative in &declaration.alternatives {
        if alternative.key.family != expected_family {
            return Err(MachineEffectCatalogValidationError::AlternativeFamilyMismatch(semantic));
        }
        validate_applicability(constraint, alternative.applicability).map_err(|()| {
            MachineEffectCatalogValidationError::InvalidAlternativeApplicability(semantic)
        })?;
        validate_encoded_effects(
            architecture,
            constraint,
            declaration,
            alternative.applicability,
            &alternative.encoded,
        )
        .map_err(|()| MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic))?;
        match alternative.size {
            MachineSizeKnowledge::ExactBytes(0)
            | MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 0, ..
            } => {
                return Err(MachineEffectCatalogValidationError::InvalidSizeKnowledge(
                    semantic,
                ));
            }
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes,
                maximum_bytes: Some(maximum),
            } if maximum < minimum_bytes => {
                return Err(MachineEffectCatalogValidationError::InvalidSizeKnowledge(
                    semantic,
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_encoded_effects(
    architecture: target::Architecture,
    constraint: &RegisterInstructionConstraint,
    declaration: &MachineEffectDeclaration,
    applicability: MachineAlternativeApplicability,
    encoded: &MachineEncodedEffects,
) -> Result<(), ()> {
    if let MachineSemanticKind::FloatResultPolicy(form) = declaration.semantic {
        let input_count = form.operand_count();
        if constraint.operands.len() != input_count + 1
            || constraint
                .operands
                .iter()
                .enumerate()
                .any(|(position, operand)| {
                    usize::from(operand.operand) != position
                        || operand.access
                            != if position < input_count {
                                RegisterOperandAccess::Use
                            } else {
                                RegisterOperandAccess::Def
                            }
                })
        {
            return Err(());
        }
    }
    if declaration.semantic == MachineSemanticKind::PrivilegedCrash {
        return if constraint.operands.is_empty()
            && !constraint.implicit_uses.is_empty()
            && constraint.implicit_defs == constraint.implicit_uses
            && !constraint.clobbers.is_empty()
            && encoded.external_operand_reads.is_empty()
            && encoded.external_operand_writes.is_empty()
            && encoded.implicit_unit_uses == constraint.implicit_uses
            && encoded.implicit_unit_defs == constraint.implicit_defs
            && encoded.implicit_unit_clobbers == constraint.clobbers
            && encoded.memory == MachineEncodedMemoryEffect::NoneV1
            && encoded.stack == MachineEncodedStackEffect::UnchangedV1
            && encoded.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            && encoded.control == MachineEncodedControlEffect::PrivilegedStopV1
            && applicability == MachineAlternativeApplicability::Always
        {
            Ok(())
        } else {
            Err(())
        };
    }
    if declaration.semantic == MachineSemanticKind::Crash {
        return if constraint.operands.is_empty()
            && !constraint.implicit_uses.is_empty()
            && constraint.implicit_defs == constraint.implicit_uses
            && constraint.clobbers.is_empty()
            && encoded.external_operand_reads.is_empty()
            && encoded.external_operand_writes.is_empty()
            && encoded.implicit_unit_uses == constraint.implicit_uses
            && encoded.implicit_unit_defs == constraint.implicit_defs
            && encoded.implicit_unit_clobbers.is_empty()
            && encoded.memory == MachineEncodedMemoryEffect::NoneV1
            && encoded.stack == MachineEncodedStackEffect::UnchangedV1
            && encoded.trap == MachineEncodedTrapBehavior::ExplicitCrashV1
            && encoded.control == MachineEncodedControlEffect::CrashV1
            && applicability == MachineAlternativeApplicability::Always
        {
            Ok(())
        } else {
            Err(())
        };
    }
    if matches!(
        declaration.semantic,
        MachineSemanticKind::LoadPacked3
            | MachineSemanticKind::LoadPacked5
            | MachineSemanticKind::LoadPacked6
            | MachineSemanticKind::LoadPacked7
            | MachineSemanticKind::StorePacked
    ) {
        let load = declaration.semantic != MachineSemanticKind::StorePacked;
        if constraint.operands.len() != 3
            || constraint
                .operands
                .iter()
                .enumerate()
                .any(|(index, operand)| {
                    let writes = index == 2 || (load && index == 1);
                    operand.operand != index as u16
                        || operand.access
                            != if writes {
                                RegisterOperandAccess::Def
                            } else {
                                RegisterOperandAccess::Use
                            }
                        || operand.early_clobber != writes
                        || operand.tied_to.is_some()
                })
            || encoded.external_operand_reads.as_slice()
                != if load { &[0][..] } else { &[0, 1][..] }
            || encoded.external_operand_writes.as_slice()
                != if load { &[1, 2][..] } else { &[2][..] }
        {
            return Err(());
        }
    }
    let canonical = |values: &[u16]| values.windows(2).all(|pair| pair[0] < pair[1]);
    if !canonical(&encoded.external_operand_reads)
        || !canonical(&encoded.external_operand_writes)
        || encoded
            .implicit_unit_uses
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || encoded
            .implicit_unit_defs
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || encoded
            .implicit_unit_clobbers
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(());
    }
    for operand in &encoded.external_operand_reads {
        let row = constraint.operand(*operand).ok_or(())?;
        if !matches!(
            row.access,
            RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
        ) {
            return Err(());
        }
    }
    for operand in &encoded.external_operand_writes {
        let row = constraint.operand(*operand).ok_or(())?;
        if !matches!(
            row.access,
            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
        ) {
            return Err(());
        }
    }
    if !encoded
        .external_operand_writes
        .iter()
        .copied()
        .eq(constraint.operands.iter().filter_map(|operand| {
            matches!(
                operand.access,
                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
            )
            .then_some(operand.operand)
        }))
    {
        return Err(());
    }
    let mut contracted_reads = constraint
        .operands
        .iter()
        .filter_map(|operand| {
            matches!(
                operand.access,
                RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
            )
            .then_some(operand.operand)
        })
        .collect::<Vec<u16>>();
    match declaration.semantic {
        MachineSemanticKind::ReturnScalar
        | MachineSemanticKind::ReturnAggregate
        | MachineSemanticKind::ReturnUnit => contracted_reads.clear(),
        MachineSemanticKind::ExactSubtractI64 | MachineSemanticKind::WrappingSubtractI64 => {
            if let MachineAlternativeApplicability::ResultAliasesOperands { left, right, .. } =
                applicability
            {
                contracted_reads.retain(|operand| *operand != left && *operand != right);
            }
        }
        _ => {}
    }
    if encoded.external_operand_reads != contracted_reads {
        return Err(());
    }
    let clobbers_match = match applicability {
        MachineAlternativeApplicability::OptimizationSelected { .. } => constraint
            .clobbers
            .iter()
            .all(|unit| encoded.implicit_unit_clobbers.contains(unit)),
        _ => encoded.implicit_unit_clobbers == constraint.clobbers,
    };
    if encoded.implicit_unit_defs != constraint.implicit_defs || !clobbers_match {
        return Err(());
    }
    let implicit_uses_match = match encoded.control {
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. } => {
            encoded.implicit_unit_uses == constraint.implicit_uses
                || (!encoded.implicit_unit_uses.is_empty()
                    && encoded
                        .implicit_unit_uses
                        .iter()
                        .all(|unit| constraint.implicit_uses.contains(unit)))
        }
        _ => encoded.implicit_unit_uses == constraint.implicit_uses,
    };
    if !implicit_uses_match {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::CopyBytes
        && (declaration.memory != crate::MachineMemoryEffect::CopyBytesV1
            || !matches!(
                encoded.memory,
                MachineEncodedMemoryEffect::CopyBytesV1 { .. }
            )
            || declaration.call != crate::MachineCallEffect::NoneV1
            || !constraint.implicit_uses.is_empty()
            || !constraint.implicit_defs.is_empty()
            || constraint.clobbers.is_empty())
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::ZeroBytes
        && (declaration.memory != crate::MachineMemoryEffect::ZeroBytesV1
            || !matches!(
                encoded.memory,
                MachineEncodedMemoryEffect::ZeroBytesV1 { .. }
            )
            || declaration.call != crate::MachineCallEffect::NoneV1
            || !constraint.implicit_uses.is_empty()
            || !constraint.implicit_defs.is_empty()
            || constraint.clobbers.is_empty())
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::HostedExitProcessI32
        && (declaration.memory != crate::MachineMemoryEffect::NoneV1
            || declaration.trap != HostedExitReturnedV1
            || encoded.memory != MachineEncodedMemoryEffect::NoneV1
            || encoded.trap != MachineEncodedTrapBehavior::HostedExitReturnedV1
            || encoded.stack != MachineEncodedStackEffect::UnchangedV1
            || encoded.control != MachineEncodedControlEffect::HostedExitOrTrapV1
            || declaration.call != crate::MachineCallEffect::NoneV1)
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::HostedReadByte
        && (declaration.memory != crate::MachineMemoryEffect::HostedReadByteV1
            || declaration.trap != crate::MachineTrapBehavior::HostedReadFailureV1
            || !matches!(
                encoded.memory,
                MachineEncodedMemoryEffect::HostedReadByteV1 { .. }
            )
            || encoded.trap != MachineEncodedTrapBehavior::HostedReadFailureV1
            || encoded.stack != MachineEncodedStackEffect::UnchangedV1
            || encoded.control != MachineEncodedControlEffect::HostedReadReturnOrTrapV1)
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::HostedWriteByteI32
        && (declaration.memory != crate::MachineMemoryEffect::HostedWriteByteV1
            || declaration.trap != crate::MachineTrapBehavior::HostedWriteFailureV1
            || !matches!(
                encoded.memory,
                MachineEncodedMemoryEffect::HostedWriteByteV1 { .. }
            )
            || encoded.trap != MachineEncodedTrapBehavior::HostedWriteFailureV1
            || encoded.stack != MachineEncodedStackEffect::UnchangedV1
            || encoded.control != MachineEncodedControlEffect::HostedWriteReturnOrTrapV1)
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::InvokeNoNormal {
        let input_count = constraint
            .operands
            .iter()
            .take_while(|operand| operand.access == RegisterOperandAccess::Use)
            .count();
        let (arguments, results) = constraint.operands.split_at(input_count);
        if results
            .iter()
            .any(|operand| operand.access != RegisterOperandAccess::Def)
            || constraint
                .operands
                .iter()
                .any(|operand| operand.fixed_view.is_none())
            || !call_roster_matches(encoded, constraint, arguments, results)
            || encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            || encoded.control != MachineEncodedControlEffect::DirectRelativeCallV1
        {
            return Err(());
        }
    }
    if declaration.semantic == MachineSemanticKind::CallAggregate {
        let arity = constraint
            .operands
            .iter()
            .take_while(|operand| operand.access == RegisterOperandAccess::Use)
            .count();
        let (arguments, results) = constraint.operands.split_at(arity);
        if results.len() > 2
            || (results.is_empty() && arguments.is_empty())
            || results.iter().any(|operand| {
                operand.access != RegisterOperandAccess::Def || operand.fixed_view.is_none()
            })
            || !call_roster_matches(encoded, constraint, arguments, results)
            || encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            || encoded.control != MachineEncodedControlEffect::DirectRelativeCallV1
        {
            return Err(());
        }
    }
    if declaration.semantic == MachineSemanticKind::CallScalar {
        let (result, arguments) = constraint.operands.split_last().ok_or(())?;
        if result.access != RegisterOperandAccess::Def
            || arguments
                .iter()
                .any(|argument| argument.access != RegisterOperandAccess::Use)
            || !call_roster_matches(encoded, constraint, arguments, std::slice::from_ref(result))
            || encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            || encoded.control != MachineEncodedControlEffect::DirectRelativeCallV1
        {
            return Err(());
        }
    }
    if declaration.semantic == MachineSemanticKind::CallUnit
        && (constraint
            .operands
            .iter()
            .any(|operand| operand.access != RegisterOperandAccess::Use)
            || !call_roster_matches(encoded, constraint, &constraint.operands, &[])
            || encoded.control != MachineEncodedControlEffect::DirectRelativeCallV1)
    {
        return Err(());
    }
    if declaration.semantic == MachineSemanticKind::NormalizedForeignCall {
        let supervisor =
            constraint.key.family == register_model::RegisterConstraintFamily::SystemCall;
        let control = if supervisor {
            if encoded.memory != MachineEncodedMemoryEffect::NoneV1
                || encoded.stack != MachineEncodedStackEffect::UnchangedV1
            {
                return Err(());
            }
            MachineEncodedControlEffect::SupervisorCallV1
        } else if matches!(
            encoded.memory,
            MachineEncodedMemoryEffect::ReadTableFieldV1 { .. }
        ) {
            MachineEncodedControlEffect::IndirectExternalCallV1
        } else {
            MachineEncodedControlEffect::DirectRelativeCallV1
        };
        let arity = constraint
            .operands
            .iter()
            .take_while(|operand| operand.access == RegisterOperandAccess::Use)
            .count();
        let (arguments, results) = constraint.operands.split_at(arity);
        if constraint
            .operands
            .iter()
            .any(|operand| operand.fixed_view.is_none())
            || results
                .iter()
                .any(|operand| operand.access != RegisterOperandAccess::Def)
            || !call_roster_matches(encoded, constraint, arguments, results)
            || encoded.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1
            || encoded.control != control
        {
            return Err(());
        }
    }
    if has_policy_trap(declaration.semantic)
        != (encoded.control == MachineEncodedControlEffect::FallThroughOrTrapV1)
        || (declaration.semantic == MachineSemanticKind::InstalledInterruptTaskCommit)
            != (encoded.control == MachineEncodedControlEffect::FallThroughOrStopV1)
        || has_policy_trap(declaration.semantic)
            != (encoded.trap == MachineEncodedTrapBehavior::TrappingIntegerV1)
    {
        return Err(());
    }
    let expected_barrier = match encoded.control {
        MachineEncodedControlEffect::AtomicRetryUntilCommitV1
            if matches!(declaration.semantic, MachineSemanticKind::AtomicAccess(form)
                    if form.operation.retries_until_commit(architecture)) =>
        {
            MachineBarrier::ExternalEffect
        }
        MachineEncodedControlEffect::AtomicRetryUntilCommitV1 => return Err(()),
        MachineEncodedControlEffect::CrashV1 | MachineEncodedControlEffect::PrivilegedStopV1 => {
            return Err(());
        }
        MachineEncodedControlEffect::FallThroughOrTrapV1
        | MachineEncodedControlEffect::FallThroughOrStopV1 => MachineBarrier::ExternalEffect,
        MachineEncodedControlEffect::HostedReadReturnOrTrapV1
        | MachineEncodedControlEffect::HostedExitOrTrapV1
        | MachineEncodedControlEffect::HostedWriteReturnOrTrapV1 => MachineBarrier::ExternalEffect,
        MachineEncodedControlEffect::FallThroughV1
            if touches_external_state(declaration.semantic) =>
        {
            MachineBarrier::ExternalEffect
        }
        MachineEncodedControlEffect::FallThroughV1 => MachineBarrier::None,
        MachineEncodedControlEffect::DirectRelativeCallV1
        | MachineEncodedControlEffect::IndirectTableCallOrTrapV1
        | MachineEncodedControlEffect::IndirectExternalCallV1 => MachineBarrier::Call,
        MachineEncodedControlEffect::SupervisorCallV1
            if declaration.semantic == MachineSemanticKind::NormalizedForeignCall
                && declaration.call == SupervisorNormalReturnV1 =>
        {
            MachineBarrier::Call
        }
        MachineEncodedControlEffect::SupervisorCallV1 => return Err(()),
        MachineEncodedControlEffect::ConditionalRelativeBranchV1
        | MachineEncodedControlEffect::UnconditionalRelativeBranchV1
        | MachineEncodedControlEffect::ReturnFromActivationStackV1
        | MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. } => {
            MachineBarrier::ControlFlow
        }
    };
    if declaration.barrier != expected_barrier {
        return Err(());
    }
    let control_shape_matches = match declaration.semantic {
        MachineSemanticKind::ConditionalBranchNonZero
        | MachineSemanticKind::ConditionalBranchU64LessThan
        | MachineSemanticKind::ConditionalBranchI64LessThan => {
            encoded.control == MachineEncodedControlEffect::ConditionalRelativeBranchV1
        }
        MachineSemanticKind::Jump => {
            encoded.control == MachineEncodedControlEffect::UnconditionalRelativeBranchV1
        }
        MachineSemanticKind::ReturnScalar
        | MachineSemanticKind::ReturnAggregate
        | MachineSemanticKind::ReturnUnit => matches!(
            encoded.control,
            MachineEncodedControlEffect::ReturnFromActivationStackV1
                | MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. }
        ),
        _ => true,
    };
    if !control_shape_matches {
        return Err(());
    }
    if matches!(
        declaration.semantic,
        MachineSemanticKind::SaveFloatingControl | MachineSemanticKind::RestoreFloatingControl
    ) && (!constraint.operands.is_empty()
        || !encoded.external_operand_reads.is_empty()
        || !encoded.external_operand_writes.is_empty()
        || !implicit_roster_matches(encoded, constraint)
        || encoded.control != MachineEncodedControlEffect::FallThroughV1)
    {
        return Err(());
    }
    match (encoded.memory, encoded.stack, encoded.trap) {
        (
            MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                stack_pointer: memory_pointer,
                byte_count: 8,
            },
            MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if memory_pointer == stack_pointer
            && declaration.memory == crate::MachineMemoryEffect::TemporaryStackStorageV1
            && matches!(
                declaration.semantic,
                MachineSemanticKind::SystemRegisterRead | MachineSemanticKind::SystemRegisterWrite
            ) => {}
        (
            MachineEncodedMemoryEffect::TemporaryStackStorageV1 {
                stack_pointer: memory_pointer,
                byte_count: 16,
            },
            MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: 16,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if memory_pointer == stack_pointer
            && declaration.memory == crate::MachineMemoryEffect::TemporaryStackStorageV1
            && matches!(declaration.semantic, MachineSemanticKind::NearestIeeeFloatArithmetic(form) if form.operation.rounding_direction().is_some()) =>
            {}
        (
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                byte_count: 4 | 8, ..
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::SaveFloatingControl
            && declaration.memory == crate::MachineMemoryEffect::WriteFrameStorageV1 => {}
        (
            MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                byte_count: 4 | 8, ..
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::RestoreFloatingControl
            && declaration.memory == ReadFrameStorageV1 => {}
        (
            MachineEncodedMemoryEffect::NoneV1,
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::HostedExitReturnedV1,
        ) if declaration.semantic == MachineSemanticKind::HostedExitProcessI32
            && declaration.memory == crate::MachineMemoryEffect::NoneV1
            && declaration.trap == HostedExitReturnedV1
            && encoded.control == MachineEncodedControlEffect::HostedExitOrTrapV1
            && encoded.external_operand_reads == [0]
            && encoded.external_operand_writes.is_empty()
            && constraint.operands.len() == 1
            && constraint.operands[0].access == RegisterOperandAccess::Use
            && implicit_roster_matches(encoded, constraint) => {}
        (
            MachineEncodedMemoryEffect::HostedReadByteV1 { .. },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::HostedReadFailureV1,
        ) if declaration.semantic == MachineSemanticKind::HostedReadByte
            && declaration.memory == crate::MachineMemoryEffect::HostedReadByteV1
            && declaration.trap == crate::MachineTrapBehavior::HostedReadFailureV1
            && encoded.control == MachineEncodedControlEffect::HostedReadReturnOrTrapV1
            && encoded.external_operand_reads.is_empty()
            && encoded.external_operand_writes.is_empty()
            && constraint.operands.is_empty()
            && implicit_roster_matches(encoded, constraint) => {}
        (
            MachineEncodedMemoryEffect::HostedWriteByteV1 { .. },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::HostedWriteFailureV1,
        ) if declaration.semantic == MachineSemanticKind::HostedWriteByteI32
            && declaration.memory == crate::MachineMemoryEffect::HostedWriteByteV1
            && declaration.trap == crate::MachineTrapBehavior::HostedWriteFailureV1
            && encoded.control == MachineEncodedControlEffect::HostedWriteReturnOrTrapV1
            && encoded.external_operand_reads == [0]
            && encoded.external_operand_writes.is_empty()
            && constraint.operands.len() == 1
            && constraint.operands[0].access == RegisterOperandAccess::Use
            && implicit_roster_matches(encoded, constraint) => {}
        (
            MachineEncodedMemoryEffect::CopyBytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
                count_operand: 2,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::CopyBytes
            && declaration.memory == crate::MachineMemoryEffect::CopyBytesV1
            && declaration.trap == MayArchitecturalFaultV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1, 2]
            && encoded.external_operand_writes == [3, 4]
            && positional_roster(constraint, 3, 5)
            && implicit_roster_matches(encoded, constraint) => {}
        (
            MachineEncodedMemoryEffect::ZeroBytesV1 {
                destination_pointer_operand: 0,
                count_operand: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::ZeroBytes
            && declaration.memory == crate::MachineMemoryEffect::ZeroBytesV1
            && declaration.trap == MayArchitecturalFaultV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1]
            && encoded.external_operand_writes == [2]
            && positional_roster(constraint, 2, 3)
            && implicit_roster_matches(encoded, constraint) => {}
        (
            MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::LoadEncodedScalar
            && declaration.memory == crate::MachineMemoryEffect::ReadEncodedScalarV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1]
            && encoded.external_operand_writes == [2, 3, 4, 5]
            && encoded_scalar_constraint(constraint, 2) => {}
        (
            MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::StoreEncodedScalar
            && declaration.memory == crate::MachineMemoryEffect::WriteEncodedScalarV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1, 2]
            && encoded.external_operand_writes == [3, 4, 5]
            && encoded_scalar_constraint(constraint, 3) => {}
        (
            MachineEncodedMemoryEffect::ScanByteSequenceV1 {
                base_pointer_operand: 0,
                count_operand: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::ScanByteSequence
            && declaration.memory == crate::MachineMemoryEffect::ScanByteSequenceV1
            && declaration.trap == MayArchitecturalFaultV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1]
            && encoded.external_operand_writes == [2, 3, 4]
            && positional_roster(constraint, 2, 5)
            && implicit_roster_matches(encoded, constraint) => {}
        (
            MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                left_base_operand: 0,
                left_count_operand: 1,
                right_base_operand: 2,
                right_count_operand: 3,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::CompareByteSequences
            && declaration.memory == CompareByteSequencesV1
            && declaration.trap == MayArchitecturalFaultV1
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && encoded.external_operand_reads == [0, 1, 2, 3]
            && encoded.external_operand_writes == [4, 5, 6, 7]
            && positional_roster(constraint, 4, 8)
            && implicit_roster_matches(encoded, constraint) => {}
        (
            MachineEncodedMemoryEffect::PrivateCallbackMaterializationV1,
            stack @ (MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                return_address_byte_count: 8,
                ..
            }
            | MachineEncodedStackEffect::UnchangedV1),
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::NormalizedForeignCall
            && declaration.memory
                == crate::MachineMemoryEffect::PrivateCallbackMaterializationV1
            && declaration.trap == MayArchitecturalFaultV1
            && encoded.control == MachineEncodedControlEffect::DirectRelativeCallV1
            && match architecture {
                target::Architecture::X86_64 => matches!(
                    stack,
                    MachineEncodedStackEffect::CallReturnAddressLifecycleV1 { .. }
                ),
                target::Architecture::Aarch64 => stack == MachineEncodedStackEffect::UnchangedV1,
            } => {}
        (
            MachineEncodedMemoryEffect::ReadTableFieldV1 {
                pointer_operand, ..
            },
            stack @ (MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                return_address_byte_count: 8,
                ..
            }
            | MachineEncodedStackEffect::UnchangedV1),
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::NormalizedForeignCall
            && declaration.memory == crate::MachineMemoryEffect::ReadPointerV1
            && declaration.trap == MayArchitecturalFaultV1
            && match architecture {
                target::Architecture::X86_64 => matches!(
                    stack,
                    MachineEncodedStackEffect::CallReturnAddressLifecycleV1 { .. }
                ),
                target::Architecture::Aarch64 => stack == MachineEncodedStackEffect::UnchangedV1,
            }
            && encoded.control == MachineEncodedControlEffect::IndirectExternalCallV1
            && encoded.external_operand_reads.contains(&pointer_operand) => {}
        (
            MachineEncodedMemoryEffect::ReadIndexedPointerV1 {
                pointer_operand,
                index_operand,
                byte_count: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::Load8Indexed
            && declaration.memory == crate::MachineMemoryEffect::ReadPointerV1
            && pointer_operand != index_operand
            && encoded.external_operand_reads.contains(&pointer_operand)
            && encoded.external_operand_reads.contains(&index_operand) => {}
        (
            MachineEncodedMemoryEffect::ReadActivationStackV1 {
                stack_pointer: memory_pointer,
                byte_count: memory_bytes,
            },
            MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer,
                byte_count: stack_bytes,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if matches!(
            declaration.semantic,
            MachineSemanticKind::ReturnScalar
                | MachineSemanticKind::ReturnAggregate
                | MachineSemanticKind::ReturnUnit
        ) && memory_pointer == stack_pointer
            && memory_bytes == stack_bytes
            && memory_bytes != 0 => {}
        (
            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                stack_pointer: memory_pointer,
                byte_count: memory_bytes,
            },
            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                stack_pointer,
                return_address_byte_count,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if matches!(
            declaration.semantic,
            MachineSemanticKind::CallScalar
                | MachineSemanticKind::InvokeNoNormal
                | MachineSemanticKind::CallUnit
                | MachineSemanticKind::CallAggregate
                | MachineSemanticKind::NormalizedForeignCall
        ) && memory_pointer == stack_pointer
            && memory_bytes == return_address_byte_count
            && return_address_byte_count != 0 => {}
        (
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 10,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::GlobalDescriptorTableLoad
            && declaration.memory == crate::MachineMemoryEffect::ReadPointerV1
            && encoded.external_operand_reads == [0]
            && encoded.external_operand_writes.is_empty()
            && encoded.control == MachineEncodedControlEffect::FallThroughV1 => {}
        (
            MachineEncodedMemoryEffect::SegmentDescriptorReloadV1 {
                table_pointer_operand: 0,
                selector_operand: 1,
                scratch_pointer_operand: 2,
                table_byte_count_operand: 3,
                stack_pointer: memory_stack,
            },
            MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if matches!(
            (declaration.semantic, byte_count),
            (MachineSemanticKind::CodeSegmentReload, 16)
                | (MachineSemanticKind::DataSegmentReload, 8)
        ) && declaration.memory == crate::MachineMemoryEffect::SegmentDescriptorReloadV1
            && memory_stack == stack_pointer
            && encoded.external_operand_reads == [0, 1, 2, 3]
            && encoded.external_operand_writes == [4]
            && encoded.control == MachineEncodedControlEffect::FallThroughV1 => {}
        (
            MachineEncodedMemoryEffect::InstalledInterruptCpuCheckV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                scratch_pointer_operand: 2,
                stack_pointer: memory_stack,
            },
            MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::InstalledInterruptCpuCheck
            && declaration.memory == crate::MachineMemoryEffect::InstalledInterruptCpuCheckV1
            && memory_stack == stack_pointer
            && encoded.external_operand_reads == [0, 1, 2]
            && encoded.external_operand_writes == [3]
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && implicit_roster_matches(encoded, constraint) => {}
        (
            MachineEncodedMemoryEffect::InstalledInterruptTaskCommitV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                scratch_pointer_operand: 2,
                stack_pointer: memory_stack,
            },
            MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::InstalledInterruptTaskCommit
            && declaration.memory == crate::MachineMemoryEffect::InstalledInterruptTaskCommitV1
            && memory_stack == stack_pointer
            && encoded.external_operand_reads == [0, 1, 2]
            && encoded.external_operand_writes.is_empty()
            && encoded.control == MachineEncodedControlEffect::FallThroughOrStopV1
            && implicit_roster_matches(encoded, constraint) => {}
        (
            MachineEncodedMemoryEffect::InstalledInterruptTablePublicationV1 {
                table_pointer_operand: 0,
                scratch_pointer_operand: 1,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::InstalledInterruptTablePublication
            && declaration.memory
                == crate::MachineMemoryEffect::InstalledInterruptTablePublicationV1
            && encoded.external_operand_reads == [0, 1]
            && encoded.external_operand_writes.is_empty()
            && encoded.control == MachineEncodedControlEffect::FallThroughV1
            && implicit_roster_matches(encoded, constraint) => {}
        (
            MachineEncodedMemoryEffect::TaskRegisterActivationV1 {
                table_pointer_operand: 0,
                task_pointer_operand: 1,
                selector_operand: 2,
                scratch_pointer_operand: 3,
                table_byte_count_operand: 4,
                stack_pointer: memory_stack,
            },
            MachineEncodedStackEffect::TemporaryBytesV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::TaskRegisterActivate
            && declaration.memory == crate::MachineMemoryEffect::TaskRegisterActivationV1
            && memory_stack == stack_pointer
            && encoded.external_operand_reads == [0, 1, 2, 3, 4]
            && encoded.external_operand_writes == [5]
            && encoded.control == MachineEncodedControlEffect::FallThroughV1 => {}
        (
            MachineEncodedMemoryEffect::WriteFixedPointerV1 {
                pointer_operand: 0,
                byte_count: 10,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::GlobalDescriptorTableStore
            && declaration.memory == WritePointerV1
            && encoded.external_operand_reads == [0]
            && encoded.external_operand_writes.is_empty()
            && encoded.control == MachineEncodedControlEffect::FallThroughV1 => {}
        (
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if matches!(declaration.semantic, MachineSemanticKind::AccessorScalarAccess(form) if !form.write && form.ordinary_semantic().is_some() && u16::from(form.bytes) == byte_count)
            && declaration.memory == crate::MachineMemoryEffect::ReadPointerV1
            && encoded.external_operand_reads == [0]
            && encoded.external_operand_writes == [1]
            && encoded.control == MachineEncodedControlEffect::FallThroughV1 => {}
        (
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if matches!(declaration.semantic, MachineSemanticKind::AccessorScalarAccess(form) if form.write && form.ordinary_semantic().is_some())
            && declaration.memory == WritePointerV1
            && encoded.external_operand_reads == [0, 1]
            && encoded.external_operand_writes.is_empty()
            && encoded.control == MachineEncodedControlEffect::FallThroughV1 => {}
        (
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand,
                byte_count,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.memory == crate::MachineMemoryEffect::ReadPointerV1
            && matches!(
                (declaration.semantic, byte_count),
                (MachineSemanticKind::LoadPacked3, 3)
                    | (MachineSemanticKind::LoadPacked5, 5)
                    | (MachineSemanticKind::LoadPacked6, 6)
                    | (MachineSemanticKind::LoadPacked7, 7)
                    | (MachineSemanticKind::Load8, 1)
                    | (MachineSemanticKind::Load16, 2)
                    | (MachineSemanticKind::Load32, 4)
                    | (MachineSemanticKind::Load64, 8)
            )
            && encoded.external_operand_reads.contains(&pointer_operand) => {}
        (
            MachineEncodedMemoryEffect::WriteFrameStorageV1 { byte_count: 8, .. },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::Store64
            && declaration.memory == crate::MachineMemoryEffect::WriteFrameStorageV1 => {}
        (
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if matches!(
            declaration.semantic,
            crate::MachineSemanticKind::Store | crate::MachineSemanticKind::StorePacked
        ) && declaration.memory == WritePointerV1
            && encoded.external_operand_reads == [0, 1]
            && (if declaration.semantic == crate::MachineSemanticKind::StorePacked {
                encoded.external_operand_writes == [2]
            } else {
                encoded.external_operand_writes.is_empty()
            }) => {}
        (
            MachineEncodedMemoryEffect::NoneV1,
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::TrappingIntegerV1,
        ) if has_policy_trap(declaration.semantic)
            && declaration.memory == crate::MachineMemoryEffect::NoneV1
            && declaration.trap == crate::MachineTrapBehavior::TrappingIntegerV1
            && encoded.control == MachineEncodedControlEffect::FallThroughOrTrapV1 => {}
        (
            MachineEncodedMemoryEffect::CatalogFenceV1,
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::NeverV1,
        ) if declaration.semantic == MachineSemanticKind::MemoryFence
            && declaration.memory == crate::MachineMemoryEffect::CatalogFenceV1
            && encoded == &crate::MachineEncodedEffects::catalog_fence_v1()
            && constraint.operands.is_empty() => {}
        (
            MachineEncodedMemoryEffect::AtomicFenceV1,
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::NeverV1,
        ) if declaration.semantic == MachineSemanticKind::AtomicFence
            && declaration.memory == crate::MachineMemoryEffect::AtomicFenceV1
            && encoded == &crate::MachineEncodedEffects::atomic_fence_v1()
            && constraint.operands.is_empty() => {}
        (
            MachineEncodedMemoryEffect::AtomicPointerV1 {
                pointer_operand: 0,
                form,
            },
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.semantic == MachineSemanticKind::AtomicAccess(form)
            && declaration.memory == crate::MachineMemoryEffect::AtomicPointerV1
            && encoded.control
                == if form.operation.retries_until_commit(architecture) {
                    MachineEncodedControlEffect::AtomicRetryUntilCommitV1
                } else {
                    MachineEncodedControlEffect::FallThroughV1
                }
            && constraint.operands.len() == usize::from(form.operand_count())
            && encoded
                .external_operand_reads
                .iter()
                .copied()
                .eq(0..form.operation.input_count())
            && encoded
                .external_operand_writes
                .iter()
                .copied()
                .eq(form.output_operands())
            && constraint
                .operands
                .iter()
                .enumerate()
                .all(|(index, operand)| {
                    if usize::from(operand.operand) != index || operand.tied_to.is_some() {
                        return false;
                    }
                    if form.output_operands().contains(&operand.operand) {
                        operand.access == RegisterOperandAccess::Def
                            && (operand.early_clobber
                                || (encoded.control
                                    == MachineEncodedControlEffect::AtomicRetryUntilCommitV1
                                    && operand.fixed_view.is_some()
                                    && constraint.operands
                                        [..usize::from(form.operation.input_count())]
                                        .iter()
                                        .all(|input| {
                                            input.fixed_view.is_some()
                                                && input.fixed_view != operand.fixed_view
                                        })))
                    } else {
                        operand.access == RegisterOperandAccess::Use && !operand.early_clobber
                    }
                }) => {}
        (
            MachineEncodedMemoryEffect::NoneV1,
            MachineEncodedStackEffect::UnchangedV1,
            MachineEncodedTrapBehavior::NeverV1
            | MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ) if declaration.memory == crate::MachineMemoryEffect::NoneV1 => {}
        _ => return Err(()),
    }
    Ok(())
}

fn validate_applicability(
    constraint: &RegisterInstructionConstraint,
    applicability: MachineAlternativeApplicability,
) -> Result<(), ()> {
    let operand = |number| constraint.operand(number);
    let reads = |access| {
        matches!(
            access,
            RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
        )
    };
    let writes = |access| {
        matches!(
            access,
            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
        )
    };
    match applicability {
        MachineAlternativeApplicability::Always => Ok(()),
        MachineAlternativeApplicability::ResultAliasesOperand {
            result,
            operand: input,
        } => {
            let (Some(result), Some(input)) = (operand(result), operand(input)) else {
                return Err(());
            };
            (result.operand != input.operand
                && writes(result.access)
                && reads(input.access)
                && result.class == input.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            let (Some(result), Some(aliased), Some(distinct)) = (
                operand(result),
                operand(aliased_operand),
                operand(distinct_operand),
            ) else {
                return Err(());
            };
            (result.operand != aliased.operand
                && result.operand != distinct.operand
                && aliased.operand != distinct.operand
                && writes(result.access)
                && reads(aliased.access)
                && reads(distinct.access)
                && result.class == aliased.class
                && result.class == distinct.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            let (Some(result), Some(left), Some(right)) =
                (operand(result), operand(left), operand(right))
            else {
                return Err(());
            };
            (result.operand != left.operand
                && result.operand != right.operand
                && left.operand != right.operand
                && writes(result.access)
                && reads(left.access)
                && reads(right.access)
                && result.class == left.class
                && result.class == right.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            let (Some(result), Some(left), Some(right)) =
                (operand(result), operand(left), operand(right))
            else {
                return Err(());
            };
            (result.operand != left.operand
                && result.operand != right.operand
                && left.operand != right.operand
                && writes(result.access)
                && reads(left.access)
                && reads(right.access)
                && result.class == left.class
                && result.class == right.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left, right, ..
        } => {
            let (Some(left), Some(right)) = (operand(left), operand(right)) else {
                return Err(());
            };
            (left.operand != right.operand
                && reads(left.access)
                && reads(right.access)
                && left.class == right.class)
                .then_some(())
                .ok_or(())
        }
        MachineAlternativeApplicability::OptimizationSelected { optimization } => (optimization
            .execution_phase()
            == optimization_core::OptimizationExecutionPhase::PostAllocationMachine)
            .then_some(())
            .ok_or(()),
    }
}
