//! Independent reconstruction of the structural-Unit selection contract.

mod constraints;
mod layout;
mod shape;

use crate::selection::shared::*;

pub(super) struct ReconstructedStructuralUnitContract<'catalog> {
    pub(super) layout: SelectedMicrosoftX64OwnedIndirectPairLayout,
    function: usize,
    keys: SelectedConstraintKeys,
    catalog: &'catalog ValidatedRegisterConstraintCatalog,
}

impl<'catalog> ReconstructedStructuralUnitContract<'catalog> {
    pub(super) fn reconstruct_call_row(
        &self,
    ) -> Result<&'catalog RegisterInstructionConstraint, SelectedInstructionError> {
        constraints::reconstruct_structural_call_row(self.function, self.keys, self.catalog)
    }
}

pub(super) fn reconstruct_structural_unit_contract<'catalog>(
    function: usize,
    source: &SourceStructuralUnitFunction,
    keys: SelectedConstraintKeys,
    catalog: &'catalog ValidatedRegisterConstraintCatalog,
) -> Result<ReconstructedStructuralUnitContract<'catalog>, SelectedInstructionError> {
    let layout = layout::reconstruct_structural_unit_layout(function, source)?;
    Ok(ReconstructedStructuralUnitContract {
        layout,
        function,
        keys,
        catalog,
    })
}

pub(super) use layout::reconstruct_structural_unit_layout;
