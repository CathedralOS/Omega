/*
We coordinate the selected-CFG rewrite catalog (`catalog`), shared condition-flag
reachability (`condition_state`), stack-slot and projected place-storage
tracking (`place_storage`), instruction-window hazard checks (`window_hazards`),
and the joint pre-allocation fixed-point loop (`pre_allocation`). Individual
rewrite families live in sibling modules and are re-exported here so callers and
tests share one stage-rewrite namespace.
*/

macro_rules! declared_kind_matchers {
    ($name:ident) => {
        impl $name {
            pub(crate) fn matches_consumer(&self, kind: SelectedInstructionKind) -> bool {
                machine_semantic_kind(kind) == self.consumer
            }

            pub(crate) fn matches_producer(&self, kind: SelectedInstructionKind) -> bool {
                machine_semantic_kind(kind) == self.producer
            }
        }
    };
}

macro_rules! declared_pair_lookup {
    ($lookup:ident, $table:ident, $rule:ty) => {
        pub(crate) fn $lookup(
            producer: MachineSemanticKind,
            consumer: MachineSemanticKind,
        ) -> Option<&'static $rule> {
            let mut matches = $table
                .iter()
                .filter(|rule| rule.producer() == producer && rule.consumer() == consumer);
            let rule = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            Some(rule)
        }
    };
}

pub(crate) use crate::address_and_load_rewrites::address_fold;
pub(crate) mod catalog {
    use optimization_core::{Optimization, OptimizationExecutionPhase};

    use selected_instruction_analyses::{
        ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry,
    };

