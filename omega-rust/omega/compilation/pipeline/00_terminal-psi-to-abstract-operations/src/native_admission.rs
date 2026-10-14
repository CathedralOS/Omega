//! Complete Terminal-to-native admission before abstract optimization.
//!
//! This stage owns the ordering of canonical input preparation, executable
//! scope selection, provider settlement, installation and mechanism review.
//! The native proposal derives each body's boundary coverage and selected
//! provider joins once. Application-entry and fused-service custody belong to
//! receiver/image production, not admission of a private callback body. The
//! compiler supplies policy and consumes the result; it does not rebuild this
//! admission between stages.

use crate::provider_admission::{
    AdmittedNativeProviders, ProviderAdmissionRequest, admit_native_providers, realization_error,
};
use crate::{ArtifactLoweringError, VerifiedNativeArtifactInput};
use boundary_applications::TerminalBoundaryApplicationCoverage;
use diagnostics::Diagnostic;
use terminal_codec::CanonicalTerminalArtifact;

/// Independent native custody beside one canonical Terminal product.
#[derive(Clone, Copy)]
pub struct NativeAdmissionRequest<'request, 'execution> {
    /// Exact retained callable selected by the application's checked custody.
    pub entry: terminal_psi::TargetMachineCallable,
    pub private_entry_ram: &'request [terminal_installation::CheckedPrivateEntryRamRecipe],
    /// Canonical boundary obligations required by this deployment's receiver.
    /// They retain selected bodies but grant no installation or runtime custody.
    pub receiving_provider_roots: &'request [semantic_vocabulary::BoundaryMachineId],
    /// Exact provider custody plus the receiving authority policies.
    /// An absent permission policy makes no receiver-admission claim; it is
    /// neither an empty policy nor permission inferred from accepted packages.
    pub providers: ProviderAdmissionRequest<'request, 'execution>,
    /// The boundary-application coverage the proposal's producer settled
    /// against this artifact's sealed checked scope.
    pub boundary_application_coverage: Option<&'request TerminalBoundaryApplicationCoverage>,
    /// Retained exclusions also constrain physical authority when no receiving
    /// permission policy is supplied.
    pub behavior_exclusions: &'request effects::BehaviorExclusions,
}

/// Complete stage-00 native input. No physical work or publication has occurred.
pub struct AdmittedNativeRealization<'execution> {
    pub input: VerifiedNativeArtifactInput,
    pub providers: AdmittedNativeProviders<'execution>,
    pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
}

/// Admit the artifact's canonical input and its providers. Boundary coverage
/// and selected provider joins arrive from the proposal this compile derived
/// for this artifact, so admission does not replay their artifact joins again
/// (pipeline.md, "Verification placement"). Roster borrows are invocation-local;
/// admitted execution evidence keeps its provider-owned lifetime.
pub fn admit_native_realization<'execution>(
    artifact: &CanonicalTerminalArtifact,
    request: &NativeAdmissionRequest<'_, 'execution>,
) -> Result<AdmittedNativeRealization<'execution>, Vec<Diagnostic>> {
    let input = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
        artifact,
        request.providers.profile,
        request.entry,
        request.private_entry_ram,
    )
    .map_err(artifact_lowering_error)?;
    let input = input
        .select_target_execution(
            artifact,
            request.providers.profile,
            request.providers.checked_adapters,
            request.receiving_provider_roots,
        )
        .map_err(|error| realization_error("execution scope", error))?
        .bind_selected_family_realizations(artifact, request.providers.selected_provider_plans)
        .map_err(|error| realization_error("selected family realization", error))?;
    // Invocation-scoped supplies must answer the same executable occurrences
    // as the plan. Complete declarations stay retained in canonical Terminal.
    let settlements = request
        .providers
        .settlements
        .iter()
        .copied()
        .filter(|settlement| {
            let mut matching = input
                .context()
                .module()
                .boundary_machines
                .iter()
                .filter(|boundary| {
                    request
                        .providers
                        .selected_provider_plans
                        .selected_row_matches_requirement(
                            input
                                .context()
                                .module()
                                .identities
                                .text(boundary.identity)
                                .unwrap_or_default(),
                            settlement.provider_execution.requirement_identity(),
                            settlement
                                .provider_execution
                                .provider_plan_report_identity(),
                        )
                })
                .peekable();
            // Unknown supplies still reach admission and refuse there. Only
            // a declared, out-of-scope occurrence may be omitted by selection.
            matching.peek().is_none()
                || matching.any(|boundary| {
                    input
                        .plan()
                        .boundary_machines
                        .iter()
                        .any(|executed| executed.id == boundary.id)
                })
        })
        .collect::<Vec<_>>();
    let compiler_builtins = request
        .providers
        .compiler_builtins
        .iter()
        .copied()
        .filter(|proposal| {
            input
                .plan()
                .boundary_machines
                .iter()
                .any(|boundary| boundary.id == proposal.boundary)
                || !input
                    .context()
                    .module()
                    .boundary_machines
                    .iter()
                    .any(|boundary| boundary.id == proposal.boundary)
        })
        .collect::<Vec<_>>();
    let native_callbacks = request
        .providers
        .native_callbacks
        .iter()
        .filter(|callback| {
            input.plan().operations().any(|operation|
                matches!(operation, abstract_operations::AbstractOperation::BoundaryCall { psi_operation, .. } if *psi_operation == callback.terminal_operation))
                || !input
                    .context()
                    .module()
                    .machines
                    .iter()
                    .flat_map(|machine| machine.operations())
                    .any(|operation| {
                        operation.id == callback.terminal_operation
                            && matches!(
                                operation.kind,
                                terminal_psi::OperationKind::BoundaryCall { .. }
                            )
                    })
        })
        .cloned()
        .collect::<Vec<_>>();
    let scoped_providers = ProviderAdmissionRequest {
        profile: request.providers.profile,
        selected_provider_plans: request.providers.selected_provider_plans,
        checked_adapters: request.providers.checked_adapters,
        external_binding_rows: request.providers.external_binding_rows,
        settlements: &settlements,
        compiler_builtins: &compiler_builtins,
        native_callbacks: &native_callbacks,
        terminal_authority_policy: request.providers.terminal_authority_policy,
        terminal_authority_permission_policy: request
            .providers
            .terminal_authority_permission_policy,
        x86_scalar_fma_provider: request.providers.x86_scalar_fma_provider,
    };
    let providers = admit_native_providers(&input, artifact, &scoped_providers)?;
    // Exclusions constrain the exercised mechanism closure even when the
    // caller makes no receiver-permission claim.
    request
        .behavior_exclusions
        .validate_physical_authority_closure(&providers.terminal_authority_closure_review)?;
    Ok(AdmittedNativeRealization {
        input,
        providers,
        boundary_application_coverage: request.boundary_application_coverage.cloned(),
    })
}

pub fn artifact_lowering_error(error: ArtifactLoweringError) -> Vec<Diagnostic> {
    match error {
        ArtifactLoweringError::SemanticDecode(error) => {
            realization_error("semantic section", error)
        }
        error => realization_error("native artifact lowering", error),
    }
}
