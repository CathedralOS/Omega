//! Exact semantic sites and spans; ordinal lookup is an input predicate.

#[cfg(test)]
mod tests;

use super::{Error, host};
use abstract_operations::{AbstractFunction, AbstractOperation};
use machine_code::{
    FunctionFragment, FunctionFragmentBranchEvidence, FunctionFragmentControlProvenance as Control,
    SemanticCodeAttribution, SemanticCodeSite,
};

pub(super) fn ordinal(source: &AbstractFunction, site: SemanticCodeSite) -> Result<usize, Error> {
    let mut matches = source
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            let matches = match operation {
                AbstractOperation::IntegerConstant { psi_operation, .. }
                | AbstractOperation::IntegerEqual { psi_operation, .. }
                | AbstractOperation::IntegerLessThan { psi_operation, .. }
                | AbstractOperation::IntegerLessOrEqual { psi_operation, .. }
                | AbstractOperation::BooleanNot { psi_operation, .. }
                | AbstractOperation::IntegerWiden { psi_operation, .. }
                | AbstractOperation::ExactIntegerAdd { psi_operation, .. }
                | AbstractOperation::ExactIntegerSubtract { psi_operation, .. }
                | AbstractOperation::Call { psi_operation, .. } => {
                    site == SemanticCodeSite::Operation(*psi_operation)
                }
                AbstractOperation::Return { psi_edge, .. }
                | AbstractOperation::ReturnUnit { psi_edge, .. }
                | AbstractOperation::Jump { psi_edge, .. } => {
                    site == SemanticCodeSite::Edge(*psi_edge)
                }
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    site == SemanticCodeSite::Edge(when_true.psi_edge)
                        || site == SemanticCodeSite::Edge(when_false.psi_edge)
                }
                _ => false,
            };
            matches.then_some(index)
        });
    match (matches.next(), matches.next()) {
        (Some(index), None) => Ok(index),
        _ => Err(Error::Mismatch(
            "fragment semantic site has no unique authored ordinal",
        )),
    }
}

pub(super) fn produce(
    fragment: &FunctionFragment,
    source: &AbstractFunction,
) -> Result<Vec<SemanticCodeAttribution>, Error> {
    let mut rows = Vec::new();
    for instruction in fragment.blocks.iter().flat_map(|block| &block.instructions) {
        let offset = host(instruction.offset)?;
        let length = instruction.bytes.len();
        let mut push = |site, code_offset, byte_count| -> Result<(), Error> {
            rows.push(SemanticCodeAttribution {
                site,
                operation_ordinal: ordinal(source, site)?,
                code_offset,
                byte_count,
            });
            Ok(())
        };
        for operation in &instruction.provenance.operations {
            push(SemanticCodeSite::Operation(*operation), offset, length)?;
        }
        match &instruction.control {
            Control::Return { psi_return_edge } => {
                push(SemanticCodeSite::Edge(*psi_return_edge), offset, length)?
            }
            Control::Jump { successor } => {
                push(SemanticCodeSite::Edge(successor.psi_edge), offset, length)?
            }
            Control::ConditionalBranch {
                when_taken,
                when_fallthrough,
                ..
            } => {
                push(SemanticCodeSite::Edge(when_taken.psi_edge), offset, length)?;
                let Some(branch) = instruction.branch.as_deref() else {
                    return Err(Error::Mismatch("conditional has no decoded branch"));
                };
                let FunctionFragmentBranchEvidence::Conditional(branch) = branch else {
                    return Err(Error::Mismatch("conditional has jump evidence"));
                };
                push(
                    SemanticCodeSite::Edge(when_fallthrough.psi_edge),
                    host(branch.when_fallthrough_offset)?,
                    0,
                )?;
            }
            Control::DirectInternalCall { .. } | Control::None => {}
        }
    }
    rows.sort_by_key(|row| (row.operation_ordinal, row.code_offset, row.byte_count));
    rows.dedup();
    let mut joined: Vec<SemanticCodeAttribution> = Vec::new();
    for row in rows {
        if let Some(previous) = joined.iter_mut().find(|previous| previous.site == row.site) {
            let end = previous
                .code_offset
                .checked_add(previous.byte_count)
                .ok_or(Error::Overflow)?;
            if row.code_offset != end {
                return Err(Error::Unsupported(
                    "one semantic site has disjoint physical intervals",
                ));
            }
            previous.byte_count = previous
                .byte_count
                .checked_add(row.byte_count)
                .ok_or(Error::Overflow)?;
        } else {
            joined.push(row);
        }
    }
    Ok(joined)
}

