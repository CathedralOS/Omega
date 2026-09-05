//! Optimizer module role: executable entrance. Current fragments to dense text placement.
//!
//! Source and frame admission belong to the caller. These entrances transform
//! raw data and check its exact placement; neither grants publication authority.
mod error;
mod production;
mod source;
mod validation;

pub use error::TextPlacementError;
use omega_machine_code::{
    FunctionFragmentEmissionPlan, RelocationFreeTextSectionPlacement, ResolvedMachineProgram,
    WholeFunctionExitContract,
};
use omega_post_allocation_machine_to_selected_form_encoding::SelectedStructuralUnitFunctionEncoding;
use omega_register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog};

#[derive(Clone, Copy)]
pub struct StructuralFragmentPlacementInputs<'a> {
    pub program: &'a ResolvedMachineProgram,
    pub structural_encoding: &'a [SelectedStructuralUnitFunctionEncoding],
    pub exit: &'a WholeFunctionExitContract,
    pub physical: &'a ValidatedPhysicalRegisterModel,
    pub constraints: &'a ValidatedRegisterConstraintCatalog,
}

#[derive(Clone, Copy)]
pub enum TextPlacementInput<'a> {
    RelocationFree(&'a FunctionFragmentEmissionPlan),
    InternalCalls(&'a FunctionFragmentEmissionPlan),
    Structural {
        fragments: &'a FunctionFragmentEmissionPlan,
        facts: StructuralFragmentPlacementInputs<'a>,
    },
}
impl<'a> TextPlacementInput<'a> {
    fn fragments(self) -> &'a FunctionFragmentEmissionPlan {
        match self {
            Self::RelocationFree(fragments)
            | Self::InternalCalls(fragments)
            | Self::Structural { fragments, .. } => fragments,
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
        TextPlacementInput::Structural { fragments, facts } => {
            production::structural_unit(fragments, &facts)?
        }
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
