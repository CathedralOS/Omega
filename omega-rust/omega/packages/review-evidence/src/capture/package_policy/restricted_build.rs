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
use crate::record::{
    PackagePolicyRestrictedBuildGrant, PackagePolicyRestrictedBuildGrantRoot,
    PackagePolicyRestrictedBuildOperation, PackagePolicyRestrictedBuildRequest,
};
use package_compilation::build_execution::{
    RestrictedBuildGrant, RestrictedBuildGrantRoot, RestrictedBuildOperation,
};

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
) -> Vec<PackagePolicyRestrictedBuildRequest> {
    compilation
        .custody
        .build_admission()
        .restricted_build_requests
        .iter()
        .map(request)
        .collect()
}

fn request(
    request: &package_compilation::build_execution::RestrictedBuildRequest,
) -> PackagePolicyRestrictedBuildRequest {
    let bounds = request.bounds();
    PackagePolicyRestrictedBuildRequest {
        operation: match request.operation() {
            RestrictedBuildOperation::ScopedFilesystemExecution => {
                PackagePolicyRestrictedBuildOperation::ScopedFilesystemExecution
            }
            RestrictedBuildOperation::UnscopedFilesystemExecution => {
                PackagePolicyRestrictedBuildOperation::UnscopedFilesystemExecution
            }
        },
        read_grants: request.read_grants().iter().map(grant).collect(),
        write_grants: request.write_grants().iter().map(grant).collect(),
        bounds: bounds.into(),
        build_execution_profile: request.build_execution_profile(),
        selected_target_profile: request.selected_target_profile(),
    }
}

fn grant(grant: &RestrictedBuildGrant) -> PackagePolicyRestrictedBuildGrant {
    PackagePolicyRestrictedBuildGrant {
        root: match grant.root() {
            RestrictedBuildGrantRoot::SourceInventory => {
                PackagePolicyRestrictedBuildGrantRoot::SourceInventory
            }
            RestrictedBuildGrantRoot::StagedOutput => {
                PackagePolicyRestrictedBuildGrantRoot::StagedOutput
            }
            RestrictedBuildGrantRoot::Other(identity) => {
                PackagePolicyRestrictedBuildGrantRoot::Other(identity)
            }
        },
        narrowed: grant.narrowed(),
        captured: grant.captured().is_some(),
    }
}
