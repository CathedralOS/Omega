use super::{artifact, execute, integer, normal_guarantee_source, ordered_contract_source, source};
use crate::unit_scalar_result_source::CheckedScalarExpression;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_codec::decode_module;
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

#[test]
fn ordered_scalar_guarantees_reject_changed_source_predicates() {
    use checked_trees::{CheckedBooleanExpression as Boolean, ClosedScalarContractValue as Clause};
    let original =
        crate::front_end::checked_program(&normal_guarantee_source("Host::finish(11); value"));
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
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Main::main")
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordered_boundary_return_preserves_boolean_entry_predicates() {
    let source = source()
        .replace("Scalar::measure() -> i32 reaches Host", "Scalar::measure(value: i32, allowed: bool, denied: bool) -> i32\nrequires allowed && !denied\nreaches Host")
        .replace("let result: i32 = Host::measure(70);\n            result", "Host::finish(11); Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70, true, false);");
    let artifact = artifact(&crate::front_end::checked_program(&source));
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
fn ordered_boundary_requirements_reject_source_predicate_substitution() {
    use checked_trees::{CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind};
    let original = crate::front_end::checked_program(&ordered_contract_source());
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
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Main::main")
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}
