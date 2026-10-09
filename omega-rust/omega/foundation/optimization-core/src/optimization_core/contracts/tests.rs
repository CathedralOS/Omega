use super::budget::InvalidOptimizationWorkBudget;
use super::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, CoreContractDecodeError,
    OPTIMIZATION_REFERENCE_EXTENT, OptimizationCandidateVerdict,
    OptimizationReasonCode, OptimizationRuleContract, OptimizationSafetyClass,
    OptimizationWorkBudget,
};
use crate::{OptimizationPassIdentity, OptimizationRuleIdentity};

#[test]
fn analysis_sets_are_ordered_and_reject_unknown_bits() {
    let set = AnalysisSet::new([
        AnalysisKind::ValueLiveness,
        AnalysisKind::ControlFlowGraph,
        AnalysisKind::Dominators,
    ]);
    assert_eq!(
        set.iter().collect::<Vec<_>>(),
        vec![
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::ValueLiveness,
        ]
    );
    assert_eq!(AnalysisSet::decode(&set.encode()), Ok(set));
    assert!(matches!(
        AnalysisSet::decode(&(1_u64 << 63).to_le_bytes()),
        Err(CoreContractDecodeError::UnknownAnalysisBits(_))
    ));
}

#[test]
fn safety_verdict_and_budget_encodings_are_total() {
    for safety in [
        OptimizationSafetyClass::StructuralIdentity,
        OptimizationSafetyClass::ExactOperationSemantics,
        OptimizationSafetyClass::ProofCertified,
        OptimizationSafetyClass::OwnershipCertified,
        OptimizationSafetyClass::TranslationValidated,
    ] {
        assert_eq!(
            OptimizationSafetyClass::decode(&safety.encode()),
            Ok(safety)
        );
    }
    for verdict in [
        OptimizationCandidateVerdict::Applied,
        OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::NotProfitable),
        OptimizationCandidateVerdict::Rejected(OptimizationReasonCode::ValidationFailed),
    ] {
        assert_eq!(
            OptimizationCandidateVerdict::decode(&verdict.encode()),
            Ok(verdict)
        );
    }
    for reason in OptimizationReasonCode::ALL {
        let verdict = OptimizationCandidateVerdict::Rejected(reason);
        assert_eq!(
            OptimizationCandidateVerdict::decode(&verdict.encode()),
            Ok(verdict)
        );
    }
    assert_eq!(
        OptimizationCandidateVerdict::decode(&[1, 1]),
        Err(CoreContractDecodeError::UnexpectedReason(1))
    );

    let budget = OptimizationWorkBudget::new(10, 20, 30, 4, 5).unwrap();
    assert_eq!(OptimizationWorkBudget::decode(&budget.encode()), Ok(budget));
    assert_eq!(
        OptimizationWorkBudget::new(0, 1, 1, 1, 1),
        Err(InvalidOptimizationWorkBudget)
    );
}

#[test]
fn extent_scaled_ceilings_grow_once_per_started_reference_extent() {
    let budget = OptimizationWorkBudget::new(10, 20, 30, 4, 5).unwrap();
    // Inputs within the reference extent keep the calibrated ceilings.
    assert_eq!(budget.for_extent(0), budget);
    assert_eq!(budget.for_extent(OPTIMIZATION_REFERENCE_EXTENT), budget);
    // Every started reference extent grants every axis again.
    assert_eq!(
        budget.for_extent(OPTIMIZATION_REFERENCE_EXTENT + 1),
        OptimizationWorkBudget::new(20, 40, 60, 8, 10).unwrap()
    );
    assert_eq!(
        budget.for_extent(3 * OPTIMIZATION_REFERENCE_EXTENT),
        OptimizationWorkBudget::new(30, 60, 90, 12, 15).unwrap()
    );
    // Linear work that fits at the reference extent fits at every extent:
    // two charges per input unit against a ceiling calibrated for them.
    let linear =
        OptimizationWorkBudget::new(1, 1, 2 * OPTIMIZATION_REFERENCE_EXTENT, 1, 1).unwrap();
    for extent in [
        1,
        OPTIMIZATION_REFERENCE_EXTENT,
        7 * OPTIMIZATION_REFERENCE_EXTENT + 5,
    ] {
        assert!(2 * extent <= linear.for_extent(extent).validation_steps());
    }
    // Quadratic work exhausts the scaled ceiling once the input grows.
    let extent = 64 * OPTIMIZATION_REFERENCE_EXTENT;
    assert!(extent * extent > linear.for_extent(extent).validation_steps());
    // Saturation keeps the scaled ceilings total.
    let wide = OptimizationWorkBudget::new(1, 1, u64::MAX / 2, 1, 1).unwrap();
    assert_eq!(
        wide.for_extent(3 * OPTIMIZATION_REFERENCE_EXTENT)
            .validation_steps(),
        u64::MAX
    );
}

#[test]
fn rule_contract_round_trip_binds_every_axis() {
    let contract = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(b"cfg/fold-branch/v1"),
        OptimizationPassIdentity::from_canonical_bytes(b"cfg-cleanup/v1"),
        1,
        AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::ScalarConstants,
        ]),
        AnalysisInvalidationSet::new([AnalysisKind::ControlFlowGraph, AnalysisKind::Dominators]),
        OptimizationSafetyClass::ExactOperationSemantics,
    )
    .unwrap();
    let encoded = contract.encode();
    assert_eq!(encoded.len(), 97);
    assert_eq!(OptimizationRuleContract::decode(&encoded), Ok(contract));

    let mut unknown_analysis = encoded;
    unknown_analysis[87] |= 0x80;
    assert!(matches!(
        OptimizationRuleContract::decode(&unknown_analysis),
        Err(CoreContractDecodeError::UnknownAnalysisBits(_))
    ));
}
