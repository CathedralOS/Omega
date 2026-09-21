//! Normalized restricted build-host request meaning retained for comparison.
//!
//! These records mirror the compiler's admitted
//! `build_evaluation::RestrictedBuildRequest` in evidence vocabulary so an
//! installer can retain accepted request meaning — in `omega.lock`, for
//! example — without storing host paths, live handles, or ephemeral
//! capabilities. A captured source inventory is recorded by presence only: the
//! captured counts and identity bind source content, and folding them into the
//! retained row would demand a fresh acceptance on every source edit even when
//! the requested authority is unchanged
//! (wiki/spec/packages/acceptance.md#restricted-build-acceptance).

use build_evaluation::{RestrictedBuildGrant, RestrictedBuildGrantRoot, RestrictedBuildOperation};
use target::TargetProfile;

/// The restricted operation one retained request asks of the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyRestrictedBuildOperation {
    /// Scoped real filesystem roots: reads under granted read roots, writes
    /// under granted write roots, anything else refused.
    ScopedFilesystemExecution,
    /// Real filesystem without path grants. Admission does not currently
    /// select this mode; it is retained so a review cannot silently drop it.
    UnscopedFilesystemExecution,
}

/// The logical grant root a retained request names, by compiler vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyRestrictedBuildGrantRoot {
    /// The package's source inventory.
    SourceInventory,
    /// The activation's staged build-output tree.
    StagedOutput,
    /// A compiler-issued grant root identity this projection does not name.
    Other(u32),
}

/// One logical root grant inside a retained request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRestrictedBuildGrant {
    pub(crate) root: PackagePolicyRestrictedBuildGrantRoot,
    /// The granted root's canonical metadata index narrows every operation
    /// to captured membership and kind.
    pub(crate) narrowed: bool,
    /// Whether an immutable captured inventory is bound to the root. The
    /// inventory's entry counts and identity are source content, not request
    /// meaning, and stay out of the retained row.
    pub(crate) captured: bool,
}

/// The shared filesystem sponsor account's staging ceilings, present when a
/// caller-installed sponsor binds every granted operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyFilesystemSponsorLimits {
    pub(crate) maximum_entries: u64,
    pub(crate) maximum_total_logical_bytes: u64,
    pub(crate) maximum_object_extent: u64,
}

/// The evaluation sponsor's resource ceilings, present when a
/// caller-installed sponsor caps this activation's evaluator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyEvaluationSponsorLimits {
    pub(crate) maximum_fuel_units: u64,
    pub(crate) maximum_build_log_bytes: u64,
    pub(crate) maximum_filesystem_operation_attempts: u64,
    pub(crate) maximum_live_filesystem_handles: u64,
    pub(crate) maximum_live_cells: u64,
    pub(crate) maximum_live_text_bytes: u64,
    pub(crate) maximum_result_cells: u64,
    pub(crate) maximum_result_text_bytes: u64,
}

/// The bounds admission attached to one retained request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRestrictedBuildBounds {
    pub(crate) filesystem_sponsor_limits: Option<PackagePolicyFilesystemSponsorLimits>,
    pub(crate) evaluation_sponsor_limits: Option<PackagePolicyEvaluationSponsorLimits>,
    /// Declared sealed outputs that must complete in the staged-output tree
    /// before this activation's result may publish.
    pub(crate) required_outputs: Vec<Vec<u8>>,
    /// Whether this activation publishes retained outputs only.
    pub(crate) artifact_only: bool,
}

/// One normalized restricted build-host request as retained comparison
/// meaning. This is admission intent, not execution evidence and not an
/// acceptance decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRestrictedBuildRequest {
    pub(crate) operation: PackagePolicyRestrictedBuildOperation,
    /// Logical roots this request may read, in grant order.
    pub(crate) read_grants: Vec<PackagePolicyRestrictedBuildGrant>,
    /// Logical roots this request may write, in grant order.
    pub(crate) write_grants: Vec<PackagePolicyRestrictedBuildGrant>,
    pub(crate) bounds: PackagePolicyRestrictedBuildBounds,
    /// The admitted build execution profile the request's build-scope
    /// declarations were checked against; `None` names an admitted host no
    /// catalogued profile describes.
    pub(crate) build_execution_profile: Option<TargetProfile>,
    /// The product target the requesting compilation selected for this
    /// activation.
    pub(crate) selected_target_profile: Option<TargetProfile>,
}

/// Retained-request meaning projects from the compiler's admitted request
/// unchanged — logical grant roots by compiler vocabulary, narrowing and
/// capture *presence*, sponsor ceilings, declared required outputs, and the
/// activation profiles — never host paths, live handles, or ephemeral
/// capability state.
impl From<&build_evaluation::RestrictedBuildRequest> for PackagePolicyRestrictedBuildRequest {
    fn from(request: &build_evaluation::RestrictedBuildRequest) -> Self {
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
