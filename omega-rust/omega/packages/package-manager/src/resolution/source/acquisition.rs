//! Acquire immutable sources, then bind the declarations their Builds execute.

use super::build_resolution::{BuildSourceAcquisition, PendingBuildSource};
use crate::declarations::dependencies::read::{DependencySourceRequest, PackageSelection};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, GitResolutionOptions, admit_locked_source_request,
};
use package_source::{GitSourceRequest, SourceResolver};

use super::{DeclaredBuildSource, GitPackageSourceRequest, PackageSourceCustody};
use crate::declarations::PackageKey;
use crate::resolution::graph::PackageRootSourceRequest;
use package_source::local::resolution_observations::CapturedLocalSource;
use package_source::{
    ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitTreeId, ImmutableSourceResolution,
    LocalSourceLimits, ResolvedGitSource, SourceContentDigest, SourceLineage,
};
use std::path::Path;

/// Invocation-owned source acquisition. Package policy selects pins and offline
/// behavior before execution; repeated selectors retain their first Git result.
pub struct BuildSourceAcquisitions<'operation> {
    storage: &'operation SourceResolver,
    options: GitResolutionOptions<'operation>,
    root_request: PackageRootSourceRequest,
    locked_source: Option<&'operation CanonicalSourceClosureSubject>,
    git_sources: Vec<(GitSourceRequest, ResolvedGitSource)>,
    sources: Vec<AcquiredBuildSource>,
    pub(super) workspaces: Vec<super::workspace_acquisition::ExecutedWorkspace>,
}

impl<'operation> BuildSourceAcquisitions<'operation> {
    /// Apply resolver work limits before recursive Build work. Final graph
    /// reconciliation still rejects cycles that do not involve readiness waits.
    pub fn admit_build_descent(
        &self,
        active: &[AcquiredBuildSource],
        acquired: &AcquiredBuildSource,
    ) -> Result<(), String> {
        if active.iter().any(|source| source.same_source(acquired)) {
            return Err(format!(
                "cyclic Build readiness dependency at {}",
                acquired.entry_path().display()
            ));
        }
        if active.len()
            > crate::resolution::graph::reconcile::PackageSourceClosureLimits::default().max_depth
        {
            return Err("Build dependency depth exceeds the package resolution limit".into());
        }
        Ok(())
    }

    pub(crate) fn new(
        storage: &'operation SourceResolver,
        options: GitResolutionOptions<'operation>,
        root_request: PackageRootSourceRequest,
    ) -> Self {
        Self {
            storage,
            options,
            root_request,
            locked_source: None,
            git_sources: Vec::new(),
            sources: Vec::new(),
            workspaces: Vec::new(),
        }
    }

    /// Observational inspection may reacquire accepted edges, never discover
    /// a changed selector or new local source before reporting a graph mismatch.
    pub(crate) fn require_locked_graph(
        &mut self,
        source: &'operation CanonicalSourceClosureSubject,
    ) {
        self.locked_source = Some(source);
    }

    /// Retain the operation's original root, including staged source custody.
    /// Relative requests use its original lineage, never its materialized path.
    pub fn retain_root(&mut self, acquired: &AcquiredBuildSource) -> Result<(), String> {
        if acquired.root_request != self.root_request {
            return Err("source acquisition policy belongs to another project request".into());
        }
        self.retain_source(acquired.clone())?;
        Ok(())
    }

