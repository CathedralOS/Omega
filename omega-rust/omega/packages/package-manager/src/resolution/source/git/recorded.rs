//! Recover authenticated historical Git bytes under their accepted metadata.
//! This issues no fresh Build declaration and never interprets old source text.

use crate::declarations::dependencies::read::DependencyProjections;
use crate::declarations::{BuildDeclarationKind, PackageKey};
use crate::resolution::source::{
    PackageSourceCustody, PackageSourceMaterialization, PackageSourceNavigation,
    PackageSourceSelectionEvidence, ResolvePackageSourceError,
};
use package_source::git::resolution::{
    GitExactRevisionAcquisition, GitWorkspaceSession, resolve_git_source_at_revision_in_lane,
};
use package_source::{
    GitSourceRequest, ImmutableSourceResolution, LocalSourceLimits, SourceResolver,
};
use std::sync::Arc;

/// The accepted closure supplies these together at one immutable resolution.
/// Names and ledgers are not rediscovered from historical code or today's Build.
pub(crate) struct RecordedGitPackage<'record> {
    pub key: &'record PackageKey,
    pub resolution: &'record ImmutableSourceResolution,
    pub navigation: &'record PackageSourceNavigation,
    pub role: BuildDeclarationKind,
    pub dependencies: &'record DependencyProjections,
}

pub(crate) fn recover_recorded_git_source(
    storage: &SourceResolver,
    request: &GitSourceRequest,
    recorded: RecordedGitPackage<'_>,
    acquisition: GitExactRevisionAcquisition,
) -> Result<PackageSourceCustody, ResolvePackageSourceError> {
    let refuse = |message: &str| ResolvePackageSourceError::RecordedGitPin {
        locator: request.locator_identity().to_owned(),
        revision: request.requested_revision().to_owned(),
        message: message.to_owned(),
    };
    if request.lineage() != recorded.key.source_lineage() {
        return Err(refuse(
            "recorded Git request lineage differs from its accepted package",
        ));
    }
    let ImmutableSourceResolution::Git { commit, tree, .. } = recorded.resolution else {
        return Err(refuse(
            "historical Git recovery requires its exact Git resolution",
        ));
    };
    let limits = LocalSourceLimits::default().compiler_bounded();
    storage.verify_path_identity()?;
    let source = match recorded.navigation {
        PackageSourceNavigation::Root => resolve_git_source_at_revision_in_lane(
            storage.primary_git()?,
            request,
            commit,
            tree,
            acquisition,
            storage.git_sources(),
            limits,
        )?,
        PackageSourceNavigation::Member(path) => {
            // The accepted subject already selected this exact path at this pin.
            // SDK tree inspection authenticates it; no declaration parser or
            // another Build invocation supplies historical selection semantics.
            GitWorkspaceSession::open_at_revision(
                storage,
                request,
                commit,
                tree,
                acquisition,
                limits,
                super::workspace::git_workspace_declaration_limits(),
            )?
            .open_members(storage, std::slice::from_ref(path))?
            .materialize(storage, path)?
        }
    };
    storage.verify_path_identity()?;
    let navigation_agrees = match recorded.navigation {
        PackageSourceNavigation::Root => source.selected_member().is_none(),
        PackageSourceNavigation::Member(path) => source.selected_member() == Some(path),
    };
    if source.lineage() != recorded.key.source_lineage()
        || source.commit() != commit.to_hex()
        || source.tree() != tree.to_hex()
        || !navigation_agrees
    {
        return Err(refuse(
            "recovered Git custody differs from its accepted lineage, pin, or navigation",
        ));
    }
    let materialization = PackageSourceMaterialization::from_local(source.local());
    let root = source.snapshot_root().to_path_buf();
    let limits = source.source_limits();
    Ok(PackageSourceCustody::from_resolved_parts(
        recorded.key.clone(),
        recorded.role,
        recorded.resolution.clone(),
        materialization,
        root,
        recorded.navigation.clone(),
        PackageSourceSelectionEvidence::HistoricalGit {
            _source: Arc::new(source),
        },
        limits,
        recorded.dependencies.clone(),
    ))
}
