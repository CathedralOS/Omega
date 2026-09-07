//! Installed affine providers retain the caller's result, not the candidate's.

use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralValue,
};
use terminal_psi_to_abstract_operations::{
    SelectedProviderAdapter, admit_provider_installation,
    admit_provider_installation_for_optimization, lower_artifact_sections,
    lower_artifact_sections_for_optimization,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    pub data Token { value: u64; }
    pub data Pair { left: Token; right: Token; }
    boundary trait Factory {
        machine forward(value: Pair) -> Pair reaches Factory;
    }
    data Provider {}
    machine Provider::forward(value: Pair) -> Pair satisfies Factory::forward { value }
    data Alternative {}
    machine Alternative::forward(value: Pair) -> Pair satisfies Factory::forward { value }
    data Sink {}
    machine Sink::take(token: Token) {}
    machine enter(value: Pair) reaches Factory {
        let result: Pair = Factory::forward(value);
        Sink::take(result.right);
    }
"#;

fn artifact(source: &str) -> (terminal_psi::TerminalModule, Vec<u8>, Vec<u8>) {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "enter").unwrap();
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let proof = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    (lowered.semantic_module, semantic, proof)
}

#[test]
fn omega_installs_affine_result_providers_without_relabeling_custody() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace("value: u64;", ""),
        SOURCE.replace("left: Token; right: Token;", "left: u64; right: Token;"),
        SOURCE
            .replace(
                "left: Token; right: Token;",
                "left: Token; row: [Token; 3];",
            )
            .replace("result.right", "result.row[1]"),
        SOURCE
            .replace("let result: Pair = Factory::forward(value);", "")
            .replace("result.right", "Factory::forward(value).right"),
        SOURCE
            .replace("value: Pair", "value: [Token; 2]")
            .replace("-> Pair", "-> [Token; 2]")
            .replace("let result: Pair", "let result: [Token; 2]")
            .replace("result.right", "result[1]"),
        SOURCE
            .replace(
                "data Sink {}",
                "data Sink {} machine Sink::take_pair(value: Pair) {}",
            )
            .replace("Sink::take(result.right);", "Sink::take_pair(result);"),
    ] {
        check_source(&source);
    }
}

fn check_source(source: &str) {
    let (module, semantic, proof) = artifact(source);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let operation = &caller.blocks[0].operations[0];
    let result = operation.result.structural().unwrap();
    assert_ne!(result.place, caller.structural_parameters[0].place);
    for candidate in &plan.provider_candidates {
        let selected = [SelectedProviderAdapter {
            requirement_identity: candidate.requirement_identity.clone(),
            provider_identity: candidate.provider_identity.clone(),
            machine_identity: candidate.candidate_identity.clone(),
        }];
        let installation =
            admit_provider_installation(&plan, &semantic, &proof, &profile, &selected)
                .expect("the installed provider retains its affine result occurrence");
        let [call] = installation.installed_calls() else {
            panic!("one installed call")
        };
        assert_eq!(call.caller(), caller.id);
        assert_eq!(call.psi_operation(), operation.id);
        assert_eq!(call.result(), &operation.result);
        assert_eq!(call.provider(), candidate);
        assert_ne!(candidate.candidate, caller.id);
        assert!(call.completion_claim_sources().is_empty());
        assert!(call.completion_receipts().is_empty());
        let evidence =
            installation_evidence::ProviderInstallationEvidence::installed_provider_calls(
                &installation,
            );
        assert_eq!(evidence[0].result, operation.result);
        let optimized =
            lower_artifact_sections_for_optimization(&semantic, &proof, &profile).unwrap();
        let optimized_installation = admit_provider_installation_for_optimization(
            optimized.plan(),
            &semantic,
            &proof,
            &profile,
            &selected,
        )
        .unwrap();
        assert_eq!(
            optimized_installation.installed_calls(),
            installation.installed_calls()
        );
        let mut reference_usage = None;
        for incremental in [false, true] {
            let input = TerminalStructuralValue {
                opaque_identity: 123,
                structural_type: caller.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            };
            let mut execution = TerminalExecution::start_artifact_with_provider_installation(
                &semantic,
                &proof,
                &profile,
                &[],
                &[input],
                installation.psi_installation(),
            )
            .unwrap();
            let mut fuel = if incremental {
                TerminalFuelMeter::with_allowance(0)
            } else {
                TerminalFuelMeter::unbounded()
            };
            let mut complete = false;
            for _ in 0..16 {
                match execution.resume(&mut fuel).unwrap() {
                    TerminalExecutionStatus::SponsorExhausted(_) => {
                        assert!(incremental);
                        fuel.replenish(1).unwrap();
                    }
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                        complete = true;
                        break;
                    }
                    status => panic!("unexpected {status:?}"),
                }
            }
            assert!(complete);
            assert!(
                execution.effects().is_empty(),
                "the selected provider runs without host fallback"
            );
            assert!(execution.live_affine_frontier().next().is_none());
            assert_eq!(fuel.usage().total_units(), 5);
            if let Some(expected) = &reference_usage {
                assert_eq!(fuel.usage(), expected);
            } else {
                reference_usage = Some(fuel.usage().clone());
            }
        }
    }
}