    /// Acquire a post-return graph occurrence through the same transport used
    /// while Build is suspended. Graph continuation stays outside transport.
    pub fn acquire(
        &mut self,
        pending: PendingBuildSource,
    ) -> Result<BuildSourceAcquisition, String> {
        if pending.request.root_request() != &self.root_request
            && !self
                .sources
                .iter()
                .any(|source| &source.root_request == pending.request.root_request())
        {
            return Err(pending
                .request
                .fail("source acquisition policy belongs to another project request")
                .to_string());
        }
        if let Some(source) = self.locked_source
            && let Err(error) = admit_locked_source_request(source, &pending.request)
        {
            return Err(pending.request.fail(error).to_string());
        }
        let requester = pending.request.requester();
        let origin = SourceAcquisitionOrigin {
            lineage: requester.key().source_lineage(),
            resolution: requester.resolution(),
            navigation: requester.navigation(),
            limits: requester.source_limits(),
        };
        let selection = match self.acquire_source(origin, pending.request.source()) {
            Ok(selection) => selection,
            Err(error) => return Err(pending.request.fail(error).to_string()),
        };
        match selection {
            AcquiredSourceSelection::Source(source) => pending.select(source),
            AcquiredSourceSelection::Workspace(workspace) => {
                Ok(BuildSourceAcquisition::Workspace { pending, workspace })
            }
        }
    }

    /// Resolve an actually issued occurrence before the requester has returned.
    /// This establishes source custody only. Its name, complete ledger, aliases
    /// and graph still require the ordinary normal-return join.
    pub fn acquire_issued(
        &mut self,
        requester: &AcquiredBuildSource,
        requests: &[build_evaluation::terminal_requests::IssuedBuildRequest],
        purpose: build_declarations::DependencyPurpose,
        ordinal: usize,
    ) -> Result<AcquiredSourceSelection, String> {
        if !self.sources.iter().any(|source| {
            source.lineage == requester.lineage
                && source.navigation == requester.navigation
                && source.resolution == requester.resolution
                && source.capture == requester.capture
        }) {
            return Err(
                "requesting Build has no source custody in this acquisition operation".into(),
            );
        }
        let projected =
            crate::declarations::dependencies::read::DependencyProjections::from_executed_requests(
                requests.iter().map(|issued| issued.request()),
            )?;
        if projected.authored_request_count()
            > crate::resolution::graph::reconcile::PackageSourceClosureLimits::default()
                .max_dependency_requests
        {
            return Err("issued Build requests exceed the package resolution limit".into());
        }
        let request = projected
            .requests(purpose)
            .get(ordinal)
            .ok_or("Build dependency occurrence has not been issued")?;
        let origin = requester.acquisition_origin();
        if let Some(subject) = self.locked_source {
            crate::resolution::graph::admit_locked_issued_source_request(
                subject, origin, purpose, ordinal, request,
            )?;
        }
        self.acquire_source(origin, request)
    }

    /// One source-selection operation, independent of whether its caller has a
    /// package declaration yet. The first immutable capture is reused throughout
    /// discovery, helper preparation and final graph reconciliation.
    fn acquire_source(
        &mut self,
        requester: SourceAcquisitionOrigin<'_>,
        request: &DependencySourceRequest,
    ) -> Result<AcquiredSourceSelection, String> {
        let acquired = match request {
            DependencySourceRequest::Path { location, .. } => {
                if let SourceLineage::ExternalLocal(lineage) = requester.lineage {
                    let root = super::external_local_dependency_root(
                        lineage.canonical_absolute_path(),
                        location,
                    )?;
                    let selected_lineage = SourceLineage::ExternalLocal(
                        ExternalLocalLineage::canonicalize(&root, lineage.source_context().clone())
                            .map_err(|error| error.to_string())?,
                    );
                    if let Some(source) = self.sources.iter().find(|source| {
                        source.lineage == selected_lineage
                            && source.navigation == super::PackageSourceNavigation::Root
                    }) {
                        return Ok(AcquiredSourceSelection::Source(source.clone()));
                    }
                    AcquiredBuildSource::capture_local(
                        &root,
                        requester.limits,
                        lineage.source_context().clone(),
                    )
                    .map_err(|error| error.to_string())?
                } else {
                    self.acquire_workspace_path(requester, location)?
                }
            }
            DependencySourceRequest::Git {
                repository,
                revision,
                selection,
                ..
            } => {
                let selected = GitSourceRequest::new(repository.clone(), Some(revision.clone()))
                    .map_err(|error| error.to_string())?;
                let source = self.acquire_git(&selected, requester.limits)?;
                let request = GitPackageSourceRequest::new(selected, selection.clone());
                if let PackageSelection::Named(package) = selection {
                    match self.acquire_workspace_member(source, request, package)? {
                        AcquiredSourceSelection::Workspace(workspace) => {
                            return Ok(AcquiredSourceSelection::Workspace(workspace));
                        }
                        AcquiredSourceSelection::Source(acquired) => acquired,
                    }
                } else if let Some(existing) = self.sources.iter().find(|acquired| {
                    &acquired.lineage == source.lineage()
                        && acquired.navigation == super::PackageSourceNavigation::Root
                }) {
                    let resolution = ImmutableSourceResolution::git(
                        GitCommitId::parse_hex(source.commit())
                            .map_err(|error| error.to_string())?,
                        GitTreeId::parse_hex(source.tree()).map_err(|error| error.to_string())?,
                    )
                    .map_err(|error| error.to_string())?;
                    if existing.resolution != resolution
                        || super::PackageSourceMaterialization::from_local(
                            existing.capture.normalized(),
                        ) != super::PackageSourceMaterialization::from_local(source.local())
                    {
                        return Err("Git requests for one source lineage select different immutable content".into());
                    }
                    let mut acquired = existing.clone();
                    acquired.root_request = PackageRootSourceRequest::Git(request);
                    acquired
                } else {
                    AcquiredBuildSource::capture_git(&source, request)?
                }
            }
        };
        self.retain_source(acquired)
            .map(AcquiredSourceSelection::Source)
    }

