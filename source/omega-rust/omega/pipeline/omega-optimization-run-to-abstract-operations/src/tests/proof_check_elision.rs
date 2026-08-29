//! Proof-elision projection custody.

use super::*;

#[test]
fn proof_check_elision_projects_dead_exact_work_and_retains_evidence() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let optimized = project_optimization_run(run(dead_exact_add_verified(), selections)).unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
    assert_eq!(optimized.pass_manifests().len(), 1);
    assert_eq!(optimized.plan().functions[0].operations.len(), 3);
    assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
    assert_eq!(
        optimized.pass_manifests()[0].decisions()[0]
            .consumed_facts()
            .len(),
        1
    );
    let terminal = &optimized.unit().functions[0].blocks[0].nodes[2];
    assert!(matches!(
        terminal.operation,
        AbstractOperation::ReturnUnit { .. }
    ));
    assert_eq!(terminal.provenance.len(), 2);
    assert_eq!(terminal.fuel.len(), 2);
}

#[test]
fn proof_check_elision_projects_live_exact_identity_with_fact_and_fuel_custody() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let optimized =
        project_optimization_run(run(live_exact_add_zero_verified(), selections)).unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
    assert_eq!(optimized.pass_manifests().len(), 1);
    assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
    assert_eq!(optimized.plan().functions[0].operations.len(), 2);
    assert!(matches!(
        optimized.plan().functions[0].operations[1],
        AbstractOperation::Return { value, .. }
            if value == ValueId::new(1_083).unwrap()
    ));
    assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
    assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
        !matches!(
            fact,
            omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
        )
    }));
    assert_eq!(
        optimized.pass_manifests()[0].decisions()[0]
            .consumed_facts()
            .len(),
        2
    );
    let terminal = &optimized.unit().functions[0].blocks[0].nodes[1];
    assert_eq!(terminal.provenance.len(), 2);
    assert_eq!(terminal.fuel.len(), 2);
}

#[test]
fn proof_check_elision_projects_and_lowers_live_exact_divide_by_one() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let optimized =
        project_optimization_run(run(live_exact_divide_by_one_verified(), selections)).unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
    assert_eq!(optimized.pass_manifests().len(), 1);
    assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
    assert_eq!(
        optimized.pass_manifests()[0].ordered_rules()[2],
        omega_optimization_core::OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1"
        )
    );
    assert_eq!(optimized.plan().functions[0].operations.len(), 2);
    assert!(matches!(
        optimized.plan().functions[0].operations[1],
        AbstractOperation::Return { value, .. }
            if value == ValueId::new(1_093).unwrap()
    ));
    assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
    assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
        !matches!(
            fact,
            omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
        )
    }));
    assert_eq!(
        optimized.pass_manifests()[0].decisions()[0]
            .consumed_facts()
            .len(),
        2
    );
    let terminal = &optimized.unit().functions[0].blocks[0].nodes[1];
    assert_eq!(terminal.provenance.len(), 2);
    assert_eq!(terminal.fuel.len(), 2);

    let lowered = lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64())
        .expect("the independently projected divide-free plan remains target lowerable");
    assert_eq!(lowered.target_operations().functions.len(), 1);
    assert_eq!(lowered.optimized().commits().len(), 1);
}

#[test]
fn proof_check_elision_projects_and_lowers_live_exact_multiply_by_zero() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let optimized =
        project_optimization_run(run(live_exact_multiply_by_zero_verified(), selections)).unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
    assert_eq!(optimized.pass_manifests().len(), 1);
    assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
    assert_eq!(
        optimized.pass_manifests()[0].ordered_rules()[3],
        omega_optimization_core::OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1"
        )
    );
    assert_eq!(optimized.plan().functions[0].operations.len(), 2);
    assert!(matches!(
        optimized.plan().functions[0].operations[1],
        AbstractOperation::Return { value, .. }
            if value == ValueId::new(1_104).unwrap()
    ));
    assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
    assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
        !matches!(
            fact,
            omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
        )
    }));
    assert_eq!(
        optimized.pass_manifests()[0].decisions()[0]
            .consumed_facts()
            .len(),
        2
    );
    let terminal = &optimized.unit().functions[0].blocks[0].nodes[1];
    assert_eq!(terminal.provenance.len(), 2);
    assert_eq!(terminal.fuel.len(), 2);

    let lowered = lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64())
        .expect("the independently projected zero-product-free plan remains target lowerable");
    assert_eq!(lowered.target_operations().functions.len(), 1);
    assert_eq!(lowered.optimized().commits().len(), 1);
}

