//! Exact source-roster joining and body-specific selected-function replay.

use super::functions::{validate_function, validate_unit_function};
use super::{scalar_call_unit, scalar_leaf};
use crate::selection::shared::*;

pub(super) fn validate(
    target: &LegalizedOperationPlan,
    functions: &[SelectedFunction],
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    for (function_index, selected) in functions.iter().enumerate() {
        let scalar = target
            .functions
            .iter()
            .filter(|source| source.machine() == selected.machine)
            .collect::<Vec<_>>();
        let unit = target
            .unit_functions
            .iter()
            .filter(|source| source.machine == selected.machine)
            .collect::<Vec<_>>();
        let scalar_call_unit = target
            .scalar_call_unit_functions
            .iter()
            .filter(|source| source.machine == selected.machine)
            .collect::<Vec<_>>();
        match (
            scalar.as_slice(),
            unit.as_slice(),
            scalar_call_unit.as_slice(),
        ) {
            (
                [legalized_operations::LegalizedFunction::SharedReturnConditional(source)],
                [],
                [],
            ) => super::shared_return::validate(
                function_index,
                source,
                selected,
                constraints,
                physical,
                catalog,
            )?,
            ([legalized_operations::LegalizedFunction::Conditional(source)], [], []) => {
                validate_function(
                    function_index,
                    source,
                    selected,
                    constraints,
                    physical,
                    catalog,
                )?
            }
            ([legalized_operations::LegalizedFunction::Leaf(source)], [], []) => {
                scalar_leaf::validate(
                    function_index,
                    source,
                    selected,
                    target.target,
                    constraints,
                    physical,
                    catalog,
                )?
            }
            ([], [source], []) => {
                validate_unit_function(function_index, source, selected, constraints.keys, catalog)?
            }
            ([], [], [source]) => {
                scalar_call_unit::validate(function_index, source, selected, constraints, catalog)?
            }
            _ => return Err(SelectedInstructionError::SourceCustodyMismatch),
        }
    }
    Ok(())
}
