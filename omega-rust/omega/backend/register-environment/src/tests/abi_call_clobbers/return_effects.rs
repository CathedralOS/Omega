use super::{
    Architecture, BTreeSet, EffectRejection, RegisterConstraintKey, ScalarAbiCase,
    assert_target_semantic_error, baseline_target_register_environment, convention_for,
    declaration_mut, produced_effects, row_mut, scalar_abi_cases, target_constraint_catalog,
    target_physical_register_model, units_for_names, validate_effects,
    validate_physical_register_model, validate_target_register_environment, validated_effects,
};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    MachineAlternativeApplicability, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalogValidationError, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};

/// Every selected return rule key with the semantic family the effect
/// catalog must declare for it. The sweep is keyed off the validated
/// environment's selection, never off the effect producer's own roster.
fn selected_return_rules(
    environment: &crate::ValidatedTargetRegisterEnvironment,
) -> Vec<(RegisterConstraintKey, MachineSemanticKind)> {
    let keys = environment.selected_keys();
    std::iter::once((keys.return_i64, MachineSemanticKind::ReturnScalar))
        .chain(
            keys.return_float
                .iter()
                .map(|key| (*key, MachineSemanticKind::ReturnScalar)),
        )
        .chain(std::iter::once((
            keys.return_unit,
            MachineSemanticKind::ReturnUnit,
        )))
        .chain(
            keys.return_aggregate
                .iter()
                .map(|key| (*key, MachineSemanticKind::ReturnAggregate)),
        )
        .collect()
}

/// The declared ABI result views one selected return rule consumes, in
/// operand order. Scalar integer and float returns read their single ABI
/// home, a Unit return reads none, and each aggregate ordinal extends the
/// ordered integer-result prefix by one view.
fn expected_result_views(
    case: ScalarAbiCase,
    environment: &crate::ValidatedTargetRegisterEnvironment,
    key: RegisterConstraintKey,
) -> Vec<&'static str> {
    let keys = environment.selected_keys();
    if key == keys.return_i64 {
        vec![case.integer_results[0]]
    } else if keys.return_float.contains(&key) {
        vec![case.float_result]
    } else if key == keys.return_unit {
        Vec::new()
    } else if let Some(ordinal) = keys.return_aggregate.iter().position(|entry| *entry == key) {
        case.integer_results[..=ordinal].to_vec()
    } else {
        panic!("{key:?} is not a selected return rule")
    }
}

