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

#[derive(Default)]
pub(super) struct BlockFlow {
    pub(super) virtual_entry: Vec<VirtualRegisterId>,
    pub(super) virtual_exit: Vec<VirtualRegisterId>,
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
    pub(super) max_reg_id: usize,
}

#[derive(Default)]
struct BlockTransfer {
    virtual_uses_bits: Vec<u64>,
    virtual_kills_bits: Vec<u64>,
    unit_uses: BTreeSet<RegisterUnitId>,
    unit_kills: BTreeSet<RegisterUnitId>,
}

fn bit_test(words: &[u64], id: u32) -> bool {
    let idx = id as usize;
    words
        .get(idx / 64)
        .is_some_and(|&word| (word & (1u64 << (idx % 64))) != 0)
}

fn bit_set(words: &mut [u64], id: u32) {
    let idx = id as usize;
    words[idx / 64] |= 1u64 << (idx % 64);
}

pub(super) fn bits_to_vregs(words: &[u64]) -> Vec<VirtualRegisterId> {
    let mut out = Vec::new();
    for (word_idx, &words) in words.iter().enumerate() {
        let mut bits = words;
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            out.push(VirtualRegisterId((word_idx * 64 + bit) as u32));
        }
    }
    out
}

impl FunctionFlow {
    /// Called after exact edge transport validation. Only parameter substitutions
    /// are retained per edge; non-parameters keep their own register identity.
    pub(super) fn compute(
        function_index: usize,
        function: &SelectedFunction,
    ) -> Result<Self, LivenessError> {
        Self::compute_domains(function_index, function, true, None)
    }

    pub(super) fn compute_units_filtered(
        function_index: usize,
        function: &SelectedFunction,
        unit_filter: &[RegisterUnitId],
    ) -> Result<Self, LivenessError> {
        Self::compute_domains(function_index, function, false, Some(unit_filter))
    }

