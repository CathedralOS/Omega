//! Exact machine join between legal and selected functions.

use super::scalar_graph;
use crate::selection::shared::*;

pub(super) fn validate(
    target: &LegalizedOperationPlan,
    functions: &[SelectedFunction],
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let catalog = environment.constraints();
    for (function_index, selected) in functions.iter().enumerate() {
        let graph = target
            .scalar_functions
            .iter()
            .filter(|source| source.machine == selected.machine)
            .collect::<Vec<_>>();
        match graph.as_slice() {
            [source] => {
                let projected =
                    super::super::edge_transfers::project(function_index, selected, constraints)?;
                scalar_graph::validate_with_environment(
                    function_index,
                    source,
                    &projected,
                    constraints,
                    environment,
                )?;
                for block in &selected.blocks {
                    super::integrity::validate_block_constraints(
                        function_index,
                        block,
                        selected,
                        catalog,
                    )?;
                }
                super::integrity::validate_def_use(function_index, selected, catalog)?;
            }
            _ => return Err(SelectedInstructionError::SourceCustodyMismatch),
        }
    }
    Ok(())
}
