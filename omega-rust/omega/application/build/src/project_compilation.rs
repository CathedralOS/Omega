//! Execute an admitted package batch; package management never calls back here.
//!
//! Admission retains source/consent identity and owns staging until completion.
//! The application owns the compiler session and submits all accepted targets
//! together, preserving shared source work and invocation-wide resource budgets.

use package_manager::operations::{
    ProjectCompilationAdmissionRequest, ProjectCompilationOutcome, admit_project_compilation,
};

pub(crate) fn compile(
    request: ProjectCompilationAdmissionRequest<'_>,
) -> Vec<ProjectCompilationOutcome> {
    let admitted = admit_project_compilation(request);
    let mut compiler = compiler::CompilationSession::default();
    let products = compiler.compile_packages(admitted.requests);
    admitted.completion.complete(products)
}
