//! Placement of current fragments into one dense text section.
//!
//! [`place_fragment_text_section`] checks relocation eligibility (`source`),
//! runs the relocation-free or internal-call placement (`production`), and
//! returns only after the independent checker (`validation`) accepts the
//! section. Source and frame admission belong to the text-placement stage
//! above; neither entrance grants publication authority.

mod conversion;
mod error;
mod production;
mod source;
mod statistics;
mod validation;

pub use error::TextPlacementError;
use machine_code::{FunctionFragmentEmissionPlan, RelocationFreeTextSectionPlacement};
pub use statistics::text_section_statistics;

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
