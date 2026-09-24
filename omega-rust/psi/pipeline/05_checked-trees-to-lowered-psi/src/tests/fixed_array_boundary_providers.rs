//! Installed checked providers write the caller's fixed extent, not replacement storage.
use super::{CheckedTrees, byte_sequence_write, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    ProviderInstallationSelection, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalStructuralByteArrayValue, TerminalStructuralValue,
    admit_provider_installation_from_artifact,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::StructuralPathSegment;

const PROVIDER: &str = r#"
    data ReadResult { case Empty; case Bytes(count: u64); }
    boundary trait Input {
        machine read(out: &mut [u8], byte: u8) -> ReadResult reaches Input;
    }
    data Provider {}
    machine Provider::read(out: &mut [u8], byte: u8) -> ReadResult satisfies Input::read {
        put(out, byte);
        ReadResult::Bytes { count: 1 }
    }
"#;

fn checked_array_caller(field: bool) -> CheckedTrees {
    let caller = if field {
        r#"
            data Root { out: [u8; 3]; other: [u8; 3]; }
            machine Root::run(&mut self) reaches Input {
                _ = Input::read(&mut self.out, 65);
                _ = Input::read(&mut self.out, 0);
            }
        "#
    } else {
        r#"
            machine run(out: &mut [u8; 3]) reaches Input {
                _ = Input::read(out, 65);
                _ = Input::read(out, 0);
            }
        "#
    };
    crate::front_end::checked_program(&format!(
        "{}\n{PROVIDER}\n{caller}",
        byte_sequence_write::PUT
    ))
}

#[test]
fn fixed_array_boundary_provider_publishes_whole_root_and_record_field() {
    for field in [false, true] {
        let checked = checked_array_caller(field);
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name(if field {
                "Root::run"
            } else {
                "run"
            }),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("fixed-array boundary call retains the exact writable range")
        .into_artifact();
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        terminal_verifier::verify_module(
            &module,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
        assert_eq!(module.provider_candidates.len(), 1);
        assert_eq!(
            module.machines.len(),
            3,
            "caller, provider, ordinary write helper"
        );
    }
}

#[test]
fn fixed_array_boundary_provider_writes_original_storage_across_every_fuel_pause() {
    for field in [false, true] {
        let checked = checked_array_caller(field);
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name(if field {
                "Root::run"
            } else {
                "run"
            }),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .unwrap()
        .into_artifact();
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let [candidate] = module.provider_candidates.as_slice() else {
            panic!("one checked provider")
        };
        let profile = proof_admission::AdmissionProfile::default();
        let installation = admit_provider_installation_from_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            &[ProviderInstallationSelection {
                boundary: candidate.boundary,
                provider_identity: candidate.provider_identity.clone(),
                candidate: candidate.candidate,
            }],
        )
        .unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let argument = TerminalStructuralValue {
            opaque_identity: 73,
            structural_type: entry.structural_parameters[0].structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        let path = if field {
            vec![StructuralPathSegment::Field("out".into())]
        } else {
            Vec::new()
        };
        let sibling = vec![StructuralPathSegment::Field("other".into())];
        let mut arrays = vec![TerminalStructuralByteArrayValue {
            argument_index: 0,
            path: path.clone(),
            bytes: vec![0x11, 0x80, 0xff],
        }];
        if field {
            arrays.push(TerminalStructuralByteArrayValue {
                argument_index: 0,
                path: sibling.clone(),
                bytes: vec![0x42; 3],
            });
        }
        drop(checked);
        let mut usages = Vec::new();
        for incremental in [false, true] {
            let mut execution = TerminalExecution::start_installed_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
                &[],
                TerminalStructuralInputs {
                    arguments: std::slice::from_ref(&argument),
                    byte_arrays: &arrays,
                    ..Default::default()
                },
                &installation,
            )
            .unwrap();
            let mut fuel =
                terminal_fuel::TerminalFuelMeter::with_allowance(if incremental { 0 } else { 200 });
            let mut complete = false;
            let mut saw_first_write = false;
            for _ in 0..200 {
                let status = execution
                    .resume(&mut fuel, &mut AcceptTerminalEffects)
                    .unwrap();
                let observed = execution.structural_byte_array(73, &path).unwrap().to_vec();
                assert_eq!(
                    observed.len(),
                    3,
                    "borrowed view cannot resize the fixed extent"
                );
                assert_eq!(&observed[1..], &[0x80, 0xff], "untouched suffix survives");
                saw_first_write |= observed[0] == 65;
                if field {
                    assert_eq!(
                        execution.structural_byte_array(73, &sibling),
                        Some([0x42; 3].as_slice())
                    );
                }
                assert!(
                    execution.effects().is_empty(),
                    "installed provider never invokes a host result fabricator"
                );
                match status {
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        assert!(matches!(
                            execution
                                .resume(&mut fuel, &mut AcceptTerminalEffects)
                                .unwrap(),
                            TerminalExecutionStatus::SponsorExhausted(_)
                        ));
                        assert_eq!(
                            execution.structural_byte_array(73, &path),
                            Some(observed.as_slice()),
                            "paused work cannot replay writes"
                        );
                        fuel.replenish(1).unwrap();
                    }
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                        assert_eq!(observed, [0, 0x80, 0xff]);
                        complete = true;
                        break;
                    }
                    status => panic!("unexpected provider outcome {status:?}"),
                }
            }
            assert!(complete);
            assert!(
                !incremental || saw_first_write,
                "both calls must write the same backing"
            );
            usages.push(fuel.into_usage());
        }
        assert_eq!(
            usages[0], usages[1],
            "suspension cannot repeat charged work"
        );
    }
}

