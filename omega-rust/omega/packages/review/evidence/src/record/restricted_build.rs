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
