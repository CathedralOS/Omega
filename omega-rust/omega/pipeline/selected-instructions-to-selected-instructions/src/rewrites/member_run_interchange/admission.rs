//! Shared admission for the in-block member-against-run interchange:
//! locate the named member and the two named members bounding one
//! contiguous run inside one block's body — the member strictly on one
//! side of the run's span — and prove the window they bound independent:
//! no register or condition-state hazard between any member and any
//! instruction outside its own side inside the window, no roster-carrying
//! side trading order with a second memory-access actor, no call,
//! hosted-effect, or terminator barrier anywhere in the window, and no
//! boundary settlement inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::MemberRunInterchangeError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

pub(super) struct Admission {
    pub block_index: usize,
    /// The member's index in the block body. The member sits strictly on
    /// one side of the run's span: before `run_first_index` or after
    /// `run_last_index`.
    pub member_index: usize,
    /// The run's first member index in the block body; the run is the
    /// contiguous span `run_first_index..=run_last_index` of at least two
    /// members.
    pub run_first_index: usize,
    /// The run's last member index. The window the interchange crosses is
    /// `member_index..=run_last_index` when the member precedes the run or
    /// `run_first_index..=member_index` when it follows.
    pub run_last_index: usize,
}

pub(super) fn admit(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    run_first: SelectedInstructionId,
    run_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, MemberRunInterchangeError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(MemberRunInterchangeError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(MemberRunInterchangeError::SourceMismatch)?;
    let (block_index, member_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == member)
                .map(|member_index| (block_index, member_index))
        })
        .ok_or(MemberRunInterchangeError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span its two named members bound in this
    // block, in the named order; a run of one member is the pair
    // interchange's granularity and a repeated or absent id names no run.
    let run_first_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == run_first)
        .ok_or(MemberRunInterchangeError::UnsupportedPair)?;
    let run_last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == run_last)
        .filter(|position| *position > run_first_index)
        .ok_or(MemberRunInterchangeError::UnsupportedPair)?;
    // The member sits strictly on one side of the run's span: inside the
    // span it would be part of the side it trades against, and a member
    // equal to either named bound names no disjoint side.
    if (run_first_index..=run_last_index).contains(&member_index) {
        return Err(MemberRunInterchangeError::UnsupportedPair);
    }
    let member_instruction = &block.instructions[member_index];
    let run = &block.instructions[run_first_index..=run_last_index];
    // The positions between the member and the run form the interior,
    // possibly empty, which keeps its relative order while the two sides
    // trade places.
    let interior = if member_index < run_first_index {
        &block.instructions[member_index + 1..run_first_index]
    } else {
        &block.instructions[run_last_index + 1..member_index]
    };
    // Every side member meets the schedulable bar itself: no barrier kind,
    // no call contract, and no unaccounted memory reach. A side's memory
    // accounting is the union of its members' roster rows — roster-carrying
    // members of the run keep their relative order, so the run may carry
    // several rows itself, but a row-carrying side may only cross row-less
    // positions: a second accounted actor on the other side or in the
    // interior would reorder recorded accesses.
    let member_accounted = schedulable(function, member_instruction)
        .ok_or(MemberRunInterchangeError::UnsupportedInstruction)?;
    let mut run_accounted = false;
    for run_member in run {
        run_accounted |= schedulable(function, run_member)
            .ok_or(MemberRunInterchangeError::UnsupportedInstruction)?;
    }
    if member_accounted && run_accounted {
        return Err(MemberRunInterchangeError::UnsupportedPair);
    }
    let side_accounted = member_accounted || run_accounted;
    for crossed in interior {
        // Every interior position meets the same schedulable bar as the
        // side members, since both sides trade order with it. Its roster
        // rows may stay only while neither side carries any — the
        // interior's recorded accesses then keep their relative order
        // while two memory-inert sides trade places around them.
        let interior_accounted = schedulable(function, crossed)
            .ok_or(MemberRunInterchangeError::UnsupportedInstruction)?;
        if interior_accounted && side_accounted {
            return Err(MemberRunInterchangeError::UnsupportedPair);
        }
    }
    // Every side member trades order with every window position outside
    // its own side — the member with the whole run and the interior, each
    // run member with the member and the whole interior — so each
    // direction of every hazard applies against each pair. The run's
    // members keep their relative order and never face this audit against
    // each other.
    for crossed in interior.iter().chain(run.iter()) {
        if coupled(member_instruction, crossed) {
            return Err(MemberRunInterchangeError::UnsupportedPair);
        }
    }
    for run_member in run {
        for crossed in interior.iter().chain(std::iter::once(member_instruction)) {
            if coupled(run_member, crossed) {
                return Err(MemberRunInterchangeError::UnsupportedPair);
            }
        }
    }
    let (window_first, window_last) = (
        member_index.min(run_first_index),
        member_index.max(run_last_index),
    );
    if interior_settlement(function, block.id, window_first + 1..=window_last) {
        return Err(MemberRunInterchangeError::UnsupportedPair);
    }
    // The search scans the plan's body and terminator instructions once;
    // the window audit walks every member-against-crossed operand and unit
    // surface plus the function's three rosters.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            interior
                .iter()
                .chain(run.iter())
                .try_fold(total, |total, crossed| {
                    total
                        .checked_add(surface(member_instruction))?
                        .checked_add(surface(crossed))
                })
        })
        .and_then(|total| {
            run.iter().try_fold(total, |total, run_member| {
                interior
                    .iter()
                    .chain(std::iter::once(member_instruction))
                    .try_fold(total, |total, crossed| {
                        total
                            .checked_add(surface(run_member))?
                            .checked_add(surface(crossed))
                    })
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(MemberRunInterchangeError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| MemberRunInterchangeError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(MemberRunInterchangeError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        member_index,
        run_first_index,
        run_last_index,
    })
}
