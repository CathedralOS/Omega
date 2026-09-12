//! Checked state-graph providers retain their exact body and ordinary call closure.

use super::*;
use terminal_interpreter::{
    ProviderInstallationSelection, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    admit_provider_installation_from_artifact,
};

const SOURCE: &str = r#"
    data ReadResult { case Empty; case Bytes(count: u64); }
    boundary trait Host {
        machine read(flag: bool) -> ReadResult reaches Host invokes Host;
        machine mark(value: u64) reaches Host;
    }
    data Provider {}
    machine Provider::read(flag: bool) -> ReadResult satisfies Host::read {
        transition flag { true -> bytes() false -> empty() }
        state bytes() { ReadResult::Bytes { count: 7 } }
        state empty() { ReadResult::Empty }
    }
    machine run(flag: bool) reaches Host {
        Host::mark(1);
        _ = Host::read(flag);
        Host::mark(2);
    }
"#;

#[test]
fn composed_provider_candidate_publishes_from_ordinary_discarding_caller() {
    let checked = checked_source(SOURCE);
    let provider = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Provider::read")
        .expect("authored attached provider");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(provider.symbol)
            .is_some()
    );
    let artifact = produce_terminal_artifact(&checked, "run")
        .expect("checked state-graph provider belongs to the ordinary caller closure");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let [candidate] = module.provider_candidates.as_slice() else {
        panic!("one exact authored checked-body provider")
    };
    assert!(
        module
            .machines
            .iter()
            .any(|machine| machine.id == candidate.candidate)
    );
}

#[test]
fn composed_provider_candidate_preserves_helper_effects_across_every_fuel_pause() {
    let source = SOURCE
        .replace(
            "reaches Host invokes Host",
            "reaches Host + Relay invokes Relay",
        )
        .replace(
            "machine run(flag: bool) reaches Host",
            "machine run(flag: bool) reaches Host + Relay",
        )
        .replace(
            "state bytes() { ReadResult::Bytes",
            "state bytes() { mark_middle(); ReadResult::Bytes",
        )
        .replace(
            "state empty() { ReadResult::Empty",
            "state empty() { mark_middle(); ReadResult::Empty",
        )
        + r#"
            boundary trait Relay {
                machine mark(value: u64) reaches Host invokes Host;
            }
            data RelayProvider {}
            machine RelayProvider::mark(value: u64) satisfies Relay::mark reaches Host {
                Host::mark(value);
            }
            machine mark_middle() reaches Host + Relay { Relay::mark(7); }
        "#;
    let checked = checked_source(&source);
    let artifact =
        produce_terminal_artifact(&checked, "run").expect("complete provider helper closure");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(
        module.provider_candidates.len(),
        2,
        "nested provider discovery reaches a fixed point"
    );
    assert_eq!(
        module.machines.len(),
        4,
        "caller, graph provider, shared ordinary helper, and nested provider"
    );
    let profile = proof_admission::AdmissionProfile::default();
    let installation = admit_provider_installation_from_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        &module
            .provider_candidates
            .iter()
            .map(|candidate| ProviderInstallationSelection {
                boundary: candidate.boundary,
                provider_identity: candidate.provider_identity.clone(),
                candidate: candidate.candidate,
            })
            .collect::<Vec<_>>(),
    )
    .expect("source-free exact provider installation");
    drop(checked);
    struct Host {
        values: Vec<u128>,
        installed_boundaries: Vec<semantic_vocabulary::BoundaryMachineId>,
    }
    impl TerminalEffectHandler for Host {
        fn handle_effect(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                boundary,
                arguments,
                ..
            } = effect
            else {
                panic!("only authored mark effects")
            };
            assert!(
                !self.installed_boundaries.contains(boundary),
                "both installed requirements execute their checked bodies"
            );
            let [
                TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("exact scalar mark argument")
            };
            self.values.push(*value);
            Ok(())
        }
    }
    for flag in [false, true] {
        let mut usages = Vec::new();
        for incremental in [false, true] {
            let mut execution = TerminalExecution::start_artifact_with_provider_installation(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
                &[TerminalScalarValue::Boolean(flag)],
                &[],
                &[],
                &installation,
            )
            .unwrap();
            let mut host = Host {
                values: Vec::new(),
                installed_boundaries: module
                    .provider_candidates
                    .iter()
                    .map(|candidate| candidate.boundary)
                    .collect(),
            };
            let mut fuel =
                terminal_fuel::TerminalFuelMeter::with_allowance(if incremental { 0 } else { 200 });
            let mut complete = false;
            for _ in 0..200 {
                match execution
                    .resume_with_effect_handler(&mut fuel, &mut host)
                    .unwrap()
                {
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                        complete = true;
                        break;
                    }
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        let before = host.values.clone();
                        assert!(matches!(
                            execution
                                .resume_with_effect_handler(&mut fuel, &mut host)
                                .unwrap(),
                            TerminalExecutionStatus::SponsorExhausted(_)
                        ));
                        assert_eq!(host.values, before, "paused effect is not replayed");
                        fuel.replenish(1).unwrap();
                    }
                    status => panic!("unexpected execution status {status:?}"),
                }
            }
            assert!(complete);
            assert_eq!(host.values, [1, 7, 2]);
            assert_eq!(execution.effects().len(), 3);
            usages.push(fuel.into_usage());
        }
        assert_eq!(usages[0], usages[1]);
    }
}