    pub(super) fn retain_source(
        &mut self,
        acquired: AcquiredBuildSource,
    ) -> Result<AcquiredBuildSource, String> {
        if let Some(existing) = self.sources.iter().find(|source| {
            source.lineage == acquired.lineage && source.navigation == acquired.navigation
        }) {
            if existing.resolution != acquired.resolution || existing.capture != acquired.capture {
                return Err("source requests select conflicting immutable content".into());
            }
            // Workspace selection belongs to this request; capture and lineage
            // belong to the operation's first acquisition.
            return Ok(acquired);
        }
        if self.sources.len()
            >= crate::resolution::graph::reconcile::PackageSourceClosureLimits::default()
                .max_packages
        {
            return Err("acquired Build sources exceed the package resolution limit".into());
        }
        self.sources.push(acquired.clone());
        Ok(acquired)
    }

    fn acquire_git(
        &mut self,
        request: &GitSourceRequest,
        limits: package_source::LocalSourceLimits,
    ) -> Result<ResolvedGitSource, String> {
        use package_source::git::resolution::{
            resolve_git_source_at_revision_in_lane, resolve_git_source_from_pin_in_lane,
        };
        // The first acquisition freezes a selector for this operation. Reuse
        // never runs Git or recaptures a source whose Build already returned.
        if let Some((_, source)) = self
            .git_sources
            .iter()
            .find(|(selected, _)| selected == request)
        {
            return Ok(source.clone());
        }
        let recorded = self
            .options
            .recorded(request)
            .map_err(|error| error.to_string())?;
        self.storage
            .verify_path_identity()
            .map_err(|error| error.to_string())?;
        let primary_git = self
            .storage
            .primary_git()
            .map_err(|error| error.to_string())?;
        let source = if let Some(recorded) = recorded {
            let ImmutableSourceResolution::Git { commit, tree, .. } = recorded.resolution else {
                return Err("recorded Git source has a non-Git resolution".into());
            };
            resolve_git_source_at_revision_in_lane(
                primary_git,
                request,
                commit,
                tree,
                recorded.acquisition,
                self.storage.git_sources(),
                limits,
            )
        } else {
            resolve_git_source_from_pin_in_lane(
                primary_git,
                request,
                None,
                self.storage.git_sources(),
                limits,
            )
        }
        .map_err(|error| error.to_string())?;
        self.storage
            .verify_path_identity()
            .map_err(|error| error.to_string())?;
        self.git_sources.push((request.clone(), source.clone()));
        Ok(source)
    }
}

