use super::*;

pub(super) fn assert_source(
    source: &str,
    boundary: bool,
    expected: &[Vec<Vec<StructuralPathSegment>>],
) {
    let checked = checked(source);
    let artifact = terminal_production::produce_terminal_artifact(&checked, "Root::enter")
        .unwrap_or_else(|error| panic!("{source}\n{error:?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").unwrap();
    let module = &lowered.semantic_module;
    assert_eq!(decode_module(artifact.semantic_bytes()).unwrap(), *module);
    let semantic = encode_module(module).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    let proof = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    assert_eq!(decode_proof_bundle(&proof).unwrap(), lowered.proof_bundle);
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    let certificate =
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let cleanups = caller
        .blocks
        .iter()
        .filter_map(|block| {
            let Terminator::Jump {
                edge,
                target,
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } = &block.terminator
            else {
                return None;
            };
            let result = block
                .operations
                .iter()
                .find_map(|operation| operation.result.structural())
                .unwrap();
            assert!(trivial_affine_discards.is_empty());
            assert!(
                residual_affine_discards
                    .iter()
                    .all(|discard| discard.place == result.place)
            );
            let successor = caller
                .blocks
                .iter()
                .find(|block| block.id == *target)
                .unwrap();
            let next_site = successor.operations.first().map_or(
                FuelChargeSite::Edge(successor.terminator.edge()),
                |operation| FuelChargeSite::Operation(operation.id),
            );
            Some((*edge, result.place, residual_affine_discards, next_site))
        })
        .collect::<Vec<_>>();
    assert_eq!(cleanups.len(), expected.len());
    for ((_, _, residuals, _), expected) in cleanups.iter().zip(expected) {
        assert_eq!(
            residuals
                .iter()
                .map(|discard| discard.path.clone())
                .collect::<Vec<_>>(),
            *expected
        );
    }
    let arguments = caller
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| TerminalStructuralValue {
            opaque_identity: 123 + ordinal as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut reference = None;
    for incremental in [false, true] {
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &arguments,
        )
        .unwrap();
        let mut factory = Factory::default();
        let mut meter = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut before = vec![false; cleanups.len()];
        let mut after = before.clone();
        let mut complete = false;
        for _ in 0..256 {
            match execution
                .resume_with_effect_handler(&mut meter, &mut factory)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                    assert!(incremental);
                    for (ordinal, (edge, place, residuals, next_site)) in
                        cleanups.iter().enumerate()
                    {
                        if exhaustion.site == FuelChargeSite::Edge(*edge) {
                            assert_eq!(
                                execution
                                    .live_affine_frontier()
                                    .filter(|entry| entry.place == *place)
                                    .cloned()
                                    .collect::<std::collections::BTreeSet<_>>(),
                                residuals.iter().cloned().collect()
                            );
                            assert!(meter.usage().at(exhaustion.site).is_none());
                            before[ordinal] = true;
                        }
                        if exhaustion.site == *next_site {
                            assert!(
                                !execution
                                    .live_affine_frontier()
                                    .any(|entry| entry.place == *place)
                            );
                            after[ordinal] = true;
                        }
                    }
                    meter.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(value) => {
                    assert_eq!(value, TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                status => panic!("unexpected {status:?}"),
            }
        }
        assert!(complete);
        assert_eq!(before, vec![incremental; cleanups.len()]);
        assert_eq!(after, before);
        assert_eq!(factory.calls, if boundary { cleanups.len() } else { 0 });
        if source.contains("Trace::tick") {
            assert_eq!(
                factory.ticks,
                vec![terminal_interpreter::TerminalScalarValue::Integer {
                    scalar_type: semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        16
                    )
                    .unwrap(),
                    value: semantic_vocabulary::IntegerValue::Unsigned(17),
                }]
            );
        }
        assert!(execution.live_affine_frontier().next().is_none());
        assert_eq!(meter.usage().total_units(), certificate.ceiling_units());
        let observed = (execution.effects().to_vec(), meter.usage().clone());
        if let Some(reference) = &reference {
            assert_eq!(&observed, reference);
        } else {
            reference = Some(observed);
        }
    }
}

#[test]
fn projected_continuations_keep_nested_residuals_and_prior_scalar_values() {
    for boundary in [false, true] {
        for (projection, residuals) in [
            (
                "grid[0][1]",
                vec![
                    path(&["tail"]),
                    path(&["grid", "1"]),
                    path(&["grid", "0", "2"]),
                    path(&["grid", "0", "0"]),
                    path(&["left"]),
                ],
            ),
            (
                "grid[1]",
                vec![path(&["tail"]), path(&["grid", "0"]), path(&["left"])],
            ),
        ] {
            let consumer = if projection == "grid[1]" {
                "take_row"
            } else {
                "take"
            };
            let source = anonymous_source(boundary, true, &format!("let prefix: u16 = 17; Sink::{consumer}(result.{projection}); Trace::tick(prefix);"))
                .replace("Root::enter(value: Pair) {", "Root::enter(value: Pair) reaches Trace {")
                .replace("Root::enter() reaches Factory", "Root::enter() reaches Factory + Trace");
            let source = format!(
                "{source} boundary trait Trace {{ machine tick(value: u16) reaches Trace; }}"
            );
            assert_source(&source, boundary, &[residuals]);
        }
    }
}

#[test]
fn projected_continuations_keep_distinct_successive_owners_and_empty_remainders() {
    for boundary in [false, true] {
        let source = anonymous_source(
            boundary,
            false,
            "Sink::take(result.right); Sink::take(result.left);",
        );
        let source = if boundary {
            source
        } else {
            source
                .replace(
                    "Root::enter(value: Pair)",
                    "Root::enter(value: Pair, other: Pair)",
                )
                .replace("Root::forward(value).left", "Root::forward(other).left")
        };
        assert_source(
            &source,
            boundary,
            &[vec![path(&["left"])], vec![path(&["right"])]],
        );
        let source = anonymous_source(boundary, false, "Sink::take(result.right); Sink::done();")
            .replace("left: Token; right: Token;", "right: Token;");
        assert_source(
            &format!("{source} machine Sink::done() {{}}"),
            boundary,
            &[Vec::new()],
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
            let original = checked(&format!("{source} machine Sink::done() {{}}"));
            let _artifact =
                terminal_production::produce_terminal_artifact(&original, "Root::enter").unwrap();
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
                    terminal_production::produce_terminal_artifact(&changed, "Root::enter")
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
                        terminal_production::produce_terminal_artifact(&changed, "Root::enter")
                            .is_err(),
                        "boundary={boundary}, empty={empty}, event={:?}, permission mutation={mutation}",
                        event.kind
                    );
                }
            }
        }
    }
}
