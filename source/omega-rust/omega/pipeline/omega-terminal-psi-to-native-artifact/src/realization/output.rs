use crate::realization::diagnostics::realization_error;
use crate::realization::model::NativeRealizationRequest;
use omega_machine_code::MachineCodePlan;
use omega_native_artifact::{
    NativeArtifact, NativeArtifactEmissionParts, NativeProviderExecution,
    NativeSelectedProviderClosureDigest, NativeSelectedProviderPlan,
    NativeSelectedProviderPlanDigest,
};
use psi_diagnostics::Diagnostic;

pub(crate) fn assemble_native_artifact(
    psi_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    machine_code: &MachineCodePlan,
    provider_executions: Vec<NativeProviderExecution>,
    physical_evidence_scope: omega_native_artifact::NativePhysicalEvidenceScope,
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
                plan.report_fingerprint(),
                NativeSelectedProviderPlanDigest::from_digest(*plan.identity_digest().as_bytes()),
                plan.rows
                    .iter()
                    .map(|row| row.requirement_identity.clone())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    selected_provider_plans.sort_by_key(NativeSelectedProviderPlan::report_identity);
    NativeArtifact::from_emitted_parts(NativeArtifactEmissionParts {
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
        physical_evidence_scope,
    })
    .map_err(|error| realization_error("native artifact replay", error))
}