#[test]
fn fixed_array_boundary_provider_rejects_missing_initialization_and_installation() {
    let checked = checked_array_caller(false);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let candidate = &module.provider_candidates[0];
    let profile = proof_admission::AdmissionProfile::default();
    let installation = admit_provider_installation_from_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        &[ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        }],
    )
    .unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let argument = TerminalStructuralValue {
        opaque_identity: 73,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let valid = TerminalStructuralByteArrayValue {
        argument_index: 0,
        path: Vec::new(),
        bytes: vec![1, 2, 3],
    };
    for arrays in [
        vec![TerminalStructuralByteArrayValue {
            bytes: vec![1, 2],
            ..valid.clone()
        }],
        vec![valid.clone(), valid.clone()],
    ] {
        assert!(
            TerminalExecution::start_installed_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
                &[],
                TerminalStructuralInputs {
                    arguments: std::slice::from_ref(&argument),
                    byte_arrays: &arrays,
                    ..Default::default()
                },
                &installation
            )
            .is_err(),
            "short or duplicate backing rejects during startup"
        );
    }
    if let Ok(mut execution) = TerminalExecution::start_installed_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
        &installation,
    ) {
        assert!(
            execution
                .resume(
                    &mut terminal_fuel::TerminalFuelMeter::with_allowance(200),
                    &mut AcceptTerminalEffects
                )
                .is_err(),
            "an opaque root cannot fabricate initialized array bytes"
        );
        assert!(execution.effects().is_empty());
    }
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            byte_arrays: &[valid],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        matches!(execution.resume(&mut terminal_fuel::TerminalFuelMeter::with_allowance(200), &mut AcceptTerminalEffects), Err(terminal_interpreter::TerminalInterpretError::ProviderInstallationMissing(boundary)) if boundary == candidate.boundary)
    );
    assert!(
        execution.effects().is_empty(),
        "missing installation cannot fall through to a host handler"
    );
}

#[test]
fn fixed_array_boundary_provider_replays_exact_field_and_mutable_access() {
    let checked = checked_array_caller(true);
    let _ = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Root::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    for change_access in [false, true] {
        let mut changed = checked.clone();
        let call = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|plan| &mut plan.operations)
            .find(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                )
            })
            .unwrap();
        let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            structural_arguments,
            ..
        } = call
        else {
            panic!("authored boundary read")
        };
        if change_access {
            structural_arguments[0].access = checked_trees::CheckedStructuralAccess::SharedBorrow;
        } else {
            structural_arguments[0].path =
                vec![CheckedUnitStructuralPathSegment::Field("other".into())];
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Root::run")).is_err(),
            "same-typed field or access substitution cannot borrow another source operand"
        );
    }
}
