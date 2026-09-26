//! Interpret authored paths from fresh requester locations, never lock paths.

use super::super::dependencies::{normalize_member_path, workspace_path_escapes};
use super::{ResolveLockedPackageClosureError as Error, Resolver};
use crate::declarations::{PackageName, PackageSelection};
use crate::resolution::graph::{PackageRootSourceRequest, ResolvedSourceIdentity};
use crate::resolution::source::workspace_path::source_relative_path;
use crate::resolution::source::{
    GitPackageSourceRequest, PackageSourceCustody, PackageSourceNavigation,
    resolve_external_local_package_source_in_lane, resolve_workspace_member_package_source_in_lane,
};
use package_source::SourceLineage;
use std::path::Path;

impl Resolver<'_> {
    pub(super) fn path(
        &mut self,
        requester: &PackageSourceCustody,
        location: &str,
        expected: &ResolvedSourceIdentity,
    ) -> Result<PackageSourceCustody, Error> {
        match requester.key().source_lineage() {
            SourceLineage::ExternalLocal(_) => self.external_path(requester, location, expected),
            SourceLineage::Workspace(lineage) => {
                let base = Some(lineage.member_path().as_str());
                match normalize_member_path(base, location) {
                    Ok(member) => {
                        let PackageRootSourceRequest::WorkspaceMember {
                            workspace_root_source,
                            ..
                        } = self.root_request
                        else {
                            return Err(Error::mismatch(
                                requester.key(),
                                "local workspace has no explicit caller root",
                            ));
                        };
                        let root = self.local_workspace_root.as_ref().ok_or_else(|| {
                            Error::mismatch(
                                requester.key(),
                                "local workspace has no stable canonical root",
                            )
                        })?;
                        let resolved = resolve_workspace_member_package_source_in_lane(
                            workspace_root_source,
                            source_relative_path(&member),
                            root,
                            self.storage.workspace_members(),
                            self.source_limits,
                        )?;
                        self.register_local_root(
                            resolved.key(),
                            resolved.source().canonical_live_root(),
                        )?;
                        Ok(resolved.into_custody())
                    }
                    Err(_) if workspace_path_escapes(base, location) => {
                        self.external_path(requester, location, expected)
                    }
                    Err(error) => Err(error.into()),
                }
            }
            SourceLineage::GitHub(_) | SourceLineage::GitLab(_) | SourceLineage::Git(_) => {
                self.git_path(requester, location, expected)
            }
        }
    }

    fn external_path(
        &mut self,
        requester: &PackageSourceCustody,
        location: &str,
        expected: &ResolvedSourceIdentity,
    ) -> Result<PackageSourceCustody, Error> {
        if location.is_empty() || location.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(Error::mismatch(
                requester.key(),
                "invalid external-local path",
            ));
        }
        let SourceLineage::ExternalLocal(selected) = expected.key().source_lineage() else {
            return Err(Error::mismatch(
                expected.key(),
                "external path has a non-local recorded selection",
            ));
        };
        // This explicit locked operation reconstructs the recorded consuming
        // context only after matching the fresh complete projection and exact
        // authored edge. It is identity, not a persisted filesystem capability.
        // All external edges still share one context, as in ordinary traversal.
        if let Some(context) = &self.external_context {
            if context != selected.source_context() {
                return Err(Error::mismatch(
                    expected.key(),
                    "external path consuming context differs",
                ));
            }
        } else {
            self.external_context = Some(selected.source_context().clone());
        }
        let root = self.local_roots.get(requester.key()).ok_or_else(|| {
            Error::mismatch(
                requester.key(),
                "external path has no fresh live requester root",
            )
        })?;
        let authored = Path::new(location);
        let path = if authored.is_absolute() {
            authored.to_path_buf()
        } else {
            root.join(authored)
        };
        let resolved = resolve_external_local_package_source_in_lane(
            path,
            self.storage.external_local_sources(),
            self.source_limits,
            selected.source_context().clone(),
        )?;
        self.register_local_root(resolved.key(), resolved.source().canonical_live_root())?;
        Ok(resolved.into_custody())
    }

    fn git_path(
        &mut self,
        requester: &PackageSourceCustody,
        location: &str,
        expected: &ResolvedSourceIdentity,
    ) -> Result<PackageSourceCustody, Error> {
        let base = match requester.navigation() {
            PackageSourceNavigation::Root => None,
            PackageSourceNavigation::Member(path) => Some(path.as_str()),
        };
        let member_path = normalize_member_path(base, location)?;
        let plan = requester
            .selection_evidence()
            .git_workspace()
            .ok_or_else(|| {
                Error::mismatch(requester.key(), "Git path has no fresh declared workspace")
            })?;
        let member = plan
            .members()
            .iter()
            .find(|member| member.member_path() == &member_path)
            .ok_or_else(|| {
                Error::mismatch(
                    requester.key(),
                    "Git path selects an undeclared workspace member",
                )
            })?;
        if expected.key().source_lineage() != requester.key().source_lineage()
            || expected.resolution() != requester.resolution()
            || self.subject.package_navigation(expected.key())
                != Some(&PackageSourceNavigation::Member(source_relative_path(
                    &member_path,
                )))
            || expected.key().name().as_str() != member.package_name().as_str()
        {
            return Err(Error::mismatch(
                expected.key(),
                "relative Git member differs from recorded selection",
            ));
        }
        let acquisition = self
            .git_requests
            .get(requester.key())
            .ok_or_else(|| {
                Error::mismatch(requester.key(), "Git path has no fresh acquisition request")
            })?
            .clone();
        let name = PackageName::parse(member.package_name().as_str()).map_err(|_| {
            Error::mismatch(
                expected.key(),
                "declared Git member has an invalid package name",
            )
        })?;
        let request = GitPackageSourceRequest::new(acquisition, PackageSelection::Named(name));
        self.git(&request, expected, false)
    }
}
