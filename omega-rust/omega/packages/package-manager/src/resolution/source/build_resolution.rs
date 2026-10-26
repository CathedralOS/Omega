//! Resolve source requests issued by actual Build returns. Application code
//! executes each acquired source; this owner never invokes a compiler. The
//! resulting graph is source custody, not package review or execution authority.

use super::{AcquiredLocalSource, PackageSourceCustody};
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::resolution::graph::reconcile::{
    PendingSourceResolution, SourceClosureStep, begin_source_closure,
};
use crate::resolution::graph::{PackageRootSourceRequest, ResolvedPackageSourceClosure};
use package_compilation::{CapturedLocalSource, PackageCompilationInputs, PackageSourceBinding};
use semantic_vocabulary::PackageKeyIdentity;

pub struct DeclaredBuildSource {
    pub(super) custody: PackageSourceCustody,
    pub(super) root_request: PackageRootSourceRequest,
}

impl DeclaredBuildSource {
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

    /// Start bounded reconciliation from the executed root ledger. A request
    /// remains outstanding until its selected source has also returned Build.
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
    fn from_step(step: SourceClosureStep) -> Self {
        match step {
            SourceClosureStep::Request(request) => Self::Request(PendingBuildSource { request }),
            SourceClosureStep::Complete(closure) => {
                Self::Complete(ResolvedBuildSources { closure })
            }
        }
    }
}

pub struct PendingBuildSource {
    request: PendingSourceResolution,
}

impl PendingBuildSource {
    /// The local transport adapter preserves the original requester's base and
    /// context. Git/workspace requests need their pinned acquisition adapter;
    /// this operation never treats their locator as an ambient local path.
    pub fn capture_local_source(self) -> Result<AcquiredBuildDependency, String> {
        let requester = self.request.requester();
        let DependencySourceRequest::Path { location, .. } = self.request.source() else {
            return Err(self
                .request
                .fail("executed Git requests require pinned source acquisition")
                .to_string());
        };
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
        let acquired = match AcquiredLocalSource::capture(
            &root,
            requester.source_limits(),
            lineage.source_context().clone(),
        ) {
            Ok(acquired) => acquired,
            Err(error) => return Err(self.request.fail(error).to_string()),
        };
        Ok(AcquiredBuildDependency {
            pending: self.request,
            acquired,
        })
    }
}

pub struct AcquiredBuildDependency {
    pending: PendingSourceResolution,
    acquired: AcquiredLocalSource,
}

impl AcquiredBuildDependency {
    pub fn source_capture(&self) -> &CapturedLocalSource {
        self.acquired.source_capture()
    }

    /// Supply the executed declaration for this exact acquisition. The compiler
    /// separately authenticates each retained activation when completing it.
    pub fn complete(
        self,
        returned: &build_evaluation::ReturnedBuild,
    ) -> Result<ExecutedSourceResolution, String> {
        let declared = match self.acquired.declare_build(returned) {
            Ok(declared) => declared,
            Err(error) => return Err(self.pending.fail(error).to_string()),
        };
        let step = self
            .pending
            .complete::<String>(declared.custody)
            .map_err(|error| error.to_string())?;
        Ok(ExecutedSourceResolution::from_step(step))
    }
}

pub struct ResolvedBuildSources {
    closure: ResolvedPackageSourceClosure,
}

impl ResolvedBuildSources {
    pub fn root(&self) -> PackageKeyIdentity {
        self.closure.graph().root().identity()
    }

    /// Join this package's authenticated request ledger with the acquisitions
    /// completed by this resolution. A lookalike alias graph cannot substitute
    /// different Path/Git requests from another execution.
    pub fn join_build(
        &self,
        root: PackageKeyIdentity,
        returned: &mut build_evaluation::ReturnedBuild,
    ) -> Result<PackageCompilationInputs, String> {
        let custody = self
            .closure
            .custodies()
            .iter()
            .find(|custody| custody.key().identity() == root)
            .ok_or("Build does not belong to this resolved source graph")?;
        let requests =
            crate::declarations::dependencies::read::DependencyProjections::from_executed_requests(
                returned.requests().iter().map(|issued| issued.request()),
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
            .map(|dependency| build_evaluation::BuildDependencySelection {
                purpose: dependency.purpose(),
                request_ordinal: dependency.dependency_index(),
                package: dependency.target().identity(),
            })
            .collect::<Vec<_>>();
        let inputs = self.compilation_inputs(root)?;
        returned.join_dependency_sources(&inputs, &selections)?;
        Ok(inputs)
    }

    /// Build a package's source view without reopening any acquired namespace.
    /// Review grants and generated output remain separate joins.
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
