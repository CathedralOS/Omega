// Normalized restricted build-host requests (`RestrictedBuildRequest`,
// `RestrictedBuildOperation`, `RestrictedBuildGrant`, `RestrictedBuildBounds`,
// `RestrictedBuildGrants`) projected from `BuildMachineFilesystemScope` before
// execution. Reports logical grant roots, captured inventories, and sponsor
// limits in package vocabulary without exposing physical host paths or live
// capabilities.

use super::{BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope};
use super::{BuildMachineExecutionMode, BuildMachineFilesystemAccess};
use build_filesystem::authority::FilesystemGrantRoot;
pub use build_filesystem::consent::{
    ProgramDeclarationOwner, RestrictedBuildBounds, RestrictedBuildGrant, RestrictedBuildGrantRoot,
    RestrictedBuildOperation, RestrictedBuildRequest, RestrictedBuildSite, RestrictedBuildSubject,
    RestrictedProgramLaunch,
};
use build_model::accounting::BuildEvaluationSponsor;

pub trait RestrictedBuildGrants: Send {
    fn subject(&self) -> Option<RestrictedBuildSubject> {
        None
    }

    fn admits(
        &mut self,
        site: &RestrictedBuildSite,
        request: &RestrictedBuildRequest,
    ) -> Result<bool, Vec<diagnostics::Diagnostic>>;
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
        let grant = |grant_root: &FilesystemGrantRoot| {
            let identity = grant_root.identity();
            let (root, captured) = if identity == BUILD_SOURCE_ROOT_IDENTITY {
                (RestrictedBuildGrantRoot::SourceInventory, captured)
            } else if identity == BUILD_OUTPUT_ROOT_IDENTITY {
                (RestrictedBuildGrantRoot::StagedOutput, None)
            } else {
                (RestrictedBuildGrantRoot::Other(identity.get()), None)
            };
            RestrictedBuildGrant::from_retained(
                root,
                grant_root.canonical_metadata().is_some(),
                captured,
            )
        };
        let mut read_grants: Vec<_> = read_roots.iter().map(&grant).collect();
        if let BuildMachineFilesystemAccess::CapturedScoped { source, .. } = filesystem {
            read_grants.push(RestrictedBuildGrant::from_retained(
                RestrictedBuildGrantRoot::SourceInventory,
                true,
                Some(source.inventory()),
            ));
        }
        let bounds = RestrictedBuildBounds::from_retained(
            filesystem_sponsor.and_then(|sponsor| sponsor.limits().ok()),
            evaluation_sponsor.map(BuildEvaluationSponsor::limits),
            self.required_outputs().iter().cloned().collect(),
            artifact_only,
        );
        vec![RestrictedBuildRequest::from_retained(
            operation,
            read_grants,
            write_roots.iter().map(&grant).collect(),
            bounds,
            self.activation().build_execution_profile(),
            None,
        )]
    }
}
