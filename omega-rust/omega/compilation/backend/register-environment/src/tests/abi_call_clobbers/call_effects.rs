use super::{
    Architecture, BTreeSet, EffectRejection, RegisterConstraintKey, assert_target_semantic_error,
    baseline_target_register_environment, convention_for, declaration_mut, produced_effects,
    row_mut, scalar_abi_cases, target_constraint_catalog, target_physical_register_model,
    validate_effects, validate_physical_register_model, validate_target_register_environment,
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

        let stack_pointer = model
            .view_named(match case.target.architecture {
                Architecture::X86_64 => "rsp",
                Architecture::Aarch64 => "sp",
            })
            .expect("call matrix names a stack-pointer view");

        for (key, semantic) in &call_rules {
            let row = environment
                .constraint(*key)
                .unwrap_or_else(|| panic!("environment missing selected call row {key:?}"));
            let declaration = catalog
                .catalog()
                .declarations
                .iter()
                .find(|entry| entry.constraint == *key)
                .unwrap_or_else(|| panic!("effect catalog missing call declaration {key:?}"));
            assert_eq!(declaration.semantic, *semantic, "{key:?}");
            assert_eq!(declaration.barrier, MachineBarrier::Call, "{key:?}");
            // The call contract is bound to the declared preservation
            // convention, not to the effect producer's own constant.
            // Normalized foreign rows cross an external boundary: their
            // normal-return contract is the external variant at the same
            // convention alignment.
            let expected_call = if *semantic == MachineSemanticKind::NormalizedForeignCall {
                MachineCallEffect::DirectExternalNormalReturnV1 {
                    pre_call_stack_alignment: convention.stack_alignment,
                }
            } else {
                MachineCallEffect::DirectInternalNormalReturnV1 {
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
                    .and_then(|id| model.views.iter().find(|view| view.id == id))
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
            // fixed state, must consume the stack pointer, and must never
            // clobber state the ABI preserves.
            assert!(
                stack_pointer
                    .units
                    .iter()
                    .all(|unit| row.implicit_uses.contains(unit)),
                "{key:?} must implicitly use the stack pointer"
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
                    MachineEncodedControlEffect::DirectRelativeCallV1,
                    "{key:?}"
                );
                match case.target.architecture {
                    Architecture::X86_64 => {
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
                    Architecture::Aarch64 => {
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

/// The remaining call-contract corruptions are checked once per selected call
/// family on every declared target/ABI pair: unit, mixed unit, scalar, and
/// aggregate rows each prove the validator sees their constraint row.
#[test]
fn every_call_family_rejects_call_contract_corruption_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let convention = convention_for(case, environment.physical());
        let keys = environment.selected_keys();
        let catalog = produced_effects(case, environment.constraints());
        let raw = target_physical_register_model(case.target);
        let physical = validate_physical_register_model(raw.clone()).unwrap();
        let constraints = target_constraint_catalog(case.target, &physical);
        let families: [(&[RegisterConstraintKey], MachineSemanticKind); 4] = [
            (&keys.call_unit, MachineSemanticKind::CallUnit),
            (&keys.call_unit_mixed, MachineSemanticKind::CallUnit),
            (&keys.call_scalar, MachineSemanticKind::CallScalar),
            (&keys.call_aggregate, MachineSemanticKind::CallAggregate),
        ];
        for (family_keys, semantic) in families {
            let Some(key) = family_keys.first().copied() else {
                continue;
            };

            // A doubled alignment is still a power of two, so structural
            // admission accepts it; only canonical replay may reject it.
            let mut corrupted = catalog.clone();
            declaration_mut(&mut corrupted, key).call =
                MachineCallEffect::DirectInternalNormalReturnV1 {
                    pre_call_stack_alignment: convention.stack_alignment * 2,
                };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::SemanticMismatch),
                "{key:?} call alignment drift must reject"
            );

            let mut corrupted = catalog.clone();
            declaration_mut(&mut corrupted, key).barrier = MachineBarrier::None;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::BarrierMismatch(semantic)
                )),
                "{key:?} lost call barrier must reject"
            );

            // The call's declared surfaces claim no memory footprint and no
            // architectural fault: the return-address lifecycle lives in the
            // encoded stack surface, and the declaration cannot launder
            // either one through it.
            let mut corrupted = catalog.clone();
            declaration_mut(&mut corrupted, key).memory = MachineMemoryEffect::WritePointerV1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} claimed declared footprint must reject"
            );
            let mut corrupted = catalog.clone();
            declaration_mut(&mut corrupted, key).trap =
                MachineTrapBehavior::MayArchitecturalFaultV1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} claimed declared fault must reject"
            );

            let mut corrupted = catalog.clone();
            declaration_mut(&mut corrupted, key).alternatives[0]
                .encoded
                .control = MachineEncodedControlEffect::FallThroughV1;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} call encoded as fall-through must reject"
            );

            let mut corrupted = catalog.clone();
            let encoded = &mut declaration_mut(&mut corrupted, key).alternatives[0].encoded;
            encoded.stack = match case.target.architecture {
                Architecture::X86_64 => MachineEncodedStackEffect::UnchangedV1,
                Architecture::Aarch64 => MachineEncodedStackEffect::PopBytesV1 {
                    stack_pointer: model.view_named("sp").unwrap().id,
                    byte_count: 8,
                },
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} stack-effect drift must reject"
            );

            // The constraint row itself is exercised: losing one caller-saved
            // clobber under the selected key must fail the environment join.
            let mut corrupted = constraints.clone();
            row_mut(&mut corrupted, key).clobbers.remove(0);
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("every call family must retain its exact ABI clobbers");
            assert_target_semantic_error(case.target, key, error);

            // An ABI operand pinned to a callee-saved view must reject wherever
            // the target declares a same-class preserved view to substitute.
            let row = environment.constraint(key).unwrap();
            if let Some((position, preserved)) =
                row.operands
                    .iter()
                    .enumerate()
                    .find_map(|(position, operand)| {
                        model
                            .views
                            .iter()
                            .find(|view| {
                                view.class == operand.class
                                    && view
                                        .units
                                        .iter()
                                        .all(|unit| convention.callee_saved.contains(unit))
                            })
                            .map(|view| (position, view.id))
                    })
            {
                let mut corrupted = constraints.clone();
                row_mut(&mut corrupted, key).operands[position].fixed_view = Some(preserved);
                let error =
                    validate_target_register_environment(case.target, raw.clone(), corrupted)
                        .expect_err("a callee-saved call operand must reject");
                assert_target_semantic_error(case.target, key, error);
            }
        }
    }
}
