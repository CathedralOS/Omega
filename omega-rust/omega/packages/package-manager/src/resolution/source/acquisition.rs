//! Acquire immutable sources, then bind the declarations their Builds execute.

use super::build_resolution::{
    AcquiredBuildDependency, BuildSourceAcquisition, ExecutedSourceResolution, PendingBuildSource,
};
use crate::declarations::dependencies::read::{DependencySourceRequest, PackageSelection};
use crate::resolution::graph::GitResolutionOptions;
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
    git_sources: Vec<(GitSourceRequest, ResolvedGitSource)>,
    pub(super) workspaces: Vec<super::workspace_acquisition::ExecutedWorkspace>,
}

impl<'operation> BuildSourceAcquisitions<'operation> {
    pub(crate) fn new(
        storage: &'operation SourceResolver,
        options: GitResolutionOptions<'operation>,
        root_request: PackageRootSourceRequest,
    ) -> Self {
        Self {
            storage,
            options,
            root_request,
            git_sources: Vec::new(),
            workspaces: Vec::new(),
        }
    }

    /// Acquire only the source requested by the executed ledger. This owns no
    /// compiler and never projects package declarations from source syntax.
    pub fn acquire(
        &mut self,
        pending: PendingBuildSource,
    ) -> Result<BuildSourceAcquisition, String> {
        if pending.request.root_request() != &self.root_request {
            return Err(pending
                .request
                .fail("source acquisition policy belongs to another project request")
                .to_string());
        }
        let (selected, selection) = match pending.request.source() {
            DependencySourceRequest::Path { location, .. } => {
                let location = location.clone();
                return if matches!(
                    pending.request.requester().key().source_lineage(),
                    SourceLineage::ExternalLocal(_)
                ) {
                    pending.acquire_local_source(&location)
                } else {
                    self.acquire_workspace_path(pending, &location)
                };
            }
            DependencySourceRequest::Git {
                repository,
                revision,
                selection,
                ..
            } => (
                GitSourceRequest::new(repository.clone(), Some(revision.clone())),
                selection.clone(),
            ),
        };
        let selected = match selected {
            Ok(selected) => selected,
            Err(error) => return Err(pending.request.fail(error).to_string()),
        };
        let source = match self.acquire_git(&selected, pending.request.requester().source_limits())
        {
            Ok(source) => source,
            Err(error) => return Err(pending.request.fail(error).to_string()),
        };
        if let PackageSelection::Named(package) = selection {
            let request =
                GitPackageSourceRequest::new(selected, PackageSelection::Named(package.clone()));
            return self.acquire_workspace_member(pending, source, request, &package);
        }
        if let Some(custody) = pending
            .request
            .acquired_source(source.lineage(), &super::PackageSourceNavigation::Root)
            .cloned()
        {
            let resolution = ImmutableSourceResolution::git(
                package_source::GitCommitId::parse_hex(source.commit())
                    .map_err(|error| error.to_string())?,
                package_source::GitTreeId::parse_hex(source.tree())
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            if custody.resolution() != &resolution
                || custody.materialization()
                    != &super::PackageSourceMaterialization::from_local(source.local())
            {
                return Err(pending
                    .request
                    .fail("Git requests for one source lineage select different immutable content")
                    .to_string());
            }
            let step = pending
                .request
                .complete::<String>(custody)
                .map_err(|error| error.to_string())?;
            return Ok(BuildSourceAcquisition::Reused(
                ExecutedSourceResolution::from_step(step),
            ));
        }
        let request = GitPackageSourceRequest::new(selected, PackageSelection::Root);
        let acquired = match AcquiredBuildSource::capture_git(&source, request) {
            Ok(acquired) => acquired,
            Err(error) => return Err(pending.request.fail(error).to_string()),
        };
        Ok(BuildSourceAcquisition::Execute(AcquiredBuildDependency {
            pending: pending.request,
            acquired,
        }))
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

impl PendingBuildSource {
    /// The local transport adapter preserves the original requester's base and
    /// context. A Git-relative Path needs registered repository/workspace
    /// navigation; it is never treated as an ambient local path.
    fn acquire_local_source(self, location: &str) -> Result<BuildSourceAcquisition, String> {
        let requester = self.request.requester();
        let package_source::SourceLineage::ExternalLocal(lineage) =
            requester.key().source_lineage()
        else {
            return Err(self
                .request
                .fail("workspace requests require their registered source acquisition")
                .to_string());
        };
        let root = match super::external_local_dependency_root(requester.snapshot_root(), location)
        {
            Ok(root) => root,
            Err(error) => return Err(self.request.fail(error).to_string()),
        };
        let selected_lineage = match package_source::ExternalLocalLineage::canonicalize(
            &root,
            lineage.source_context().clone(),
        ) {
            Ok(lineage) => package_source::SourceLineage::ExternalLocal(lineage),
            Err(error) => return Err(self.request.fail(error).to_string()),
        };
        if let Some(custody) = self
            .request
            .acquired_source(&selected_lineage, &super::PackageSourceNavigation::Root)
            .cloned()
        {
            let step = self
                .request
                .complete::<String>(custody)
                .map_err(|error| error.to_string())?;
            return Ok(BuildSourceAcquisition::Reused(
                ExecutedSourceResolution::from_step(step),
            ));
        }
        let acquired = match AcquiredBuildSource::capture_local(
            &root,
            requester.source_limits(),
            lineage.source_context().clone(),
        ) {
            Ok(acquired) => acquired,
            Err(error) => return Err(self.request.fail(error).to_string()),
        };
        Ok(BuildSourceAcquisition::Execute(AcquiredBuildDependency {
            pending: self.request,
            acquired,
        }))
    }
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
