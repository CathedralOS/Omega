use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::super::GLOBAL_VALUE_NUMBERING_PASS_NAME;
use super::proposal::propose_total_scalar_identities;
use super::{
    saturating_multiply_zero_annihilation_shapes, saturating_neutral_identity_shapes,
    wrapping_multiply_zero_annihilation_shapes, wrapping_neutral_identity_shapes,
    wrapping_shift_zero_count_identity_shapes,
};

const REQUIRED_ANALYSES: [AnalysisKind; 3] = [
    AnalysisKind::ScalarConstants,
    AnalysisKind::UseDefinition,
    AnalysisKind::EffectSummaries,
];

const INVALIDATED_ANALYSES: [AnalysisKind; 2] =
    [AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries];

fn contract(identity: &[u8]) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(identity),
        OptimizationPassIdentity::from_canonical_bytes(GLOBAL_VALUE_NUMBERING_PASS_NAME),
        1,
        AnalysisSet::new(REQUIRED_ANALYSES),
        AnalysisInvalidationSet::new(INVALIDATED_ANALYSES),
        OptimizationSafetyClass::ExactOperationSemantics,
    )
    .expect("built-in rule has nonzero version")
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WrappingNeutralArithmeticIdentityRule;

impl WrappingNeutralArithmeticIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        contract(
            b"omega.psi-rule.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1",
        )
    }
}

impl PsiOptimizationRule for WrappingNeutralArithmeticIdentityRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_total_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            wrapping_neutral_identity_shapes,
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WrappingShiftZeroCountIdentityRule;

impl WrappingShiftZeroCountIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        contract(
            b"omega.psi-rule.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1",
        )
    }
}

impl PsiOptimizationRule for WrappingShiftZeroCountIdentityRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_total_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            wrapping_shift_zero_count_identity_shapes,
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WrappingMultiplyZeroAnnihilationRule;

impl WrappingMultiplyZeroAnnihilationRule {
    pub fn contract() -> OptimizationRuleContract {
        contract(
            b"omega.psi-rule.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1",
        )
    }
}

impl PsiOptimizationRule for WrappingMultiplyZeroAnnihilationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_total_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            wrapping_multiply_zero_annihilation_shapes,
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SaturatingNeutralArithmeticIdentityRule;

impl SaturatingNeutralArithmeticIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        contract(
            b"omega.psi-rule.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1",
        )
    }
}

impl PsiOptimizationRule for SaturatingNeutralArithmeticIdentityRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_total_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            saturating_neutral_identity_shapes,
        )
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SaturatingMultiplyZeroAnnihilationRule;

impl SaturatingMultiplyZeroAnnihilationRule {
    pub fn contract() -> OptimizationRuleContract {
        contract(
            b"omega.psi-rule.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1",
        )
    }
}

impl PsiOptimizationRule for SaturatingMultiplyZeroAnnihilationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_total_scalar_identities(
            unit,
            analyses,
            Self::contract(),
            saturating_multiply_zero_annihilation_shapes,
        )
    }
}
