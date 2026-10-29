use super::{super::AllocatedCalleeSavedRequirementError, state::DirectTraversal};

pub(super) fn derive(
    traversal: &mut DirectTraversal<'_>,
    selected: &selected_instructions::SelectedFunctions,
    homes: &[crate::FunctionRegisterHomes],
) -> Result<(), AllocatedCalleeSavedRequirementError> {
    if selected.len() != homes.len() {
        return Err(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
    }
    // Allocation replay already establishes the exact physical home roster;
    // dead semantic parameters remain selected without requiring a home.
    for (function, homes) in selected.iter().zip(homes) {
        traversal.walk_function(function, homes)?;
    }
    Ok(())
}
