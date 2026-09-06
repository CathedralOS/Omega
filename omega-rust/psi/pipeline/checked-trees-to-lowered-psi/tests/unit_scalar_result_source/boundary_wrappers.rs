use super::*;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
};

fn source() -> String {
    SOURCE
        .replace("Host::measure(70)", "Scalar::measure()")
        .replace(
            "data Main {}",
            r#"
        data Scalar {}
        machine Scalar::measure() -> i32 reaches Host {
            let result: i32 = Host::measure(70);
            result
        }
        data Main {}
        "#,
        )
}

fn artifact(checked: &checked_trees::CheckedTrees) -> (Vec<u8>, Vec<u8>) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("Unit closure retains scalar boundary body");
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof = decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let published = terminal_production::produce_terminal_artifact(checked, "Main::main")
        .expect("source-owned shared closure publishes");
    assert_eq!(decode_module(published.semantic_bytes()).unwrap(), module);
    (semantic, evidence)
}

fn integer(value: i128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Signed,
            32,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Signed(value),
    }
}

#[derive(Default)]
struct Observe {
    arguments: Vec<Vec<TerminalScalarValue>>,
}

impl TerminalEffectHandler for Observe {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("scalar boundaries require the result-bearing handler")
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            result,
            structural_arguments,
            ..
        } = effect
        else {
            panic!("expected boundary call")
        };
        assert!(structural_arguments.is_empty());
        self.arguments.push(arguments.clone());
        Ok(match result {
            terminal_psi::BoundaryMachineResult::Scalar(_) => {
                TerminalEffectResult::Scalar(arguments[0])
            }
            terminal_psi::BoundaryMachineResult::Unit => TerminalEffectResult::Unit,
            _ => panic!("no structural return in this scalar call lane"),
        })
    }
}

fn execute(artifact: &(Vec<u8>, Vec<u8>)) -> (TerminalExecutionStatus, Observe) {
    let module = decode_module(&artifact.0).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameters = entry
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
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
        &parameters,
    )
    .unwrap();
    let mut observer = Observe::default();
    let status = execution
        .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
        .unwrap();
    (status, observer)
}

#[test]
fn scalar_wrapper_result_executes_and_publishes_with_exact_services() {
    let artifact = artifact(&checked_from_source(&source()));
    let module = decode_module(&artifact.0).unwrap();
    assert_eq!(module.machines.len(), 2);
    assert_eq!(module.boundary_machines.len(), 2);
    assert_eq!(module.services.len(), 1);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, [vec![integer(70)], vec![integer(70)]]);
}

#[test]
fn scalar_wrapper_parameters_forward_through_named_and_unit_entries() {
    let source = source()
        .replace("Scalar::measure() ->", "Scalar::measure(value: i32) ->")
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let checked = checked_from_source(&source);
    let named = checked_trees_to_lowered_psi::lower_machine(&checked, "Scalar::measure")
        .expect("a boundary-returning body retains its scalar entry parameter");
    assert_eq!(named.semantic_module.machines[0].parameters.len(), 1);
    let artifact = artifact(&checked);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, [vec![integer(70)], vec![integer(70)]]);
}

#[test]
fn scalar_wrapper_parameter_ranges_survive_call_proofs_and_publication() {
    let source = source()
        .replace(
            "Scalar::measure() ->",
            "Scalar::measure(value: i32 [1..=100]) ->",
        )
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let checked = checked_from_source(&source);
    let artifact = artifact(&checked);
    let module = decode_module(&artifact.0).unwrap();
    let wrapper = module
        .machines
        .iter()
        .find(|machine| !machine.parameters.is_empty())
        .unwrap();
    assert_eq!(wrapper.contract.requires.len(), 1);
    assert!(!matches!(
        wrapper.contract.requires[0],
        semantic_vocabulary::Proposition::Truth
    ));
    let call_obligation = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::Call {
                callee,
                requirement_obligations,
                ..
            } if *callee == wrapper.id => {
                assert_eq!(requirement_obligations.len(), 1);
                Some(requirement_obligations[0])
            }
            _ => None,
        })
        .unwrap();
    let mut missing_call_proof = decode_proof_bundle(&artifact.1).unwrap();
    let original_count = missing_call_proof.evidence.len();
    missing_call_proof
        .evidence
        .retain(|evidence| evidence.obligation != call_obligation);
    assert_eq!(missing_call_proof.evidence.len(), original_count - 1);
    assert!(
        terminal_verifier::verify_module(
            &module,
            &missing_call_proof,
            &AdmissionProfile::default()
        )
        .is_err()
    );
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, [vec![integer(70)], vec![integer(70)]]);
}

