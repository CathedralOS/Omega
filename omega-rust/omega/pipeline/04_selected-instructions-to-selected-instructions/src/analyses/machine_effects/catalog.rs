use std::sync::{Mutex, OnceLock};

use target_operations_to_selected_instructions::isa_aarch64::{
    aarch64_machine_effect_catalog, validate_aarch64_machine_effect_catalog,
};
use target_operations_to_selected_instructions::isa_x86_64::{
    validate_x86_64_machine_effect_catalog, x86_64_machine_effect_catalog,
};

use target::NativeTarget;
use target_operations_to_selected_instructions::ValidatedMachineEffectCatalog;
use target_operations_to_selected_instructions::register_model::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    ValidatedRegisterConstraintCatalog,
};

use super::MachineEffectStageError;

/// Validated catalogs already built in this process, keyed by the target and
/// the content identities of the register constraints and physical model
/// they were built from.
type CatalogKey = (
    NativeTarget,
    RegisterConstraintCatalogIdentity,
    PhysicalRegisterModelIdentity,
);

/// The validated effect catalog is a pure function of the target and the
/// exact register-constraint catalog. Every function's effect analysis and
/// revalidation asked for it, and each request rebuilt the whole table,
/// rebuilding the physical register model per declaration, then revalidated
/// it. Each distinct table is now built and validated once per process; a
/// failed build is not remembered, so it fails again on every request.
pub(super) fn validated_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<ValidatedMachineEffectCatalog, MachineEffectStageError> {
    static CATALOGS: OnceLock<Mutex<Vec<(CatalogKey, ValidatedMachineEffectCatalog)>>> =
        OnceLock::new();
    let catalogs = CATALOGS.get_or_init(Mutex::default);
    let key = (
        target,
        constraints.identity(),
        constraints.physical_identity(),
    );
    if let Ok(catalogs) = catalogs.lock()
        && let Some((_, catalog)) = catalogs.iter().find(|(candidate, _)| *candidate == key)
    {
        return Ok(catalog.clone());
    }
    let catalog = build_validated_catalog(target, constraints)?;
    if let Ok(mut catalogs) = catalogs.lock()
        && !catalogs.iter().any(|(candidate, _)| *candidate == key)
    {
        catalogs.push((key, catalog.clone()));
    }
    Ok(catalog)
}

fn build_validated_catalog(
    target: NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<ValidatedMachineEffectCatalog, MachineEffectStageError> {
    match target.architecture {
        target::Architecture::X86_64 => {
            let catalog = x86_64_machine_effect_catalog(target, constraints)
                .map_err(MachineEffectStageError::X86_64Catalog)?;
            validate_x86_64_machine_effect_catalog(target, constraints, catalog)
                .map_err(MachineEffectStageError::X86_64Catalog)
        }
        target::Architecture::Aarch64 => {
            let catalog = aarch64_machine_effect_catalog(target, constraints)
                .map_err(MachineEffectStageError::Aarch64Catalog)?;
            validate_aarch64_machine_effect_catalog(target, constraints, catalog)
                .map_err(MachineEffectStageError::Aarch64Catalog)
        }
    }
}
