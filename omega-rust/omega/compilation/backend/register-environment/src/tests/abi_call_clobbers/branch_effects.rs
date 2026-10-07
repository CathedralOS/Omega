use super::{
    Architecture, BTreeSet, EffectRejection, RegisterConstraintKey,
    baseline_target_register_environment, convention_for, produced_effects, scalar_abi_cases,
    validate_effects, validated_effects,
};
use selected_instructions::{
    MachineAlternativeApplicability, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalog, MachineEffectCatalogValidationError, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior,
};

/// Every selected branch rule key with the semantic family the effect
/// catalog must declare for it. The three conditional semantics share one
/// constraint key; the sweep is keyed off the validated environment's
/// selection, never off the effect producer's own roster.
fn selected_branch_rules(
    environment: &crate::ValidatedTargetRegisterEnvironment,
) -> Vec<(RegisterConstraintKey, MachineSemanticKind)> {
    let keys = environment.selected_keys();
    [
        MachineSemanticKind::ConditionalBranchNonZero,
        MachineSemanticKind::ConditionalBranchU64LessThan,
        MachineSemanticKind::ConditionalBranchI64LessThan,
    ]
    .into_iter()
    .map(|semantic| (keys.conditional_branch, semantic))
    .chain(std::iter::once((keys.jump, MachineSemanticKind::Jump)))
    .collect()
}

/// The declared branch contract one selected branch semantic must carry on
/// every target/ABI pair: whether the rule reads the architecture's
/// condition-code view, and the exact encoded control shape inside the
/// control-flow barrier class.
struct BranchContract {
    conditional: bool,
    control: MachineEncodedControlEffect,
}

fn branch_contract(semantic: MachineSemanticKind) -> BranchContract {
    use MachineSemanticKind::*;
    match semantic {
        ConditionalBranchNonZero | ConditionalBranchU64LessThan | ConditionalBranchI64LessThan => {
            BranchContract {
                conditional: true,
                control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
            }
        }
        Jump => BranchContract {
            conditional: false,
            control: MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
        },
        other => panic!("{other:?} is not a selected branch rule"),
    }
}

/// The declared size knowledge for one selected branch semantic on each ISA:
/// the x86-64 conditional envelope and near forms, the widened AArch64
/// conditional encodings, and the fixed-width AArch64 unconditional `B`.
fn expected_size(
    semantic: MachineSemanticKind,
    architecture: Architecture,
) -> MachineSizeKnowledge {
    use MachineSemanticKind::*;
    match architecture {
        Architecture::X86_64 => match semantic {
            ConditionalBranchNonZero => MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 2,
                maximum_bytes: Some(6),
            },
            ConditionalBranchU64LessThan | ConditionalBranchI64LessThan => {
                MachineSizeKnowledge::ExactBytes(6)
            }
            Jump => MachineSizeKnowledge::ExactBytes(5),
            other => panic!("{other:?} is not a selected branch rule"),
        },
        Architecture::Aarch64 => match semantic {
            // `B.cond` when the taken edge fits imm19; widens to
            // `B.<invcond> +8; B target` past it.
            ConditionalBranchNonZero
            | ConditionalBranchU64LessThan
            | ConditionalBranchI64LessThan => MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(8),
            },
            Jump => MachineSizeKnowledge::ExactBytes(4),
            other => panic!("{other:?} is not a selected branch rule"),
        },
    }
}

/// Conditional-branch declarations share one constraint key, so branch
/// declarations are found by (key, semantic) rather than by key alone.
fn branch_declaration_mut(
    catalog: &mut MachineEffectCatalog,
    key: RegisterConstraintKey,
    semantic: MachineSemanticKind,
) -> &mut MachineEffectDeclaration {
    catalog
        .declarations
        .iter_mut()
        .find(|declaration| declaration.constraint == key && declaration.semantic == semantic)
        .unwrap_or_else(|| panic!("effect catalog missing branch declaration {key:?}/{semantic:?}"))
}

