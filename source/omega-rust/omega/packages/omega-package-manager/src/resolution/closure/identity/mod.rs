//! Canonical, recoverable identity of one complete resolved source closure.

mod model;

pub use model::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint,
    CanonicalSourceClosureSubjectLimits, SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
};
