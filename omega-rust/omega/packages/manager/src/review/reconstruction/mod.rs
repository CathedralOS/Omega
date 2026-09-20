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
/// Version 2 prepends each entry frame with the bitmask of the occurrence
/// purposes the entry's review answered; version 1 entries were bare ledgers.
pub const PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION: u16 = 2;
const RECONSTRUCTION_QUESTION_FINGERPRINT_DOMAIN: &[u8] =
    b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION-FINGERPRINT\0";
