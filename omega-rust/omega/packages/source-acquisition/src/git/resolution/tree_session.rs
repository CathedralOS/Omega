//! Staged workspace discovery over one authenticated, pinned Git tree.
//!
//! Build execution may suspend between reading the root catalog, requesting
//! member declarations and selecting a package. Only metadata and requested
//! declarations cross those pauses; unselected package payloads stay unopened.

use super::GitExactRevisionAcquisition;
use super::acquisition::{
    resolve_git_source_from_retained_cache, with_git_repository_from_retained_cache,
};
use super::selection::{GitRevisionSelection, RecordedGitRevision};
use super::workspace_member::{declarations, materialize_member};
use crate::git::executable::selection::resolver_package_controlled_roots;
use crate::git::objects::{AuthenticatedGitTreeInspection, inspect_git_tree_graph};
use crate::git::request::GitSourceRequest;
use crate::git::workspace::{
    GitWorkspaceDeclaration, GitWorkspaceDeclarationLimits, GitWorkspaceProjectionError,
};
use crate::{
    GitCommitId, GitTreeId, LocalSourceLimits, ResolvedGitSource, SourceRelativePath,
    SourceResolveError, SourceResolver,
};
use std::convert::Infallible;

#[derive(Debug)]
pub struct GitWorkspaceSession {
    request: GitSourceRequest,
    commit: String,
    tree: String,
    inspection: AuthenticatedGitTreeInspection,
    root: GitWorkspaceDeclaration,
    limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
}

/// Exact declaration roster requested after the root Build returns. Selection
/// remains the caller's semantic decision; this type establishes source custody.
#[derive(Debug)]
pub struct GitWorkspaceCatalog {
    session: GitWorkspaceSession,
    members: Vec<GitWorkspaceDeclaration>,
}

impl GitWorkspaceSession {
    pub fn open(
        storage: &SourceResolver,
        request: &GitSourceRequest,
        pin: Option<&crate::GitAcquisitionPin>,
        limits: LocalSourceLimits,
        declaration_limits: GitWorkspaceDeclarationLimits,
    ) -> Result<Self, SourceResolveError> {
        Self::acquire(
            storage,
            request,
            limits,
            declaration_limits,
            GitRevisionSelection::Ordinary(pin),
        )
    }

    pub fn open_at_revision(
        storage: &SourceResolver,
        request: &GitSourceRequest,
        commit: &GitCommitId,
        tree: &GitTreeId,
        acquisition: GitExactRevisionAcquisition,
        limits: LocalSourceLimits,
        declaration_limits: GitWorkspaceDeclarationLimits,
    ) -> Result<Self, SourceResolveError> {
        let revision = RecordedGitRevision::new(request, commit, tree, acquisition)?;
        Self::acquire(
            storage,
            request,
            limits,
            declaration_limits,
            GitRevisionSelection::Recorded(&revision),
        )
    }

    fn acquire(
        storage: &SourceResolver,
        request: &GitSourceRequest,
        limits: LocalSourceLimits,
        declaration_limits: GitWorkspaceDeclarationLimits,
        selection: GitRevisionSelection<'_>,
    ) -> Result<Self, SourceResolveError> {
        let limits = limits.compiler_bounded();
        storage.verify_path_identity()?;
        let lane = storage.git_sources();
        let controlled_roots = resolver_package_controlled_roots(&[lane.path()])?;
        let result = with_git_repository_from_retained_cache(
            storage.primary_git()?,
            &controlled_roots,
            request,
            lane.path(),
            lane.directory(),
            limits,
            selection,
            |executor, repository, commit, tree, limits| {
                let inspection = inspect_git_tree_graph(executor, repository, tree)?;
                let root = declarations::open_root(
                    &inspection,
                    executor,
                    repository,
                    limits,
                    declaration_limits,
                )?;
                Ok(Self {
                    request: request.clone(),
                    commit: commit.to_owned(),
                    tree: tree.to_owned(),
                    inspection,
                    root,
                    limits,
                    declaration_limits,
                })
            },
            |evidence, _lock, _root| Ok(evidence),
        );
        storage.verify_path_identity()?;
        result.map_err(source_error)
    }

