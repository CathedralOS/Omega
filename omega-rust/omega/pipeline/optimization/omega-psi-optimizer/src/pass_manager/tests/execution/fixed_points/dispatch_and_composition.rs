//! Pass dispatch, ordering, manifest, and composed-pipeline fixed points.

use super::super::super::*;

#[test]
fn fixed_point_dispatch_validates_then_commits_with_stable_usage() {
    let unit = exact_add_unit();
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, decisions, pass_manifest, ledger) =
        run_unit(unit.clone(), &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].input, unit.identity);
    assert_eq!(commits[0].output, output.identity);
    assert_eq!(usage.commits, 1);
    assert_eq!(usage.validation_steps, 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 40);
    assert_eq!(decisions.records.len(), 1);
    assert_eq!(
        decisions.records[0].outcome,
        BaselineDecisionOutcome::Choose(commits[0].candidate)
    );
    let pass_manifest = pass_manifest.expect("selected pass emits a manifest row");
    assert_eq!(pass_manifest.ordered_rules().len(), 39);
    assert_eq!(pass_manifest.input(), unit.identity);
    assert_eq!(pass_manifest.output(), output.identity);
    assert_eq!(pass_manifest.decisions().len(), 1);
    assert_eq!(pass_manifest.decisions()[0].input(), unit.identity);
    assert_eq!(pass_manifest.decisions()[0].consumed_facts().len(), 3);
    assert_eq!(
        pass_manifest.decisions()[0].verdict(),
        OptimizationCandidateVerdict::Applied
    );
    assert_eq!(
        OptimizationPassManifestRecord::decode(&pass_manifest.encode()),
        Ok(pass_manifest)
    );
    assert_eq!(ledger.input(), unit.identity);
    assert_eq!(ledger.output(), output.identity);
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(ledger.records()[0].provenance, commits[0].provenance);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant { .. }
    ));
}

#[test]
fn ordered_multi_rule_group_reaches_a_dependent_exact_fixed_point() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, pass_manifest, ledger) =
        run_unit(dependent_exact_chain_unit(), &registry, budget(8)).unwrap();

    assert_eq!(commits.len(), 2);
    assert_eq!(usage.iterations, 3);
    assert_eq!(usage.rule_evaluations, 43);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(15),
            ..
        }
    ));
    assert!(matches!(
        output.functions[0].blocks[0].nodes[3].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(120),
            ..
        }
    ));
    let manifest = pass_manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 39);
    assert_eq!(manifest.decisions().len(), 2);
    assert_eq!(ledger.records().len(), 2);
}

#[test]
fn named_dead_scalar_suite_reaches_a_custody_preserving_fixed_point() {
    let selections =
        OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let unit = dead_wrapping_add_unit();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 3);
    assert_eq!(usage.iterations, 4);
    assert_eq!(output.functions[0].blocks[0].nodes.len(), 1);
    assert_eq!(output.functions[0].blocks[0].nodes[0].provenance.len(), 4);
    assert_eq!(ledger.records().len(), 3);
    assert!(
        ledger
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .all(|row| matches!(
                row.disposition,
                omega_optimization_unit::ProvenanceDisposition::RealizedAt(_)
            ))
    );
    assert_eq!(manifest.unwrap().ordered_rules().len(), 2);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn shuffled_builtin_registration_constructs_identical_multi_rule_runs() {
    for (optimization, unit) in [
        (
            Optimization::SparseConditionalConstantPropagation,
            dependent_exact_chain_unit(),
        ),
        (
            Optimization::ControlFlowCleanup,
            propagated_block_parameter_unit(true),
        ),
        (
            Optimization::GlobalValueNumbering,
            compatible_policy_local_cse_unit(),
        ),
        (
            Optimization::ProofCheckElision,
            live_self_divide_unit(
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
                SelfDividePolicy::Exact,
            ),
        ),
        (
            Optimization::DeadPureScalarElimination,
            dead_wrapping_add_unit(),
        ),
    ] {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let expected_registry = built_in_psi_registry(&selections).unwrap();
        let expected = run_unit(unit.clone(), &expected_registry, budget(8)).unwrap();

        for (seed, registry) in randomized_built_in_registries(
            optimization
                .psi_optimization()
                .expect("the test roster contains only Psi optimizations"),
        )
        .into_iter()
        .enumerate()
        {
            let actual = run_unit(unit.clone(), &registry, budget(8)).unwrap();
            let context = format!("{optimization:?} registration seed {}", seed + 1);
            assert_eq!(actual.0, expected.0, "final unit differs for {context}");
            assert_eq!(actual.1, expected.1, "commits differ for {context}");
            assert_eq!(actual.2, expected.2, "usage differs for {context}");
            assert_eq!(actual.3, expected.3, "decisions differ for {context}");
            assert_eq!(actual.4, expected.4, "manifest differs for {context}");
            assert_eq!(actual.5, expected.5, "ledger differs for {context}");
        }
    }
}

