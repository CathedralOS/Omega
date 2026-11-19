//! Prepare executable output from the accepted program and settled providers.
//! The entry receipt, arrival plans and permission rows all describe that same
//! artifact; application completion owns native compilation and publication.

use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use products::PublishedCompilation;

pub(super) fn prepare(
    checked: &PublishedCompilation,
    proposal: Option<backend_plan::TerminalTargetProviderProposal>,
    selections: &OptimizationSelections,
    request: Option<&products::NativeProductRequest>,
) -> Result<
    (
        products::RetainedTerminalArtifact,
        native_compiler::TerminalAuthorityPermissionPolicy,
        Option<Vec<u8>>,
    ),
    Vec<Diagnostic>,
> {
    let profile = checked
        .execution()
        .target_profile()
        .unwrap_or_else(target::TargetProfile::host);
    native_compiler::prepare_published_application(native_compiler::PublishedApplicationInputs {
        artifact: checked.program().artifact(),
        execution: checked.execution(),
        proposal,
        bindings: checked.package_inputs().accepted_semantic_bindings(),
        package_permissions: checked.package_inputs().declaration_permissions(),
        application: checked.native_application_metadata(),
        optimizations: selections,
        behavior_exclusions: checked.behavior_exclusions(),
        x86_scalar_fma_provider: checked.x86_deployment_claim().admit_for(Some(profile)),
        audit_assumptions: request
            .filter(|request| request.retain_native_inputs)
            .map(|request| request.installation_assumptions.as_slice()),
    })
}
