//! Complete Terminal-to-native admission before abstract optimization.
//!
//! This stage owns the ordering of canonical input preparation, executable
//! scope selection, provider
//! settlement, installation and mechanism review. The entry, fused-service and
//! boundary-coverage custody arrive from the native proposal this compile
//! derived for the same artifact and are not replayed here. The compiler
//! supplies policy and consumes the result; it does not rebuild this
//! admission between stages.

use crate::provider_admission::{
    AdmittedNativeProviders, ProviderAdmissionRequest, admit_native_providers,
};
use crate::{ArtifactLoweringError, VerifiedNativeArtifactInput};
use boundary_applications::TerminalBoundaryApplicationCoverage;
use diagnostics::Diagnostic;
use terminal_codec::CanonicalTerminalArtifact;

/// Independent native custody beside one canonical Terminal product.
pub struct NativeAdmissionRequest<'request> {
    /// Exact provider and entry custody plus the receiving authority policies.
    /// An absent permission policy makes no receiver-admission claim; it is
    /// neither an empty policy nor permission inferred from accepted packages.
    pub providers: ProviderAdmissionRequest<'request, 'request>,
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

/// Admit the artifact's canonical input and its providers. The entry
/// settlement, fused establishments and boundary coverage arrive from the
/// proposal this compile derived for this artifact, so admission does not
/// replay their artifact joins again (pipeline.md, "Verification placement").
pub fn admit_native_realization<'request>(
    artifact: &CanonicalTerminalArtifact,
    request: &NativeAdmissionRequest<'request>,
) -> Result<AdmittedNativeRealization<'request>, Vec<Diagnostic>> {
    let input =
        VerifiedNativeArtifactInput::prepare(artifact, &[]).map_err(artifact_lowering_error)?;
    let adapters = crate::provider_admission::project_selected_provider_adapters(
        request.providers.selected_provider_plans,
        input.plan(),
    )
    .map_err(|error| realization_error("selected checked-provider projection", error))?;
    let input = input
        .select_target_execution(
            request.providers.program_entry.source().target_slot().owner,
            &adapters,
        )
        .map_err(|error| realization_error("execution scope", error))?;
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
                    boundary.identity == settlement.provider_execution.requirement_identity()
                        || request
                            .providers
                            .selected_provider_plans
                            .specialized_requirement_row(&boundary.identity)
                            .is_some_and(|row| {
                                row.selected_requirement_identity
                                    == settlement.provider_execution.requirement_identity()
                                    && row.provider_plan_report_identity
                                        == settlement
                                            .provider_execution
                                            .provider_plan_report_identity()
                            })
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
            input.plan().functions.iter().flat_map(|function| &function.operations).any(|operation|
                matches!(operation, abstract_operations::AbstractOperation::BoundaryCall { psi_operation, .. } if *psi_operation == callback.terminal_operation))
                || !input
                    .context()
                    .module()
                    .machines
                    .iter()
                    .flat_map(|machine| &machine.blocks)
                    .flat_map(|block| &block.operations)
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
        target: request.providers.target,
        program_entry: request.providers.program_entry,
        selected_provider_plans: request.providers.selected_provider_plans,
        external_binding_rows: request.providers.external_binding_rows,
        settlements: &settlements,
        compiler_builtins: &compiler_builtins,
        native_callbacks: &native_callbacks,
        terminal_authority_policy: request.providers.terminal_authority_policy,
        terminal_authority_permission_policy: request
            .providers
            .terminal_authority_permission_policy,
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

pub(crate) fn artifact_lowering_error(error: ArtifactLoweringError) -> Vec<Diagnostic> {
    match error {
        ArtifactLoweringError::CanonicalArtifact(error) => {
            realization_error("canonical artifact replay", error)
        }
        ArtifactLoweringError::SemanticDecode(error) => {
            realization_error("semantic section", error)
        }
        error => realization_error("native artifact lowering", error),
    }
}

fn realization_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}