pub(super) fn validate(
    fragment: &FunctionFragment,
    source: &AbstractFunction,
    rows: &[SemanticCodeAttribution],
) -> Result<(), Error> {
    // Replay each maximal contiguous site interval directly from physical
    // membership and extent. No producer merge algorithm is invoked.
    for (index, row) in rows.iter().enumerate() {
        if ordinal(source, row.site)? != row.operation_ordinal
            || rows[..index]
                .iter()
                .any(|previous| previous.site == row.site)
            || !interval_is_exact(fragment, row)?
        {
            return Err(Error::Mismatch(
                "object semantic interval differs from its physical spans",
            ));
        }
    }
    if rows.windows(2).any(|pair| {
        (pair[0].operation_ordinal, pair[0].code_offset)
            >= (pair[1].operation_ordinal, pair[1].code_offset)
    }) {
        return Err(Error::Mismatch(
            "object semantic attribution order is not canonical",
        ));
    }
    for instruction in fragment.blocks.iter().flat_map(|block| &block.instructions) {
        let present = |site, offset, length| {
            rows.iter().any(|row| {
                row.site == site
                    && row.code_offset <= offset
                    && offset.checked_add(length).is_some_and(|end| {
                        row.code_offset
                            .checked_add(row.byte_count)
                            .is_some_and(|row_end| end <= row_end)
                    })
            })
        };
        for operation in &instruction.provenance.operations {
            if !present(
                SemanticCodeSite::Operation(*operation),
                host(instruction.offset)?,
                instruction.bytes.len(),
            ) {
                return Err(Error::Mismatch("object omits an attributed operation"));
            }
        }
        let edges: Vec<_> = match &instruction.control {
            Control::Return { psi_return_edge } => vec![*psi_return_edge],
            Control::Jump { successor } => vec![successor.psi_edge],
            Control::ConditionalBranch {
                when_taken,
                when_fallthrough,
                ..
            } => vec![when_taken.psi_edge, when_fallthrough.psi_edge],
            Control::DirectInternalCall { .. } | Control::None => Vec::new(),
        };
        for edge in edges {
            if !rows.iter().any(|row| {
                row.site == SemanticCodeSite::Edge(edge)
                    && supports(instruction, row.site, row.code_offset, row.byte_count)
            }) {
                return Err(Error::Mismatch("object omits an attributed control edge"));
            }
        }
    }
    Ok(())
}

fn supports(
    instruction: &machine_code::FunctionFragmentInstructionSpan,
    site: SemanticCodeSite,
    offset: usize,
    length: usize,
) -> bool {
    let ordinary =
        u64::try_from(offset).ok() == Some(instruction.offset) && length == instruction.bytes.len();
    match site {
        SemanticCodeSite::Operation(operation) => {
            ordinary && instruction.provenance.operations.contains(&operation)
        }
        SemanticCodeSite::Edge(edge) => match &instruction.control {
            Control::Return { psi_return_edge } => ordinary && edge == *psi_return_edge,
            Control::Jump { successor } => ordinary && edge == successor.psi_edge,
            Control::ConditionalBranch {
                when_taken,
                when_fallthrough,
                ..
            } => {
                (ordinary && edge == when_taken.psi_edge)
                    || (edge == when_fallthrough.psi_edge
                        && length == 0
                        && matches!(
                            instruction.branch.as_deref(),
                            Some(FunctionFragmentBranchEvidence::Conditional(branch)) if u64::try_from(offset).ok() == Some(branch.when_fallthrough_offset)
                        ))
            }
            Control::None | Control::DirectInternalCall { .. } => false,
        },
    }
}
fn interval_is_exact(
    fragment: &FunctionFragment,
    row: &SemanticCodeAttribution,
) -> Result<bool, Error> {
    let SemanticCodeSite::Operation(operation) = row.site else {
        return Ok(fragment
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| supports(instruction, row.site, row.code_offset, row.byte_count)));
    };
    let end = row
        .code_offset
        .checked_add(row.byte_count)
        .ok_or(Error::Overflow)?;
    let mut minimum = usize::MAX;
    let mut maximum = 0usize;
    let mut covered = 0usize;
    let mut found = false;
    for instruction in fragment.blocks.iter().flat_map(|block| &block.instructions) {
        let start = host(instruction.offset)?;
        let instruction_end = start
            .checked_add(instruction.bytes.len())
            .ok_or(Error::Overflow)?;
        if instruction.provenance.operations.contains(&operation) {
            found = true;
            minimum = minimum.min(start);
            maximum = maximum.max(instruction_end);
            covered = covered
                .checked_add(instruction.bytes.len())
                .ok_or(Error::Overflow)?;
        } else if start < end && instruction_end > row.code_offset {
            return Ok(false);
        } else if instruction.bytes.is_empty() && start > row.code_offset && start < end {
            // Even zero-width foreign control/provenance is not absorbed.
            return Ok(false);
        }
    }
    Ok(found && minimum == row.code_offset && maximum == end && covered == row.byte_count)
}
