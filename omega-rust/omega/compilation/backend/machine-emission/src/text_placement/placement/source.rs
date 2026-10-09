//! Input-only relocation eligibility; source admission remains with the caller.
use super::{TextPlacementError, TextPlacementInput};
pub(super) fn validate(input: TextPlacementInput<'_>) -> Result<(), TextPlacementError> {
    match input {
        TextPlacementInput::InternalCalls(_) => Ok(()),
    }
}
