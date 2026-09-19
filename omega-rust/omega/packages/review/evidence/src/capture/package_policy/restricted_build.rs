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
    PackagePolicyEvaluationSponsorLimits, PackagePolicyFilesystemSponsorLimits,
    PackagePolicyRestrictedBuildBounds, PackagePolicyRestrictedBuildGrant,
    PackagePolicyRestrictedBuildGrantRoot, PackagePolicyRestrictedBuildOperation,
    PackagePolicyRestrictedBuildRequest,
};
use build_evaluation::{RestrictedBuildGrant, RestrictedBuildGrantRoot, RestrictedBuildOperation};

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
) -> Vec<PackagePolicyRestrictedBuildRequest> {
    compilation
        .custody
        .restricted_build_requests()
        .iter()
        .map(request)
        .collect()
}

fn request(
    request: &build_evaluation::RestrictedBuildRequest,
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
        bounds: PackagePolicyRestrictedBuildBounds {
            filesystem_sponsor_limits: bounds.filesystem_sponsor_limits().map(|limits| {
                PackagePolicyFilesystemSponsorLimits {
                    maximum_entries: limits.maximum_entries,
                    maximum_total_logical_bytes: limits.maximum_total_logical_bytes,
                    maximum_object_extent: limits.maximum_object_extent,
                }
            }),
            evaluation_sponsor_limits: bounds.evaluation_sponsor_limits().map(|limits| {
                PackagePolicyEvaluationSponsorLimits {
                    maximum_fuel_units: limits.maximum_fuel_units(),
                    maximum_build_log_bytes: limits.maximum_build_log_bytes(),
                    maximum_filesystem_operation_attempts: limits
                        .maximum_filesystem_operation_attempts(),
                    maximum_live_filesystem_handles: limits.maximum_live_filesystem_handles(),
                    maximum_live_cells: limits.maximum_live_cells(),
                    maximum_live_text_bytes: limits.maximum_live_text_bytes(),
                    maximum_result_cells: limits.maximum_result_cells(),
                    maximum_result_text_bytes: limits.maximum_result_text_bytes(),
                }
            }),
            required_outputs: bounds.required_outputs().to_vec(),
            artifact_only: bounds.artifact_only(),
        },
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