    use super::{
        PRE_ALLOCATION_RULE_CATALOG, PreAllocationRuleCatalogEntry, SELECTED_LOWERING_RULE_CATALOG,
        SelectedLoweringRuleCatalogEntry,
    };

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum SelectedStageRuleRows {
        SelectedLowering(&'static [SelectedLoweringRuleCatalogEntry]),
        PreAllocation(&'static [PreAllocationRuleCatalogEntry]),
        AllocationRecovery(&'static [AllocationRecoveryRuleCatalogEntry]),
    }

    impl SelectedStageRuleRows {
        pub fn contains(&self, optimization: Optimization) -> bool {
            match self {
                Self::SelectedLowering(rows) => rows
                    .iter()
                    .any(|entry| entry.optimization() == optimization),
                Self::PreAllocation(rows) => rows
                    .iter()
                    .any(|entry| entry.optimization() == optimization),
                Self::AllocationRecovery(rows) => rows
                    .iter()
                    .any(|entry| entry.optimization() == optimization),
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct SelectedStageRuleCatalogSlice {
        phase: OptimizationExecutionPhase,
        rows: SelectedStageRuleRows,
    }

    impl SelectedStageRuleCatalogSlice {
        pub const fn new(phase: OptimizationExecutionPhase, rows: SelectedStageRuleRows) -> Self {
            Self { phase, rows }
        }

        receipt_accessor_rows!(ref SelectedStageRuleCatalogSlice {
            phase: OptimizationExecutionPhase,
            rows: SelectedStageRuleRows,
        });
    }

    pub(crate) const SELECTED_STAGE_RULE_CATALOG: [SelectedStageRuleCatalogSlice; 3] = [
        SelectedStageRuleCatalogSlice::new(
            OptimizationExecutionPhase::SelectedLowering,
            SelectedStageRuleRows::SelectedLowering(&SELECTED_LOWERING_RULE_CATALOG),
        ),
        SelectedStageRuleCatalogSlice::new(
            OptimizationExecutionPhase::PreAllocation,
            SelectedStageRuleRows::PreAllocation(&PRE_ALLOCATION_RULE_CATALOG),
        ),
        SelectedStageRuleCatalogSlice::new(
            OptimizationExecutionPhase::AllocationRecovery,
            SelectedStageRuleRows::AllocationRecovery(&ALLOCATION_RECOVERY_RULE_CATALOG),
        ),
    ];

    pub(crate) fn selected_stage_rule_rows(
        phase: OptimizationExecutionPhase,
    ) -> Option<SelectedStageRuleRows> {
        SELECTED_STAGE_RULE_CATALOG
            .iter()
            .find(|slice| slice.phase() == phase)
            .map(|slice| slice.rows())
    }

    pub(crate) fn selected_stage_catalog_contains(
        phase: OptimizationExecutionPhase,
        optimization: Optimization,
    ) -> bool {
        selected_stage_rule_rows(phase).is_some_and(|rows| rows.contains(optimization))
    }
}
pub(crate) mod condition_state {
    use std::collections::{BTreeSet, VecDeque};

    use crate::rewrites::block_edges::{block_instructions, terminator_successors};
    use register_model::{RegisterOperandAccess, RegisterUnitId};
    use selected_instructions::{
        SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionKind,
        VirtualRegisterId, terminator_instruction,
    };
    use semantic_vocabulary::IntegerValue;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ConditionStateError {
        Use,
        Producer,
        Literal,
    }

    fn literal_bits(value: IntegerValue) -> Option<u64> {
        match value {
            IntegerValue::Signed(value) => i64::try_from(value).ok().map(|value| value as u64),
            IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
        }
    }

    pub(crate) fn immediate_bits(value: IntegerValue) -> Option<u64> {
        match value {
            IntegerValue::Signed(value) => u64::try_from(value).ok(),
            IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
        }
    }

    pub(crate) fn materialized_bits(
        function: &SelectedFunction,
        register: VirtualRegisterId,
    ) -> Result<u64, ConditionStateError> {
        let mut producers = function.blocks.iter().flat_map(|block| {
            block_instructions(block).filter(|instruction| {
                instruction.operands.iter().any(|operand| {
                    operand.access != RegisterOperandAccess::Use
                        && operand.virtual_register == register
                })
            })
        });
        let producer = producers.next().ok_or(ConditionStateError::Producer)?;
        if producers.next().is_some() {
            return Err(ConditionStateError::Producer);
        }
        let SelectedInstructionKind::MaterializeI64 { value } = producer.kind else {
            return Err(ConditionStateError::Producer);
        };
        if producer.operands.len() != 1
            || producer.operands[0].operand != 0
            || producer.operands[0].access != RegisterOperandAccess::Def
            || producer.operands[0].virtual_register != register
            || producer.operands[0].fixed_view.is_some()
            || producer.operands[0].tied_to.is_some()
            || producer.operands[0].early_clobber
            || !producer.implicit_uses.is_empty()
            || !producer.implicit_defs.is_empty()
            || !producer.clobbers.is_empty()
        {
            return Err(ConditionStateError::Producer);
        }
        literal_bits(value).ok_or(ConditionStateError::Literal)
    }

    fn plain_use(
        instruction: &SelectedInstruction,
        position: usize,
    ) -> Result<VirtualRegisterId, ConditionStateError> {
        let operand = instruction
            .operands
            .get(position)
            .ok_or(ConditionStateError::Use)?;
        if operand.operand != position as u16
            || operand.access != RegisterOperandAccess::Use
            || operand.fixed_view.is_some()
            || operand.tied_to.is_some()
            || operand.early_clobber
        {
            return Err(ConditionStateError::Use);
        }
        Ok(operand.virtual_register)
    }

    pub(crate) fn constant_operands(
        function: &SelectedFunction,
        compare: &SelectedInstruction,
    ) -> Result<(u64, u64), ConditionStateError> {
        match compare.kind {
            SelectedInstructionKind::CompareI64 => {
                if compare.operands.len() != 2 {
                    return Err(ConditionStateError::Use);
                }
                let left = plain_use(compare, 0)?;
                let right = plain_use(compare, 1)?;
                if left == right {
                    return Ok((0, 0));
                }
                Ok((
                    materialized_bits(function, left)?,
                    materialized_bits(function, right)?,
                ))
            }
            SelectedInstructionKind::CompareI64Immediate { immediate } => {
                if compare.operands.len() != 1 {
                    return Err(ConditionStateError::Use);
                }
                let left = plain_use(compare, 0)?;
                let immediate = immediate_bits(immediate).ok_or(ConditionStateError::Literal)?;
                Ok((materialized_bits(function, left)?, immediate))
            }
            SelectedInstructionKind::CompareI64Zero => {
                if compare.operands.len() != 1 {
                    return Err(ConditionStateError::Use);
                }
                let left = plain_use(compare, 0)?;
                Ok((materialized_bits(function, left)?, 0))
            }
            _ => Err(ConditionStateError::Use),
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ConditionOperandResolution {
        TwoRegisterOperands,
        RegisterAndKindImmediate,
        RegisterAndZeroBound,
    }

    pub(crate) fn resolved_operands(
        function: &SelectedFunction,
        producer: &SelectedInstruction,
        resolution: ConditionOperandResolution,
    ) -> Result<(u64, u64), ConditionStateError> {
        let grammar_kind = matches!(
            (resolution, producer.kind),
            (
                ConditionOperandResolution::TwoRegisterOperands,
                SelectedInstructionKind::CompareI64
            ) | (
                ConditionOperandResolution::RegisterAndKindImmediate,
                SelectedInstructionKind::CompareI64Immediate { .. },
            ) | (
                ConditionOperandResolution::RegisterAndZeroBound,
                SelectedInstructionKind::CompareI64Zero,
            )
        );
        if !grammar_kind {
            return Err(ConditionStateError::Use);
        }
        constant_operands(function, producer)
    }

    pub(crate) fn resolved_unit_surface(
        flag_uses: &[RegisterUnitId],
        plain_uses: &[RegisterUnitId],
        consumer: &SelectedInstruction,
        rewritten: &register_model::RegisterInstructionConstraint,
    ) -> bool {
        !flag_uses.is_empty()
            && plain_uses
                .iter()
                .all(|unit| rewritten.implicit_uses.contains(unit))
            && rewritten.implicit_defs == consumer.implicit_defs
            && rewritten.clobbers == consumer.clobbers
            && rewritten
                .implicit_uses
                .iter()
                .all(|unit| consumer.implicit_uses.contains(unit))
    }
    pub(crate) type EventSite = (usize, usize);

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum ReachingEvent {
        Unknown,
        At(EventSite),
    }

    pub(crate) fn instruction_at(
        function: &SelectedFunction,
        site: EventSite,
    ) -> &SelectedInstruction {
        let block = &function.blocks[site.0];
        if site.1 == block.instructions.len() {
            terminator_instruction(&block.terminator)
        } else {
            &block.instructions[site.1]
        }
    }

    pub(crate) fn adjacency(function: &SelectedFunction) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
        let block_index = |id: SelectedBlockId| function.block_index(id);
        let successors: Vec<Vec<usize>> = function
            .blocks
            .iter()
            .map(|block| {
                terminator_successors(&block.terminator)
                    .iter()
                    .filter_map(|successor| block_index(successor.block))
                    .collect()
            })
            .collect();
        let mut predecessors = vec![Vec::new(); function.blocks.len()];
        for (source, targets) in successors.iter().enumerate() {
            for &target in targets {
                predecessors[target].push(source);
            }
        }
        (successors, predecessors)
    }

    pub(crate) fn entry_index(function: &SelectedFunction) -> Option<usize> {
        function.block_index(function.entry_block)
    }

    pub(crate) fn backward_cone(predecessors: &[Vec<usize>], target: usize) -> Vec<bool> {
        let mut cone = vec![false; predecessors.len()];
        cone[target] = true;
        let mut frontier = vec![target];
        while let Some(current) = frontier.pop() {
            for &predecessor in &predecessors[current] {
                if !cone[predecessor] {
                    cone[predecessor] = true;
                    frontier.push(predecessor);
                }
            }
        }
        cone
    }

    pub(crate) fn reaching_event(
        function: &SelectedFunction,
        entry: usize,
        successors: &[Vec<usize>],
        cone: &[bool],
        block_index: usize,
        read_position: usize,
        unit: RegisterUnitId,
    ) -> Result<EventSite, ConditionStateError> {
        let sites = reaching_events(
            function,
            entry,
            successors,
            cone,
            block_index,
            read_position,
            unit,
        )?;
        if sites.len() == 1
            && let Some(&site) = sites.iter().next()
        {
            return Ok(site);
        }
        Err(ConditionStateError::Use)
    }

    pub(crate) fn reaching_events(
        function: &SelectedFunction,
        entry: usize,
        successors: &[Vec<usize>],
        cone: &[bool],
        block_index: usize,
        read_position: usize,
        unit: RegisterUnitId,
    ) -> Result<BTreeSet<EventSite>, ConditionStateError> {
        let block = &function.blocks[block_index];
        if let Some(position) =
            block.instructions[..read_position]
                .iter()
                .rposition(|instruction| {
                    instruction.implicit_defs.contains(&unit)
                        || instruction.clobbers.contains(&unit)
                })
        {
            return Ok(BTreeSet::from([(block_index, position)]));
        }
        let last_event: Vec<Option<EventSite>> = function
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                block_instructions(block)
                    .enumerate()
                    .filter(|(_, instruction)| {
                        instruction.implicit_defs.contains(&unit)
                            || instruction.clobbers.contains(&unit)
                    })
                    .map(|(position, _)| (index, position))
                    .last()
            })
            .collect();
        let mut entry_events = vec![BTreeSet::new(); function.blocks.len()];
        entry_events[entry].insert(ReachingEvent::Unknown);
        let mut pending: VecDeque<usize> =
            (0..function.blocks.len()).filter(|&b| cone[b]).collect();
        while let Some(current) = pending.pop_front() {
            let contribution = match last_event[current] {
                Some(site) => BTreeSet::from([ReachingEvent::At(site)]),
                None => entry_events[current].clone(),
            };
            if contribution.is_empty() {
                continue;
            }
            for &successor in &successors[current] {
                if !cone[successor] {
                    continue;
                }
                let before = entry_events[successor].len();
                entry_events[successor].extend(contribution.iter().copied());
                if entry_events[successor].len() != before {
                    pending.push_back(successor);
                }
            }
        }
        if entry_events[block_index].is_empty()
            || entry_events[block_index].contains(&ReachingEvent::Unknown)
        {
            return Err(ConditionStateError::Use);
        }
        Ok(entry_events[block_index]
            .iter()
            .map(|event| match event {
                ReachingEvent::At(site) => *site,
                ReachingEvent::Unknown => unreachable!(),
            })
            .collect())
    }
}
pub(crate) use crate::address_and_load_rewrites::load_forwarding;
pub(crate) use crate::branch_and_compare_rewrites::constant_boolean;
pub(crate) use crate::branch_and_compare_rewrites::constant_branch;
pub(crate) use crate::branch_and_compare_rewrites::dead_compare;
pub(crate) use crate::copy_and_extension_rewrites::copied_call_operand;
pub(crate) use crate::copy_and_extension_rewrites::copy_removal;
pub(crate) use crate::literal_fold_rewrites::literal_folds;
pub(crate) mod place_storage {
    use selected_instructions::{
        LocalStorageSlotId, SelectedCasePayloadTransport, SelectedFunction, SelectedMemoryAccess,
        SelectedMemoryAccessRole, SelectedValueTransport, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
    use terminal_psi::StructuralPlaceDeclaration;

    use crate::rewrites::block_edges::terminator_successors;
    use crate::rewrites::condition_state::materialized_bits;

    pub(crate) fn structural_place_declarations(
        function: &SelectedFunction,
    ) -> &[StructuralPlaceDeclaration] {
        function
            .structural
            .as_ref()
            .map_or(&[], |contract| contract.structural_places.as_slice())
    }

    pub(crate) fn local_slot_is_place_storage(
        slot: LocalStorageSlotId,
        place: PlaceId,
        structural_places: &[StructuralPlaceDeclaration],
    ) -> bool {
        match slot {
            LocalStorageSlotId::StructuralParameter { place: slot_place }
            | LocalStorageSlotId::StructuralBlockParameter {
                place: slot_place, ..
            } => slot_place == place,
            LocalStorageSlotId::Structural {
                operation,
                place: slot_place,
            } => {
                slot_place == place
                    && structural_places.iter().any(|declaration| {
                        declaration.id == place
                            && matches!(
                                declaration.kind,
                                StructuralPlaceKind::OperationResult { producer, .. }
                                    if producer == operation
                            )
                    })
            }
            LocalStorageSlotId::Spill { .. }
            | LocalStorageSlotId::Boundary { .. }
            | LocalStorageSlotId::EdgeSnapshot { .. }
            | LocalStorageSlotId::StructuralCallArgument { .. }
            | LocalStorageSlotId::InvocationArgument { .. }
            | LocalStorageSlotId::InvocationResult { .. }
            | LocalStorageSlotId::BoundaryResult { .. } => false,
        }
    }
    pub(crate) fn extent_intersects(
        byte_offset: u32,
        byte_count: u32,
        dynamic_reach: bool,
        access: &SelectedMemoryAccess,
    ) -> bool {
        if dynamic_reach {
            return u64::from(byte_offset)
                < u64::from(access.byte_offset) + u64::from(access.byte_count);
        }
        u64::from(access.byte_offset) < u64::from(byte_offset) + u64::from(byte_count)
            && u64::from(byte_offset) < u64::from(access.byte_offset) + u64::from(access.byte_count)
    }

    pub(crate) fn resolved_sequence_position(
        access: &SelectedMemoryAccess,
        function: &SelectedFunction,
    ) -> Option<u64> {
        match access.role {
            SelectedMemoryAccessRole::ReadByteSequence { index, .. }
            | SelectedMemoryAccessRole::WriteByteSequence { index, .. } => {
                return constant_index(function, index)
                    .and_then(|landed| u64::from(access.byte_offset).checked_add(landed));
            }
            SelectedMemoryAccessRole::ReadIndexedPrimitive { .. }
            | SelectedMemoryAccessRole::WriteIndexedPrimitive { .. }
            | SelectedMemoryAccessRole::WriteIndexedStructural { .. } => {
                return function
                    .memory_accesses
                    .iter()
                    .filter(|element| {
                        element.instruction == access.instruction
                            && element.origin == access.origin
                            && element.place == access.place
                    })
                    .filter_map(|element| match element.role {
                        SelectedMemoryAccessRole::ReadIndexedPrimitive {
                            index, stride, ..
                        }
                        | SelectedMemoryAccessRole::WriteIndexedPrimitive {
                            index, stride, ..
                        }
                        | SelectedMemoryAccessRole::WriteIndexedStructural {
                            index, stride, ..
                        } => Some((index, stride)),
                        _ => None,
                    })
                    .try_fold(u64::from(access.byte_offset), |landed, (index, stride)| {
                        landed.checked_add(
                            constant_index(function, index)?.checked_mul(u64::from(stride))?,
                        )
                    });
            }
            _ => {}
        }
        None
    }

    pub(crate) fn extent_reached_by(
        byte_offset: u32,
        byte_count: u32,
        dynamic_reach: bool,
        access: &SelectedMemoryAccess,
        function: &SelectedFunction,
    ) -> bool {
        if let Some(position) = resolved_sequence_position(access, function) {
            let start = u64::from(byte_offset);
            return if dynamic_reach {
                position >= start
            } else {
                position >= start && position < start + u64::from(byte_count)
            };
        }
        if dynamic_reach {
            return true;
        }
        u64::from(access.byte_offset) < u64::from(byte_offset) + u64::from(byte_count)
    }

    pub(crate) fn constant_index(
        function: &SelectedFunction,
        index: semantic_vocabulary::ValueId,
    ) -> Option<u64> {
        let mut carriers = function.virtual_registers.iter().filter(|register| {
            matches!(
                register.origin,
                VirtualRegisterOrigin::InstructionResult { source_value, .. } if source_value == index
            )
        });
        let carrier = carriers.next()?;
        if carriers.next().is_some() {
            return None;
        }
        let landed = materialized_bits(function, carrier.id).ok()?;
        if transport_defines(function, carrier.id) {
            return None;
        }
        Some(landed)
    }

    pub(crate) fn transport_defines(
        function: &SelectedFunction,
        register: VirtualRegisterId,
    ) -> bool {
        for block in &function.blocks {
            for successor in terminator_successors(&block.terminator) {
                if successor.bindings.iter().any(|binding| {
                    matches!(
                        binding.transport,
                        SelectedValueTransport::Registers { parameter, .. } if parameter == register
                    )
                }) {
                    return true;
                }
                if let Some(case) = &successor.structural_case
                    && case.payloads.iter().any(|payload| {
                        matches!(
                            payload.transport,
                            SelectedCasePayloadTransport::Unmaterialized { parameter }
                                | SelectedCasePayloadTransport::Registers { parameter, .. }
                                if parameter == register
                        )
                    })
                {
                    return true;
                }
            }
        }
        false
    }
}
pub(crate) mod pre_allocation {
    pub(crate) mod catalog {
        use optimization_core::{
            Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch,
        };

        use super::PreAllocationPolicy;
        use super::execution::{
            PreAllocationCandidateSurface, PreAllocationRuleAdmission, address_fold_candidate,
            constant_boolean_candidate, constant_branch_candidate, copied_call_operand_candidate,
            copy_removal_candidate, dead_compare_candidate, equivalent_compare_candidate,
            extension_removal_candidate, is_boolean_reader, is_branch_reader, is_call_consumer,
            is_compare_publisher, is_copy_i64, is_extension_candidate, is_fold_consumer,
            is_load_candidate, redundant_compare_candidate, stored_load_forwarding_candidate,
        };

        #[derive(Debug, Clone, Copy)]
        pub(crate) struct PreAllocationRuleCatalogPayload {
            policy: PreAllocationPolicy,
            admission: PreAllocationRuleAdmission,
        }

        impl PreAllocationRuleCatalogPayload {
            receipt_accessor_rows!(PreAllocationRuleCatalogPayload {
                policy: PreAllocationPolicy,
            });

            pub const fn admission(&self) -> &PreAllocationRuleAdmission {
                &self.admission
            }
        }

        pub(crate) type PreAllocationRuleCatalogEntry =
            OptimizationCatalogDescriptor<PreAllocationRuleCatalogPayload>;

        const fn row(
            optimization: Optimization,
            policy: PreAllocationPolicy,
            admission: PreAllocationRuleAdmission,
        ) -> PreAllocationRuleCatalogEntry {
            PreAllocationRuleCatalogEntry::new(
                optimization,
                PreAllocationRuleCatalogPayload { policy, admission },
            )
        }

        pub(crate) const PRE_ALLOCATION_RULE_CATALOG: [PreAllocationRuleCatalogEntry; 10] = [
            row(
                Optimization::SelectedSameBlockCopyI64RemovalV1,
                PreAllocationPolicy::SAME_BLOCK_COPY_I64_V1,
                PreAllocationRuleAdmission::new(
                    PreAllocationCandidateSurface::body(is_copy_i64),
                    copy_removal_candidate,
                ),
            ),
            row(
                Optimization::SelectedRedundantExtensionRemovalV1,
                PreAllocationPolicy::REDUNDANT_EXTENSION_V1,
                PreAllocationRuleAdmission::new(
                    PreAllocationCandidateSurface::body(is_extension_candidate),
                    extension_removal_candidate,
                ),
            ),
            row(
                Optimization::SelectedAddressOffsetFoldV1,
                PreAllocationPolicy::ADDRESS_FOLD_V1,
                PreAllocationRuleAdmission::new(
                    PreAllocationCandidateSurface::body(is_fold_consumer),
                    address_fold_candidate,
                ),
            ),
            row(
                Optimization::SelectedConstantBooleanFoldV1,
                PreAllocationPolicy::CONSTANT_BOOLEAN_V1,
                PreAllocationRuleAdmission::new(
                    PreAllocationCandidateSurface::body(is_boolean_reader),
                    constant_boolean_candidate,
                ),
            ),
            row(
                Optimization::SelectedConstantBranchFoldV1,
                PreAllocationPolicy::CONSTANT_BRANCH_V1,
                PreAllocationRuleAdmission::new(
                    PreAllocationCandidateSurface::terminator(is_branch_reader),
                    constant_branch_candidate,
                ),
            ),
            row(
                Optimization::SelectedDeadCompareRemovalV1,
                PreAllocationPolicy::DEAD_COMPARE_V1,
                PreAllocationRuleAdmission::new(
                    PreAllocationCandidateSurface::body(is_compare_publisher),
                    dead_compare_candidate,
                ),
            ),
            row(
                Optimization::SelectedRedundantCompareRemovalV1,
                PreAllocationPolicy::REDUNDANT_COMPARE_V1,
                PreAllocationRuleAdmission::new(
                    PreAllocationCandidateSurface::body(is_compare_publisher),
                    redundant_compare_candidate,
                ),
            ),
            row(
                Optimization::SelectedEquivalentCompareRemovalV1,
                PreAllocationPolicy::EQUIVALENT_COMPARE_V1,
                PreAllocationRuleAdmission::new(
                    PreAllocationCandidateSurface::body(is_compare_publisher),
                    equivalent_compare_candidate,
                ),
            ),
            row(
                Optimization::SelectedCopiedCallOperandFoldV1,
                PreAllocationPolicy::COPIED_CALL_OPERAND_V1,
                PreAllocationRuleAdmission::new(
                    PreAllocationCandidateSurface::body(is_call_consumer),
                    copied_call_operand_candidate,
                ),
            ),
            row(
                Optimization::SelectedStoredLoadForwardingV1,
                PreAllocationPolicy::STORED_LOAD_FORWARDING_V1,
                PreAllocationRuleAdmission::new(
                    PreAllocationCandidateSurface::body(is_load_candidate),
                    stored_load_forwarding_candidate,
                ),
            ),
        ];

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum PreAllocationRuleCatalogError {
            WrongPhase(OptimizationPhaseMismatch),
            MissingSelection,
            UnsupportedSelection(Optimization),
        }
    }
    pub(crate) mod execution {
        use crate::{StoredLoadForwardingError, forward_selected_stored_load};
        use selected_instruction_analyses::SelectedProgramView;
        use std::collections::BTreeSet;

        use crate::analyses::prepared_machine_effect_catalog;
        use crate::rewrites::condition_state::adjacency;
        use crate::rewrites::{
            SelectedFunction, address_fold_measured_steps, constant_boolean_measured_steps,
            constant_boolean_surface_sizes, constant_branch_measured_steps,
            constant_branch_surface_sizes, copied_call_operand_measured_steps,
            copy_removal_measured_steps, dead_compare_measured_steps,
            equivalent_compare_measured_steps, fold_selected_address,
            fold_selected_constant_boolean, fold_selected_constant_branch,
            fold_selected_copied_call_operand, redundant_compare_measured_steps,
            redundant_extension_measured_steps, remove_dead_compare, remove_equivalent_compare,
            remove_redundant_compare, remove_selected_copy, remove_selected_redundant_extension,
        };
        use crate::{
            AddressFoldError, ConstantBooleanError, ConstantBranchError, CopiedCallOperandError,
            CopyRemovalError, DeadCompareError, EquivalentCompareError,
            OptimizedPreAllocationCustodyError, PreAllocationPolicy, RedundantCompareError,
            RedundantExtensionError, SelectedProgramRef, StagedOptimizedAllocationLegality,
            StagedOptimizedAllocationLegalityCustodyReceipt, StagedOptimizedPreAllocationAttempt,
            StagedOptimizedPreAllocationAttemptReceipt,
            StagedOptimizedPreAllocationIterationReceipt, StagedOptimizedPreAllocationStep,
            StagedPreAllocationOptimizationCustodyReceipt, StagedPreAllocationOptimizationRun,
            ValidatedPreAllocationTransformation,
        };
        use crate::{
            analyze_allocation_legality, analyze_live_ranges, analyze_liveness,
            resolve_pre_allocation_rules,
        };
        use optimization_core::{
            OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
            PreAllocationOptimizationCompletionIdentity,
        };
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::RegisterOperandAccess;
        use selected_instructions::{
            SelectedInstruction, SelectedInstructionKind, SelectedInstructionPlan,
            terminator_instruction,
        };

        use super::PRE_ALLOCATION_RULE_CATALOG;

        pub(crate) enum CandidateAttempt {
            Clean {
                declined: usize,
                evaluated: usize,
                validation_steps: u64,
            },
            Applied {
                transformation: ValidatedPreAllocationTransformation,
                declined: usize,
                evaluated: usize,
                validation_steps: u64,
            },
        }

        impl CandidateAttempt {
            pub(crate) fn clean(declined: usize, evaluated: usize, validation_steps: u64) -> Self {
                Self::Clean {
                    declined,
                    evaluated,
                    validation_steps,
                }
            }

            pub(crate) fn applied(
                transformation: ValidatedPreAllocationTransformation,
                declined: usize,
                evaluated: usize,
                validation_steps: u64,
            ) -> Self {
                Self::Applied {
                    transformation,
                    declined,
                    evaluated,
                    validation_steps,
                }
            }
        }

        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub(crate) enum CandidatePosition {
            Body(usize),
            Terminator,
        }

        #[derive(Clone, Copy)]
        pub(crate) struct CandidateSite {
            function_index: usize,
            block_index: usize,
            position: CandidatePosition,
        }

        impl CandidateSite {
            fn instruction(
                self,
                plan: &SelectedInstructionPlan,
            ) -> Result<&SelectedInstruction, OptimizedPreAllocationCustodyError> {
                let block = plan
                    .functions
                    .get(self.function_index)
                    .and_then(|function| function.blocks.get(self.block_index))
                    .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch)?;
                match self.position {
                    CandidatePosition::Body(position) => block
                        .instructions
                        .get(position)
                        .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch),
                    CandidatePosition::Terminator => Ok(terminator_instruction(&block.terminator)),
                }
            }
        }

        #[derive(Debug, Clone, Copy)]
        pub(crate) enum PreAllocationCandidateSurface {
            Body(fn(SelectedInstructionKind) -> bool),
            Terminator(fn(SelectedInstructionKind) -> bool),
        }

        impl PreAllocationCandidateSurface {
            pub(crate) const fn body(predicate: fn(SelectedInstructionKind) -> bool) -> Self {
                Self::Body(predicate)
            }

            pub(crate) const fn terminator(predicate: fn(SelectedInstructionKind) -> bool) -> Self {
                Self::Terminator(predicate)
            }

            fn admits(
                self,
                position: CandidatePosition,
                instruction: &SelectedInstruction,
            ) -> bool {
                match (self, position) {
                    (Self::Body(predicate), CandidatePosition::Body(_)) => {
                        predicate(instruction.kind)
                    }
                    (Self::Terminator(predicate), CandidatePosition::Terminator) => {
                        predicate(instruction.kind)
                    }
                    _ => false,
                }
            }
        }

        #[derive(Debug, Clone, Copy)]
        pub(crate) struct PreAllocationRuleAdmission {
            surface: PreAllocationCandidateSurface,
            attempt: PreAllocationSiteAttempt,
        }

        pub(crate) type PreAllocationSiteAttempt =
            fn(
                SelectedProgramRef<'_>,
                CandidateSite,
                &PreAllocationAdmissionContext<'_>,
            ) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError>;

        impl PreAllocationRuleAdmission {
            pub(crate) const fn new(
                surface: PreAllocationCandidateSurface,
                attempt: PreAllocationSiteAttempt,
            ) -> Self {
                Self { surface, attempt }
            }

            pub(crate) fn admits(
                &self,
                position: CandidatePosition,
                instruction: &SelectedInstruction,
            ) -> bool {
                self.surface.admits(position, instruction)
            }

            pub(crate) fn attempt(&self) -> PreAllocationSiteAttempt {
                self.attempt
            }
        }

        pub(crate) struct PreAllocationAdmissionContext<'environment> {
            pub(crate) environment: &'environment ValidatedTargetRegisterEnvironment,
            pub(crate) budget: OptimizationWorkBudget,
        }

        impl<'environment> PreAllocationAdmissionContext<'environment> {
            pub(crate) const fn new(
                environment: &'environment ValidatedTargetRegisterEnvironment,
                budget: OptimizationWorkBudget,
            ) -> Self {
                Self {
                    environment,
                    budget,
                }
            }
        }

        enum CandidateDisposition {
            Ready,
            Declined {
                revision: usize,
                candidates: usize,
                declined: usize,
            },
            Retired,
        }

        struct PendingCandidate {
            family_index: usize,
            site: CandidateSite,
            disposition: CandidateDisposition,
        }

        impl PendingCandidate {
            fn order(&self) -> (usize, usize, usize, CandidatePosition) {
                (
                    self.family_index,
                    self.site.function_index,
                    self.site.block_index,
                    self.site.position,
                )
            }
        }

        struct CandidateFrontier {
            pending: Vec<PendingCandidate>,
            function_revisions: Vec<usize>,
            policy: PreAllocationPolicy,
        }

        impl CandidateFrontier {
            fn new(policy: PreAllocationPolicy, plan: &SelectedInstructionPlan) -> Self {
                let mut frontier = Self {
                    pending: Vec::new(),
                    function_revisions: vec![1; plan.functions.len()],
                    policy,
                };
                for (family_index, row) in PRE_ALLOCATION_RULE_CATALOG.iter().enumerate() {
                    if !policy.contains(row.payload().policy()) {
                        continue;
                    }
                    let admission = row.payload().admission();
                    for (function_index, function) in plan.functions.iter().enumerate() {
                        for (block_index, block) in function.blocks.iter().enumerate() {
                            for (position, instruction) in block.instructions.iter().enumerate() {
                                let site = CandidateSite {
                                    function_index,
                                    block_index,
                                    position: CandidatePosition::Body(position),
                                };
                                if admission.admits(site.position, instruction) {
                                    frontier.pending.push(PendingCandidate {
                                        family_index,
                                        site,
                                        disposition: CandidateDisposition::Ready,
                                    });
                                }
                            }
                            let site = CandidateSite {
                                function_index,
                                block_index,
                                position: CandidatePosition::Terminator,
                            };
                            if admission
                                .admits(site.position, terminator_instruction(&block.terminator))
                            {
                                frontier.pending.push(PendingCandidate {
                                    family_index,
                                    site,
                                    disposition: CandidateDisposition::Ready,
                                });
                            }
                        }
                    }
                }
                frontier
            }

            fn next(&self) -> Option<usize> {
                self.pending
                    .iter()
                    .position(|candidate| match candidate.disposition {
                        CandidateDisposition::Ready => true,
                        CandidateDisposition::Declined { revision, .. } => {
                            revision != self.function_revisions[candidate.site.function_index]
                        }
                        CandidateDisposition::Retired => false,
                    })
            }

            fn committed(
                &mut self,
                site: CandidateSite,
                previous_body_count: usize,
                plan: &SelectedInstructionPlan,
            ) -> Result<(), OptimizedPreAllocationCustodyError> {
                let function = plan
                    .functions
                    .get(site.function_index)
                    .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch)?;
                let block = function
                    .blocks
                    .get(site.block_index)
                    .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch)?;
                let removed = previous_body_count
                    .checked_sub(block.instructions.len())
                    .filter(|removed| *removed <= 1)
                    .ok_or(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch)?;
                let revision = &mut self.function_revisions[site.function_index];
                *revision = revision
                    .checked_add(1)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                for candidate in &mut self.pending {
                    if candidate.site.function_index != site.function_index {
                        continue;
                    }
                    if removed != 0 && candidate.site.block_index == site.block_index {
                        let CandidatePosition::Body(removed_position) = site.position else {
                            return Err(
                                OptimizedPreAllocationCustodyError::CandidateFrontierMismatch,
                            );
                        };
                        if let CandidatePosition::Body(position) = &mut candidate.site.position {
                            if *position == removed_position {
                                candidate.disposition = CandidateDisposition::Retired;
                                continue;
                            }
                            if *position > removed_position {
                                *position -= removed;
                            }
                        }
                    }
                    let admitted = PRE_ALLOCATION_RULE_CATALOG[candidate.family_index]
                        .payload()
                        .admission()
                        .admits(candidate.site.position, candidate.site.instruction(plan)?);
                    if !admitted {
                        candidate.disposition = CandidateDisposition::Retired;
                    }
                }
                self.pending.retain(|candidate| {
                    !matches!(candidate.disposition, CandidateDisposition::Retired)
                });
                if removed == 0 {
                    let instruction = site.instruction(plan)?;
                    for (family_index, row) in PRE_ALLOCATION_RULE_CATALOG.iter().enumerate() {
                        if !self.policy.contains(row.payload().policy())
                            || !row.payload().admission().admits(site.position, instruction)
                        {
                            continue;
                        }
                        let candidate = PendingCandidate {
                            family_index,
                            site,
                            disposition: CandidateDisposition::Ready,
                        };
                        if let Err(position) = self
                            .pending
                            .binary_search_by_key(&candidate.order(), PendingCandidate::order)
                        {
                            self.pending.insert(position, candidate);
                        }
                    }
                }
                Ok(())
            }
        }

        macro_rules! retryable_candidate_error {
            ($error:ident, $binding:ident) => {
                $binding @ ($error::WorkBudgetExceeded
                    | $error::IdentityOverflow
                    | $error::ReplayMismatch)
            };
        }

        macro_rules! pre_allocation_candidate {
            (
                $(#[$meta:meta])*
                $name:ident,
                $current:ident, $function_index:ident, $function:ident, $instruction:ident,
                $context:ident,
                $(setup: $setup:stmt;)*
                audit: $audit:expr,
                reach: $reach:expr,
                transformation: $transformation:ident,
                refusal: $refusal:ident,
                custody: $custody:ident $(,)?
            ) => {
                $(#[$meta])*
                pub(crate) fn $name(
                    current: SelectedProgramRef<'_>,
                    site: CandidateSite,
                    context: &PreAllocationAdmissionContext<'_>,
                ) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
                    let ($function_index, $function, $instruction) = candidate_context(&current, site)?;
                    let $current = &current;
                    let $context = context;
                    $( $setup )*
                    let audit_cost = $audit;
                    let charged = charge(0, audit_cost)?;
                    match $reach {
                        Ok(removal) => Ok(CandidateAttempt::applied(
                            ValidatedPreAllocationTransformation::$transformation(removal),
                            0,
                            1,
                            charge(charged, audit_cost)?,
                        )),
                        Err(retryable_candidate_error!($refusal, error)) => {
                            Err(OptimizedPreAllocationCustodyError::$custody(error))
                        }
                        Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
                    }
                }
            };
        }

        pub(crate) fn is_copy_i64(kind: SelectedInstructionKind) -> bool {
            kind == SelectedInstructionKind::CopyI64
        }

        pub(crate) fn is_load_candidate(kind: SelectedInstructionKind) -> bool {
            matches!(
                kind,
                SelectedInstructionKind::Load64 { .. }
                    | selected_instructions::load_width_triple_pattern!(..)
                    | SelectedInstructionKind::Load8Indexed
            )
        }

        pub(crate) fn is_extension_candidate(kind: SelectedInstructionKind) -> bool {
            matches!(
                kind,
                SelectedInstructionKind::ZeroExtendU8
                    | SelectedInstructionKind::ZeroExtendU16
                    | SelectedInstructionKind::ZeroExtendU32
                    | SelectedInstructionKind::SignExtendI8
                    | SelectedInstructionKind::SignExtendI16
                    | SelectedInstructionKind::SignExtendI32
            )
        }

        pub(crate) fn is_fold_consumer(kind: SelectedInstructionKind) -> bool {
            match kind {
                selected_instructions::load_width_triple_pattern!(..)
                | SelectedInstructionKind::Load64 { .. }
                | SelectedInstructionKind::AddressOffset { .. } => true,
                SelectedInstructionKind::Store { byte_size, .. } => {
                    matches!(byte_size, 1 | 2 | 4 | 8)
                }
                _ => false,
            }
        }

        pub(crate) fn is_boolean_reader(kind: SelectedInstructionKind) -> bool {
            matches!(
                kind,
                SelectedInstructionKind::MaterializeBooleanEqual
                    | SelectedInstructionKind::MaterializeBooleanU64LessThan
                    | SelectedInstructionKind::MaterializeBooleanI64LessThan
                    | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                    | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
            )
        }

        pub(crate) fn is_branch_reader(kind: SelectedInstructionKind) -> bool {
            matches!(
                kind,
                SelectedInstructionKind::ConditionalBranchNonZero
                    | SelectedInstructionKind::ConditionalBranchU64LessThan
                    | SelectedInstructionKind::ConditionalBranchI64LessThan
            )
        }

        pub(crate) fn is_compare_publisher(kind: SelectedInstructionKind) -> bool {
            matches!(
                kind,
                SelectedInstructionKind::CompareI64
                    | SelectedInstructionKind::CompareI64Immediate { .. }
                    | SelectedInstructionKind::CompareI64Zero
            )
        }

        fn published_flag_units(instruction: &SelectedInstruction) -> usize {
            instruction
                .implicit_defs
                .iter()
                .chain(instruction.clobbers.iter())
                .collect::<BTreeSet<&register_model::RegisterUnitId>>()
                .len()
        }

        fn operand_chain_length(
            instructions: &[SelectedInstruction],
            consumer_index: usize,
            operand_index: usize,
        ) -> usize {
            let mut register = match instructions[consumer_index].operands.get(operand_index) {
                Some(operand) if operand.access == RegisterOperandAccess::Use => {
                    operand.virtual_register
                }
                _ => return 0,
            };
            let mut position = consumer_index;
            let mut links = 0usize;
            while let Some(definition) = instructions[..position].iter().rposition(|instruction| {
                instruction.operands.iter().any(|operand| {
                    operand.access != RegisterOperandAccess::Use
                        && operand.virtual_register == register
                })
            }) {
                links += 1;
                match instructions[definition].operands.first() {
                    Some(next) if next.access == RegisterOperandAccess::Use => {
                        register = next.virtual_register;
                        position = definition;
                    }
                    _ => break,
                }
            }
            links
        }

        pub(crate) fn is_call_consumer(kind: SelectedInstructionKind) -> bool {
            matches!(kind, selected_instructions::selected_call_pattern!(..))
        }

        fn measure(plan: &SelectedInstructionPlan, virtual_registers: usize) -> usize {
            let mut total = virtual_registers
                + plan
                    .functions
                    .iter()
                    .map(|function| function.memory_accesses.len())
                    .sum::<usize>();
            for function in &plan.functions {
                for block in &function.blocks {
                    for (index, instruction) in block.instructions.iter().enumerate() {
                        if is_extension_candidate(instruction.kind) {
                            total += 1;
                        }
                        if is_fold_consumer(instruction.kind) {
                            total += operand_chain_length(&block.instructions, index, 0);
                        }
                        if is_call_consumer(instruction.kind) {
                            for operand_index in 0..instruction.operands.len() {
                                total +=
                                    operand_chain_length(&block.instructions, index, operand_index);
                            }
                        }
                        if is_boolean_reader(instruction.kind) {
                            total += instruction.implicit_uses.len();
                        }
                        if is_compare_publisher(instruction.kind) {
                            total += 1;
                        }
                    }
                    let terminator = terminator_instruction(&block.terminator);
                    if is_branch_reader(terminator.kind) {
                        total += terminator.implicit_uses.len();
                    }
                }
            }
            total
        }

        fn candidate_context<'a>(
            current: &'a SelectedProgramRef<'a>,
            site: CandidateSite,
        ) -> Result<
            (usize, &'a SelectedFunction, &'a SelectedInstruction),
            OptimizedPreAllocationCustodyError,
        > {
            let function_index = site.function_index;
            let function = &current.selected_plan().functions[function_index];
            let instruction = site.instruction(current.selected_plan())?;
            Ok((function_index, function, instruction))
        }

        pre_allocation_candidate!(
            #[doc = "Admit the current copy. Ordinary legality refusals decline the site;"]
            #[doc = "budget, identity and validator failures abort the run. Published"]
            #[doc = "measured-step costs charge the admission audit and, on commit, its"]
            #[doc = "independent validation."]
            copy_removal_candidate,
            current, function_index, function, instruction, context,
            audit: copy_removal_measured_steps(current.selected_plan(), function)
                .map_err(OptimizedPreAllocationCustodyError::CopyRemoval)?,
            reach: remove_selected_copy(
                current,
                function_index,
                instruction.id,
                context.environment,
                context.budget,
            ),
            transformation: CopyRemoval,
            refusal: CopyRemovalError,
            custody: CopyRemoval,
        );

        pre_allocation_candidate!(
            #[doc = "Admit the current extension under its own measured-step contract."]
            extension_removal_candidate,
            current, function_index, function, instruction, context,
            audit: redundant_extension_measured_steps(current.selected_plan(), function)
                .map_err(OptimizedPreAllocationCustodyError::RedundantExtension)?,
            reach: remove_selected_redundant_extension(
                current,
                function_index,
                instruction.id,
                context.environment,
                context.budget,
            ),
            transformation: RedundantExtension,
            refusal: RedundantExtensionError,
            custody: RedundantExtension,
        );

        pre_allocation_candidate!(
            #[doc = "Admit the current displacement-carrying consumer."]
            address_fold_candidate,
            current, function_index, function, instruction, context,
            audit: address_fold_measured_steps(current.selected_plan(), function)
                .map_err(OptimizedPreAllocationCustodyError::AddressFold)?,
            reach: fold_selected_address(
                current,
                function_index,
                instruction.id,
                context.environment,
                context.budget,
            ),
            transformation: AddressFold,
            refusal: AddressFoldError,
            custody: AddressFold,
        );

        pre_allocation_candidate!(
            #[doc = "Admit the current flag reader. The target flag universe bounds the"]
            #[doc = "reached compare's definitions in the pre-admission work charge."]
            constant_boolean_candidate,
            current, function_index, function, instruction, context,
            setup: let (flag_universe, materialize_surface) =
                constant_boolean_surface_sizes(context.environment)
                    .map_err(OptimizedPreAllocationCustodyError::ConstantBoolean)?;
            setup: let (successors, _) = adjacency(function);
            setup: let effect_catalog = prepared_machine_effect_catalog(
                context.environment.target(),
                context.environment.constraints(),
            )
            .map_err(OptimizedPreAllocationCustodyError::MachineEffects)?;
            audit: constant_boolean_measured_steps(
                current.selected_plan(),
                function,
                &successors,
                instruction.implicit_uses.len(),
                flag_universe,
                materialize_surface,
                flag_universe,
            )
            .map_err(OptimizedPreAllocationCustodyError::ConstantBoolean)?,
            reach: fold_selected_constant_boolean(
                current,
                function_index,
                instruction.id,
                context.environment,
                &effect_catalog,
                context.budget,
            ),
            transformation: ConstantBoolean,
            refusal: ConstantBooleanError,
            custody: ConstantBoolean,
        );

        pre_allocation_candidate!(
            #[doc = "Admit the current branch terminator with the target flag-universe bound."]
            constant_branch_candidate,
            current, function_index, function, instruction, context,
            setup: let (flag_universe, jump_surface) = constant_branch_surface_sizes(context.environment)
                .map_err(OptimizedPreAllocationCustodyError::ConstantBranch)?;
            setup: let (successors, _) = adjacency(function);
            setup: let effect_catalog = prepared_machine_effect_catalog(
                context.environment.target(),
                context.environment.constraints(),
            )
            .map_err(OptimizedPreAllocationCustodyError::MachineEffects)?;
            audit: constant_branch_measured_steps(
                current.selected_plan(),
                function,
                &successors,
                instruction.implicit_uses.len(),
                flag_universe,
                jump_surface,
                flag_universe,
            )
            .map_err(OptimizedPreAllocationCustodyError::ConstantBranch)?,
            reach: fold_selected_constant_branch(
                current,
                function_index,
                instruction.id,
                context.environment,
                &effect_catalog,
                context.budget,
            ),
            transformation: ConstantBranch,
            refusal: ConstantBranchError,
            custody: ConstantBranch,
        );

        pre_allocation_candidate!(
            #[doc = "Admit the current dead-compare proposal, charging its published flag units."]
            dead_compare_candidate,
            current, function_index, function, instruction, context,
            audit: dead_compare_measured_steps(
                current.selected_plan(),
                function,
                published_flag_units(instruction),
            )
            .map_err(OptimizedPreAllocationCustodyError::DeadCompare)?,
            reach: remove_dead_compare(
                current,
                function_index,
                instruction.id,
                context.environment,
                context.budget,
            ),
            transformation: DeadCompare,
            refusal: DeadCompareError,
            custody: DeadCompare,
        );

        pre_allocation_candidate!(
            #[doc = "Admit the current redundant-compare proposal."]
            redundant_compare_candidate,
            current, function_index, function, instruction, context,
            setup: let (successors, _) = adjacency(function);
            audit: redundant_compare_measured_steps(
                current.selected_plan(),
                function,
                &successors,
                published_flag_units(instruction),
            )
            .map_err(OptimizedPreAllocationCustodyError::RedundantCompare)?,
            reach: remove_redundant_compare(
                current,
                function_index,
                instruction.id,
                context.environment,
                context.budget,
            ),
            transformation: RedundantCompare,
            refusal: RedundantCompareError,
            custody: RedundantCompare,
        );

        pre_allocation_candidate!(
            #[doc = "Admit the current equivalent-compare proposal."]
            equivalent_compare_candidate,
            current, function_index, function, instruction, context,
            setup: let (successors, _) = adjacency(function);
            audit: equivalent_compare_measured_steps(
                current.selected_plan(),
                function,
                &successors,
                published_flag_units(instruction),
            )
            .map_err(OptimizedPreAllocationCustodyError::EquivalentCompare)?,
            reach: remove_equivalent_compare(
                current,
                function_index,
                instruction.id,
                context.environment,
                context.budget,
            ),
            transformation: EquivalentCompare,
            refusal: EquivalentCompareError,
            custody: EquivalentCompare,
        );

        fn charge(left: u64, right: u64) -> Result<u64, OptimizedPreAllocationCustodyError> {
            left.checked_add(right)
                .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)
        }

        fn pass_usage(
            evaluated: usize,
            validation_steps: u64,
            commits: usize,
        ) -> Result<OptimizationWorkUsage, OptimizedPreAllocationCustodyError> {
            let count = |value: usize| {
                u64::try_from(value).map_err(|_| OptimizedPreAllocationCustodyError::WorkOverflow)
            };
            let counted = count(evaluated)?;
            Ok(OptimizationWorkUsage {
                rule_evaluations: counted,
                candidates: counted,
                validation_steps,
                commits: count(commits)?,
                iterations: 0,
            })
        }

        pub(crate) fn add_usage(
            left: OptimizationWorkUsage,
            right: OptimizationWorkUsage,
        ) -> Result<OptimizationWorkUsage, OptimizedPreAllocationCustodyError> {
            crate::work_accounting::combine(left, right)
                .map_err(|_| OptimizedPreAllocationCustodyError::WorkOverflow)
        }

        pub(crate) fn ensure_pre_allocation_budget(
            usage: OptimizationWorkUsage,
            budget: OptimizationWorkBudget,
        ) -> Result<(), OptimizedPreAllocationCustodyError> {
            crate::work_accounting::admit(usage, budget, |required, budget| {
                OptimizedPreAllocationCustodyError::PreAllocationBudgetExceeded { required, budget }
            })
        }

        fn complete_transformation(
            transformation: ValidatedPreAllocationTransformation,
            declined: usize,
            evaluated: usize,
            source: &StagedOptimizedAllocationLegality,
        ) -> Result<StagedOptimizedPreAllocationStep, OptimizedPreAllocationCustodyError> {
            let environment = source.register_environment();
            let liveness = analyze_liveness(&transformation)
                .map_err(OptimizedPreAllocationCustodyError::Liveness)?;
            let ranges = analyze_live_ranges(&transformation, &liveness)
                .map_err(OptimizedPreAllocationCustodyError::LiveRanges)?;
            let legality = analyze_allocation_legality(
                &ranges,
                source.allocator_availability(),
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &environment.allocation_constraint_keys(),
            )
            .map_err(OptimizedPreAllocationCustodyError::AllocationLegality)?;
            let transition_count = legality.receipt().entry_transition_count();
            if transition_count != 0 {
                return Err(OptimizedPreAllocationCustodyError::RemainingTransitions {
                    count: transition_count,
                });
            }
            Ok(StagedOptimizedPreAllocationStep {
                transformation,
                liveness,
                ranges,
                legality,
                declined,
                evaluated,
            })
        }

        use super::super::operand_probes::last_definition_before;

        pub(crate) fn stored_load_forwarding_candidate(
            current: SelectedProgramRef<'_>,
            site: CandidateSite,
            context: &PreAllocationAdmissionContext<'_>,
        ) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
            let function_index = site.function_index;
            let instruction = site.instruction(current.selected_plan())?;
            let surface = current
                .selected_plan()
                .functions
                .iter()
                .try_fold(1u64, |total, function| {
                    let members = function.blocks.iter().try_fold(0u64, |total, block| {
                        total
                            .checked_add(block.instructions.len() as u64)?
                            .checked_add(1)
                    })?;
                    total
                        .checked_add(members)?
                        .checked_add(function.memory_accesses.len() as u64)?
                        .checked_add(function.virtual_registers.len() as u64)
                })
                .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
            let audit_cost = surface
                .checked_mul(surface)
                .and_then(|value| value.checked_mul(surface))
                .and_then(|value| value.checked_mul(2))
                .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
            let charged = charge(0, audit_cost)?;
            if charged > context.budget.validation_steps() {
                return Err(OptimizedPreAllocationCustodyError::StoredLoadForwarding(
                    StoredLoadForwardingError::WorkBudgetExceeded,
                ));
            }
            match forward_selected_stored_load(
                &current,
                function_index,
                instruction.id,
                context.environment,
                context.budget,
            ) {
                Ok(forwarding) => Ok(CandidateAttempt::applied(
                    ValidatedPreAllocationTransformation::StoredLoadForwarding(forwarding),
                    0,
                    1,
                    charged,
                )),
                Err(retryable_candidate_error!(StoredLoadForwardingError, error)) => Err(
                    OptimizedPreAllocationCustodyError::StoredLoadForwarding(error),
                ),
                Err(_) => Ok(CandidateAttempt::clean(1, 1, charged)),
            }
        }

