//! Optimizer module role: executable entrance. Current fragments to dense text placement.
//!
//! Source and frame admission belong to the caller. These entrances transform
//! raw data and check its exact placement; neither grants publication authority.
mod conversion;
pub(crate) mod custody;
mod error;
mod production;
mod source;
mod statistics;
pub use statistics::text_section_statistics;
mod validation;

pub use error::TextPlacementError;
use machine_code::{FunctionFragmentEmissionPlan, RelocationFreeTextSectionPlacement};

#[derive(Clone, Copy)]
pub enum TextPlacementInput<'a> {
    RelocationFree(&'a FunctionFragmentEmissionPlan),
    InternalCalls(&'a FunctionFragmentEmissionPlan),
}
impl<'a> TextPlacementInput<'a> {
    fn fragments(self) -> &'a FunctionFragmentEmissionPlan {
        match self {
            Self::RelocationFree(fragments) | Self::InternalCalls(fragments) => fragments,
        }
    }
}
pub fn place_fragment_text_section(
    input: TextPlacementInput<'_>,
) -> Result<RelocationFreeTextSectionPlacement, TextPlacementError> {
    source::validate(input)?;
    let section = match input {
        TextPlacementInput::RelocationFree(fragments) => production::relocation_free(fragments)?,
        TextPlacementInput::InternalCalls(fragments) => production::fixed_frame(fragments)?,
    };
    validation::check(input, &section)?;
    Ok(section)
}
pub fn validate_fragment_text_section(
    input: TextPlacementInput<'_>,
    candidate: &RelocationFreeTextSectionPlacement,
) -> Result<(), TextPlacementError> {
    source::validate(input)?;
    validation::check(input, candidate)
}
