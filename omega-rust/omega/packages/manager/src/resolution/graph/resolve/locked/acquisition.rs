//! Fresh source-owner acquisition; recorded IDs are only equality expectations.

use super::{ResolveLockedPackageClosureError as Error, Resolver};
use crate::resolution::graph::{PackageRootSourceRequest, ResolvedSourceIdentity};
use crate::resolution::source::{
    GitPackageSourceRequest, PackageSourceCustody, ResolvePackageSourceError,
    resolve_external_local_project_source_in_lane,
    resolve_selected_git_package_source_at_revision_in_lanes,
    resolve_selected_git_project_source_at_revision_in_lanes,
    resolve_workspace_member_project_source_in_lane,
};
use package_source::ImmutableSourceResolution;

impl Resolver<'_> {
    pub(super) fn root(&mut self) -> Result<PackageSourceCustody, Error> {
        match self.root_request {
            PackageRootSourceRequest::Git(request) => {
                self.git(request, self.subject.root().selected(), true)
            }
            PackageRootSourceRequest::ExternalLocal {
                requested_root,
                source_context,
            } => {
                let resolved = resolve_external_local_project_source_in_lane(
                    requested_root,
                    self.storage.external_local_sources(),
                    self.source_limits,
                    source_context.clone(),
                )?;
                self.register_local_root(resolved.key(), resolved.source().canonical_live_root())?;
                Ok(resolved.into_custody())
            }
            PackageRootSourceRequest::WorkspaceMember {
                workspace_root_source,
                member_path,
                requested_workspace_root,
            } => {
                let canonical_root = requested_workspace_root.canonicalize().map_err(|error| {
                    ResolvePackageSourceError::WorkspacePath {
                        path: requested_workspace_root.clone(),
                        message: error.to_string(),
                    }
                })?;
                let resolved = resolve_workspace_member_project_source_in_lane(
                    workspace_root_source,
                    member_path.clone(),
                    &canonical_root,
                    self.storage.workspace_members(),
                    self.source_limits,
                )?;
                self.register_local_root(resolved.key(), resolved.source().canonical_live_root())?;
                self.local_workspace_root = Some(canonical_root);
                Ok(resolved.into_custody())
            }
        }
    }

    pub(super) fn register_local_root(
        &mut self,
        package: &crate::declarations::PackageKey,
        root: &std::path::Path,
    ) -> Result<(), Error> {
        if let Some(existing) = self.local_roots.get(package) {
            if existing != root {
                return Err(Error::mismatch(
                    package,
                    "fresh canonical live root conflicts with prior custody",
                ));
            }
        } else {
            self.local_roots.insert(package.clone(), root.to_path_buf());
        }
        Ok(())
    }

    pub(super) fn git(
        &mut self,
        request: &GitPackageSourceRequest,
        expected: &ResolvedSourceIdentity,
        project: bool,
    ) -> Result<PackageSourceCustody, Error> {
        if request.acquisition().lineage() != expected.key().source_lineage() {
            return Err(Error::mismatch(
                expected.key(),
                "Git request lineage differs before acquisition",
            ));
        }
        let ImmutableSourceResolution::Git { commit, tree, .. } = expected.resolution() else {
            return Err(Error::mismatch(
                expected.key(),
                "Git request has a non-Git recorded resolution",
            ));
        };
        let resolved = if project {
            resolve_selected_git_project_source_at_revision_in_lanes(
                request,
                commit,
                tree,
                self.acquisition,
                self.storage.git_sources(),
                self.storage.workspace_members(),
                self.source_limits,
            )
        } else {
            resolve_selected_git_package_source_at_revision_in_lanes(
                request,
                commit,
                tree,
                self.acquisition,
                self.storage.git_sources(),
                self.storage.workspace_members(),
                self.source_limits,
            )
        }?;
        super::super::dependencies::register_git_repository(
            &mut self.workspaces,
            request.acquisition(),
            resolved.key().source_lineage(),
            resolved.resolution(),
            resolved.selection_evidence(),
            resolved.source_limits(),
        )?;
        self.git_requests
            .insert(resolved.key().clone(), request.acquisition().clone());
        Ok(resolved.into_custody())
    }
}
