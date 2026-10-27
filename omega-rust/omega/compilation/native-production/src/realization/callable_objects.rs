//! Realize one exact callable while retaining its original Terminal namespace.
//!
//! Callback and component export entries use the same source-free admission and
//! native pipeline. Their identity, installation and publication contracts stay
//! with the callers; this operation emits a body and grants no entry authority.

use crate::pipeline::{self, BodyEmissionError, TargetLoweringError};
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::provider_admission::realization_error;

pub(super) struct CallableObjectProduct {
    pub projection: native_artifact::FragmentObjectProjection,
    pub runtime: native_artifact::TerminalCallableRuntimeProvisions,
    pub authority:
        terminal_psi_to_abstract_operations::provider_admission::AdmittedNativeCallableAuthority,
}

pub(super) fn emit_callable_object(
    admission: terminal_psi_to_abstract_operations::NativeAdmissionRequest<'_, '_>,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    budget: optimization_core::OptimizationWorkBudget,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    entry: terminal_psi::TargetMachineCallable,
) -> Result<CallableObjectProduct, Vec<Diagnostic>> {
    let admission = terminal_psi_to_abstract_operations::NativeAdmissionRequest {
        private_entry_ram: &[],
        entry,
        receiving_provider_roots: &[],
        ..admission
    };
    let mut admitted =
        terminal_psi_to_abstract_operations::admit_native_realization(artifact, &admission)?;
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
        &admitted.input.plan().boundary_machines,
        admission.providers.selected_provider_plans,
        &artifact.semantic_module().identities,
        admission.providers.external_binding_rows,
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
    )
    .map_err(|error| match error {
        TargetLoweringError::AbstractOptimization(error) => {
            realization_error("callable abstract optimization", error)
        }
        TargetLoweringError::Target(error) => realization_error("callable target lowering", error),
    })?;
    let host =
        host.with_borrowed_storage_layouts(optimized_target.borrowed_storage_layouts().to_vec());
    let runtime = native_artifact::TerminalCallableRuntimeProvisions::capture_native_admission(
        artifact.view(),
        callable_entry,
        admission.providers.profile,
        host,
    )
    .map_err(|reason| realization_error("callable runtime companions", reason))?;
    let projection = pipeline::emit_target(optimized_target).map_err(|error| match error {
        BodyEmissionError::Physical(error) => {
            realization_error("callable physical pipeline", error)
        }
        BodyEmissionError::Object(diagnostics) => diagnostics,
    })?;
    Ok(CallableObjectProduct {
        projection,
        runtime,
        authority,
    })
}
