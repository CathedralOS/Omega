//! A required adjacent transfer has no machine opcode and cannot perform a
//! parallel copy. For a linear assembly body its scalar block parameters are
//! names for the same SSA definitions on both sides of the local label.
use super::LoweringError;
use abstract_operations::{AbstractBlock, AbstractOperation};
use semantic_vocabulary::{BlockId, ValueId};
use std::collections::BTreeMap;
use terminal_psi::{AssemblyExitConstraint, TerminalMachine};

pub(super) fn physical_blocks(
    machine: &TerminalMachine,
    blocks: Vec<AbstractBlock>,
) -> Result<Vec<AbstractBlock>, LoweringError> {
    if !machine.blocks.iter().any(|block| {
        matches!(
            block.assembly_exit,
            Some(AssemblyExitConstraint::Adjacent(_))
        )
    }) {
        return Ok(blocks);
    }
    let refuse = |block: BlockId, reason| LoweringError::UnsupportedAssemblyAdjacency {
        machine: machine.id,
        block,
        reason,
    };
    // Refuse additional incoming paths until they can retain an independently
    // addressable label inside the physical block. A straight-line chain has
    // exactly one entry and each interior label has a unique predecessor.
    if machine.blocks.first().map(|block| block.id) != Some(machine.entry)
        || !machine.block_entry_claims.is_empty()
    {
        return Err(refuse(
            machine.entry,
            "local assembly labels need a single entry and no runtime custody setup",
        ));
    }
    let mut input = blocks.into_iter();
    let mut physical = input.next().expect("an adjacent edge has a source block");
    let mut operations = std::mem::take(&mut physical.operations);
    let mut aliases = BTreeMap::<ValueId, ValueId>::new();
    for (index, source) in machine.blocks.iter().enumerate() {
        let mut terminal = operations
            .pop()
            .ok_or_else(|| refuse(source.id, "missing local terminator"))?;
        if operations
            .iter()
            .any(|instruction| !matches!(instruction, AbstractOperation::IntegerConstant { .. }))
        {
            return Err(refuse(
                source.id,
                "an assembly instruction needs a checked physical placement in this range",
            ));
        }
        physical.operations.append(&mut operations);
        match (source.assembly_exit, &mut terminal) {
            (
                Some(AssemblyExitConstraint::Adjacent(next)),
                AbstractOperation::Jump {
                    target,
                    bindings,
                    structural_bindings,
                    cleanup_actions,
                    ..
                },
            ) if next == *target
                && machine
                    .blocks
                    .get(index + 1)
                    .is_some_and(|block| block.id == next)
                && structural_bindings.is_empty()
                && cleanup_actions.is_empty() =>
            {
                // Resolve incoming names simultaneously. No move will be
                // selected for an assembly label or any of these bindings.
                aliases = bindings
                    .iter()
                    .map(|binding| {
                        (
                            binding.parameter,
                            aliases
                                .get(&binding.argument)
                                .copied()
                                .unwrap_or(binding.argument),
                        )
                    })
                    .collect();
                operations = input
                    .next()
                    .expect("one lowered block for each source block")
                    .operations;
            }
            (
                Some(AssemblyExitConstraint::ScalarReturn(_)),
                AbstractOperation::Return {
                    value,
                    cleanup_actions,
                    ..
                },
            ) if index + 1 == machine.blocks.len() => {
                if !cleanup_actions.is_empty() {
                    return Err(refuse(
                        source.id,
                        "assembly completion requires runtime cleanup",
                    ));
                }
                *value = aliases.get(value).copied().unwrap_or(*value);
                physical.operations.push(terminal);
            }
            _ => {
                return Err(refuse(
                    source.id,
                    "required adjacent range must have one incoming path per label and a checked final return",
                ));
            }
        }
    }
    Ok(vec![physical])
}