/// Every selected branch rule on every declared target/ABI pair is bound to
/// the control-transfer mechanism and register custody that pair's ABI
/// declares.
#[test]
fn every_selected_branch_rule_binds_the_declared_abi_control_contract() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let convention = convention_for(case, environment.physical());
        let catalog = validated_effects(case, environment.constraints());
        let keys = environment.selected_keys();
        let branch_rules = selected_branch_rules(&environment);
        assert_eq!(
            branch_rules.len(),
            4,
            "{} selects an unexpected branch roster",
            case.convention
        );

        // Branch authority is declared for exactly the selected branch
        // rules: no selected rule may lack a branch declaration and no other
        // declaration may claim a branch semantic or a relative-branch
        // encoding.
        let declared_branches = catalog
            .catalog()
            .declarations
            .iter()
            .filter(|declaration| {
                matches!(
                    declaration.semantic,
                    MachineSemanticKind::ConditionalBranchNonZero
                        | MachineSemanticKind::ConditionalBranchU64LessThan
                        | MachineSemanticKind::ConditionalBranchI64LessThan
                        | MachineSemanticKind::Jump
                )
            })
            .map(|declaration| (declaration.constraint, declaration.semantic))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            declared_branches,
            branch_rules.iter().copied().collect::<BTreeSet<_>>(),
            "{}",
            case.convention
        );
        let encoded_branches = catalog
            .catalog()
            .declarations
            .iter()
            .filter(|declaration| {
                declaration.alternatives.iter().any(|alternative| {
                    matches!(
                        alternative.encoded.control,
                        MachineEncodedControlEffect::ConditionalRelativeBranchV1
                            | MachineEncodedControlEffect::UnconditionalRelativeBranchV1
                    )
                })
            })
            .map(|declaration| (declaration.constraint, declaration.semantic))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            encoded_branches,
            branch_rules.iter().copied().collect::<BTreeSet<_>>(),
            "{}",
            case.convention
        );

        // The catalog is bound to this exact validated constraint catalog,
        // target, and key selection — provenance, not a re-derived roster.
        assert_eq!(catalog.catalog().target, case.target);
        assert_eq!(
            catalog.catalog().register_constraints,
            environment.constraints().identity()
        );
        assert_eq!(catalog.catalog().selected_keys, keys);

        let flags = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rflags",
                Architecture::Aarch64 => "nzcv",
            })
            .expect("branch matrix names a condition-codes view");
        let program_counter = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rip",
                Architecture::Aarch64 => "pc",
            })
            .expect("branch matrix names a program-counter view");
        // The ABI fixes the program counter and leaves the condition codes
        // volatile: a branch may consume both but can never touch preserved
        // state.
        assert!(
            program_counter
                .units
                .iter()
                .all(|unit| convention.fixed.contains(unit)),
            "{} must fix the program counter",
            case.convention
        );
        assert!(
            flags.units.iter().all(|unit| {
                convention.caller_saved.contains(unit)
                    && !convention.callee_saved.contains(unit)
                    && !convention.fixed.contains(unit)
            }),
            "{} must keep the condition codes volatile",
            case.convention
        );

        for (key, semantic) in &branch_rules {
            let contract = branch_contract(*semantic);
            let row = environment
                .constraint(*key)
                .unwrap_or_else(|| panic!("environment missing selected branch row {key:?}"));
            let declaration = catalog
                .catalog()
                .declarations
                .iter()
                .find(|entry| entry.constraint == *key && entry.semantic == *semantic)
                .unwrap_or_else(|| panic!("effect catalog missing branch declaration {key:?}"));
            assert_eq!(declaration.semantic, *semantic, "{key:?}");
            assert_eq!(declaration.barrier, MachineBarrier::ControlFlow, "{key:?}");
            assert_eq!(declaration.call, MachineCallEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.cleanup, MachineCleanupEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.memory, MachineMemoryEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.trap, MachineTrapBehavior::NeverV1, "{key:?}");

            // A selected branch carries no register operands: its whole
            // custody is the implicit condition-code read on a conditional
            // form plus the program-counter use/def pair the control
            // transfer itself requires, and it clobbers nothing.
            assert!(row.operands.is_empty(), "{key:?}");
            assert_eq!(
                row.implicit_uses,
                if contract.conditional {
                    flags
                        .units
                        .iter()
                        .chain(program_counter.units.iter())
                        .copied()
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect()
                } else {
                    program_counter.units.clone()
                },
                "{key:?}"
            );
            assert_eq!(row.implicit_defs, program_counter.units, "{key:?}");
            assert!(row.clobbers.is_empty(), "{key:?}");
            assert!(
                row.implicit_uses
                    .iter()
                    .chain(&row.implicit_defs)
                    .all(|unit| !convention.callee_saved.contains(unit)),
                "{key:?} implicit state must not include callee-saved units"
            );

            // One canonical alternative replays the exact ABI branch
            // mechanism for this target.
            assert_eq!(declaration.alternatives.len(), 1, "{key:?}");
            let alternative = &declaration.alternatives[0];
            assert_eq!(alternative.key.family, (*semantic).into(), "{key:?}");
            assert_eq!(alternative.key.variant, 0, "{key:?}");
            assert_eq!(
                alternative.applicability,
                MachineAlternativeApplicability::Always,
                "{key:?}"
            );
            assert_eq!(
                alternative.size,
                expected_size(*semantic, case.target.architecture),
                "{key:?}"
            );
            let encoded = &alternative.encoded;
            assert!(encoded.external_operand_reads.is_empty(), "{key:?}");
            assert!(encoded.external_operand_writes.is_empty(), "{key:?}");
            // Encoded implicit state refines exactly to the constraint row's
            // declared custody for every selected branch rule.
            assert_eq!(encoded.implicit_unit_uses, row.implicit_uses, "{key:?}");
            assert_eq!(encoded.implicit_unit_defs, row.implicit_defs, "{key:?}");
            assert_eq!(encoded.implicit_unit_clobbers, row.clobbers, "{key:?}");
            assert_eq!(
                encoded.memory,
                MachineEncodedMemoryEffect::NoneV1,
                "{key:?}"
            );
            assert_eq!(
                encoded.stack,
                MachineEncodedStackEffect::UnchangedV1,
                "{key:?}"
            );
            assert_eq!(
                encoded.trap,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                "{key:?}"
            );
            assert_eq!(encoded.control, contract.control, "{key:?}");
        }
    }
}

