use super::*;

const CHAIN: &str = "data Value { number: u64; }
    machine forward(value: Value) -> Value { value }
    machine Main::consume(value: Value) {}
    machine Main::caller(value: Value) {
        let first: Value = forward(value);
        let second: Value = forward(first);
        Main::consume(second);
    }";

#[test]
fn structural_result_feeds_another_producer() {
    assert_chain(CHAIN, "Main::caller", &[], &[0, 1, 2], 7);
}

fn assert_chain(
    source: &str,
    name: &str,
    scalar_arguments: &[TerminalScalarValue],
    statements: &[usize],
    fuel: usize,
) {
    let checked = checked(source);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| {
            plan.operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralCall { .. }
                    )
                })
                .count()
                == 2
        })
        .expect("checked result sequence");
    let scalar_bindings = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. }
            | CheckedUnitEffectOperationPlan::ScalarCall { result, .. } => {
                Some(result.binding_ordinal)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        scalar_bindings,
        (0..scalar_bindings.len() as u32).collect::<Vec<_>>()
    );
    assert_eq!(
        plan.operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralCall { result, .. } =>
                    Some(result.binding_ordinal),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [0, 1]
    );
    let lowered = lower_machine(&checked, name)
        .unwrap_or_else(|error| panic!("result chain lowers: {error:?}\n{source}"));
    let module = &lowered.semantic_module;
    let caller = &module.machines[0];
    let structural_calls = caller.blocks[0]
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::CallStructuralWithScalarArguments { .. }
            )
        })
        .collect::<Vec<_>>();
    let [first, second] = structural_calls.as_slice() else {
        panic!("two structural producers")
    };
    let OperationResult::Structural(first_result) = &first.result else {
        unreachable!()
    };
    let OperationResult::Structural(second_result) = &second.result else {
        unreachable!()
    };
    assert_ne!(first_result.place, second_result.place);
    let OperationKind::CallStructuralWithScalarArguments {
        structural_arguments,
        ..
    } = &second.kind
    else {
        unreachable!()
    };
    assert_eq!(structural_arguments[0].place, first_result.place);
    assert!(structural_arguments[0].path.is_empty());
    let last = caller.blocks[0].operations.last().unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &last.kind
    else {
        panic!("Unit consumer")
    };
    assert_eq!(structural_arguments[0].place, second_result.place);
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &caller.blocks[0].terminator
    else {
        unreachable!()
    };
    assert!(trivial_affine_discards.is_empty());
    assert_eq!(
        lowered
            .source_call_occurrences
            .iter()
            .map(|occurrence| occurrence.statement_index)
            .collect::<Vec<_>>(),
        statements
    );
    assert!(
        lowered
            .source_call_occurrences
            .iter()
            .all(|occurrence| occurrence.call_ordinal == 0)
    );
    assert_execution(module, &lowered.proof_bundle, scalar_arguments, fuel);
}

fn assert_execution(
    module: &terminal_psi::TerminalModule,
    proof_bundle: &terminal_psi::ProofBundle,
    scalar_arguments: &[TerminalScalarValue],
    fuel: usize,
) {
    let semantic = encode_module(module).unwrap();
    let proof = encode_proof_bundle(proof_bundle).unwrap();
    let decoded = decode_module(&semantic).unwrap();
    let decoded_proof = decode_proof_bundle(&proof).unwrap();
    let verified =
        terminal_verifier::verify_module(&decoded, &decoded_proof, &AdmissionProfile::default())
            .unwrap();
    let certificate =
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, decoded.entry).unwrap();
    terminal_fixed_fuel::validate_fixed_entry_fuel(&verified, &certificate).unwrap();
    assert_eq!(certificate.ceiling_units(), fuel as u64);
    let caller = &module.machines[0];
    let structural_arguments = caller
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 0xcafe + index as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        scalar_arguments,
        &structural_arguments,
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..fuel {
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
    assert_eq!(meter.usage().total_units(), fuel as u64);
    for operation in &caller.blocks[0].operations {
        assert_eq!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(operation.id))
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
fn free_callers_chain_nested_arrays_and_generic_results() {
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
                let first: {identity} = forward(value);
                let second: {identity} = forward(first);
                Main::consume(second);
            }}"
        );
        assert_chain(&source, "caller", &[], &[0, 1, 2], 7);
    }
}

#[test]
fn calls_before_and_between_initializers_preserve_authored_order() {
    let source = "data Value { number: u64; }
        machine forward(value: Value) -> Value { value }
        machine Main::tick() {}
        machine Main::consume(value: Value) {}
        machine caller(value: Value) {
            Main::tick();
            let first: Value = forward(value);
            Main::tick();
            let second: Value = forward(first);
            Main::consume(second);
        }";
    assert_chain(source, "caller", &[], &[0, 1, 2, 3, 4], 11);
}

