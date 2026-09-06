//! Canonical ordering and maximality checks for retained replay rows.

use crate::{FunctionLiveRanges, LiveRangeEdgeConnector, LiveRangeError, LiveRangeFragment};

pub(super) fn validate(function: usize, actual: &FunctionLiveRanges) -> Result<(), LiveRangeError> {
    if actual
        .block_domains
        .windows(2)
        .any(|rows| rows[0].block >= rows[1].block)
        || actual
            .virtual_registers
            .windows(2)
            .any(|rows| rows[0].virtual_register >= rows[1].virtual_register)
        || actual.tied_pairs.windows(2).any(|rows| rows[0] >= rows[1])
        || actual
            .edge_transfers
            .windows(2)
            .any(|rows| rows[0] >= rows[1])
        || actual
            .early_clobbers
            .windows(2)
            .any(|rows| rows[0] >= rows[1])
        || actual
            .early_clobbers
            .iter()
            .any(|row| row.uses.is_empty() || row.uses.windows(2).any(|uses| uses[0] >= uses[1]))
        || actual
            .architectural_units
            .windows(2)
            .any(|rows| rows[0].unit >= rows[1].unit)
        || actual
            .interference
            .windows(2)
            .any(|rows| rows[0] >= rows[1])
        || actual
            .interference
            .iter()
            .any(|pair| pair.lower >= pair.higher)
    {
        return Err(LiveRangeError::NonCanonicalRows { function });
    }
    for range in &actual.virtual_registers {
        require_maximal_fragments(function, &range.fragments)?;
        require_ordered_connectors(function, &range.edge_connectors)?;
    }
    for range in &actual.architectural_units {
        require_maximal_fragments(function, &range.fragments)?;
        require_ordered_connectors(function, &range.edge_connectors)?;
    }
    Ok(())
}

fn require_maximal_fragments(
    function: usize,
    fragments: &[LiveRangeFragment],
) -> Result<(), LiveRangeError> {
    if fragments.iter().any(|row| row.start >= row.end)
        || fragments.windows(2).any(|rows| {
            rows[0].block > rows[1].block
                || (rows[0].block == rows[1].block && rows[0].end >= rows[1].start)
        })
    {
        return Err(LiveRangeError::NonCanonicalRows { function });
    }
    Ok(())
}

fn require_ordered_connectors(
    function: usize,
    connectors: &[LiveRangeEdgeConnector],
) -> Result<(), LiveRangeError> {
    if connectors.windows(2).any(|rows| {
        (rows[0].source, rows[0].polarity_ordinal) >= (rows[1].source, rows[1].polarity_ordinal)
    }) {
        return Err(LiveRangeError::NonCanonicalRows { function });
    }
    Ok(())
}
