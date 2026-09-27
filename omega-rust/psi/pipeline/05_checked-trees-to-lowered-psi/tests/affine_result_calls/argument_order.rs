use super::{
    AdmissionProfile, FuelChargeSite, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalFuelMeter, TerminalScalarValue, TerminalStructuralValue,
    checked, decode_module, decode_proof_bundle, encode_module, encode_proof_section,
    lower_machine,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
fn assert_order(source: &str, arguments: &[TerminalScalarValue], expected: &[(usize, usize)]) {
    let checked = checked(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::caller"))
        .expect("mixed arguments lower");
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
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
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        arguments,
        TerminalStructuralInputs {
            arguments: &inputs,
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let mut observed = Vec::new();
    let mut charged = Vec::new();
    let mut completed = false;
    for _ in 0..=certificate.ceiling_units() {
        let status = execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap();
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
                assert_eq!(
                    execution
                        .resume(&mut meter, &mut AcceptTerminalEffects)
                        .unwrap(),
                    status
                );
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
