use super::*;

fn assert_order(source: &str, arguments: &[TerminalScalarValue], expected: &[(usize, usize)]) {
    let checked = checked(source);
    let lowered = lower_machine(&checked, "Main::caller").expect("mixed arguments lower");
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let proof = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof_bundle = decode_proof_bundle(&proof).unwrap();
    let verified =
        terminal_verifier::verify_module(&module, &proof_bundle, &AdmissionProfile::default())
            .unwrap();
    let certificate =
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry).unwrap();
    terminal_fixed_fuel::validate_fixed_entry_fuel(&verified, &certificate).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let inputs = caller
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 100 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        arguments,
        &inputs,
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let mut observed = Vec::new();
    let mut charged = Vec::new();
    let mut completed = false;
    for _ in 0..=certificate.ceiling_units() {
        let status = execution.resume(&mut meter).unwrap();
        for receipt in &lowered.source_call_occurrences {
            if meter
                .usage()
                .at(FuelChargeSite::Operation(receipt.terminal_operation))
                .is_some()
                && !charged.contains(&receipt.terminal_operation)
            {
                charged.push(receipt.terminal_operation);
                observed.push((receipt.statement_index, receipt.call_ordinal));
            }
        }
        match status {
            TerminalExecutionStatus::SponsorExhausted(_) => {
                let units = meter.usage().total_units();
                let frontier = execution
                    .live_affine_frontier()
                    .cloned()
                    .collect::<Vec<_>>();
                assert_eq!(execution.resume(&mut meter).unwrap(), status);
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
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                completed = true;
                break;
            }
            other => panic!("mixed argument execution failed: {other:?}"),
        }
    }
    assert!(completed, "fixed fuel bounds mixed evaluation");
    assert_eq!(observed, expected);
    for operation in charged {
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

fn integer(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn computed_scalar_operands_interleave_with_structural_results() {
    assert_order(
        "data Value { number: u64; }
         machine forward(value: Value) -> Value { value }
         machine numeric(count: u32) -> u32 { count ^ 1u32 }
         machine Main::consume(before: u32, first: Value, between: u32, second: Value, after: u32) {}
         machine Main::caller(count: u32, first: Value, second: Value) {
             Main::consume(numeric(count), forward(first), numeric(count ^ 1u32), forward(second), numeric(count ^ 2u32));
         }",
        &[integer(4)],
        &[(0, 1), (0, 2), (0, 3), (0, 4), (0, 5), (0, 0)],
    );
}

#[test]
fn scalar_calls_inside_structural_producers_keep_their_own_argument_roles() {
    assert_order(
        "data Value { number: u64; }
         machine forward(value: Value, count: u32) -> Value { value }
         machine numeric(count: u32) -> u32 { count ^ 1u32 }
         machine Main::consume(count: u32, value: Value) {}
         machine Main::caller(count: u32, value: Value) {
             Main::consume(numeric(count), forward(forward(value, numeric(count)), numeric(count ^ 1u32)));
         }",
        &[integer(4)],
        &[(0, 1), (0, 4), (0, 3), (0, 5), (0, 2), (0, 0)],
    );
}

#[test]
fn pure_nested_operands_use_the_pre_statement_scalar_namespace() {
    assert_order(
        "data Value { number: u64; }
         machine forward(value: Value, count: u32) -> Value { value }
         machine Main::consume(before: u32, value: Value, after: u32) {}
         machine Main::caller(count: u32, value: Value) {
             let prior: u32 = count ^ 2u32;
             Main::consume(prior ^ 4u32, forward(value, prior ^ 1u32), prior ^ 8u32);
         }",
        &[integer(4)],
        &[(1, 1), (1, 0)],
    );
}

#[test]
fn selective_scalar_operands_preserve_skipped_calls_and_live_structural_results() {
    let source = "data Value { number: u64; }
         machine forward(value: Value) -> Value { value }
         machine negate(flag: bool) -> bool { !flag }
         machine Main::consume(before: bool, first: Value, between: bool, second: Value, after: bool) {}
         machine Main::caller(flag: bool, first: Value, second: Value) {
             Main::consume(flag && negate(flag), forward(first), !flag || negate(flag), forward(second), negate(flag));
         }";
    assert_order(
        source,
        &[TerminalScalarValue::Boolean(false)],
        &[(0, 2), (0, 4), (0, 5), (0, 0)],
    );
    assert_order(
        source,
        &[TerminalScalarValue::Boolean(true)],
        &[(0, 1), (0, 2), (0, 3), (0, 4), (0, 5), (0, 0)],
    );
    assert_order(
        &source
            .replace("flag && negate(flag)", "false && negate(flag)")
            .replace("!flag || negate(flag)", "true || negate(flag)"),
        &[TerminalScalarValue::Boolean(true)],
        &[(0, 2), (0, 4), (0, 5), (0, 0)],
    );
}

#[test]
fn argument_failure_preserves_only_already_established_structural_storage() {
    for structural_first in [false, true] {
        let (signature, operands) = if structural_first {
            (
                "value: Value, flag: bool",
                "forward(value), checked_flag(flag)",
            )
        } else {
            (
                "flag: bool, value: Value",
                "checked_flag(flag), forward(value)",
            )
        };
        let source = format!("data Value {{ number: u64; }}
            machine forward(value: Value) -> Value {{ value }}
            machine checked_flag(flag: bool) -> bool crashes Abort {{ transition {{ flag -> true }} crash Abort; }}
            machine Main::consume({signature}) {{}}
            machine Main::caller(flag: bool, value: Value) crashes Abort {{ Main::consume({operands}); }}");
        let checked = checked(&source);
        let lowered =
            lower_machine(&checked, "Main::caller").expect("crashing mixed operands lower");
        let module = &lowered.semantic_module;
        let caller = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let producer = caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::CallStructuralWithScalarArguments { .. }
                )
            })
            .unwrap();
        let OperationResult::Structural(result) = &producer.result else {
            unreachable!()
        };
        let semantic = encode_module(module).unwrap();
        let proof = encode_proof_bundle(&lowered.proof_bundle).unwrap();
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(false)],
            &[TerminalStructuralValue {
                opaque_identity: 17,
                structural_type: caller.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
        )
        .unwrap();
        let failing_call = lowered
            .source_call_occurrences
            .iter()
            .find(|receipt| receipt.call_ordinal == if structural_first { 2 } else { 1 })
            .unwrap();
        let mut meter = TerminalFuelMeter::with_allowance(0);
        loop {
            let status = execution.resume(&mut meter).unwrap();
            let TerminalExecutionStatus::SponsorExhausted(exhaustion) = status else {
                panic!("must pause before entering the failing scalar call");
            };
            if exhaustion.site == FuelChargeSite::Operation(failing_call.terminal_operation) {
                break;
            }
            meter.replenish(1).unwrap();
        }
        // The public frontier describes the current frame, not suspended
        // callers. Observe the caller before entering its failing operand.
        let frontier = execution.live_affine_frontier().collect::<Vec<_>>();
        assert_eq!(frontier.len(), 1);
        let expected = if structural_first {
            result.place
        } else {
            caller.structural_parameters[0].place
        };
        assert_eq!(frontier[0].place, expected);
        meter.replenish(1000).unwrap();
        let outcome = execution.resume(&mut meter);
        assert!(
            matches!(&outcome,
            Ok(TerminalExecutionStatus::Crashed(crash))
                if crash.cause == terminal_psi::CrashCause::Abort),
            "{outcome:?}"
        );
        assert_eq!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(producer.id))
                .is_some(),
            structural_first
        );
        for block in &caller.blocks {
            if let Terminator::ReturnUnit { edge, .. } = block.terminator {
                assert!(
                    meter.usage().at(FuelChargeSite::Edge(edge)).is_none(),
                    "caller cleanup cannot run after operand failure"
                );
            }
        }
        let outer = lowered
            .source_call_occurrences
            .iter()
            .find(|receipt| receipt.call_ordinal == 0)
            .unwrap();
        assert!(
            meter
                .usage()
                .at(FuelChargeSite::Operation(outer.terminal_operation))
                .is_none()
        );
    }
}

#[test]
fn staged_arguments_do_not_change_later_source_local_ordinals() {
    assert_order(
        "data Value { number: u64; }
         machine forward(value: Value) -> Value { value }
         machine numeric(count: u32) -> u32 { count ^ 1u32 }
         machine Main::consume(count: u32, value: Value) {}
         machine Main::caller(count: u32, first: Value, second: Value) {
             let before: u32 = count ^ 2u32;
             Main::consume(numeric(before), forward(first));
             let after: u32 = before ^ 4u32;
             Main::consume(numeric(after), forward(second));
         }",
        &[integer(4)],
        &[(1, 1), (1, 2), (1, 0), (3, 1), (3, 2), (3, 0)],
    );
}