    fn compute_domains(
        function_index: usize,
        function: &SelectedFunction,
        include_virtual: bool,
        unit_filter: Option<&[RegisterUnitId]>,
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
        let max_reg_id = if include_virtual {
            let mut max_id = function
                .virtual_registers
                .iter()
                .map(|reg| reg.id.0 as usize)
                .max()
                .unwrap_or(0);
            for block in &function.blocks {
                for instruction in block_instructions(block) {
                    for operand in &instruction.operands {
                        max_id = max_id.max(operand.virtual_register.0 as usize);
                    }
                }
                for successor in control::successors(&block.terminator) {
                    for binding in &successor.bindings {
                        if let SelectedValueTransport::Registers {
                            argument,
                            parameter,
                        } = binding.transport
                        {
                            max_id = max_id.max(argument.0 as usize).max(parameter.0 as usize);
                        }
                    }
                    if let Some(case) = &successor.structural_case {
                        for payload in &case.payloads {
                            if let selected_instructions::SelectedCasePayloadTransport::Registers {
                                argument,
                                parameter,
                            } = payload.transport
                            {
                                max_id = max_id.max(argument.0 as usize).max(parameter.0 as usize);
                            }
                        }
                    }
                }
            }
            max_id
        } else {
            0
        };
        let word_count = if include_virtual {
            (max_reg_id + 1).div_ceil(64)
        } else {
            0
        };
        // Ordinals index actual block storage, never a possibly sparse raw ID.
        let mut predecessors = vec![Vec::new(); function.blocks.len()];
        let mut edges = Vec::with_capacity(function.blocks.len());
        let mut transfers = Vec::with_capacity(function.blocks.len());
        for (block_index, block) in function.blocks.iter().enumerate() {
            let mut block_edges = Vec::new();
            for successor in control::successors(&block.terminator) {
                let target = *ordinals.get(&successor.block).ok_or_else(mismatch)?;
                predecessors[target].push(block_index);
                let mut substitutions = if include_virtual {
                    let mut subs = successor
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
                        subs.extend(case.payloads.iter().filter_map(|payload| {
                            match payload.transport {
                                selected_instructions::SelectedCasePayloadTransport::Registers { argument, parameter } => Some((parameter, argument)),
                                selected_instructions::SelectedCasePayloadTransport::Unused
                                | selected_instructions::SelectedCasePayloadTransport::Unmaterialized { .. } => None,
                            }
                        }));
                    }
                    subs.sort_unstable_by_key(|(parameter, _)| *parameter);
                    subs
                } else {
                    Vec::new()
                };
                let _ = &mut substitutions;
                block_edges.push(EdgeFlow {
                    target,
                    substitutions,
                });
            }
            edges.push(block_edges);
            let mut transfer = BlockTransfer {
                virtual_uses_bits: vec![0; word_count],
                virtual_kills_bits: vec![0; word_count],
                unit_uses: BTreeSet::new(),
                unit_kills: BTreeSet::new(),
            };
            if let Some((upward_unit_uses, unit_kills)) =
                function.blocks.block_unit_transfer(block_index)
            {
                if let Some(filter) = unit_filter {
                    for &unit in upward_unit_uses {
                        if filter.contains(&unit) {
                            transfer.unit_uses.insert(unit);
                        }
                    }
                    for &unit in unit_kills {
                        if filter.contains(&unit) {
                            transfer.unit_kills.insert(unit);
                        }
                    }
                } else {
                    transfer.unit_uses.extend(upward_unit_uses.iter().copied());
                    transfer.unit_kills.extend(unit_kills.iter().copied());
                }
            }
            if include_virtual {
                for instruction in block_instructions(block) {
                    // Uses precede simultaneous definitions, as in reverse transfer.
                    for operand in &instruction.operands {
                        if operand.access == RegisterOperandAccess::Use
                            && !bit_test(&transfer.virtual_kills_bits, operand.virtual_register.0)
                        {
                            bit_set(&mut transfer.virtual_uses_bits, operand.virtual_register.0);
                        }
                    }
                    for operand in &instruction.operands {
                        if operand.access == RegisterOperandAccess::Def {
                            bit_set(&mut transfer.virtual_kills_bits, operand.virtual_register.0);
                        }
                    }
                }
            }
            transfers.push(transfer);
        }
        // Leaf functions never query an incoming register, so they need no roster.
        let mut registers = if !include_virtual || edges.iter().all(Vec::is_empty) {
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
            max_reg_id,
        };
        if !include_virtual && transfers.iter().all(|t| t.unit_uses.is_empty()) {
            return Ok(flow);
        }
        let mut virtual_entry_bits = vec![vec![0u64; word_count]; function.blocks.len()];
        let mut virtual_exit_bits = vec![vec![0u64; word_count]; function.blocks.len()];
        let mut next_exit_bits = vec![0u64; word_count];
        let mut next_entry_bits = vec![0u64; word_count];
        let mut pending = vec![true; function.blocks.len()];
        let mut work = (0..function.blocks.len()).rev().collect::<VecDeque<_>>();
        while let Some(block_index) = work.pop_front() {
            pending[block_index] = false;
            let mut unit_exit = BTreeSet::new();
            if include_virtual {
                next_exit_bits.fill(0);
                for edge in &flow.edges[block_index] {
                    let target = &flow.blocks[edge.target];
                    for &register in &target.virtual_entry {
                        let arg = flow.incoming_argument(function_index, edge, register)?;
                        bit_set(&mut next_exit_bits, arg.0);
                    }
                    unit_exit.extend(target.unit_entry.iter().copied());
                }
                let transfer = &transfers[block_index];
                let mut virtual_changed = false;
                for w in 0..word_count {
                    let entry_w =
                        (next_exit_bits[w] & !transfer.virtual_kills_bits[w]) | transfer.virtual_uses_bits[w];
                    next_entry_bits[w] = entry_w;
                    if entry_w != virtual_entry_bits[block_index][w] {
                        virtual_changed = true;
                    }
                }
                virtual_exit_bits[block_index].copy_from_slice(&next_exit_bits);
                let unit_entry: BTreeSet<RegisterUnitId> = unit_exit
                    .difference(&transfer.unit_kills)
                    .copied()
                    .chain(transfer.unit_uses.iter().copied())
                    .collect();
                let state = &mut flow.blocks[block_index];
                let entry_changed = virtual_changed || state.unit_entry != unit_entry;
                if virtual_changed {
                    virtual_entry_bits[block_index].copy_from_slice(&next_entry_bits);
                    state.virtual_entry.clear();
                    for (word_idx, &words) in next_entry_bits.iter().enumerate() {
                        let mut bits = words;
                        while bits != 0 {
                            let bit = bits.trailing_zeros() as usize;
                            bits &= bits - 1;
                            state
                                .virtual_entry
                                .push(VirtualRegisterId((word_idx * 64 + bit) as u32));
                        }
                    }
                }
                state.unit_entry = unit_entry;
                state.unit_exit = unit_exit;
                if entry_changed {
                    for predecessor in &predecessors[block_index] {
                        if !pending[*predecessor] {
                            pending[*predecessor] = true;
                            work.push_back(*predecessor);
                        }
                    }
                }
            } else {
                for edge in &flow.edges[block_index] {
                    let target = &flow.blocks[edge.target];
                    unit_exit.extend(target.unit_entry.iter().copied());
                }
                let transfer = &transfers[block_index];
                let unit_entry = unit_exit
                    .difference(&transfer.unit_kills)
                    .copied()
                    .chain(transfer.unit_uses.iter().copied())
                    .collect();
                let state = &mut flow.blocks[block_index];
                let entry_changed = state.unit_entry != unit_entry;
                state.unit_entry = unit_entry;
                state.unit_exit = unit_exit;
                if entry_changed {
                    for predecessor in &predecessors[block_index] {
                        if !pending[*predecessor] {
                            pending[*predecessor] = true;
                            work.push_back(*predecessor);
                        }
                    }
                }
            }
        }
        if include_virtual {
            for (block_index, state) in flow.blocks.iter_mut().enumerate() {
                state.virtual_exit = bits_to_vregs(&virtual_exit_bits[block_index]);
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
