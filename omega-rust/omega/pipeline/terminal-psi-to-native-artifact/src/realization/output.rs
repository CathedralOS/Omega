use crate::realization::diagnostics::realization_error;
use crate::realization::model::{NativeRealizationCoreRequest, RequestedNativeArtifact};
use diagnostics::Diagnostic;
use machine_code::{MachineCodePlan, MachineCodePlanWithPrivateFunctions};
use native_artifact::{
    DynamicElfNativeArtifact, DynamicElfNativeArtifactEmissionParts, NativeArtifact,
    NativeArtifactEmissionParts, NativeProviderExecution, NativeSelectedProviderClosureDigest,
    NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest,
};

pub(crate) fn assemble_requested_native_artifact(
    psi_artifact: terminal_codec::CanonicalTerminalArtifact,
    machine_code: &MachineCodePlanWithPrivateFunctions,
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
    if !machine_code.private_functions.is_empty() && !request.ieee_float_fma.is_empty() {
        return Err(realization_error(
            "terminal object construction",
            "compiler-private callback functions cannot yet share the feature-authorized x86 FMA object cohort",
        ));
    }
    validate_ieee_float_fma_rejoin(&machine_code.plan, request)?;
    let object = match (
        machine_code.private_functions.is_empty(),
        request.ieee_float_fma.first(),
    ) {
        (false, None) => image_emission::build_object_artifact_with_private_functions(machine_code),
        (true, Some(first)) => image_emission::build_admitted_x86_fma_object_artifact(
            &machine_code.plan,
            first.provider,
        ),
        (true, None) => image_emission::build_object_artifact(&machine_code.plan),
        (false, Some(_)) => unreachable!("mixed callback/FMA cohort rejected above"),
    }
    .map_err(|error| realization_error("terminal object construction", error))?;
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

fn validate_ieee_float_fma_rejoin(
    machine_code: &MachineCodePlan,
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let occurrences = machine_code
        .functions
        .iter()
        .flat_map(|function| &function.x86_scalar_fma_occurrences)
        .collect::<Vec<_>>();
    if occurrences.len() != request.ieee_float_fma.len() {
        return Err(realization_error(
            "nearest-FMA native rejoin",
            "machine emission did not retain every admitted occurrence exactly once",
        ));
    }
    let mut operations = std::collections::BTreeSet::new();
    for settlement in request.ieee_float_fma {
        if !operations.insert(settlement.terminal_operation) {
            return Err(realization_error(
                "nearest-FMA native rejoin",
                "request repeats one Terminal occurrence",
            ));
        }
        let matching = occurrences
            .iter()
            .filter(|occurrence| occurrence.terminal_operation == settlement.terminal_operation)
            .collect::<Vec<_>>();
        let [occurrence] = matching.as_slice() else {
            return Err(realization_error(
                "nearest-FMA native rejoin",
                "one admitted Terminal occurrence does not rejoin exactly one machine operation",
            ));
        };
        let format = match occurrence.format {
            machine_code::X86ScalarFmaFormat::Binary32 => {
                semantic_vocabulary::IeeeFloatFormat::Binary32
            }
            machine_code::X86ScalarFmaFormat::Binary64 => {
                semantic_vocabulary::IeeeFloatFormat::Binary64
            }
        };
        if format != settlement.format
            || occurrence.slot != settlement.slot
            || occurrence.admitted_provider != settlement.provider
            || occurrence.provider_plan_report_identity
                != settlement.provider_plan.report_fingerprint()
            || occurrence.provider_plan_digest
                != *settlement.provider_plan.identity_digest().as_bytes()
            || request
                .ieee_float_fma
                .first()
                .is_some_and(|first| first.provider != settlement.provider)
        {
            return Err(realization_error(
                "nearest-FMA native rejoin",
                "machine occurrence changed its exact plan, format, slot, or admitted provider",
            ));
        }
    }
    Ok(())
}