/// Every selected branch rule replays its encoded control form: borrowing a
/// sibling control shape, leaving the barrier class, understating the
/// architectural fault, or forging custody must reject for every rule on
/// every declared target/ABI pair.
#[test]
fn every_selected_branch_rule_rejects_encoded_control_forgery_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let catalog = produced_effects(case, environment.constraints());
        let stack_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rsp",
                Architecture::Aarch64 => "sp",
            })
            .unwrap();
        let flags = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rflags",
                Architecture::Aarch64 => "nzcv",
            })
            .unwrap();
        let program_counter = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rip",
                Architecture::Aarch64 => "pc",
            })
            .unwrap();
        for (key, semantic) in selected_branch_rules(&environment) {
            let contract = branch_contract(semantic);

            // A branch cannot borrow a sibling control shape inside the
            // control-flow barrier class: the conditional forms reject the
            // unconditional encoding, and Jump rejects the conditional one.
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .control = if contract.conditional {
                MachineEncodedControlEffect::UnconditionalRelativeBranchV1
            } else {
                MachineEncodedControlEffect::ConditionalRelativeBranchV1
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} borrowed sibling control shape must reject"
            );

            // A return encoding is inside the barrier class but outside the
            // branch shape.
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .control = MachineEncodedControlEffect::ReturnFromActivationStackV1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} borrowed return control shape must reject"
            );

            // Leaving the barrier class entirely — fall-through, call, or
            // hosted exit — fails the encoded control/barrier join.
            for control in [
                MachineEncodedControlEffect::FallThroughV1,
                MachineEncodedControlEffect::DirectRelativeCallV1,
                MachineEncodedControlEffect::HostedExitOrTrapV1,
            ] {
                let mut corrupted = catalog.clone();
                branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                    .encoded
                    .control = control;
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} foreign control shape {control:?} must reject"
                );
            }

            // Understating the architectural fault stays inside the admitted
            // fallthrough trap surface, so only canonical ISA replay rejects it.
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .trap = MachineEncodedTrapBehavior::NeverV1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::SemanticMismatch),
                "{key:?} understated encoded trap must reject"
            );

            // A hosted trap shape is outside the admitted branch surface.
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .trap = MachineEncodedTrapBehavior::HostedReadFailureV1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} hosted trap shape must reject"
            );

            // A branch cannot grow a memory footprint or a stack lifecycle.
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 8,
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} forged memory footprint must reject"
            );
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .stack = MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer: stack_pointer.id,
                byte_count: 8,
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} forged stack lifecycle must reject"
            );

            // The operand-free row admits no encoded operand custody.
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .external_operand_reads
                .push(0);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} forged operand read must reject"
            );
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .external_operand_writes
                .push(0);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} forged operand write must reject"
            );

            // Losing a contracted implicit unit understates the row's
            // custody and fails structural admission: the condition-code
            // use on a conditional row, and the program-counter use and
            // definition on either row.
            if contract.conditional {
                let mut corrupted = catalog.clone();
                branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                    .encoded
                    .implicit_unit_uses
                    .retain(|unit| !flags.units.contains(unit));
                assert_eq!(
                    validate_effects(case, environment.constraints(), corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                    )),
                    "{key:?} lost condition-code use must reject"
                );
            }
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .implicit_unit_uses
                .retain(|unit| !program_counter.units.contains(unit));
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} lost program-counter use must reject"
            );
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .implicit_unit_defs
                .retain(|unit| !program_counter.units.contains(unit));
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} lost program-counter definition must reject"
            );

            // Custody the row does not declare fails structural admission:
            // a foreign implicit unit, or any clobber on a row that declares
            // none.
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .implicit_unit_uses
                .push(stack_pointer.units[0]);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} forged stack-pointer use must reject"
            );
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .implicit_unit_defs
                .push(stack_pointer.units[0]);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} forged stack-pointer definition must reject"
            );
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .encoded
                .implicit_unit_clobbers
                .push(flags.units[0]);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} forged clobber must reject"
            );

            // The single alternative cannot change family, borrow an
            // operand-shaped applicability, or empty out; drifting its
            // variant or size needs canonical replay.
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .key
                .family = if contract.conditional {
                MachineSemanticKind::Jump.into()
            } else {
                MachineSemanticKind::ConditionalBranchNonZero.into()
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::AlternativeFamilyMismatch(semantic)
                )),
                "{key:?} borrowed alternative family must reject"
            );
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0].applicability =
                MachineAlternativeApplicability::ResultAliasesOperand {
                    result: 0,
                    operand: 0,
                };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidAlternativeApplicability(semantic)
                )),
                "{key:?} operand-shaped applicability must reject"
            );
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic)
                .alternatives
                .clear();
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::EmptyAlternatives(semantic)
                )),
                "{key:?} empty alternatives must reject"
            );
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0]
                .key
                .variant = 1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::SemanticMismatch),
                "{key:?} variant drift must reject"
            );
            let mut corrupted = catalog.clone();
            branch_declaration_mut(&mut corrupted, key, semantic).alternatives[0].size =
                MachineSizeKnowledge::ExactBytes(997);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::SemanticMismatch),
                "{key:?} size drift must reject"
            );
        }
    }
}
