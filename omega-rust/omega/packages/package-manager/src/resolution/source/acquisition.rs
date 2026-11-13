// We acquire immutable sources and bind the declarations their `Build`
// invocations execute. Package policy selects pins and offline behavior before
// execution, and the first acquisition freezes a selector for the operation so
// reuse never re-runs Git or recaptures a source whose `Build` already
// returned. Retaining a source always selects cache coordinates from this
// acquisition owner, and `capture_staged` captures proposed bytes without
// changing the original package lineage or the base used by relative dependency
// requests.

use super::build_resolution::{BuildSourceAcquisition, PendingBuildSource};
use crate::declarations::dependencies::read::{DependencySourceRequest, PackageSelection};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, GitResolutionOptions, admit_locked_source_request,
};
use package_source::{GitSourceRequest, SourceResolver};

use super::{DeclaredBuildSource, GitPackageSourceRequest, PackageSourceCustody};
use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::DependencyProjections;
use crate::resolution::graph::PackageRootSourceRequest;
use crate::resolution::graph::admit_locked_issued_source_request;
use crate::resolution::graph::reconcile::PackageSourceClosureLimits;
use package_source::local::operations::capture_local_source_input;
use package_source::local::resolution_observations::CapturedLocalSource;
use package_source::{
    ExternalLocalLineage, ExternalSourceContext, GitCommitId, GitTreeId, ImmutableSourceResolution,
    LocalSourceLimits, ResolvedGitSource, SourceContentDigest, SourceLineage,
};
use std::path::Path;

pub struct BuildSourceAcquisitions<'operation> {
    pub(super) storage: &'operation SourceResolver,
    pub(super) options: GitResolutionOptions<'operation>,
    root_request: PackageRootSourceRequest,
    locked_source: Option<&'operation CanonicalSourceClosureSubject>,
    git_sources: Vec<(GitSourceRequest, ResolvedGitSource)>,
    sources: Vec<AcquiredBuildSource>,
    cache_directories: std::sync::OnceLock<Option<super::custody::SourceCacheDirectories>>,
    pub(super) workspaces: Vec<super::workspace_acquisition::ExecutedWorkspace>,
}

