//! Canonical, recoverable subject of one complete resolved source closure.

mod encoding;
mod model;
mod request_view;
mod text;
mod usage;
mod validation;

pub(crate) use text::{recover_package_key_text, write_package_key_text};
pub use usage::CanonicalSourceClosureSubjectRecoveryUsage;

pub use model::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
};
