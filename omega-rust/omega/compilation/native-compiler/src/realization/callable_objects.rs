// Realize one exact callable while retaining its original Terminal namespace.
// Callback and component export entries use the same source-free admission and
// native pipeline (`pipeline::lower_to_target` and `pipeline::emit_target`)
// while leaving their identity, installation, and publication contracts with
// the caller.

use crate::pipeline::{self, BodyEmissionError, TargetLoweringError};
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::provider_admission::realization_error;

pub(super) struct CallableObjectProduct {
    pub projection: native_artifact::FragmentObjectProjection,
    pub runtime: native_artifact::TerminalCallableRuntimeProvisions,
    pub authority:
        terminal_psi_to_abstract_operations::provider_admission::AdmittedNativeCallableAuthority,
}

pub(super) struct CallableTargetProduct {
    pub code: terminal_codec::RetainedTerminalCode,
    pub coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
    pub target: abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    pub runtime: native_artifact::TerminalCallableRuntimeProvisions,
    pub authority:
        terminal_psi_to_abstract_operations::provider_admission::AdmittedNativeCallableAuthority,
    pub observation: crate::observations::NativePipelineObservation,
}

pub(super) fn prepare_callable_object(
    mut admitted: terminal_psi_to_abstract_operations::AdmittedNativeRealization<'_>,
    admission: terminal_psi_to_abstract_operations::NativeAdmissionRequest<'_, '_>,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    budget: optimization_core::OptimizationWorkBudget,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> Result<CallableTargetProduct, Vec<Diagnostic>> {
    let observation = crate::observations::NativePipelineObservation::new(
        admission.providers.profile,
        crate::observations::NativePipelineUnit::Callable,
    );
    let callable_entry = admitted.input.plan().entry;
    let authority = admitted.providers.callable_authority().clone();
    let host = native_artifact::TerminalRuntimeHostProvisions::capture_native_admission(
        admitted
            .providers
            .installation
            .as_ref()
            .map(|installation| installation.terminal_selection()),
        admitted
            .providers
            .settlements
            .iter()
            .map(|settlement| native_artifact::TerminalRuntimeHostProvision {
                boundary: settlement.boundary,
                realization: settlement.realization.clone(),
            })
            .collect(),
        &admitted.input,
        admission.providers.selected_provider_plans,
        admission.providers.external_binding_rows,
    )
    .map_err(|reason| realization_error("callable runtime companions", reason))?;
    let runtime = native_artifact::TerminalCallableRuntimeProvisions::capture_native_admission(
        artifact.view(),
        &admitted.input,
        callable_entry,
        admission.providers.profile,
        host,
    )
    .map_err(|reason| realization_error("callable runtime companions", reason))?;
    let optimized_target = pipeline::lower_to_target(
        admitted.input,
        abstract_operations_to_target_operations::OptimizedTargetLoweringRequest {
            target: admission.providers.target(),
            settlements: &admitted.providers.settlements,
            installation: admitted.providers.installation.take(),
        },
        selections,
        budget,
        &observation,
    )
    .map_err(|error| match error {
        TargetLoweringError::AbstractOptimization(error) => {
            realization_error("callable abstract optimization", error)
        }
        TargetLoweringError::Target(error) => realization_error("callable target lowering", error),
    })?;
    let runtime =
        runtime.with_borrowed_storage_layouts(optimized_target.borrowed_storage_layouts().to_vec());
    Ok(CallableTargetProduct {
        code: artifact.retain_code(),
        coverage: admission
            .boundary_application_coverage
            .ok_or_else(|| {
                realization_error(
                    "callable physical evidence",
                    "callable has no semantic occurrence coverage",
                )
            })?
            .clone(),
        target: optimized_target,
        runtime,
        authority,
        observation,
    })
}

pub(super) fn emit_prepared_callable_object(
    prepared: CallableTargetProduct,
) -> Result<CallableObjectProduct, Vec<Diagnostic>> {
    let CallableTargetProduct {
        code,
        coverage,
        target: optimized_target,
        runtime,
        authority,
        observation,
    } = prepared;
    let projection =
        pipeline::emit_target(optimized_target, &observation).map_err(|error| match error {
            BodyEmissionError::Physical(error) => {
                realization_error("callable physical pipeline", error)
            }
            BodyEmissionError::Object(diagnostics) => diagnostics,
        })?;
    let projection = projection
        .with_semantic_source(code, coverage)
        .map_err(|reason| realization_error("callable physical evidence", reason))?;
    observation.complete();
    Ok(CallableObjectProduct {
        projection,
        runtime,
        authority,
    })
}

pub(super) fn emit_callable_object(
    admitted: terminal_psi_to_abstract_operations::AdmittedNativeRealization<'_>,
    admission: terminal_psi_to_abstract_operations::NativeAdmissionRequest<'_, '_>,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    budget: optimization_core::OptimizationWorkBudget,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> Result<CallableObjectProduct, Vec<Diagnostic>> {
    emit_prepared_callable_object(prepare_callable_object(
        admitted, admission, selections, budget, artifact,
    )?)
}
