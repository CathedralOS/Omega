// We resolve source requests issued by actual `Build` returns. Application code
// executes each acquired source while this owner tracks source custody without
// invoking a compiler. A new source needs one executed declaration, whereas a
// repeated acquisition keeps the first immutable capture and advances the graph
// while still validating this occurrence's role, alias, and dependency edge.

use super::{AcquiredBuildSource, PackageSourceCustody};
use crate::resolution::graph::reconcile::{
    PendingSourceResolution, SourceClosureStep, begin_source_closure,
};
use crate::resolution::graph::{PackageRootSourceRequest, ResolvedPackageSourceClosure};
use package_compilation::{CapturedLocalSource, PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;

#[derive(Clone)]
pub struct DeclaredBuildSource {
    pub(super) custody: PackageSourceCustody,
    pub(super) backings: std::collections::BTreeMap<
        (package_compilation::declarations::DependencyPurpose, usize),
        build_execution::DependencyBacking,
    >,
    pub(super) root_request: PackageRootSourceRequest,
}

impl DeclaredBuildSource {
    pub fn package_name(&self) -> &str {
        self.custody.key().name().as_str()
    }

    pub fn require_dependency(&self) -> Result<(), String> {
        if self.custody.role() != crate::declarations::BuildDeclarationKind::Package {
            return Err("Build provider dependency must declare a package".into());
        }
        Ok(())
    }

    pub fn identity(&self) -> PackageKeyIdentity {
        self.custody.key().identity()
    }

    pub fn source_binding(&self) -> Result<PackageSourceBinding, String> {
        let custody = &self.custody;
        let tree = custody
            .captured_source()
            .ok_or("executed source lost its capture")?;
        PackageSourceBinding::new(
            custody.key().identity(),
            custody.key().name().as_str(),
            custody.snapshot_root().to_path_buf(),
        )
        .with_resolution(custody.resolution())
        .with_preparing_source(custody.preparing_source().cloned())
        .with_captured_source(std::sync::Arc::clone(tree))?
        .with_build_entries(custody.build_entries.clone())
    }

    pub fn resolve(self) -> Result<ExecutedSourceResolution, String> {
        let step =
            begin_source_closure::<String>(self.root_request, self.custody, Default::default())
                .map_err(|error| error.to_string())?;
        Ok(ExecutedSourceResolution::from_step(step))
    }
}

pub enum ExecutedSourceResolution {
    Request(PendingBuildSource),
    Complete(ResolvedBuildSources),
}

impl ExecutedSourceResolution {
    pub(super) fn from_step(step: SourceClosureStep) -> Self {
        match step {
            SourceClosureStep::Request(request) => Self::Request(PendingBuildSource { request }),
            SourceClosureStep::Complete(closure) => Self::Complete(ResolvedBuildSources {
                closure: Arc::new(closure),
            }),
        }
    }
}

pub struct PendingBuildSource {
    pub(super) request: PendingSourceResolution,
}

pub enum BuildSourceAcquisition {
    Execute(AcquiredBuildDependency),
    Reused(AcquiredBuildSource, ExecutedSourceResolution),
    Workspace {
        pending: PendingBuildSource,
        workspace: super::AcquiredWorkspaceBuild,
    },
}

impl PendingBuildSource {
    pub(crate) fn requester(&self) -> PackageKeyIdentity {
        self.request.requester().key().identity()
    }

    pub(crate) fn purpose(&self) -> package_compilation::declarations::DependencyPurpose {
        self.request.purpose()
    }

    pub(crate) fn backing<'a>(
        &self,
        from: &'a DeclaredBuildSource,
    ) -> Option<&'a build_execution::DependencyBacking> {
        from.backings
            .get(&(self.request.purpose(), self.request.dependency_index()))
    }

    pub fn select(self, acquired: AcquiredBuildSource) -> Result<BuildSourceAcquisition, String> {
        if let Some(custody) = self
            .request
            .acquired_source(&acquired.lineage, &acquired.navigation)
            .cloned()
        {
            if custody.resolution() != &acquired.resolution
                || !custody.matches_requested_content(&acquired.capture)
            {
                return Err(self
                    .request
                    .fail("source requests select conflicting immutable content")
                    .to_string());
            }
            let next = self
                .request
                .complete::<String>(custody)
                .map_err(|error| error.to_string())?;
            return Ok(BuildSourceAcquisition::Reused(
                acquired,
                ExecutedSourceResolution::from_step(next),
            ));
        }
        Ok(BuildSourceAcquisition::Execute(AcquiredBuildDependency {
            pending: self.request,
            acquired,
        }))
    }
}

pub struct AcquiredBuildDependency {
    pub(super) pending: PendingSourceResolution,
    pub(super) acquired: AcquiredBuildSource,
}

impl AcquiredBuildDependency {
    pub fn acquired_source(&self) -> &AcquiredBuildSource {
        &self.acquired
    }

