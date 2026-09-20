//! Canonical question binding the closure to locally reconstructed evidence.

mod assembly;
mod encoding;
mod model;
mod results;
mod root_policy;
mod validation;

pub use model::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionFingerprint,
    CanonicalPackageReconstructionQuestionLimits,
};
pub use results::{LocallyComposedPackageObligationEntry, LocallyComposedPackageObligationResults};
pub(crate) use root_policy::bind_root_policy_with_associated_reviews;
pub use root_policy::{
    FreshPackageRootPolicyAcceptance, FreshPackageRootPolicyError, bind_fresh_package_root_policy,
};

const RECONSTRUCTION_QUESTION_MAGIC: &[u8] = b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION\0";
/// Version 3 retains one explicit checked context and ledger per occurrence.
/// Earlier package-only and implicit-purpose entries are not accepted.
pub const PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION: u16 = 3;
const RECONSTRUCTION_QUESTION_FINGERPRINT_DOMAIN: &[u8] =
    b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION-FINGERPRINT\0";
