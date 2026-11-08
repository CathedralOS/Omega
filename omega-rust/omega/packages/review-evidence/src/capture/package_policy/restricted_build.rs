//! Project admitted restricted build-host requests into retained evidence.
//!
//! The projection copies request meaning only: logical grant roots by compiler
//! vocabulary, narrowing and capture *presence*, sponsor ceilings, declared
//! required outputs, and the activation profiles. Host paths, live handles,
//! ephemeral capabilities, and captured-inventory contents never enter the
//! record — a captured inventory's counts bind source content, and folding
//! them into retained meaning would demand a fresh acceptance on every source
//! edit even when the requested authority is unchanged.

use crate::capture::PackageReviewInput;
use crate::record::PackagePolicyRestrictedBuildRequest;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
) -> Vec<PackagePolicyRestrictedBuildRequest> {
    compilation
        .custody
        .build_admission()
        .restricted_build_requests
        .iter()
        .map(Into::into)
        .collect()
}
