//! The current allocator realizes live successor arguments by checked
//! same-home assignment. This records the relation; it does not assume it.

use crate::LiveRangeError;
use selected_instructions::{EdgeRegisterTransfer, FunctionLiveness};
use selected_instructions::{SelectedFunction, terminator_successors};

pub(super) fn derive(
    function_index: usize,
    selected: &SelectedFunction,
    live: &FunctionLiveness,
) -> Result<Vec<EdgeRegisterTransfer>, LiveRangeError> {
    if !selected
        .blocks
        .iter()
        .any(|block| !terminator_successors(&block.terminator).is_empty())
    {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    let index = crate::analyses::liveness::edge_values::FunctionIndex::new(selected);
    for block in &selected.blocks {
        let successors = terminator_successors(&block.terminator);
        for edge in successors {
            let target = live
                .blocks
                .iter()
                .find(|target| target.block == edge.block)
                .ok_or(LiveRangeError::FunctionMismatch {
                    function: function_index,
                })?;
            for parameter in index
                .parameters_of(edge.block)
                .filter(|register| target.virtual_live_in.contains(&register.id))
            {
                let argument = crate::analyses::liveness::edge_values::incoming_argument(
                    function_index,
                    &index,
                    edge,
                    parameter.id,
                )
                .map_err(LiveRangeError::LivenessRevalidation)?;
                rows.push(EdgeRegisterTransfer {
                    source: block.id,
                    target: edge.block,
                    psi_edge: edge.psi_edge.ok_or(LiveRangeError::FunctionMismatch {
                        function: function_index,
                    })?,
                    argument,
                    parameter: parameter.id,
                    class: parameter.class,
                });
            }
        }
    }
    rows.sort();
    Ok(rows)
}
