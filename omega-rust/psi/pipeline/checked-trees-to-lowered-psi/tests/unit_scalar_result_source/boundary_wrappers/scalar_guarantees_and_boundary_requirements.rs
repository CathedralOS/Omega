use super::{
    artifact, assert_exact_normal_return_evidence, boolean_guarantee_source, execute, integer,
    normal_guarantee_source, ordered_contract_source, source,
};
use crate::unit_scalar_result_source::{CheckedScalarExpression, checked_from_source};
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue};

#[test]
fn ordered_scalar_guarantees_require_return_evidence_and_exact_return_value() {
    assert_exact_normal_return_evidence(&normal_guarantee_source(
        "let observed: i32 = Host::measure(11); value",
    ));
    assert_exact_normal_return_evidence(&boolean_guarantee_source(
        "let observed: bool = Host::measure(false); value",
        true,
    ));
}

#[test]
fn ordered_boolean_guarantees_keep_mixed_scalar_slots_and_source_identity() {
    use checked_trees::{CheckedBooleanExpression as Boolean, ClosedScalarContractValue as Clause};
    for input in [false, true] {
        let source = boolean_guarantee_source("Host::finish(false); value", input)
            .replace(
                "Scalar::measure(value: bool)",
                "Scalar::measure(marker: u16, value: bool, spare: bool)",
            )
            .replace(
                &format!("Scalar::measure({input})"),
                &format!("Scalar::measure(9u16, {input}, {})", !input),
            );
        let original = checked_from_source(&source);
        let published = artifact(&original);
        let (status, observed) = execute(&published);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            observed.arguments.last(),
            Some(&vec![TerminalScalarValue::Boolean(input)])
        );
        let target = original
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Scalar::measure")
            .unwrap()
            .symbol;
        for mutation in 0..6 {
            let mut changed = original.clone();
            let contract = changed
                .facts
                .contract_plans
                .machines
                .iter_mut()
                .find(|contract| contract.machine == target)
                .unwrap();
            let mut guarantees = contract.closed_scalar_values.ensures().to_vec();
            match mutation {
                0 => guarantees.clear(),
                1 => guarantees.push(guarantees[0].clone()),
                2 => guarantees[0] = Some(Clause::Boolean(true)),
                3..=5 => {
                    let Some(Clause::Predicate(Boolean::Equal { left, .. })) = &mut guarantees[0]
                    else {
                        panic!("Boolean equality")
                    };
                    assert_eq!(**left, Boolean::Parameter { position: 3 });
                    **left = match mutation {
                        3 => Boolean::Parameter { position: 2 },
                        4 => Boolean::Local { position: 3 },
                        5 => Boolean::Parameter { position: 0 },
                        _ => unreachable!(),
                    };
                }
                _ => unreachable!(),
            }
            contract.closed_scalar_values = checked_trees::ClosedScalarValueContractPlan::new(
                contract.closed_scalar_values.requires().to_vec(),
                guarantees,
                contract.closed_scalar_values.has_crash_clauses(),
                contract.closed_scalar_values.has_outcome_specific_clauses(),
            );
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn ordered_scalar_guarantees_reject_changed_source_predicates() {
    use checked_trees::{CheckedBooleanExpression as Boolean, ClosedScalarContractValue as Clause};
    let original = checked_from_source(&normal_guarantee_source("Host::finish(11); value"));
    artifact(&original);
    let target = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .unwrap()
        .symbol;
    for mutation in 0..7 {
        let mut changed = original.clone();
        let contract = changed
            .facts
            .contract_plans
            .machines
            .iter_mut()
            .find(|contract| contract.machine == target)
            .unwrap();
        let mut guarantees = contract.closed_scalar_values.ensures().to_vec();
        match mutation {
            0 => guarantees.clear(),
            1 => guarantees.push(guarantees[0].clone()),
            2 => guarantees[0] = None,
            3 => guarantees[0] = Some(Clause::Boolean(true)),
            4..=6 => {
                let Some(Clause::Predicate(Boolean::IntegerComparison { kind, left, .. })) =
                    &mut guarantees[0]
                else {
                    panic!("integer guarantee")
                };
                match mutation {
                    4 => {
                        **left = CheckedScalarExpression::Parameter {
                            position: 0,
                            primitive_type: typed_trees::types::PrimitiveType::I32,
                        }
                    }
                    5 => {
                        **left = CheckedScalarExpression::Local {
                            position: 1,
                            primitive_type: typed_trees::types::PrimitiveType::I32,
                        }
                    }
                    6 => *kind = checked_trees::CheckedIntegerComparisonKind::LessThan,
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
        contract.closed_scalar_values = checked_trees::ClosedScalarValueContractPlan::new(
            contract.closed_scalar_values.requires().to_vec(),
            guarantees,
            contract.closed_scalar_values.has_crash_clauses(),
            contract.closed_scalar_values.has_outcome_specific_clauses(),
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordered_boundary_return_preserves_entry_predicates() {
    let source = source()
        .replace(
            "Scalar::measure() -> i32 reaches Host",
            "Scalar::measure(value: i32) -> i32\nrequires value >= 1\nreaches Host",
        )
        .replace(
            "let result: i32 = Host::measure(70);\n            result",
            "Host::finish(11); Host::measure(value)",
        )
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let artifact = artifact(&checked_from_source(&source));
    let module = decode_module(&artifact.0).unwrap();
    let wrapper = module
        .machines
        .iter()
        .find(|machine| !machine.parameters.is_empty())
        .unwrap();
    assert_eq!(wrapper.contract.requires.len(), 1);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        observed.arguments,
        [vec![integer(11)], vec![integer(70)], vec![integer(70)]]
    );
}

#[test]
fn ordered_boundary_return_preserves_boolean_entry_predicates() {
    let source = source()
        .replace("Scalar::measure() -> i32 reaches Host", "Scalar::measure(value: i32, allowed: bool, denied: bool) -> i32\nrequires allowed && !denied\nreaches Host")
        .replace("let result: i32 = Host::measure(70);\n            result", "Host::finish(11); Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70, true, false);");
    let artifact = artifact(&checked_from_source(&source));
    let module = decode_module(&artifact.0).unwrap();
    assert!(
        !module
            .machines
            .iter()
            .find(|machine| machine.parameters.len() == 3)
            .unwrap()
            .contract
            .requires
            .is_empty()
    );
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        observed.arguments,
        [vec![integer(11)], vec![integer(70)], vec![integer(70)]]
    );
}

#[test]
fn ordered_boundary_requirements_keep_clause_slots_and_call_proofs() {
    for body in [
        "Host::finish(other); Host::measure(value)",
        "Host::finish(other); let result: i32 = Host::measure(value); result",
        "let saved: i32 = value; Host::finish(other); Host::measure(saved)",
    ] {
        let source =
            ordered_contract_source().replace("Host::finish(other); Host::measure(value)", body);
        let artifact = artifact(&checked_from_source(&source));
        let module = decode_module(&artifact.0).unwrap();
        let wrapper = module
            .machines
            .iter()
            .find(|machine| machine.parameters.len() == 2)
            .unwrap();
        // Source duplicates normalize before publication. The merged authored
        // clause publishes beside the three runtime requirement rows; every
        // published requirement still owns a separate call proof slot.
        assert_eq!(wrapper.contract.requires.len(), 4);
        let obligations = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::CallStructuralScalar {
                    callee,
                    requirement_obligations,
                    ..
                } if *callee == wrapper.id => Some(requirement_obligations),
                _ => None,
            })
            .unwrap();
        assert_eq!(obligations.len(), 4);
        for obligation in obligations {
            let mut proof = decode_proof_bundle(&artifact.1).unwrap();
            let count = proof.evidence.len();
            proof
                .evidence
                .retain(|evidence| evidence.obligation != *obligation);
            assert_eq!(proof.evidence.len() + 1, count);
            assert!(
                terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
                    .is_err()
            );
        }
        let (status, observed) = execute(&artifact);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            observed.arguments,
            [vec![integer(11)], vec![integer(70)], vec![integer(70)]]
        );
    }
}

#[test]
fn ordered_boundary_requirements_reject_source_predicate_substitution() {
    use checked_trees::{CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind};
    let original = checked_from_source(&ordered_contract_source());
    artifact(&original);
    let target = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .unwrap()
        .symbol;
    for mutation in 0..10 {
        let mut changed = original.clone();
        let contract = changed
            .facts
            .contract_plans
            .machines
            .iter_mut()
            .find(|contract| contract.machine == target)
            .unwrap();
        let mut requirements = contract
            .crash
            .structural_runtime_requirements()
            .unwrap()
            .to_vec();
        match mutation {
            0 => {
                requirements.remove(0);
            }
            1 => requirements.push(requirements[0].clone()),
            2 => requirements.swap(0, 1),
            3 => requirements[0] = Boolean::Constant(true),
            4 => {
                let Boolean::IntegerComparison { kind, .. } = &mut requirements[0] else {
                    panic!("comparison");
                };
                *kind = CheckedIntegerComparisonKind::LessThan;
            }
            5 | 6 => {
                let Boolean::IntegerComparison { right, .. } = &mut requirements[0] else {
                    panic!("comparison");
                };
                **right = if mutation == 5 {
                    CheckedScalarExpression::Parameter {
                        position: 1,
                        primitive_type: checked_trees::types::PrimitiveType::I32,
                    }
                } else {
                    CheckedScalarExpression::Local {
                        position: 0,
                        primitive_type: checked_trees::types::PrimitiveType::I32,
                    }
                };
            }
            7 | 8 => {
                let Boolean::And { left, right } = &mut requirements[3] else {
                    panic!("range");
                };
                let Boolean::IntegerComparison {
                    left: minimum,
                    right: subject,
                    ..
                } = left.as_mut()
                else {
                    panic!("lower bound");
                };
                if mutation == 7 {
                    **subject = CheckedScalarExpression::Parameter {
                        position: 1,
                        primitive_type: checked_trees::types::PrimitiveType::I32,
                    };
                } else {
                    let Boolean::IntegerComparison { right: maximum, .. } = right.as_ref() else {
                        panic!("upper bound");
                    };
                    *minimum = maximum.clone();
                }
            }
            _ => {}
        }
        contract.crash = contract
            .crash
            .clone()
            .with_structural_runtime_requirements((mutation != 9).then_some(requirements));
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}