/// Every selected return rule on every declared target/ABI pair is bound to
/// the preservation convention and return mechanism that pair declares.
#[test]
fn every_selected_return_rule_binds_the_declared_abi_return_contract() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let convention = convention_for(case, environment.physical());
        let catalog = validated_effects(case, environment.constraints());
        let keys = environment.selected_keys();
        let return_rules = selected_return_rules(&environment);
        assert!(
            !return_rules.is_empty(),
            "{} selects no return rules",
            case.convention
        );

        // Return authority is declared for exactly the selected return rules:
        // no selected rule may lack a return declaration and no other
        // declaration may claim a return semantic.
        let declared_returns = catalog
            .catalog()
            .declarations
            .iter()
            .filter(|declaration| {
                matches!(
                    declaration.semantic,
                    MachineSemanticKind::ReturnScalar
                        | MachineSemanticKind::ReturnUnit
                        | MachineSemanticKind::ReturnAggregate
                )
            })
            .map(|declaration| (declaration.constraint, declaration.semantic))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            declared_returns,
            return_rules.iter().copied().collect::<BTreeSet<_>>(),
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

        for (key, semantic) in &return_rules {
            let row = environment
                .constraint(*key)
                .unwrap_or_else(|| panic!("environment missing selected return row {key:?}"));
            let declaration = catalog
                .catalog()
                .declarations
                .iter()
                .find(|entry| entry.constraint == *key)
                .unwrap_or_else(|| panic!("effect catalog missing return declaration {key:?}"));
            assert_eq!(declaration.semantic, *semantic, "{key:?}");
            assert_eq!(declaration.barrier, MachineBarrier::ControlFlow, "{key:?}");
            assert_eq!(declaration.call, MachineCallEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.cleanup, MachineCleanupEffect::NoneV1, "{key:?}");
            assert_eq!(declaration.trap, MachineTrapBehavior::NeverV1, "{key:?}");
            assert_eq!(declaration.memory, MachineMemoryEffect::NoneV1, "{key:?}");

            // ABI operand structure: dense operand numbers of Use operands
            // pinned to the declared result views; a Unit return has none.
            let expected_views = expected_result_views(case, &environment, *key);
            assert_eq!(row.operands.len(), expected_views.len(), "{key:?}");
            for (position, (operand, name)) in
                row.operands.iter().zip(expected_views.iter()).enumerate()
            {
                assert_eq!(operand.operand as usize, position, "{key:?}");
                assert_eq!(operand.access, RegisterOperandAccess::Use, "{key:?}");
                assert!(!operand.early_clobber, "{key:?}");
                assert!(operand.tied_to.is_none(), "{key:?}");
                let expected = model
                    .view_named(name)
                    .unwrap_or_else(|| panic!("ABI matrix names missing view `{name}`"));
                assert_eq!(operand.fixed_view, Some(expected.id), "{key:?}");
                assert_eq!(operand.class, expected.class, "{key:?}");
                assert!(
                    expected
                        .units
                        .iter()
                        .all(|unit| convention.caller_saved.contains(unit)),
                    "{key:?} result {name} must travel through a caller-saved view under {}",
                    convention.name
                );
            }

            // A return may only implicitly touch volatile or architecturally
            // fixed control state, and it never clobbers anything itself.
            assert_eq!(
                row.implicit_uses,
                units_for_names(model, case.return_uses),
                "{key:?}"
            );
            assert_eq!(
                row.implicit_defs,
                units_for_names(model, case.return_defs),
                "{key:?}"
            );
            assert!(
                row.implicit_uses
                    .iter()
                    .chain(&row.implicit_defs)
                    .all(|unit| !convention.callee_saved.contains(unit)),
                "{key:?} implicit state must not include callee-saved units"
            );
            assert!(row.clobbers.is_empty(), "{key:?}");

            // One canonical alternative replays the exact ABI return
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
            let encoded = &alternative.encoded;
            assert!(encoded.external_operand_reads.is_empty(), "{key:?}");
            assert!(encoded.external_operand_writes.is_empty(), "{key:?}");
            assert!(encoded.implicit_unit_clobbers.is_empty(), "{key:?}");
            // Encoded implicit state refines, and stays inside, the
            // constraint row's conservative custody.
            assert!(
                encoded
                    .implicit_unit_uses
                    .iter()
                    .all(|unit| row.implicit_uses.contains(unit)),
                "{key:?} encoded uses must stay inside the constraint row"
            );
            assert!(
                encoded
                    .implicit_unit_defs
                    .iter()
                    .all(|unit| row.implicit_defs.contains(unit)),
                "{key:?} encoded defs must stay inside the constraint row"
            );
            assert_eq!(
                encoded.trap,
                MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                "{key:?}"
            );
            match case.target.architecture {
                Architecture::X86_64 => {
                    let stack_pointer = model.view_named("rsp").unwrap();
                    assert_eq!(encoded.implicit_unit_uses, stack_pointer.units, "{key:?}");
                    assert_eq!(
                        encoded.implicit_unit_defs,
                        units_for_names(model, &["rsp", "rip"]),
                        "{key:?}"
                    );
                    assert_eq!(
                        encoded.memory,
                        MachineEncodedMemoryEffect::ReadActivationStackV1 {
                            stack_pointer: stack_pointer.id,
                            byte_count: 8,
                        },
                        "{key:?}"
                    );
                    assert_eq!(
                        encoded.stack,
                        MachineEncodedStackEffect::PopBytesV1 {
                            stack_pointer: stack_pointer.id,
                            byte_count: 8,
                        },
                        "{key:?}"
                    );
                    assert_eq!(
                        encoded.control,
                        MachineEncodedControlEffect::ReturnFromActivationStackV1,
                        "{key:?}"
                    );
                    assert_eq!(
                        alternative.size,
                        MachineSizeKnowledge::ExactBytes(1),
                        "{key:?}"
                    );
                }
                Architecture::Aarch64 => {
                    let link = model.view_named("x30").unwrap();
                    let program_counter = model.view_named("pc").unwrap();
                    // The constraint row retains frame custody of `sp`; the
                    // encoded `ret` itself reads only the link register.
                    assert_eq!(encoded.implicit_unit_uses, link.units, "{key:?}");
                    assert_eq!(encoded.implicit_unit_defs, program_counter.units, "{key:?}");
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
                        encoded.control,
                        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target: link.id },
                        "{key:?}"
                    );
                    assert_eq!(
                        alternative.size,
                        MachineSizeKnowledge::ExactBytes(4),
                        "{key:?}"
                    );
                }
            }
        }
    }
}

