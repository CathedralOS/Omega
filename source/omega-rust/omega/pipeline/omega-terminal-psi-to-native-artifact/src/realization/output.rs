use crate::realization::diagnostics::realization_error;
use crate::realization::model::NativeRealizationRequest;
use omega_machine_code::{MachineCodePlan, MachineCodePlanWithPrivateFunctions};
use omega_native_artifact::{
    NativeArtifact, NativeArtifactEmissionParts, NativeProviderExecution,
    NativeSelectedProviderClosureDigest, NativeSelectedProviderPlan,
    NativeSelectedProviderPlanDigest,
};
use psi_diagnostics::Diagnostic;

pub(crate) fn assemble_native_artifact(
    psi_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    machine_code: &MachineCodePlanWithPrivateFunctions,
    provider_executions: Vec<NativeProviderExecution>,
    terminal_authority_policy_identity: omega_effects::TerminalAuthorityPolicyIdentity,
    boundary_application_coverage: Option<
        omega_boundary_applications::TerminalBoundaryApplicationCoverage,
    >,
    physical_evidence_scope: omega_native_artifact::NativePhysicalEvidenceScope,
    request: &NativeRealizationRequest<'_>,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    if !machine_code.private_functions.is_empty() {
        return Err(realization_error(
            "terminal object construction",
            "compiler-private callback functions require private object-symbol custody",
        ));
    }
    validate_ieee_float_fma_rejoin(&machine_code.plan, request)?;
    let object = match request.ieee_float_fma.first() {
        Some(first) => omega_image_emission::build_admitted_x86_fma_object_artifact(
            &machine_code.plan,
            first.provider,
        ),
        None => omega_image_emission::build_object_artifact(&machine_code.plan),
    }
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
        terminal_authority_policy_identity,
        boundary_application_coverage,
        physical_evidence_scope,
    })
    .map_err(|error| realization_error("native artifact replay", error))
}

fn validate_ieee_float_fma_rejoin(
    machine_code: &MachineCodePlan,
    request: &NativeRealizationRequest<'_>,
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
            omega_machine_code::X86ScalarFmaFormat::Binary32 => psi_core::IeeeFloatFormat::Binary32,
            omega_machine_code::X86ScalarFmaFormat::Binary64 => psi_core::IeeeFloatFormat::Binary64,
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
