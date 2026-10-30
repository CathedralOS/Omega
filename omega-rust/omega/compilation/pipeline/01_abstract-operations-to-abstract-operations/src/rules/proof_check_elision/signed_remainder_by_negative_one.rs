//! Proof-certified signed remainder by negative one.

use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
use semantic_vocabulary::IntegerValue;

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

psi_rule_contract!(
    LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
    super::proof_check_elision_rule_contract(
                b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
                AnalysisSet::new([
                    AnalysisKind::ScalarConstants,
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ]),
            ),
);

psi_rule_impl!(
    LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
    |unit, analyses| {
        super::propose_proof_certified_remainder_by_constant(
            unit,
            analyses,
            Self::contract(),
            true,
            |_, constant| constant == IntegerValue::Signed(-1),
        )
    }
);
