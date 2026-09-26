//! Assembly of the requested native artifact from the emitted object and
//! its retained realization evidence.

use crate::compiler::native::native_realization::realization_diagnostics::realization_error;
use crate::compiler::native::native_realization::realization_request::{
    NativeRealizationRequest, RequestedNativeArtifact,
};
use diagnostics::Diagnostic;
use resolved_layout_to_resolved_layout::native_artifact::{
    DynamicElfNativeArtifact, DynamicElfNativeArtifactEmissionParts, NativeArtifact,
    NativeArtifactEmissionParts, NativeProviderExecution, NativeSelectedProviderClosureDigest,
    NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest,
};

pub(crate) fn assemble_requested_native_artifact(
    psi_artifact: terminal_codec::CanonicalTerminalArtifact,
    object: resolved_layout_to_resolved_layout::image_emission::ObjectArtifact,
    provider_executions: Vec<NativeProviderExecution>,
    terminal_authority_policy_identity: abstract_operations_to_target_operations::effects::TerminalAuthorityPolicyIdentity,
    terminal_authority_permission_policy_identity: Option<
        abstract_operations_to_target_operations::effects::TerminalAuthorityPermissionPolicyIdentity,
    >,
    terminal_authority_closure_review: abstract_operations_to_target_operations::effects::TerminalAuthorityClosureReviewReceipt,
    boundary_application_coverage: Option<
        resolved_layout_to_resolved_layout::boundary_applications::TerminalBoundaryApplicationCoverage,
    >,
    physical_evidence_scope: resolved_layout_to_resolved_layout::native_artifact::NativePhysicalEvidenceScope,
    image_request: resolved_layout_to_resolved_layout::image_emission::ExecutableImageEmissionRequest,
    request: &NativeRealizationRequest<'_>,
) -> Result<RequestedNativeArtifact, Vec<Diagnostic>> {
    let image = resolved_layout_to_resolved_layout::image_emission::emit_executable_image(
        &object,
        image_request,
    )
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
        resolved_layout_to_resolved_layout::image_emission::RequestedExecutableImage::Direct(image) => {
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
        resolved_layout_to_resolved_layout::image_emission::RequestedExecutableImage::DynamicElf(image) => {
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
