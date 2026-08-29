//! Canonical question binding the closure to locally reconstructed evidence.

mod assembly;
mod encoding;
mod model;
mod validation;

pub use model::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionFingerprint,
    CanonicalPackageReconstructionQuestionLimits,
};

const RECONSTRUCTION_QUESTION_MAGIC: &[u8] = b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION\0";
pub const PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION: u16 = 1;
const RECONSTRUCTION_QUESTION_FINGERPRINT_DOMAIN: &[u8] =
    b"OMEGA-PACKAGE-RECONSTRUCTION-QUESTION-FINGERPRINT\0";
