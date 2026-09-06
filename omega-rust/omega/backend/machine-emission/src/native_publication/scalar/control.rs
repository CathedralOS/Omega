//! Bind current semantic edges to the physical graph without changing topology.

use abstract_operations::{AbstractFunction, AbstractOperation};
use machine_code::{
    FunctionFragment, FunctionFragmentBranchEvidence as Branch,
    FunctionFragmentControlProvenance as Control, FunctionFragmentSuccessorProvenance,
    ScalarControlBlockEvidence, ScalarControlFlowEvidence,
    ScalarControlTerminatorEvidence as Terminator, ScalarDirectConditionalBranchEvidence,
};
use selected_instructions::SelectedBlockId;
use semantic_vocabulary::BlockId;

pub(super) fn project(
    fragment: &FunctionFragment,
    source: &AbstractFunction,
) -> Result<ScalarControlFlowEvidence, &'static str> {
    if fragment.blocks.len() != source.block_entries.len()
        || fragment.blocks.is_empty()
        || usize::try_from(fragment.byte_count).ok() != Some(fragment.bytes.len())
    {
        return Err("scalar physical blocks differ from the current semantic graph");
    }
    let mut ordered = fragment.blocks.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|block| block.offset);
    let mut mappings = vec![(ordered[0].block, source.entry)];
    // Bindings remain semantic facts even when independently checked equal
    // register homes realize a transfer without a physical copy instruction.
    for block in &ordered {
        let last = block
            .instructions
            .last()
            .ok_or("scalar block has no terminator")?;
        for successor in successors(&last.control) {
            if !fragment
                .blocks
                .iter()
                .any(|block| block.block == successor.block)
                || !source
                    .block_entries
                    .iter()
                    .any(|block| block.block == successor.source_target)
            {
                return Err("scalar successor names a missing block");
            }
            match mappings.iter().find(|(block, _)| *block == successor.block) {
                Some((_, target)) if *target != successor.source_target => {
                    return Err("scalar block has conflicting semantic targets");
                }
                Some(_) => {}
                None => mappings.push((successor.block, successor.source_target)),
            }
        }
    }
    if mappings.len() != ordered.len()
        || mappings
            .iter()
            .enumerate()
            .any(|(index, (_, source))| mappings[..index].iter().any(|(_, prior)| prior == source))
    {
        return Err("scalar block map is not one-to-one");
    }
    let mut blocks = Vec::new();
    let mut edges = Vec::new();
    let mut end = 0;
    for block in ordered {
        let (last, body) = block
            .instructions
            .split_last()
            .ok_or("scalar block has no terminator")?;
        if block.offset != end
            || body.iter().any(|instruction| {
                instruction.control != Control::None || instruction.branch.is_some()
            })
            || block
                .instructions
                .iter()
                .any(|instruction| instruction.internal_machine_fixup.is_some())
            || last.offset.checked_add(last.bytes.len() as u64)
                != block.offset.checked_add(block.byte_count)
        {
            return Err("scalar blocks must partition a no-call physical graph");
        }
        end = block
            .offset
            .checked_add(block.byte_count)
            .ok_or("scalar block overflows")?;
        let source_block = mappings
            .iter()
            .find(|(current, _)| *current == block.block)
            .ok_or("unmapped scalar block")?
            .1;
        let source_terminator = source_terminator(source, source_block)?;
        let offset = usize::try_from(last.offset).map_err(|_| "scalar offset exceeds host size")?;
        let byte_count = last.bytes.len();
        let terminator = match (&last.control, source_terminator, last.branch.as_deref()) {
            (
                Control::Return { psi_return_edge },
                AbstractOperation::Return {
                    psi_edge,
                    cleanup_actions,
                    ..
                },
                None,
            ) if psi_return_edge == psi_edge && cleanup_actions.is_empty() => {
                edges.push(*psi_edge);
                Terminator::Return { offset, byte_count }
            }
            (
                Control::Jump { successor },
                AbstractOperation::Jump {
                    psi_edge,
                    target,
                    bindings,
                    trivial_affine_discards,
                },
                Some(Branch::Jump(branch)),
            ) if successor.psi_edge == *psi_edge
                && successor.source_target == *target
                && successor.bindings == *bindings
                && trivial_affine_discards.is_empty() =>
            {
                let target_offset = physical_target(fragment, successor.block)?;
                if branch.source_block != block.block
                    || branch.target_edge != successor.psi_edge
                    || branch.target_block != successor.block
                    || branch.target_offset != target_offset
                {
                    return Err("scalar jump evidence differs from its successor");
                }
                edges.push(*psi_edge);
                Terminator::Jump {
                    offset,
                    byte_count,
                    target_offset: usize::try_from(target_offset)
                        .map_err(|_| "scalar target exceeds host size")?,
                }
            }
            (
                Control::ConditionalBranch {
                    predicate,
                    when_taken,
                    when_fallthrough,
                },
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                },
                Some(Branch::Conditional(branch)),
            ) => {
                let actual = [when_taken, when_fallthrough];
                let expected = [when_true, when_false];
                if when_taken.psi_edge == when_fallthrough.psi_edge
                    || actual.iter().any(|successor| {
                        !expected.iter().any(|expected| {
                            expected.psi_edge == successor.psi_edge
                                && expected.target == successor.source_target
                                && expected.bindings == successor.bindings
                                && expected.trivial_affine_discards.is_empty()
                        })
                    })
                {
                    return Err("scalar branch successors differ from the current semantic graph");
                }
                let taken_offset = physical_target(fragment, when_taken.block)?;
                let fallthrough_offset = physical_target(fragment, when_fallthrough.block)?;
                if branch.source_block != block.block
                    || branch.predicate != *predicate
                    || branch.when_taken_edge != when_taken.psi_edge
                    || branch.when_taken_block != when_taken.block
                    || branch.when_taken_offset != taken_offset
                    || branch.when_fallthrough_edge != when_fallthrough.psi_edge
                    || branch.when_fallthrough_block != when_fallthrough.block
                    || branch.when_fallthrough_offset != fallthrough_offset
                    || last.offset.checked_add(byte_count as u64) != Some(fallthrough_offset)
                {
                    return Err("scalar branch evidence differs from its physical successors");
                }
                edges.extend([when_taken.psi_edge, when_fallthrough.psi_edge]);
                Terminator::Conditional(ScalarDirectConditionalBranchEvidence {
                    predicate: *predicate,
                    branch_offset: offset,
                    branch_byte_count: byte_count,
                    taken_offset: usize::try_from(taken_offset)
                        .map_err(|_| "scalar target exceeds host size")?,
                    fallthrough_offset: usize::try_from(fallthrough_offset)
                        .map_err(|_| "scalar target exceeds host size")?,
                })
            }
            _ => return Err("scalar physical terminator differs from its semantic block"),
        };
        blocks.push(ScalarControlBlockEvidence {
            offset: usize::try_from(block.offset).map_err(|_| "scalar block exceeds host size")?,
            byte_count: usize::try_from(block.byte_count)
                .map_err(|_| "scalar block exceeds host size")?,
            terminator,
        });
    }
    if end != fragment.byte_count
        || edges.len() != fragment.provenance.edges.len()
        || edges.iter().enumerate().any(|(index, edge)| {
            edges[..index].contains(edge) || !fragment.provenance.edges.contains(edge)
        })
    {
        return Err("scalar graph loses semantic edge custody");
    }
    if blocks.len() == 1 && matches!(blocks[0].terminator, Terminator::Return { .. }) {
        Ok(ScalarControlFlowEvidence::Linear)
    } else {
        Ok(ScalarControlFlowEvidence::Acyclic { blocks })
    }
}

fn successors(control: &Control) -> Vec<&FunctionFragmentSuccessorProvenance> {
    match control {
        Control::Jump { successor } => vec![successor],
        Control::ConditionalBranch {
            when_taken,
            when_fallthrough,
            ..
        } => vec![when_taken, when_fallthrough],
        _ => Vec::new(),
    }
}

fn physical_target(
    fragment: &FunctionFragment,
    block: SelectedBlockId,
) -> Result<u64, &'static str> {
    fragment
        .blocks
        .iter()
        .find(|candidate| candidate.block == block)
        .map(|block| block.offset)
        .ok_or("scalar target block is missing")
}

fn source_terminator(
    source: &AbstractFunction,
    block: BlockId,
) -> Result<&AbstractOperation, &'static str> {
    let index = source
        .block_entries
        .iter()
        .position(|entry| entry.block == block)
        .ok_or("scalar source block is missing")?;
    let start = source.block_entries[index].operation_offset;
    let end = source
        .block_entries
        .get(index + 1)
        .map_or(source.operations.len(), |entry| entry.operation_offset);
    source
        .operations
        .get(start..end)
        .and_then(|operations| operations.last())
        .ok_or("scalar source block has no terminator")
}
