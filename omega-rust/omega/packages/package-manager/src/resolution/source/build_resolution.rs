//! Resolve source requests issued by actual Build returns. Application code
//! executes each acquired source; this owner never invokes a compiler. The
//! resulting graph is source custody, not package review or execution authority.

use super::{AcquiredBuildSource, PackageSourceCustody};
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

/// A new source needs one executed declaration. A repeated acquisition keeps
/// the first immutable capture and advances the same graph without replaying
/// Build. Reuse still checks this occurrence's role, alias and dependency edge.
pub enum BuildSourceAcquisition {
    Execute(AcquiredBuildDependency),
    Reused(ExecutedSourceResolution),
}

pub struct AcquiredBuildDependency {
    pub(super) pending: PendingSourceResolution,
    pub(super) acquired: AcquiredBuildSource,
}

impl AcquiredBuildDependency {
    pub fn entry_path(&self) -> std::path::PathBuf {
        let capture = self.source_capture();
        super::package_entry(capture.canonical_live_root(), Some(capture.tree()))
    }

    pub fn source_capture(&self) -> &CapturedLocalSource {
        self.acquired.source_capture()
    }

    /// Supply the executed declaration for this exact acquisition. The compiler
    /// separately authenticates each retained activation when completing it.
    pub fn complete(
        self,
        returned: &build_evaluation::ReturnedBuild,
    ) -> Result<(PackageKeyIdentity, ExecutedSourceResolution), String> {
        let declared = match self.acquired.declare_build(returned) {
            Ok(declared) => declared,
            Err(error) => return Err(self.pending.fail(error).to_string()),
        };
        let identity = declared.identity();
        let step = self
            .pending
            .complete::<String>(declared.custody)
            .map_err(|error| error.to_string())?;
        Ok((identity, ExecutedSourceResolution::from_step(step)))
    }
}

pub struct ResolvedBuildSources {
    closure: ResolvedPackageSourceClosure,
}

impl ResolvedBuildSources {
    pub(crate) fn into_source_closure(self) -> ResolvedPackageSourceClosure {
        self.closure
    }

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
        let inputs = self.compilation_inputs(root)?;
        let join = super::build_source_join::prepare(&self.closure, custody.key(), &inputs)?;
        let requests = returned
            .requests()
            .iter()
            .map(|issued| issued.request())
            .collect::<Vec<_>>();
        let selections = join.resolve_issued_requests(&inputs, &requests)?;
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
