//! Ordinary and structural function reconstruction.

use std::collections::BTreeSet;

use crate::LiveRangeError;
use selected_instructions::{
    FunctionLiveRanges, VirtualFixedConstraint, VirtualFixedConstraintSite, VirtualInterference,
    VirtualLiveRange, VirtualOccurrence, VirtualRegisterId,
};

use super::{architectural_units, constraints, fragments};

pub(super) fn replay_function(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    live: &selected_instructions::FunctionLiveness,
) -> Result<FunctionLiveRanges, LiveRangeError> {
    constraints::reject_unsupported(function, live)?;
    let tied_pairs = constraints::derive_ties(function, live)?;
    let edge_transfers = replay_edge_transfers(function, selected, live)?;
    let copy_affinities = replay_copy_affinities(function, selected)?;
    let early_clobbers = constraints::derive_early_clobbers(function, live)?;
    let block_domains = fragments::block_domains(function, live)?;

    let virtual_registers = replay_virtual_ranges(function, selected, live)?;

    let architectural_units = architectural_units::replay_all(function, live)?;
    let mut interference = Vec::new();
    for left_index in 0..virtual_registers.len() {
        for right_index in (left_index + 1)..virtual_registers.len() {
            let left = &virtual_registers[left_index];
            let right = &virtual_registers[right_index];
            if fragments::overlaps(&left.fragments, &right.fragments) {
                interference.push(VirtualInterference {
                    lower: left.virtual_register,
                    higher: right.virtual_register,
                });
            }
        }
    }
    Ok(FunctionLiveRanges {
        machine: selected.machine,
        block_domains,
        virtual_registers,
        tied_pairs,
        edge_transfers,
        copy_affinities,
        early_clobbers,
        architectural_units,
        interference,
    })
}

/// Reconstruct from actual memberships, retaining selected roster order and
/// operand/edge order within each range. IDs can be sparse after rewrites, so
/// scratch storage follows roster positions, never the largest register ID.
/// Only rows occupied in this block need their point sets drained. All facts
/// come from the independently revalidated liveness input, not producer ranges.
fn replay_virtual_ranges(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    live: &selected_instructions::FunctionLiveness,
) -> Result<Vec<VirtualLiveRange>, LiveRangeError> {
    let mut roster = selected
        .virtual_registers
        .iter()
        .enumerate()
        .map(|(position, register)| (register.id, position))
        .collect::<Vec<_>>();
    roster.sort_unstable_by_key(|(register, _)| *register);
    if roster.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(LiveRangeError::NonCanonicalRows { function });
    }
    let mut ranges = selected
        .virtual_registers
        .iter()
        .map(|register| VirtualLiveRange {
            virtual_register: register.id,
            class: register.class,
            occurrences: Vec::new(),
            fixed_constraints: register
                .entry_fixed_view
                .map(|view| VirtualFixedConstraint {
                    site: VirtualFixedConstraintSite::Entry,
                    view,
                })
                .into_iter()
                .collect(),
            fragments: Vec::new(),
            edge_connectors: Vec::new(),
        })
        .collect::<Vec<_>>();
    for operand in &live.operand_positions {
        let position = register_position(function, &roster, operand.virtual_register)?;
        let range = &mut ranges[position];
        let point = fragments::operand_point(function, operand)?;
        range.occurrences.push(VirtualOccurrence {
            position: operand.position,
            point,
            instruction: operand.instruction,
            operand: operand.operand,
            access: operand.access,
        });
        if let Some(view) = operand.fixed_view {
            range.fixed_constraints.push(VirtualFixedConstraint {
                site: VirtualFixedConstraintSite::Operand {
                    position: operand.position,
                    point,
                    instruction: operand.instruction,
                    operand: operand.operand,
                    access: operand.access,
                },
                view,
            });
        }
    }
    let mut occupied = vec![BTreeSet::new(); ranges.len()];
    let mut touched = Vec::new();
    for block in &live.blocks {
        for instruction in &block.instructions {
            for (point, members, operands) in [
                (
                    fragments::checked_before(function, instruction.position.0)?,
                    &instruction.virtual_live_in,
                    &instruction.virtual_uses,
                ),
                (
                    fragments::checked_after(function, instruction.position.0)?,
                    &instruction.virtual_live_out,
                    &instruction.virtual_defs,
                ),
            ] {
                for register in members.iter().chain(operands) {
                    let position = register_position(function, &roster, *register)?;
                    if occupied[position].is_empty() {
                        touched.push(position);
                    }
                    occupied[position].insert(point);
                }
            }
        }
        for position in touched.drain(..) {
            fragments::append_maximal(
                block.block,
                std::mem::take(&mut occupied[position]),
                &mut ranges[position].fragments,
            );
        }
        for edge in &block.successors {
            for register in &edge.virtual_live {
                let position = register_position(function, &roster, *register)?;
                ranges[position]
                    .edge_connectors
                    .push(fragments::edge_row(block.block, edge));
            }
        }
    }
    Ok(ranges)
}

