//! Optimizer module role: executable entrance. Proof-check elision, cataloged by the exact scalar identity being proved.

use crate::rules::catalog::BuiltInRuleRegistration;

mod divide_by_one;
mod identity_rewrite;
mod multiply_by_zero;
mod negative_one_shift_right;
mod proof_certified_dead_scalar_elimination;
mod remainder_by_one;
mod scalar_identities;
mod self_divide;
mod self_remainder;
mod self_subtract;
mod signed_remainder_by_negative_one;
mod zero_dividend;
mod zero_value_shift;

pub use divide_by_one::LiveProofCertifiedIntegerDivideByOneEliminationRule;
pub use multiply_by_zero::LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule;
pub use negative_one_shift_right::LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule;
pub use proof_certified_dead_scalar_elimination::ProofCertifiedDeadScalarEliminationRule;
pub use remainder_by_one::LiveProofCertifiedIntegerRemainderByOneEliminationRule;
pub use scalar_identities::LiveProofCertifiedIntegerIdentityEliminationRule;
pub use self_divide::LiveProofCertifiedIntegerSelfDivideEliminationRule;
pub use self_remainder::LiveProofCertifiedIntegerSelfRemainderEliminationRule;
pub use self_subtract::LiveProofCertifiedExactIntegerSelfSubtractEliminationRule;
pub use signed_remainder_by_negative_one::LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule;
pub use zero_dividend::LiveProofCertifiedIntegerZeroDividendEliminationRule;
pub use zero_value_shift::LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule;

#[cfg(test)]
pub(super) use identity_rewrite::{integer_one, integer_zero};

/// The exact local rule order for this pass.
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, ProofCertifiedDeadScalarEliminationRule),
        BuiltInRuleRegistration::new(1, LiveProofCertifiedIntegerIdentityEliminationRule),
        BuiltInRuleRegistration::new(2, LiveProofCertifiedIntegerDivideByOneEliminationRule),
        BuiltInRuleRegistration::new(
            3,
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule,
        ),
        BuiltInRuleRegistration::new(4, LiveProofCertifiedIntegerZeroDividendEliminationRule),
        BuiltInRuleRegistration::new(
            5,
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule,
        ),
        BuiltInRuleRegistration::new(6, LiveProofCertifiedExactIntegerSelfSubtractEliminationRule),
        BuiltInRuleRegistration::new(7, LiveProofCertifiedIntegerSelfRemainderEliminationRule),
        BuiltInRuleRegistration::new(8, LiveProofCertifiedIntegerSelfDivideEliminationRule),
        BuiltInRuleRegistration::new(9, LiveProofCertifiedIntegerRemainderByOneEliminationRule),
        BuiltInRuleRegistration::new(
            10,
            LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
        ),
        BuiltInRuleRegistration::new(
            11,
            LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
        ),
    ]
}
