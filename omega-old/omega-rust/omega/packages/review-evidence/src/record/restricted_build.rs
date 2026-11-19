// Normalized restricted build-host request meaning retained for comparison.
//
// These records mirror package build-execution requests in evidence vocabulary
// so an installer can retain accepted request meaning (e.g. in `omega.lock`)
// without storing host paths, live handles, or ephemeral capabilities. A
// captured source inventory is recorded by presence (`captured: bool`) only:
// captured counts and identity bind source content, and folding them into the
// retained row would demand a fresh acceptance on every source edit even when
// the requested authority is unchanged. `UnscopedFilesystemExecution` remains
// in the enum so review surfaces it if an evaluator ever carries it, even
// though build admission issues only scoped grants.

use package_compilation::build_execution::{
    RestrictedBuildBounds, RestrictedBuildGrant, RestrictedBuildGrantRoot,
    RestrictedBuildOperation, RestrictedBuildRequest,
};
use target::TargetProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackagePolicyRestrictedBuildOperation {
    ScopedFilesystemExecution,
    UnscopedFilesystemExecution,
}

pub(crate) type PackagePolicyRestrictedBuildGrantRoot =
    package_compilation::RestrictedBuildGrantRoot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyRestrictedBuildGrant {
    pub(crate) root: PackagePolicyRestrictedBuildGrantRoot,
    pub(crate) narrowed: bool, // Canonical metadata index narrows operations to captured membership/kind.
    pub(crate) captured: bool, // Whether an immutable captured inventory is bound to the root.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyFilesystemSponsorLimits {
    pub(crate) maximum_entries: u64,
    pub(crate) maximum_total_logical_bytes: u64,
    pub(crate) maximum_object_extent: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyEvaluationSponsorLimits {
    pub(crate) work_budget: u64,
    pub(crate) maximum_build_log_bytes: u64,
    pub(crate) maximum_filesystem_operation_attempts: u64,
    pub(crate) maximum_live_filesystem_handles: u64,
    pub(crate) maximum_live_cells: u64,
    pub(crate) maximum_live_text_bytes: u64,
    pub(crate) maximum_result_cells: u64,
    pub(crate) maximum_result_text_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyRestrictedBuildBounds {
    pub(crate) filesystem_sponsor_limits: Option<PackagePolicyFilesystemSponsorLimits>,
    pub(crate) evaluation_sponsor_limits: Option<PackagePolicyEvaluationSponsorLimits>,
    pub(crate) required_outputs: Vec<Vec<u8>>, // Declared sealed outputs required in staged-output tree.
    pub(crate) artifact_only: bool, // True when activation publishes retained outputs only.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRestrictedBuildRequest {
    pub(crate) operation: PackagePolicyRestrictedBuildOperation,
    pub(crate) read_grants: Vec<PackagePolicyRestrictedBuildGrant>,
    pub(crate) write_grants: Vec<PackagePolicyRestrictedBuildGrant>,
    pub(crate) bounds: PackagePolicyRestrictedBuildBounds,
    pub(crate) build_execution_profile: Option<TargetProfile>,
    pub(crate) selected_target_profile: Option<TargetProfile>,
}

impl From<&RestrictedBuildRequest> for PackagePolicyRestrictedBuildRequest {
    fn from(request: &RestrictedBuildRequest) -> Self {
        let bounds = request.bounds();
        Self {
            operation: match request.operation() {
                RestrictedBuildOperation::ScopedFilesystemExecution => {
                    PackagePolicyRestrictedBuildOperation::ScopedFilesystemExecution
                }
                RestrictedBuildOperation::UnscopedFilesystemExecution => {
                    PackagePolicyRestrictedBuildOperation::UnscopedFilesystemExecution
                }
            },
            read_grants: request.read_grants().iter().map(Into::into).collect(),
            write_grants: request.write_grants().iter().map(Into::into).collect(),
            bounds: bounds.into(),
            build_execution_profile: request.build_execution_profile(),
            selected_target_profile: request.selected_target_profile(),
        }
    }
}

impl From<&RestrictedBuildGrant> for PackagePolicyRestrictedBuildGrant {
    fn from(grant: &RestrictedBuildGrant) -> Self {
        Self {
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
}

impl From<&RestrictedBuildBounds> for PackagePolicyRestrictedBuildBounds {
    fn from(bounds: &RestrictedBuildBounds) -> Self {
        Self {
            filesystem_sponsor_limits: bounds.filesystem_sponsor_limits().map(|limits| {
                PackagePolicyFilesystemSponsorLimits {
                    maximum_entries: limits.maximum_entries,
                    maximum_total_logical_bytes: limits.maximum_total_logical_bytes,
                    maximum_object_extent: limits.maximum_object_extent,
                }
            }),
            evaluation_sponsor_limits: bounds.evaluation_sponsor_limits().map(|limits| {
                PackagePolicyEvaluationSponsorLimits {
                    work_budget: limits.work_budget(),
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
        }
    }
}
