//! Optimizer module role: stage group. Byte-stability and validation contract tests, grouped by record family.

use super::*;
use crate::{
    AcceptedObligationFactIdentity, AnalysisKind, AnalysisSet, OptimizationCandidateIdentity,
    OptimizationCandidateVerdict, OptimizationPassIdentity, OptimizationReasonCode,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkBudget, OwnershipFrontierFactIdentity,
    ScalarConstantFactIdentity, ValueRangeFactIdentity,
};

mod decision;
mod fact_reference;
mod pass;
mod work_usage;

fn rule(name: &[u8]) -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(name)
}

fn fact(name: &[u8]) -> OptimizationFactReference {
    OptimizationFactReference::ScalarConstant(ScalarConstantFactIdentity::from_canonical_bytes(
        name,
    ))
}

fn obligation_fact(name: &[u8]) -> OptimizationFactReference {
    OptimizationFactReference::AcceptedObligation(
        AcceptedObligationFactIdentity::from_canonical_bytes(name),
    )
}

fn ownership_fact(name: &[u8]) -> OptimizationFactReference {
    OptimizationFactReference::OwnershipFrontier(
        OwnershipFrontierFactIdentity::from_canonical_bytes(name),
    )
}

fn range_fact(name: &[u8]) -> OptimizationFactReference {
    OptimizationFactReference::ValueRange(ValueRangeFactIdentity::from_canonical_bytes(name))
}

fn decision(rule: OptimizationRuleIdentity) -> OptimizationDecisionRecord {
    OptimizationDecisionRecord::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        OptimizationCandidateIdentity::from_canonical_bytes(b"candidate"),
        rule,
        OptimizationCandidateVerdict::Applied,
        AnalysisSet::new([AnalysisKind::ControlFlowGraph]),
        vec![fact(b"fact")],
        Some(OptimizationValidatorIdentity::from_canonical_bytes(
            b"validator",
        )),
    )
    .unwrap()
}
