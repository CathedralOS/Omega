//! Borrow a block's operation range without confusing storage and execution order.
use super::*;

pub(super) fn block_body(
    function: &AbstractFunction,
    block: semantic_vocabulary::BlockId,
) -> Result<(usize, &[AbstractOperation]), LoweringError> {
    let invalid = || LoweringError::ConditionalControlFlowRequiresBlockLowering(function.machine);
    let (start, end) = if function.block_entries.is_empty() {
        (0, function.operations.len())
    } else {
        let (block_index, entry) = function
            .block_entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.block == block)
            .ok_or_else(invalid)?;
        let end = function
            .block_entries
            .get(block_index + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        (entry.operation_offset, end)
    };
    let body = function
        .operations
        .get(start..end)
        .filter(|body| !body.is_empty())
        .ok_or_else(invalid)?;
    Ok((start, body))
}
