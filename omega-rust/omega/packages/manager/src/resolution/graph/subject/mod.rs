//! Canonical, recoverable subject of one complete resolved source closure.

mod encoding;
mod model;
mod request_view;
mod text;
mod usage;
mod validation;

pub use usage::CanonicalSourceClosureSubjectRecoveryUsage;

pub use model::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
};
