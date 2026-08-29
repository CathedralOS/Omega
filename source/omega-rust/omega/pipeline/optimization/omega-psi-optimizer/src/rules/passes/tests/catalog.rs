//! Built-in rule-catalog selection and deterministic-order tests.

use super::*;
use crate::PsiPassTargetApplicability;

#[test]
fn ordered_catalog_covers_every_declared_psi_optimization_once() {
    assert_eq!(
        PSI_PASS_CATALOG.map(|entry| entry.optimization()),
        ORDERED_PSI_PASSES,
        "the compatibility order must be derived from the descriptor catalog",
    );
    assert!(PSI_PASS_CATALOG.iter().all(|entry| {
        entry.target_applicability() == PsiPassTargetApplicability::TargetIndependent
    }));
    let mut declared = Optimization::ALL
        .into_iter()
        .filter(|optimization| {
            optimization.execution_phase()
                == omega_optimization_core::OptimizationExecutionPhase::Psi
        })
        .collect::<Vec<_>>();
    let mut catalog = ORDERED_PSI_PASSES.to_vec();
    declared.sort_unstable();
    catalog.sort_unstable();
    catalog.dedup();
    assert_eq!(catalog.len(), ORDERED_PSI_PASSES.len());
    assert_eq!(catalog, declared);
    for optimization in ORDERED_PSI_PASSES {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let scheduled = built_in_psi_registries(&selections).unwrap();
        assert_eq!(scheduled.len(), 1, "{optimization:?} must schedule once");
        assert!(!scheduled[0].is_empty(), "{optimization:?} has no rules");
    }
}

#[test]
fn built_in_schedule_is_independent_of_registration_arrival_order() {
    for optimization in [
        Optimization::SparseConditionalConstantPropagation,
        Optimization::ControlFlowCleanup,
        Optimization::GlobalValueNumbering,
        Optimization::DeadPureScalarElimination,
    ] {
        let expected = registry_for_optimization(optimization).unwrap();
        let expected_contracts = expected.contracts().collect::<Vec<_>>();

        for registry in randomized_built_in_registries(optimization) {
            assert_eq!(registry.identity(), expected.identity());
            assert_eq!(registry.contracts().collect::<Vec<_>>(), expected_contracts);
        }
    }
}

#[test]
fn absent_selection_registers_nothing_and_missing_analysis_fails_closed() {
    let unit = exact_add_unit();
    assert!(
        built_in_psi_registry(&OptimizationSelections::default())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ExactIntegerAddConstantsRule.propose(&unit, RuleAnalysisView::new(&[])),
        Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants
        ))
    );
    let cleanup = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    assert_eq!(built_in_psi_registry(&cleanup).unwrap().len(), 7);
    let copy = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    assert_eq!(built_in_psi_registry(&copy).unwrap().len(), 1);
    let gvn = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let gvn = built_in_psi_registry(&gvn).unwrap();
    assert_eq!(gvn.len(), 13);
    assert_eq!(
        gvn.contracts()
            .map(|contract| contract.identity())
            .collect::<Vec<_>>(),
        [
            SameBlockTotalScalarCseRule::contract().identity(),
            SameBlockProofCertifiedScalarCseRule::contract().identity(),
            DominatorTotalScalarGvnRule::contract().identity(),
            DominatorProofCertifiedScalarGvnRule::contract().identity(),
            PhiTranslatedObligationFreeScalarGvnRule::contract().identity(),
            PhiTranslatedProofCertifiedScalarGvnRule::contract().identity(),
            SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract().identity(),
            DominatorProofCertifiedCompatiblePolicyScalarGvnRule::contract().identity(),
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract().identity(),
            WrappingNeutralArithmeticIdentityRule::contract().identity(),
            WrappingShiftZeroCountIdentityRule::contract().identity(),
            WrappingMultiplyZeroAnnihilationRule::contract().identity(),
            SaturatingNeutralArithmeticIdentityRule::contract().identity(),
        ]
    );
    assert!(gvn.contracts().all(|contract| {
        contract.pass()
            == OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME)
    }));
    let dead = OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap();
    assert_eq!(built_in_psi_registry(&dead).unwrap().len(), 2);
    let proof = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let proof = built_in_psi_registry(&proof).unwrap();
    assert_eq!(proof.len(), 12);
    assert_eq!(
        proof
            .contracts()
            .map(|contract| contract.identity())
            .collect::<Vec<_>>(),
        [
            ProofCertifiedDeadScalarEliminationRule::contract().identity(),
            LiveProofCertifiedIntegerIdentityEliminationRule::contract().identity(),
            LiveProofCertifiedIntegerDivideByOneEliminationRule::contract().identity(),
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule::contract().identity(),
            LiveProofCertifiedIntegerZeroDividendEliminationRule::contract().identity(),
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule::contract().identity(),
            LiveProofCertifiedExactIntegerSelfSubtractEliminationRule::contract().identity(),
            LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract().identity(),
            LiveProofCertifiedIntegerSelfDivideEliminationRule::contract().identity(),
            LiveProofCertifiedIntegerRemainderByOneEliminationRule::contract().identity(),
            LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule::contract()
                .identity(),
            LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule::contract()
                .identity(),
        ]
    );
    let unsupported_combination = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::CopyPropagation,
    ])
    .unwrap();
    assert!(matches!(
        built_in_psi_registry(&unsupported_combination),
        Err(RuleRegistryError::UnsupportedOptimizationCombination)
    ));

    let lower_only =
        OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate]).unwrap();
    assert!(built_in_psi_registry(&lower_only).unwrap().is_empty());
    assert!(built_in_psi_registries(&lower_only).unwrap().is_empty());

    let sccp =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let mixed = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let sccp_registries = built_in_psi_registries(&sccp).unwrap();
    let mixed_registries = built_in_psi_registries(&mixed).unwrap();
    assert_eq!(mixed_registries.len(), 1);
    assert_eq!(
        mixed_registries[0].identity(),
        sccp_registries[0].identity()
    );
    assert_eq!(
        mixed_registries[0].contracts().collect::<Vec<_>>(),
        sccp_registries[0].contracts().collect::<Vec<_>>()
    );
}