#[test]
fn proof_check_elision_projects_and_lowers_live_exact_zero_dividend() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized =
            project_optimization_run(run(live_exact_zero_dividend_verified(), selections.clone()))
                .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
        assert_eq!(
            optimized.pass_manifests()[0].ordered_rules()[4],
            omega_optimization_core::OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1"
            )
        );
        assert_eq!(optimized.plan().functions[0].operations.len(), 3);
        assert!(matches!(
            optimized.plan().functions[0].operations[2],
            AbstractOperation::Return { value, .. }
                if value == ValueId::new(1_113).unwrap()
        ));
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            )
        }));
        assert_eq!(
            optimized.pass_manifests()[0].decisions()[0]
                .consumed_facts()
                .len(),
            2
        );
        let terminal = &optimized.unit().functions[0].blocks[0].nodes[2];
        assert_eq!(terminal.provenance.len(), 2);
        assert_eq!(terminal.fuel.len(), 2);

        let lowered = lower_optimized_to_target_operations(optimized, target)
            .expect("the independently projected zero-dividend-free plan remains lowerable");
        assert_eq!(lowered.target(), target);
        assert_eq!(lowered.target_operations().functions.len(), 1);
        assert_eq!(lowered.optimized().commits().len(), 1);
    }
}

#[test]
fn proof_check_elision_projects_and_lowers_live_exact_zero_value_shift() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized = project_optimization_run(run(
            live_exact_zero_value_shift_verified(),
            selections.clone(),
        ))
        .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
        assert_eq!(
                optimized.pass_manifests()[0].ordered_rules()[5],
                omega_optimization_core::OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.live-proof-certified-exact-integer-zero-value-shift-elimination.v1"
                )
            );
        assert_eq!(optimized.plan().functions[0].operations.len(), 3);
        assert!(matches!(
            optimized.plan().functions[0].operations[2],
            AbstractOperation::Return { value, .. }
                if value == ValueId::new(1_124).unwrap()
        ));
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            )
        }));
        assert_eq!(
            optimized.pass_manifests()[0].decisions()[0]
                .consumed_facts()
                .len(),
            2
        );
        let terminal = &optimized.unit().functions[0].blocks[0].nodes[2];
        assert_eq!(terminal.provenance.len(), 2);
        assert_eq!(terminal.fuel.len(), 2);

        let lowered = lower_optimized_to_target_operations(optimized, target)
            .expect("the independently projected zero-value-shift-free plan remains lowerable");
        assert_eq!(lowered.target(), target);
        assert_eq!(lowered.target_operations().functions.len(), 1);
        assert_eq!(lowered.optimized().commits().len(), 1);
    }
}

#[test]
fn proof_check_elision_projects_and_lowers_live_exact_self_subtract() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized =
            project_optimization_run(run(live_exact_self_subtract_verified(), selections.clone()))
                .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
        assert_eq!(
            optimized.pass_manifests()[0].ordered_rules()[6],
            omega_optimization_core::OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1"
            )
        );
        assert_eq!(optimized.plan().functions[0].operations.len(), 2);
        assert!(matches!(
            optimized.plan().functions[0].operations[0],
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                value: IntegerValue::Unsigned(0),
                ..
            } if psi_operation == OperationId::new(1_139).unwrap()
                && result == ValueId::new(1_136).unwrap()
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[1],
            AbstractOperation::Return { value, .. }
                if value == ValueId::new(1_136).unwrap()
        ));
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            )
        }));
        assert!(optimized.unit().functions[0].facts.iter().any(|fact| {
            matches!(
                fact,
                omega_optimization_unit::OptimizationFact::IntegerConstant {
                    value,
                    constant: IntegerValue::Unsigned(0),
                    support,
                } if *value == ValueId::new(1_136).unwrap()
                    && *support == OperationId::new(1_139).unwrap()
            )
        }));
        assert_eq!(
            optimized.pass_manifests()[0].decisions()[0]
                .consumed_facts()
                .len(),
            1
        );
        let constant = &optimized.unit().functions[0].blocks[0].nodes[0];
        assert_eq!(constant.provenance.len(), 1);
        assert_eq!(constant.fuel.len(), 1);

        let lowered = lower_optimized_to_target_operations(optimized, target)
            .expect("the independently projected self-subtract zero remains lowerable");
        assert_eq!(lowered.target(), target);
        assert_eq!(lowered.target_operations().functions.len(), 1);
        assert_eq!(lowered.optimized().commits().len(), 1);
    }
}

