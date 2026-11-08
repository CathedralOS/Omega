// Admitted restricted build-host request projection into retained package policy.
// We copy request meaning only (logical grant roots by compiler vocabulary,
// narrowing and capture presence, sponsor ceilings, declared required outputs, and
// activation profiles) while excluding host paths, live handles, ephemeral
// capabilities, and captured-inventory contents so source edits do not demand fresh
// permission acceptance when requested authority is unchanged.

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
