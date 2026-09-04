use super::*;
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
