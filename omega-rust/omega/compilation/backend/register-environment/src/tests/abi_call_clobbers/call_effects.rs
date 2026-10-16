use super::{
    Architecture, BTreeSet, RegisterConstraintKey, baseline_target_register_environment,
    convention_for, declaration_mut, named_view, produced_effects, scalar_abi_cases,
    validated_effects,
};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectCatalogValidationError,
    MachineEncodedControlEffect, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSemanticKind, MachineTrapBehavior,
};

/// Every selected register-call rule key with the semantic family the effect
/// catalog must declare for it. The sweep is keyed off the validated
/// environment's selection, never off the effect producer's own roster.
fn selected_call_rules(
    environment: &crate::ValidatedTargetRegisterEnvironment,
) -> Vec<(RegisterConstraintKey, MachineSemanticKind)> {
    let keys = environment.selected_keys();
    keys.call_unit
        .iter()
        .chain(&keys.call_unit_mixed)
        .map(|key| (*key, MachineSemanticKind::CallUnit))
        .chain(
            keys.call_scalar
                .iter()
                .map(|key| (*key, MachineSemanticKind::CallScalar)),
        )
        .chain(
            keys.call_aggregate
                .iter()
                .map(|key| (*key, MachineSemanticKind::CallAggregate)),
        )
        .chain(
            keys.call_normalized_foreign
                .iter()
                .map(|key| (*key, MachineSemanticKind::NormalizedForeignCall)),
        )
        .collect()
}

