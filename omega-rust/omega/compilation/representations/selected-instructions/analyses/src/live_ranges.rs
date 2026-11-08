/*
  Block-local live-range, constraint, copy-affinity, edge-transfer, and
  interference analysis (`analyze_live_ranges`, `analyze_live_ranges_reusing`,
  `ValidatedLiveRanges`, `StagedOptimizedLiveRanges`).

  Constructs `LiveRangePlan` facts from `SelectedInstructionPlan` and
  `ValidatedLiveness`, reusing unchanged function rows across register-pressure
  recovery steps.
*/

use crate::{SelectedProgramView, ValidatedLiveness};

pub mod compute {

    mod architectural_units {
        use super::{
            ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
            LiveRangeFragment, LiveRangePoint, RegisterUnitId,
        };
        use super::{LiveRangeError, after_point, before_point, connector};

        pub(super) fn architectural_units(
            function_index: usize,
            liveness: &selected_instructions::FunctionLiveness,
        ) -> Result<Vec<ArchitecturalUnitLiveRange>, LiveRangeError> {
            let mut unit_indices = Vec::<u32>::new();
            let mut rows = Vec::<ArchitecturalUnitLiveRange>::new();
            let mut open_unit_end = Vec::<u32>::new();
            let mut touched_units = Vec::<(usize, usize)>::new();
            let ensure_unit = |unit: RegisterUnitId,
                               unit_indices: &mut Vec<u32>,
                               rows: &mut Vec<ArchitecturalUnitLiveRange>,
                               open_unit_end: &mut Vec<u32>|
             -> usize {
                let idx = usize::from(unit.0);
                if idx >= unit_indices.len() {
                    unit_indices.resize(idx + 1, u32::MAX);
                }
                let slot = &mut unit_indices[idx];
                if *slot == u32::MAX {
                    let new_slot = rows.len();
                    *slot = new_slot as u32;
                    rows.push(ArchitecturalUnitLiveRange {
                        unit,
                        actions: Vec::new(),
                        fragments: Vec::new(),
                        edge_connectors: Vec::new(),
                    });
                    open_unit_end.push(u32::MAX);
                    new_slot
                } else {
                    *slot as usize
                }
            };

            for block in &liveness.blocks {
                for &unit in block.unit_live_in.iter().chain(&block.unit_live_out) {
                    ensure_unit(unit, &mut unit_indices, &mut rows, &mut open_unit_end);
                }
                if let (Some(first), Some(last)) =
                    (block.instructions.first(), block.instructions.last())
                {
                    let block_start = before_point(function_index, first.position)?.0;
                    let block_end = after_point(function_index, last.position)?
                        .0
                        .checked_add(1)
                        .ok_or(LiveRangeError::PointOverflow {
                            function: function_index,
                        })?;
                    for &unit in &block.unit_live_out {
                        let slot =
                            ensure_unit(unit, &mut unit_indices, &mut rows, &mut open_unit_end);
                        open_unit_end[slot] = block_end;
                    }
                    let mut unit_effect_indices = Vec::new();
                    for (inst_idx, instruction) in block.instructions.iter().enumerate().rev() {
                        if instruction.unit_defs.is_empty()
                            && instruction.unit_clobbers.is_empty()
                            && instruction.unit_uses.is_empty()
                        {
                            continue;
                        }
                        unit_effect_indices.push(inst_idx);
                        let _before = before_point(function_index, instruction.position)?.0;
                        let after = after_point(function_index, instruction.position)?.0;
                        for &killed in instruction
                            .unit_defs
                            .iter()
                            .chain(&instruction.unit_clobbers)
                        {
                            if !instruction.unit_uses.contains(&killed) {
                                let slot = ensure_unit(
                                    killed,
                                    &mut unit_indices,
                                    &mut rows,
                                    &mut open_unit_end,
                                );
                                if open_unit_end[slot] != u32::MAX {
                                    let frags = &mut rows[slot].fragments;
                                    if frags.last().map(|f| f.block) != Some(block.block) {
                                        touched_units.push((slot, frags.len()));
                                    }
                                    frags.push(LiveRangeFragment {
                                        block: block.block,
                                        start: LiveRangePoint(after),
                                        end: LiveRangePoint(open_unit_end[slot]),
                                    });
                                    open_unit_end[slot] = u32::MAX;
                                }
                            }
                        }
                        for &used in &instruction.unit_uses {
                            let slot =
                                ensure_unit(used, &mut unit_indices, &mut rows, &mut open_unit_end);
                            if open_unit_end[slot] == u32::MAX {
                                open_unit_end[slot] = after;
                            }
                        }
                    }
                    for &unit in &block.unit_live_in {
                        let slot =
                            ensure_unit(unit, &mut unit_indices, &mut rows, &mut open_unit_end);
                        if open_unit_end[slot] != u32::MAX {
                            let frags = &mut rows[slot].fragments;
                            if frags.last().map(|f| f.block) != Some(block.block) {
                                touched_units.push((slot, frags.len()));
                            }
                            frags.push(LiveRangeFragment {
                                block: block.block,
                                start: LiveRangePoint(block_start),
                                end: LiveRangePoint(open_unit_end[slot]),
                            });
                            open_unit_end[slot] = u32::MAX;
                        }
                    }
                    for (slot, first_frag) in touched_units.drain(..) {
                        rows[slot].fragments[first_frag..].reverse();
                    }
                    for &inst_idx in unit_effect_indices.iter().rev() {
                        let instruction = &block.instructions[inst_idx];
                        let before = before_point(function_index, instruction.position)?;
                        let after = after_point(function_index, instruction.position)?;
                        for (kind, values, point) in [
                            (
                                ArchitecturalUnitActionKind::Use,
                                &instruction.unit_uses,
                                before,
                            ),
                            (
                                ArchitecturalUnitActionKind::Def,
                                &instruction.unit_defs,
                                after,
                            ),
                            (
                                ArchitecturalUnitActionKind::Clobber,
                                &instruction.unit_clobbers,
                                after,
                            ),
                        ] {
                            for (idx, &unit) in values.iter().enumerate() {
                                if values[..idx].contains(&unit) {
                                    continue;
                                }
                                let slot = ensure_unit(
                                    unit,
                                    &mut unit_indices,
                                    &mut rows,
                                    &mut open_unit_end,
                                );
                                rows[slot].actions.push(ArchitecturalUnitAction {
                                    block: block.block,
                                    position: instruction.position,
                                    point,
                                    instruction: instruction.instruction,
                                    kind,
                                });
                            }
                        }
                    }
                }
                for edge in &block.successors {
                    for (idx, &unit) in edge.unit_live.iter().enumerate() {
                        if edge.unit_live[..idx].contains(&unit) {
                            continue;
                        }
                        let slot =
                            ensure_unit(unit, &mut unit_indices, &mut rows, &mut open_unit_end);
                        rows[slot]
                            .edge_connectors
                            .push(connector(block.block, edge));
                    }
                }
            }
            rows.sort_by_key(|row| row.unit);
            Ok(rows)
        }
    }
    mod constraints {
        use super::{
            BTreeSet, DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse, LiveRangeError,
            RegisterOperandAccess, VirtualRegisterId, after_point, before_point,
        };

