// Normalized restricted build-host requests (`RestrictedBuildRequest`,
// `RestrictedBuildOperation`, `RestrictedBuildGrant`, `RestrictedBuildBounds`,
// `RestrictedBuildGrants`) projected from `BuildMachineFilesystemScope` before
// execution. Reports logical grant roots, captured inventories, and sponsor
// limits in package vocabulary without exposing physical host paths or live
// capabilities.

use super::BuildCapturedSourceInventory;
use super::{BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope};
use build_time_evaluation::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildMachineExecutionMode,
    BuildMachineFilesystemAccess, BuildMachineFilesystemGrantRoot,
};

pub trait RestrictedBuildGrants: Send {
    fn admit(
        &mut self,
        request: &RestrictedBuildRequest,
    ) -> Result<(), Vec<diagnostics::Diagnostic>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestrictedBuildOperation {
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
    pub const fn operation(&self) -> RestrictedBuildOperation {
        self.operation
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

impl BuildMachineFilesystemScope {
    pub fn restricted_build_requests(
        &self,
        evaluation_sponsor: Option<&BuildEvaluationSponsor>,
        artifact_only: bool,
        execution_mode: &BuildMachineExecutionMode,
    ) -> Vec<RestrictedBuildRequest> {
        let BuildMachineExecutionMode::Granted { filesystem, .. } = execution_mode else {
            return Vec::new();
        };
        if self.is_private_snapshot_execution(filesystem) {
            return Vec::new();
        }
        let (operation, read_roots, write_roots, filesystem_sponsor) = match filesystem {
            BuildMachineFilesystemAccess::RealScoped(grants) => (
                RestrictedBuildOperation::ScopedFilesystemExecution,
                &grants.read_roots[..],
                &grants.write_roots[..],
                None,
            ),
            BuildMachineFilesystemAccess::RealScopedSponsored { grants, sponsor } => (
                RestrictedBuildOperation::ScopedFilesystemExecution,
                &grants.read_roots[..],
                &grants.write_roots[..],
                Some(sponsor),
            ),
            BuildMachineFilesystemAccess::CapturedScoped {
                grants, sponsor, ..
            } => (
                RestrictedBuildOperation::ScopedFilesystemExecution,
                &grants.read_roots[..],
                &grants.write_roots[..],
                sponsor.as_ref(),
            ),
            BuildMachineFilesystemAccess::RealUnscoped => (
                RestrictedBuildOperation::UnscopedFilesystemExecution,
                &[][..],
                &[][..],
                None,
            ),
            BuildMachineFilesystemAccess::Virtual => return Vec::new(),
        };
        let captured = self.captured_source_inventory();
        let grant = |grant_root: &BuildMachineFilesystemGrantRoot| {
            let identity = grant_root.identity();
            let (root, captured) = if identity == BUILD_SOURCE_ROOT_IDENTITY {
                (RestrictedBuildGrantRoot::SourceInventory, captured)
            } else if identity == BUILD_OUTPUT_ROOT_IDENTITY {
                (RestrictedBuildGrantRoot::StagedOutput, None)
            } else {
                (
                    RestrictedBuildGrantRoot::Other(identity.get()),
                    self.named_input_inventory(identity),
                )
            };
            RestrictedBuildGrant {
                root,
                narrowed: grant_root.canonical_metadata().is_some(),
                captured,
            }
        };
        let mut read_grants: Vec<_> = read_roots.iter().map(&grant).collect();
        if let BuildMachineFilesystemAccess::CapturedScoped { inputs, .. } = filesystem {
            read_grants.extend(inputs.iter().map(|input| RestrictedBuildGrant {
                root: if input.identity == BUILD_SOURCE_ROOT_IDENTITY {
                    RestrictedBuildGrantRoot::SourceInventory
                } else {
                    RestrictedBuildGrantRoot::Other(input.identity.get())
                },
                narrowed: true,
                captured: if input.identity == BUILD_SOURCE_ROOT_IDENTITY {
                    captured
                } else {
                    self.named_input_inventory(input.identity)
                },
            }));
        }
        vec![RestrictedBuildRequest {
            operation,
            read_grants,
            write_grants: write_roots.iter().map(&grant).collect(),
            bounds: RestrictedBuildBounds {
                filesystem_sponsor_limits: filesystem_sponsor
                    .and_then(|sponsor| sponsor.limits().ok()),
                evaluation_sponsor_limits: evaluation_sponsor.map(BuildEvaluationSponsor::limits),
                required_outputs: self.required_outputs().iter().cloned().collect(),
                artifact_only,
            },
            build_execution_profile: self.activation().build_execution_profile(),
            selected_target_profile: None,
        }]
    }
}
