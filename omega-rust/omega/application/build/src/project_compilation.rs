//! Execute an admitted package batch; package management never calls back here.
//!
//! Admission retains source/consent identity. The application retains the
//! package session through scheduling, then returns it for disposal. Accepted
//! targets run together, sharing source work and invocation-wide budgets.

use package_manager::PackageBuildSession;
use package_manager::operations::{
    ProjectCompilationAdmissionRequest, ProjectCompilationOutcome, admit_project_compilation,
};
use std::path::Path;

pub(crate) struct ProjectCompilationRequest<'request> {
    pub prepared: package_manager::operations::PreparedLocalProject,
    pub build_directory: &'request Path,
    pub target_profiles: &'request [target::TargetProfile],
    pub snapshot: Option<&'request build_evaluation::BuildSnapshotRequest>,
    pub verify: bool,
    pub timings: bool,
    pub product: compiler::CompilationProduct,
}

pub(crate) fn compile(request: ProjectCompilationRequest<'_>) -> Vec<ProjectCompilationOutcome> {
    let session = PackageBuildSession::create(request.build_directory);
    let request = ProjectCompilationAdmissionRequest {
        prepared: request.prepared,
        target_profiles: request.target_profiles,
        snapshot: request.snapshot,
        verify: request.verify,
        timings: request.timings,
        product: request.product,
    };
    let mut session = match session {
        Ok(session) => session,
        Err(error) => return request.refuse(error.to_string()),
    };
    let admitted = admit_project_compilation(request, &mut session);
    let products = crate::compile_packages(session.compiler(), admitted.requests);
    admitted.completion.complete(products, session)
}