        pub(crate) fn derive_early_clobbers(
            function: usize,
            liveness: &selected_instructions::FunctionLiveness,
        ) -> Result<Vec<EarlyClobberConstraint>, LiveRangeError> {
            if !liveness
                .operand_positions
                .storage_slice()
                .iter()
                .any(|operand| operand.early_clobber)
            {
                return Ok(Vec::new());
            }
            let instruction_operands = || {
                liveness.blocks.iter().flat_map(|block| {
                    block.instructions.iter().map(move |instruction| {
                        (
                            block,
                            liveness
                                .operand_positions
                                .span_or_empty(instruction.operands),
                        )
                    })
                })
            };
            let early_definitions = instruction_operands()
                .flat_map(|(block, operands)| {
                    operands
                        .iter()
                        .filter(|operand| operand.early_clobber)
                        .map(move |definition| (block, operands, definition))
                })
                .collect::<Vec<_>>();
            if early_definitions.is_empty() {
                return Ok(Vec::new());
            }
            let tied_edges = instruction_operands()
                .flat_map(|(_, operands)| {
                    operands
                        .iter()
                        .filter(|operand| operand.tied_to.is_some())
                        .filter_map(move |definition| {
                            operands
                                .iter()
                                .find(|operand| Some(operand.operand) == definition.tied_to)
                                .map(|source| {
                                    (source.virtual_register, definition.virtual_register)
                                })
                        })
                })
                .collect::<Vec<_>>();
            let mut tied_registers = BTreeSet::new();
            for &(left, right) in &tied_edges {
                tied_registers.insert(left);
                tied_registers.insert(right);
            }
            let mut rows = Vec::with_capacity(early_definitions.len());
            for (block, operands, definition) in &early_definitions {
                let tied_source = match definition.tied_to {
                    None => None,
                    Some(source_operand) => Some(
                        operands
                            .iter()
                            .find(|operand| operand.operand == source_operand)
                            .ok_or(LiveRangeError::UnsupportedEarlyClobber {
                                function,
                                instruction: definition.instruction.0,
                                operand: definition.operand,
                            })?,
                    ),
                };
                let multiple_definitions = operands
                    .iter()
                    .filter(|operand| operand.access == RegisterOperandAccess::Def)
                    .count()
                    > 1;
                let source_is_valid = tied_source.is_none_or(|source| {
                    source.access == RegisterOperandAccess::Use
                        && source.operand < definition.operand
                        && source.virtual_register != definition.virtual_register
                        && source.class == definition.class
                        && source.tied_to.is_none()
                });
                let is_unrelated = |operand: &selected_instructions::OperandPosition| {
                    operand.operand != definition.operand
                        && tied_source.is_none_or(|source| source.operand != operand.operand)
                };
                let tie_component_has_one_early_definition = tied_source.is_none_or(|source| {
                    let component = tied_component(source.virtual_register, &tied_edges);
                    tied_edges.contains(&(source.virtual_register, definition.virtual_register))
                        && component.contains(&definition.virtual_register)
                        && early_definitions
                            .iter()
                            .filter(|(_, _, candidate)| {
                                candidate.tied_to.is_some()
                                    && component.contains(&candidate.virtual_register)
                            })
                            .count()
                            == 1
                });
                let untied_definition_is_free =
                    tied_source.is_some() || !tied_registers.contains(&definition.virtual_register);
                if definition.access != RegisterOperandAccess::Def
                    || !multiple_definitions
                        && operands
                            .iter()
                            .filter(|operand| operand.early_clobber)
                            .count()
                            != 1
                    || operands.len() < 2
                    || !operands
                        .iter()
                        .any(|operand| operand.access == RegisterOperandAccess::Use)
                    || multiple_definitions
                        && operands.iter().any(|operand| operand.tied_to.is_some())
                    || !source_is_valid
                    || tied_source.is_some() && !operands.iter().any(&is_unrelated)
                    || operands.iter().enumerate().any(|(idx, operand)| {
                        (operand.operand != definition.operand
                            && ((operand.access != RegisterOperandAccess::Use
                                && !(multiple_definitions
                                    && operand.access == RegisterOperandAccess::Def
                                    && operand.early_clobber))
                                || operand.tied_to.is_some()))
                            || operands[..idx].iter().any(|prior| {
                                prior.virtual_register == operand.virtual_register
                                    || prior.operand == operand.operand
                            })
                    })
                    || !tie_component_has_one_early_definition
                    || !untied_definition_is_free
                    || operands
                        .iter()
                        .filter(|operand| is_unrelated(operand))
                        .any(|operand| tied_registers.contains(&operand.virtual_register))
                {
                    return Err(LiveRangeError::UnsupportedEarlyClobber {
                        function,
                        instruction: definition.instruction.0,
                        operand: definition.operand,
                    });
                }
                let uses = operands
                    .iter()
                    .filter(|operand| {
                        is_unrelated(operand) && operand.access == RegisterOperandAccess::Use
                    })
                    .map(|operand| EarlyClobberUse {
                        operand: operand.operand,
                        virtual_register: operand.virtual_register,
                        class: operand.class,
                    })
                    .collect();
                rows.push(EarlyClobberConstraint {
                    block: block.block,
                    position: definition.position,
                    instruction: definition.instruction,
                    early_point: before_point(function, definition.position)?,
                    def_operand: definition.operand,
                    def_virtual_register: definition.virtual_register,
                    def_class: definition.class,
                    def_point: after_point(function, definition.position)?,
                    uses,
                });
            }
            Ok(rows)
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
                        component.extend([*left, *right]);
                    }
                }
                if component.len() == previous_len {
                    return component;
                }
            }
        }

        pub(crate) fn derive_tied_pairs(
            function: usize,
            liveness: &selected_instructions::FunctionLiveness,
        ) -> Result<Vec<DistinctUseDefTie>, LiveRangeError> {
            if !liveness
                .operand_positions
                .storage_slice()
                .iter()
                .any(|operand| operand.tied_to.is_some())
            {
                return Ok(Vec::new());
            }
            let mut pairs = Vec::new();
            for block in &liveness.blocks {
                for instruction in &block.instructions {
                    let operands = liveness
                        .operand_positions
                        .span_or_empty(instruction.operands);
                    for definition in operands.iter().filter(|operand| operand.tied_to.is_some()) {
                        let Some(use_operand) = operands
                            .iter()
                            .find(|candidate| Some(candidate.operand) == definition.tied_to)
                        else {
                            return Err(LiveRangeError::UnsupportedTiedOperand {
                                function,
                                instruction: definition.instruction.0,
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
                            return Err(LiveRangeError::UnsupportedTiedOperand {
                                function,
                                instruction: definition.instruction.0,
                                operand: definition.operand,
                            });
                        }
                        pairs.push(DistinctUseDefTie {
                            block: block.block,
                            position: definition.position,
                            instruction: definition.instruction,
                            use_operand: use_operand.operand,
                            use_virtual_register: use_operand.virtual_register,
                            use_point: before_point(function, definition.position)?,
                            def_operand: definition.operand,
                            def_virtual_register: definition.virtual_register,
                            def_point: after_point(function, definition.position)?,
                            class: definition.class,
                        });
                    }
                }
            }
            pairs.sort_unstable();
            Ok(pairs)
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use arena::{Arena, HandleSpan};
            use register_model::RegisterClassId;
            use selected_instructions::{
                BlockLiveness, FunctionLiveness, InstructionLiveness, LivenessPosition,
                OperandPosition, SelectedBlockId, SelectedInstructionId,
            };
            use semantic_vocabulary::{BlockId, MachineId};

            fn operands(
                arena: &mut Arena<OperandPosition>,
                position: u32,
                instruction: u32,
                base: u32,
            ) -> HandleSpan<OperandPosition> {
                arena.insert_many((0..3).map(|ordinal| OperandPosition {
                    position: LivenessPosition(position),
                    instruction: SelectedInstructionId(instruction),
                    operand: ordinal as u16,
                    virtual_register: VirtualRegisterId(base + ordinal),
                    class: RegisterClassId(0),
                    access: if ordinal == 2 {
                        RegisterOperandAccess::Def
                    } else {
                        RegisterOperandAccess::Use
                    },
                    tied_to: (ordinal == 2).then_some(0),
                    early_clobber: ordinal == 2,
                    ..OperandPosition::default()
                }))
            }
            fn block(
                block: u32,
                position: u32,
                instruction: u32,
                operands: HandleSpan<OperandPosition>,
            ) -> BlockLiveness {
                BlockLiveness {
                    block: SelectedBlockId(block),
                    source_block: BlockId::new(u64::from(block) + 1).unwrap(),
                    virtual_live_in: vec![],
                    virtual_live_out: vec![],
                    unit_live_in: vec![],
                    unit_live_out: vec![],
                    successors: vec![],
                    instructions: vec![InstructionLiveness {
                        position: LivenessPosition(position),
                        instruction: SelectedInstructionId(instruction),
                        operands,
                        virtual_uses: vec![],
                        virtual_defs: vec![],
                        virtual_live_in: vec![],
                        virtual_live_out: vec![],
                        unit_uses: vec![],
                        unit_defs: vec![],
                        unit_clobbers: vec![],
                        unit_live_in: vec![],
                        unit_live_out: vec![],
                    }],
                }
            }
            fn function() -> FunctionLiveness {
                let mut arena = Arena::new();
                let second = operands(&mut arena, 1, 17, 20);
                let first = operands(&mut arena, 0, 900, 10);
                FunctionLiveness {
                    machine: MachineId::new(1).unwrap(),
                    entry_definitions: vec![],
                    operand_positions: arena,
                    blocks: vec![block(100, 0, 900, first), block(200, 1, 17, second)],
                }
            }
            #[test]
            fn constraints_follow_instruction_spans_and_owning_blocks() {
                let function = function();
                let ties = derive_tied_pairs(0, &function).unwrap();
                let early = derive_early_clobbers(0, &function).unwrap();
                assert_eq!(
                    ties.iter()
                        .map(|tie| (
                            tie.block.0,
                            tie.instruction.0,
                            tie.use_virtual_register.0,
                            tie.def_virtual_register.0
                        ))
                        .collect::<Vec<_>>(),
                    [(100, 900, 10, 12), (200, 17, 20, 22)]
                );
                assert_eq!(
                    early
                        .iter()
                        .map(|row| (
                            row.block.0,
                            row.instruction.0,
                            row.uses[0].virtual_register.0
                        ))
                        .collect::<Vec<_>>(),
                    [(100, 900, 11), (200, 17, 21)]
                );
            }
            #[test]
            fn missing_tie_operand_still_refuses() {
                let mut function = function();
                let definition = function.blocks[0].instructions[0].operands.handle_at(2);
                function.operand_positions.get_mut(definition).tied_to = Some(77);
                assert_eq!(
                    derive_tied_pairs(3, &function),
                    Err(LiveRangeError::UnsupportedTiedOperand {
                        function: 3,
                        instruction: 900,
                        operand: 2
                    })
                );
            }
            #[test]
            fn liveness_identity_binds_operand_membership() {
                let mut function = function();
                let original = selected_instructions::liveness_function_digest(&function);
                function.blocks[0].instructions[0].operands =
                    function.blocks[1].instructions[0].operands;
                assert_ne!(
                    original,
                    selected_instructions::liveness_function_digest(&function)
                );
            }
        }
    }
    mod copy_affinities {

        use crate::LiveRangeError;
        use selected_instructions::{CopyAffinity, SelectedFunction, SelectedInstructionKind};

        pub(super) fn derive(
            function_index: usize,
            selected: &SelectedFunction,
        ) -> Result<Vec<CopyAffinity>, LiveRangeError> {
            let mut rows = Vec::new();
            for (block_index, block) in selected.blocks.iter().enumerate() {
                if let Some((block_affinities, has_malformed_copy)) =
                    selected.blocks.block_copy_affinities(block_index)
                {
                    if has_malformed_copy {
                        return Err(LiveRangeError::FunctionMismatch {
                            function: function_index,
                        });
                    }
                    rows.extend_from_slice(block_affinities);
                    continue;
                }
                for instruction in &block.instructions {
                    if instruction.kind != SelectedInstructionKind::CopyI64 {
                        continue;
                    }
                    let [source, destination] = instruction.operands.as_slice() else {
                        return Err(LiveRangeError::FunctionMismatch {
                            function: function_index,
                        });
                    };
                    if source.virtual_register == destination.virtual_register {
                        continue;
                    }
                    rows.push(CopyAffinity {
                        block: block.id,
                        instruction: instruction.id,
                        source: source.virtual_register,
                        destination: destination.virtual_register,
                    });
                }
            }
            if rows.windows(2).any(|w| w[0] > w[1]) {
                rows.sort_unstable();
            }
            Ok(rows)
        }
    }
    mod edge_transfers {

        use crate::LiveRangeError;
        use selected_instructions::{EdgeRegisterTransfer, FunctionLiveness};
        use selected_instructions::{SelectedFunction, terminator_successors};

        pub(super) fn derive(
            function_index: usize,
            selected: &SelectedFunction,
            live: &FunctionLiveness,
        ) -> Result<Vec<EdgeRegisterTransfer>, LiveRangeError> {
            if !selected
                .blocks
                .iter()
                .any(|block| !terminator_successors(&block.terminator).is_empty())
            {
                return Ok(Vec::new());
            }
            let mut rows = Vec::new();
            let index = crate::liveness::edge_values::FunctionIndex::new(selected);
            for block in &selected.blocks {
                let successors = terminator_successors(&block.terminator);
                for edge in successors {
                    let target = index
                        .block_index(edge.block)
                        .and_then(|idx| live.blocks.get(idx))
                        .filter(|target| target.block == edge.block)
                        .ok_or(LiveRangeError::FunctionMismatch {
                            function: function_index,
                        })?;
                    for parameter in index.parameters_of(edge.block).filter(|register| {
                        target.virtual_live_in.binary_search(&register.id).is_ok()
                    }) {
                        let argument = crate::liveness::edge_values::incoming_argument(
                            function_index,
                            &index,
                            edge,
                            parameter.id,
                        )
                        .map_err(LiveRangeError::LivenessRevalidation)?;
                        rows.push(EdgeRegisterTransfer {
                            source: block.id,
                            target: edge.block,
                            psi_edge: edge.psi_edge.ok_or(LiveRangeError::FunctionMismatch {
                                function: function_index,
                            })?,
                            argument,
                            parameter: parameter.id,
                            class: parameter.class,
                        });
                    }
                }
            }
            rows.sort();
            Ok(rows)
        }
    }
    mod fragments {
        #[cfg(test)]
        use super::BTreeSet;
        use super::{
            BlockLiveness, BlockPointDomain, LiveRangeEdgeConnector, LiveRangeFragment,
            LiveRangePoint, LivenessPosition, RegisterOperandAccess, SelectedBlockId,
        };
        use crate::LiveRangeError;

        pub(super) fn block_domain(
            function: usize,
            block: &BlockLiveness,
        ) -> Result<BlockPointDomain, LiveRangeError> {
            let first = block
                .instructions
                .first()
                .ok_or(LiveRangeError::BlockDomainMismatch {
                    function,
                    block: block.block.0,
                })?;
            let last = block
                .instructions
                .last()
                .expect("nonempty block established above");
            Ok(BlockPointDomain {
                block: block.block,
                source_block: block.source_block,
                start: before_point(function, first.position)?,
                end: after_point(function, last.position)?
                    .0
                    .checked_add(1)
                    .map(LiveRangePoint)
                    .ok_or(LiveRangeError::PointOverflow { function })?,
            })
        }

        pub(super) fn register_fragments(
            function: usize,
            liveness: &selected_instructions::FunctionLiveness,
            register_count: usize,
            register_index: &[u32],
        ) -> Result<Vec<Vec<LiveRangeFragment>>, LiveRangeError> {
            let mut fragments = vec![Vec::<LiveRangeFragment>::new(); register_count];
            let mut open_end = vec![u32::MAX; register_count];
            let mut touched_in_block = Vec::<(u32, u32)>::new();
            for block in &liveness.blocks {
                let Some(first) = block.instructions.first() else {
                    continue;
                };
                let last = block.instructions.last().unwrap();
                let block_start = before_point(function, first.position)?.0;
                let block_end = after_point(function, last.position)?
                    .0
                    .checked_add(1)
                    .ok_or(LiveRangeError::PointOverflow { function })?;
                for &register in &block.virtual_live_out {
                    if let Some(&idx) = register_index.get(register.0 as usize)
                        && idx != u32::MAX
                    {
                        open_end[idx as usize] = block_end;
                    }
                }
                for instruction in block.instructions.iter().rev() {
                    let _before = before_point(function, instruction.position)?.0;
                    let after = after_point(function, instruction.position)?.0;
                    let after_end = after
                        .checked_add(1)
                        .ok_or(LiveRangeError::PointOverflow { function })?;
                    if instruction.virtual_defs.is_empty() && instruction.virtual_uses.is_empty() {
                        let ops = liveness
                            .operand_positions
                            .span_or_empty(instruction.operands);
                        for (op_idx, op) in ops.iter().enumerate() {
                            if !matches!(
                                op.access,
                                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                            ) {
                                continue;
                            }
                            let register = op.virtual_register;
                            if ops[..op_idx].iter().any(|prev| {
                                prev.virtual_register == register
                                    && matches!(
                                        prev.access,
                                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                                    )
                            }) {
                                continue;
                            }
                            if let Some(&idx) = register_index.get(register.0 as usize)
                                && idx != u32::MAX
                            {
                                if open_end[idx as usize] == u32::MAX {
                                    open_end[idx as usize] = after_end;
                                }
                                let also_used = op.access == RegisterOperandAccess::UseDef
                                    || ops.iter().any(|other| {
                                        other.virtual_register == register
                                            && matches!(
                                                other.access,
                                                RegisterOperandAccess::Use
                                                    | RegisterOperandAccess::UseDef
                                            )
                                    });
                                if !also_used {
                                    let slot = &mut fragments[idx as usize];
                                    if slot.last().map(|f| f.block) != Some(block.block) {
                                        touched_in_block.push((idx, slot.len() as u32));
                                    }
                                    slot.push(LiveRangeFragment {
                                        block: block.block,
                                        start: LiveRangePoint(after),
                                        end: LiveRangePoint(open_end[idx as usize]),
                                    });
                                    open_end[idx as usize] = u32::MAX;
                                }
                            }
                        }
                        for op in ops {
                            if matches!(
                                op.access,
                                RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
                            ) && let Some(&idx) =
                                register_index.get(op.virtual_register.0 as usize)
                                && idx != u32::MAX
                                && open_end[idx as usize] == u32::MAX
                            {
                                open_end[idx as usize] = after;
                            }
                        }
                    } else {
                        for &register in &instruction.virtual_defs {
                            if let Some(&idx) = register_index.get(register.0 as usize)
                                && idx != u32::MAX
                            {
                                if open_end[idx as usize] == u32::MAX {
                                    open_end[idx as usize] = after_end;
                                }
                                if !instruction.virtual_uses.contains(&register) {
                                    let slot = &mut fragments[idx as usize];
                                    if slot.last().map(|f| f.block) != Some(block.block) {
                                        touched_in_block.push((idx, slot.len() as u32));
                                    }
                                    slot.push(LiveRangeFragment {
                                        block: block.block,
                                        start: LiveRangePoint(after),
                                        end: LiveRangePoint(open_end[idx as usize]),
                                    });
                                    open_end[idx as usize] = u32::MAX;
                                }
                            }
                        }
                        for &register in &instruction.virtual_uses {
                            if let Some(&idx) = register_index.get(register.0 as usize)
                                && idx != u32::MAX
                                && open_end[idx as usize] == u32::MAX
                            {
                                open_end[idx as usize] = after;
                            }
                        }
                    }
                }
                for &register in &block.virtual_live_in {
                    if let Some(&idx) = register_index.get(register.0 as usize)
                        && idx != u32::MAX
                        && open_end[idx as usize] != u32::MAX
                    {
                        let slot = &mut fragments[idx as usize];
                        if slot.last().map(|f| f.block) != Some(block.block) {
                            touched_in_block.push((idx, slot.len() as u32));
                        }
                        slot.push(LiveRangeFragment {
                            block: block.block,
                            start: LiveRangePoint(block_start),
                            end: LiveRangePoint(open_end[idx as usize]),
                        });
                        open_end[idx as usize] = u32::MAX;
                    }
                }
                for (idx, first_frag) in touched_in_block.drain(..) {
                    fragments[idx as usize][first_frag as usize..].reverse();
                }
            }
            Ok(fragments)
        }

        #[cfg(test)]
        pub(super) fn append_point(
            function: usize,
            fragments: &mut Vec<LiveRangeFragment>,
            block: SelectedBlockId,
            point: LiveRangePoint,
        ) -> Result<(), LiveRangeError> {
            let end = point
                .0
                .checked_add(1)
                .map(LiveRangePoint)
                .ok_or(LiveRangeError::PointOverflow { function })?;
            if let Some(last) = fragments.last_mut()
                && last.block == block
                && point <= last.end
            {
                last.end = last.end.max(end);
            } else {
                fragments.push(LiveRangeFragment {
                    block,
                    start: point,
                    end,
                });
            }
            Ok(())
        }

        #[cfg(test)]
        pub(super) fn fragments_from_points(
            block: SelectedBlockId,
            points: BTreeSet<LiveRangePoint>,
        ) -> Vec<LiveRangeFragment> {
            let mut fragments = Vec::new();
            let mut iterator = points.into_iter();
            let Some(first) = iterator.next() else {
                return fragments;
            };
            let mut start = first;
            let mut last = first;
            for point in iterator {
                if last.0.checked_add(1) == Some(point.0) {
                    last = point;
                } else {
                    fragments.push(LiveRangeFragment {
                        block,
                        start,
                        end: LiveRangePoint(last.0 + 1),
                    });
                    start = point;
                    last = point;
                }
            }
            fragments.push(LiveRangeFragment {
                block,
                start,
                end: LiveRangePoint(last.0 + 1),
            });
            fragments
        }

        pub(super) fn connector(
            source: SelectedBlockId,
            edge: &selected_instructions::SuccessorLiveness,
        ) -> LiveRangeEdgeConnector {
            LiveRangeEdgeConnector {
                source,
                terminator: edge.terminator,
                polarity_ordinal: edge.polarity_ordinal,
                psi_edge: edge.psi_edge,
                target: edge.target,
            }
        }

        pub(super) fn operand_point(
            function: usize,
            position: LivenessPosition,
            access: RegisterOperandAccess,
        ) -> Result<LiveRangePoint, LiveRangeError> {
            match access {
                RegisterOperandAccess::Use => before_point(function, position),
                RegisterOperandAccess::Def => after_point(function, position),
                RegisterOperandAccess::UseDef => Err(LiveRangeError::UnsupportedUseDef {
                    function,
                    instruction: 0,
                    operand: 0,
                }),
            }
        }

        pub(super) fn before_point(
            function: usize,
            position: LivenessPosition,
        ) -> Result<LiveRangePoint, LiveRangeError> {
            position
                .0
                .checked_mul(2)
                .map(LiveRangePoint)
                .ok_or(LiveRangeError::PointOverflow { function })
        }

        pub(super) fn after_point(
            function: usize,
            position: LivenessPosition,
        ) -> Result<LiveRangePoint, LiveRangeError> {
            position
                .0
                .checked_mul(2)
                .and_then(|point| point.checked_add(1))
                .map(LiveRangePoint)
                .ok_or(LiveRangeError::PointOverflow { function })
        }

        #[cfg(test)]
        mod tests {
            use super::{LiveRangePoint, SelectedBlockId};

            #[test]
            fn ordered_membership_matches_point_set_fragments() {
                for membership in 0u32..1024 {
                    let points = (0..10)
                        .filter(|point| membership & (1 << point) != 0)
                        .map(LiveRangePoint)
                        .collect::<std::collections::BTreeSet<_>>();
                    let mut actual = Vec::new();
                    let mut expected = Vec::new();
                    for block in [SelectedBlockId(9), SelectedBlockId(2)] {
                        for point in &points {
                            super::append_point(0, &mut actual, block, *point).unwrap();
                            super::append_point(0, &mut actual, block, *point).unwrap();
                        }
                        expected.extend(super::fragments_from_points(block, points.clone()));
                    }
                    assert_eq!(actual, expected);
                }
                assert!(matches!(
                    super::append_point(
                        3,
                        &mut Vec::new(),
                        SelectedBlockId(0),
                        LiveRangePoint(u32::MAX)
                    ),
                    Err(super::LiveRangeError::PointOverflow { function: 3 })
                ));
            }
        }
    }
    mod function {
        use super::{
            FunctionLiveRanges, LiveRangeError, RegisterOperandAccess, VirtualFixedConstraint,
            VirtualFixedConstraintSite, VirtualLiveRange, VirtualOccurrence, architectural_units,
            block_domain, connector, derive_early_clobbers, derive_tied_pairs, operand_point,
            register_fragments,
        };

        pub(crate) fn compute_function(
            function_index: usize,
            selected: &selected_instructions::SelectedFunction,
            liveness: &selected_instructions::FunctionLiveness,
        ) -> Result<FunctionLiveRanges, LiveRangeError> {
            reject_unsupported(function_index, liveness)?;
            let block_domains = liveness
                .blocks
                .iter()
                .map(|block| block_domain(function_index, block))
                .collect::<Result<Vec<_>, _>>()?;
            let tied_pairs = derive_tied_pairs(function_index, liveness)?;
            let edge_transfers = super::edge_transfers::derive(function_index, selected, liveness)?;
            let copy_affinities = super::copy_affinities::derive(function_index, selected)?;
            let early_clobbers = derive_early_clobbers(function_index, liveness)?;

            let register_count = selected.virtual_registers.len();
            let max_reg_id = selected
                .virtual_registers
                .iter()
                .map(|register| register.id.0 as usize)
                .max()
                .unwrap_or(0);
            let mut register_index = vec![u32::MAX; max_reg_id + 1];
            let mut has_duplicate_register_ids = false;
            for (idx, register) in selected.virtual_registers.iter().enumerate() {
                let slot = &mut register_index[register.id.0 as usize];
                if *slot == u32::MAX {
                    *slot = idx as u32;
                } else {
                    has_duplicate_register_ids = true;
                }
            }
            let mut fragment_rows =
                register_fragments(function_index, liveness, register_count, &register_index)?;
            let operand_slice = liveness.operand_positions.storage_slice();
            let mut operand_counts = vec![0u32; register_count];
            for row in operand_slice {
                if let Some(&idx) = register_index.get(row.virtual_register.0 as usize)
                    && idx != u32::MAX
                {
                    operand_counts[idx as usize] += 1;
                }
            }
            let mut operand_offsets = vec![0u32; register_count + 1];
            for i in 0..register_count {
                operand_offsets[i + 1] = operand_offsets[i] + operand_counts[i];
            }
            let mut operand_cursors = operand_offsets.clone();
            let mut operand_indices = vec![0u32; operand_offsets[register_count] as usize];
            for (row_idx, row) in operand_slice.iter().enumerate() {
                if let Some(&idx) = register_index.get(row.virtual_register.0 as usize)
                    && idx != u32::MAX
                {
                    let cursor = &mut operand_cursors[idx as usize];
                    operand_indices[*cursor as usize] = row_idx as u32;
                    *cursor += 1;
                }
            }
            let has_edges = liveness.blocks.iter().any(|b| !b.successors.is_empty());
            let mut edge_rows = if has_edges {
                vec![Vec::new(); register_count]
            } else {
                Vec::new()
            };
            if has_edges {
                for block in &liveness.blocks {
                    for edge in &block.successors {
                        let mut prev = None;
                        for &register in &edge.virtual_live {
                            if prev == Some(register) {
                                continue;
                            }
                            prev = Some(register);
                            if let Some(&idx) = register_index.get(register.0 as usize)
                                && idx != u32::MAX
                            {
                                edge_rows[idx as usize].push(connector(block.block, edge));
                            }
                        }
                    }
                }
            }

            let mut virtual_rows = Vec::with_capacity(register_count);
            for (pos, register) in selected.virtual_registers.iter().enumerate() {
                let canonical_idx = register_index[register.id.0 as usize] as usize;
                let row_start = operand_offsets[canonical_idx] as usize;
                let row_end = operand_offsets[canonical_idx + 1] as usize;
                let row_indices = &operand_indices[row_start..row_end];
                let mut occurrences = Vec::with_capacity(row_indices.len());
                let mut fixed_constraints = Vec::new();
                if let Some(view) = register.entry_fixed_view {
                    fixed_constraints.push(VirtualFixedConstraint {
                        site: VirtualFixedConstraintSite::Entry,
                        view,
                    });
                }
                for &row_idx in row_indices {
                    let row = &operand_slice[row_idx as usize];
                    let point = operand_point(function_index, row.position, row.access)?;
                    occurrences.push(VirtualOccurrence {
                        position: row.position,
                        point,
                        instruction: row.instruction,
                        operand: row.operand,
                        access: row.access,
                    });
                    if let Some(view) = row.fixed_view {
                        fixed_constraints.push(VirtualFixedConstraint {
                            site: VirtualFixedConstraintSite::Operand {
                                position: row.position,
                                point,
                                instruction: row.instruction,
                                operand: row.operand,
                                access: row.access,
                            },
                            view,
                        });
                    }
                }
                let fragments = if pos == canonical_idx {
                    std::mem::take(&mut fragment_rows[canonical_idx])
                } else {
                    Vec::new()
                };
                let edge_connectors = if has_edges {
                    if !has_duplicate_register_ids {
                        std::mem::take(&mut edge_rows[canonical_idx])
                    } else {
                        edge_rows[canonical_idx].clone()
                    }
                } else {
                    Vec::new()
                };
                virtual_rows.push(VirtualLiveRange {
                    virtual_register: register.id,
                    class: register.class,
                    occurrences,
                    fixed_constraints,
                    fragments,
                    edge_connectors,
                });
            }

            let architectural_units = architectural_units(function_index, liveness)?;

            let interference = super::interference(&virtual_rows);
            Ok(FunctionLiveRanges {
                machine: selected.machine,
                block_domains,
                virtual_registers: virtual_rows,
                tied_pairs,
                edge_transfers,
                copy_affinities,
                early_clobbers,
                architectural_units,
                interference,
            })
        }

        fn reject_unsupported(
            function: usize,
            liveness: &selected_instructions::FunctionLiveness,
        ) -> Result<(), LiveRangeError> {
            for operand in liveness.operand_positions.storage_slice() {
                if operand.access == RegisterOperandAccess::UseDef {
                    return Err(LiveRangeError::UnsupportedUseDef {
                        function,
                        instruction: operand.instruction.0,
                        operand: operand.operand,
                    });
                }
            }
            Ok(())
        }
    }
    mod interference {
        use super::{
            LiveRangeFragment, SelectedBlockId, VirtualInterference, VirtualInterferences,
            VirtualLiveRange,
        };
        use std::collections::BTreeMap;

        struct Interval {
            start: u32,
            end: u32,
            row: u32,
            max_end: u32,
        }

        const _: () = assert!(std::mem::size_of::<Interval>() == 16);

        fn prepare(intervals: &mut [Interval]) -> u32 {
            if intervals.is_empty() {
                return 0;
            }
            let middle = intervals.len() / 2;
            let (left, rest) = intervals.split_at_mut(middle);
            let (node, right) = rest.split_first_mut().expect("nonempty interval slice");
            node.max_end = node.end.max(prepare(left)).max(prepare(right));
            node.max_end
        }

        fn append_overlaps(
            intervals: &[Interval],
            fragment: &LiveRangeFragment,
            lower_row: usize,
            rows: &[VirtualLiveRange],
            seen: &mut [usize],
            pairs: &mut VirtualInterferences,
        ) {
            if intervals.is_empty() {
                return;
            }
            let middle = intervals.len() / 2;
            let node = &intervals[middle];
            if node.max_end <= fragment.start.0 {
                return;
            }
            append_overlaps(&intervals[..middle], fragment, lower_row, rows, seen, pairs);
            if node.start >= fragment.end.0 {
                return;
            }
            let higher_row = node.row as usize;
            if lower_row < higher_row
                && fragment.start.0 < node.end
                && seen[higher_row] != lower_row
            {
                seen[higher_row] = lower_row;
                pairs.push(VirtualInterference {
                    lower: rows[lower_row].virtual_register,
                    higher: rows[higher_row].virtual_register,
                });
            }
            append_overlaps(
                &intervals[middle + 1..],
                fragment,
                lower_row,
                rows,
                seen,
                pairs,
            );
        }

        pub(super) fn interference(rows: &[VirtualLiveRange]) -> VirtualInterferences {
            let mut blocks = BTreeMap::<SelectedBlockId, Vec<Interval>>::new();
            for (row, range) in rows.iter().enumerate() {
                for fragment in &range.fragments {
                    if fragment.start < fragment.end {
                        blocks.entry(fragment.block).or_default().push(Interval {
                            start: fragment.start.0,
                            end: fragment.end.0,
                            row: row as u32,
                            max_end: 0,
                        });
                    }
                }
            }
            for intervals in blocks.values_mut() {
                intervals.sort_unstable_by_key(|node| (node.start, node.row));
                prepare(intervals);
            }

            let mut pairs = VirtualInterferences::new();
            let mut seen = vec![usize::MAX; rows.len()];
            for (row, range) in rows.iter().enumerate() {
                let first_pair = pairs.len();
                for fragment in &range.fragments {
                    if fragment.start < fragment.end {
                        append_overlaps(
                            &blocks[&fragment.block],
                            fragment,
                            row,
                            rows,
                            &mut seen,
                            &mut pairs,
                        );
                    }
                }
                pairs.sort_lower_tail(first_pair);
            }
            if !rows
                .windows(2)
                .all(|pair| pair[0].virtual_register < pair[1].virtual_register)
            {
                pairs.sort_unstable_and_dedup();
            }
            pairs
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use selected_instructions::{LiveRangePoint, VirtualRegisterId};

            fn range(id: u32, fragments: Vec<LiveRangeFragment>) -> VirtualLiveRange {
                VirtualLiveRange {
                    virtual_register: VirtualRegisterId(id),
                    class: register_model::RegisterClassId(0),
                    occurrences: Vec::new(),
                    fixed_constraints: Vec::new(),
                    fragments,
                    edge_connectors: Vec::new(),
                }
            }

            fn expected(rows: &[VirtualLiveRange]) -> Vec<VirtualInterference> {
                let mut pairs = Vec::new();
                for (lower, left) in rows.iter().enumerate() {
                    for right in rows.iter().skip(lower + 1) {
                        if left.fragments.iter().any(|left| {
                            left.start < left.end
                                && right.fragments.iter().any(|right| {
                                    right.start < right.end
                                        && left.block == right.block
                                        && left.start < right.end
                                        && right.start < left.end
                                })
                        }) {
                            pairs.push(VirtualInterference {
                                lower: left.virtual_register,
                                higher: right.virtual_register,
                            });
                        }
                    }
                }
                pairs.sort_unstable();
                pairs.dedup();
                pairs
            }

            #[test]
            fn sweep_matches_every_row_pair() {
                let mut seed = 0x2545_f491_4f6c_dd1du64;
                let mut next = |bound: u32| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    (seed % u64::from(bound)) as u32
                };
                for _ in 0..200 {
                    let rows = (0..next(24) + 1)
                        .map(|row| {
                            let fragments = (0..next(5))
                                .map(|_| {
                                    let start = next(40);
                                    LiveRangeFragment {
                                        block: SelectedBlockId(next(3) * 100_003),
                                        start: LiveRangePoint(start),
                                        end: LiveRangePoint(start + next(12)),
                                    }
                                })
                                .collect::<Vec<_>>();
                            range(row * 100_003, fragments)
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        interference(&rows).iter().collect::<Vec<_>>(),
                        expected(&rows)
                    );
                    let mut dense_blocks = rows.clone();
                    for row in &mut dense_blocks {
                        for fragment in &mut row.fragments {
                            fragment.block.0 %= 3;
                        }
                    }
                    assert_eq!(
                        interference(&dense_blocks).iter().collect::<Vec<_>>(),
                        expected(&dense_blocks)
                    );
                    let reversed = rows.into_iter().rev().collect::<Vec<_>>();
                    assert_eq!(
                        interference(&reversed).iter().collect::<Vec<_>>(),
                        expected(&reversed)
                    );
                }
                let fragments = vec![
                    LiveRangeFragment {
                        block: SelectedBlockId(u32::MAX),
                        start: LiveRangePoint(0),
                        end: LiveRangePoint(10),
                    };
                    128
                ];
                let rows = vec![
                    range(u32::MAX, fragments.clone()),
                    range(0, fragments.clone()),
                    range(u32::MAX, fragments),
                    range(9, Vec::new()),
                ];
                assert_eq!(
                    interference(&rows).iter().collect::<Vec<_>>(),
                    expected(&rows)
                );
                assert!(interference(&[]).is_empty());
            }
        }
    }

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use crate::{LiveRangeError, ValidatedLiveness};
    use register_model::{RegisterOperandAccess, RegisterUnitId};
    use selected_instructions::{
        ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
        BlockLiveness, BlockPointDomain, DistinctUseDefTie, EarlyClobberConstraint,
        EarlyClobberUse, FunctionLiveRanges, LiveRangeEdgeConnector, LiveRangeFragment,
        LiveRangePlan, LiveRangePoint, LivenessPosition, VirtualFixedConstraint,
        VirtualFixedConstraintSite, VirtualInterference, VirtualInterferences, VirtualLiveRange,
        VirtualOccurrence,
    };
    use selected_instructions::{SelectedBlockId, VirtualRegisterId};

    use architectural_units::architectural_units;
    pub(crate) use constraints::{derive_early_clobbers, derive_tied_pairs};
    use fragments::{
        after_point, before_point, block_domain, connector, operand_point, register_fragments,
    };
    pub(crate) use function::compute_function;
    use interference::interference;

    pub(crate) fn compute_terminal_live_ranges(
        selected: &impl crate::SelectedProgramView,
        liveness: &ValidatedLiveness,
    ) -> Result<LiveRangePlan, LiveRangeError> {
        let functions = selected
            .selected_plan()
            .functions
            .iter()
            .zip(&liveness.plan().functions)
            .enumerate()
            .map(|(index, (selected, live))| compute_function(index, selected, live).map(Arc::new))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(LiveRangePlan {
            selected: selected.selected_identity(),
            liveness: liveness.receipt().identity(),
            optimization_unit: selected.optimization_unit_identity(),
            work_schedule: selected.work_schedule_identity(),
            target: selected.selected_plan().target,
            functions,
        })
    }

    pub(crate) fn compute_terminal_live_ranges_reusing(
        previous: &impl crate::SelectedProgramView,
        previous_liveness: &ValidatedLiveness,
        previous_ranges: &crate::ValidatedLiveRanges,
        selected: &impl crate::SelectedProgramView,
        liveness: &ValidatedLiveness,
    ) -> Result<(LiveRangePlan, Vec<bool>), LiveRangeError> {
        let prior = previous_ranges.plan();
        let mut carried = vec![false; selected.selected_plan().functions.len()];
        let compatible = prior.selected == previous.selected_identity()
            && prior.liveness == previous_liveness.receipt().identity()
            && previous_liveness.plan().selected == previous.selected_identity()
            && prior.optimization_unit == previous.optimization_unit_identity()
            && prior.work_schedule == previous.work_schedule_identity()
            && prior.target == previous.selected_plan().target
            && prior.optimization_unit == selected.optimization_unit_identity()
            && prior.work_schedule == selected.work_schedule_identity()
            && prior.target == selected.selected_plan().target
            && prior.functions.len() == previous.selected_plan().functions.len();
        let functions = selected
            .selected_plan()
            .functions
            .iter()
            .zip(&liveness.plan().functions)
            .enumerate()
            .map(|(function_index, (function, live))| {
                if compatible
                    && (previous.selected_plan().functions.shares_function_storage(
                        &selected.selected_plan().functions,
                        function_index,
                    ) || previous.selected_plan().functions.get(function_index)
                        == Some(function))
                    && previous_liveness.plan().functions.get(function_index) == Some(live)
                    && let Some(facts) = prior.functions.get(function_index)
                {
                    carried[function_index] = true;
                    return Ok(facts.clone());
                }
                compute_function(function_index, function, live).map(Arc::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((
            LiveRangePlan {
                selected: selected.selected_identity(),
                liveness: liveness.receipt().identity(),
                optimization_unit: selected.optimization_unit_identity(),
                work_schedule: selected.work_schedule_identity(),
                target: selected.selected_plan().target,
                functions,
            },
            carried,
        ))
    }
}
pub(crate) mod validate {

    mod counts {

        use selected_instructions::FunctionLiveRanges;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct FunctionCounts {
            pub(super) virtual_register_count: usize,
            pub(super) virtual_fragment_count: usize,
        }

        impl FunctionCounts {
            pub fn new(function: &FunctionLiveRanges) -> Self {
                Self {
                    virtual_register_count: function.virtual_registers.len(),
                    virtual_fragment_count: function
                        .virtual_registers
                        .iter()
                        .map(|row| row.fragments.len())
                        .sum(),
                }
            }
        }
    }
    mod receipt {

        use super::FunctionCounts;

        use crate::{LiveRangeValidationReceipt, ValidatedLiveRanges};
        use selected_instructions::{LiveRangePlan, live_range_identity_from_digests};

        pub fn admit(
            plan: LiveRangePlan,
            carried: &[bool],
            previous_digests: &[[u8; 32]],
            previous_counts: &[FunctionCounts],
        ) -> ValidatedLiveRanges {
            let function_digests =
                crate::carried_digests(&plan.functions, carried, previous_digests, |function| {
                    selected_instructions::live_range_function_digest(function)
                });
            let function_counts: std::sync::Arc<[FunctionCounts]> = plan
                .functions
                .iter()
                .enumerate()
                .map(|(index, function)| match previous_counts.get(index) {
                    Some(counts) if carried.get(index) == Some(&true) => *counts,
                    _ => FunctionCounts::new(function),
                })
                .collect();
            let receipt = build_receipt(&plan, &function_digests, &function_counts);
            ValidatedLiveRanges {
                plan: plan.into(),
                receipt,
                function_digests,
                function_counts,
            }
        }

        fn build_receipt(
            plan: &LiveRangePlan,
            function_digests: &[[u8; 32]],
            function_counts: &[FunctionCounts],
        ) -> LiveRangeValidationReceipt {
            LiveRangeValidationReceipt {
                identity: live_range_identity_from_digests(plan, function_digests),
                selected: plan.selected,
                liveness: plan.liveness,
                optimization_unit: plan.optimization_unit,
                work_schedule: plan.work_schedule,
                virtual_register_count: function_counts
                    .iter()
                    .map(|counts| counts.virtual_register_count)
                    .sum(),
                virtual_fragment_count: function_counts
                    .iter()
                    .map(|counts| counts.virtual_fragment_count)
                    .sum(),
            }
        }
    }
    pub(crate) use counts::FunctionCounts;
    pub use receipt::admit;

    use crate::{LiveRangeError, ValidatedLiveness};
    pub(super) fn require_liveness_of(
        selected: &impl crate::SelectedProgramView,
        liveness: &ValidatedLiveness,
    ) -> Result<(), LiveRangeError> {
        if liveness.plan().selected != selected.selected_identity()
            || liveness.receipt().selected() != selected.selected_identity()
        {
            return Err(LiveRangeError::RootMismatch);
        }
        Ok(())
    }
}

use crate::LivenessError;
use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{
    LiveRangeIdentity, LiveRangePlan, LivenessIdentity, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::WorkScheduleIdentity;

pub fn analyze_live_ranges<S: SelectedProgramView>(
    selected: &S,
    liveness: &ValidatedLiveness,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    validate::require_liveness_of(selected, liveness)?;
    let plan = compute::compute_terminal_live_ranges(selected, liveness)?;
    Ok(validate::admit(plan, &[], &[], &[]))
}

pub fn analyze_live_ranges_reusing(
    previous: &impl SelectedProgramView,
    previous_liveness: &ValidatedLiveness,
    previous_ranges: &ValidatedLiveRanges,
    selected: &impl SelectedProgramView,
    liveness: &ValidatedLiveness,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    validate::require_liveness_of(selected, liveness)?;
    let (plan, carried) = compute::compute_terminal_live_ranges_reusing(
        previous,
        previous_liveness,
        previous_ranges,
        selected,
        liveness,
    )?;
    Ok(validate::admit(
        plan,
        &carried,
        &previous_ranges.function_digests,
        &previous_ranges.function_counts,
    ))
}

mod staging {

    mod compute {
        use crate::{ValidatedLiveRanges, analyze_live_ranges};

        use crate::StagedOptimizedLiveness;

        use super::OptimizedLiveRangeCustodyError;

        pub(super) fn compute_live_ranges(
            liveness: &StagedOptimizedLiveness,
        ) -> Result<ValidatedLiveRanges, OptimizedLiveRangeCustodyError> {
            analyze_live_ranges(liveness.selected(), liveness.liveness())
                .map_err(OptimizedLiveRangeCustodyError::Analysis)
        }
    }
    mod custody {
        use crate::LiveRangeValidationReceipt;

        use crate::StagedOptimizedLivenessCustodyReceipt;

        use super::StagedOptimizedLiveRangeCustodyReceipt;

        pub(super) fn live_range_custody_receipt(
            upstream: StagedOptimizedLivenessCustodyReceipt,
            ranges: LiveRangeValidationReceipt,
        ) -> StagedOptimizedLiveRangeCustodyReceipt {
            StagedOptimizedLiveRangeCustodyReceipt {
                core: upstream.core(),
                selected: upstream.selected(),
                liveness: upstream.liveness(),
                ranges: ranges.identity(),
            }
        }
    }

    use crate::{LiveRangeError, StagedOptimizedLiveness, ValidatedLiveRanges, ValidatedLiveness};
    use optimization_core::{
        OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
        OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
        PrePhysicalOptimizationManifestIdentity,
    };
    use register_environment::ValidatedTargetRegisterEnvironment;
    use selected_instructions::SelectedProgram;
    use selected_instructions::StagedOptimizedCustodyCore;
    use selected_instructions::{LiveRangeIdentity, SelectedInstructionPlanIdentity};
    use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
    use terminal_psi::TerminalPsiIdentity;

    pub fn stage_optimized_live_ranges(
        liveness: StagedOptimizedLiveness,
    ) -> Result<StagedOptimizedLiveRanges, OptimizedLiveRangeCustodyError> {
        let ranges = compute::compute_live_ranges(&liveness)?;
        let custody = custody::live_range_custody_receipt(liveness.custody(), ranges.receipt());
        Ok(StagedOptimizedLiveRanges {
            liveness,
            ranges,
            custody,
        })
    }

    #[derive(Debug)]
    pub struct StagedOptimizedLiveRanges {
        liveness: StagedOptimizedLiveness,
        ranges: ValidatedLiveRanges,
        custody: StagedOptimizedLiveRangeCustodyReceipt,
    }

    impl StagedOptimizedLiveRanges {
        pub const fn liveness_stage(&self) -> &StagedOptimizedLiveness {
            &self.liveness
        }

        staged_producer_accessors!(
            liveness; liveness, selected, register_environment, selections, budget_per_pass
        );

        pub const fn ranges(&self) -> &ValidatedLiveRanges {
            &self.ranges
        }

        receipt_accessor_rows!(ref StagedOptimizedLiveRanges {
            custody: StagedOptimizedLiveRangeCustodyReceipt,
        });
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StagedOptimizedLiveRangeCustodyReceipt {
        core: StagedOptimizedCustodyCore,
        selected: SelectedInstructionPlanIdentity,
        liveness: selected_instructions::LivenessIdentity,
        ranges: LiveRangeIdentity,
    }

    impl StagedOptimizedLiveRangeCustodyReceipt {
        selected_instructions::staging_custody_core_accessors!();

        receipt_accessor_rows!(StagedOptimizedLiveRangeCustodyReceipt {
            selected: SelectedInstructionPlanIdentity,
            liveness: selected_instructions::LivenessIdentity,
            ranges: LiveRangeIdentity,
        });
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum OptimizedLiveRangeCustodyError {
        Analysis(LiveRangeError),
    }

    failed_display!(
        OptimizedLiveRangeCustodyError,
        "optimized live-range staging failed"
    );
}
pub use staging::StagedOptimizedLiveRanges;
pub use staging::{
    OptimizedLiveRangeCustodyError, StagedOptimizedLiveRangeCustodyReceipt,
    stage_optimized_live_ranges,
};

copy_receipt! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LiveRangeValidationReceipt {
        pub(crate) identity: LiveRangeIdentity,
        pub(crate) selected: SelectedInstructionPlanIdentity,
        pub(crate) liveness: LivenessIdentity,
        pub(crate) optimization_unit: OptimizationUnitIdentity,
        pub(crate) work_schedule: WorkScheduleIdentity,
        pub(crate) virtual_register_count: usize,
        pub(crate) virtual_fragment_count: usize,
    }
}

shared_plan_carrier!(ValidatedLiveRanges: LiveRangePlan, LiveRangeValidationReceipt {
    pub(crate) function_counts: std::sync::Arc<[validate::FunctionCounts]>,
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveRangeError {
    LivenessRevalidation(LivenessError),
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
    PointOverflow {
        function: usize,
    },
    FunctionMismatch {
        function: usize,
    },
    BlockDomainMismatch {
        function: usize,
        block: u32,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    ArchitecturalUnitMismatch {
        function: usize,
        unit: u16,
    },
    InterferenceMismatch {
        function: usize,
    },
    TiedPairMismatch {
        function: usize,
    },
    EarlyClobberMismatch {
        function: usize,
    },
    NonCanonicalRows {
        function: usize,
    },
}

failed_display!(LiveRangeError, "Terminal live-range derivation failed");