    pub fn entry_path(&self) -> std::path::PathBuf {
        self.acquired.entry_path()
    }

    pub fn source_capture(&self) -> &CapturedLocalSource {
        self.acquired.source_capture()
    }

    pub fn complete_declared(
        self,
        declared: &DeclaredBuildSource,
    ) -> Result<(PackageKeyIdentity, ExecutedSourceResolution), String> {
        let custody = &declared.custody;
        if custody.key().source_lineage() != &self.acquired.lineage
            || custody.navigation() != &self.acquired.navigation
            || custody.resolution() != &self.acquired.resolution
            || !custody.matches_acquisition(&self.acquired.capture)
        {
            return Err(self
                .pending
                .fail("returned declaration belongs to another acquired source")
                .to_string());
        }
        let identity = declared.identity();
        let step = self
            .pending
            .complete::<String>(custody.clone())
            .map_err(|error| error.to_string())?;
        Ok((identity, ExecutedSourceResolution::from_step(step)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBuildSources {
    pub(crate) closure: Arc<ResolvedPackageSourceClosure>,
}

impl ResolvedBuildSources {
    pub(crate) fn into_source_closure(self) -> ResolvedPackageSourceClosure {
        Arc::unwrap_or_clone(self.closure)
    }

    /// A Build encountered while resolving a provider can itself be requested later.
    /// Review it with its own root and only the dependencies reachable from that root.
    pub(crate) fn rooted_at(
        &self,
        declared: &DeclaredBuildSource,
        acquisition: &AcquiredBuildSource,
    ) -> Result<Self, String> {
        let root = &declared.custody;
        if self.closure.graph().root() == root.key()
            && self.closure.source_requests().root().request() == &acquisition.root_request
            && root.selection_evidence() == &acquisition.selection
        {
            return Ok(self.clone());
        }
        // Two requests can reach the same captured package through different Git revisions.
        // Begin with this request's selection and follow the dependency edges already checked.
        let mut step = begin_source_closure::<String>(
            acquisition.root_request.clone(),
            root.clone()
                .with_selection_evidence(acquisition.selection.clone()),
            Default::default(),
        )
        .map_err(|error| error.to_string())?;
        loop {
            let request = match step {
                SourceClosureStep::Complete(closure) => {
                    return Ok(Self {
                        closure: Arc::new(closure),
                    });
                }
                SourceClosureStep::Request(request) => request,
            };
            let edge = self
                .closure
                .graph()
                .package(request.requester().key())
                .and_then(|node| {
                    node.dependencies().iter().find(|edge| {
                        edge.purpose() == request.purpose()
                            && edge.dependency_index() == request.dependency_index()
                    })
                })
                .ok_or("settled package has no source for this dependency request")?;
            let child = self
                .closure
                .custody(edge.target())
                .ok_or("settled dependency has no captured source")?;
            step = request
                .complete::<String>(child.clone())
                .map_err(|error| error.to_string())?;
        }
    }

    pub fn root(&self) -> PackageKeyIdentity {
        self.closure.graph().root().identity()
    }

    pub fn join_source_result(
        &self,
        root: PackageKeyIdentity,
        result: &mut build_execution::BuildSourceResult<'_>,
    ) -> Result<PackageCompilationInputs, String> {
        let custody = self
            .closure
            .custodies()
            .iter()
            .find(|custody| custody.key().identity() == root)
            .ok_or("Build does not belong to this resolved source graph")?;
        let requests =
            crate::declarations::dependencies::read::DependencyProjections::from_executed_requests(
                result.source_requests(),
            )?;
        if &requests != custody.dependency_projections() {
            return Err("Build source requests differ from the executed acquisition ledger".into());
        }
        let node = self
            .closure
            .graph()
            .package(custody.key())
            .ok_or("resolved source has no graph node")?;
        let selections = node
            .dependencies()
            .iter()
            .map(|dependency| build_execution::BuildDependencySelection {
                purpose: dependency.purpose(),
                request_ordinal: dependency.dependency_index(),
                package: dependency.target().identity(),
            })
            .collect::<Vec<_>>();
        let inputs = self.compilation_inputs(root)?;
        result.join_dependency_sources(&inputs, &selections)?;
        Ok(inputs)
    }

    pub fn compilation_inputs(
        &self,
        root: PackageKeyIdentity,
    ) -> Result<PackageCompilationInputs, String> {
        let custody = self
            .closure
            .custodies()
            .iter()
            .find(|custody| custody.key().identity() == root)
            .ok_or("requested package does not belong to the resolved Build source graph")?;
        crate::resolution::PackageCompilationScope::new(&self.closure, custody.key())
            .compilation_inputs_reusing(&mut None)
            .map_err(|errors| format!("cannot join executed source graph: {errors:?}"))
    }
}