    pub fn acquisition_pin(&self) -> crate::GitAcquisitionPin {
        crate::GitAcquisitionPin::authenticated_revision(
            &self.request,
            self.commit.clone(),
            self.tree.clone(),
        )
    }
    pub fn source_limits(&self) -> LocalSourceLimits {
        self.limits
    }

    pub fn request(&self) -> &GitSourceRequest {
        &self.request
    }
    pub fn commit(&self) -> &str {
        &self.commit
    }
    pub fn tree(&self) -> &str {
        &self.tree
    }
    pub fn root_declaration(&self) -> &GitWorkspaceDeclaration {
        &self.root
    }

    pub fn open_members(
        self,
        storage: &SourceResolver,
        paths: &[SourceRelativePath],
    ) -> Result<GitWorkspaceCatalog, SourceResolveError> {
        storage.verify_path_identity()?;
        let lane = storage.git_sources();
        let controlled_roots = resolver_package_controlled_roots(&[lane.path()])?;
        let revision = self.recorded_revision()?;
        let result = with_git_repository_from_retained_cache(
            storage.primary_git()?,
            &controlled_roots,
            &self.request,
            lane.path(),
            lane.directory(),
            self.limits,
            GitRevisionSelection::Recorded(&revision),
            |executor, repository, _commit, _tree, limits| {
                declarations::open_members(
                    &self.inspection,
                    executor,
                    repository,
                    &self.root,
                    paths,
                    limits,
                    self.declaration_limits,
                )
                .map_err(GitWorkspaceProjectionError::Source)
            },
            |evidence, _lock, _root| Ok(evidence),
        );
        storage.verify_path_identity()?;
        let members = result.map_err(source_error)?;
        Ok(GitWorkspaceCatalog {
            session: self,
            members,
        })
    }

    fn recorded_revision(&self) -> Result<RecordedGitRevision, SourceResolveError> {
        // These strings were authenticated at acquisition; parse failures still
        // remain explicit rather than turning resolver invariants into panics.
        let commit = GitCommitId::parse_hex(&self.commit).map_err(|error| {
            SourceResolveError::GitObjectInvalid {
                oid: self.commit.clone(),
                message: error.to_string(),
            }
        })?;
        let tree = GitTreeId::parse_hex(&self.tree).map_err(|error| {
            SourceResolveError::GitObjectInvalid {
                oid: self.tree.clone(),
                message: error.to_string(),
            }
        })?;
        RecordedGitRevision::new(
            &self.request,
            &commit,
            &tree,
            GitExactRevisionAcquisition::Offline,
        )
    }
}

impl GitWorkspaceCatalog {
    pub fn session(&self) -> &GitWorkspaceSession {
        &self.session
    }
    pub fn member_declarations(&self) -> &[GitWorkspaceDeclaration] {
        &self.members
    }

    pub fn materialize(
        &self,
        storage: &SourceResolver,
        selected: &SourceRelativePath,
    ) -> Result<ResolvedGitSource, SourceResolveError> {
        if !self
            .members
            .iter()
            .any(|member| member.member_path() == Some(selected))
        {
            return Err(SourceResolveError::GitTreeInvalid {
                path: selected.as_str().as_bytes().to_vec(),
                message: "selected member is absent from the acquired declaration roster"
                    .to_owned(),
            });
        }
        storage.verify_path_identity()?;
        let git_lane = storage.git_sources();
        let member_lane = storage.workspace_members();
        let revision = self.session.recorded_revision()?;
        let result = resolve_git_source_from_retained_cache(
            storage.primary_git()?,
            &[git_lane, member_lane],
            &self.session.request,
            self.session.limits,
            GitRevisionSelection::Recorded(&revision),
            |executor, repository, _tree, limits| {
                materialize_member(
                    &self.session.inspection,
                    executor,
                    repository,
                    member_lane,
                    limits,
                    self.session.root.clone(),
                    self.members.clone(),
                    selected.clone(),
                    (),
                )
            },
        );
        storage.verify_path_identity()?;
        result.map(|(source, ())| source).map_err(source_error)
    }
}

fn source_error(error: GitWorkspaceProjectionError<Infallible>) -> SourceResolveError {
    match error {
        GitWorkspaceProjectionError::Source(error) => error,
        GitWorkspaceProjectionError::Planner(never) => match never {},
    }
}
