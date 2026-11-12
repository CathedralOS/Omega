// Pre-execution inputs (`BuildExecutionInputs`) and contract projection
// (`BuildExecutionContract`) supplied before a `Build` activation starts.
// Carries the execution target profile, writable staging directory, filesystem
// and evaluation sponsors, `BuildSnapshotRequest`, and optional restricted
// build-grant consent guard.

use super::{BuildSnapshotRequest, RestrictedBuildGrants};
use crate::PackageCompilationInputs;
use std::path::PathBuf;
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildExecutionContract {
    pub profile: Option<TargetProfile>,
    pub snapshot: Option<BuildSnapshotRequest>,
}

#[derive(Default)]
pub struct BuildExecutionInputs {
    pub profile: Option<TargetProfile>, // Compilation requests default to the host, independently of Product target.
    pub directory: Option<PathBuf>, // Writable staging, separate from immutable source snapshots.
    pub filesystem_sponsor: Option<build_time_evaluation::BuildMachineFilesystemSponsor>,
    pub evaluation_sponsor: Option<build_time_evaluation::BuildEvaluationSponsor>,
    pub snapshot: Option<BuildSnapshotRequest>,
    pub restricted_grants: Option<Box<dyn RestrictedBuildGrants>>, // Absence reports restricted requests without executing them.
    // A dependency's Build executed while resolving its consumer's sources.
    // The request's subject is the consumer, so this Build checks only what
    // its entry reaches (wiki/spec/build/validation_subjects.md#orchestration).
    pub dependency_activation: bool,
}

impl BuildExecutionInputs {
    pub fn contract(&self) -> BuildExecutionContract {
        BuildExecutionContract {
            profile: self.profile,
            snapshot: self.snapshot.clone(),
        }
    }

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
