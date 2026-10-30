use super::{miniature_model, model_rejection};
use crate::{
    RegisterModelValidationError as Rejected, RegisterUnitId, RegisterViewId,
    RegisterWriteSemantics, validate_physical_register_model,
};

#[test]
fn validator_accepts_a_closed_model_and_rejects_noncanonical_units() {
    assert!(validate_physical_register_model(miniature_model()).is_ok());
    model_rejection(
        |model| model.views[0].units.push(RegisterUnitId(0)),
        Rejected::NonCanonicalUnitSet,
    );
}

#[test]
fn validator_rejects_a_false_zero_extension_footprint() {
    model_rejection(
        |model| model.views[0].write_semantics = RegisterWriteSemantics::ZeroExtendsParent,
        Rejected::WriteFootprintMismatch(RegisterViewId(0)),
    );
}