/// Every selected return rule replays its encoded control form: a return
/// encoded as fall-through, stripped of its return-address read, or carrying
/// a forged clobber must reject for every rule on every declared target/ABI
/// pair.
#[test]
fn every_selected_return_rule_rejects_encoded_return_forgery_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let convention = convention_for(case, environment.physical());
        let catalog = produced_effects(case, environment.constraints());
        for (key, semantic) in selected_return_rules(&environment) {
            // A return encoded as ordinary fall-through fails the encoded
            // control/barrier join.
            let mut corrupted = catalog.clone();
            declaration_mut(&mut corrupted, key).alternatives[0]
                .encoded
                .control = MachineEncodedControlEffect::FallThroughV1;
            assert_eq!(
                selected_instructions::validate_machine_effect_catalog(
                    environment.constraints(),
                    corrupted,
                ),
                Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                    semantic
                )),
                "{key:?} fall-through return must reject"
            );

            // Dropping the encoded return-address read understates the
            // row's contracted uses: an indirect-register return may narrow
            // to a non-empty subset, and a use list that reads nothing has
            // dropped the read its control effect still performs.
            let mut corrupted = catalog.clone();
            let encoded = &mut declaration_mut(&mut corrupted, key).alternatives[0].encoded;
            assert!(
                !encoded.implicit_unit_uses.is_empty(),
                "{key:?} declares no return-address use"
            );
            encoded.implicit_unit_uses.remove(0);
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic)
                )),
                "{key:?} lost return-address use must reject"
            );

            // A forged clobber outside the return row's empty clobber set
            // fails structural admission before canonical replay runs.
            let mut corrupted = catalog.clone();
            let row = environment.constraint(key).unwrap();
            assert!(row.clobbers.is_empty(), "{key:?}");
            let preserved = convention.callee_saved[0];
            declaration_mut(&mut corrupted, key).alternatives[0]
                .encoded
                .implicit_unit_clobbers
                .push(preserved);
            assert_eq!(
                selected_instructions::validate_machine_effect_catalog(
                    environment.constraints(),
                    corrupted,
                ),
                Err(MachineEffectCatalogValidationError::InvalidEncodedEffects(
                    semantic
                )),
                "{key:?} forged callee-saved clobber must reject"
            );
        }
    }
}

