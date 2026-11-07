/*

Compile an admitted package project across all requested targets (`compile_returned`).

Source discovery (`resolve_project_sources`) has already executed the project's
`build.omg` activations and returned `Vec<ResolvedPackageBuild>`. Here we:
1. Settle those returned builds (`settle_source_builds`) into `CompletedBuild`s
   without re-running `build.omg` or opening a second resource grant.
2. Admit the target-product requests through
   `package_manager::operations::admit_project_products`.
3. Compile all admitted package plans together (`compile_packages`) so targets
   share source checking and invocation-wide budgets.
4. Reconcile the results and dispose staging via `admitted.completion.complete`.

*/

use crate::BuildSession;
use package_manager::operations::{ProjectCompilationAdmissionRequest, ProjectCompilationOutcome};

pub(crate) struct ProjectCompilationRequest<'request> {
    pub prepared: package_manager::operations::PreparedLocalProject,
    pub target_profiles: &'request [target::TargetProfile],
    pub snapshot: Option<&'request build_settings::BuildSnapshotRequest>,
    pub verify: bool,
    pub timings: bool,
    pub product: compiler::CompilationProduct,
}

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
    let mut completed =
        match crate::compile::build_execution::source_resolution::settle_source_builds(
            builds,
            &mut session.compiler,
            execution_profile,
        ) {
            Ok(completed) => completed,
            Err(crate::ProjectSourceResolutionError::Compilation(error)) => {
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
            Err(error) => {
                let mut reason = error.to_string();
                if let Err(error) = session.close() {
                    reason.push_str(&format!("; session disposal: {error}"));
                }
                return request.refuse(reason);
            }
        };
    let admitted = package_manager::operations::admit_project_products(
        request,
        &mut session.packages,
        execution_profile,
    );
    let products = crate::package::package_compilation::compile_packages(
        &mut session.compiler,
        admitted.requests,
        &mut completed,
    );
    admitted
        .completion
        .complete(products, &mut session.packages)
}
