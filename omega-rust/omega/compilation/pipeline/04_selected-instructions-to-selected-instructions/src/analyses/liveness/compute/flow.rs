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
    pub(super) unit_entry: Vec<RegisterUnitId>,
    pub(super) unit_exit: Vec<RegisterUnitId>,
    pub(super) unit_entry_mask: u64,
    pub(super) unit_exit_mask: u64,
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
struct BlockTransfer<'a> {
    virtual_uses_bits: &'a [u64],
    virtual_kills_bits: &'a [u64],
    unit_uses_mask: u64,
    unit_kills_mask: u64,
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

pub(super) fn mask_to_units(mut mask: u64) -> Vec<RegisterUnitId> {
    if mask == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(mask.count_ones() as usize);
    while mask != 0 {
        let bit = mask.trailing_zeros() as u16;
        mask &= mask - 1;
        out.push(RegisterUnitId(bit));
    }
    out
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
        let mut blocks_identity = true;
        let mut max_block_id = 0usize;
        for (block_index, block) in function.blocks.iter().enumerate() {
            let id = block.id.0 as usize;
            if id != block_index {
                blocks_identity = false;
            }
            if id > max_block_id {
                max_block_id = id;
            }
        }
        let mut dense_ordinals = Vec::new();
        let mut sparse_ordinals = BTreeMap::new();
        if !blocks_identity {
            if max_block_id < 65536 {
                dense_ordinals.resize(max_block_id + 1, u32::MAX);
                for (block_index, block) in function.blocks.iter().enumerate() {
                    let slot = &mut dense_ordinals[block.id.0 as usize];
                    if *slot != u32::MAX {
                        return Err(mismatch());
                    }
                    *slot = block_index as u32;
                }
            } else {
                for (block_index, block) in function.blocks.iter().enumerate() {
                    if sparse_ordinals.insert(block.id, block_index).is_some() {
                        return Err(mismatch());
                    }
                }
            }
        }
        let resolve_block =
            |id: selected_instructions::SelectedBlockId| -> Result<usize, LivenessError> {
                let idx = id.0 as usize;
                if blocks_identity {
                    if idx < function.blocks.len() {
                        Ok(idx)
                    } else {
                        Err(mismatch())
                    }
                } else if !dense_ordinals.is_empty() {
                    dense_ordinals
                        .get(idx)
                        .copied()
                        .filter(|&v| v != u32::MAX)
                        .map(|v| v as usize)
                        .ok_or_else(mismatch)
                } else {
                    sparse_ordinals.get(&id).copied().ok_or_else(mismatch)
                }
            };
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
        let mut filter_mask = u64::MAX;
        let mut can_use_unit_masks = true;
        if let Some(filter) = unit_filter {
            filter_mask = 0;
            for &u in filter {
                if (u.0 as usize) < 64 {
                    filter_mask |= 1u64 << u.0;
                } else {
                    can_use_unit_masks = false;
                }
            }
        }
        // Ordinals index actual block storage, never a possibly sparse raw ID.
        let mut predecessors = vec![Vec::new(); function.blocks.len()];
        let mut edges = Vec::with_capacity(function.blocks.len());
        let mut transfers = Vec::with_capacity(function.blocks.len());
        for (block_index, block) in function.blocks.iter().enumerate() {
            let mut block_edges = Vec::new();
            for successor in control::successors(&block.terminator) {
                let target = resolve_block(successor.block)?;
                predecessors[target].push(block_index);
                let substitutions = if include_virtual {
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
            let (virtual_uses_bits, virtual_kills_bits) = if include_virtual {
                function
                    .blocks
                    .block_virtual_transfer(block_index)
                    .unwrap_or((&[], &[]))
            } else {
                (&[][..], &[][..])
            };
            let mut transfer = BlockTransfer {
                virtual_uses_bits,
                virtual_kills_bits,
                unit_uses_mask: 0,
                unit_kills_mask: 0,
                unit_uses: BTreeSet::new(),
                unit_kills: BTreeSet::new(),
            };
            if let Some((_, upward_mask, kills_mask, _, has_high)) =
                function.blocks.block_unit_masks(block_index)
            {
                if !has_high && can_use_unit_masks {
                    transfer.unit_uses_mask = upward_mask & filter_mask;
                    transfer.unit_kills_mask = kills_mask & filter_mask;
                } else {
                    can_use_unit_masks = false;
                }
            } else {
                can_use_unit_masks = false;
            }
            transfers.push(transfer);
        }
        if !can_use_unit_masks {
            for (block_index, transfer) in transfers.iter_mut().enumerate() {
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
            }
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
        if !include_virtual
            && ((can_use_unit_masks && transfers.iter().all(|t| t.unit_uses_mask == 0))
                || (!can_use_unit_masks && transfers.iter().all(|t| t.unit_uses.is_empty())))
        {
            return Ok(flow);
        }
        let mut virtual_entry_bits = vec![vec![0u64; word_count]; function.blocks.len()];
        let mut virtual_exit_bits = vec![vec![0u64; word_count]; function.blocks.len()];
        let mut next_exit_bits = vec![0u64; word_count];
        let mut next_entry_bits = vec![0u64; word_count];
        let mut unit_entry_sets = if can_use_unit_masks {
            Vec::new()
        } else {
            vec![BTreeSet::new(); function.blocks.len()]
        };
        let mut unit_exit_sets = if can_use_unit_masks {
            Vec::new()
        } else {
            vec![BTreeSet::new(); function.blocks.len()]
        };
        let mut pending = vec![true; function.blocks.len()];
        let mut work = (0..function.blocks.len()).rev().collect::<VecDeque<_>>();
        while let Some(block_index) = work.pop_front() {
            pending[block_index] = false;
            if include_virtual {
                next_exit_bits.fill(0);
                let mut unit_exit_mask = 0u64;
                let mut unit_exit = BTreeSet::new();
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
                    if can_use_unit_masks {
                        unit_exit_mask |= flow.blocks[edge.target].unit_entry_mask;
                    } else {
                        unit_exit.extend(unit_entry_sets[edge.target].iter().copied());
                    }
                }
                let transfer = &transfers[block_index];
                let mut virtual_changed = false;
                for w in 0..word_count {
                    let kill_w = transfer.virtual_kills_bits.get(w).copied().unwrap_or(0);
                    let use_w = transfer.virtual_uses_bits.get(w).copied().unwrap_or(0);
                    let entry_w = (next_exit_bits[w] & !kill_w) | use_w;
                    next_entry_bits[w] = entry_w;
                    if entry_w != virtual_entry_bits[block_index][w] {
                        virtual_changed = true;
                    }
                }
                virtual_exit_bits[block_index].copy_from_slice(&next_exit_bits);
                if virtual_changed {
                    virtual_entry_bits[block_index].copy_from_slice(&next_entry_bits);
                }
                let entry_changed = if can_use_unit_masks {
                    let unit_entry_mask =
                        (unit_exit_mask & !transfer.unit_kills_mask) | transfer.unit_uses_mask;
                    let state = &mut flow.blocks[block_index];
                    let changed = virtual_changed || state.unit_entry_mask != unit_entry_mask;
                    state.unit_entry_mask = unit_entry_mask;
                    state.unit_exit_mask = unit_exit_mask;
                    changed
                } else {
                    let unit_entry: BTreeSet<RegisterUnitId> = unit_exit
                        .difference(&transfer.unit_kills)
                        .copied()
                        .chain(transfer.unit_uses.iter().copied())
                        .collect();
                    let changed = virtual_changed || unit_entry_sets[block_index] != unit_entry;
                    unit_entry_sets[block_index] = unit_entry;
                    unit_exit_sets[block_index] = unit_exit;
                    changed
                };
                if entry_changed {
                    for predecessor in &predecessors[block_index] {
                        if !pending[*predecessor] {
                            pending[*predecessor] = true;
                            work.push_back(*predecessor);
                        }
                    }
                }
            } else if can_use_unit_masks {
                let mut unit_exit_mask = 0u64;
                for edge in &flow.edges[block_index] {
                    unit_exit_mask |= flow.blocks[edge.target].unit_entry_mask;
                }
                let transfer = &transfers[block_index];
                let unit_entry_mask =
                    (unit_exit_mask & !transfer.unit_kills_mask) | transfer.unit_uses_mask;
                let state = &mut flow.blocks[block_index];
                let entry_changed = state.unit_entry_mask != unit_entry_mask;
                state.unit_entry_mask = unit_entry_mask;
                state.unit_exit_mask = unit_exit_mask;
                if entry_changed {
                    for predecessor in &predecessors[block_index] {
                        if !pending[*predecessor] {
                            pending[*predecessor] = true;
                            work.push_back(*predecessor);
                        }
                    }
                }
            } else {
                let mut unit_exit = BTreeSet::new();
                for edge in &flow.edges[block_index] {
                    unit_exit.extend(unit_entry_sets[edge.target].iter().copied());
                }
                let transfer = &transfers[block_index];
                let unit_entry: BTreeSet<RegisterUnitId> = unit_exit
                    .difference(&transfer.unit_kills)
                    .copied()
                    .chain(transfer.unit_uses.iter().copied())
                    .collect();
                let entry_changed = unit_entry_sets[block_index] != unit_entry;
                unit_entry_sets[block_index] = unit_entry;
                unit_exit_sets[block_index] = unit_exit;
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
        for (block_index, state) in flow.blocks.iter_mut().enumerate() {
            if include_virtual {
                state.virtual_entry = bits_to_vregs(&virtual_entry_bits[block_index]);
                state.virtual_exit = bits_to_vregs(&virtual_exit_bits[block_index]);
            }
            if can_use_unit_masks {
                state.unit_entry = mask_to_units(state.unit_entry_mask);
                state.unit_exit = mask_to_units(state.unit_exit_mask);
            } else {
                state.unit_entry = unit_entry_sets[block_index].iter().copied().collect();
                state.unit_exit = unit_exit_sets[block_index].iter().copied().collect();
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
