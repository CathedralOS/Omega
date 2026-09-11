//! Function-local transfer preparation and predecessor-driven fixed point.
//!
//! Exact transport validation runs before preparation. Retaining only its edge
//! substitutions avoids re-resolving every live register through the full roster
//! on each visit. Block use/kill summaries avoid re-reading instructions, and only
//! changed entries schedule predecessors. These are producer working facts:
//! independent full-function replay still reconstructs and checks the result.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use register_model::{RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{SelectedFunction, SelectedValueTransport, VirtualRegisterId};

use super::{LivenessError, block_instructions, control};

#[cfg(test)]
std::thread_local! {
    pub(crate) static BLOCK_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Default)]
pub(super) struct BlockFlow {
    pub(super) virtual_entry: BTreeSet<VirtualRegisterId>,
    pub(super) virtual_exit: BTreeSet<VirtualRegisterId>,
    pub(super) unit_entry: BTreeSet<RegisterUnitId>,
    pub(super) unit_exit: BTreeSet<RegisterUnitId>,
}

pub(super) struct EdgeFlow {
    pub(super) target: usize,
    substitutions: Vec<(VirtualRegisterId, VirtualRegisterId)>,
}

pub(super) struct FunctionFlow {
    pub(super) blocks: Vec<BlockFlow>,
    pub(super) edges: Vec<Vec<EdgeFlow>>,
    registers: Vec<VirtualRegisterId>,
}

#[derive(Default)]
struct BlockTransfer {
    virtual_uses: BTreeSet<VirtualRegisterId>,
    virtual_kills: BTreeSet<VirtualRegisterId>,
    unit_uses: BTreeSet<RegisterUnitId>,
    unit_kills: BTreeSet<RegisterUnitId>,
}