impl<'operation> BuildSourceAcquisitions<'operation> {
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
        if active.len() > PackageSourceClosureLimits::default().max_depth {
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
            cache_directories: std::sync::OnceLock::new(),
            workspaces: Vec::new(),
        }
    }

    pub(crate) fn require_locked_graph(
        &mut self,
        source: &'operation CanonicalSourceClosureSubject,
    ) {
        self.locked_source = Some(source);
    }

    pub fn retain_root(
        &mut self,
        acquired: AcquiredBuildSource,
    ) -> Result<AcquiredBuildSource, String> {
        if acquired.root_request != self.root_request {
            return Err("source acquisition policy belongs to another project request".into());
        }
        self.retain_source(acquired)
    }

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

    pub fn acquire_issued(
        &mut self,
        requester: &AcquiredBuildSource,
        requests: &[build_settings::host::requests::IssuedBuildRequest],
        purpose: build_declarations::DependencyPurpose,
        ordinal: usize,
    ) -> Result<AcquiredSourceSelection, String> {
        self.acquire_requests(
            requester,
            requests.iter().map(|issued| issued.request()),
            purpose,
            ordinal,
        )
    }

    pub fn acquire_completed(
        &mut self,
        requester: &AcquiredBuildSource,
        result: &build_settings::BuildSourceResult<'_>,
        purpose: build_declarations::DependencyPurpose,
        ordinal: usize,
    ) -> Result<AcquiredSourceSelection, String> {
        self.acquire_requests(requester, result.requests(), purpose, ordinal)
    }

    fn acquire_requests<'request>(
        &mut self,
        requester: &AcquiredBuildSource,
        requests: impl Iterator<Item = &'request build_declarations::BuildRequest>,
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
        let projected = DependencyProjections::from_executed_requests(requests)?;
        if projected.authored_request_count()
            > PackageSourceClosureLimits::default().max_dependency_requests
        {
            return Err("issued Build requests exceed the package resolution limit".into());
        }
        let request = projected
            .requests(purpose)
            .get(ordinal)
            .ok_or("Build dependency occurrence has not been issued")?;
        let origin = requester.acquisition_origin();
        if let Some(subject) = self.locked_source {
            admit_locked_issued_source_request(subject, origin, purpose, ordinal, request)?;
        }
        self.acquire_source(origin, request)
    }

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
                let request = GitPackageSourceRequest::new(selected, selection.clone());
                if let PackageSelection::Named(package) = selection {
                    return self.acquire_workspace_member(request, package, requester.limits);
                }
                let source = self.acquire_git(request.acquisition(), requester.limits)?;
                if let Some(existing) = self.sources.iter().find(|acquired| {
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
        mut acquired: AcquiredBuildSource,
    ) -> Result<AcquiredBuildSource, String> {
        acquired.cache_directories = self
            .cache_directories
            .get_or_init(|| super::custody::SourceCacheDirectories::retain(self.storage))
            .clone();
        if let Some(existing) = self.sources.iter().find(|source| {
            source.lineage == acquired.lineage && source.navigation == acquired.navigation
        }) {
            if existing.resolution != acquired.resolution || existing.capture != acquired.capture {
                return Err("source requests select conflicting immutable content".into());
            }
            return Ok(acquired);
        }
        if self.sources.len() >= PackageSourceClosureLimits::default().max_packages {
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

#[derive(Clone)]
pub struct AcquiredBuildSource {
    pub(super) capture: CapturedLocalSource,
    pub(super) lineage: SourceLineage,
    pub(super) limits: LocalSourceLimits,
    pub(super) resolution: ImmutableSourceResolution,
    pub(super) root_request: PackageRootSourceRequest,
    pub(super) navigation: super::PackageSourceNavigation,
    pub(super) selection: super::PackageSourceSelectionEvidence,
    pub(super) cache_directories: Option<super::custody::SourceCacheDirectories>,
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
    pub fn same_source(&self, other: &Self) -> bool {
        self.lineage == other.lineage && self.navigation == other.navigation
    }

    /// Admit a completed Build only for the same source and exact captured authority.
    pub fn verify_reused_capture(&self, other: &Self) -> Result<(), String> {
        if !self.same_source(other) {
            return Err("reused Build belongs to another source".into());
        }
        if self.capture != other.capture {
            return Err("reused Build source changed its immutable capture".into());
        }
        Ok(())
    }

    pub(crate) fn acquisition_origin(&self) -> SourceAcquisitionOrigin<'_> {
        SourceAcquisitionOrigin {
            lineage: &self.lineage,
            resolution: &self.resolution,
            navigation: &self.navigation,
            limits: self.limits,
        }
    }

    pub(crate) fn capture_staged(
        stage: &package_source::local::staging::StagedLocalSnapshot,
        context: ExternalSourceContext,
    ) -> Result<Self, String> {
        let mut acquired = Self::capture_selected_local(
            stage.snapshot_root(),
            LocalSourceLimits::default(),
            context.clone(),
            stage.selection(),
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

    pub fn capture_local(
        root: &Path,
        limits: LocalSourceLimits,
        context: ExternalSourceContext,
    ) -> Result<Self, BuildSourceAcquisitionError> {
        Self::capture_selected_local(
            root,
            limits,
            context,
            &package_source::SourceCaptureRequest::entire_tree(),
        )
    }

    pub fn capture_selected_local(
        root: &Path,
        limits: LocalSourceLimits,
        context: ExternalSourceContext,
        selected: &package_source::SourceCaptureRequest,
    ) -> Result<Self, BuildSourceAcquisitionError> {
        let limits = limits.compiler_bounded();
        let capture = package_source::local::operations::capture_selected_local_source_input(
            root, limits, selected,
        )
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
            cache_directories: None,
        })
    }

    pub(super) fn capture_git(
        source: &ResolvedGitSource,
        request: GitPackageSourceRequest,
    ) -> Result<Self, String> {
        let navigation = match (request.selection(), source.selected_member()) {
            (PackageSelection::Root, None) => super::PackageSourceNavigation::Root,
            (PackageSelection::Named(_), Some(path)) => {
                super::PackageSourceNavigation::Member(path.clone())
            }
            _ => return Err("Git acquisition has the wrong source selection".into()),
        };
        if source.lineage() != request.acquisition().lineage()
            || source.requested_locator() != request.requested_locator()
            || source.requested_revision() != request.requested_revision()
            || source.transport_profile() != request.transport_profile()
        {
            return Err("Git acquisition does not match the requested root source".into());
        }
        let capture = capture_local_source_input(source.snapshot_root(), source.source_limits())
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
            navigation,
            selection: super::PackageSourceSelectionEvidence::Root,
            cache_directories: None,
        })
    }

    pub fn source_capture(&self) -> &CapturedLocalSource {
        &self.capture
    }

    pub fn bind_returned_build(
        &self,
        returned: &build_settings::ReturnedBuild,
    ) -> Result<package_compilation::PackageSourceBinding, String> {
        self.declare_build(returned)?.source_binding()
    }

    pub fn declare_build(
        &self,
        returned: &build_settings::ReturnedBuild,
    ) -> Result<DeclaredBuildSource, String> {
        self.declare_requests(
            returned.declaration(),
            returned.requests().iter().map(|issued| issued.request()),
        )
    }

    pub fn declare_source_result(
        &self,
        result: &build_settings::BuildSourceResult<'_>,
    ) -> Result<DeclaredBuildSource, String> {
        self.declare_requests(result.declaration(), result.requests())
    }

    fn declare_requests<'request>(
        &self,
        declaration: Option<&build_declarations::BuildDeclaration>,
        requests: impl Iterator<Item = &'request build_declarations::BuildRequest>,
    ) -> Result<DeclaredBuildSource, String> {
        let declaration = declaration.ok_or("default Build has no authored named source owner")?;
        let name = match declaration {
            build_declarations::BuildDeclaration::Package(package) => &package.name,
            build_declarations::BuildDeclaration::Application(application) => &application.name,
            build_declarations::BuildDeclaration::Workspace(_) => {
                return Err("a workspace declaration does not name a Product source owner".into());
            }
        };
        if let super::PackageSourceSelectionEvidence::GitWorkspace(evidence) = &self.selection
            && name != evidence.plan().selected_member().package_name()
        {
            return Err("returned Build name differs from its executed workspace selection".into());
        }
        let requests = DependencyProjections::from_executed_requests(requests)?;
        let custody = PackageSourceCustody::from_resolved_parts(
            PackageKey::new(name.clone().into(), self.lineage.clone()),
            declaration.kind(),
            self.resolution.clone(),
            super::PackageSourceMaterialization::from_local(self.capture.normalized()),
            self.capture.canonical_live_root().to_path_buf(),
            self.navigation.clone(),
            self.selection.clone(),
            self.limits,
            requests,
        )
        .with_captured_source(std::sync::Arc::clone(self.capture.tree()));
        let custody = match &self.cache_directories {
            Some(directories) => directories.apply_to(custody),
            None => custody,
        };
        Ok(DeclaredBuildSource {
            custody,
            root_request: self.root_request.clone(),
        })
    }
}
