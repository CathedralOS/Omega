//! Exact machine join between legal and selected functions.

use super::scalar_graph;
use crate::selection::shared::*;

pub(super) fn validate(
    target: &LegalizedOperationPlan,
    functions: &[SelectedFunction],
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    for (function_index, selected) in functions.iter().enumerate() {
        let graph = target
            .scalar_functions
            .iter()
            .filter(|source| source.machine == selected.machine)
            .collect::<Vec<_>>();
        match graph.as_slice() {
            [source] => scalar_graph::validate(
                function_index,
                source,
                selected,
                target.target,
                constraints,
                physical,
                catalog,
            )?,
            _ => return Err(SelectedInstructionError::SourceCustodyMismatch),
        }
    }
    Ok(())
}
