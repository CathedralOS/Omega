//! Project actual taken/fallthrough topology without changing branch polarity.

use abstract_operations::{AbstractFunction, AbstractOperation};
use machine_code::{
    FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentControlProvenance as Control,
    FunctionFragmentInstructionSpan, ScalarControlFlowEvidence,
    ScalarDirectConditionalBranchEvidence,
};

pub(super) fn project(
    fragment: &FunctionFragment,
    source: &AbstractFunction,
) -> Result<ScalarControlFlowEvidence, &'static str> {
    for block in &fragment.blocks {
        let Some((last, body)) = block.instructions.split_last() else {
            return Err("scalar block has no terminator");
        };
        if body
            .iter()
            .any(|instruction| instruction.control != Control::None || instruction.branch.is_some())
            || block
                .instructions
                .iter()
                .any(|instruction| instruction.internal_machine_fixup.is_some())
            || last.offset.checked_add(last.bytes.len() as u64)
                != block.offset.checked_add(block.byte_count)
        {
            return Err("scalar native publication requires no-call terminating blocks");
        }
    }
    if let [block] = fragment.blocks.as_slice() {
        let returned = return_instruction(block)?;
        let Control::Return { psi_return_edge } = returned.control else {
            unreachable!("return checked");
        };
        if source.block_entries.len() != 1 || fragment.provenance.edges != [psi_return_edge] {
            return Err("linear scalar fragment has a different semantic control shape");
        }
        return Ok(ScalarControlFlowEvidence::Linear);
    }
    if fragment.blocks.len() != 3 || source.block_entries.len() != 3 {
        return Err(
            "scalar native publication requires a leaf or one conditional with two returns",
        );
    }
    conditional(fragment, source)
}

fn conditional(
    fragment: &FunctionFragment,
    source: &AbstractFunction,
) -> Result<ScalarControlFlowEvidence, &'static str> {
    let mut entries = fragment.blocks.iter().filter(|block| {
        matches!(
            block
                .instructions
                .last()
                .map(|instruction| &instruction.control),
            Some(Control::ConditionalBranch { .. })
        )
    });
    let entry = entries
        .next()
        .ok_or("scalar conditional has no branch block")?;
    if entries.next().is_some() {
        return Err("scalar publication admits only one conditional");
    }
    let instruction = entry
        .instructions
        .last()
        .ok_or("scalar entry has no branch")?;
    let Control::ConditionalBranch {
        predicate,
        when_taken,
        when_fallthrough,
    } = &instruction.control
    else {
        unreachable!("branch checked");
    };
    let branch = instruction
        .branch
        .as_ref()
        .ok_or("scalar branch lacks decoded target evidence")?;
    let taken = fragment
        .blocks
        .iter()
        .find(|block| block.block == when_taken.block)
        .ok_or("scalar branch target is missing")?;
    let fallthrough = fragment
        .blocks
        .iter()
        .find(|block| block.block == when_fallthrough.block)
        .ok_or("scalar fallthrough is missing")?;
    let taken_return = return_instruction(taken)?;
    let fallthrough_return = return_instruction(fallthrough)?;
    if taken.block == fallthrough.block
        || taken.block == entry.block
        || fallthrough.block == entry.block
        || branch.predicate != *predicate
        || branch.source_block != entry.block
        || branch.when_taken_edge != when_taken.psi_edge
        || branch.when_taken_block != when_taken.block
        || branch.when_taken_offset != taken.offset
        || branch.when_fallthrough_edge != when_fallthrough.psi_edge
        || branch.when_fallthrough_block != when_fallthrough.block
        || branch.when_fallthrough_offset != fallthrough.offset
        || !when_taken.bindings.is_empty()
        || !when_fallthrough.bindings.is_empty()
        || instruction
            .offset
            .checked_add(instruction.bytes.len() as u64)
            != Some(fallthrough.offset)
        || fallthrough.offset.checked_add(fallthrough.byte_count) != Some(taken.offset)
        || taken.offset.checked_add(taken.byte_count) != Some(fragment.byte_count)
        || usize::try_from(fragment.byte_count).ok() != Some(fragment.bytes.len())
    {
        return Err("scalar branch evidence differs from its physical successors");
    }
    let mut source_branches = source
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => Some([when_true, when_false]),
            _ => None,
        });
    let successors = source_branches
        .next()
        .ok_or("scalar source has no conditional")?;
    if source_branches.next().is_some()
        || [when_taken, when_fallthrough].iter().any(|target| {
            !successors.iter().any(|successor| {
                successor.psi_edge == target.psi_edge
                    && successor.target == target.source_target
                    && successor.bindings.is_empty()
                    && successor.trivial_affine_discards.is_empty()
            })
        })
    {
        return Err("scalar branch successors differ from the current abstract program");
    }
    let Control::Return {
        psi_return_edge: taken_edge,
    } = taken_return.control
    else {
        unreachable!("return checked");
    };
    let Control::Return {
        psi_return_edge: fallthrough_edge,
    } = fallthrough_return.control
    else {
        unreachable!("return checked");
    };
    let edges = [
        when_taken.psi_edge,
        when_fallthrough.psi_edge,
        taken_edge,
        fallthrough_edge,
    ];
    if fragment.provenance.edges.len() != edges.len()
        || edges.iter().enumerate().any(|(index, edge)| {
            edges[..index].contains(edge) || !fragment.provenance.edges.contains(edge)
        })
    {
        return Err("scalar conditional loses semantic edge custody");
    }
    Ok(ScalarControlFlowEvidence::DirectConditional {
        branch: ScalarDirectConditionalBranchEvidence {
            predicate: *predicate,
            branch_offset: usize::try_from(instruction.offset)
                .map_err(|_| "scalar branch offset exceeds host size")?,
            branch_byte_count: instruction.bytes.len(),
            taken_offset: usize::try_from(taken.offset)
                .map_err(|_| "scalar taken offset exceeds host size")?,
            fallthrough_offset: usize::try_from(fallthrough.offset)
                .map_err(|_| "scalar fallthrough offset exceeds host size")?,
        },
    })
}

fn return_instruction(
    block: &FunctionFragmentBlockSpan,
) -> Result<&FunctionFragmentInstructionSpan, &'static str> {
    let instruction = block
        .instructions
        .last()
        .ok_or("scalar return block is empty")?;
    if !matches!(instruction.control, Control::Return { .. }) || instruction.branch.is_some() {
        return Err("scalar branch arm must terminate in a direct return");
    }
    Ok(instruction)
}