fn register_position(
    function: usize,
    roster: &[(VirtualRegisterId, usize)],
    register: VirtualRegisterId,
) -> Result<usize, LiveRangeError> {
    roster
        .binary_search_by_key(&register, |(identity, _)| *identity)
        .map(|position| roster[position].1)
        .map_err(|_| LiveRangeError::VirtualRegisterMismatch {
            function,
            register: register.0,
        })
}

fn replay_copy_affinities(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
) -> Result<Vec<selected_instructions::CopyAffinity>, LiveRangeError> {
    use selected_instructions::SelectedInstructionKind;
    let mut affinities = Vec::new();
    for block in &selected.blocks {
        for row in &block.instructions {
            if row.kind != SelectedInstructionKind::CopyI64 {
                continue;
            }
            let [input, output] = row.operands.as_slice() else {
                return Err(LiveRangeError::FunctionMismatch { function });
            };
            if input.virtual_register == output.virtual_register {
                continue;
            }
            affinities.push(selected_instructions::CopyAffinity {
                block: block.id,
                instruction: row.id,
                source: input.virtual_register,
                destination: output.virtual_register,
            });
        }
    }
    // The same canonical (block, instruction) order compute publishes.
    affinities.sort_unstable();
    Ok(affinities)
}

fn replay_edge_transfers(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    live: &selected_instructions::FunctionLiveness,
) -> Result<Vec<selected_instructions::EdgeRegisterTransfer>, LiveRangeError> {
    use selected_instructions::{SelectedTerminator, VirtualRegisterOrigin};
    let mut transfers = BTreeSet::new();
    for predecessor in &selected.blocks {
        let edges = match &predecessor.terminator {
            SelectedTerminator::Return { .. }
            | SelectedTerminator::Crash { .. }
            | SelectedTerminator::HostedExitProcess { .. } => Vec::new(),
            SelectedTerminator::Jump { successor, .. } => vec![successor],
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
        };
        for edge in edges {
            let successor = live
                .blocks
                .iter()
                .find(|row| row.block == edge.block)
                .ok_or(LiveRangeError::FunctionMismatch { function })?;
            for destination in &successor.virtual_live_in {
                let parameter = selected
                    .virtual_registers
                    .iter()
                    .find(|row| row.id == *destination)
                    .ok_or(LiveRangeError::FunctionMismatch { function })?;
                if !matches!(parameter.origin, VirtualRegisterOrigin::BlockParameter { block, .. } if block == edge.block)
                {
                    continue;
                }
                let argument = crate::analyses::liveness::edge_values::incoming_argument(
                    function,
                    selected,
                    edge,
                    *destination,
                )
                .map_err(LiveRangeError::LivenessRevalidation)?;
                if !transfers.insert(selected_instructions::EdgeRegisterTransfer {
                    source: predecessor.id,
                    target: edge.block,
                    psi_edge: edge.psi_edge,
                    argument,
                    parameter: *destination,
                    class: parameter.class,
                }) {
                    return Err(LiveRangeError::FunctionMismatch { function });
                }
            }
        }
    }
    Ok(transfers.into_iter().collect())
}