#[test]
fn composed_provider_candidate_preserves_borrowed_byte_view_signature() {
    let source = SOURCE
        .replace("flag: bool)", "flag: bool, buffer: &mut [u8])")
        .replace("Host::read(flag)", "Host::read(flag, buffer)");
    let checked = checked_source(&source);
    let artifact = produce_terminal_artifact(&checked, "run")
        .expect("borrowed mutable byte view remains part of the composed provider signature");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let [candidate] = module.provider_candidates.as_slice() else {
        panic!("one exact checked provider")
    };
    let provider = module
        .machines
        .iter()
        .find(|machine| machine.id == candidate.candidate)
        .unwrap();
    assert_eq!(provider.structural_parameters.len(), 1);
    let _ = admit_provider_installation_from_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        }],
    )
    .expect("independent provider signature admission preserves borrowed view");
}

#[test]
fn composed_provider_candidate_rejects_result_and_body_roster_corruption() {
    let source = SOURCE.to_owned() + "machine identity(value: ReadResult) -> ReadResult { value }";
    let baseline = checked_source(&source);
    let _ = produce_terminal_artifact(&baseline, "run").expect("unmodified exact provider");
    let provider = baseline.facts.flow.terminal_unit_effects.composed_machines[0].machine;
    for corruption in 0..6 {
        let mut changed = baseline.clone();
        let plans = &mut changed.facts.flow.terminal_unit_effects;
        match corruption {
            0 | 1 => {
                let checked_trees::CheckedControlResultPlan::Structural(result) =
                    &mut plans.composed_machines[0].result
                else {
                    panic!("provider owns a structural sum result")
                };
                if corruption == 0 {
                    result.type_identity = "named(name(OtherResult))".to_owned();
                } else {
                    result.multiplicity = Multiplicity::Linear;
                }
            }
            2 => plans
                .composed_machines
                .push(plans.composed_machines[0].clone()),
            3 => {
                let mut ordinary = plans.machines[0].clone();
                ordinary.machine = provider;
                plans.machines.push(ordinary);
            }
            4 => {
                let mut affine = changed
                    .facts
                    .flow
                    .terminal_structural_returns
                    .claim_free_affine_machines[0]
                    .clone();
                affine.machine = provider;
                changed
                    .facts
                    .flow
                    .terminal_structural_returns
                    .claim_free_affine_machines
                    .push(affine);
            }
            _ => plans.composed_machines.clear(),
        }
        assert!(
            produce_terminal_artifact(&changed, "run").is_err(),
            "provider corruption {corruption}"
        );
    }
}