#[test]
fn scalar_and_structural_bindings_have_independent_dense_ordinals() {
    let source = "data Value { number: u64; }
        machine numeric(value: u32) -> u32 { value }
        machine forward(value: Value, count: u32) -> Value { value }
        machine Main::consume(value: Value, count: u32) {}
        machine Main::caller(value: Value, count: u32) {
            let before: u32 = count ^ 1u32;
            let first: Value = forward(value, before);
            let between: u32 = before ^ 2u32;
            let second: Value = forward(first, between);
            let after: u32 = between ^ 3u32;
            Main::consume(second, after);
        }";
    assert_chain(
        source,
        "Main::caller",
        &[TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            value: IntegerValue::Unsigned(4),
        }],
        &[1, 3, 5],
        13,
    );
    assert_chain(
        &source.replace("before ^ 2u32", "numeric(before)"),
        "Main::caller",
        &[TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            value: IntegerValue::Unsigned(4),
        }],
        &[1, 2, 3, 5],
        13,
    );
}

#[test]
fn transitive_chains_share_the_producer_catalog() {
    let source = format!("{CHAIN} machine Main::root(value: Value) {{ Main::caller(value); }}");
    let checked = checked(&source);
    let lowered = lower_machine(&checked, "Main::root").expect("transitive chain lowers");
    assert_eq!(lowered.semantic_module.machines.len(), 4);
    assert_eq!(lowered.source_call_occurrences.len(), 4);
    assert_execution(&lowered.semantic_module, &lowered.proof_bundle, &[], 9);
}

#[test]
fn unused_results_are_disposed_in_reverse_order_before_older_parameters() {
    let source = "data Value { number: u64; }
        machine forward(value: Value) -> Value { value }
        machine Main::caller(first: Value, second: Value, older: Value) {
            let earlier: Value = forward(first);
            let later: Value = forward(second);
        }";
    let checked = checked(source);
    let lowered = lower_machine(&checked, "Main::caller").expect("two retained results lower");
    let caller = &lowered.semantic_module.machines[0];
    let results = caller.blocks[0]
        .operations
        .iter()
        .map(|operation| {
            let OperationResult::Structural(result) = &operation.result else {
                panic!("structural result")
            };
            result.place
        })
        .collect::<Vec<_>>();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &caller.blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        trivial_affine_discards,
        &[
            results[1],
            results[0],
            caller.structural_parameters[2].place
        ]
    );
    assert_execution(&lowered.semantic_module, &lowered.proof_bundle, &[], 5);
}

#[test]
fn result_chains_reject_binding_source_and_cleanup_drift() {
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan as ArgumentSource;
    for mutation in 0..7 {
        let mut checked = checked(CHAIN);
        let caller = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| {
                plan.operations
                    .iter()
                    .filter(|operation| {
                        matches!(
                            operation,
                            CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        )
                    })
                    .count()
                    == 2
            })
            .expect("checked chain");
        match mutation {
            0..=3 => {
                let CheckedUnitEffectOperationPlan::StructuralCall {
                    result,
                    structural_arguments,
                    discard_result_on_return,
                    ..
                } = &mut caller.operations[1]
                else {
                    unreachable!()
                };
                match mutation {
                    0 => result.binding_ordinal = 0,
                    1 => {
                        structural_arguments[0].source =
                            ArgumentSource::StructuralResult { binding_ordinal: 1 }
                    }
                    2 => {
                        structural_arguments[0].source =
                            ArgumentSource::Parameter { parameter_index: 0 }
                    }
                    3 => *discard_result_on_return = true,
                    _ => unreachable!(),
                }
            }
            4 => caller.operations.swap(0, 1),
            5 => {
                let CheckedUnitEffectOperationPlan::StructuralCall {
                    discard_result_on_return,
                    ..
                } = &mut caller.operations[0]
                else {
                    unreachable!()
                };
                *discard_result_on_return = true;
            }
            6 => {
                let CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } = &mut caller.operations[2]
                else {
                    unreachable!()
                };
                structural_arguments[0].source =
                    ArgumentSource::StructuralResult { binding_ordinal: 0 };
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
fn a_structural_consumer_moves_its_input_in_the_source_timeline() {
    let source = CHAIN.replace(
        "Main::consume(second);",
        "Main::consume(first); Main::consume(second);",
    );
    let error = typed_trees_to_checked_trees::lower_typed_trees(typed(&source))
        .expect_err("first already moved into second");
    assert!(format!("{error:?}").contains("already transferred or consumed"));
}