#[test]
fn scalar_wrapper_explicit_entry_predicate_survives_call_proofs() {
    let source = source()
        .replace(
            "Scalar::measure() -> i32 reaches Host",
            "Scalar::measure(value: i32) -> i32\nrequires value >= 1\nreaches Host",
        )
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let checked = checked_from_source(&source);
    let artifact = artifact(&checked);
    let module = decode_module(&artifact.0).unwrap();
    assert_eq!(
        module
            .machines
            .iter()
            .find(|machine| !machine.parameters.is_empty())
            .unwrap()
            .contract
            .requires
            .len(),
        1
    );
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, [vec![integer(70)], vec![integer(70)]]);
}

#[test]
fn parameterized_wrappers_nested_as_actuals_keep_one_body_and_ordered_effects() {
    let source = source()
        .replace("Scalar::measure() ->", "Scalar::measure(value: i32) ->")
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace(
            "Scalar::measure();",
            "Scalar::measure(Scalar::measure(70));",
        );
    let artifact = artifact(&checked_from_source(&source));
    assert_eq!(decode_module(&artifact.0).unwrap().machines.len(), 2);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        observed.arguments,
        [vec![integer(70)], vec![integer(70)], vec![integer(70)]]
    );
}

#[test]
fn scalar_wrapper_signature_and_parameter_range_custody_reject_mutations() {
    let source = source()
        .replace(
            "Scalar::measure() ->",
            "Scalar::measure(value: i32 [1..=100]) ->",
        )
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let original = checked_from_source(&source);
    for mutation in 0..4 {
        let mut checked = original.clone();
        let plan = &mut checked.facts.flow.terminal_boundary_scalar_returns.machines[0];
        match mutation {
            0 => plan.scalar_parameters.clear(),
            1 => plan.scalar_parameters[0].source_position = 1,
            2 => plan.scalar_parameters[0].primitive_type = typed_trees::types::PrimitiveType::Bool,
            3 => {
                let machine = plan.machine;
                let contract = checked
                    .facts
                    .contract_plans
                    .machines
                    .iter_mut()
                    .find(|contract| contract.machine == machine)
                    .unwrap();
                contract.closed_scalar_values = checked_trees::ClosedScalarValueContractPlan::new(
                    Vec::new(),
                    contract.closed_scalar_values.ensures().to_vec(),
                    contract.closed_scalar_values.has_crash_clauses(),
                    contract.closed_scalar_values.has_outcome_specific_clauses(),
                );
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Scalar::measure").is_err(),
            "mutation {mutation}"
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn repeated_wrappers_share_boundaries_and_nested_helper_identities() {
    let source = format!(
        "machine identity(value: i32) -> i32\nrequires 0i32 == 0i32\nensures 0i32 == 0i32\n{{ value }}\n{}",
        source().replace("Host::measure(70)", "Host::measure(identity(identity(70)))")
            .replace("Host::finish(result);", "let second: i32 = Scalar::measure();\n Host::finish(result);\n Host::finish(second);")
    );
    let artifact = artifact(&checked_from_source(&source));
    let module = decode_module(&artifact.0).unwrap();
    assert_eq!(
        module.machines.len(),
        3,
        "one helper, one wrapper, one caller"
    );
    assert_eq!(module.boundary_machines.len(), 2);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, vec![vec![integer(70)]; 4]);
}

#[test]
fn wrapper_boundary_and_direct_boundary_share_one_exact_declaration() {
    let source = source().replace(
        "Host::finish(result);",
        "let direct: i32 = Host::measure(21);\n Host::finish(result);\n Host::finish(direct);",
    );
    let artifact = artifact(&checked_from_source(&source));
    assert_eq!(
        decode_module(&artifact.0).unwrap().boundary_machines.len(),
        2
    );
    let (_, observed) = execute(&artifact);
    assert_eq!(
        observed.arguments,
        [
            vec![integer(70)],
            vec![integer(21)],
            vec![integer(70)],
            vec![integer(21)]
        ]
    );
}

#[test]
fn scalar_wrapper_registration_and_result_drift_reject() {
    for mutation in 0..4 {
        let mut checked = checked_from_source(&source());
        let plans = &mut checked.facts.flow.terminal_boundary_scalar_returns;
        match mutation {
            0 => plans.machines.push(plans.machines[0].clone()),
            1 => plans.machines[0].result_type = typed_trees::types::PrimitiveType::Bool,
            2 => plans.boundary_machines.clear(),
            3 => plans.machines[0].attachment_type_identity = "named(name(Main))".into(),
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn wrappers_nested_as_operands_share_the_complete_unit_catalog() {
    let source = source()
        .replace("Host::measure(70)", "Host::measure(Other::measure())")
        .replace(
            "data Main {}",
            r#"
            data Other {}
            machine Other::measure() -> i32 reaches Host {
                let result: i32 = Host::measure(70);
                result
            }
            data Main {}
        "#,
        )
        .replace(
            "let result: i32 = Scalar::measure();\n    Host::finish(result);",
            "Host::finish(Scalar::measure());",
        );
    let artifact = artifact(&checked_from_source(&source));
    let module = decode_module(&artifact.0).unwrap();
    assert_eq!(module.machines.len(), 3);
    assert_eq!(module.boundary_machines.len(), 2);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, vec![vec![integer(70)]; 3]);
}

#[test]
fn selected_wrapper_type_duplicates_and_cross_owner_conflicts_reject() {
    for duplicate in [false, true] {
        let mut checked = checked_from_source(&source());
        let declaration = checked
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .structural_types
            .iter()
            .find(|plan| plan.identity == "named(name(Scalar))")
            .unwrap()
            .clone();
        if duplicate {
            checked
                .facts
                .flow
                .terminal_boundary_scalar_returns
                .structural_types
                .push(declaration);
        } else {
            let mut conflicting = declaration;
            conflicting.shape = checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(
                typed_trees::types::PrimitiveType::Bool,
            );
            checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .push(conflicting);
        }
        assert!(checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").is_err());
    }
}

#[test]
fn equal_selected_type_declarations_coalesce_without_changing_module_bytes() {
    let mut checked = checked_from_source(&source());
    let original = artifact(&checked);
    let declaration = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .structural_types
        .iter()
        .find(|plan| plan.identity == "named(name(Scalar))")
        .unwrap()
        .clone();
    assert!(
        !checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .any(|plan| plan.identity == declaration.identity)
    );
    checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .push(declaration);
    assert_eq!(artifact(&checked), original);
}

#[test]
fn removing_wrapper_service_authority_rejects_independently() {
    let artifact = artifact(&checked_from_source(&source()));
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    for root in [false, true] {
        let mut module = decode_module(&artifact.0).unwrap();
        if root {
            module.root_service_reach.concrete.clear();
        } else {
            module
                .machines
                .iter_mut()
                .find(|machine| machine.id != module.entry)
                .unwrap()
                .published_service_ceiling
                .clear();
        }
        assert!(
            terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
                .is_err()
        );
    }
}

#[test]
fn wrapper_operand_crash_preserves_call_ceiling_and_prevents_boundary_effects() {
    let source = format!(
        "machine abort() -> i32 crashes Abort {{ crash Abort; }}\n{}",
        source()
            .replace("Host::measure(70)", "Host::measure(abort())")
            .replace("reaches Host {", "reaches Host crashes Abort {")
            .replace("reaches Host\n{", "reaches Host\ncrashes Abort\n{")
    );
    let artifact = artifact(&checked_from_source(&source));
    let (status, observed) = execute(&artifact);
    let TerminalExecutionStatus::Crashed(crash) = status else {
        panic!("the nested helper must crash before invoking the boundary")
    };
    assert_eq!(crash.cause, terminal_psi::CrashCause::Abort);
    assert!(observed.arguments.is_empty());
    let mut module = decode_module(&artifact.0).unwrap();
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    for operation in entry
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
    {
        if let terminal_psi::OperationKind::Call {
            crash_continuations,
            ..
        } = &mut operation.kind
        {
            crash_continuations.clear();
        }
    }
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );
}
