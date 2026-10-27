//! Direct source-to-object correspondence: a retained plan is exactly what the
//! placed text constructs.
//!
//! The correspondence has one owner. `construct_relocation_free_object_from_text`
//! derives every object field the placed text pins; admission re-derives the
//! plan here and compares it whole instead of restating which derived fields the
//! source decided.

use crate::{
    RelocationFreeObjectFromTextError, RelocationFreeObjectPlan,
    RelocationFreeTextSectionPlacement, construct_relocation_free_object_from_text,
    validate_relocation_free_object,
};
use optimization_core::OptimizationSelectionIdentity;
pub(crate) fn validate_relocation_free_object_from_text(
    text: &RelocationFreeTextSectionPlacement,
    selections: OptimizationSelectionIdentity,
    object: &RelocationFreeObjectPlan,
) -> Result<(), RelocationFreeObjectFromTextError> {
    // Canonical admission first: a plan that is not canonical at all answers
    // `InvalidObject`, never a source disagreement.
    validate_relocation_free_object(object)
        .map_err(RelocationFreeObjectFromTextError::InvalidObject)?;
    // A retained plan is this source's object exactly when it equals what the
    // source constructs: symbol ids, names, roles, linkage, the text spelling
    // and the relocation record count all follow from the source through that
    // one constructor.
    if *object == construct_relocation_free_object_from_text(text, selections)? {
        Ok(())
    } else {
        Err(RelocationFreeObjectFromTextError::SourceMismatch)
    }
}
