//! Ordinary and structural function reconstruction.

use std::collections::BTreeSet;

use crate::{
    FunctionLiveRanges, LiveRangeError, VirtualFixedConstraint, VirtualFixedConstraintSite,
    VirtualInterference, VirtualLiveRange, VirtualOccurrence,
};

use super::{architectural_units, constraints, fragments};

pub(super) fn replay_structural_function(
    function: usize,
    machine: semantic_vocabulary::MachineId,
    live: &crate::FunctionLiveness,
) -> Result<FunctionLiveRanges, LiveRangeError> {
    if live.machine != machine
        || !live.entry_definitions.is_empty()
        || !live.operand_positions.is_empty()
    {
        return Err(LiveRangeError::FunctionMismatch { function });
    }
    let block_domains = fragments::block_domains(function, live)?;
    let architectural_units = architectural_units::replay_all(function, live)?;
    Ok(FunctionLiveRanges {
        machine,
        block_domains,
        virtual_registers: Vec::new(),
        edge_transfers: Vec::new(),
        tied_pairs: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units,
        interference: Vec::new(),
    })
}

pub(super) fn replay_function(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    live: &crate::FunctionLiveness,
) -> Result<FunctionLiveRanges, LiveRangeError> {
    constraints::reject_unsupported(function, live)?;
    let tied_pairs = constraints::derive_ties(function, live)?;
    let edge_transfers = replay_edge_transfers(function, selected, live)?;
    let early_clobbers = constraints::derive_early_clobbers(function, live)?;
    let block_domains = fragments::block_domains(function, live)?;

    let mut virtual_registers = Vec::new();
    for register in &selected.virtual_registers {
        let mut occurrences = Vec::new();
        let mut fixed_constraints = Vec::new();
        if let Some(view) = register.entry_fixed_view {
            fixed_constraints.push(VirtualFixedConstraint {
                site: VirtualFixedConstraintSite::Entry,
                view,
            });
        }
        for operand in &live.operand_positions {
            if operand.virtual_register != register.id {
                continue;
            }
            let point = fragments::operand_point(function, operand)?;
            occurrences.push(VirtualOccurrence {
                position: operand.position,
                point,
                instruction: operand.instruction,
                operand: operand.operand,
                access: operand.access,
            });
            if let Some(view) = operand.fixed_view {
                fixed_constraints.push(VirtualFixedConstraint {
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
        let mut range_fragments = Vec::new();
        let mut edge_connectors = Vec::new();
        for block in &live.blocks {
            let mut occupied = BTreeSet::new();
            for instruction in &block.instructions {
                if instruction
                    .virtual_live_in
                    .binary_search(&register.id)
                    .is_ok()
                    || instruction.virtual_uses.binary_search(&register.id).is_ok()
                {
                    occupied.insert(fragments::checked_before(function, instruction.position.0)?);
                }
                if instruction
                    .virtual_live_out
                    .binary_search(&register.id)
                    .is_ok()
                    || instruction.virtual_defs.binary_search(&register.id).is_ok()
                {
                    occupied.insert(fragments::checked_after(function, instruction.position.0)?);
                }
            }
            fragments::append_maximal(block.block, occupied, &mut range_fragments);
            edge_connectors.extend(
                block
                    .successors
                    .iter()
                    .filter(|edge| edge.virtual_live.binary_search(&register.id).is_ok())
                    .map(|edge| fragments::edge_row(block.block, edge)),
            );
        }
        virtual_registers.push(VirtualLiveRange {
            virtual_register: register.id,
            class: register.class,
            occurrences,
            fixed_constraints,
            fragments: range_fragments,
            edge_connectors,
        });
    }

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
        early_clobbers,
        architectural_units,
        interference,
    })
}

fn replay_edge_transfers(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    live: &crate::FunctionLiveness,
) -> Result<Vec<crate::EdgeRegisterTransfer>, LiveRangeError> {
    use selected_instructions::{SelectedTerminator, VirtualRegisterOrigin};
    let mut transfers = BTreeSet::new();
    for predecessor in &selected.blocks {
        let edges = match &predecessor.terminator {
            SelectedTerminator::Return { .. } => Vec::new(),
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
                if !transfers.insert(crate::EdgeRegisterTransfer {
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
