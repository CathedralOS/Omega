//! Proof-certified remainder by one.

use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    LiveProofCertifiedIntegerRemainderByOneEliminationRule,
    super::proof_check_elision_rule_contract(
        b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
        AnalysisSet::new([
            AnalysisKind::ScalarConstants,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
    ),
);

psi_rule_impl!(
    LiveProofCertifiedIntegerRemainderByOneEliminationRule,
    |unit, analyses| {
        super::propose_proof_certified_remainder_by_constant(
            unit,
            analyses,
            Self::contract(),
            false,
            |scalar_type, constant| constant == scalar_type.one(),
        )
    }
);
