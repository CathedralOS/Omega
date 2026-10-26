//! Execute an admitted package batch; package management never calls back here.
//!
//! Admission retains source/consent identity. The application retains the
//! package session through scheduling, then returns it for disposal. Accepted
//! targets run together, sharing source work and invocation-wide budgets.

use crate::BuildSession;
use package_manager::operations::{ProjectCompilationAdmissionRequest, ProjectCompilationOutcome};

pub(crate) struct ProjectCompilationRequest<'request> {
    pub prepared: package_manager::operations::PreparedLocalProject,
    pub target_profiles: &'request [target::TargetProfile],
    pub snapshot: Option<&'request build_evaluation::BuildSnapshotRequest>,
    pub verify: bool,
    pub timings: bool,
    pub product: compiler::CompilationProduct,
}

/// Source discovery already executed these Builds. Settle their exact returns,
/// admit Product contexts, then use the ordinary dependency/product scheduler.
/// No fresh execution request or grant is discarded at this boundary.
pub(crate) fn compile_returned(
    request: ProjectCompilationRequest<'_>,
    builds: Vec<crate::ResolvedPackageBuild>,
    mut session: BuildSession,
    execution_profile: target::TargetProfile,
) -> Vec<ProjectCompilationOutcome> {
    let request = ProjectCompilationAdmissionRequest {
        prepared: request.prepared,
        target_profiles: request.target_profiles,
        snapshot: request.snapshot,
        verify: request.verify,
        timings: request.timings,
        product: request.product,
    };
    let mut completed = Vec::with_capacity(builds.len());
    for build in builds {
        let inputs = build.package_inputs().clone();
        match build.settle(&mut session.compiler, execution_profile, inputs) {
            Ok(build) => completed.push(build),
            Err(error) => {
                let mut reason = error
                    .into_diagnostics()
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                if let Err(error) = session.close() {
                    reason.push_str(&format!("; session disposal: {error}"));
                }
                return request.refuse(reason);
            }
        }
    }
    let admitted = package_manager::operations::admit_project_products(
        request,
        &mut session.packages,
        execution_profile,
    );
    let products = crate::package_compilation::compile_returned_packages(
        &mut session.compiler,
        admitted.requests,
        &mut completed,
    );
    admitted.completion.complete(products, session.packages)
}
