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

#[derive(Clone)]
pub struct DeclaredBuildSource {
    pub(super) custody: PackageSourceCustody,
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
        .with_captured_source(std::sync::Arc::clone(tree))
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
            SourceClosureStep::Complete(closure) => {
                Self::Complete(ResolvedBuildSources { closure })
            }
        }
    }
}

pub struct PendingBuildSource {
    pub(super) request: PendingSourceResolution,
}

pub enum BuildSourceAcquisition {
    Execute(AcquiredBuildDependency),
    Reused(ExecutedSourceResolution),
    Workspace {
        pending: PendingBuildSource,
        workspace: super::AcquiredWorkspaceBuild,
    },
}

impl PendingBuildSource {
    pub fn select(self, acquired: AcquiredBuildSource) -> Result<BuildSourceAcquisition, String> {
        if let Some(custody) = self
            .request
            .acquired_source(&acquired.lineage, &acquired.navigation)
            .cloned()
        {
            if custody.resolution() != &acquired.resolution
                || custody.materialization()
                    != &super::PackageSourceMaterialization::from_local(
                        acquired.capture.normalized(),
                    )
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
        let capture = self.source_capture();
        super::package_entry(capture.canonical_live_root(), Some(capture.tree()))
    }

    pub fn source_capture(&self) -> &CapturedLocalSource {
        self.acquired.source_capture()
    }

    pub fn complete(
        self,
        returned: &build_settings::ReturnedBuild,
    ) -> Result<(PackageKeyIdentity, ExecutedSourceResolution), String> {
        let declared = match self.acquired.declare_build(returned) {
            Ok(declared) => declared,
            Err(error) => return Err(self.pending.fail(error).to_string()),
        };
        self.complete_declared(&declared)
    }

    pub fn complete_declared(
        self,
        declared: &DeclaredBuildSource,
    ) -> Result<(PackageKeyIdentity, ExecutedSourceResolution), String> {
        let custody = &declared.custody;
        if custody.key().source_lineage() != &self.acquired.lineage
            || custody.navigation() != &self.acquired.navigation
            || custody.resolution() != &self.acquired.resolution
            || custody.captured_source().map(std::sync::Arc::as_ref)
                != Some(self.acquired.capture.tree().as_ref())
            || custody.snapshot_root() != self.acquired.capture.canonical_live_root()
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

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedBuildSources {
    pub(crate) closure: ResolvedPackageSourceClosure,
}

impl ResolvedBuildSources {
    pub fn contains(&self, identity: PackageKeyIdentity) -> bool {
        self.closure
            .custodies()
            .iter()
            .any(|custody| custody.key().identity() == identity)
    }

    pub(crate) fn into_source_closure(self) -> ResolvedPackageSourceClosure {
        self.closure
    }

    pub fn root(&self) -> PackageKeyIdentity {
        self.closure.graph().root().identity()
    }

    pub fn join_build(
        &self,
        root: PackageKeyIdentity,
        returned: &mut build_settings::ReturnedBuild,
    ) -> Result<PackageCompilationInputs, String> {
        self.join_source_result(root, &mut returned.source_result())
    }

    pub fn join_source_result(
        &self,
        root: PackageKeyIdentity,
        result: &mut build_settings::BuildSourceResult<'_>,
    ) -> Result<PackageCompilationInputs, String> {
        let custody = self
            .closure
            .custodies()
            .iter()
            .find(|custody| custody.key().identity() == root)
            .ok_or("Build does not belong to this resolved source graph")?;
        let requests =
            crate::declarations::dependencies::read::DependencyProjections::from_executed_requests(
                result.requests(),
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
            .map(|dependency| build_settings::BuildDependencySelection {
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