/// Source selection may require executing an authenticated workspace catalog.
/// Neither result contains a graph continuation or provisional package name.
pub enum AcquiredSourceSelection {
    Source(AcquiredBuildSource),
    Workspace(super::AcquiredWorkspaceBuild),
}

#[derive(Clone, Copy)]
pub(crate) struct SourceAcquisitionOrigin<'source> {
    pub lineage: &'source SourceLineage,
    pub resolution: &'source ImmutableSourceResolution,
    pub navigation: &'source super::PackageSourceNavigation,
    pub limits: LocalSourceLimits,
}

/// Acquired bytes and authenticated lineage, without a parsed declaration or
/// provisional package key. Both transports establish the same owner at return.
#[derive(Clone)]
pub struct AcquiredBuildSource {
    pub(super) capture: CapturedLocalSource,
    pub(super) lineage: SourceLineage,
    pub(super) limits: LocalSourceLimits,
    pub(super) resolution: ImmutableSourceResolution,
    pub(super) root_request: PackageRootSourceRequest,
    pub(super) navigation: super::PackageSourceNavigation,
    pub(super) selection: super::PackageSourceSelectionEvidence,
}

#[derive(Debug)]
pub enum BuildSourceAcquisitionError {
    Source(package_source::SourceResolveError),
    Identity(package_source::IdentityError),
}

