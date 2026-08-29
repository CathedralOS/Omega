//! Exact global-value-numbering rule order.

use crate::rules::catalog::BuiltInRuleRegistration;

use super::*;

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
    ]
}
