/*
  Selected-CFG dataflow liveness analysis (`analyze_liveness`,
  `analyze_liveness_reusing`, `ValidatedLiveness`, `StagedOptimizedLiveness`).

  Computes bounded backward liveness over virtual registers and physical
  architectural units across `SelectedInstructionPlan` control flow, edge
  bindings, and structural case payloads.
*/

use crate::SelectedProgramView;
pub mod compute {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use crate::liveness::LivenessError;
    use register_model::{RegisterOperandAccess, RegisterUnitId};
    use selected_instructions::{
        BlockLiveness, EntryDefinition, FunctionLiveness, InstructionLiveness, LivenessPlan,
        LivenessPosition, OperandPosition, SelectedBlock, SelectedFunction, SelectedInstruction,
        SuccessorLiveness, VirtualRegisterId,
    };

    mod control {

        use selected_instructions::{SelectedSuccessor, SelectedTerminator};

        pub(super) fn successors(
            terminator: &SelectedTerminator,
        ) -> impl DoubleEndedIterator<Item = &SelectedSuccessor> {
            let successors = match terminator {
                SelectedTerminator::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                    ..
                } => [Some(when_nonzero), Some(when_zero)],
                selected_instructions::conditional_branch_less_pair_pattern!(
                    when_less,
                    when_not_less,
                    ..
                ) => [Some(when_less), Some(when_not_less)],
                SelectedTerminator::Jump { successor, .. } => [Some(successor), None],
                selected_instructions::terminator_no_successor_pattern!(..) => [None, None],
            };
            successors.into_iter().flatten()
        }
    }
    mod flow {

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
            pub fn compute(
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
                    for (valid, &dup) in
                        valid_register_bits.iter_mut().zip(&duplicate_register_bits)
                    {
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
                        || (!can_use_unit_masks
                            && transfers.iter().all(|t| t.unit_uses.is_empty())))
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
                    let mut schedule_predecessors = |entry_changed: bool| {
                        if entry_changed {
                            for predecessor in &predecessors[block_index] {
                                if !pending[*predecessor] {
                                    pending[*predecessor] = true;
                                    work.push_back(*predecessor);
                                }
                            }
                        }
                    };
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
                            let unit_entry_mask = (unit_exit_mask & !transfer.unit_kills_mask)
                                | transfer.unit_uses_mask;
                            let state = &mut flow.blocks[block_index];
                            let changed =
                                virtual_changed || state.unit_entry_mask != unit_entry_mask;
                            state.unit_entry_mask = unit_entry_mask;
                            state.unit_exit_mask = unit_exit_mask;
                            changed
                        } else {
                            let unit_entry: BTreeSet<RegisterUnitId> = unit_exit
                                .difference(&transfer.unit_kills)
                                .copied()
                                .chain(transfer.unit_uses.iter().copied())
                                .collect();
                            let changed =
                                virtual_changed || unit_entry_sets[block_index] != unit_entry;
                            unit_entry_sets[block_index] = unit_entry;
                            unit_exit_sets[block_index] = unit_exit;
                            changed
                        };
                        schedule_predecessors(entry_changed);
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
                        schedule_predecessors(entry_changed);
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
                        schedule_predecessors(entry_changed);
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
    }

    pub(crate) fn compute_terminal_liveness(
        selected: &impl crate::SelectedProgramView,
    ) -> Result<LivenessPlan, LivenessError> {
        let plan = selected.selected_plan();
        let functions = plan
            .functions
            .iter()
            .enumerate()
            .map(|(index, function)| compute_function(index, function).map(Arc::new))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(LivenessPlan {
            selected: selected.selected_identity(),
            optimization_unit: selected.optimization_unit_identity(),
            work_schedule: selected.work_schedule_identity(),
            target: plan.target,
            functions,
        })
    }

    pub(crate) fn compute_terminal_liveness_reusing(
        previous: &impl crate::SelectedProgramView,
        previous_liveness: &crate::ValidatedLiveness,
        selected: &impl crate::SelectedProgramView,
    ) -> Result<(LivenessPlan, Vec<bool>), LivenessError> {
        let plan = selected.selected_plan();
        let prior = previous_liveness.plan();
        let mut carried = vec![false; plan.functions.len()];
        let compatible = prior.selected == previous.selected_identity()
            && prior.optimization_unit == previous.optimization_unit_identity()
            && prior.work_schedule == previous.work_schedule_identity()
            && prior.target == previous.selected_plan().target
            && prior.optimization_unit == selected.optimization_unit_identity()
            && prior.work_schedule == selected.work_schedule_identity()
            && prior.target == plan.target
            && prior.functions.len() == previous.selected_plan().functions.len();
        let functions = plan
            .functions
            .iter()
            .enumerate()
            .map(|(function_index, function)| {
                if compatible
                    && (previous
                        .selected_plan()
                        .functions
                        .shares_function_storage(&plan.functions, function_index)
                        || previous.selected_plan().functions.get(function_index) == Some(function))
                    && let Some(facts) = prior.functions.get(function_index)
                {
                    carried[function_index] = true;
                    return Ok(facts.clone());
                }
                compute_function(function_index, function).map(Arc::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((
            LivenessPlan {
                selected: selected.selected_identity(),
                optimization_unit: selected.optimization_unit_identity(),
                work_schedule: selected.work_schedule_identity(),
                target: plan.target,
                functions,
            },
            carried,
        ))
    }

    pub struct UnitInstructionLiveness {
        pub instruction: selected_instructions::SelectedInstructionId,
        pub unit_live_in: Vec<RegisterUnitId>,
        pub unit_live_out: Vec<RegisterUnitId>,
    }
    pub struct UnitBlockLiveness {
        pub unit_live_in: Vec<RegisterUnitId>,
        pub instructions: Vec<UnitInstructionLiveness>,
    }
    pub struct UnitFunctionLiveness {
        pub blocks: Vec<UnitBlockLiveness>,
    }
    pub fn compute_function_units_filtered(
        function_index: usize,
        function: &SelectedFunction,
        unit_filter: &[RegisterUnitId],
        needed_blocks: &[bool],
    ) -> Result<UnitFunctionLiveness, LivenessError> {
        let flow =
            flow::FunctionFlow::compute_units_filtered(function_index, function, unit_filter)?;
        let mut filter_mask = 0u64;
        let mut can_use_mask = true;
        for &u in unit_filter {
            if (u.0 as usize) < 64 {
                filter_mask |= 1u64 << u.0;
            } else {
                can_use_mask = false;
            }
        }
        let blocks = function
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                if !needed_blocks.get(index).copied().unwrap_or(false) {
                    return UnitBlockLiveness {
                        unit_live_in: Vec::new(),
                        instructions: Vec::new(),
                    };
                }
                let state = &flow.blocks[index];
                let mask_info = function.blocks.block_unit_masks(index);
                let block_use_mask =
                    can_use_mask && mask_info.is_some_and(|(_, _, _, _, has_high)| !has_high);
                let has_local_unit_use = if block_use_mask {
                    let (implicit_mask, _, _, _, _) = mask_info.unwrap();
                    (implicit_mask & filter_mask) != 0
                } else {
                    function
                        .blocks
                        .block_implicit_units_and_fixed_views(index)
                        .is_some_and(|(units, _)| units.iter().any(|u| unit_filter.contains(u)))
                };
                if state.unit_exit.is_empty() && !has_local_unit_use {
                    return UnitBlockLiveness {
                        unit_live_in: Vec::new(),
                        instructions: Vec::new(),
                    };
                }
                let mut instructions = Vec::with_capacity(block.instructions.len() + 1);
                if block_use_mask {
                    let mut live_mask = state.unit_exit_mask;
                    for instruction in block_instructions(block).rev() {
                        let unit_live_out = flow::mask_to_units(live_mask);
                        for killed in instruction
                            .implicit_defs
                            .iter()
                            .chain(&instruction.clobbers)
                        {
                            let bit = 1u64 << killed.0;
                            if (filter_mask & bit) != 0 {
                                live_mask &= !bit;
                            }
                        }
                        for used in &instruction.implicit_uses {
                            let bit = 1u64 << used.0;
                            if (filter_mask & bit) != 0 {
                                live_mask |= bit;
                            }
                        }
                        instructions.push(UnitInstructionLiveness {
                            instruction: instruction.id,
                            unit_live_in: flow::mask_to_units(live_mask),
                            unit_live_out,
                        });
                    }
                } else {
                    let mut live: BTreeSet<_> = state.unit_exit.iter().copied().collect();
                    for instruction in block_instructions(block).rev() {
                        let unit_live_out = sorted(&live);
                        for killed in instruction
                            .implicit_defs
                            .iter()
                            .chain(&instruction.clobbers)
                        {
                            if unit_filter.contains(killed) {
                                live.remove(killed);
                            }
                        }
                        for used in &instruction.implicit_uses {
                            if unit_filter.contains(used) {
                                live.insert(*used);
                            }
                        }
                        instructions.push(UnitInstructionLiveness {
                            instruction: instruction.id,
                            unit_live_in: sorted(&live),
                            unit_live_out,
                        });
                    }
                }
                instructions.reverse();
                UnitBlockLiveness {
                    unit_live_in: state.unit_entry.clone(),
                    instructions,
                }
            })
            .collect();
        Ok(UnitFunctionLiveness { blocks })
    }
    fn transfer_units(live: &mut BTreeSet<RegisterUnitId>, instruction: &SelectedInstruction) {
        for killed in instruction
            .implicit_defs
            .iter()
            .chain(&instruction.clobbers)
        {
            live.remove(killed);
        }
        live.extend(instruction.implicit_uses.iter().copied());
    }

    pub(crate) fn compute_function(
        function_index: usize,
        function: &SelectedFunction,
    ) -> Result<FunctionLiveness, LivenessError> {
        compute_function_inner(function_index, function, true)
    }

    pub(crate) fn compute_function_for_ranges(
        function_index: usize,
        function: &SelectedFunction,
    ) -> Result<FunctionLiveness, LivenessError> {
        compute_function_inner(function_index, function, false)
    }

    fn compute_function_inner(
        function_index: usize,
        function: &SelectedFunction,
        materialize_instruction_live_sets: bool,
    ) -> Result<FunctionLiveness, LivenessError> {
        reject_unsupported_constraints(function_index, function)?;
        super::edge_values::validate_transports(function_index, function)?;
        let flow = flow::FunctionFlow::compute(function_index, function)?;

        let max_inst_id = function
            .blocks
            .maximum_instruction_id()
            .map_or(0, |id| id.0 as usize);
        let mut positions = vec![u32::MAX; max_inst_id + 1];
        let mut next_position = 0_u32;
        for block in &function.blocks {
            for instruction in block_instructions(block) {
                positions[instruction.id.0 as usize] = next_position;
                next_position =
                    next_position
                        .checked_add(1)
                        .ok_or(LivenessError::NonDensePositions {
                            function: function_index,
                        })?;
            }
        }

        let entry_definitions = function
            .virtual_registers
            .iter()
            .filter(|register| function.is_entry_register(register))
            .map(|register| EntryDefinition {
                virtual_register: register.id,
                class: register.class,
                fixed_view: register.entry_fixed_view,
            })
            .collect();
        let total_operands = function.blocks.total_operand_count().unwrap_or(0);
        let mut operand_positions = arena::Arena::with_capacity(total_operands);
        let mut operand_spans = Vec::with_capacity(next_position as usize);
        for block in &function.blocks {
            for instruction in block_instructions(block) {
                let instruction_position = LivenessPosition(positions[instruction.id.0 as usize]);
                operand_spans.push(
                    operand_positions.insert_many(instruction.operands.iter().map(|operand| {
                        OperandPosition {
                            position: instruction_position,
                            instruction: instruction.id,
                            operand: operand.operand,
                            virtual_register: operand.virtual_register,
                            access: operand.access,
                            class: operand.class,
                            fixed_view: operand.fixed_view,
                            tied_to: operand.tied_to,
                            early_clobber: operand.early_clobber,
                        }
                    })),
                );
            }
        }
        let blocks = function
            .blocks
            .iter()
            .enumerate()
            .map(|(block_index, block)| {
                materialize_block(
                    function_index,
                    block,
                    &positions,
                    &operand_spans,
                    &flow,
                    block_index,
                    materialize_instruction_live_sets,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(FunctionLiveness {
            machine: function.machine,
            entry_definitions,
            operand_positions,
            blocks,
        })
    }

    pub(super) fn reject_unsupported_constraints(
        function_index: usize,
        function: &SelectedFunction,
    ) -> Result<(), LivenessError> {
        let mut tied_edges = Vec::new();
        let mut early_rows = Vec::new();
        for instruction in function
            .blocks
            .iter()
            .enumerate()
            .filter(|&(block_index, _)| function.blocks.block_has_constraint_operands(block_index))
            .flat_map(|(_, block)| block_instructions(block))
        {
            for operand in &instruction.operands {
                if operand.access == RegisterOperandAccess::UseDef {
                    return Err(LivenessError::UnsupportedUseDef {
                        function: function_index,
                        instruction: instruction.id.0,
                        operand: operand.operand,
                    });
                }
            }
            if instruction
                .operands
                .iter()
                .any(|operand| operand.early_clobber)
            {
                let early = instruction
                    .operands
                    .iter()
                    .filter(|operand| operand.early_clobber)
                    .collect::<Vec<_>>();
                if early.len() > 1 {
                    let mut participants = BTreeSet::new();
                    let mut positions = BTreeSet::new();
                    let valid = instruction.operands.iter().all(|operand| {
                        operand.tied_to.is_none()
                            && matches!(
                                (operand.access, operand.early_clobber),
                                (RegisterOperandAccess::Use, false)
                                    | (RegisterOperandAccess::Def, true)
                            )
                            && participants.insert(operand.virtual_register)
                            && positions.insert(operand.operand)
                    }) && instruction
                        .operands
                        .iter()
                        .any(|operand| operand.access == RegisterOperandAccess::Use);
                    if !valid {
                        return Err(LivenessError::UnsupportedEarlyClobber {
                            function: function_index,
                            instruction: instruction.id.0,
                            operand: early[1].operand,
                        });
                    }
                    for definition in early {
                        early_rows.push((
                            instruction.id.0,
                            definition.operand,
                            definition.virtual_register,
                            None,
                            instruction
                                .operands
                                .iter()
                                .filter(|operand| operand.access == RegisterOperandAccess::Use)
                                .map(|operand| (operand.virtual_register, operand.operand))
                                .collect::<Vec<_>>(),
                        ));
                    }
                } else if !early.is_empty() {
                    let definition = early[0];
                    let mut participants = BTreeSet::new();
                    let tied_source = definition
                        .tied_to
                        .and_then(|operand| instruction.operand(operand));
                    let valid_source = tied_source.is_none_or(|source| {
                        source.access == RegisterOperandAccess::Use
                            && source.operand < definition.operand
                            && source.virtual_register != definition.virtual_register
                            && source.class == definition.class
                            && source.tied_to.is_none()
                    });
                    let valid = early.len() == 1
                        && definition.access == RegisterOperandAccess::Def
                        && instruction.operands.len() > 1
                        && valid_source
                        && instruction.operands.iter().all(|operand| {
                            (operand.operand == definition.operand
                                || (tied_source
                                    .is_some_and(|source| source.operand == operand.operand)
                                    && operand.tied_to.is_none())
                                || (operand.tied_to.is_none()
                                    && operand.access == RegisterOperandAccess::Use))
                                && participants.insert(operand.virtual_register)
                        });
                    let unrelated_use_count = instruction
                        .operands
                        .iter()
                        .filter(|operand| {
                            operand.operand != definition.operand
                                && tied_source
                                    .is_none_or(|source| source.operand != operand.operand)
                        })
                        .count();
                    if !valid || tied_source.is_some() && unrelated_use_count == 0 {
                        let operand = early.get(1).copied().unwrap_or(definition).operand;
                        return Err(LivenessError::UnsupportedEarlyClobber {
                            function: function_index,
                            instruction: instruction.id.0,
                            operand,
                        });
                    }
                    early_rows.push((
                        instruction.id.0,
                        definition.operand,
                        definition.virtual_register,
                        tied_source.map(|source| source.virtual_register),
                        instruction
                            .operands
                            .iter()
                            .filter(|operand| {
                                operand.operand != definition.operand
                                    && tied_source
                                        .is_none_or(|source| source.operand != operand.operand)
                            })
                            .map(|operand| (operand.virtual_register, operand.operand))
                            .collect::<Vec<_>>(),
                    ));
                }
            }
            if instruction
                .operands
                .iter()
                .any(|operand| operand.tied_to.is_some())
            {
                for definition in instruction
                    .operands
                    .iter()
                    .filter(|operand| operand.tied_to.is_some())
                {
                    let Some(use_operand) = instruction
                        .operands
                        .iter()
                        .find(|operand| Some(operand.operand) == definition.tied_to)
                    else {
                        return Err(LivenessError::UnsupportedTiedOperand {
                            function: function_index,
                            instruction: instruction.id.0,
                            operand: definition.operand,
                        });
                    };
                    if definition.access != RegisterOperandAccess::Def
                        || use_operand.access != RegisterOperandAccess::Use
                        || definition.operand <= use_operand.operand
                        || definition.virtual_register == use_operand.virtual_register
                        || definition.class != use_operand.class
                        || use_operand.tied_to.is_some()
                    {
                        return Err(LivenessError::UnsupportedTiedOperand {
                            function: function_index,
                            instruction: instruction.id.0,
                            operand: definition.operand,
                        });
                    }
                    tied_edges.push((use_operand.virtual_register, definition.virtual_register));
                }
            }
        }
        for (instruction, def_operand, definition, tied_source, unrelated) in &early_rows {
            let valid_tie = tied_source.is_none_or(|source| {
                let component = tied_component(source, &tied_edges);
                tied_edges.contains(&(source, *definition))
                    && component.contains(definition)
                    && early_rows
                        .iter()
                        .filter(|(_, _, candidate, candidate_source, _)| {
                            candidate_source.is_some() && component.contains(candidate)
                        })
                        .count()
                        == 1
            });
            let untied_definition_is_free = tied_source.is_some()
                || tied_edges
                    .iter()
                    .all(|(left, right)| left != definition && right != definition);
            if !valid_tie
                || !untied_definition_is_free
                || unrelated.iter().any(|(register, _)| {
                    tied_edges
                        .iter()
                        .any(|(left, right)| left == register || right == register)
                })
            {
                return Err(LivenessError::UnsupportedEarlyClobber {
                    function: function_index,
                    instruction: *instruction,
                    operand: unrelated
                        .iter()
                        .find(|(register, _)| {
                            tied_edges
                                .iter()
                                .any(|(left, right)| left == register || right == register)
                        })
                        .map_or(*def_operand, |(_, operand)| *operand),
                });
            }
        }
        Ok(())
    }

    fn tied_component(
        seed: VirtualRegisterId,
        edges: &[(VirtualRegisterId, VirtualRegisterId)],
    ) -> BTreeSet<VirtualRegisterId> {
        let mut component = BTreeSet::from([seed]);
        loop {
            let previous_len = component.len();
            for (left, right) in edges {
                if component.contains(left) || component.contains(right) {
                    component.insert(*left);
                    component.insert(*right);
                }
            }
            if component.len() == previous_len {
                return component;
            }
        }
    }

    fn materialize_block(
        function_index: usize,
        block: &SelectedBlock,
        positions: &[u32],
        operand_spans: &[arena::HandleSpan<OperandPosition>],
        flow: &flow::FunctionFlow,
        block_index: usize,
        materialize_instruction_live_sets: bool,
    ) -> Result<BlockLiveness, LivenessError> {
        let state = &flow.blocks[block_index];
        let instructions = if materialize_instruction_live_sets {
            let mut current_virtual_live = state.virtual_exit.clone();
            let mut unit_live: BTreeSet<_> = state.unit_exit.iter().copied().collect();
            let mut current_unit_live = state.unit_exit.clone();
            let mut instructions = Vec::with_capacity(block.instructions.len() + 1);
            for instruction in block_instructions(block).rev() {
                let virtual_live_out = current_virtual_live.clone();
                let unit_live_out = current_unit_live.clone();
                let (uses, defs) = virtual_uses_defs_vec(instruction);
                for definition in &defs {
                    if let Ok(pos) = current_virtual_live.binary_search(definition) {
                        current_virtual_live.remove(pos);
                    }
                }
                for used in &uses {
                    if let Err(pos) = current_virtual_live.binary_search(used) {
                        current_virtual_live.insert(pos, *used);
                    }
                }
                let virtual_live_in = current_virtual_live.clone();
                if !instruction.implicit_defs.is_empty()
                    || !instruction.clobbers.is_empty()
                    || !instruction.implicit_uses.is_empty()
                {
                    transfer_units(&mut unit_live, instruction);
                    current_unit_live = sorted(&unit_live);
                }
                let pos = positions[instruction.id.0 as usize];
                instructions.push(InstructionLiveness {
                    position: LivenessPosition(pos),
                    instruction: instruction.id,
                    operands: operand_spans[pos as usize],
                    virtual_uses: uses,
                    virtual_defs: defs,
                    virtual_live_in,
                    virtual_live_out,
                    unit_uses: instruction.implicit_uses.clone(),
                    unit_defs: instruction.implicit_defs.clone(),
                    unit_clobbers: instruction.clobbers.clone(),
                    unit_live_in: current_unit_live.clone(),
                    unit_live_out,
                });
            }
            instructions.reverse();
            instructions
        } else {
            block_instructions(block)
                .map(|instruction| {
                    let pos = positions[instruction.id.0 as usize];
                    InstructionLiveness {
                        position: LivenessPosition(pos),
                        instruction: instruction.id,
                        operands: operand_spans[pos as usize],
                        virtual_uses: Vec::new(),
                        virtual_defs: Vec::new(),
                        virtual_live_in: Vec::new(),
                        virtual_live_out: Vec::new(),
                        unit_uses: instruction.implicit_uses.clone(),
                        unit_defs: instruction.implicit_defs.clone(),
                        unit_clobbers: instruction.clobbers.clone(),
                        unit_live_in: Vec::new(),
                        unit_live_out: Vec::new(),
                    }
                })
                .collect()
        };
        let terminator = selected_instructions::terminator_instruction(&block.terminator);
        let successors = control::successors(&block.terminator)
            .enumerate()
            .map(|(ordinal, successor)| {
                let edge = &flow.edges[block_index][ordinal];
                let target_entry = &flow.blocks[edge.target].virtual_entry;
                let virtual_live = if edge.substitutions.is_empty()
                    && target_entry.iter().all(|&reg| flow.is_valid_register(reg))
                {
                    target_entry.clone()
                } else {
                    let mut virtual_live = target_entry
                        .iter()
                        .map(|register| flow.incoming_argument(function_index, edge, *register))
                        .collect::<Result<Vec<_>, _>>()?;
                    virtual_live.sort_unstable();
                    virtual_live.dedup();
                    virtual_live
                };
                Ok(SuccessorLiveness {
                    terminator: terminator.id,
                    polarity_ordinal: ordinal as u8,
                    psi_edge: successor.psi_edge,
                    target: successor.block,
                    virtual_live,
                    unit_live: flow.blocks[edge.target].unit_entry.clone(),
                })
            })
            .collect::<Result<Vec<_>, LivenessError>>()?;
        Ok(BlockLiveness {
            block: block.id,
            source_block: block.source_block(),
            virtual_live_in: state.virtual_entry.clone(),
            virtual_live_out: state.virtual_exit.clone(),
            unit_live_in: state.unit_entry.clone(),
            unit_live_out: state.unit_exit.clone(),
            instructions,
            successors,
        })
    }

    fn virtual_uses_defs_vec(
        instruction: &SelectedInstruction,
    ) -> (Vec<VirtualRegisterId>, Vec<VirtualRegisterId>) {
        let mut uses = Vec::new();
        let mut defs = Vec::new();
        for operand in &instruction.operands {
            match operand.access {
                RegisterOperandAccess::Use => {
                    uses.push(operand.virtual_register);
                }
                RegisterOperandAccess::Def => {
                    defs.push(operand.virtual_register);
                }
                RegisterOperandAccess::UseDef => {
                    uses.push(operand.virtual_register);
                    defs.push(operand.virtual_register);
                }
            }
        }
        if uses.len() > 1 {
            uses.sort_unstable();
            uses.dedup();
        }
        if defs.len() > 1 {
            defs.sort_unstable();
            defs.dedup();
        }
        (uses, defs)
    }

    fn block_instructions(
        block: &SelectedBlock,
    ) -> impl DoubleEndedIterator<Item = &SelectedInstruction> {
        block.instructions.iter().chain(std::iter::once(
            selected_instructions::terminator_instruction(&block.terminator),
        ))
    }

    fn sorted<T: Copy + Ord>(values: &BTreeSet<T>) -> Vec<T> {
        values.iter().copied().collect()
    }
}
pub(crate) mod edge_values {
    use super::LivenessError;

    #[cfg(test)]
    mod tests {
        use super::*;
        use optimization_unit::WorkSettlement;
        use selected_instructions::{
            SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance,
        };
        use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId};

        fn function() -> SelectedFunction {
            let source = BlockId::new(1).unwrap();
            let operation = OperationId::new(2).unwrap();
            let returned = EdgeId::new(3).unwrap();
            let instruction = |id, kind| SelectedInstruction {
                id: SelectedInstructionId(id),
                kind,
                constraint: register_model::RegisterConstraintKey {
                    family: register_model::RegisterConstraintFamily::Instruction,
                    variant: 0,
                },
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance {
                    operations: vec![operation],
                    ..Default::default()
                },
            };
            let successor = |block| SelectedSuccessor {
                role: SelectedSuccessorRole::OperationContinuation { operation },
                psi_edge: None,
                block: SelectedBlockId(block),
                source_target: source,
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                structural_case: None,
                work: Vec::new(),
            };
            SelectedFunction {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                nominal_cleanup_subject: None,
                provenance: target_operations::TerminalPsiProvenance {
                    operations: vec![operation],
                    edges: vec![returned],
                },
                structural: None,
                local_storage_slots: Vec::new(),
                outgoing_arguments: Vec::new(),
                calls: Vec::new(),
                normalized_foreign_calls: Vec::new(),
                memory_accesses: Vec::new(),
                boundary_settlements: Vec::new(),
                entry_block: SelectedBlockId(0),
                virtual_registers: Vec::new(),
                blocks: vec![
                    SelectedBlock {
                        id: SelectedBlockId(0),
                        origin: SelectedBlockOrigin::Source(source),
                        instructions: Vec::new(),
                        terminator: SelectedTerminator::Jump {
                            instruction: instruction(0, SelectedInstructionKind::Jump),
                            successor: successor(1),
                        },
                    },
                    SelectedBlock {
                        id: SelectedBlockId(1),
                        origin: SelectedBlockOrigin::Operation { source, operation },
                        instructions: Vec::new(),
                        terminator: SelectedTerminator::ConditionalBranch {
                            instruction: instruction(
                                1,
                                SelectedInstructionKind::ConditionalBranchNonZero,
                            ),
                            when_nonzero: successor(1),
                            when_zero: successor(2),
                        },
                    },
                    SelectedBlock {
                        id: SelectedBlockId(2),
                        origin: SelectedBlockOrigin::Operation { source, operation },
                        instructions: Vec::new(),
                        terminator: SelectedTerminator::Return {
                            instruction: instruction(2, SelectedInstructionKind::ReturnUnit),
                            psi_return_edge: returned,
                        },
                    },
                ]
                .into(),
            }
        }

        #[test]
        fn operation_trap_has_no_successor_and_rejects_forged_lineage() {
            let mut original = function();
            let operation = OperationId::new(2).unwrap();
            let mut instruction =
                selected_instructions::terminator_instruction(&original.blocks[2].terminator)
                    .clone();
            instruction.kind = SelectedInstructionKind::Crash;
            original.blocks[2].terminator = SelectedTerminator::OperationTrap {
                instruction,
                operation,
            };
            validate_transports(0, &original).unwrap();
            let liveness = super::super::compute::compute_function(0, &original).unwrap();
            assert!(liveness.blocks[2].successors.is_empty());
            for fault in 0..5 {
                let mut changed = original.clone();
                let SelectedTerminator::OperationTrap {
                    instruction,
                    operation,
                } = &mut changed.blocks[2].terminator
                else {
                    panic!("trap");
                };
                match fault {
                    0 => *operation = OperationId::new(9).unwrap(),
                    1 => instruction.provenance.edges.push(EdgeId::new(3).unwrap()),
                    2 => instruction.provenance.operations.clear(),
                    3 => instruction.kind = SelectedInstructionKind::ReturnUnit,
                    _ => instruction.provenance.work.push(WorkSettlement {
                        site: optimization_unit::PsiProvenance::Operation(*operation),
                        units: 1,
                    }),
                }
                assert!(validate_transports(0, &changed).is_err());
            }
        }

        #[test]
        fn operation_loop_liveness_retains_physical_edges_without_semantic_transfers() {
            let function = function();
            validate_transports(0, &function).unwrap();
            let liveness = super::super::compute::compute_function(0, &function).unwrap();
            assert_eq!(liveness.blocks.len(), 3);
            assert_eq!(
                liveness
                    .blocks
                    .iter()
                    .map(|block| block.successors.len())
                    .sum::<usize>(),
                3
            );
            assert!(
                liveness
                    .blocks
                    .iter()
                    .flat_map(|block| &block.successors)
                    .all(|edge| edge.psi_edge.is_none())
            );
        }

        #[test]
        fn operation_control_rejects_semantic_authority_and_changed_owners() {
            let original = function();
            let changed = |edit: &dyn Fn(&mut SelectedFunction)| {
                let mut candidate = original.clone();
                edit(&mut candidate);
                assert!(validate_transports(0, &candidate).is_err());
            };
            changed(&|function| {
                selected_instructions::terminator_successors_mut(
                    &mut function.blocks[0].terminator,
                )[0]
                .psi_edge = EdgeId::new(3)
            });
            changed(&|function| {
                selected_instructions::terminator_successors_mut(
                    &mut function.blocks[0].terminator,
                )[0]
                .role = SelectedSuccessorRole::Semantic
            });
            changed(&|function| {
                selected_instructions::terminator_instruction_mut(
                    &mut function.blocks[0].terminator,
                )
                .provenance
                .operations
                .clear()
            });
            changed(&|function| {
                selected_instructions::terminator_instruction_mut(
                    &mut function.blocks[0].terminator,
                )
                .provenance
                .edges
                .push(EdgeId::new(3).unwrap())
            });
            changed(&|function| {
                selected_instructions::terminator_successors_mut(&mut function.blocks[0].terminator)
                    [0]
                .work
                .push(WorkSettlement {
                    site: optimization_unit::PsiProvenance::Operation(OperationId::new(2).unwrap()),
                    units: 1,
                })
            });
            changed(&|function| {
                function.blocks[1].origin = SelectedBlockOrigin::Operation {
                    source: BlockId::new(1).unwrap(),
                    operation: OperationId::new(9).unwrap(),
                }
            });
            changed(&|function| {
                function.blocks[1].origin = SelectedBlockOrigin::Source(BlockId::new(1).unwrap())
            });
            changed(&|function| {
                selected_instructions::terminator_successors_mut(
                    &mut function.blocks[1].terminator,
                )[0]
                .block = SelectedBlockId(0)
            });
            changed(&|function| function.entry_block = SelectedBlockId(1));
        }
    }
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedSuccessor,
        SelectedSuccessorRole, SelectedTerminator, SelectedValueTransport, VirtualRegister,
        VirtualRegisterId, VirtualRegisterOrigin, terminator_successors,
    };
    mod case_payload {
        use super::FunctionIndex;
        use register_model::RegisterOperandAccess;
        use selected_instructions::{
            SelectedBlockOrigin, SelectedInstructionKind, SelectedSuccessor, VirtualRegister,
            VirtualRegisterOrigin,
        };
        use semantic_vocabulary::{IntegerSign, ScalarType};

        pub(super) fn observed_source<'a>(
            index: &FunctionIndex<'a>,
            successor: &SelectedSuccessor,
            value: &'a VirtualRegister,
        ) -> Option<&'a VirtualRegister> {
            let psi_edge = successor.psi_edge?;
            if matches!(
                value.origin,
                VirtualRegisterOrigin::StructuralObservation { .. }
            ) {
                return Some(value);
            }
            let VirtualRegisterOrigin::AbiTransport {
                instruction,
                place,
                byte_offset,
            } = value.origin
            else {
                return None;
            };
            let ScalarType::Integer(integer) = value.scalar_type else {
                return None;
            };
            if integer.sign() != IntegerSign::Signed {
                return None;
            }
            let kind = match integer.bits() {
                8 => SelectedInstructionKind::SignExtendI8,
                16 => SelectedInstructionKind::SignExtendI16,
                32 => SelectedInstructionKind::SignExtendI32,
                _ => return None,
            };
            let (owner, normalization, position) = index.unique_instruction(instruction)?;
            if normalization.kind != kind
                || !matches!(owner.origin, SelectedBlockOrigin::EdgeTransfer { edge, target }
                    if edge == psi_edge && target == successor.source_target)
                || normalization.provenance.edges != [psi_edge]
                || !normalization.provenance.operations.is_empty()
                || !normalization.provenance.work.is_empty()
            {
                return None;
            }
            let [input, output] = normalization.operands.as_slice() else {
                return None;
            };
            if input.access != RegisterOperandAccess::Use
                || output.access != RegisterOperandAccess::Def
                || output.virtual_register != value.id
                || input.virtual_register == value.id
                || input.class != value.class
                || output.class != value.class
            {
                return None;
            }
            let observed = index.unique_register(input.virtual_register)?;
            if observed.scalar_type != value.scalar_type
                || observed.class != value.class
                || observed.definition_site.is_some()
            {
                return None;
            }
            let VirtualRegisterOrigin::StructuralObservation {
                instruction: load_id,
                place: load_place,
                byte_offset: load_offset,
            } = observed.origin
            else {
                return None;
            };
            if load_place != place || load_offset != byte_offset {
                return None;
            }
            let (load_owner, load, load_position) = index.unique_instruction(load_id)?;
            let [address, loaded] = load.operands.as_slice() else {
                return None;
            };
            if !std::ptr::eq(load_owner, owner)
                || load_position >= position
                || load.provenance.edges != [psi_edge]
                || !matches!(
                    (integer.bits(), load.kind),
                    (8, SelectedInstructionKind::Load8 { .. })
                        | (16, SelectedInstructionKind::Load16 { .. })
                        | (32, SelectedInstructionKind::Load32 { .. })
                )
                || address.access != RegisterOperandAccess::Use
                || loaded.access != RegisterOperandAccess::Def
                || loaded.virtual_register != observed.id
                || loaded.class != observed.class
            {
                return None;
            }
            Some(observed)
        }
    }

    pub(crate) struct FunctionIndex<'function> {
        function: &'function SelectedFunction,
        ids_are_positions: bool,
        parameters: Vec<(SelectedBlockId, usize)>,
        instructions:
            std::sync::OnceLock<Vec<(selected_instructions::SelectedInstructionId, usize, usize)>>,
    }

    impl<'function> FunctionIndex<'function> {
        pub fn new(function: &'function SelectedFunction) -> Self {
            let mut parameters = function
                .virtual_registers
                .iter()
                .enumerate()
                .filter_map(|(position, register)| match register.origin {
                    VirtualRegisterOrigin::BlockParameter { block, .. } => Some((block, position)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            parameters.sort_unstable();
            Self {
                function,
                ids_are_positions: function.ids_are_positions(),
                parameters,
                instructions: std::sync::OnceLock::new(),
            }
        }

        fn unique_instruction(
            &self,
            id: selected_instructions::SelectedInstructionId,
        ) -> Option<(
            &'function SelectedBlock,
            &'function selected_instructions::SelectedInstruction,
            usize,
        )> {
            let instructions = self.instructions.get_or_init(|| {
                let mut list =
                    self.function
                        .blocks
                        .iter()
                        .enumerate()
                        .flat_map(|(block, row)| {
                            row.instructions.iter().enumerate().map(
                                move |(position, instruction)| (instruction.id, block, position),
                            )
                        })
                        .collect::<Vec<_>>();
                list.sort_unstable();
                list
            });
            let start = instructions.partition_point(|(other, _, _)| *other < id);
            let [(first, block, position), rest @ ..] = instructions.get(start..)? else {
                return None;
            };
            if *first != id || rest.first().is_some_and(|(next, _, _)| *next == id) {
                return None;
            }
            let block = self.function.blocks.get(*block)?;
            Some((block, block.instructions.get(*position)?, *position))
        }

        pub(crate) fn unique_register(
            &self,
            id: VirtualRegisterId,
        ) -> Option<&'function VirtualRegister> {
            self.ids_are_positions
                .then(|| self.function.virtual_register(id))
                .flatten()
        }

        pub(crate) fn parameters_of(
            &self,
            block: SelectedBlockId,
        ) -> impl Iterator<Item = &'function VirtualRegister> + '_ {
            let start = self.parameters.partition_point(|(other, _)| *other < block);
            self.parameters[start..]
                .iter()
                .take_while(move |(other, _)| *other == block)
                .filter_map(|(_, position)| self.function.virtual_registers.get(*position))
        }

        pub(crate) fn block_index(&self, id: SelectedBlockId) -> Option<usize> {
            self.ids_are_positions
                .then(|| self.function.block_index(id))
                .flatten()
        }

        pub(crate) fn block(&self, id: SelectedBlockId) -> Option<&'function SelectedBlock> {
            self.block_index(id)
                .map(|index| &self.function.blocks[index])
        }
    }

    pub(crate) fn has_edge_use(function: &SelectedFunction, register: VirtualRegisterId) -> bool {
        function.blocks.iter().any(|block| {
            let edges = match &block.terminator {
                SelectedTerminator::Jump { successor, .. } => vec![successor],
                SelectedTerminator::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                    ..
                } => vec![when_nonzero, when_zero],
                selected_instructions::conditional_branch_less_pair_pattern!(when_less,when_not_less, ..) => vec![when_less, when_not_less],
                selected_instructions::terminator_no_successor_pattern!(..) => {
                    Vec::new()
                }
            };
            edges.iter().any(|edge| {
                edge.structural_case.as_ref().is_some_and(|case| case.payloads.iter().any(|payload| matches!(payload.transport,
                    selected_instructions::SelectedCasePayloadTransport::Registers { argument, .. } if argument == register)))
                || edge.bindings.iter().any(|binding| {
                    matches!(binding.transport,
                SelectedValueTransport::Registers {argument,..} if argument == register)
                })
            })
        })
    }

    pub(crate) fn validate_transports(
        function_index: usize,
        function: &SelectedFunction,
    ) -> Result<(), LivenessError> {
        let mismatch = || LivenessError::FunctionMismatch {
            function: function_index,
        };
        if !function.blocks.iter().any(|block| {
            matches!(block.origin, SelectedBlockOrigin::Operation { .. })
                || matches!(block.terminator, SelectedTerminator::OperationTrap { .. })
                || matches!(
                    block.terminator,
                    SelectedTerminator::OperationInvokeNoNormal { .. }
                )
                || !terminator_successors(&block.terminator).is_empty()
        }) {
            return Ok(());
        }
        let index = FunctionIndex::new(function);
        for block in &function.blocks {
            if let SelectedBlockOrigin::Operation { operation, .. } = block.origin
                && (!function.provenance.operations.contains(&operation)
                    || block.id == function.entry_block)
            {
                return Err(mismatch());
            }
            if let SelectedTerminator::OperationInvokeNoNormal {
                instruction,
                operation,
            } = &block.terminator
            {
                let mut calls = function
                    .calls
                    .iter()
                    .filter(|row| row.instruction == instruction.id);
                let (Some(call), None) = (calls.next(), calls.next()) else {
                    return Err(mismatch());
                };
                if !matches!(block.origin, SelectedBlockOrigin::Operation { operation: owner, .. } if owner == *operation)
                    || !matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::InvokeNoNormal { .. }
                    )
                    || instruction.kind.call_target()
                        != Some(selected_instructions::SelectedCallTarget::from_native(
                            call.call.target,
                        ))
                    || call.owner
                        != (target_operations::CallSiteOwner::Installation {
                            operation: *operation,
                            role: target_operations::InstallationCallRole::Work,
                        })
                    || instruction.provenance.operations.as_slice() != [*operation]
                    || !instruction.provenance.edges.is_empty()
                    || !instruction.provenance.work.is_empty()
                {
                    return Err(mismatch());
                }
            }
            if let SelectedTerminator::OperationTrap {
                instruction,
                operation,
            } = &block.terminator
                && (!matches!(block.origin, SelectedBlockOrigin::Operation { operation: owner, .. } if owner == *operation)
                    || !matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::Crash
                            | selected_instructions::SelectedInstructionKind::PrivilegedCrash
                    )
                    || instruction.provenance.operations.as_slice() != [*operation]
                    || !instruction.provenance.edges.is_empty()
                    || !instruction.provenance.work.is_empty())
            {
                return Err(mismatch());
            }
            let edges = terminator_successors(&block.terminator);
            for edge in edges {
                if edge.psi_edge.is_none()
                    != matches!(
                        edge.role,
                        SelectedSuccessorRole::OperationContinuation { .. }
                    )
                {
                    return Err(mismatch());
                }
                match (block.origin, edge.role) {
                    (
                        SelectedBlockOrigin::Source(_)
                        | SelectedBlockOrigin::CaseDispatch { .. }
                        | SelectedBlockOrigin::Operation { .. },
                        SelectedSuccessorRole::Semantic,
                    ) => {}
                    (
                        SelectedBlockOrigin::Source(_)
                        | SelectedBlockOrigin::CaseDispatch { .. }
                        | SelectedBlockOrigin::Operation { .. },
                        SelectedSuccessorRole::CaseDispatchContinuation,
                    ) if edge.work.is_empty()
                        && edge.bindings.is_empty()
                        && edge.structural_bindings.is_empty()
                        && edge.structural_case.is_none()
                        && matches!(
                            block.terminator,
                            SelectedTerminator::ConditionalBranch { .. }
                        ) => {}
                    (
                        SelectedBlockOrigin::Source(_) | SelectedBlockOrigin::Operation { .. },
                        SelectedSuccessorRole::OperationContinuation { operation },
                    ) if edge.source_target == block.source_block()
                        && edge.work.is_empty()
                        && edge.bindings.is_empty()
                        && edge.structural_bindings.is_empty()
                        && edge.structural_case.is_none()
                        && selected_instructions::terminator_instruction(&block.terminator)
                            .provenance
                            .operations
                            .contains(&operation)
                        && selected_instructions::terminator_instruction(&block.terminator)
                            .provenance
                            .edges
                            .is_empty()
                        && selected_instructions::terminator_instruction(&block.terminator)
                            .provenance
                            .work
                            .is_empty() => {}
                    (
                        SelectedBlockOrigin::EdgeTransfer {
                            edge: owner,
                            target,
                        },
                        SelectedSuccessorRole::EdgeTransferContinuation,
                    ) if Some(owner) == edge.psi_edge
                        && target == edge.source_target
                        && edge.work.is_empty()
                        && matches!(block.terminator, SelectedTerminator::Jump { .. }) => {}
                    _ => return Err(mismatch()),
                }
                let destination = index
                    .block(edge.block)
                    .filter(|target| target.source_block() == edge.source_target)
                    .ok_or_else(mismatch)?;
                match destination.origin {
                    SelectedBlockOrigin::Source(_)
                        if !matches!(
                            edge.role,
                            SelectedSuccessorRole::OperationContinuation { .. }
                        ) => {}
                    SelectedBlockOrigin::Operation { source, operation }
                        if edge.role
                            == (SelectedSuccessorRole::OperationContinuation { operation })
                            && source == edge.source_target => {}
                    SelectedBlockOrigin::CaseDispatch { source, .. }
                        if edge.role == SelectedSuccessorRole::CaseDispatchContinuation
                            && source == edge.source_target => {}
                    SelectedBlockOrigin::EdgeTransfer {
                        edge: owner,
                        target,
                    } if edge.role == SelectedSuccessorRole::Semantic
                        && Some(owner) == edge.psi_edge
                        && target == edge.source_target
                        && edge
                            .bindings
                            .iter()
                            .all(|binding| binding.transport == SelectedValueTransport::Unused) => {
                    }
                    _ => return Err(mismatch()),
                }
                validate_case_transport(function_index, &index, edge, destination)?;
                for binding in &edge.bindings {
                    if edge
                        .bindings
                        .iter()
                        .filter(|other| other.semantic.parameter == binding.semantic.parameter)
                        .count()
                        != 1
                    {
                        return Err(mismatch());
                    }
                    let mut destinations = index.parameters_of(edge.block).filter(|register| {
                        matches!(register.origin,
                    VirtualRegisterOrigin::BlockParameter {source_value,..}
                        if source_value == binding.semantic.parameter)
                    });
                    let first_dest = destinations.next();
                    match binding.transport {
                        SelectedValueTransport::Unused if first_dest.is_none() => {}
                        SelectedValueTransport::Registers { parameter, .. }
                            if first_dest.is_some_and(|d| d.id == parameter)
                                && destinations.next().is_none() =>
                        {
                            incoming_argument(function_index, &index, edge, parameter)?;
                        }
                        _ => return Err(mismatch()),
                    }
                }
                for destination in index.parameters_of(edge.block) {
                    incoming_argument(function_index, &index, edge, destination.id)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn incoming_argument(
        function_index: usize,
        index: &FunctionIndex<'_>,
        successor: &SelectedSuccessor,
        destination: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, LivenessError> {
        let mismatch = || LivenessError::FunctionMismatch {
            function: function_index,
        };
        let destination_register = index.unique_register(destination).ok_or_else(mismatch)?;
        let VirtualRegisterOrigin::BlockParameter {
            source_value,
            block,
            parameter_index,
        } = destination_register.origin
        else {
            return Ok(destination);
        };
        if block != successor.block {
            return Ok(destination);
        }
        if destination_register.definition_site
            != Some(optimization_unit::ValueDefinitionSite::BlockParameter {
                block: successor.source_target,
                position: u32::try_from(parameter_index).map_err(|_| mismatch())?,
            })
        {
            return Err(mismatch());
        }
        if let Some(case) = &successor.structural_case {
            let mut payloads = case
                .payloads
                .iter()
                .filter(|payload| payload.semantic.parameter.value == source_value);
            if let Some(payload) = payloads.next() {
                if payloads.next().is_some()
                    || successor
                        .bindings
                        .iter()
                        .any(|binding| binding.semantic.parameter == source_value)
                    || payload.semantic.parameter.scalar_type != destination_register.scalar_type
                    || Some(payload.semantic.parameter.definition_site)
                        != destination_register.definition_site
                {
                    return Err(mismatch());
                }
                let selected_instructions::SelectedCasePayloadTransport::Registers {
                    argument,
                    parameter,
                } = payload.transport
                else {
                    return Err(mismatch());
                };
                if parameter != destination
                    || successor.role != SelectedSuccessorRole::EdgeTransferContinuation
                {
                    return Err(mismatch());
                }
                let source = index.unique_register(argument).ok_or_else(mismatch)?;
                if source.scalar_type != destination_register.scalar_type
                    || source.class != destination_register.class
                    || source.definition_site.is_some()
                {
                    return Err(mismatch());
                }
                let observed =
                    case_payload::observed_source(index, successor, source).ok_or_else(mismatch)?;
                if !matches!(observed.origin, VirtualRegisterOrigin::StructuralObservation { place, byte_offset, .. }
                    if case.source.place() == Some(place) && byte_offset == payload.semantic.field_byte_offset)
                {
                    return Err(mismatch());
                }
                return Ok(argument);
            }
        }
        let mut bindings = successor
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| binding.semantic.parameter == source_value);
        let (binding_position, binding) = bindings.next().ok_or_else(mismatch)?;
        if bindings.next().is_some()
            || binding_position != parameter_index
            || binding.semantic.scalar_type != destination_register.scalar_type
        {
            return Err(mismatch());
        }
        let SelectedValueTransport::Registers {
            argument,
            parameter,
        } = binding.transport
        else {
            return Err(mismatch());
        };
        if parameter != destination {
            return Err(mismatch());
        }
        let source = index.unique_register(argument).ok_or_else(mismatch)?;
        let value = match source.origin {
            VirtualRegisterOrigin::StructuralParameter { .. }
            | VirtualRegisterOrigin::SpillAddress { .. }
            | VirtualRegisterOrigin::StructuralObservation { .. }
            | VirtualRegisterOrigin::ScalarAbiAddress { .. }
            | VirtualRegisterOrigin::InstructionScratch { .. }
            | VirtualRegisterOrigin::AbiTransport { .. } => return Err(mismatch()),
            VirtualRegisterOrigin::EntryParameter { source_value, .. }
            | VirtualRegisterOrigin::BlockParameter { source_value, .. }
            | VirtualRegisterOrigin::InstructionResult { source_value, .. } => source_value,
        };
        if value != binding.semantic.argument
            || source.scalar_type != binding.semantic.scalar_type
            || source.class != destination_register.class
        {
            return Err(mismatch());
        }
        Ok(argument)
    }

    fn validate_case_transport(
        function_index: usize,
        index: &FunctionIndex<'_>,
        edge: &SelectedSuccessor,
        destination: &SelectedBlock,
    ) -> Result<(), LivenessError> {
        use selected_instructions::SelectedCasePayloadTransport;
        let mismatch = || LivenessError::FunctionMismatch {
            function: function_index,
        };
        let Some(case) = &edge.structural_case else {
            return Ok(());
        };
        if case.source.place().is_none()
            || case.case_tag < 0
            || (edge.role == SelectedSuccessorRole::EdgeTransferContinuation
                && (!case.cleanup_actions.is_empty()))
        {
            return Err(mismatch());
        }
        if matches!(destination.origin, SelectedBlockOrigin::EdgeTransfer { .. }) {
            let SelectedTerminator::Jump {
                successor: continuation,
                ..
            } = &destination.terminator
            else {
                return Err(mismatch());
            };
            let Some(next_case) = &continuation.structural_case else {
                return Err(mismatch());
            };
            if next_case.source != case.source
                || next_case.case != case.case
                || next_case.case_tag != case.case_tag
                || !next_case.cleanup_actions.is_empty()
                || next_case.payloads.len() != case.payloads.len()
                || next_case
                    .payloads
                    .iter()
                    .zip(&case.payloads)
                    .any(|(next, semantic)| next.semantic != semantic.semantic)
            {
                return Err(mismatch());
            }
        }
        for payload in &case.payloads {
            if case
                .payloads
                .iter()
                .filter(|other| other.semantic.parameter.value == payload.semantic.parameter.value)
                .count()
                != 1
                || edge
                    .bindings
                    .iter()
                    .any(|binding| binding.semantic.parameter == payload.semantic.parameter.value)
            {
                return Err(mismatch());
            }
            let destinations = index
                .parameters_of(edge.block)
                .filter(|register| {
                    matches!(register.origin, VirtualRegisterOrigin::BlockParameter { source_value, .. }
                    if source_value == payload.semantic.parameter.value)
                })
                .collect::<Vec<_>>();
            match payload.transport {
                SelectedCasePayloadTransport::Unused if destinations.is_empty() => {}
                SelectedCasePayloadTransport::Registers { parameter, .. }
                    if edge.role == SelectedSuccessorRole::EdgeTransferContinuation
                        && destinations.len() == 1
                        && destinations[0].id == parameter =>
                {
                    incoming_argument(function_index, index, edge, parameter)?;
                }
                _ => return Err(mismatch()),
            }
        }
        Ok(())
    }
}
pub(crate) mod validate {
    mod receipt {

        use crate::liveness::{LivenessValidationReceipt, ValidatedLiveness};
        use selected_instructions::{
            LivenessPlan, liveness_function_digest, liveness_identity_from_digests,
        };

        pub(crate) fn admit_validated_liveness(
            plan: LivenessPlan,
            carried: &[bool],
            previous_digests: &[[u8; 32]],
        ) -> ValidatedLiveness {
            let function_digests =
                crate::carried_digests(&plan.functions, carried, previous_digests, |function| {
                    liveness_function_digest(function)
                });
            let receipt = LivenessValidationReceipt {
                identity: liveness_identity_from_digests(&plan, &function_digests),
                selected: plan.selected,
                optimization_unit: plan.optimization_unit,
                work_schedule: plan.work_schedule,
            };
            ValidatedLiveness {
                plan: plan.into(),
                receipt,
                function_digests,
            }
        }
    }
    pub(crate) use receipt::admit_validated_liveness;
}

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{LivenessIdentity, LivenessPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::WorkScheduleIdentity;

pub fn analyze_liveness<S: SelectedProgramView>(
    selected: &S,
) -> Result<ValidatedLiveness, LivenessError> {
    let plan = compute::compute_terminal_liveness(selected)?;
    Ok(validate::admit_validated_liveness(plan, &[], &[]))
}

pub fn analyze_liveness_reusing(
    previous: &impl SelectedProgramView,
    previous_liveness: &ValidatedLiveness,
    selected: &impl SelectedProgramView,
) -> Result<ValidatedLiveness, LivenessError> {
    let (plan, carried) =
        compute::compute_terminal_liveness_reusing(previous, previous_liveness, selected)?;
    Ok(validate::admit_validated_liveness(
        plan,
        &carried,
        &previous_liveness.function_digests,
    ))
}

mod staging {

    mod compute {
        use crate::{ValidatedLiveness, analyze_liveness};

        use crate::SelectedOptimizationInput;

        use super::OptimizedLivenessCustodyError;

        pub(super) fn compute_liveness(
            selected: &SelectedOptimizationInput,
        ) -> Result<ValidatedLiveness, OptimizedLivenessCustodyError> {
            analyze_liveness(selected.selected()).map_err(OptimizedLivenessCustodyError::Analysis)
        }
    }
    mod custody {
        use crate::LivenessValidationReceipt;

        use super::StagedOptimizedLivenessCustodyReceipt;

        pub(super) fn liveness_custody_receipt(
            core: selected_instructions::StagedOptimizedCustodyCore,
            selected: selected_instructions::SelectedInstructionPlanIdentity,
            validation: LivenessValidationReceipt,
        ) -> StagedOptimizedLivenessCustodyReceipt {
            StagedOptimizedLivenessCustodyReceipt {
                core,
                selected,
                liveness: validation.identity(),
            }
        }
    }

    use crate::SelectedOptimizationInput;
    use crate::{LivenessError, ValidatedLiveness};
    use optimization_core::{
        OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
        OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
        PrePhysicalOptimizationManifestIdentity,
    };
    use register_environment::ValidatedTargetRegisterEnvironment;
    use selected_instructions::SelectedProgram;
    use selected_instructions::StagedOptimizedCustodyCore;
    use selected_instructions::{LivenessIdentity, SelectedInstructionPlanIdentity};
    use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
    use terminal_psi::TerminalPsiIdentity;

    pub fn stage_optimized_liveness(
        selected: SelectedOptimizationInput,
    ) -> Result<StagedOptimizedLiveness, OptimizedLivenessCustodyError> {
        let liveness = compute::compute_liveness(&selected)?;
        let custody = custody::liveness_custody_receipt(
            selected.custody,
            selected.program.identity,
            liveness.receipt(),
        );
        Ok(StagedOptimizedLiveness {
            selected,
            liveness,
            custody,
        })
    }

    #[derive(Debug)]
    pub struct StagedOptimizedLiveness {
        selected: SelectedOptimizationInput,
        liveness: ValidatedLiveness,
        custody: StagedOptimizedLivenessCustodyReceipt,
    }

    impl StagedOptimizedLiveness {
        staged_producer_accessors!(selected; selected, register_environment, selections, budget_per_pass);

        pub const fn liveness(&self) -> &ValidatedLiveness {
            &self.liveness
        }

        receipt_accessor_rows!(ref StagedOptimizedLiveness {
            custody: StagedOptimizedLivenessCustodyReceipt,
        });
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StagedOptimizedLivenessCustodyReceipt {
        core: StagedOptimizedCustodyCore,
        selected: SelectedInstructionPlanIdentity,
        liveness: LivenessIdentity,
    }

    impl StagedOptimizedLivenessCustodyReceipt {
        selected_instructions::staging_custody_core_accessors!();

        receipt_accessor_rows!(StagedOptimizedLivenessCustodyReceipt {
            selected: SelectedInstructionPlanIdentity,
            liveness: LivenessIdentity,
        });
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedLivenessCustodyError {
        Analysis(LivenessError),
    }

    failed_display!(
        OptimizedLivenessCustodyError,
        "optimized liveness staging failed"
    );
}
pub use staging::{
    OptimizedLivenessCustodyError, StagedOptimizedLiveness, StagedOptimizedLivenessCustodyReceipt,
    stage_optimized_liveness,
};

copy_receipt! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LivenessValidationReceipt {
        pub(crate) identity: LivenessIdentity,
        pub(crate) selected: SelectedInstructionPlanIdentity,
        pub(crate) optimization_unit: OptimizationUnitIdentity,
        pub(crate) work_schedule: WorkScheduleIdentity,
    }
}

shared_plan_carrier!(ValidatedLiveness: LivenessPlan, LivenessValidationReceipt {});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LivenessError {
    RootMismatch,
    UnsupportedUseDef {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    UnsupportedTiedOperand {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    UnsupportedEarlyClobber {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    FunctionMismatch {
        function: usize,
    },
    BlockMismatch {
        function: usize,
        block: u32,
    },
    InstructionMismatch {
        function: usize,
        instruction: u32,
    },
    SuccessorMismatch {
        function: usize,
        block: u32,
        ordinal: u8,
    },
    NonCanonicalSet {
        function: usize,
        instruction: Option<u32>,
    },
    NonDensePositions {
        function: usize,
    },
    TransferMismatch {
        function: usize,
        instruction: u32,
    },
    FixedConstraintMismatch {
        function: usize,
    },
}

failed_display!(LivenessError, "Terminal selected liveness failed");
