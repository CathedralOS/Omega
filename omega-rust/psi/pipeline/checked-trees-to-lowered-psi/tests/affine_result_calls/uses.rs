use super::*;

const RESULT_USE: &str = "data Value { number: u64; }
    machine forward(value: Value) -> Value { value }
    machine Main::consume(value: Value) {}
    machine Main::caller(value: Value) {
        let result: Value = forward(value);
        Main::consume(result);
    }";

#[test]
fn an_ordinary_unit_call_consumes_the_retained_structural_result() {
    assert_result_use(RESULT_USE, "Main::caller");
}

fn assert_result_use(source: &str, name: &str) {
    let checked = checked(source);
    let lowered = lower_machine(&checked, name)
        .unwrap_or_else(|error| panic!("result consumer lowers: {error:?}\n{source}"));
    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let module = &lowered.semantic_module;
    let caller = &module.machines[0];
    let [producer, consumer] = caller.blocks[0].operations.as_slice() else {
        panic!("one producing and one consuming call");
    };
    let OperationResult::Structural(result) = &producer.result else {
        panic!("whole result")
    };
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        ..
    } = &consumer.kind
    else {
        panic!("ordinary Unit consumer")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].place, result.place);
    assert!(structural_arguments[0].path.is_empty());
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &caller.blocks[0].terminator
    else {
        panic!("caller Unit exit")
    };
    assert!(
        trivial_affine_discards.is_empty(),
        "caller has transferred both owned values"
    );
    let consuming_machine = module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .unwrap();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &consuming_machine.blocks[0].terminator
    else {
        panic!("consumer exit")
    };
    assert_eq!(
        trivial_affine_discards,
        &[consuming_machine.structural_parameters[0].place]
    );
    let semantic = encode_module(module).unwrap();
    let proof = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let decoded = decode_module(&semantic).unwrap();
    let proof_bundle = decode_proof_bundle(&proof).unwrap();
    terminal_verifier::verify_module(&decoded, &proof_bundle, &AdmissionProfile::default())
        .unwrap();
    assert_eq!(lowered.source_call_occurrences.len(), 2);
    assert_eq!(
        lowered.source_call_occurrences[1].terminal_operation,
        consumer.id
    );
    assert_eq!(lowered.source_call_occurrences[1].statement_index, 1);
    assert_eq!(lowered.source_call_occurrences[1].call_ordinal, 0);
    assert_eq!(
        lowered.source_call_occurrences[1].source_state,
        lowered.source_call_occurrences[0].source_state
    );
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 0xcafe,
            structural_type: caller.structural_parameters[0].structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..5 {
        let paused = execution.resume(&mut meter).unwrap();
        assert!(matches!(
            paused,
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        let frontier = execution
            .live_affine_frontier()
            .cloned()
            .collect::<Vec<_>>();
        let units = meter.usage().total_units();
        assert_eq!(execution.resume(&mut meter).unwrap(), paused);
        assert_eq!(meter.usage().total_units(), units);
        assert_eq!(
            execution
                .live_affine_frontier()
                .cloned()
                .collect::<Vec<_>>(),
            frontier
        );
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 5);
    for operation in [producer.id, consumer.id] {
        assert_eq!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(operation))
                .unwrap()
                .units(),
            1
        );
    }
    assert!(execution.live_affine_frontier().next().is_none());
    assert!(execution.live_claim_frontier().next().is_none());
    assert!(execution.effects().is_empty());
}

#[test]
fn result_reuse_preserves_nested_records_arrays_and_generic_types() {
    for (declarations, identity) in [
        (
            "data Inner { number: u64; } data Outer { inner: Inner; count: u32; }",
            "Outer",
        ),
        ("data Entry { number: u64; }", "[Entry; 3]"),
        (
            "data Entry { number: u64; } data Buffer<T> { entries: [T; 3]; }",
            "Buffer<Entry>",
        ),
    ] {
        let source = format!(
            "{declarations}
            machine forward(value: {identity}) -> {identity} {{ value }}
            machine Main::consume(value: {identity}) {{}}
            machine caller(value: {identity}) {{
                let result: {identity} = forward(value);
                Main::consume(result);
            }}"
        );
        assert_result_use(&source, "caller");
    }
}

#[test]
fn result_use_rejects_binding_cleanup_and_source_drift() {
    for mutation in 0..4 {
        let mut checked = checked(RESULT_USE);
        let caller = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| {
                plan.operations.iter().any(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralCall { .. }
                    )
                })
            })
            .unwrap();
        match mutation {
            0 => {
                let CheckedUnitEffectOperationPlan::StructuralCall {
                    discard_result_on_return,
                    ..
                } = &mut caller.operations[0]
                else {
                    unreachable!()
                };
                *discard_result_on_return = true;
            }
            1 | 2 => {
                let CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } = &mut caller.operations[1]
                else {
                    unreachable!()
                };
                structural_arguments[0].source = if mutation == 1 {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: 1,
                    }
                } else {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 0,
                    }
                };
            }
            3 => {
                caller.operations.swap(0, 1);
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&checked, "Main::caller").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn a_call_initialized_affine_local_cannot_be_moved_twice() {
    let source = RESULT_USE.replace(
        "Main::consume(result);",
        "Main::consume(result); Main::consume(result);",
    );
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed(&source))
        .expect_err("second ownership transfer must fail source checking");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("already transferred or consumed")),
        "{diagnostics:?}"
    );
}
