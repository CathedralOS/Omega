//! Private comparison evidence joining compiler rows to source and observations.

mod adapter;
mod commitments;
mod model;
pub(crate) mod validation;

pub(crate) use adapter::PackageReviewEvidence;
pub(crate) use commitments::{build_observation_commitment, whole_review_commitment};
pub use model::{ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment};
