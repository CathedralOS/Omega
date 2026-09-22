//! Shared admission for the in-block commuting member-against-run
//! interchange: locate the named member and the two named members bounding
//! one contiguous run inside one block's body — the member strictly on one
//! side of the run's span — and prove the window they bound independent:
//! no register or condition-state hazard between any member and any
//! instruction outside its own side inside the window, every roster row
//! that newly trades order commuting with every row of the position it
//! crosses, no call, hosted-effect, or terminator barrier anywhere in the
//! window, and no boundary settlement inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::CommutingMemberRunInterchangeError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::unexecuted::commuting_accesses as accesses;
use crate::rewrites::unexecuted::place_storage::structural_place_declarations;
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
) -> Result<Admission, CommutingMemberRunInterchangeError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CommutingMemberRunInterchangeError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CommutingMemberRunInterchangeError::SourceMismatch)?;
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
        .ok_or(CommutingMemberRunInterchangeError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span its two named members bound in this
    // block, in the named order; a run of one member is the commuting
    // pair's granularity and a repeated or absent id names no run.
    let run_first_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == run_first)
        .ok_or(CommutingMemberRunInterchangeError::UnsupportedPair)?;
    let run_last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == run_last)
        .filter(|position| *position > run_first_index)
        .ok_or(CommutingMemberRunInterchangeError::UnsupportedPair)?;
    // The member sits strictly on one side of the run's span: inside the
    // span it would be part of the side it trades against, and a member
    // equal to either named bound names no disjoint side.
    if (run_first_index..=run_last_index).contains(&member_index) {
        return Err(CommutingMemberRunInterchangeError::UnsupportedPair);
    }
    let (window_first, window_last) = (
        member_index.min(run_first_index),
        member_index.max(run_last_index),
    );
    let window = &block.instructions[window_first..=window_last];
    // The window holds earlier-side ++ interior ++ later-side, where the
    // earlier side is the member when it precedes the run and the run when
    // it follows; the positions between the sides form the interior,
    // possibly empty, which keeps its relative order while the two sides
    // trade places.
    let run_len = run_last_index - run_first_index + 1;
    let member_earlier = member_index < run_first_index;
    let earlier_len = if member_earlier { 1 } else { run_len };
    let later_len = if member_earlier { run_len } else { 1 };
    let interior_len = window.len() - earlier_len - later_len;
    let structural_places = structural_place_declarations(function);
    // Every window position meets the same schedulable bar the
    // member-against-run interchange enforces: no barrier kind, no call
    // contract, and no unaccounted memory reach. What changes here is only
    // the accounting rule — a roster-carrying member may trade order with
    // a crossed position whose own rows commute with it.
    for position in window {
        schedulable(function, position)
            .ok_or(CommutingMemberRunInterchangeError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // Every member of each side trades order with every window position
    // outside its own side — the earlier side with the interior and the
    // later side, the later side with the earlier side and the interior —
    // so each direction of every register and condition-state hazard
    // applies against each pair, and every row the member carries must
    // commute with every row the crossed position carries. The run's
    // members keep their relative order and never face this audit against
    // each other, and interior positions keep their relative order with
    // each other. A window in which no trading pair is rowed on both
    // sides is the member-against-run interchange's own accounting case
    // and stays with it.
    let mut rowed_trade = false;
    for (member_window_index, member_instruction) in window.iter().enumerate() {
        let crossed_range = if member_window_index < earlier_len {
            // An earlier-side member crosses the interior and the later
            // side.
            earlier_len..window.len()
        } else if member_window_index < earlier_len + interior_len {
            // An interior position crosses nothing alone: its trades are
            // already audited as the crossed side of both sides' members.
            continue;
        } else {
            // A later-side member crosses the earlier side and the
            // interior.
            0..earlier_len + interior_len
        };
        for crossed_index in crossed_range {
            let crossed = &window[crossed_index];
            if coupled(member_instruction, crossed) {
                return Err(CommutingMemberRunInterchangeError::UnsupportedPair);
            }
            rowed_trade |= !rows[member_window_index].is_empty() && !rows[crossed_index].is_empty();
            for member_row in &rows[member_window_index] {
                for crossed_row in &rows[crossed_index] {
                    if !accesses::commutes(member_row, crossed_row, structural_places) {
                        return Err(CommutingMemberRunInterchangeError::UnsupportedPair);
                    }
                }
            }
        }
    }
    if !rowed_trade {
        return Err(CommutingMemberRunInterchangeError::UnsupportedPair);
    }
    if interior_settlement(function, block.id, window_first + 1..=window_last) {
        return Err(CommutingMemberRunInterchangeError::UnsupportedPair);
    }
    // The search scans the plan's body and terminator instructions once;
    // the window audit walks every member-against-crossed operand, unit,
    // and roster-row surface plus the function's three rosters.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            window
                .iter()
                .enumerate()
                .try_fold(total, |total, (member_window_index, member)| {
                    let mut crossed_range = if member_window_index < earlier_len {
                        earlier_len..window.len()
                    } else if member_window_index < earlier_len + interior_len {
                        return Some(total);
                    } else {
                        0..earlier_len + interior_len
                    };
                    crossed_range.try_fold(total, |total, crossed_index| {
                        total
                            .checked_add(surface(member))?
                            .checked_add(surface(&window[crossed_index]))?
                            .checked_add(
                                rows[member_window_index]
                                    .len()
                                    .checked_mul(rows[crossed_index].len())?,
                            )
                    })
                })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(CommutingMemberRunInterchangeError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CommutingMemberRunInterchangeError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CommutingMemberRunInterchangeError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        member_index,
        run_first_index,
        run_last_index,
    })
}
