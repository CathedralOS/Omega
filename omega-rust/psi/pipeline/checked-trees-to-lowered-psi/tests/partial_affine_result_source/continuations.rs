use super::*;

pub(super) fn assert_source(
    source: &str,
    boundary: bool,
    expected: &[Vec<Vec<StructuralPathSegment>>],
) {
    let ticks = if source.contains("Trace::tick") {
        vec![integer(17)]
    } else {
        Vec::new()
    };
    assert_source_with_scalars(source, boundary, expected, &[], &ticks);
}

fn integer(value: u128) -> terminal_interpreter::TerminalScalarValue {
    terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            16,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Unsigned(value),
    }
}

fn assert_source_with_scalars(
    source: &str,
    boundary: bool,
    expected: &[Vec<Vec<StructuralPathSegment>>],
    scalar_arguments: &[terminal_interpreter::TerminalScalarValue],
    expected_ticks: &[terminal_interpreter::TerminalScalarValue],
) {
    let checked = checked(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
        .produce_artifact()
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
            scalar_arguments,
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
        assert_eq!(factory.ticks, expected_ticks);
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
fn projected_result_continuations_preserve_distinct_entry_scalar_values() {
    for boundary in [false, true] {
        for (projection, empty, residuals) in [
            ("right", false, vec![path(&["left"])]),
            ("right", true, Vec::new()),
            (
                "grid[1]",
                false,
                vec![path(&["tail"]), path(&["grid", "0"]), path(&["left"])],
            ),
        ] {
            let nested = projection == "grid[1]";
            let consumer = if nested { "take_row" } else { "take" };
            let source = anonymous_source(
                boundary,
                nested,
                &format!("Sink::{consumer}(result.{projection}); Trace::tick(second); Trace::tick(first);"),
            )
            .replace("Root::enter(value: Pair) {", "Root::enter(first: u16, second: u16, value: Pair) reaches Trace {")
            .replace("Root::enter() reaches Factory", "Root::enter(first: u16, second: u16) reaches Factory + Trace");
            let source = format!(
                "{source} boundary trait Trace {{ machine tick(value: u16) reaches Trace; }}"
            );
            let source = if empty {
                source.replace("left: Token; right: Token;", "right: Token;")
            } else {
                source
            };
            for (first, second) in [(0, 65535), (391, 17)] {
                assert_source_with_scalars(
                    &source,
                    boundary,
                    std::slice::from_ref(&residuals),
                    &[integer(first), integer(second)],
                    &[integer(second), integer(first)],
                );
            }
        }
    }
}

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
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked(&source), "Root::enter")
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
fn scalar_projection_admission_keeps_parameter_and_final_return_limits() {
    for body in [
        "Sink::take(value.right); Sink::number(first);",
        "Sink::take(Root::forward(value).right);",
    ] {
        let source = format!(
            "data Token {{ value: u64; }} data Pair {{ left: Token; right: Token; }}
             data Root {{}} data Sink {{}}
             machine Root::forward(value: Pair) -> Pair {{ value }}
             machine Sink::take(value: Token) {{}}
             machine Sink::number(value: u16) {{}}
             machine Root::enter(first: u16, value: Pair) {{ {body} }}"
        );
        assert!(
            terminal_production::TerminalProductionRequest::new(&checked(&source), "Root::enter")
                .produce_artifact()
                .is_err(),
            "only result-root Jump continuations admit scalar callers"
        );
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
                terminal_production::TerminalProductionRequest::new(&original, "Root::enter")
                    .produce_artifact()
                    .unwrap();
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
                    terminal_production::TerminalProductionRequest::new(&changed, "Root::enter")
                        .produce_artifact()
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
                            "Root::enter"
                        )
                        .produce_artifact()
                        .is_err(),
                        "boundary={boundary}, empty={empty}, event={:?}, permission mutation={mutation}",
                        event.kind
                    );
                }
            }
        }
    }
}
