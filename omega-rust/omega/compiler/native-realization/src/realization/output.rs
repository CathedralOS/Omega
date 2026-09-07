use crate::realization::diagnostics::realization_error;
use crate::realization::model::{NativeRealizationCoreRequest, RequestedNativeArtifact};
use diagnostics::Diagnostic;
use native_artifact::{
    DynamicElfNativeArtifact, DynamicElfNativeArtifactEmissionParts, NativeArtifact,
    NativeArtifactEmissionParts, NativeProviderExecution, NativeSelectedProviderClosureDigest,
    NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest,
};

pub(crate) fn assemble_requested_native_artifact(
    psi_artifact: terminal_codec::CanonicalTerminalArtifact,
    object: image_emission::ObjectArtifact,
    provider_executions: Vec<NativeProviderExecution>,
    terminal_authority_policy_identity: effects::TerminalAuthorityPolicyIdentity,
    terminal_authority_permission_policy_identity:
        effects::TerminalAuthorityPermissionPolicyIdentity,
    terminal_authority_closure_review: effects::TerminalAuthorityClosureReviewReceipt,
    boundary_application_coverage: Option<
        boundary_applications::TerminalBoundaryApplicationCoverage,
    >,
    physical_evidence_scope: native_artifact::NativePhysicalEvidenceScope,
    image_request: image_emission::ExecutableImageEmissionRequest,
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<RequestedNativeArtifact, Vec<Diagnostic>> {
    let image = image_emission::emit_requested_executable_image(&object, image_request)
        .map_err(|error| vec![error.diagnostic().clone()])?;

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
    let selected_provider_closure_report_identity = request
        .selected_provider_plans
        .compatibility_report_identity();
    let selected_provider_closure_digest = NativeSelectedProviderClosureDigest::from_digest(
        *request.selected_provider_plans.identity_digest().as_bytes(),
    );
    match image {
        image_emission::RequestedExecutableImage::Direct(image) => {
            NativeArtifact::from_emitted_parts(NativeArtifactEmissionParts {
                target: request.target,
                psi_artifact,
                object,
                image,
                selected_provider_closure_report_identity,
                selected_provider_closure_digest,
                selected_provider_plans,
                provider_executions,
                terminal_authority_policy_identity,
                terminal_authority_permission_policy_identity,
                terminal_authority_closure_review,
                boundary_application_coverage,
                physical_evidence_scope,
            })
            .map(RequestedNativeArtifact::Direct)
            .map_err(|error| realization_error("native artifact replay", error))
        }
        image_emission::RequestedExecutableImage::DynamicElf(image) => {
            DynamicElfNativeArtifact::from_emitted_parts(DynamicElfNativeArtifactEmissionParts {
                target: request.target,
                psi_artifact,
                object,
                image,
                selected_provider_closure_report_identity,
                selected_provider_closure_digest,
                selected_provider_plans,
                provider_executions,
                terminal_authority_policy_identity,
                terminal_authority_permission_policy_identity,
                terminal_authority_closure_review,
                boundary_application_coverage,
                physical_evidence_scope,
            })
            .map(RequestedNativeArtifact::DynamicElf)
            .map_err(|error| realization_error("dynamic ELF native artifact replay", error))
        }
    }
}