/// The remaining return-contract corruptions are checked once per selected
/// return family on every declared target/ABI pair: integer, float, unit,
/// and aggregate rows each prove the validators see their constraint row.
#[test]
fn every_return_family_rejects_return_contract_corruption_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let model = environment.physical().model();
        let convention = convention_for(case, environment.physical());
        let keys = environment.selected_keys();
        let catalog = produced_effects(case, environment.constraints());
        let raw = target_physical_register_model(case.target);
        let physical = validate_physical_register_model(raw.clone()).unwrap();
        let constraints = target_constraint_catalog(case.target, &physical);
        let families: [(Option<RegisterConstraintKey>, MachineSemanticKind); 4] = [
            (Some(keys.return_i64), MachineSemanticKind::ReturnScalar),
            (
                keys.return_float.first().copied(),
                MachineSemanticKind::ReturnScalar,
            ),
            (Some(keys.return_unit), MachineSemanticKind::ReturnUnit),
            (
                keys.return_aggregate.first().copied(),
                MachineSemanticKind::ReturnAggregate,
            ),
        ];
        for (key, semantic) in families {
            let Some(key) = key else { continue };
            let row = environment.constraint(key).unwrap();

            // A return without its control-flow barrier must reject.
            let mut corrupted = catalog.clone();
            declaration_mut(&mut corrupted, key).barrier = MachineBarrier::None;
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(EffectRejection::Structural(
                    MachineEffectCatalogValidationError::BarrierMismatch(semantic)
                )),
                "{key:?} lost return barrier must reject"
            );

            // Encoded return-mechanism drift is rejected by the layer that
            // owns it: the activation-stack pop is structural on x86-64,
            // while a same-barrier forged link-register target needs
            // canonical replay on AArch64.
            let mut corrupted = catalog.clone();
            let encoded = &mut declaration_mut(&mut corrupted, key).alternatives[0].encoded;
            let expected_rejection = match case.target.architecture {
                Architecture::X86_64 => {
                    encoded.stack = MachineEncodedStackEffect::UnchangedV1;
                    EffectRejection::Structural(
                        MachineEffectCatalogValidationError::InvalidEncodedEffects(semantic),
                    )
                }
                Architecture::Aarch64 => {
                    encoded.control = MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                        target: model.view_named("x0").unwrap().id,
                    };
                    EffectRejection::SemanticMismatch
                }
            };
            assert_eq!(
                validate_effects(case, environment.constraints(), corrupted),
                Err(expected_rejection),
                "{key:?} encoded return-mechanism drift must reject"
            );

            // The constraint row itself is exercised: losing one implicit
            // use under the selected key must fail the environment join.
            let mut corrupted = constraints.clone();
            row_mut(&mut corrupted, key).implicit_uses.remove(0);
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("every return family must retain its implicit ABI uses");
            assert_target_semantic_error(case.target, key, error);

            // A spurious clobber on a return row must fail the join.
            let mut corrupted = constraints.clone();
            let clobbers = &mut row_mut(&mut corrupted, key).clobbers;
            clobbers.push(convention.caller_saved[0]);
            clobbers.sort_unstable();
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("a return row must retain its empty clobber set");
            assert_target_semantic_error(case.target, key, error);

            // Repinning a returned operand to another caller-saved view or to
            // a callee-saved view of the same class must fail the join.
            if let Some(operand) = row.operands.first() {
                let misplaced = model.views.iter().find(|view| {
                    view.class == operand.class
                        && Some(view.id) != operand.fixed_view
                        && view
                            .units
                            .iter()
                            .all(|unit| convention.caller_saved.contains(unit))
                });
                let preserved = model.views.iter().find(|view| {
                    view.class == operand.class
                        && view
                            .units
                            .iter()
                            .all(|unit| convention.callee_saved.contains(unit))
                });
                for (role, view) in [
                    ("caller-saved non-result", misplaced),
                    ("callee-saved", preserved),
                ] {
                    let Some(view) = view else { continue };
                    let mut corrupted = constraints.clone();
                    row_mut(&mut corrupted, key).operands[0].fixed_view = Some(view.id);
                    let error =
                        validate_target_register_environment(case.target, raw.clone(), corrupted)
                            .err()
                            .unwrap_or_else(|| {
                                panic!("{key:?} return operand repinned to {role} view must reject")
                            });
                    assert_target_semantic_error(case.target, key, error);
                }
            }
        }

        // Where the ABI selects a two-register aggregate return, swapping the
        // ordered result views must fail the join.
        if keys.return_aggregate.len() > 1 {
            let key = keys.return_aggregate[1];
            let mut corrupted = constraints.clone();
            let row = row_mut(&mut corrupted, key);
            row.operands[0].fixed_view = row.operands[1].fixed_view;
            row.operands[1].fixed_view =
                Some(model.view_named(case.integer_results[0]).unwrap().id);
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("swapped aggregate result views must reject");
            assert_target_semantic_error(case.target, key, error);
        }
    }
}
