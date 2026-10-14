//! Liveness of a fixed set of physical register units across one function.
//!
//! The post-allocation plan carries every instruction's complete unit uses,
//! definitions and clobbers, so liveness of a unit set needs no virtual
//! register facts: a unit is live after an instruction when a later
//! instruction on some path reads it before every unit of the set is
//! rewritten. This is the guard an optimizer form needs before it may
//! clobber units its baseline form leaves alone.

use physical_instructions::{PostAllocationMachineFunction, PostAllocationMachineInstruction};
use register_model::RegisterUnitId;

/// Per-block live-in facts for one unit set over one function.
pub(super) struct UnitLivenessTable {
    live_in: Vec<bool>,
}

/// What one block does to the unit set before control leaves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockEffect {
    /// Some instruction reads a unit of the set before the set is rewritten.
    Reads,
    /// Every unit of the set is rewritten before any is read.
    Kills,
    /// The block neither reads nor fully rewrites the set.
    Transparent,
}

impl UnitLivenessTable {
    /// Compute live-in facts for `units` by fixed point over `successors`,
    /// which lists successor block indexes per block index of `function`.
    pub(super) fn compute(
        function: &PostAllocationMachineFunction,
        successors: &[Vec<usize>],
        units: &[RegisterUnitId],
    ) -> Self {
        let effects = function
            .blocks
            .iter()
            .map(|block| block_effect(&block.instructions, units))
            .collect::<Vec<_>>();
        let mut live_in = vec![false; function.blocks.len()];
        // Facts only turn on, so the iteration is bounded by the block count.
        for _ in 0..=function.blocks.len() {
            let mut changed = false;
            for (block, effect) in effects.iter().enumerate() {
                let value = match effect {
                    BlockEffect::Reads => true,
                    BlockEffect::Kills => false,
                    BlockEffect::Transparent => successors.get(block).is_some_and(|successors| {
                        successors.iter().any(|successor| live_in[*successor])
                    }),
                };
                if value && !live_in[block] {
                    live_in[block] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Self { live_in }
    }

    /// Whether any unit of `units` is live after instruction `index` of
    /// `block`: read by a later instruction of the block before the set is
    /// rewritten, or live into a successor when the block leaves it alone.
    /// Returns the instructions scanned so the caller can charge its budget.
    pub(super) fn live_after(
        &self,
        function: &PostAllocationMachineFunction,
        successors: &[Vec<usize>],
        block: usize,
        index: usize,
        units: &[RegisterUnitId],
    ) -> (bool, u64) {
        let instructions = &function.blocks[block].instructions[index + 1..];
        let scanned = instructions.len() as u64;
        match block_effect(instructions, units) {
            BlockEffect::Reads => (true, scanned),
            BlockEffect::Kills => (false, scanned),
            BlockEffect::Transparent => (
                successors.get(block).is_some_and(|successors| {
                    successors
                        .iter()
                        .any(|successor| self.live_in.get(*successor).copied().unwrap_or(true))
                }),
                scanned,
            ),
        }
    }
}

fn block_effect(
    instructions: &[PostAllocationMachineInstruction],
    units: &[RegisterUnitId],
) -> BlockEffect {
    for instruction in instructions {
        if reads_any(instruction, units) {
            return BlockEffect::Reads;
        }
        if rewrites_every(instruction, units) {
            return BlockEffect::Kills;
        }
    }
    BlockEffect::Transparent
}

fn reads_any(instruction: &PostAllocationMachineInstruction, units: &[RegisterUnitId]) -> bool {
    units.iter().any(|unit| {
        instruction.unit_uses.contains(unit) || instruction.implicit_unit_uses.contains(unit)
    })
}

fn rewrites_every(
    instruction: &PostAllocationMachineInstruction,
    units: &[RegisterUnitId],
) -> bool {
    !units.is_empty()
        && units.iter().all(|unit| {
            instruction.unit_defs.contains(unit)
                || instruction.unit_clobbers.contains(unit)
                || instruction.implicit_unit_defs.contains(unit)
                || instruction.implicit_unit_clobbers.contains(unit)
        })
}
