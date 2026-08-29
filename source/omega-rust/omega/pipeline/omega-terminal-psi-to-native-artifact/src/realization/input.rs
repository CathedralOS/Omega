use crate::realization::diagnostics::realization_error;
use crate::realization::model::{NativeRealizationInput, NativeRealizationRequest};
use psi_diagnostics::Diagnostic;

pub(crate) fn lower_realization_input(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    request: &NativeRealizationRequest<'_>,
) -> Result<NativeRealizationInput, Vec<Diagnostic>> {
    if request.optimization_selections.is_empty() {
        Ok(NativeRealizationInput::Ordinary(
            omega_psi_to_abstract_operations::lower_artifact_sections(
                semantic_bytes,
                proof_bytes,
                request.profile,
            )
            .map_err(|error| realization_error("ordinary artifact lowering", error))?,
        ))
    } else {
        Ok(NativeRealizationInput::ExplicitOptimization(
            omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                semantic_bytes,
                proof_bytes,
                request.profile,
            )
            .map_err(|error| realization_error("verified optimizer artifact lowering", error))?,
        ))
    }
}