#[test]
fn full_sccp_cfg_copy_gvn_proof_dead_scalar_second_sweep_is_a_composed_ledger_fixed_point() {
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::ControlFlowCleanup,
        Optimization::CopyPropagation,
        Optimization::GlobalValueNumbering,
        Optimization::ProofCheckElision,
        Optimization::DeadPureScalarElimination,
    ])
    .unwrap();
    let registries = built_in_psi_registries(&selections).unwrap();

    for initial in [
        dependent_exact_chain_unit(),
        redundant_block_parameter_unit(true),
        dead_wrapping_add_unit(),
        dead_exact_add_unit(),
        local_cse_unit(),
        dominator_gvn_unit(),
        proof_certified_local_cse_unit(),
        proof_certified_dominator_gvn_unit(),
        live_divide_by_one_unit(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
            |psi_operation, obligation, result, scalar_type, left, right| {
                AbstractOperation::ExactIntegerDivide {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                }
            },
        ),
        live_exact_multiply_by_zero_unit(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
            false,
        ),
        live_exact_zero_value_shift_unit(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
            true,
        ),
        live_self_remainder_unit(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
            SelfRemainderPolicy::Exact,
        ),
        live_self_divide_unit(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
            SelfDividePolicy::Exact,
        ),
    ] {
        let (first_output, first_manifests, first_ledger) = run_test_pipeline(initial, &registries);
        assert_eq!(first_manifests.len(), 6);
        assert_eq!(first_manifests[0].input(), first_ledger.input());
        assert_eq!(first_manifests[0].output(), first_manifests[1].input());
        assert_eq!(first_manifests[1].output(), first_manifests[2].input());
        assert_eq!(first_manifests[2].output(), first_manifests[3].input());
        assert_eq!(first_manifests[3].output(), first_manifests[4].input());
        assert_eq!(first_manifests[4].output(), first_manifests[5].input());
        assert_eq!(first_manifests[5].output(), first_ledger.output());
        assert!(!first_ledger.records().is_empty());

        let (second_output, second_manifests, second_delta) =
            run_test_pipeline(first_output.clone(), &registries);
        assert_eq!(second_output, first_output);
        assert_eq!(second_manifests.len(), 6);
        assert!(second_delta.records().is_empty());
        assert_eq!(second_delta.input(), second_delta.output());
        assert!(second_manifests.iter().all(|manifest| {
            manifest.input() == manifest.output()
                && manifest
                    .decisions()
                    .iter()
                    .all(|decision| decision.verdict() != OptimizationCandidateVerdict::Applied)
        }));

        let mut composed_records = first_ledger.records().to_vec();
        composed_records.extend_from_slice(second_delta.records());
        let composed = PsiTransformationLedger::new(
            first_ledger.psi(),
            first_ledger.fuel_schedule(),
            first_ledger.input(),
            second_delta.output(),
            composed_records,
        )
        .unwrap();
        assert_eq!(composed, first_ledger);
    }
}

#[test]
fn manifest_retains_propagated_block_parameter_fact_identity() {
    let unit = propagated_block_parameter_unit(true);
    let AnalysisProduct::ScalarConstants(constants) = crate::compute_analysis(
        &unit,
        omega_optimization_core::AnalysisKind::ScalarConstants,
    )
    .unwrap() else {
        unreachable!()
    };
    let derived = constants
        .facts
        .iter()
        .find(|fact| !fact.support.edges.is_empty())
        .and_then(|fact| fact.identity)
        .expect("fixture has one proof-bearing propagated parameter fact");
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (_, commits, _, _, manifest, _) = run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(
        manifest.unwrap().decisions()[0].consumed_facts(),
        &[OptimizationFactReference::ScalarConstant(derived)]
    );
}

#[test]
fn pass_convergence_measure_includes_wrapping_and_saturating_rules() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, _, _) =
        run_unit(wrapping_add_unit(), &registry, budget(8)).unwrap();

    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(4),
            ..
        }
    ));
}

#[test]
fn pass_dispatches_typed_boolean_validation_to_fixed_point() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, _, _) =
        run_unit(boolean_unit(true), &registry, budget(8)).unwrap();

    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::BooleanConstant { value: false, .. }
    ));
}
