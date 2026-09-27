//! Pass dispatch, ordering, manifest, and composed-pipeline fixed points.

use super::super::super::super::run_unit;
use super::super::super::{
    AbstractOperation, AnalysisProduct, Optimization, OptimizationCandidateVerdict,
    OptimizationFactReference, OptimizationSelections, PsiTransformationLedger, SelfDividePolicy,
    SelfRemainderPolicy, budget, built_in_psi_registries, built_in_psi_registry,
    compatible_policy_local_cse_unit, dead_exact_add_unit, dead_wrapping_add_unit,
    dependent_exact_chain_unit, dominator_gvn_unit, live_divide_by_one_unit,
    live_exact_multiply_by_zero_unit, live_exact_zero_value_shift_unit, live_self_divide_unit,
    live_self_remainder_unit, local_cse_unit, proof_certified_dominator_gvn_unit,
    proof_certified_local_cse_unit, propagated_block_parameter_unit,
    randomized_built_in_registries, redundant_block_parameter_unit, run_test_pipeline,
};

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
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    8,
                )
                .unwrap(),
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
                .optimization()
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
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
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
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
            false,
        ),
        live_exact_zero_value_shift_unit(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
            true,
        ),
        live_self_remainder_unit(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
            SelfRemainderPolicy::Exact,
        ),
        live_self_divide_unit(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
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
    let AnalysisProduct::ScalarConstants(constants) =
        crate::compute_analysis(&unit, optimization_core::AnalysisKind::ScalarConstants).unwrap()
    else {
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
