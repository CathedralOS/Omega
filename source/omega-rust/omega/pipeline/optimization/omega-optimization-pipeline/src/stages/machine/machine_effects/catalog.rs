use omega_isa_aarch64::{aarch64_machine_effect_catalog, validate_aarch64_machine_effect_catalog};
use omega_isa_x86_64::{validate_x86_64_machine_effect_catalog, x86_64_machine_effect_catalog};

use crate::StagedOptimizedSelectedInstructions;

use super::model::OptimizedMachineEffectPipelineError;

pub(super) fn validated_catalog(
    source: &StagedOptimizedSelectedInstructions,
) -> Result<
    omega_selected_instructions::ValidatedMachineEffectCatalog,
    OptimizedMachineEffectPipelineError,
> {
    let target = source.optimized_target().target();
    let constraints = source.register_environment().constraints();
    match target.architecture {
        omega_target::Architecture::X86_64 => {
            let catalog = x86_64_machine_effect_catalog(target, constraints)
                .map_err(OptimizedMachineEffectPipelineError::X86_64Catalog)?;
            validate_x86_64_machine_effect_catalog(target, constraints, catalog)
                .map_err(OptimizedMachineEffectPipelineError::X86_64Catalog)
        }
        omega_target::Architecture::Aarch64 => {
            let catalog = aarch64_machine_effect_catalog(target, constraints)
                .map_err(OptimizedMachineEffectPipelineError::Aarch64Catalog)?;
            validate_aarch64_machine_effect_catalog(target, constraints, catalog)
                .map_err(OptimizedMachineEffectPipelineError::Aarch64Catalog)
        }
    }
}
