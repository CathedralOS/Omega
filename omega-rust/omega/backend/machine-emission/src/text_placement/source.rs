//! Input-only relocation eligibility; source admission remains with the caller.
pub(super) mod relocation;
use super::{TextPlacementError, TextPlacementInput};
pub(super) fn validate(input: TextPlacementInput<'_>) -> Result<(), TextPlacementError> {
    match input {
        TextPlacementInput::RelocationFree(fragments) => {
            for function in &fragments.functions {
                relocation::prove_none(function)?;
            }
            Ok(())
        }
        TextPlacementInput::InternalCalls(_) => Ok(()),
    }
}
