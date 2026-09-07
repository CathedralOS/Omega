//! Exact machine join between legal and selected functions.

use super::functions::validate_function;
use super::{scalar_graph, scalar_leaf};
use crate::selection::shared::*;

pub(super) fn validate(
    target: &LegalizedOperationPlan,
    functions: &[SelectedFunction],
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    for (function_index, selected) in functions.iter().enumerate() {
        let specialized = target
            .functions
            .iter()
            .filter(|source| source.machine() == selected.machine)
            .collect::<Vec<_>>();
        let graph = target
            .scalar_functions
            .iter()
            .filter(|source| source.machine == selected.machine)
            .collect::<Vec<_>>();
        match (specialized.as_slice(), graph.as_slice()) {
            ([legalized_operations::LegalizedFunction::SharedReturnConditional(source)], []) => {
                super::shared_return::validate(
                    function_index,
                    source,
                    selected,
                    constraints,
                    physical,
                    catalog,
                )?
            }
            ([legalized_operations::LegalizedFunction::Conditional(source)], []) => {
                validate_function(
                    function_index,
                    source,
                    selected,
                    constraints,
                    physical,
                    catalog,
                )?
            }
            ([legalized_operations::LegalizedFunction::Leaf(source)], []) => scalar_leaf::validate(
                function_index,
                source,
                selected,
                target.target,
                constraints,
                physical,
                catalog,
            )?,
            ([], [source]) => scalar_graph::validate(
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
