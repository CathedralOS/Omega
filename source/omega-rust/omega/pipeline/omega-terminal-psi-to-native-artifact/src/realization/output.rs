use crate::realization::diagnostics::realization_error;
use crate::realization::model::NativeRealizationRequest;
use omega_machine_code::MachineCodePlan;
use omega_native_artifact::{
    NativeArtifact, NativeArtifactParts, NativeProviderExecution,
    NativeSelectedProviderClosureDigest, NativeSelectedProviderPlan,
};
use psi_diagnostics::Diagnostic;

pub(crate) fn assemble_native_artifact(
    psi_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    machine_code: &MachineCodePlan,
    provider_executions: Vec<NativeProviderExecution>,
    request: &NativeRealizationRequest<'_>,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    let object = omega_image_emission::build_object_artifact(machine_code)
        .map_err(|error| realization_error("terminal object construction", error))?;
    let image = omega_image_emission::emit_executable_image(&object, request.subsystem)
        .map_err(|diagnostic| vec![diagnostic])?;

    let mut selected_provider_plans = request
        .selected_provider_plans
        .plans()
        .iter()
        .map(|plan| {
            NativeSelectedProviderPlan::new(
                plan.identity_fingerprint(),
                plan.rows
                    .iter()
                    .map(|row| row.requirement_identity.clone())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    selected_provider_plans.sort_by_key(NativeSelectedProviderPlan::identity);
    NativeArtifact::from_replayed_parts(NativeArtifactParts {
        target: request.target,
        psi_artifact,
        object,
        image,
        selected_provider_closure_report_identity: request
            .selected_provider_plans
            .compatibility_report_identity(),
        selected_provider_closure_digest: NativeSelectedProviderClosureDigest::from_digest(
            *request.selected_provider_plans.identity_digest().as_bytes(),
        ),
        selected_provider_plans,
        provider_executions,
    })
    .map_err(|error| realization_error("native artifact replay", error))
}