#[test]
fn proof_check_elision_projects_and_lowers_live_exact_self_remainder() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized = project_optimization_run(run(
            live_exact_self_remainder_verified(),
            selections.clone(),
        ))
        .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
        assert_eq!(
            optimized.pass_manifests()[0].ordered_rules()[7],
            omega_optimization_core::OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1"
            )
        );
        assert_eq!(optimized.plan().functions[0].operations.len(), 2);
        assert!(matches!(
            optimized.plan().functions[0].operations[0],
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                value: IntegerValue::Unsigned(0),
                ..
            } if psi_operation == OperationId::new(1_148).unwrap()
                && result == ValueId::new(1_145).unwrap()
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[1],
            AbstractOperation::Return { value, .. }
                if value == ValueId::new(1_145).unwrap()
        ));
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            )
        }));
        assert!(optimized.unit().functions[0].facts.iter().any(|fact| {
            matches!(
                fact,
                omega_optimization_unit::OptimizationFact::IntegerConstant {
                    value,
                    constant: IntegerValue::Unsigned(0),
                    support,
                } if *value == ValueId::new(1_145).unwrap()
                    && *support == OperationId::new(1_148).unwrap()
            )
        }));
        assert_eq!(
            optimized.pass_manifests()[0].decisions()[0]
                .consumed_facts()
                .len(),
            1
        );
        let constant = &optimized.unit().functions[0].blocks[0].nodes[0];
        assert_eq!(constant.provenance.len(), 1);
        assert_eq!(constant.fuel.len(), 1);

        let lowered = lower_optimized_to_target_operations(optimized, target)
            .expect("the independently projected self-remainder zero remains lowerable");
        assert_eq!(lowered.target(), target);
        assert_eq!(lowered.target_operations().functions.len(), 1);
        assert_eq!(lowered.optimized().commits().len(), 1);
    }
}

#[test]
fn proof_check_elision_projects_and_lowers_live_exact_self_divide() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized =
            project_optimization_run(run(live_exact_self_divide_verified(), selections.clone()))
                .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
        assert_eq!(
            optimized.pass_manifests()[0].ordered_rules()[8],
            omega_optimization_core::OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1"
            )
        );
        assert_eq!(optimized.plan().functions[0].operations.len(), 2);
        assert!(matches!(
            optimized.plan().functions[0].operations[0],
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                value: IntegerValue::Unsigned(1),
                ..
            } if psi_operation == OperationId::new(1_148).unwrap()
                && result == ValueId::new(1_145).unwrap()
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[1],
            AbstractOperation::Return { value, .. }
                if value == ValueId::new(1_145).unwrap()
        ));
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            )
        }));
        assert!(optimized.unit().functions[0].facts.iter().any(|fact| {
            matches!(
                fact,
                omega_optimization_unit::OptimizationFact::IntegerConstant {
                    value,
                    constant: IntegerValue::Unsigned(1),
                    support,
                } if *value == ValueId::new(1_145).unwrap()
                    && *support == OperationId::new(1_148).unwrap()
            )
        }));
        assert_eq!(
            optimized.pass_manifests()[0].decisions()[0]
                .consumed_facts()
                .len(),
            1
        );
        let constant = &optimized.unit().functions[0].blocks[0].nodes[0];
        assert_eq!(constant.provenance.len(), 1);
        assert_eq!(constant.fuel.len(), 1);

        let lowered = lower_optimized_to_target_operations(optimized, target)
            .expect("the independently projected self-divide one remains lowerable");
        assert_eq!(lowered.target(), target);
        assert_eq!(lowered.target_operations().functions.len(), 1);
        assert_eq!(lowered.optimized().commits().len(), 1);
    }
}

#[test]
fn proof_check_elision_projects_and_lowers_live_exact_remainder_by_one() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized = project_optimization_run(run(
            live_exact_remainder_by_one_verified(),
            selections.clone(),
        ))
        .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
        assert_eq!(
            optimized.pass_manifests()[0].ordered_rules()[9],
            omega_optimization_core::OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1"
            )
        );
        assert_eq!(optimized.plan().functions[0].operations.len(), 3);
        assert!(matches!(
            optimized.plan().functions[0].operations[0],
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                value: IntegerValue::Unsigned(1),
                ..
            } if psi_operation == OperationId::new(1_158).unwrap()
                && result == ValueId::new(1_154).unwrap()
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[1],
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                value: IntegerValue::Unsigned(0),
                ..
            } if psi_operation == OperationId::new(1_159).unwrap()
                && result == ValueId::new(1_155).unwrap()
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[2],
            AbstractOperation::Return { value, .. }
                if value == ValueId::new(1_155).unwrap()
        ));
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            )
        }));
        assert!(optimized.unit().functions[0].facts.iter().any(|fact| {
            matches!(
                fact,
                omega_optimization_unit::OptimizationFact::IntegerConstant {
                    value,
                    constant: IntegerValue::Unsigned(0),
                    support,
                } if *value == ValueId::new(1_155).unwrap()
                    && *support == OperationId::new(1_159).unwrap()
            )
        }));
        assert_eq!(
            optimized.pass_manifests()[0].decisions()[0]
                .consumed_facts()
                .len(),
            2
        );
        let constant = &optimized.unit().functions[0].blocks[0].nodes[1];
        assert_eq!(constant.provenance.len(), 1);
        assert_eq!(constant.fuel.len(), 1);

        let lowered = lower_optimized_to_target_operations(optimized, target)
            .expect("the independently projected remainder-by-one zero remains lowerable");
        assert_eq!(lowered.target(), target);
        assert_eq!(lowered.target_operations().functions.len(), 1);
        assert_eq!(lowered.optimized().commits().len(), 1);
    }
}

