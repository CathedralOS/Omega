//! The current allocator realizes live successor arguments by checked
//! same-home assignment. This records the relation; it does not assume it.

use crate::{EdgeRegisterTransfer, FunctionLiveness, LiveRangeError};
use selected_instructions::{SelectedFunction, SelectedTerminator, VirtualRegisterOrigin};

pub(super) fn derive(
    function_index: usize,
    selected: &SelectedFunction,
    live: &FunctionLiveness,
) -> Result<Vec<EdgeRegisterTransfer>, LiveRangeError> {
    let mut rows = Vec::new();
    for block in &selected.blocks {
        let successors = match &block.terminator {
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::Return { .. } => Vec::new(),
        };
        for edge in successors {
            let target = live
                .blocks
                .iter()
                .find(|target| target.block == edge.block)
                .ok_or(LiveRangeError::FunctionMismatch {
                    function: function_index,
                })?;
            for parameter in selected.virtual_registers.iter().filter(|register| {
                matches!(register.origin, VirtualRegisterOrigin::BlockParameter { block, .. } if block == edge.block)
                    && target.virtual_live_in.contains(&register.id)
            }) {
                let argument = crate::analyses::liveness::edge_values::incoming_argument(function_index, selected, edge, parameter.id)
                    .map_err(LiveRangeError::LivenessRevalidation)?;
                rows.push(EdgeRegisterTransfer { source: block.id, target: edge.block, psi_edge: edge.psi_edge,
                    argument, parameter: parameter.id, class: parameter.class });
            }
        }
    }
    rows.sort();
    Ok(rows)
}
