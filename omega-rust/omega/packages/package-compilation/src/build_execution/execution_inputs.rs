//! Owned obligations and resources supplied before a Build activation starts.

use super::{BuildSnapshotRequest, RestrictedBuildGrants};
use crate::PackageCompilationInputs;
use std::path::PathBuf;
use target::TargetProfile;

/// The source inventory and obligations supplied before one Build executes.
/// Comparing this record does not issue grants or authorize another execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildExecutionContract {
    pub profile: Option<TargetProfile>,
    pub snapshot: Option<BuildSnapshotRequest>,
}

/// Pre-effect inputs for one activation. Move this value through preparation
/// intact: source identity and Product target joins cannot replace its sponsors,
/// consent, selected inventory, named inputs, or required outputs after execution.
/// It is neither a completed Build nor reusable evidence or publication authority.
#[derive(Default)]
pub struct BuildExecutionInputs {
    /// Compilation requests default to the host, independently of Product target.
    /// An evaluator receives the resolved profile; None admits an uncatalogued host.
    pub profile: Option<TargetProfile>,
    /// Writable staging, separate from immutable source snapshots.
    pub directory: Option<PathBuf>,
    pub filesystem_sponsor: Option<build_time_evaluation::BuildMachineFilesystemSponsor>,
    pub evaluation_sponsor: Option<build_time_evaluation::BuildEvaluationSponsor>,
    pub snapshot: Option<BuildSnapshotRequest>,
    /// Absence reports restricted requests without executing them; privately
    /// issued snapshot/staging work remains independently admitted.
    pub restricted_grants: Option<Box<dyn RestrictedBuildGrants>>,
}

impl BuildExecutionInputs {
    pub fn contract(&self) -> BuildExecutionContract {
        BuildExecutionContract {
            profile: self.profile,
            snapshot: self.snapshot.clone(),
        }
    }
    /// Resolve compilation defaults once before frontend admission. Explicit
    /// snapshot custody always wins; no failed admission falls back to defaults.
    pub fn resolve_compilation_defaults(
        mut self,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Self {
        self.profile = self.profile.or_else(TargetProfile::host_if_supported);
        self.snapshot = self
            .snapshot
            .or_else(|| BuildSnapshotRequest::automatic_package_inventory(inputs));
        self
    }
}
