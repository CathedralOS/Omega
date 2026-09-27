use super::{
    AdmissionProfile, FuelChargeSite, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalFuelMeter, TerminalScalarValue, TerminalStructuralValue,
    decode_module, decode_proof_bundle, encode_module, encode_proof_section,
};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;

pub(super) fn assert_execution(
    module: &terminal_psi::TerminalModule,
    proof_bundle: &terminal_psi::ProofBundle,
    scalar_arguments: &[TerminalScalarValue],
    fuel: usize,
) {
    let semantic = encode_module(module).unwrap();
    let proof = encode_proof_section(module, proof_bundle).unwrap();
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
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        scalar_arguments,
        TerminalStructuralInputs {
            arguments: &structural_arguments,
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..fuel {
        let paused = execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap();
        assert!(matches!(
            paused,
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        let frontier = execution
            .live_affine_frontier()
            .cloned()
            .collect::<Vec<_>>();
        let units = meter.usage().total_units();
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            paused
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
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
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
