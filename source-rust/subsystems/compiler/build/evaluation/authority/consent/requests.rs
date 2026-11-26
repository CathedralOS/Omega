//! Restricted operations a Build asks its receiving host to authorize.
#![forbid(unsafe_code)]
use build_output::BuildCapturedSourceInventory;
use evaluation_contracts::BuildEvaluationSponsorLimits;

/// The checked executable and selected entry that would perform a restricted
/// operation during this review. Portable policy records the operation itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildSite {
    pub program: [u8; 32],
    pub entry: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestrictedBuildOperation {
    ProgramLaunch(Box<RestrictedProgramLaunch>),
    ScopedFilesystemExecution, // Execute against scoped real filesystem read/write roots.
    UnscopedFilesystemExecution, // Execute against the real filesystem without path grants.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestrictedBuildGrantRoot {
    SourceInventory, // Package's source inventory.
    StagedOutput,    // Activation's staged build-output tree.
    Other(u32),      // Compiler-issued grant root identity.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildGrant {
    root: RestrictedBuildGrantRoot,
    narrowed: bool, // Canonical metadata index narrows every operation to captured membership and kind.
    captured: Option<BuildCapturedSourceInventory>, // Immutable captured inventory bound to the root, when one exists.
}

impl RestrictedBuildGrant {
    #[doc(hidden)]
    pub const fn from_retained(
        root: RestrictedBuildGrantRoot,
        narrowed: bool,
        captured: Option<BuildCapturedSourceInventory>,
    ) -> Self {
        Self {
            root,
            narrowed,
            captured,
        }
    }

    pub const fn root(&self) -> RestrictedBuildGrantRoot {
        self.root
    }

    pub const fn narrowed(&self) -> bool {
        self.narrowed
    }

    pub const fn captured(&self) -> Option<BuildCapturedSourceInventory> {
        self.captured
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildBounds {
    filesystem_sponsor_limits: Option<filesystem_authority::FilesystemSponsorLimits>,
    evaluation_sponsor_limits: Option<BuildEvaluationSponsorLimits>,
    required_outputs: Vec<Vec<u8>>,
    artifact_only: bool,
}

impl RestrictedBuildBounds {
    #[doc(hidden)]
    pub const fn from_retained(
        filesystem_sponsor_limits: Option<filesystem_authority::FilesystemSponsorLimits>,
        evaluation_sponsor_limits: Option<BuildEvaluationSponsorLimits>,
        required_outputs: Vec<Vec<u8>>,
        artifact_only: bool,
    ) -> Self {
        Self {
            filesystem_sponsor_limits,
            evaluation_sponsor_limits,
            required_outputs,
            artifact_only,
        }
    }

    pub const fn filesystem_sponsor_limits(
        &self,
    ) -> Option<filesystem_authority::FilesystemSponsorLimits> {
        self.filesystem_sponsor_limits
    }

    pub const fn evaluation_sponsor_limits(&self) -> Option<BuildEvaluationSponsorLimits> {
        self.evaluation_sponsor_limits
    }

    pub fn required_outputs(&self) -> &[Vec<u8>] {
        &self.required_outputs
    }

    pub const fn artifact_only(&self) -> bool {
        self.artifact_only
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramDeclarationOwner {
    /// The request row is already scoped to the exact requesting occurrence.
    RequestingSource,
    Package([u8; 32]),
    Toolchain([u8; 32]),
}

/// Portable review meaning for one demanded program. The installed image
/// digest binds its runtime too on hosts admitting only static executables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedProgramLaunch {
    pub owner: ProgramDeclarationOwner,
    pub declaration: String,
    pub file: String,
    pub image_sha256: [u8; 32],
    pub readable: Vec<Vec<u8>>,
    pub writable: Vec<Vec<u8>>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildRequest {
    operation: RestrictedBuildOperation,
    read_grants: Vec<RestrictedBuildGrant>,
    write_grants: Vec<RestrictedBuildGrant>,
    bounds: RestrictedBuildBounds,
    build_execution_profile: Option<target::TargetProfile>,
    selected_target_profile: Option<target::TargetProfile>,
}

impl RestrictedBuildRequest {
    pub fn for_program(
        program: RestrictedProgramLaunch,
        bounds: RestrictedBuildBounds,
        profile: Option<target::TargetProfile>,
    ) -> Self {
        Self {
            operation: RestrictedBuildOperation::ProgramLaunch(Box::new(program)),
            read_grants: Vec::new(),
            write_grants: Vec::new(),
            bounds,
            build_execution_profile: profile,
            selected_target_profile: None,
        }
    }

    pub const fn operation(&self) -> &RestrictedBuildOperation {
        &self.operation
    }

    pub fn read_grants(&self) -> &[RestrictedBuildGrant] {
        &self.read_grants
    }

    pub fn write_grants(&self) -> &[RestrictedBuildGrant] {
        &self.write_grants
    }

    pub const fn bounds(&self) -> &RestrictedBuildBounds {
        &self.bounds
    }

    pub const fn build_execution_profile(&self) -> Option<target::TargetProfile> {
        self.build_execution_profile
    }

    pub const fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.selected_target_profile
    }

    #[doc(hidden)]
    pub const fn from_retained(
        operation: RestrictedBuildOperation,
        read_grants: Vec<RestrictedBuildGrant>,
        write_grants: Vec<RestrictedBuildGrant>,
        bounds: RestrictedBuildBounds,
        build_execution_profile: Option<target::TargetProfile>,
        selected_target_profile: Option<target::TargetProfile>,
    ) -> Self {
        Self {
            operation,
            read_grants,
            write_grants,
            bounds,
            build_execution_profile,
            selected_target_profile,
        }
    }

    #[must_use]
    pub const fn for_target(mut self, profile: Option<target::TargetProfile>) -> Self {
        self.selected_target_profile = profile;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use target::TargetProfile::{LinuxArm64, LinuxX64};

    #[test]
    fn assigning_a_product_target_preserves_the_required_grants_and_execution_profile() {
        let request = RestrictedBuildRequest::from_retained(
            RestrictedBuildOperation::ScopedFilesystemExecution,
            vec![RestrictedBuildGrant::from_retained(
                RestrictedBuildGrantRoot::SourceInventory,
                true,
                None,
            )],
            vec![RestrictedBuildGrant::from_retained(
                RestrictedBuildGrantRoot::StagedOutput,
                false,
                None,
            )],
            RestrictedBuildBounds::from_retained(None, None, vec![b"release.img".to_vec()], true),
            Some(LinuxX64),
            None,
        );
        let scoped = request.clone().for_target(Some(LinuxArm64));
        assert_eq!(scoped.selected_target_profile(), Some(LinuxArm64));
        assert_eq!(scoped.build_execution_profile(), Some(LinuxX64));
        assert_eq!(scoped.read_grants(), request.read_grants());
        assert_eq!(scoped.write_grants(), request.write_grants());
        assert_eq!(scoped.bounds(), request.bounds());
    }
}
