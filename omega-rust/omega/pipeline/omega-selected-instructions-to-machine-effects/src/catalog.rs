use omega_isa_aarch64::{aarch64_machine_effect_catalog, validate_aarch64_machine_effect_catalog};
use omega_isa_x86_64::{validate_x86_64_machine_effect_catalog, x86_64_machine_effect_catalog};

use omega_register_model::ValidatedRegisterConstraintCatalog;
use omega_target::NativeTarget;

use super::MachineEffectStageError;

pub(super) fn validated_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<omega_selected_instructions::ValidatedMachineEffectCatalog, MachineEffectStageError> {
    match target.architecture {
        omega_target::Architecture::X86_64 => {
            let catalog = x86_64_machine_effect_catalog(target, constraints)
                .map_err(MachineEffectStageError::X86_64Catalog)?;
            validate_x86_64_machine_effect_catalog(target, constraints, catalog)
                .map_err(MachineEffectStageError::X86_64Catalog)
        }
        omega_target::Architecture::Aarch64 => {
            let catalog = aarch64_machine_effect_catalog(target, constraints)
                .map_err(MachineEffectStageError::Aarch64Catalog)?;
            validate_aarch64_machine_effect_catalog(target, constraints, catalog)
                .map_err(MachineEffectStageError::Aarch64Catalog)
        }
    }
}
