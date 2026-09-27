use super::{
    AdmissionProfile, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, OperationResult, Terminator, anonymous_source, path,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

#[test]
fn scalar_result_continuations_reject_binding_and_cleanup_drift() {
    let source = anonymous_source(
        false,
        false,
        "Sink::take(result.right); Sink::number(first);",
    )
    .replace(
        "Root::enter(value: Pair)",
        "Root::enter(first: u16, value: Pair)",
    );
    let source = format!("{source} machine Sink::number(value: u16) {{}}");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(&source),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("valid scalar result continuation before mutations");
    let original = &lowered.semantic_module;
    for mutation in 0..6 {
        let mut changed = original.clone();
        let caller = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == original.entry)
            .unwrap();
        let block = caller
            .blocks
            .iter_mut()
            .find(|block| matches!(block.terminator, Terminator::Jump { .. }))
            .unwrap();
        let Terminator::Jump {
            target,
            arguments,
            residual_affine_discards,
            ..
        } = &mut block.terminator
        else {
            unreachable!()
        };
        let target = *target;
        assert_eq!(arguments.len(), 1);
        assert_eq!(residual_affine_discards.len(), 1);
        match mutation {
            0 => arguments.clear(),
            1 => arguments[0] = semantic_vocabulary::ValueId::new(99999).unwrap(),
            2 => residual_affine_discards.clear(),
            3 => residual_affine_discards[0].path = path(&["right"]),
            4 => {
                let result = block
                    .operations
                    .iter_mut()
                    .find_map(|operation| match &mut operation.result {
                        OperationResult::Structural(result) => Some(result),
                        _ => None,
                    })
                    .unwrap();
                result.place = caller.structural_parameters[0].place;
            }
            5 => {
                caller
                    .blocks
                    .iter_mut()
                    .find(|block| block.id == target)
                    .unwrap()
                    .parameters[0]
                    .scalar_type = semantic_vocabulary::ScalarType::Boolean;
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &lowered.proof_bundle,
                &AdmissionProfile::default()
            )
            .is_err(),
            "scalar continuation mutation {mutation} must reject"
        );
    }
}

#[test]
fn projected_continuation_plans_reject_cleanup_and_permission_drift() {
    for boundary in [false, true] {
        for empty in [false, true] {
            let source = if empty {
                anonymous_source(boundary, false, "Sink::take(result.right); Sink::done();")
                    .replace("left: Token; right: Token;", "right: Token;")
            } else {
                anonymous_source(
                    boundary,
                    true,
                    "Sink::take(result.grid[1][1]); Sink::done();",
                )
            };
            let original =
                crate::front_end::checked_program(&format!("{source} machine Sink::done() {{}}"));
            let _artifact = terminal_production::TerminalProductionRequest::new(
                &original,
                TerminalMachineSelection::Name("Root::enter"),
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .unwrap()
            .into_artifact();
            let root = original
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Root::enter")
                .unwrap()
                .symbol;
            let plan_index = original
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter()
                .position(|plan| plan.machine == root)
                .unwrap();
            let operations =
                &original.facts.flow.terminal_unit_effects.machines[plan_index].operations;
            let cleanup_index = operations
                .iter()
                .position(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                    )
                })
                .unwrap();
            for mutation in 0..9 {
                if empty && (4..=6).contains(&mutation) {
                    continue;
                }
                let mut changed = original.clone();
                let operations =
                    &mut changed.facts.flow.terminal_unit_effects.machines[plan_index].operations;
                match mutation {
                    0 => {
                        operations.remove(cleanup_index);
                    }
                    1 => operations.swap(cleanup_index, cleanup_index + 1),
                    2 => operations.insert(cleanup_index + 1, operations[cleanup_index].clone()),
                    3 => {
                        let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                            coordinate,
                            ..
                        } = &mut operations[cleanup_index]
                        else {
                            unreachable!()
                        };
                        coordinate.statement_index += 1;
                    }
                    4..=6 => {
                        let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                            affine_discards,
                            ..
                        } = &mut operations[cleanup_index]
                        else {
                            unreachable!()
                        };
                        match mutation {
                            4 => affine_discards.reverse(),
                            5 => {
                                affine_discards[0].source =
                                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                        binding_ordinal: 99,
                                    }
                            }
                            6 => {
                                affine_discards[0].path = vec![
                                    CheckedUnitStructuralPathSegment::Field("grid".into()),
                                    CheckedUnitStructuralPathSegment::FixedIndex(1),
                                    CheckedUnitStructuralPathSegment::FixedIndex(1),
                                ]
                            }
                            _ => unreachable!(),
                        }
                    }
                    7 => {
                        let (CheckedUnitEffectOperationPlan::StructuralCall {
                            discard_result_on_return,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                            discard_result_on_return,
                            ..
                        }) = &mut operations[0]
                        else {
                            unreachable!()
                        };
                        *discard_result_on_return = true;
                    }
                    8 => {
                        let mut repeated = operations[cleanup_index - 1].clone();
                        let CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. } =
                            &mut repeated
                        else {
                            unreachable!()
                        };
                        coordinate.statement_index += 1;
                        operations[cleanup_index + 1] = repeated;
                    }
                    _ => unreachable!(),
                }
                assert!(
                    terminal_production::TerminalProductionRequest::new(
                        &changed,
                        TerminalMachineSelection::Name("Root::enter")
                    )
                    .produce(TerminalProductionCustody::artifact_only(
                        &mut TerminalProductionTimings::default()
                    ))
                    .is_err(),
                    "boundary={boundary}, empty={empty}, cleanup mutation={mutation}"
                );
            }
            for (handle, event) in
                original
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .iter()
                    .filter(|(_, event)| {
                        event.machine_symbol == root
                            && matches!(event.root, facts::PlaceRoot::Expression(_))
                    })
            {
                for mutation in 0..4 {
                    let mut changed = original.clone();
                    let altered = changed.facts.flow.ownership.permissions.get_mut(handle);
                    match mutation {
                        0 => altered.provenance = language_semantics::PermissionProvenance::Unknown,
                        1 => altered.source = language_semantics::PermissionEventSource::StateExit,
                        2 => altered.root = facts::PlaceRoot::Unknown,
                        3 => altered.obligation_live = true,
                        _ => unreachable!(),
                    }
                    assert!(
                        terminal_production::TerminalProductionRequest::new(
                            &changed,
                            TerminalMachineSelection::Name("Root::enter")
                        )
                        .produce(TerminalProductionCustody::artifact_only(
                            &mut TerminalProductionTimings::default()
                        ))
                        .is_err(),
                        "boundary={boundary}, empty={empty}, event={:?}, permission mutation={mutation}",
                        event.kind
                    );
                }
            }
        }
    }
}
