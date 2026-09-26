use super::miniature_model;
use crate::register_model::{
    RegisterModelValidationError, RegisterUnitId, RegisterViewId, RegisterWriteSemantics,
    validate_physical_register_model,
};

#[test]
fn validator_accepts_a_closed_model_and_rejects_noncanonical_units() {
    assert!(validate_physical_register_model(miniature_model()).is_ok());
    let mut duplicate = miniature_model();
    duplicate.views[0].units.push(RegisterUnitId(0));
    assert_eq!(
        validate_physical_register_model(duplicate),
        Err(RegisterModelValidationError::NonCanonicalUnitSet)
    );
}

#[test]
fn validator_rejects_a_false_zero_extension_footprint() {
    let mut false_zero_extension = miniature_model();
    false_zero_extension.views[0].write_semantics = RegisterWriteSemantics::ZeroExtendsParent;
    assert_eq!(
        validate_physical_register_model(false_zero_extension),
        Err(RegisterModelValidationError::WriteFootprintMismatch(
            RegisterViewId(0)
        ))
    );
}
