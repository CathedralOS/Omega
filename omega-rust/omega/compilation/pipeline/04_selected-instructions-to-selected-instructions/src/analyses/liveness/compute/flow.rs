//! Function-local transfer preparation and predecessor-driven fixed point.
//!
//! Exact transport validation runs before preparation. Retaining only its edge
//! substitutions avoids re-resolving every live register through the full roster
//! on each visit. Block use/kill summaries avoid re-reading instructions, and only
//! changed entries schedule predecessors. These are producer working facts:
//! independent full-function replay still reconstructs and checks the result.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use register_model::RegisterUnitId;
use selected_instructions::{SelectedFunction, SelectedValueTransport, VirtualRegisterId};

use super::{LivenessError, control};

#[derive(Default)]
pub(super) struct BlockFlow {
    pub(super) virtual_entry: Vec<VirtualRegisterId>,
    pub(super) virtual_exit: Vec<VirtualRegisterId>,
    pub(super) unit_entry: BTreeSet<RegisterUnitId>,
    pub(super) unit_exit: BTreeSet<RegisterUnitId>,
}

pub(super) struct EdgeFlow {
    pub(super) target: usize,
    pub(super) substitutions: Vec<(VirtualRegisterId, VirtualRegisterId)>,
    param_bits: Vec<u64>,
}

pub(super) struct FunctionFlow {
    pub(super) blocks: Vec<BlockFlow>,
    pub(super) edges: Vec<Vec<EdgeFlow>>,
    valid_register_bits: Vec<u64>,
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
        let word_count = if include_virtual {
            let max_decl_words = function
                .virtual_registers
                .last()
                .map_or(0, |last| (last.id.0 as usize / 64) + 1)
                .max(
                    function
                        .virtual_registers
                        .iter()
                        .map(|reg| (reg.id.0 as usize / 64) + 1)
                        .max()
                        .unwrap_or(0),
                );
            max_decl_words
                .max(function.blocks.max_referenced_register_word_count())
                .max(1)
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
                let mut param_bits = Vec::new();
                if !substitutions.is_empty() {
                    param_bits.resize(word_count, 0);
                    for &(parameter, _) in &substitutions {
                        bit_set(&mut param_bits, parameter.0);
                    }
                }
                block_edges.push(EdgeFlow {
                    target,
                    substitutions,
                    param_bits,
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
            if include_virtual
                && let Some((upward_uses, kills)) =
                    function.blocks.block_virtual_transfer(block_index)
            {
                let uses_len = upward_uses.len().min(word_count);
                transfer.virtual_uses_bits[..uses_len].copy_from_slice(&upward_uses[..uses_len]);
                let kills_len = kills.len().min(word_count);
                transfer.virtual_kills_bits[..kills_len].copy_from_slice(&kills[..kills_len]);
            }
            transfers.push(transfer);
        }
        // Leaf functions never query an incoming register, so they need no roster.
        let mut valid_register_bits = Vec::new();
        if include_virtual && !edges.iter().all(Vec::is_empty) {
            valid_register_bits.resize(word_count, 0);
            let mut duplicate_register_bits = vec![0u64; word_count];
            for register in &function.virtual_registers {
                let id = register.id.0;
                if bit_test(&valid_register_bits, id) {
                    bit_set(&mut duplicate_register_bits, id);
                } else {
                    bit_set(&mut valid_register_bits, id);
                }
            }
            for (valid, &dup) in valid_register_bits.iter_mut().zip(&duplicate_register_bits) {
                *valid &= !dup;
            }
        }
        let mut flow = Self {
            blocks: (0..function.blocks.len())
                .map(|_| BlockFlow::default())
                .collect(),
            edges,
            valid_register_bits,
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
                    let target_entry_bits = &virtual_entry_bits[edge.target];
                    if edge.substitutions.is_empty() {
                        for ((exit_w, &entry_w), &valid_w) in next_exit_bits
                            .iter_mut()
                            .zip(target_entry_bits)
                            .zip(&flow.valid_register_bits)
                        {
                            if (entry_w & !valid_w) != 0 {
                                return Err(mismatch());
                            }
                            *exit_w |= entry_w;
                        }
                    } else {
                        for (((exit_w, &entry_w), &valid_w), &param_w) in next_exit_bits
                            .iter_mut()
                            .zip(target_entry_bits)
                            .zip(&flow.valid_register_bits)
                            .zip(&edge.param_bits)
                        {
                            if (entry_w & !valid_w) != 0 {
                                return Err(mismatch());
                            }
                            *exit_w |= entry_w & !param_w;
                        }
                        for &(parameter, argument) in &edge.substitutions {
                            if bit_test(target_entry_bits, parameter.0) {
                                bit_set(&mut next_exit_bits, argument.0);
                            }
                        }
                    }
                    unit_exit.extend(flow.blocks[edge.target].unit_entry.iter().copied());
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
                state.virtual_entry = bits_to_vregs(&virtual_entry_bits[block_index]);
                state.virtual_exit = bits_to_vregs(&virtual_exit_bits[block_index]);
            }
        }
        Ok(flow)
    }

    #[inline]
    pub(super) fn is_valid_register(&self, destination: VirtualRegisterId) -> bool {
        bit_test(&self.valid_register_bits, destination.0)
    }

    pub(super) fn incoming_argument(
        &self,
        function_index: usize,
        edge: &EdgeFlow,
        destination: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, LivenessError> {
        if !self.is_valid_register(destination) {
            return Err(LivenessError::FunctionMismatch {
                function: function_index,
            });
        }
        Ok(edge
            .substitutions
            .binary_search_by_key(&destination, |(parameter, _)| *parameter)
            .map_or(destination, |binding_index| {
                edge.substitutions[binding_index].1
            }))
    }
}