impl FunctionFlow {
    /// Called after exact edge transport validation. Only parameter substitutions
    /// are retained per edge; non-parameters keep their own register identity.
    pub(super) fn compute(
        function_index: usize,
        function: &SelectedFunction,
    ) -> Result<Self, LivenessError> {
        let mismatch = || LivenessError::FunctionMismatch {
            function: function_index,
        };
        let mut ordinals = BTreeMap::new();
        for (block_index, block) in function.blocks.iter().enumerate() {
            if ordinals.insert(block.id, block_index).is_some() {
                return Err(mismatch());
            }
        }
        // Ordinals index actual block storage, never a possibly sparse raw ID.
        let mut predecessors = vec![Vec::new(); function.blocks.len()];
        let mut edges = Vec::with_capacity(function.blocks.len());
        let mut transfers = Vec::with_capacity(function.blocks.len());
        for (block_index, block) in function.blocks.iter().enumerate() {
            let mut block_edges = Vec::new();
            for successor in control::successors(&block.terminator) {
                let target = *ordinals.get(&successor.block).ok_or_else(mismatch)?;
                predecessors[target].push(block_index);
                let mut substitutions = successor
                    .bindings
                    .iter()
                    .filter_map(|binding| match binding.transport {
                        SelectedValueTransport::Registers {
                            argument,
                            parameter,
                        } => Some((parameter, argument)),
                        SelectedValueTransport::Unused => None,
                    })
                    .collect::<Vec<_>>();
                if let Some(case) = &successor.structural_case {
                    substitutions.extend(case.payloads.iter().filter_map(|payload| {
                        match payload.transport {
                            selected_instructions::SelectedCasePayloadTransport::Registers { argument, parameter } => Some((parameter, argument)),
                            selected_instructions::SelectedCasePayloadTransport::Unused
                            | selected_instructions::SelectedCasePayloadTransport::Unmaterialized { .. } => None,
                        }
                    }));
                }
                substitutions.sort_unstable_by_key(|(parameter, _)| *parameter);
                block_edges.push(EdgeFlow {
                    target,
                    substitutions,
                });
            }
            edges.push(block_edges);
            let mut transfer = BlockTransfer::default();
            for instruction in block_instructions(block) {
                // Uses precede simultaneous definitions, as in reverse transfer.
                for operand in &instruction.operands {
                    if operand.access == RegisterOperandAccess::Use
                        && !transfer.virtual_kills.contains(&operand.virtual_register)
                    {
                        transfer.virtual_uses.insert(operand.virtual_register);
                    }
                }
                transfer.virtual_kills.extend(
                    instruction
                        .operands
                        .iter()
                        .filter(|operand| operand.access == RegisterOperandAccess::Def)
                        .map(|operand| operand.virtual_register),
                );
                transfer.unit_uses.extend(
                    instruction
                        .implicit_uses
                        .iter()
                        .filter(|unit| !transfer.unit_kills.contains(*unit))
                        .copied(),
                );
                transfer.unit_kills.extend(
                    instruction
                        .implicit_defs
                        .iter()
                        .chain(&instruction.clobbers)
                        .copied(),
                );
            }
            transfers.push(transfer);
        }
        // Leaf functions never query an incoming register, so they need no roster.
        let mut registers = if edges.iter().all(Vec::is_empty) {
            Vec::new()
        } else {
            function
                .virtual_registers
                .iter()
                .map(|register| register.id)
                .collect::<Vec<_>>()
        };
        registers.sort_unstable();
        let mut flow = Self {
            blocks: (0..function.blocks.len())
                .map(|_| BlockFlow::default())
                .collect(),
            edges,
            registers,
        };
        let mut pending = vec![true; function.blocks.len()];
        let mut work = (0..function.blocks.len()).rev().collect::<VecDeque<_>>();
        while let Some(block_index) = work.pop_front() {
            #[cfg(test)]
            BLOCK_VISITS.set(BLOCK_VISITS.get() + 1);
            pending[block_index] = false;
            let mut virtual_exit = BTreeSet::new();
            let mut unit_exit = BTreeSet::new();
            for edge in &flow.edges[block_index] {
                let target = &flow.blocks[edge.target];
                for register in &target.virtual_entry {
                    virtual_exit.insert(flow.incoming_argument(function_index, edge, *register)?);
                }
                unit_exit.extend(target.unit_entry.iter().copied());
            }
            let transfer = &transfers[block_index];
            let virtual_entry = virtual_exit
                .difference(&transfer.virtual_kills)
                .copied()
                .chain(transfer.virtual_uses.iter().copied())
                .collect();
            let unit_entry = unit_exit
                .difference(&transfer.unit_kills)
                .copied()
                .chain(transfer.unit_uses.iter().copied())
                .collect();
            let state = &mut flow.blocks[block_index];
            let entry_changed =
                state.virtual_entry != virtual_entry || state.unit_entry != unit_entry;
            // Exits can change even when a local definition kills every new
            // incoming value. Keep them for instruction/successor evidence.
            *state = BlockFlow {
                virtual_entry,
                virtual_exit,
                unit_entry,
                unit_exit,
            };
            if entry_changed {
                for predecessor in &predecessors[block_index] {
                    if !pending[*predecessor] {
                        pending[*predecessor] = true;
                        work.push_back(*predecessor);
                    }
                }
            }
        }
        Ok(flow)
    }

    pub(super) fn incoming_argument(
        &self,
        function_index: usize,
        edge: &EdgeFlow,
        destination: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, LivenessError> {
        let mismatch = || LivenessError::FunctionMismatch {
            function: function_index,
        };
        let register_index = self
            .registers
            .binary_search(&destination)
            .map_err(|_| mismatch())?;
        // Keep missing/duplicate live-register rejection, including non-parameters.
        if register_index
            .checked_sub(1)
            .and_then(|previous| self.registers.get(previous))
            == Some(&destination)
            || self.registers.get(register_index + 1) == Some(&destination)
        {
            return Err(mismatch());
        }
        Ok(edge
            .substitutions
            .binary_search_by_key(&destination, |(parameter, _)| *parameter)
            .map_or(destination, |binding_index| {
                edge.substitutions[binding_index].1
            }))
    }
}