#[test]
fn proof_check_elision_projects_signed_remainder_by_negative_one_to_both_targets() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized = project_optimization_run(run(
            live_exact_signed_remainder_by_negative_one_verified(),
            selections.clone(),
        ))
        .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
        assert_eq!(
                optimized.pass_manifests()[0].ordered_rules()[10],
                omega_optimization_core::OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1"
                )
            );
        assert_eq!(optimized.plan().functions[0].operations.len(), 4);
        assert!(matches!(
            optimized.plan().functions[0].operations[0],
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                value: IntegerValue::Signed(7),
                ..
            } if psi_operation == OperationId::new(1_161).unwrap()
                && result == ValueId::new(1_153).unwrap()
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[1],
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                value: IntegerValue::Signed(-1),
                ..
            } if psi_operation == OperationId::new(1_158).unwrap()
                && result == ValueId::new(1_154).unwrap()
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[2],
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                value: IntegerValue::Signed(0),
                ..
            } if psi_operation == OperationId::new(1_159).unwrap()
                && result == ValueId::new(1_155).unwrap()
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[3],
            AbstractOperation::Return { value, .. }
                if value == ValueId::new(1_155).unwrap()
        ));
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            )
        }));
        assert!(optimized.unit().functions[0].facts.iter().any(|fact| {
            matches!(
                fact,
                omega_optimization_unit::OptimizationFact::IntegerConstant {
                    value,
                    constant: IntegerValue::Signed(0),
                    support,
                } if *value == ValueId::new(1_155).unwrap()
                    && *support == OperationId::new(1_159).unwrap()
            )
        }));
        assert_eq!(
            optimized.pass_manifests()[0].decisions()[0]
                .consumed_facts()
                .len(),
            2
        );
        let constant = &optimized.unit().functions[0].blocks[0].nodes[2];
        assert_eq!(constant.provenance.len(), 1);
        assert_eq!(constant.fuel.len(), 1);

        let lowered = lower_optimized_to_target_operations(optimized, target)
            .expect("the signed remainder-by-negative-one zero remains lowerable");
        assert_eq!(lowered.target(), target);
        assert_eq!(lowered.target_operations().functions.len(), 1);
        assert_eq!(lowered.optimized().commits().len(), 1);
    }
}

#[test]
fn proof_check_elision_projects_exact_signed_negative_one_shift_right_to_both_targets() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let optimized = project_optimization_run(run(
            live_exact_signed_negative_one_shift_right_verified(),
            selections.clone(),
        ))
        .unwrap();

        assert_eq!(optimized.commits().len(), 1);
        assert_eq!(optimized.transformation_ledger().records().len(), 1);
        assert_eq!(optimized.pass_manifests().len(), 1);
        assert_eq!(optimized.pass_manifests()[0].ordered_rules().len(), 12);
        assert_eq!(
                optimized.pass_manifests()[0].ordered_rules()[11],
                omega_optimization_core::OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1"
                )
            );
        assert_eq!(optimized.plan().functions[0].operations.len(), 3);
        assert!(matches!(
            optimized.plan().functions[0].operations[0],
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                value: IntegerValue::Signed(-1),
                ..
            } if psi_operation == OperationId::new(1_169).unwrap()
                && result == ValueId::new(1_164).unwrap()
        ));
        assert!(matches!(
            optimized.plan().functions[0].operations[2],
            AbstractOperation::Return { value, .. }
                if value == ValueId::new(1_164).unwrap()
        ));
        assert_eq!(optimized.unit().accepted_obligation_facts.len(), 1);
        assert!(optimized.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            )
        }));
        assert_eq!(
            optimized.pass_manifests()[0].decisions()[0]
                .consumed_facts()
                .len(),
            2
        );
        let terminal = &optimized.unit().functions[0].blocks[0].nodes[2];
        assert_eq!(terminal.provenance.len(), 2);
        assert_eq!(terminal.fuel.len(), 2);

        let lowered = lower_optimized_to_target_operations(optimized, target)
            .expect("the exact negative-one shift-free plan remains lowerable");
        assert_eq!(lowered.target(), target);
        assert_eq!(lowered.target_operations().functions.len(), 1);
        assert_eq!(lowered.optimized().commits().len(), 1);
    }
}