        pub(crate) fn copied_call_operand_candidate(
            current: SelectedProgramRef<'_>,
            site: CandidateSite,
            context: &PreAllocationAdmissionContext<'_>,
        ) -> Result<CandidateAttempt, OptimizedPreAllocationCustodyError> {
            let function_index = site.function_index;
            let function = &current.selected_plan().functions[function_index];
            let block = &function.blocks[site.block_index];
            let instruction = site.instruction(current.selected_plan())?;
            let CandidatePosition::Body(position) = site.position else {
                return Err(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch);
            };
            let effect_catalog = prepared_machine_effect_catalog(
                context.environment.target(),
                context.environment.constraints(),
            )
            .map_err(OptimizedPreAllocationCustodyError::MachineEffects)?;
            let audit_cost =
                copied_call_operand_measured_steps(current.selected_plan(), function, block)
                    .map_err(OptimizedPreAllocationCustodyError::CopiedCallOperand)?;
            let mut evaluated = 0usize;
            let mut declined = 0usize;
            let mut validation_steps = 0u64;
            let mut tried = BTreeSet::new();
            for operand in instruction
                .operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Use)
            {
                let Some(producer_index) =
                    last_definition_before(&block.instructions, position, operand.virtual_register)
                else {
                    continue;
                };
                let producer = &block.instructions[producer_index];
                if producer.kind != SelectedInstructionKind::CopyI64 || !tried.insert(producer.id) {
                    continue;
                }
                evaluated += 1;
                validation_steps = charge(validation_steps, audit_cost)?;
                match fold_selected_copied_call_operand(
                    &current,
                    function_index,
                    producer.id,
                    instruction.id,
                    context.environment,
                    &effect_catalog,
                    context.budget,
                ) {
                    Ok(reroute) => {
                        validation_steps = charge(validation_steps, audit_cost)?;
                        return Ok(CandidateAttempt::applied(
                            ValidatedPreAllocationTransformation::CopiedCallOperand(reroute),
                            declined,
                            evaluated,
                            validation_steps,
                        ));
                    }
                    Err(retryable_candidate_error!(CopiedCallOperandError, error)) => {
                        return Err(OptimizedPreAllocationCustodyError::CopiedCallOperand(error));
                    }
                    Err(_) => {
                        declined += 1;
                    }
                }
            }
            Ok(CandidateAttempt::clean(
                declined,
                evaluated,
                validation_steps,
            ))
        }

        fn run_passes(
            source: &StagedOptimizedAllocationLegality,
            policy: PreAllocationPolicy,
            iteration_bound: usize,
        ) -> Result<
            (
                Vec<StagedOptimizedPreAllocationStep>,
                StagedOptimizedPreAllocationAttempt,
                OptimizationWorkUsage,
            ),
            OptimizedPreAllocationCustodyError,
        > {
            let budget = source.budget_per_pass();
            let context = PreAllocationAdmissionContext::new(source.register_environment(), budget);
            let mut usage = OptimizationWorkUsage {
                iterations: 1,
                ..OptimizationWorkUsage::default()
            };
            ensure_pre_allocation_budget(usage, budget)?;
            let mut steps = Vec::new();
            let mut previous_measure = iteration_bound;
            let mut current = SelectedProgramRef::new(source.selected());
            let mut frontier = CandidateFrontier::new(policy, current.selected_plan());
            while let Some(position) = frontier.next() {
                let pending = &frontier.pending[position];
                let site = pending.site;
                let previous_body_count = current.selected_plan().functions[site.function_index]
                    .blocks[site.block_index]
                    .instructions
                    .len();
                let admission = PRE_ALLOCATION_RULE_CATALOG[pending.family_index]
                    .payload()
                    .admission();
                let attempt = (admission.attempt())(current, site, &context)?;
                match attempt {
                    CandidateAttempt::Clean {
                        declined,
                        evaluated,
                        validation_steps,
                    } => {
                        usage = add_usage(usage, pass_usage(evaluated, validation_steps, 0)?)?;
                        ensure_pre_allocation_budget(usage, budget)?;
                        frontier.pending[position].disposition = CandidateDisposition::Declined {
                            revision: frontier.function_revisions[site.function_index],
                            candidates: evaluated,
                            declined,
                        };
                    }
                    CandidateAttempt::Applied {
                        transformation,
                        declined,
                        evaluated,
                        validation_steps,
                    } => {
                        if steps.len() >= iteration_bound {
                            return Err(
                                OptimizedPreAllocationCustodyError::PreAllocationIterationBoundExceeded {
                                    bound: iteration_bound,
                                },
                            );
                        }
                        usage = add_usage(usage, pass_usage(evaluated, validation_steps, 1)?)?;
                        ensure_pre_allocation_budget(usage, budget)?;
                        let step =
                            complete_transformation(transformation, declined, evaluated, source)?;
                        let current_measure = measure(
                            step.transformation.transformed(),
                            step.legality.receipt().virtual_register_count(),
                        );
                        if current_measure >= previous_measure {
                            return Err(
                                OptimizedPreAllocationCustodyError::PreAllocationMeasureMismatch {
                                    previous: previous_measure,
                                    current: current_measure,
                                },
                            );
                        }
                        previous_measure = current_measure;
                        frontier.committed(
                            site,
                            previous_body_count,
                            step.transformation.transformed(),
                        )?;
                        steps.push(step);
                        current = SelectedProgramRef::new(
                            &steps
                                .last()
                                .ok_or(
                                    OptimizedPreAllocationCustodyError::CandidateFrontierMismatch,
                                )?
                                .transformation,
                        );
                    }
                }
            }
            let mut candidates = 0usize;
            let mut declined = 0usize;
            for pending in &frontier.pending {
                let CandidateDisposition::Declined {
                    revision,
                    candidates: remaining,
                    declined: refusals,
                } = pending.disposition
                else {
                    return Err(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch);
                };
                if revision != frontier.function_revisions[pending.site.function_index] {
                    return Err(OptimizedPreAllocationCustodyError::CandidateFrontierMismatch);
                }
                candidates = candidates
                    .checked_add(remaining)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
                declined = declined
                    .checked_add(refusals)
                    .ok_or(OptimizedPreAllocationCustodyError::WorkOverflow)?;
            }
            let source_selected = current.selected_identity();
            Ok((
                steps,
                StagedOptimizedPreAllocationAttempt {
                    source_selected,
                    candidates,
                    declined,
                },
                usage,
            ))
        }

        pub(crate) fn execute_pre_allocation_optimizations(
            source: StagedOptimizedAllocationLegality,
            selections: OptimizationSelections,
            pre_allocation_selections: OptimizationSelections,
            policy: PreAllocationPolicy,
        ) -> Result<StagedPreAllocationOptimizationRun, OptimizedPreAllocationCustodyError>
        {
            let upstream = source.custody();
            let budget = source.budget_per_pass();
            let iteration_bound = measure(
                source.selected().selected_plan(),
                source.legality().receipt().virtual_register_count(),
            );
            let (steps, attempt, usage) = run_passes(&source, policy, iteration_bound)?;
            let custody = pre_allocation_custody_receipt(
                upstream,
                &selections,
                &pre_allocation_selections,
                policy,
                &source,
                &steps,
                &attempt,
                budget,
                usage,
                iteration_bound,
            );
            Ok(StagedPreAllocationOptimizationRun {
                source,
                selections,
                pre_allocation_selections,
                policy,
                steps,
                attempt,
                custody,
            })
        }

        pub(crate) fn validate_pre_allocation_optimization_custody(
            run: &StagedPreAllocationOptimizationRun,
        ) -> Result<StagedPreAllocationOptimizationCustodyReceipt, OptimizedPreAllocationCustodyError>
        {
            let upstream = run.source.custody();
            let expected_budget = run.source.budget_per_pass();
            let pre_allocation = run
                .selections
                .project_phase(optimization_core::OptimizationExecutionPhase::PreAllocation);
            let (projected, policy) = resolve_pre_allocation_rules(&pre_allocation)?;
            if run.selections != *run.source.selections()
                || run.custody.selections != run.source.selections().identity()
                || run.custody.budget != expected_budget
            {
                return Err(OptimizedPreAllocationCustodyError::SelectionProjectionMismatch);
            }
            if projected != run.pre_allocation_selections
                || policy != run.policy
                || run.custody.policy != policy
                || run.custody.pre_allocation_selections != run.pre_allocation_selections.identity()
            {
                return Err(OptimizedPreAllocationCustodyError::SelectionProjectionMismatch);
            }
            let iteration_bound = measure(
                run.source.selected().selected_plan(),
                run.source.legality().receipt().virtual_register_count(),
            );
            let (replayed, terminal, usage) = run_passes(&run.source, policy, iteration_bound)?;
            if replayed != run.steps {
                return Err(OptimizedPreAllocationCustodyError::StepMismatch { step: 0 });
            }
            if terminal != run.attempt {
                return Err(OptimizedPreAllocationCustodyError::TerminalAttemptMismatch);
            }
            let receipt = pre_allocation_custody_receipt(
                upstream,
                &run.selections,
                &run.pre_allocation_selections,
                policy,
                &run.source,
                &replayed,
                &terminal,
                expected_budget,
                usage,
                iteration_bound,
            );
            if receipt != run.custody {
                return Err(OptimizedPreAllocationCustodyError::ReceiptMismatch);
            }
            Ok(receipt)
        }

        #[allow(clippy::too_many_arguments)]
        fn pre_allocation_custody_receipt(
            source_receipt: StagedOptimizedAllocationLegalityCustodyReceipt,
            selections: &OptimizationSelections,
            pre_allocation_selections: &OptimizationSelections,
            policy: PreAllocationPolicy,
            source: &StagedOptimizedAllocationLegality,
            steps: &[StagedOptimizedPreAllocationStep],
            attempt: &StagedOptimizedPreAllocationAttempt,
            budget: OptimizationWorkBudget,
            usage: OptimizationWorkUsage,
            iteration_bound: usize,
        ) -> StagedPreAllocationOptimizationCustodyReceipt {
            let (final_selected, final_liveness, final_ranges, final_legality, final_vregs) =
                match steps.last() {
                    Some(step) => (
                        step.transformation.transformed_selected(),
                        step.liveness.receipt().identity(),
                        step.ranges.receipt().identity(),
                        step.legality.receipt().identity(),
                        step.legality.receipt().virtual_register_count(),
                    ),
                    None => (
                        source.selected().identity(),
                        source.liveness().receipt().identity(),
                        source.ranges().receipt().identity(),
                        source.legality().receipt().identity(),
                        source.legality().receipt().virtual_register_count(),
                    ),
                };
            let mut receipt = StagedPreAllocationOptimizationCustodyReceipt {
                identity: PreAllocationOptimizationCompletionIdentity::from_canonical_bytes(
                    b"pending",
                ),
                source: source_receipt,
                selections: selections.identity(),
                pre_allocation_selections: pre_allocation_selections.identity(),
                policy,
                budget,
                usage,
                iteration_bound,
                removal_count: steps.len(),
                initial_virtual_register_count: source
                    .legality()
                    .receipt()
                    .virtual_register_count(),
                iterations: steps.iter().map(iteration_receipt).collect(),
                attempt: attempt_receipt(attempt),
                final_selected,
                final_liveness,
                final_ranges,
                final_legality,
                final_virtual_register_count: final_vregs,
            };
            receipt.identity = pre_allocation_completion_identity(&receipt);
            receipt
        }

        fn iteration_receipt(
            step: &StagedOptimizedPreAllocationStep,
        ) -> StagedOptimizedPreAllocationIterationReceipt {
            StagedOptimizedPreAllocationIterationReceipt {
                source_selected: step.transformation.source_selected(),
                transformation: step.transformation.identity(),
                function_index: step.transformation.function_index(),
                instruction: step.transformation.instruction(),
                transformed_selected: step.transformation.transformed_selected(),
                fresh_liveness: step.liveness.receipt().identity(),
                fresh_ranges: step.ranges.receipt().identity(),
                fresh_legality: step.legality.receipt().identity(),
                declined: step.declined,
                evaluated: step.evaluated,
            }
        }

        fn attempt_receipt(
            attempt: &StagedOptimizedPreAllocationAttempt,
        ) -> StagedOptimizedPreAllocationAttemptReceipt {
            StagedOptimizedPreAllocationAttemptReceipt {
                source_selected: attempt.source_selected,
                candidates: attempt.candidates,
                declined: attempt.declined,
            }
        }

        fn pre_allocation_completion_identity(
            receipt: &StagedPreAllocationOptimizationCustodyReceipt,
        ) -> PreAllocationOptimizationCompletionIdentity {
            let mut canonical = Vec::new();
            canonical.extend_from_slice(b"omega.pre-allocation-optimization-completion.v6\0");
            let source = receipt.source;
            for identity in [
                source.optimization().bytes(),
                source.manifest().bytes(),
                source.register_environment().bytes(),
                source.allocator_availability().bytes(),
                source.selected().bytes(),
                source.liveness().bytes(),
                source.ranges().bytes(),
                source.legality().bytes(),
                receipt.selections.bytes(),
                receipt.pre_allocation_selections.bytes(),
            ] {
                canonical.extend_from_slice(&identity);
            }
            crate::codec_bytes::push_encoded(
                &mut canonical,
                receipt.policy.canonical_bits().to_le_bytes(),
            );
            canonical.extend_from_slice(&receipt.budget.encode());
            canonical.extend_from_slice(&receipt.usage.encode());
            for count in [
                receipt.iteration_bound,
                receipt.removal_count,
                receipt.initial_virtual_register_count,
                receipt.iterations.len(),
            ] {
                encode_count(&mut canonical, count);
            }
            macro_rules! encode_transformations {
                ($canonical:ident, $transformation:expr; $($variant:ident, $tag:literal),* $(,)?) => {
                    match $transformation {
                        $(
                            crate::PreAllocationTransformationIdentity::$variant(identity) => {
                                $canonical.push($tag);
                                $canonical.extend_from_slice(&identity.bytes());
                            }
                        )*
                    }
                };
            }
            for iteration in &receipt.iterations {
                canonical.extend_from_slice(&iteration.source_selected().bytes());
                encode_transformations!(canonical, iteration.transformation();
                    CopyRemoval, 1, RedundantExtension, 2, AddressFold, 3, ConstantBoolean, 4,
                    ConstantBranch, 5, DeadCompare, 6, RedundantCompare, 7, EquivalentCompare, 8,
                    CopiedCallOperand, 9, StoredLoadForwarding, 10);
                encode_count(&mut canonical, iteration.function_index());
                crate::codec_bytes::push_encoded(
                    &mut canonical,
                    iteration.instruction().0.to_le_bytes(),
                );
                canonical.extend_from_slice(&iteration.transformed_selected().bytes());
                canonical.extend_from_slice(&iteration.fresh_liveness().bytes());
                canonical.extend_from_slice(&iteration.fresh_ranges().bytes());
                canonical.extend_from_slice(&iteration.fresh_legality().bytes());
                encode_count(&mut canonical, iteration.declined());
                encode_count(&mut canonical, iteration.evaluated());
            }
            canonical.extend_from_slice(&receipt.attempt.source_selected().bytes());
            encode_count(&mut canonical, receipt.attempt.candidates());
            encode_count(&mut canonical, receipt.attempt.declined());
            canonical.extend_from_slice(&receipt.final_selected.bytes());
            canonical.extend_from_slice(&receipt.final_liveness.bytes());
            canonical.extend_from_slice(&receipt.final_ranges.bytes());
            canonical.extend_from_slice(&receipt.final_legality.bytes());
            encode_count(&mut canonical, receipt.final_virtual_register_count);
            PreAllocationOptimizationCompletionIdentity::from_canonical_bytes(&canonical)
        }

        fn encode_count(canonical: &mut Vec<u8>, count: usize) {
            canonical.extend_from_slice(
                &u64::try_from(count)
                    .expect("pre-allocation completion count fits u64")
                    .to_le_bytes(),
            );
        }
    }
    pub(crate) mod model {
        use std::sync::Arc;

        use crate::{
            AddressFoldError, AllocationLegalityError, ConstantBooleanError, ConstantBranchError,
            CopiedCallOperandError, CopyRemovalError, DeadCompareError, EquivalentCompareError,
            LiveRangeError, LivenessError, MachineEffectStageError, RedundantCompareError,
            RedundantExtensionError, StagedOptimizedAllocationLegality,
            StagedOptimizedAllocationLegalityCustodyReceipt, StoredLoadForwardingError,
            ValidatedAddressFold, ValidatedAllocationLegality, ValidatedConstantBoolean,
            ValidatedConstantBranch, ValidatedCopiedCallOperand, ValidatedCopyRemoval,
            ValidatedDeadCompare, ValidatedEquivalentCompare, ValidatedLiveRanges,
            ValidatedLiveness, ValidatedRedundantCompare, ValidatedRedundantExtension,
            ValidatedStoredLoadForwarding,
        };
        use optimization_core::{
            Optimization, OptimizationSelectionIdentity, OptimizationSelections,
            OptimizationWorkBudget, OptimizationWorkUsage,
            PreAllocationOptimizationCompletionIdentity,
        };
        use selected_instructions::{
            AddressFoldIdentity, ConstantBooleanIdentity, ConstantBranchIdentity,
            CopiedCallOperandIdentity, CopyRemovalIdentity, DeadCompareIdentity,
            EquivalentCompareIdentity, LiveRangeIdentity, LivenessIdentity,
            RedundantCompareIdentity, RedundantExtensionIdentity, SelectedInstructionId,
            SelectedInstructionPlan, SelectedInstructionPlanIdentity, StoredLoadForwardingIdentity,
        };

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct PreAllocationPolicy {
            enabled: u32,
        }

        impl PreAllocationPolicy {
            const SAME_BLOCK_COPY_I64_V1_BIT: u32 = 1 << 0;
            const REDUNDANT_EXTENSION_V1_BIT: u32 = 1 << 1;
            const ADDRESS_FOLD_V1_BIT: u32 = 1 << 2;
            const CONSTANT_BOOLEAN_V1_BIT: u32 = 1 << 3;
            const CONSTANT_BRANCH_V1_BIT: u32 = 1 << 4;
            const DEAD_COMPARE_V1_BIT: u32 = 1 << 5;
            const REDUNDANT_COMPARE_V1_BIT: u32 = 1 << 6;
            const EQUIVALENT_COMPARE_V1_BIT: u32 = 1 << 7;
            const COPIED_CALL_OPERAND_V1_BIT: u32 = 1 << 8;
            const STORED_LOAD_FORWARDING_V1_BIT: u32 = 1 << 9;
            const KNOWN_BITS: u32 = Self::SAME_BLOCK_COPY_I64_V1_BIT
                | Self::REDUNDANT_EXTENSION_V1_BIT
                | Self::ADDRESS_FOLD_V1_BIT
                | Self::CONSTANT_BOOLEAN_V1_BIT
                | Self::CONSTANT_BRANCH_V1_BIT
                | Self::DEAD_COMPARE_V1_BIT
                | Self::REDUNDANT_COMPARE_V1_BIT
                | Self::EQUIVALENT_COMPARE_V1_BIT
                | Self::COPIED_CALL_OPERAND_V1_BIT
                | Self::STORED_LOAD_FORWARDING_V1_BIT;

            pub(crate) const SAME_BLOCK_COPY_I64_V1: Self = Self {
                enabled: Self::SAME_BLOCK_COPY_I64_V1_BIT,
            };

            pub(crate) const REDUNDANT_EXTENSION_V1: Self = Self {
                enabled: Self::REDUNDANT_EXTENSION_V1_BIT,
            };

            pub(crate) const ADDRESS_FOLD_V1: Self = Self {
                enabled: Self::ADDRESS_FOLD_V1_BIT,
            };

            pub(crate) const CONSTANT_BOOLEAN_V1: Self = Self {
                enabled: Self::CONSTANT_BOOLEAN_V1_BIT,
            };

            pub(crate) const CONSTANT_BRANCH_V1: Self = Self {
                enabled: Self::CONSTANT_BRANCH_V1_BIT,
            };

            pub(crate) const DEAD_COMPARE_V1: Self = Self {
                enabled: Self::DEAD_COMPARE_V1_BIT,
            };

            pub(crate) const REDUNDANT_COMPARE_V1: Self = Self {
                enabled: Self::REDUNDANT_COMPARE_V1_BIT,
            };

            pub(crate) const EQUIVALENT_COMPARE_V1: Self = Self {
                enabled: Self::EQUIVALENT_COMPARE_V1_BIT,
            };

            pub(crate) const COPIED_CALL_OPERAND_V1: Self = Self {
                enabled: Self::COPIED_CALL_OPERAND_V1_BIT,
            };

            pub(crate) const STORED_LOAD_FORWARDING_V1: Self = Self {
                enabled: Self::STORED_LOAD_FORWARDING_V1_BIT,
            };

            pub const fn empty() -> Self {
                Self { enabled: 0 }
            }

            pub const fn union(self, other: Self) -> Self {
                Self {
                    enabled: self.enabled | other.enabled,
                }
            }

            pub const fn contains(self, other: Self) -> bool {
                self.enabled & other.enabled == other.enabled
            }

            pub const fn canonical_bits(self) -> u32 {
                self.enabled
            }

            pub const fn from_canonical_bits(bits: u32) -> Option<Self> {
                if bits & !Self::KNOWN_BITS != 0 {
                    return None;
                }
                Some(Self { enabled: bits })
            }
        }

        macro_rules! transformation_accessors {
            ($( $name:ident : $( $call:ident ).+ -> $ret:ty ; )+) => {
                $(
                    pub fn $name(&self) -> $ret {
                        match self {
                            Self::CopyRemoval(payload) => payload.$($call()).+,
                            Self::RedundantExtension(payload) => payload.$($call()).+,
                            Self::AddressFold(payload) => payload.$($call()).+,
                            Self::ConstantBoolean(payload) => payload.$($call()).+,
                            Self::ConstantBranch(payload) => payload.$($call()).+,
                            Self::DeadCompare(payload) => payload.$($call()).+,
                            Self::RedundantCompare(payload) => payload.$($call()).+,
                            Self::EquivalentCompare(payload) => payload.$($call()).+,
                            Self::CopiedCallOperand(payload) => payload.$($call()).+,
                            Self::StoredLoadForwarding(payload) => payload.$($call()).+,
                        }
                    }
                )+
            };
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ValidatedPreAllocationTransformation {
            CopyRemoval(ValidatedCopyRemoval),
            RedundantExtension(ValidatedRedundantExtension),
            AddressFold(ValidatedAddressFold),
            ConstantBoolean(ValidatedConstantBoolean),
            ConstantBranch(ValidatedConstantBranch),
            DeadCompare(ValidatedDeadCompare),
            RedundantCompare(ValidatedRedundantCompare),
            EquivalentCompare(ValidatedEquivalentCompare),
            CopiedCallOperand(ValidatedCopiedCallOperand),
            StoredLoadForwarding(ValidatedStoredLoadForwarding),
        }

        impl ValidatedPreAllocationTransformation {
            transformation_accessors! {
                transformed : transformed -> &SelectedInstructionPlan;
                shared_transformed : shared_transformed -> Arc<SelectedInstructionPlan>;
                source_selected : receipt . source_selected -> SelectedInstructionPlanIdentity;
                transformed_selected : receipt . transformed_selected -> SelectedInstructionPlanIdentity;
                optimization_unit : receipt . optimization_unit -> optimization_core::OptimizationUnitIdentity;
                work_schedule : receipt . work_schedule -> semantic_vocabulary::WorkScheduleIdentity;
                function_index : receipt . function_index -> usize;
            }

            pub fn instruction(&self) -> SelectedInstructionId {
                match self {
                    Self::CopyRemoval(removal) => removal.receipt().copy(),
                    Self::RedundantExtension(removal) => removal.receipt().extension(),
                    Self::AddressFold(fold) => fold.receipt().access(),
                    Self::ConstantBoolean(fold) => fold.receipt().materialization(),
                    Self::ConstantBranch(fold) => fold.receipt().branch(),
                    Self::DeadCompare(removal) => removal.receipt().compare(),
                    Self::RedundantCompare(removal) => removal.receipt().compare(),
                    Self::EquivalentCompare(removal) => removal.receipt().compare(),
                    Self::CopiedCallOperand(reroute) => reroute.receipt().call(),
                    Self::StoredLoadForwarding(forwarding) => forwarding.receipt().load(),
                }
            }

            pub fn identity(&self) -> PreAllocationTransformationIdentity {
                macro_rules! project {
                    ($($variant:ident($binding:ident)),* $(,)?) => {
                        match self {
                            $(Self::$variant($binding) => {
                                PreAllocationTransformationIdentity::$variant($binding.receipt().identity())
                            })*
                        }
                    };
                }
                project!(
                    CopyRemoval(removal),
                    RedundantExtension(removal),
                    AddressFold(fold),
                    ConstantBoolean(fold),
                    ConstantBranch(fold),
                    DeadCompare(removal),
                    RedundantCompare(removal),
                    EquivalentCompare(removal),
                    CopiedCallOperand(reroute),
                    StoredLoadForwarding(forwarding),
                )
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PreAllocationTransformationIdentity {
            CopyRemoval(CopyRemovalIdentity),
            RedundantExtension(RedundantExtensionIdentity),
            AddressFold(AddressFoldIdentity),
            ConstantBoolean(ConstantBooleanIdentity),
            ConstantBranch(ConstantBranchIdentity),
            DeadCompare(DeadCompareIdentity),
            RedundantCompare(RedundantCompareIdentity),
            EquivalentCompare(EquivalentCompareIdentity),
            CopiedCallOperand(CopiedCallOperandIdentity),
            StoredLoadForwarding(StoredLoadForwardingIdentity),
        }

        impl From<PreAllocationTransformationIdentity>
            for register_homes::PostAllocationSelectedTransformation
        {
            fn from(identity: PreAllocationTransformationIdentity) -> Self {
                macro_rules! forward {
                    ($($variant:ident),* $(,)?) => {
                        match identity {
                            $(PreAllocationTransformationIdentity::$variant(identity) => {
                                Self::$variant(identity)
                            })*
                        }
                    };
                }
                forward!(
                    CopyRemoval,
                    RedundantExtension,
                    AddressFold,
                    ConstantBoolean,
                    ConstantBranch,
                    DeadCompare,
                    RedundantCompare,
                    EquivalentCompare,
                    CopiedCallOperand,
                    StoredLoadForwarding,
                )
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct StagedOptimizedPreAllocationStep {
            pub(crate) transformation: ValidatedPreAllocationTransformation,
            pub(crate) liveness: ValidatedLiveness,
            pub(crate) ranges: ValidatedLiveRanges,
            pub(crate) legality: ValidatedAllocationLegality,
            pub(crate) declined: usize,
            pub(crate) evaluated: usize,
        }

        impl StagedOptimizedPreAllocationStep {
            pub const fn transformation(&self) -> &ValidatedPreAllocationTransformation {
                &self.transformation
            }
            validated_analysis_accessors!();
            receipt_accessor_rows!(ref StagedOptimizedPreAllocationStep {
                declined: usize,
                evaluated: usize,
            });
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct StagedOptimizedPreAllocationAttempt {
            pub(crate) source_selected: SelectedInstructionPlanIdentity,
            pub(crate) candidates: usize,
            pub(crate) declined: usize,
        }

        impl StagedOptimizedPreAllocationAttempt {
            receipt_accessor_rows!(ref StagedOptimizedPreAllocationAttempt {
                    source_selected: SelectedInstructionPlanIdentity,
                    candidates: usize,
                    declined: usize,
            });
        }

        #[derive(Debug)]
        pub struct StagedPreAllocationOptimizationRun {
            pub(crate) source: StagedOptimizedAllocationLegality,
            pub(crate) selections: OptimizationSelections,
            pub(crate) pre_allocation_selections: OptimizationSelections,
            pub(crate) policy: PreAllocationPolicy,
            pub(crate) steps: Vec<StagedOptimizedPreAllocationStep>,
            pub(crate) attempt: StagedOptimizedPreAllocationAttempt,
            pub(crate) custody: StagedPreAllocationOptimizationCustodyReceipt,
        }

        impl StagedPreAllocationOptimizationRun {
            pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
                &self.source
            }
            pub const fn selections(&self) -> &OptimizationSelections {
                &self.selections
            }
            pub const fn pre_allocation_selections(&self) -> &OptimizationSelections {
                &self.pre_allocation_selections
            }
            receipt_accessor_rows!(ref StagedPreAllocationOptimizationRun {
                policy: PreAllocationPolicy,
            });
            pub fn steps(&self) -> &[StagedOptimizedPreAllocationStep] {
                &self.steps
            }
            pub const fn attempt(&self) -> &StagedOptimizedPreAllocationAttempt {
                &self.attempt
            }
            pub const fn custody(&self) -> &StagedPreAllocationOptimizationCustodyReceipt {
                &self.custody
            }
            pub fn current(&self) -> crate::SelectedProgramRef<'_> {
                match self.steps.last() {
                    Some(step) => crate::SelectedProgramRef::new(&step.transformation),
                    None => crate::SelectedProgramRef::new(self.source.selected()),
                }
            }
            pub fn liveness(&self) -> &ValidatedLiveness {
                match self.steps.last() {
                    Some(step) => step.liveness(),
                    None => self.source.liveness(),
                }
            }
            pub fn ranges(&self) -> &ValidatedLiveRanges {
                match self.steps.last() {
                    Some(step) => step.ranges(),
                    None => self.source.ranges(),
                }
            }
            pub fn legality(&self) -> &ValidatedAllocationLegality {
                match self.steps.last() {
                    Some(step) => step.legality(),
                    None => self.source.legality(),
                }
            }
        }

        impl StagedPreAllocationOptimizationRun {
            pub(crate) fn into_completed(
                self,
            ) -> selected_instruction_analyses::CompletedSelectedOptimization<
                optimization_core::PreAllocationOptimizationCompletionIdentity,
            > {
                let ledger = selected_instruction_analyses::SelectedOptimizationLedger {
                    identity: self.custody.identity,
                    phase_selections: self.custody.pre_allocation_selections,
                    source: self.custody.source,
                    transformations: self
                        .custody
                        .iterations
                        .iter()
                        .map(|iteration| iteration.transformation().into())
                        .collect(),
                };
                let source = self.source;
                let (current, liveness, ranges, legality) = match self.steps.into_iter().last() {
                    Some(step) => (
                        crate::OwnedSelectedProgram::retain(&step.transformation),
                        step.liveness,
                        step.ranges,
                        step.legality,
                    ),
                    None => (
                        crate::OwnedSelectedProgram::retain(source.selected()),
                        source.liveness().clone(),
                        source.ranges().clone(),
                        source.legality().clone(),
                    ),
                };
                selected_instruction_analyses::CompletedSelectedOptimization {
                    source,
                    current,
                    liveness,
                    ranges,
                    legality,
                    ledger,
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct StagedPreAllocationOptimizationCustodyReceipt {
            pub(crate) identity: PreAllocationOptimizationCompletionIdentity,
            pub(crate) source: StagedOptimizedAllocationLegalityCustodyReceipt,
            pub(crate) selections: OptimizationSelectionIdentity,
            pub(crate) pre_allocation_selections: OptimizationSelectionIdentity,
            pub(crate) policy: PreAllocationPolicy,
            pub(crate) budget: OptimizationWorkBudget,
            pub(crate) usage: OptimizationWorkUsage,
            pub(crate) iteration_bound: usize,
            pub(crate) removal_count: usize,
            pub(crate) initial_virtual_register_count: usize,
            pub(crate) iterations: Vec<StagedOptimizedPreAllocationIterationReceipt>,
            pub(crate) attempt: StagedOptimizedPreAllocationAttemptReceipt,
            pub(crate) final_selected: SelectedInstructionPlanIdentity,
            pub(crate) final_liveness: LivenessIdentity,
            pub(crate) final_ranges: LiveRangeIdentity,
            pub(crate) final_legality: register_homes::AllocationLegalityIdentity,
            pub(crate) final_virtual_register_count: usize,
        }

        impl StagedPreAllocationOptimizationCustodyReceipt {
            receipt_accessor_rows!(ref StagedPreAllocationOptimizationCustodyReceipt {
                    identity: PreAllocationOptimizationCompletionIdentity,
                    source: StagedOptimizedAllocationLegalityCustodyReceipt,
                    selections: OptimizationSelectionIdentity,
                    pre_allocation_selections: OptimizationSelectionIdentity,
                    policy: PreAllocationPolicy,
                    budget: OptimizationWorkBudget,
                    usage: OptimizationWorkUsage,
                    iteration_bound: usize,
                    removal_count: usize,
                    initial_virtual_register_count: usize,
                    attempt: StagedOptimizedPreAllocationAttemptReceipt,
                    final_virtual_register_count: usize,
                    final_selected: SelectedInstructionPlanIdentity,
                    final_liveness: LivenessIdentity,
                    final_ranges: LiveRangeIdentity,
                    final_legality: register_homes::AllocationLegalityIdentity,
            });
            pub fn iterations(&self) -> &[StagedOptimizedPreAllocationIterationReceipt] {
                &self.iterations
            }
        }

        copy_receipt! {
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct StagedOptimizedPreAllocationIterationReceipt {
                pub(crate) source_selected: SelectedInstructionPlanIdentity,
                pub(crate) transformation: PreAllocationTransformationIdentity,
                pub(crate) function_index: usize,
                pub(crate) instruction: SelectedInstructionId,
                pub(crate) transformed_selected: SelectedInstructionPlanIdentity,
                pub(crate) fresh_liveness: LivenessIdentity,
                pub(crate) fresh_ranges: LiveRangeIdentity,
                pub(crate) fresh_legality: register_homes::AllocationLegalityIdentity,
                pub(crate) declined: usize,
                pub(crate) evaluated: usize,
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct StagedOptimizedPreAllocationAttemptReceipt {
            pub(crate) source_selected: SelectedInstructionPlanIdentity,
            pub(crate) candidates: usize,
            pub(crate) declined: usize,
        }

        impl StagedOptimizedPreAllocationAttemptReceipt {
            receipt_accessor_rows!(StagedOptimizedPreAllocationAttemptReceipt {
                source_selected: SelectedInstructionPlanIdentity,
                candidates: usize,
                declined: usize,
            });
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum OptimizedPreAllocationCustodyError {
            CandidateFrontierMismatch,
            CopyRemoval(CopyRemovalError),
            RedundantExtension(RedundantExtensionError),
            AddressFold(AddressFoldError),
            ConstantBoolean(ConstantBooleanError),
            ConstantBranch(ConstantBranchError),
            DeadCompare(DeadCompareError),
            RedundantCompare(RedundantCompareError),
            EquivalentCompare(EquivalentCompareError),
            CopiedCallOperand(CopiedCallOperandError),
            StoredLoadForwarding(StoredLoadForwardingError),
            MachineEffects(MachineEffectStageError),
            Liveness(LivenessError),
            LiveRanges(LiveRangeError),
            AllocationLegality(AllocationLegalityError),
            RemainingTransitions {
                count: usize,
            },
            StepMismatch {
                step: usize,
            },
            TerminalAttemptMismatch,
            ReceiptMismatch,
            MissingPreAllocationOptimization,
            UnsupportedPreAllocationOptimization(Optimization),
            PreAllocationMeasureMismatch {
                previous: usize,
                current: usize,
            },
            PreAllocationIterationBoundExceeded {
                bound: usize,
            },
            SelectionProjectionMismatch,
            WorkOverflow,
            PreAllocationBudgetExceeded {
                required: OptimizationWorkUsage,
                budget: OptimizationWorkBudget,
            },
        }

        failed_display!(
            OptimizedPreAllocationCustodyError,
            "optimized pre-allocation staging failed"
        );
    }

    use optimization_core::{
        OptimizationExecutionPhase, OptimizationPhaseSelections, OptimizationSelections,
    };

    use crate::StagedOptimizedAllocationLegality;

    pub(crate) use catalog::{
        PRE_ALLOCATION_RULE_CATALOG, PreAllocationRuleCatalogEntry, PreAllocationRuleCatalogError,
    };
    pub(crate) use execution::validate_pre_allocation_optimization_custody;
    pub use model::{
        OptimizedPreAllocationCustodyError, StagedPreAllocationOptimizationCustodyReceipt,
        StagedPreAllocationOptimizationRun,
    };
    pub(crate) use model::{
        PreAllocationPolicy, PreAllocationTransformationIdentity,
        StagedOptimizedPreAllocationAttempt, StagedOptimizedPreAllocationAttemptReceipt,
        StagedOptimizedPreAllocationIterationReceipt, StagedOptimizedPreAllocationStep,
        ValidatedPreAllocationTransformation,
    };

    impl From<PreAllocationRuleCatalogError> for OptimizedPreAllocationCustodyError {
        fn from(error: PreAllocationRuleCatalogError) -> Self {
            match error {
                PreAllocationRuleCatalogError::WrongPhase(_) => Self::SelectionProjectionMismatch,
                PreAllocationRuleCatalogError::MissingSelection => {
                    Self::MissingPreAllocationOptimization
                }
                PreAllocationRuleCatalogError::UnsupportedSelection(optimization) => {
                    Self::UnsupportedPreAllocationOptimization(optimization)
                }
            }
        }
    }

    pub(crate) fn resolve_pre_allocation_rules(
        selections: &OptimizationPhaseSelections,
    ) -> Result<(OptimizationSelections, PreAllocationPolicy), PreAllocationRuleCatalogError> {
        let phase = crate::rewrites::required_stage_phase(
            selections,
            OptimizationExecutionPhase::PreAllocation,
            PreAllocationRuleCatalogError::WrongPhase,
            || PreAllocationRuleCatalogError::MissingSelection,
            PreAllocationRuleCatalogError::UnsupportedSelection,
        )?;
        let policy = PRE_ALLOCATION_RULE_CATALOG
            .iter()
            .filter(|entry| phase.contains(entry.optimization()))
            .fold(PreAllocationPolicy::empty(), |policy, entry| {
                policy.union(entry.payload().policy())
            });
        Ok((phase, policy))
    }

    pub(crate) fn run_pre_allocation_optimizations(
        source: StagedOptimizedAllocationLegality,
    ) -> Result<StagedPreAllocationOptimizationRun, OptimizedPreAllocationCustodyError> {
        let selections = source.selections().clone();
        let pre_allocation = selections.project_phase(OptimizationExecutionPhase::PreAllocation);
        let (selected, policy) = resolve_pre_allocation_rules(&pre_allocation)?;
        execution::execute_pre_allocation_optimizations(source, selections, selected, policy)
    }
}
pub(crate) use crate::copy_and_extension_rewrites::redundant_extension;
pub(crate) use crate::literal_fold_rewrites::selected_lowering;
pub use crate::unexecuted_rewrites as unexecuted;
pub(crate) mod window_hazards {
    use selected_instructions::{
        FrameStorageSlotId, LocalStorageSlotId, SelectedInstruction, SelectedInstructionKind,
    };
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum UnrosteredReach {
        Private,
        Store,
        Unaccounted,
    }

    pub(crate) fn unrostered_reach(instruction: &SelectedInstruction) -> UnrosteredReach {
        use SelectedInstructionKind::*;
        match instruction.kind {
            Store64 {
                slot:
                    FrameStorageSlotId::Local(
                        LocalStorageSlotId::Spill { .. } | LocalStorageSlotId::Boundary { .. },
                    ),
                ..
            }
            | Load8 { .. }
            | Load16 { .. }
            | Load32 { .. }
            | Load64 { .. }
            | Load8Indexed
            | LoadPacked { .. }
            | FrameAddress { .. }
            | AddressOffset { .. }
            | ByteViewAddress
            | CopyI64
            | MaterializeI64 { .. }
            | CompareI64
            | CompareI64Zero
            | CompareI64Immediate { .. }
            | ExactAddI64 { .. }
            | ExactSubtractI64 { .. }
            | ExactMultiplyI64 { .. }
            | ExactAddI64Immediate { .. }
            | ExactSubtractI64Immediate { .. }
            | SaturatingAdd { .. }
            | WrappingAddI64
            | WrappingSubtractI64
            | WrappingMultiplyI64
            | SaturatingSubtract { .. }
            | SaturatingDivide { .. }
            | SaturatingRemainder { .. }
            | SaturatingMultiply { .. }
            | ExactDivideU64 { .. }
            | ExactRemainderU64 { .. }
            | WrappingRemainderI64 { .. }
            | WrappingDivideI64 { .. }
            | ExactDivideI64 { .. }
            | ExactRemainderI64 { .. }
            | BitwiseAndI64
            | BitwiseOrI64
            | BitwiseNotI64
            | BitwiseXorI64
            | ZeroExtendU8
            | ZeroExtendU16
            | ZeroExtendU32
            | SignExtendI8
            | SignExtendI16
            | SignExtendI32
            | Float32ToBits
            | Float64ToBits
            | BitsToFloat32
            | BitsToFloat64
            | MaterializeBooleanEqual
            | MaterializeBooleanU64LessThan
            | MaterializeBooleanI64LessThan
            | MaterializeBooleanU64LessOrEqual
            | MaterializeBooleanI64LessOrEqual => UnrosteredReach::Private,
            Store { .. } | StorePacked { .. } | Store64 { .. } | CopyBytes | ZeroBytes => {
                UnrosteredReach::Store
            }
            _ => UnrosteredReach::Unaccounted,
        }
    }
}

pub(crate) use address_fold::{AddressFoldError, ValidatedAddressFold};
pub(crate) use address_fold::{
    fold_selected_address, measured_steps as address_fold_measured_steps,
};
pub(crate) use catalog::{SELECTED_STAGE_RULE_CATALOG, SelectedStageRuleRows};
pub(crate) use constant_boolean::{ConstantBooleanError, ValidatedConstantBoolean};
pub(crate) use constant_boolean::{
    fold_selected_constant_boolean, measured_steps as constant_boolean_measured_steps,
    surface_sizes as constant_boolean_surface_sizes,
};
pub(crate) use constant_branch::{ConstantBranchError, ValidatedConstantBranch};
pub(crate) use constant_branch::{
    fold_selected_constant_branch, measured_steps as constant_branch_measured_steps,
    surface_sizes as constant_branch_surface_sizes,
};
pub(crate) use copied_call_operand::{
    CopiedCallOperandError, ValidatedCopiedCallOperand, copied_call_operand_measured_steps,
    fold_selected_copied_call_operand,
};
pub(crate) use copy_removal::{CopyRemovalError, ValidatedCopyRemoval};
pub(crate) use copy_removal::{
    measured_steps as copy_removal_measured_steps, remove_selected_copy,
};
pub(crate) use dead_compare::{
    DeadCompareError, EquivalentCompareError, RedundantCompareError, ValidatedDeadCompare,
    ValidatedEquivalentCompare, ValidatedRedundantCompare,
};
pub(crate) use dead_compare::{
    equivalent_compare_measured_steps, measured_steps as dead_compare_measured_steps,
    redundant_compare_measured_steps, remove_dead_compare, remove_equivalent_compare,
    remove_redundant_compare,
};
pub use literal_folds::{
    OptimizedLiteralFoldCustodyError, StagedSelectedLoweringOptimizationCustodyReceipt,
    StagedSelectedLoweringOptimizationRun,
};
pub(crate) use literal_folds::{
    StagedOptimizedLiteralFoldAttempt, StagedOptimizedLiteralFoldAttemptReceipt,
    StagedOptimizedLiteralFoldIterationReceipt, StagedOptimizedLiteralFoldStep,
    run_selected_lowering_optimizations, validate_selected_lowering_optimization_custody,
};
pub use pre_allocation::{
    OptimizedPreAllocationCustodyError, StagedPreAllocationOptimizationCustodyReceipt,
    StagedPreAllocationOptimizationRun,
};
pub(crate) use pre_allocation::{
    PRE_ALLOCATION_RULE_CATALOG, PreAllocationPolicy, PreAllocationRuleCatalogEntry,
    PreAllocationTransformationIdentity, StagedOptimizedPreAllocationAttempt,
    StagedOptimizedPreAllocationAttemptReceipt, StagedOptimizedPreAllocationIterationReceipt,
    StagedOptimizedPreAllocationStep, ValidatedPreAllocationTransformation,
    resolve_pre_allocation_rules, run_pre_allocation_optimizations,
    validate_pre_allocation_optimization_custody,
};
pub(crate) use redundant_extension::{RedundantExtensionError, ValidatedRedundantExtension};
pub(crate) use redundant_extension::{
    measured_steps as redundant_extension_measured_steps, remove_selected_redundant_extension,
};

pub(crate) use selected_lowering::{
    FunctionLiteralFold, LiteralFoldAction, LiteralFoldError, LiteralFoldPlan, LiteralFoldPolicy,
    LiteralFoldValidationReceipt, PairResultDisposition, PairUnitDefRelation,
    SELECTED_LOWERING_RULE_CATALOG, SelectedInstructionPairRule, SelectedLoweringRuleCatalogEntry,
    SelectedLoweringRuleCatalogError, resolve_selected_lowering_rules,
};
pub use selected_lowering::{LiteralFoldIdentity, ValidatedLiteralFold};
pub(crate) use selected_lowering::{
    enabled_pair_rules, fold_selected_incoming_literal, literal_fold_identity,
};

pub(crate) use selected_instruction_analyses::rewriting::*;
pub(crate) use selected_instruction_analyses::sealed;

pub(crate) use load_forwarding::{
    StoredLoadForwardingError, ValidatedStoredLoadForwarding, forward_selected_stored_load,
};

use crate::rewrites::catalog::selected_stage_catalog_contains;
use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationPhaseMismatch,
    OptimizationPhaseSelections, OptimizationSelections,
};
use selected_instructions::SelectedFunction;

pub(crate) fn required_stage_phase<E>(
    selections: &OptimizationPhaseSelections,
    phase_kind: OptimizationExecutionPhase,
    wrong_phase: impl FnOnce(OptimizationPhaseMismatch) -> E,
    missing_selection: impl FnOnce() -> E,
    unsupported: impl FnOnce(Optimization) -> E,
) -> Result<OptimizationSelections, E> {
    let phase = selections.require_phase(phase_kind).map_err(wrong_phase)?;
    if phase.is_empty() {
        return Err(missing_selection());
    }
    if let Some(unsupported_selection) = phase
        .as_slice()
        .iter()
        .find(|selected| !selected_stage_catalog_contains(phase_kind, **selected))
    {
        return Err(unsupported(*unsupported_selection));
    }
    Ok(phase.clone())
}
