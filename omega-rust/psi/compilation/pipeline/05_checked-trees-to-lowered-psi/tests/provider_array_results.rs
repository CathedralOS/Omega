//! Installed checked providers return concrete array backing through ordinary frames.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_codec::{CanonicalTerminalArtifact, decode_module, encode_module};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    ProviderInstallationSelection, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
    TerminalScalarValue, TerminalStructuralInputs, admit_provider_installation_from_artifact,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, OperationResult, StructuralMultiplicity};

fn artifact(carrier: &str, value: &str, condition: &str) -> CanonicalTerminalArtifact {
    let source = format!(
        "boundary trait Factory {{ machine produce() -> {carrier}; }}
         data Provider {{}}
         machine Provider::produce() -> {carrier} satisfies Factory::produce {{ {value} }}
         machine forward(value: {carrier}) -> {carrier} {{ value }}
         machine run() -> u32 reaches Factory {{
             let produced: {carrier} = Factory::produce();
             let retained: {carrier} = forward(produced);
             transition {condition} {{ true -> (70) false -> (72) }}
         }}"
    );
    let checked = crate::front_end::checked_program(&source);
    terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("array result retains its installed provider and concrete payload")
    .into_artifact()
}

fn selection(artifact: &CanonicalTerminalArtifact) -> ProviderInstallationSelection {
    let module = decode_module(artifact.semantic_bytes()).unwrap();
    let [candidate] = module.provider_candidates.as_slice() else {
        panic!("one checked array provider");
    };
    ProviderInstallationSelection {
        boundary: candidate.boundary,
        provider_identity: candidate.provider_identity.clone(),
        candidate: candidate.candidate,
    }
}

struct NoHostEffects;

impl TerminalEffectHandler for NoHostEffects {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("an installed checked provider cannot fall back to a host effect: {effect:?}")
    }
}

#[test]
fn host_only_array_results_reject_before_invoking_the_handler() {
    for length in [0, 2] {
        let checked = crate::front_end::checked_program(&format!(
            "boundary trait Factory {{ machine produce() -> [u8; {length}]; }}
             machine run() reaches Factory {{
                 let produced: [u8; {length}] = Factory::produce();
             }}"
        ));
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("run"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("boundary array signature is a valid portable product")
        .into_artifact();
        assert!(
            decode_module(artifact.semantic_bytes())
                .unwrap()
                .provider_candidates
                .is_empty()
        );
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
        )
        .unwrap();
        assert!(
            matches!(
                execution.resume(&mut TerminalFuelMeter::default(), &mut NoHostEffects),
                Err(TerminalInterpretError::VerifiedOperationMalformed)
            ),
            "host array length {length}"
        );
    }
}

fn expected_result() -> TerminalExecutionResult {
    TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        value: IntegerValue::Unsigned(70),
    })
}

#[test]
fn installed_array_results_keep_payload_through_reload_forwarding_and_suspension() {
    for (carrier, value, condition) in [
        (
            "[u8; 2]",
            "[30, 40]",
            "retained[0] == 30 && retained[1] == 40",
        ),
        (
            "[[u8; 1]; 2]",
            "[[30], [40]]",
            "retained[0][0] == 30 && retained[1][0] == 40",
        ),
        ("[u8; 0]", "[]", "true"),
    ] {
        let artifact = artifact(carrier, value, condition);
        let selected = selection(&artifact);
        let installation = admit_provider_installation_from_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            &[selected],
        )
        .expect("exact checked provider installs from the reloaded artifact");
        let mut execution = TerminalExecution::start_installed_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
            &installation,
        )
        .unwrap();
        let mut suspended = false;
        let mut completed = false;
        // Increasing grants also cross operations costing more than one unit.
        // The same execution retains its suspended provider and caller frames.
        for allowance in 0..128 {
            match execution
                .resume(
                    &mut TerminalFuelMeter::with_allowance(allowance),
                    &mut NoHostEffects,
                )
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => suspended = true,
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, expected_result(), "carrier {carrier}");
                    completed = true;
                    break;
                }
                TerminalExecutionStatus::Crashed(crash) => {
                    panic!("unexpected provider crash: {crash:?}")
                }
            }
        }
        assert!(
            suspended && completed,
            "suspension resumes to the authored array result: {carrier}"
        );
        assert!(execution.effects().is_empty());
    }
}

#[test]
fn array_provider_requires_exact_installation_without_host_fallback() {
    let artifact = artifact(
        "[u8; 2]",
        "[30, 40]",
        "retained[0] == 30 && retained[1] == 40",
    );
    let selected = selection(&artifact);
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    assert!(
        matches!(execution.resume(&mut TerminalFuelMeter::default(), &mut NoHostEffects),
        Err(TerminalInterpretError::ProviderInstallationMissing(boundary)) if boundary == selected.boundary)
    );
    let module = decode_module(artifact.semantic_bytes()).unwrap();
    for mutation in 0..2 {
        let mut changed = selected.clone();
        if mutation == 0 {
            changed.provider_identity = "MissingProvider".into();
        } else {
            changed.candidate = module.entry;
        }
        assert!(
            admit_provider_installation_from_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &AdmissionProfile::default(),
                &[changed],
            )
            .is_err(),
            "installation mutation {mutation}"
        );
    }
}

#[test]
fn array_provider_result_and_conformance_drift_reject_after_reload() {
    let artifact = artifact(
        "[u8; 2]",
        "[30, 40]",
        "retained[0] == 30 && retained[1] == 40",
    );
    let selected = selection(&artifact);
    let module = decode_module(artifact.semantic_bytes()).unwrap();
    for mutation in 0..3 {
        let mut changed = module.clone();
        if mutation == 0 {
            changed.provider_candidates[0].candidate = changed.entry;
        } else {
            let operation = changed
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
                .flat_map(|block| &mut block.operations)
                .find(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
                .unwrap();
            let OperationResult::Structural(result) = &mut operation.result else {
                panic!("array boundary result");
            };
            if mutation == 1 {
                result.multiplicity = StructuralMultiplicity::Affine;
            } else {
                result.structural_type =
                    semantic_vocabulary::StructuralTypeId::new(u64::MAX).unwrap();
            }
        }
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "result/conformance mutation {mutation}"
        );
        // Canonical encoding independently rejects malformed module custody.
        // If its formation boundary admits a changed representation, sealed
        // admission must still refuse the original artifact's evidence.
        if let Ok(bytes) = encode_module(&changed) {
            assert!(
                admit_provider_installation_from_artifact(
                    &bytes,
                    artifact.proof_bytes(),
                    &AdmissionProfile::default(),
                    &[selected.clone()],
                )
                .is_err(),
                "changed artifact cannot reuse provider admission: {mutation}"
            );
        }
    }
}