/// Every selected call rule on every declared target/ABI pair is bound to the
/// preservation convention the physical model declares for that pair.
#[test]
fn every_selected_call_rule_binds_the_declared_abi_call_contract() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let convention = convention_for(case, environment.physical());
        let catalog = validated_effects(case, environment.constraints());
        let call_rules = selected_call_rules(&environment);
        assert!(
            !call_rules.is_empty(),
            "{} selects no call rules",
            case.convention
        );

        // Call authority is declared for exactly the selected call rules: no
        // selected rule may lack a call declaration and no other declaration
        // may claim call effects.
        let declared_calls = catalog
            .catalog()
            .declarations
            .iter()
            .filter(|declaration| declaration.call != MachineCallEffect::NoneV1)
            .map(|declaration| (declaration.constraint, declaration.semantic))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            declared_calls,
            call_rules.iter().copied().collect::<BTreeSet<_>>(),
            "{}",
            case.convention
        );

        let stack_pointer = named_view(
            model,
            case.target,
            "rsp",
            "sp",
            "call matrix names a stack-pointer view",
        );

        for (key, semantic) in &call_rules {
            let supervisor = key.family == register_model::RegisterConstraintFamily::SystemCall;
            let row = environment
                .constraint(*key)
                .unwrap_or_else(|| panic!("environment missing selected call row {key:?}"));
            let declaration = catalog
                .catalog()
                .declaration(*key)
                .unwrap_or_else(|| panic!("effect catalog missing call declaration {key:?}"));
            assert_eq!(declaration.semantic, *semantic, "{key:?}");
            assert_eq!(declaration.barrier, MachineBarrier::Call, "{key:?}");
            // The call contract is bound to the declared preservation
            // convention, not to the effect producer's own constant.
            // Imports use that convention's alignment. Supervisor entries
            // share argument transport but have no user-space call frame.
            let expected_call = if supervisor {
                MachineCallEffect::SupervisorNormalReturnV1
            } else if *semantic == MachineSemanticKind::NormalizedForeignCall {
                MachineCallEffect::ExternalNormalReturnV1 {
                    pre_call_stack_alignment: convention.stack_alignment,
                }
            } else {
                MachineCallEffect::InternalNormalReturnV1 {
                    pre_call_stack_alignment: convention.stack_alignment,
                }
            };
            assert_eq!(
                declaration.call, expected_call,
                "{key:?} must honor the {} call alignment",
                convention.name
            );
            assert_eq!(declaration.cleanup, MachineCleanupEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.trap, MachineTrapBehavior::NeverV1, "{key:?}");
            assert_eq!(declaration.memory, MachineMemoryEffect::NoneV1, "{key:?}");

            // ABI operand structure: dense operand numbers, leading Use
            // arguments, trailing Def results, every placement pinned to a
            // caller-saved view of the operand's class.
            let arity = row
                .operands
                .iter()
                .take_while(|operand| operand.access == RegisterOperandAccess::Use)
                .count();
            let (arguments, results) = row.operands.split_at(arity);
            match semantic {
                MachineSemanticKind::CallUnit => {
                    assert!(results.is_empty(), "{key:?}")
                }
                MachineSemanticKind::CallScalar => {
                    assert_eq!(results.len(), 1, "{key:?}")
                }
                MachineSemanticKind::CallAggregate => {
                    assert!(results.len() <= 2, "{key:?}")
                }
                MachineSemanticKind::NormalizedForeignCall => {
                    assert!(results.len() <= 1, "{key:?}")
                }
                _ => unreachable!("the call sweep only contains call semantics"),
            }
            for (position, operand) in row.operands.iter().enumerate() {
                assert_eq!(operand.operand as usize, position, "{key:?}");
                assert!(!operand.early_clobber, "{key:?}");
                assert!(operand.tied_to.is_none(), "{key:?}");
                let view = operand
                    .fixed_view
                    .and_then(|id| model.view(id))
                    .unwrap_or_else(|| panic!("{key:?} operand {position} pins no ABI view"));
                assert_eq!(view.class, operand.class, "{key:?}");
                assert!(
                    view.units
                        .iter()
                        .all(|unit| convention.caller_saved.contains(unit)),
                    "{key:?} operand {position} travels through a non-volatile view {}",
                    view.name
                );
            }

            // A call may only implicitly touch volatile or architecturally
            // fixed state and must never clobber state the ABI preserves.
            // Only a user-space call consumes the activation stack pointer.
            assert_eq!(
                stack_pointer
                    .units
                    .iter()
                    .all(|unit| row.implicit_uses.contains(unit)),
                !supervisor,
                "{key:?} only a user-space call consumes the stack pointer"
            );
            assert!(
                row.implicit_uses
                    .iter()
                    .chain(&row.implicit_defs)
                    .all(|unit| !convention.callee_saved.contains(unit)),
                "{key:?} implicit state must not include callee-saved units"
            );
            assert!(!row.clobbers.is_empty(), "{key:?}");
            assert!(
                row.clobbers
                    .iter()
                    .all(|unit| convention.caller_saved.contains(unit)),
                "{key:?} clobbers must stay inside the caller-saved set"
            );
            for result in results {
                let view = model
                    .views
                    .iter()
                    .find(|view| Some(view.id) == result.fixed_view)
                    .unwrap();
                assert!(
                    view.write_units
                        .iter()
                        .all(|unit| !row.clobbers.contains(unit)),
                    "{key:?} result {} must not also be clobbered",
                    view.name
                );
            }

            for alternative in &declaration.alternatives {
                assert_eq!(alternative.key.family, (*semantic).into(), "{key:?}");
                let encoded = &alternative.encoded;
                assert_eq!(
                    encoded.external_operand_reads,
                    arguments
                        .iter()
                        .map(|operand| operand.operand)
                        .collect::<Vec<_>>(),
                    "{key:?}"
                );
                assert_eq!(
                    encoded.external_operand_writes,
                    results
                        .iter()
                        .map(|operand| operand.operand)
                        .collect::<Vec<_>>(),
                    "{key:?}"
                );
                assert_eq!(encoded.implicit_unit_uses, row.implicit_uses, "{key:?}");
                assert_eq!(encoded.implicit_unit_defs, row.implicit_defs, "{key:?}");
                assert_eq!(encoded.implicit_unit_clobbers, row.clobbers, "{key:?}");
                assert_eq!(
                    encoded.trap,
                    MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    "{key:?}"
                );
                assert_eq!(
                    encoded.control,
                    if supervisor {
                        MachineEncodedControlEffect::SupervisorCallV1
                    } else {
                        MachineEncodedControlEffect::DirectRelativeCallV1
                    },
                    "{key:?}"
                );
                match (case.target.architecture, supervisor) {
                    (Architecture::X86_64, false) => {
                        assert_eq!(
                            encoded.memory,
                            MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
                                stack_pointer: stack_pointer.id,
                                byte_count: 8,
                            },
                            "{key:?}"
                        );
                        assert_eq!(
                            encoded.stack,
                            MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
                                stack_pointer: stack_pointer.id,
                                return_address_byte_count: 8,
                            },
                            "{key:?}"
                        );
                    }
                    (Architecture::Aarch64, _) | (Architecture::X86_64, true) => {
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
                    }
                }
            }
        }
    }
}

/// Every selected call rule replays its encoded clobber row: dropping one
/// caller-saved unit from the encoded effects must reject for every rule on
/// every declared target/ABI pair.
#[test]
fn every_selected_call_rule_rejects_encoded_clobber_loss_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let catalog = produced_effects(case, environment.constraints());
        for (key, semantic) in selected_call_rules(&environment) {
            let mut corrupted = catalog.clone();
            let encoded = &mut declaration_mut(&mut corrupted, key).alternatives[0].encoded;
            assert!(
                !encoded.implicit_unit_clobbers.is_empty(),
                "{key:?} declares no caller-saved clobbers"
            );
            encoded.implicit_unit_clobbers.remove(0);
            assert_eq!(
                selected_instructions::validate_machine_effect_catalog(
                    environment.constraints(),
                    corrupted,
                ),
                Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                    semantic
                )),
                "{key:?} lost encoded clobber must reject"
            );
        }
    }
}
