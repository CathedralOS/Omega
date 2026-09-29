//! Authored checked providers preserve borrowed field custody across calls.
//!
//! A provider body forwards its byte-view formal to another boundary, directly
//! or through an ordinary helper. A shared view of a restricted field keeps
//! its original root and complete path through that forwarding. A bare
//! mutable view cannot receive the restricted field at all: the outer
//! boundary call's field loan is refused before any provider is installed.
use super::{
    INPUT_SOURCE, LoweringError, ModuleError, OperationKind, TerminalEffect, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalMachineSelection,
    TerminalStructuralValue, lower_machine, shared_twin,
};
use terminal_interpreter::{
    ProviderInstallationSelection, TerminalEffectHandler, TerminalEffectRejection,
    TerminalStructuralInputs, admit_provider_installation_from_artifact,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

fn forwarding_source(source: &str, ordinary_helper: bool) -> String {
    let mut source = source.replace(
        "boundary trait Input { machine read(out: &mut [u8]) reaches Input; }",
        "boundary trait Input { machine read(out: &mut [u8]) reaches Input invokes Input; machine refill(out: &mut [u8]) reaches Input; }",
    ).replace(
        "machine Record::run",
        r#"
        data Provider {}
        machine Provider::read(out: &mut [u8]) satisfies Input::read reaches Input {
            Input::refill(&mut out);
        }
        machine Record::run"#,
    );
    if ordinary_helper {
        source = source.replace("Input::refill(&mut out);", "forward(&mut out);");
        source = source.replace(
            "satisfies Input::read reaches Input {",
            "satisfies Input::read {",
        );
        source.push_str(
            r#"
            machine forward(out: &mut [u8]) reaches Input {
                Input::refill(&mut out);
            }
        "#,
        );
    }
    source
}

/// The shared twin of a forwarding program: every loan, including the
/// provider's forwarded formal, is a shared byte view.
fn shared_forwarding_source(source: &str, ordinary_helper: bool) -> String {
    shared_twin(&forwarding_source(source, ordinary_helper)).replace("(&mut out)", "(out)")
}

fn array_element_source() -> String {
    INPUT_SOURCE
        .replace("data Record {", "data Cell {")
        .replace(
            "machine Record::run",
            "data Record { cells: [Cell; 2]; } machine Record::run",
        )
        .replace("self.out", "self.cells[1].out")
        .replace("self.other", "self.cells[1].other")
}

#[test]
fn checked_provider_forwarding_cannot_receive_a_restricted_field() {
    for ordinary_helper in [false, true] {
        let checked =
            crate::front_end::checked_program(&forwarding_source(INPUT_SOURCE, ordinary_helper));
        let error = lower_machine(&checked, TerminalMachineSelection::Name("Record::run"))
            .expect_err("the restricted field cannot enter a bare mutable view");
        assert!(
            matches!(
                error,
                LoweringError::InvalidTerminalModule(ModuleError::InvalidStructuralArgumentPath {
                    argument_index: 0,
                    ..
                })
            ),
            "ordinary helper {ordinary_helper}: {error:?}"
        );
    }
}

#[test]
fn checked_provider_shared_views_forward_original_array_path_across_fuel_suspension() {
    assert_forwarded_input(&shared_forwarding_source(&array_element_source(), false));
}

#[test]
fn checked_provider_shared_views_forward_through_ordinary_helper() {
    assert_forwarded_input(&shared_forwarding_source(INPUT_SOURCE, true));
}

fn assert_forwarded_input(source: &str) {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Record::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("authored forwarding provider produces verified Terminal")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let [candidate] = module.provider_candidates.as_slice() else {
        panic!("one authored provider candidate")
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
    .expect("exact source-produced provider installation");
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
    let expected_path = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } => Some(structural_arguments[0].path.clone()),
            _ => None,
        })
        .expect("authored outer field path");
    struct Input {
        calls: usize,
        outer: semantic_vocabulary::BoundaryMachineId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    }
    impl TerminalEffectHandler for Input {
        fn handle_effect(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                boundary,
                structural_arguments,
                byte_sequence_arguments,
                ..
            } = effect
            else {
                panic!("inner boundary effect")
            };
            assert_ne!(
                *boundary, self.outer,
                "installed outer call must execute its body"
            );
            assert_eq!(structural_arguments[0].opaque_identity, 73);
            assert_eq!(
                structural_arguments[0].path, self.path,
                "forwarding keeps the original field path"
            );
            assert_eq!(
                byte_sequence_arguments[0].as_deref(),
                Some(b"old".as_slice())
            );
            self.calls += 1;
            Ok(())
        }
    }
    let mut usages = Vec::new();
    for incremental in [false, true] {
        let mut execution = TerminalExecution::start_installed_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            &[],
            TerminalStructuralInputs {
                arguments: std::slice::from_ref(&argument),
                ..Default::default()
            },
            &installation,
        )
        .unwrap();
        let mut input = Input {
            calls: 0,
            outer: candidate.boundary,
            path: expected_path.clone(),
        };
        let mut fuel =
            terminal_fuel::TerminalFuelMeter::with_allowance(if incremental { 0 } else { 100 });
        let mut complete = false;
        let mut pauses = 0;
        for _ in 0..100 {
            match execution
                .resume(&mut fuel, &mut input)
                .expect("checked provider forwards the shared field view")
            {
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                    complete = true;
                    break;
                }
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    pauses += 1;
                    fuel.replenish(1).unwrap();
                }
                status => panic!("unexpected {status:?}"),
            }
        }
        assert!(complete);
        assert_eq!(pauses > 0, incremental);
        assert_eq!(input.calls, 2);
        assert_eq!(execution.effects().len(), 2);
        usages.push(fuel.into_usage());
    }
    assert_eq!(
        usages[0], usages[1],
        "suspension cannot repeat charged work"
    );
}
