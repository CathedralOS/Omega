use omega_isa_x86_64::{
    X86_64_COMPARE_I64_ZERO, X86_64RegisterConstraintCatalogValidationError,
    x86_64_physical_register_model, x86_64_register_constraint_catalog,
};
use omega_register_model::validate_physical_register_model;
use omega_target::{Architecture, NativeTarget};

use super::super::*;

#[test]
fn raw_join_rejects_target_drift_and_target_semantic_corruption() {
    let raw = x86_64_physical_register_model();
    let physical = validate_physical_register_model(raw.clone()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&physical);
    assert_eq!(
        validate_target_register_environment(
            NativeTarget::linux_arm64(),
            raw.clone(),
            catalog.clone()
        ),
        Err(
            TargetRegisterEnvironmentValidationError::TargetArchitectureMismatch {
                target: Architecture::Aarch64,
                model: Architecture::X86_64,
            }
        )
    );

    let mut corrupted = catalog;
    let compare = corrupted
        .constraints
        .iter_mut()
        .find(|constraint| constraint.key == X86_64_COMPARE_I64_ZERO)
        .unwrap();
    compare.implicit_defs.clear();
    assert!(matches!(
        validate_target_register_environment(NativeTarget::linux_x64(), raw, corrupted),
        Err(TargetRegisterEnvironmentValidationError::X86_64(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                X86_64_COMPARE_I64_ZERO
            )
        ))
    ));

    let canonical = x86_64_physical_register_model();
    let canonical_validated = validate_physical_register_model(canonical.clone()).unwrap();
    let canonical_catalog = x86_64_register_constraint_catalog(&canonical_validated);
    let mut forged = canonical;
    forged.views[0].name = "forged.rax".into();
    assert_eq!(
        validate_target_register_environment(NativeTarget::linux_x64(), forged, canonical_catalog,),
        Err(TargetRegisterEnvironmentValidationError::X86_64(
            X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel,
        ))
    );
}
