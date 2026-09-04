//! Optimizer module role: executable entrance. Independent structural selection replay.

mod source;
mod target;

use crate::selection::shared::*;

pub(super) fn validate(
    source: &LegalizedProjectedStructuralCallReturn,
    legalized_plan: LegalizedOperationPlanIdentity,
    selected: &SelectedProjectedStructuralCallReturn,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if selected.recipe != SelectedProjectedStructuralCallReturnRecipe::OwnedLinearIntegerFragmentV1
        || selected.legalized_plan != legalized_plan
        || selected.caller != source.caller.machine
        || selected.callee != source.callee.machine
    {
        return Err(SelectedInstructionError::ProjectedStructuralCustodyMismatch);
    }
    let (roster, fragments) = source::replay(source)?;
    if selected.projected_qualifications != roster || selected.fragments != fragments {
        return Err(SelectedInstructionError::ProjectedStructuralCustodyMismatch);
    }
    target::replay(selected, constraints, physical, catalog)
}
