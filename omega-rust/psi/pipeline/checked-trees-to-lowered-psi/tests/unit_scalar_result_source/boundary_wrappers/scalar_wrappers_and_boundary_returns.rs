use super::{artifact, execute, integer, source};
use crate::unit_scalar_result_source::{CheckedUnitEffectOperationPlan, checked_from_source};
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

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
fn direct_boundary_return_composes_with_ordered_calls_and_locals() {
    for (body, expected) in [
        ("Host::measure(70)", &[70, 70][..]),
        ("let input: i32 = 70; Host::measure(input)", &[70, 70][..]),
        ("Host::finish(11); Host::measure(70)", &[11, 70, 70][..]),
        (
            "let first: i32 = Host::measure(11); Host::finish(first); Host::measure(70)",
            &[11, 11, 70, 70][..],
        ),
        ("Host::measure(identity(70))", &[70, 70][..]),
    ] {
        let source = format!(
            "machine identity(value: i32) -> i32 {{ value }}\n{}",
            source().replace(
                "let result: i32 = Host::measure(70);\n            result",
                body
            )
        );
        let checked = checked_from_source(&source);
        let artifact = artifact(&checked);
        let (status, observed) = execute(&artifact);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        let expected = expected
            .iter()
            .map(|value| vec![integer(*value)])
            .collect::<Vec<_>>();
        assert_eq!(observed.arguments, expected, "{body}");
    }
}

#[test]
fn direct_boundary_return_rejects_result_and_occurrence_substitution() {
    let source = source().replace(
        "let result: i32 = Host::measure(70);\n            result",
        "let prior: i32 = Host::measure(11); Host::measure(70)",
    );
    let original = checked_from_source(&source);
    let target = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .unwrap()
        .symbol;
    artifact(&original);
    for mutation in 0..4 {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == target)
            .unwrap();
        let first = plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. } => Some(*result),
                _ => None,
            })
            .unwrap();
        match mutation {
            0 => plan.scalar_result = Some(first),
            1 => plan.operations.insert(1, plan.operations[0].clone()),
            2 => {
                let earlier_site = match &plan.operations[0] {
                    CheckedUnitEffectOperationPlan::BoundaryScalarCall { source_site, .. } => {
                        *source_site
                    }
                    _ => unreachable!(),
                };
                let CheckedUnitEffectOperationPlan::BoundaryScalarCall { source_site, .. } =
                    &mut plan.operations[1]
                else {
                    unreachable!()
                };
                *source_site = earlier_site;
            }
            _ => {
                let CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. } =
                    &mut plan.operations[1]
                else {
                    unreachable!()
                };
                result.binding_ordinal = first.binding_ordinal;
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "mutation {mutation} must not substitute another same-typed result"
        );
    }
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
