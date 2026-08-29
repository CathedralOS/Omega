use omega_machine_code::FunctionFragmentEmissionPlan;
use omega_target::Architecture;

use crate::{
    validate_aarch64_cbnz_function_relative_realization_custody,
    validate_function_relative_layout_optimization_realization_custody,
    validate_optimized_aarch64_movn_function_relative_realization,
    validate_optimized_active_resident_rematerialization_function_relative_realization,
    validate_optimized_structural_unit_function_relative_realization,
    validate_optimized_unit_function_relative_realization,
    validate_selected_lowering_aarch64_cbnz_function_relative_realization_custody,
    validate_selected_lowering_aarch64_movn_function_relative_realization,
    validate_selected_lowering_function_relative_realization_custody,
};

use super::error::FunctionFragmentEmissionError;
use super::model::{
    StagedFunctionFragmentEmissionCustodyReceipt, ValidatedFunctionFragmentEmissionManifest,
};
use super::source::StagedOptimizedFunctionFragmentEmissionSource;

pub(super) fn validate_source(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<(), FunctionFragmentEmissionError> {
    match source {
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(realization) => {
            validate_function_relative_layout_optimization_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
            if realization.layout().target().architecture != Architecture::X86_64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(
            realization,
        ) => {
            validate_selected_lowering_function_relative_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
            if realization.relaxation().is_none() {
                return Err(FunctionFragmentEmissionError::MissingX86Rel8Realization);
            }
            if realization.layout().target().architecture != Architecture::X86_64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzDirect(realization) => {
            validate_aarch64_cbnz_function_relative_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
            if realization.layout().target().architecture != Architecture::Aarch64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::Aarch64CbnzAfterSelectedLowering(
            realization,
        ) => {
            validate_selected_lowering_aarch64_cbnz_function_relative_realization_custody(
                realization,
            )
            .map_err(FunctionFragmentEmissionError::Source)?;
            if realization.layout().target().architecture != Architecture::Aarch64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::Aarch64MovnDirect(realization) => {
            validate_optimized_aarch64_movn_function_relative_realization(realization)
                .map_err(|_| FunctionFragmentEmissionError::Aarch64MovnSource)?;
            if realization.layout().target().architecture != Architecture::Aarch64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::Aarch64MovnAfterSelectedLowering(
            realization,
        ) => {
            validate_selected_lowering_aarch64_movn_function_relative_realization(realization)
                .map_err(|_| FunctionFragmentEmissionError::Aarch64MovnSource)?;
            if realization.layout().target().architecture != Architecture::Aarch64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(
            realization,
        ) => {
            validate_optimized_active_resident_rematerialization_function_relative_realization(
                realization,
            )
            .map_err(FunctionFragmentEmissionError::ActiveResidentRematerializationSource)?;
        }
        StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(realization) => {
            validate_optimized_unit_function_relative_realization(realization)
                .map_err(FunctionFragmentEmissionError::UnitSource)?;
        }
        StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(realization) => {
            validate_optimized_structural_unit_function_relative_realization(realization)
                .map_err(FunctionFragmentEmissionError::StructuralUnitSource)?;
        }
    }
    Ok(())
}
pub(super) fn receipt(
    manifest: &ValidatedFunctionFragmentEmissionManifest,
    fragments: &FunctionFragmentEmissionPlan,
) -> StagedFunctionFragmentEmissionCustodyReceipt {
    StagedFunctionFragmentEmissionCustodyReceipt {
        source_realization: manifest.record.source_realization,
        fragments: fragments.identity,
        manifest: manifest.record.identity,
    }
}