#[test]
fn installed_affine_result_custody_rejects_changed_plan_and_selection() {
    use abstract_operations::{AbstractBoundaryResult, AbstractFunctionResult, AbstractOperation};
    use terminal_psi_to_abstract_operations::ProviderInstallationError;

    let (_, semantic, proof) = artifact(SOURCE);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).unwrap();
    let candidate = &plan.provider_candidates[1];
    let selected = [SelectedProviderAdapter {
        requirement_identity: candidate.requirement_identity.clone(),
        provider_identity: candidate.provider_identity.clone(),
        machine_identity: candidate.candidate_identity.clone(),
    }];
    admit_provider_installation(&plan, &semantic, &proof, &profile, &selected).unwrap();
    for mutation in 0..7 {
        let mut changed = plan.clone();
        if mutation < 4 {
            let caller = changed
                .functions
                .iter_mut()
                .find(|function| function.machine == changed.entry)
                .unwrap();
            let input = caller.structural_parameters[0].place;
            let AbstractOperation::BoundaryCall { result, .. } = &mut caller.operations[0] else {
                panic!("boundary producer")
            };
            if mutation == 0 {
                *result = AbstractBoundaryResult::Unit;
            } else {
                let AbstractBoundaryResult::Structural(result) = result else {
                    panic!("structural result")
                };
                match mutation {
                    1 => result.place = input,
                    2 => result.multiplicity = terminal_psi::StructuralMultiplicity::Linear,
                    3 => {
                        result.structural_type = changed
                            .structural_types
                            .iter()
                            .find(|declaration| declaration.id != result.structural_type)
                            .unwrap()
                            .id
                    }
                    _ => unreachable!(),
                }
            }
        } else if mutation == 4 {
            changed
                .functions
                .iter_mut()
                .find(|function| function.machine == candidate.candidate)
                .unwrap()
                .result = AbstractFunctionResult::Unit;
        } else if mutation == 5 {
            changed.boundary_machines[0].result = terminal_psi::BoundaryMachineResult::Unit;
        } else {
            changed.provider_candidates[1]
                .candidate_identity
                .push_str("::substituted");
        }
        assert!(
            matches!(
                admit_provider_installation(&changed, &semantic, &proof, &profile, &selected),
                Err(ProviderInstallationError::PlanReplayMismatch)
            ),
            "mutation={mutation}"
        );
    }
    assert!(matches!(
        admit_provider_installation(&plan, &semantic, &proof, &profile, &[]),
        Err(ProviderInstallationError::MissingSelectedProvider { .. })
    ));
    let mut wrong = selected.clone();
    wrong[0].machine_identity.push_str("::substituted");
    assert!(matches!(
        admit_provider_installation(&plan, &semantic, &proof, &profile, &wrong),
        Err(ProviderInstallationError::SelectedProviderMismatch { .. })
    ));
}
