//! Restart-stable review baselines and bounded rooted-file custody.

mod capsule;
mod encoding;
mod model;
mod operations;
mod storage;
mod validation;

pub use capsule::{ReviewOnlyBaselineCapsule, ReviewOnlyBaselinePackage};
pub use model::{ReviewOnlyBaselineError, ReviewOnlyBaselineLimits};
pub use operations::{
    assemble_update_source_review_from_baseline, compare_review_only_capabilities_from_baseline,
    triage_review_update_from_baseline,
};
pub use storage::{
    ReviewOnlyBaselineDirectory, ReviewOnlyBaselineFileError, ReviewOnlyBaselineName,
    ReviewOnlyBaselineNameError,
};

use model::{
    BASELINE_NAME_MAXIMUM_BYTES, CHECKSUM_BYTES, CHECKSUM_DOMAIN, MAGIC,
    REPLAY_PARENT_BINDING_DOMAIN, REVIEW_ONLY_ARTIFACT_CLASS, VERSION,
};

#[cfg(test)]
#[path = "tests.rs"]
mod replay_record_tests;
