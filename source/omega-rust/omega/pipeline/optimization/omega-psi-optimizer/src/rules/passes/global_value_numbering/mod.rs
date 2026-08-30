//! Optimizer module role: executable entrance. Global value numbering, arranged by expression identity and traversal scope.

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{
    DominatingScalarCommonSubexpressionRewrite, LocalScalarCommonSubexpressionRewrite,
    NodeLocation, OptimizationFact, PhiTranslatedScalarGvnRewrite, PhiTranslatedScalarIncoming,
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite,
    PsiRewriteCandidate,
};
use psi_core::{BlockId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId};

use crate::rules::catalog::BuiltInRuleRegistration;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::{GLOBAL_VALUE_NUMBERING_PASS_NAME, accepted_obligation_fact, support::block_dominates};

mod accounting;
mod dominating;
mod expression_keys;
mod identities;
mod local;
mod phi_translated;

pub use dominating::*;
pub use identities::*;
pub use local::*;
pub use phi_translated::*;

pub(in crate::rules::passes) use accounting::local_cse_accounting;
use accounting::*;
use expression_keys::*;
pub(in crate::rules::passes) use expression_keys::{
    compatible_policy_scalar_leader, compatible_policy_scalar_redundant,
    proof_certified_scalar_expression,
};

/// The exact local rule order for this pass.
pub(in crate::rules) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, SameBlockTotalScalarCseRule),
        BuiltInRuleRegistration::new(1, SameBlockProofCertifiedScalarCseRule),
        BuiltInRuleRegistration::new(2, DominatorTotalScalarGvnRule),
        BuiltInRuleRegistration::new(3, DominatorProofCertifiedScalarGvnRule),
        BuiltInRuleRegistration::new(4, PhiTranslatedObligationFreeScalarGvnRule),
        BuiltInRuleRegistration::new(5, PhiTranslatedProofCertifiedScalarGvnRule),
        BuiltInRuleRegistration::new(6, SameBlockProofCertifiedCompatiblePolicyScalarCseRule),
        BuiltInRuleRegistration::new(7, DominatorProofCertifiedCompatiblePolicyScalarGvnRule),
        BuiltInRuleRegistration::new(8, PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule),
        BuiltInRuleRegistration::new(9, WrappingNeutralArithmeticIdentityRule),
        BuiltInRuleRegistration::new(10, WrappingShiftZeroCountIdentityRule),
        BuiltInRuleRegistration::new(11, WrappingMultiplyZeroAnnihilationRule),
        BuiltInRuleRegistration::new(12, SaturatingNeutralArithmeticIdentityRule),
        BuiltInRuleRegistration::new(13, SaturatingMultiplyZeroAnnihilationRule),
        BuiltInRuleRegistration::new(14, BitwiseNeutralLiteralIdentityRule),
        BuiltInRuleRegistration::new(15, BitwiseAbsorbingLiteralIdentityRule),
    ]
}
