//! Optimizer module role: executable entrance. Machine-effect catalog admission and identity sealing.
//!
//! Admission binds target and register roots, checks the exact semantic roster,
//! delegates row replay, and seals the independently encoded catalog identity.

mod constraint_keys;
mod identity;
mod model;
mod validation;

use std::collections::BTreeSet;

use omega_register_model::ValidatedRegisterConstraintCatalog;

use validation::{validate_declaration, validate_structural_unit_call};

pub use identity::machine_effect_catalog_identity;
pub use model::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalog, MachineEffectCatalogIdentity, MachineEffectCatalogValidationError,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior, StructuralUnitCallBarrier, StructuralUnitCallEffect,
    StructuralUnitCallEffectDeclaration, StructuralUnitCallFrameEffect,
    StructuralUnitCallMemoryEffect, ValidatedMachineEffectCatalog,
};

pub fn validate_machine_effect_catalog(
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: MachineEffectCatalog,
) -> Result<ValidatedMachineEffectCatalog, MachineEffectCatalogValidationError> {
    if catalog.target.architecture != constraints.architecture() {
        return Err(MachineEffectCatalogValidationError::TargetArchitectureMismatch);
    }
    if catalog.register_constraints != constraints.identity() {
        return Err(MachineEffectCatalogValidationError::RegisterConstraintRootMismatch);
    }
    let selected = catalog.selected_keys.in_identity_order();
    if selected.iter().copied().collect::<BTreeSet<_>>().len() != selected.len() {
        return Err(MachineEffectCatalogValidationError::DuplicateSelectedConstraintKey);
    }
    if catalog
        .declarations
        .windows(2)
        .any(|pair| pair[0].semantic >= pair[1].semantic)
    {
        return Err(MachineEffectCatalogValidationError::NonCanonicalDeclarations);
    }
    let expected = MachineSemanticKind::ALL
        .map(|semantic| (semantic, catalog.selected_keys.for_semantic(semantic)));
    if catalog.declarations.len() != expected.len()
        || catalog
            .declarations
            .iter()
            .zip(expected)
            .any(|(actual, (semantic, constraint))| {
                actual.semantic != semantic || actual.constraint != constraint
            })
    {
        return Err(MachineEffectCatalogValidationError::DeclarationRosterMismatch);
    }
    validate_structural_unit_call(constraints, &catalog)?;
    for declaration in &catalog.declarations {
        let row = constraints
            .catalog()
            .constraints
            .iter()
            .find(|row| row.key == declaration.constraint)
            .ok_or(MachineEffectCatalogValidationError::UnknownConstraint(
                declaration.semantic,
            ))?;
        validate_declaration(row, declaration)?;
    }
    let identity = machine_effect_catalog_identity(&catalog);
    Ok(ValidatedMachineEffectCatalog { catalog, identity })
}
