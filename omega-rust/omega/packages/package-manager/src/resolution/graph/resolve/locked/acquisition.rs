//! Fresh source-owner acquisition; recorded IDs are only equality expectations.

use super::super::cache::SourceCacheLane;
use super::{ResolveLockedPackageClosureError as Error, Resolver, RootPolicy};
use crate::resolution::graph::{PackageRootSourceRequest, ResolvedSourceIdentity};
use crate::resolution::source::{
    GitPackageSourceRequest, PackageSourceCustody, ResolvePackageSourceError, SourceRootRole,
    capture_external_local_project_source, resolve_external_local_source_in_lane,
    resolve_selected_git_source_at_revision_in_lanes, resolve_workspace_member_source_in_lane,
};
use package_source::ImmutableSourceResolution;

impl Resolver<'_> {
    pub(super) fn root(&mut self) -> Result<PackageSourceCustody, Error> {
        match self.root_request {
            PackageRootSourceRequest::Git(request) => {
                let custody = self.git(
                    request,
                    self.subject.root().selected(),
                    SourceRootRole::Project,
                )?;
                let cache_dir = SourceCacheLane::Retained(self.storage.git_sources())
                    .checked_source_cache_dir()?;
                let review_cache_dir = SourceCacheLane::Retained(self.storage.git_sources())
                    .compiled_review_cache_dir()?;
                Ok(custody
                    .with_checked_source_cache_dir(cache_dir)
                    .with_compiled_review_cache_dir(review_cache_dir))
            }
            PackageRootSourceRequest::ExternalLocal {
                requested_root,
                source_context,
            } => {
                let (custody, canonical_root) =
                    if matches!(self.root_policy, RootPolicy::MutableLocal) {
                        let custody = capture_external_local_project_source(
                            requested_root,
                            self.source_limits,
                            source_context.clone(),
                        )?;
                        let root = custody.snapshot_root().to_path_buf();
                        (custody, root)
                    } else {
                        let resolved = resolve_external_local_source_in_lane(
                            requested_root,
                            self.storage.external_local_sources(),
                            self.source_limits,
                            source_context.clone(),
                            SourceRootRole::Project,
                        )?;
                        let root = resolved.source().canonical_live_root().to_path_buf();
                        (resolved.into_custody(), root)
                    };
                self.register_local_root(custody.key(), &canonical_root)?;
                let cache_dir = SourceCacheLane::Retained(self.storage.external_local_sources())
                    .checked_source_cache_dir()?;
                let review_cache_dir =
                    SourceCacheLane::Retained(self.storage.external_local_sources())
                        .compiled_review_cache_dir()?;
                Ok(custody
                    .with_checked_source_cache_dir(cache_dir)
                    .with_compiled_review_cache_dir(review_cache_dir))
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
                let resolved = resolve_workspace_member_source_in_lane(
                    workspace_root_source,
                    member_path.clone(),
                    &canonical_root,
                    self.storage.workspace_members(),
                    self.source_limits,
                    SourceRootRole::Project,
                )?;
                self.register_local_root(resolved.key(), resolved.source().canonical_live_root())?;
                self.local_workspace_root = Some(canonical_root);
                let cache_dir = SourceCacheLane::Retained(self.storage.workspace_members())
                    .checked_source_cache_dir()?;
                let review_cache_dir = SourceCacheLane::Retained(self.storage.workspace_members())
                    .compiled_review_cache_dir()?;
                Ok(resolved
                    .into_custody()
                    .with_checked_source_cache_dir(cache_dir)
                    .with_compiled_review_cache_dir(review_cache_dir))
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
        role: SourceRootRole,
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
        let resolved = resolve_selected_git_source_at_revision_in_lanes(
            self.storage,
            request,
            commit,
            tree,
            self.acquisition,
            self.storage.git_sources(),
            self.storage.workspace_members(),
            self.source_limits,
            role,
        )?;
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
        Ok(resolved.into_custody().with_compiled_review_cache_dir(
            SourceCacheLane::Retained(self.storage.git_sources()).compiled_review_cache_dir()?,
        ))
    }
}
