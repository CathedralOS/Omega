//! Exact copy/no-copy geometry between structural ABI sites.

use crate::selection::shared::*;

use super::constraints::{copy_constraint, fragment_register};

pub(super) fn project(
    fragments: &[SelectedStructuralFragmentConstraint],
    source_site: SelectedStructuralFragmentSite,
    destination_site: SelectedStructuralFragmentSite,
    copy_key: RegisterConstraintKey,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedStructuralTransfer, SelectedInstructionError> {
    let source = fragment_register(fragments, source_site)?;
    let destination = fragment_register(fragments, destination_site)?;
    if source == destination {
        return Ok(SelectedStructuralTransfer::SameViewNoCopy { register: source });
    }
    Ok(SelectedStructuralTransfer::FixedViewCopy {
        source,
        destination,
        constraint: copy_constraint(copy_key, source, destination, physical, catalog)?,
    })
}