impl std::fmt::Display for BuildSourceAcquisitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Identity(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BuildSourceAcquisitionError {}

impl AcquiredBuildSource {
    /// Acquisition identity exists before a Build declares its package name.
    /// An operation retains one immutable capture at this identity.
    pub fn same_source(&self, other: &Self) -> bool {
        self.lineage == other.lineage && self.navigation == other.navigation
    }

    pub(crate) fn acquisition_origin(&self) -> SourceAcquisitionOrigin<'_> {
        SourceAcquisitionOrigin {
            lineage: &self.lineage,
            resolution: &self.resolution,
            navigation: &self.navigation,
            limits: self.limits,
        }
    }

    /// Capture proposed bytes without changing the original package lineage or
    /// the base used by relative dependency requests. The staging owner retains
    /// the live-source comparison required before publication.
    pub(crate) fn capture_staged(
        stage: &package_source::local::staging::StagedLocalSnapshot,
        context: ExternalSourceContext,
    ) -> Result<Self, String> {
        let mut acquired = Self::capture_local(
            stage.snapshot_root(),
            LocalSourceLimits::default(),
            context.clone(),
        )
        .map_err(|error| error.to_string())?;
        if acquired.capture.normalized().content_identity != stage.normalized().content_identity {
            return Err("captured candidate differs from its staged source".into());
        }
        acquired.lineage = SourceLineage::ExternalLocal(
            ExternalLocalLineage::canonicalize(stage.canonical_live_root(), context.clone())
                .map_err(|error| error.to_string())?,
        );
        acquired.root_request = PackageRootSourceRequest::ExternalLocal {
            requested_root: stage.requested_root().to_path_buf(),
            source_context: context,
        };
        Ok(acquired)
    }

    pub fn entry_path(&self) -> std::path::PathBuf {
        super::package_entry(
            self.capture.canonical_live_root(),
            Some(self.capture.tree()),
        )
    }

    /// Capture local source once. The enclosing package operation owns recovery
    /// and lock policy; capture itself grants no review or execution authority.
    pub fn capture_local(
        root: &Path,
        limits: LocalSourceLimits,
        context: ExternalSourceContext,
    ) -> Result<Self, BuildSourceAcquisitionError> {
        let limits = limits.compiler_bounded();
        let capture = package_source::local::operations::capture_local_source_input(root, limits)
            .map_err(BuildSourceAcquisitionError::Source)?;
        let canonical_path = capture.canonical_live_root().to_str().ok_or_else(|| {
            BuildSourceAcquisitionError::Identity(
                package_source::IdentityError::UnsupportedNonUtf8Path(
                    capture.canonical_live_root().to_path_buf(),
                ),
            )
        })?;
        let lineage = SourceLineage::ExternalLocal(
            ExternalLocalLineage::from_canonical_path(canonical_path.to_owned(), context.clone())
                .map_err(BuildSourceAcquisitionError::Identity)?,
        );
        let resolution = ImmutableSourceResolution::external_local(SourceContentDigest::derive(
            capture.normalized().content_identity.as_bytes(),
        ));
        let root_request = PackageRootSourceRequest::ExternalLocal {
            requested_root: capture.requested_root().to_path_buf(),
            source_context: context,
        };
        Ok(Self {
            capture,
            lineage,
            limits,
            resolution,
            root_request,
            navigation: super::PackageSourceNavigation::Root,
            selection: super::PackageSourceSelectionEvidence::Root,
        })
    }

    /// Close the authenticated Git snapshot into compiler-owned source bytes.
    /// This does not reinterpret a Git locator as a local source lineage.
    pub(super) fn capture_git(
        source: &ResolvedGitSource,
        request: GitPackageSourceRequest,
    ) -> Result<Self, String> {
        if source.workspace_projection().is_some()
            || source.lineage() != request.acquisition().lineage()
            || source.requested_locator() != request.requested_locator()
            || source.requested_revision() != request.requested_revision()
            || source.transport_profile() != request.transport_profile()
        {
            return Err("Git acquisition does not match the requested root source".into());
        }
        let capture = package_source::local::operations::capture_local_source_input(
            source.snapshot_root(),
            source.source_limits(),
        )
        .map_err(|error| error.to_string())?;
        if capture.normalized().content_identity != source.content_identity() {
            return Err("captured Git source differs from its authenticated snapshot".into());
        }
        let resolution = ImmutableSourceResolution::git(
            GitCommitId::parse_hex(source.commit()).map_err(|error| error.to_string())?,
            GitTreeId::parse_hex(source.tree()).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            capture,
            lineage: source.lineage().clone(),
            limits: source.source_limits(),
            resolution,
            root_request: PackageRootSourceRequest::Git(request),
            navigation: super::PackageSourceNavigation::Root,
            selection: super::PackageSourceSelectionEvidence::Root,
        })
    }

    pub fn source_capture(&self) -> &CapturedLocalSource {
        &self.capture
    }

    /// Bind one executed declaration; graph and compiler joins remain mandatory.
    pub fn bind_returned_build(
        &self,
        returned: &build_evaluation::ReturnedBuild,
    ) -> Result<package_compilation::PackageSourceBinding, String> {
        self.declare_build(returned)?.source_binding()
    }

    pub fn declare_build(
        &self,
        returned: &build_evaluation::ReturnedBuild,
    ) -> Result<DeclaredBuildSource, String> {
        let name = match returned.declaration() {
            build_declarations::BuildDeclaration::Package(package) => &package.name,
            build_declarations::BuildDeclaration::Application(application) => &application.name,
            build_declarations::BuildDeclaration::Workspace(_) => {
                return Err("a workspace declaration does not name a Product source owner".into());
            }
        };
        if let super::PackageSourceSelectionEvidence::GitWorkspace(evidence) = &self.selection {
            if name != evidence.plan().selected_member().package_name() {
                return Err(
                    "returned Build name differs from its executed workspace selection".into(),
                );
            }
        }
        let requests =
            crate::declarations::dependencies::read::DependencyProjections::from_executed_requests(
                returned.requests().iter().map(|issued| issued.request()),
            )?;
        let custody = PackageSourceCustody::from_resolved_parts(
            PackageKey::new(name.clone().into(), self.lineage.clone()),
            returned.declaration().kind(),
            self.resolution.clone(),
            super::PackageSourceMaterialization::from_local(self.capture.normalized()),
            self.capture.canonical_live_root().to_path_buf(),
            self.navigation.clone(),
            self.selection.clone(),
            self.limits,
            requests,
        )
        .with_captured_source(std::sync::Arc::clone(self.capture.tree()));
        Ok(DeclaredBuildSource {
            custody,
            root_request: self.root_request.clone(),
        })
    }
}
