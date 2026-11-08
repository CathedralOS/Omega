/*
We implement displacement address folding (`address_fold`) for memory
consumers reading an `AddressOffset` result, and byte-exact stored-load
forwarding (`load_forwarding`) when reconstructed alias and path facts prove a
prior store still governs a subsequent read.
*/

#[allow(unused_imports)]
pub(crate) use crate::rewrites::*;

pub(crate) mod address_fold {
    pub(crate) mod admission {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::RegisterOperandAccess;
        use selected_instructions::{
            SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
            VirtualRegisterId,
        };

        use super::AddressFoldError;
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::block_edges::member_position;
        use crate::rewrites::carrier_pair;

        pub(crate) struct Admission<'source> {
            pub function: &'source SelectedFunction,
            pub block_index: usize,
            pub consumer_index: usize,
            pub input: VirtualRegisterId,
            pub kind: SelectedInstructionKind,
        }

        pub(crate) fn consumer_shape(
            kind: SelectedInstructionKind,
        ) -> Option<(u32, RegisterOperandAccess)> {
            use SelectedInstructionKind::*;
            let scale = match kind {
                Load8 { .. } | AddressOffset { .. } => 1,
                Load16 { .. } => 2,
                Load32 { .. } => 4,
                Load64 { .. } => 8,
                Store { byte_size, .. } => match byte_size {
                    1 | 2 | 4 | 8 => u32::from(byte_size),
                    _ => return None,
                },
                _ => return None,
            };
            let second = match kind {
                Load8 { .. }
                | Load16 { .. }
                | Load32 { .. }
                | Load64 { .. }
                | AddressOffset { .. } => RegisterOperandAccess::Def,
                Store { .. } => RegisterOperandAccess::Use,
                _ => return None,
            };
            Some((scale, second))
        }

        pub(crate) fn admitted_offset(
            combined: u32,
            scale: u32,
            architecture: target::Architecture,
        ) -> bool {
            match architecture {
                target::Architecture::Aarch64 => {
                    combined.is_multiple_of(scale) && combined / scale <= 4095
                }
                target::Architecture::X86_64 => combined <= i32::MAX as u32,
            }
        }

        pub(crate) fn rewrite_scan_steps(
            plan: &selected_instructions::SelectedInstructionPlan,
            function: &selected_instructions::SelectedFunction,
        ) -> Result<usize, AddressFoldError> {
            crate::rewrites::rewrite_scan(plan, function).ok_or(AddressFoldError::IdentityOverflow)
        }

        pub(crate) fn admit<'source>(
            source: &'source impl ValidatedSelectedAnalysis,
            function_index: usize,
            access: SelectedInstructionId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<Admission<'source>, AddressFoldError> {
            let (plan, function) =
                crate::rewrites::plan_function(source, function_index, environment, || {
                    AddressFoldError::SourceMismatch
                })?;
            let (block_index, consumer_index) =
                member_position(function, access).ok_or(AddressFoldError::SourceMismatch)?;
            let block = &function.blocks[block_index];
            let consumer = &block.instructions[consumer_index];
            let (scale, second_access) =
                consumer_shape(consumer.kind).ok_or(AddressFoldError::UnsupportedInstruction)?;
            let Some((pointer_operand, second_operand)) = carrier_pair(consumer, second_access)
            else {
                return Err(AddressFoldError::UnsupportedInstruction);
            };
            let pointer = pointer_operand.virtual_register;
            let find_register = |register| function.virtual_register(register);
            let pointer_register =
                find_register(pointer).ok_or(AddressFoldError::UnsupportedProducer)?;
            let second_register = find_register(second_operand.virtual_register)
                .ok_or(AddressFoldError::UnsupportedUse)?;
            let row = environment
                .constraint(consumer.constraint)
                .ok_or(AddressFoldError::ConstraintMismatch)?;
            if !crate::rewrites::declares_pair(row, RegisterOperandAccess::Use, second_access)
                || row.operands[0].class != pointer_operand.class
                || pointer_operand.class != pointer_register.class
                || row.operands[1].class != second_operand.class
                || second_operand.class != second_register.class
            {
                return Err(AddressFoldError::ConstraintMismatch);
            }
            let producer_index = super::super::operand_probes::last_definition_before(
                &block.instructions,
                consumer_index,
                pointer,
            )
            .ok_or(AddressFoldError::UnsupportedProducer)?;
            let producer = &block.instructions[producer_index];
            let SelectedInstructionKind::AddressOffset {
                byte_offset: producer_offset,
            } = producer.kind
            else {
                return Err(AddressFoldError::UnsupportedProducer);
            };
            let Some((base_operand, result_operand)) =
                carrier_pair(producer, RegisterOperandAccess::Def)
            else {
                return Err(AddressFoldError::UnsupportedProducer);
            };
            if result_operand.virtual_register != pointer {
                return Err(AddressFoldError::UnsupportedProducer);
            }
            let base = base_operand.virtual_register;
            if base == pointer {
                return Err(AddressFoldError::UnsupportedProducer);
            }
            let base_register = find_register(base).ok_or(AddressFoldError::UnsupportedUse)?;
            if base_register.class != pointer_operand.class {
                return Err(AddressFoldError::UnsupportedUse);
            }
            let producer_row = environment
                .constraint(producer.constraint)
                .ok_or(AddressFoldError::ConstraintMismatch)?;
            if !crate::rewrites::declares_pair(
                producer_row,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Def,
            ) || producer_row.operands[0].class != base_operand.class
                || base_operand.class != base_register.class
                || producer_row.operands[1].class != result_operand.class
                || result_operand.class != pointer_register.class
            {
                return Err(AddressFoldError::ConstraintMismatch);
            }
            for instruction in &block.instructions[producer_index + 1..consumer_index] {
                if instruction.operands.iter().any(|operand| {
                    operand.access != RegisterOperandAccess::Use && operand.virtual_register == base
                }) {
                    return Err(AddressFoldError::UnsupportedUse);
                }
            }
            let byte_offset = match consumer.kind {
                selected_instructions::load_width_triple_pattern!(byte_offset, ..)
                | SelectedInstructionKind::Load64 { byte_offset }
                | SelectedInstructionKind::Store { byte_offset, .. }
                | SelectedInstructionKind::AddressOffset { byte_offset } => byte_offset,
                _ => return Err(AddressFoldError::UnsupportedInstruction),
            };
            let combined = byte_offset
                .checked_add(producer_offset)
                .ok_or(AddressFoldError::UnsupportedOffset)?;
            if !admitted_offset(combined, scale, plan.target.architecture) {
                return Err(AddressFoldError::UnsupportedOffset);
            }
            let kind = match consumer.kind {
                SelectedInstructionKind::Load8 { .. } => SelectedInstructionKind::Load8 {
                    byte_offset: combined,
                },
                SelectedInstructionKind::Load16 { .. } => SelectedInstructionKind::Load16 {
                    byte_offset: combined,
                },
                SelectedInstructionKind::Load32 { .. } => SelectedInstructionKind::Load32 {
                    byte_offset: combined,
                },
                SelectedInstructionKind::Load64 { .. } => SelectedInstructionKind::Load64 {
                    byte_offset: combined,
                },
                SelectedInstructionKind::Store { byte_size, .. } => {
                    SelectedInstructionKind::Store {
                        byte_offset: combined,
                        byte_size,
                    }
                }
                SelectedInstructionKind::AddressOffset { .. } => {
                    SelectedInstructionKind::AddressOffset {
                        byte_offset: combined,
                    }
                }
                _ => return Err(AddressFoldError::UnsupportedInstruction),
            };
            let steps = rewrite_scan_steps(plan, function)?
                .checked_add(block.instructions.len())
                .and_then(|total| total.checked_add(block.instructions.len()))
                .ok_or(AddressFoldError::IdentityOverflow)?;
            crate::rewrites::charge_steps(
                steps,
                budget,
                || AddressFoldError::IdentityOverflow,
                || AddressFoldError::WorkBudgetExceeded,
            )?;
            Ok(Admission {
                function,
                block_index,
                consumer_index,
                input: base,
                kind,
            })
        }

        pub(crate) fn rewritten(admitted: &Admission<'_>) -> SelectedInstruction {
            let mut instruction = admitted.function.blocks[admitted.block_index].instructions
                [admitted.consumer_index]
                .clone();
            instruction.kind = admitted.kind;
            instruction.operands[0].virtual_register = admitted.input;
            instruction
        }
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::SelectedInstructionId;

        use super::{AddressFoldError, ValidatedAddressFold, admission};
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn fold_selected_address(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            access: SelectedInstructionId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedAddressFold, AddressFoldError> {
            let admitted = admission::admit(source, function_index, access, environment, budget)?;
            let mut transformed = source.selected_plan().clone();
            let function = &mut transformed.functions[function_index];
            function.blocks[admitted.block_index].instructions[admitted.consumer_index] =
                admission::rewritten(&admitted);
            super::validation::seal_address_fold(source, function_index, access, transformed)
        }
    }
    pub(crate) mod validation {
        use crate::rewrites::sealed;

        use selected_instructions::{
            SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
        };

        use super::{AddressFoldError, AddressFoldReceipt, ValidatedAddressFold, admission};
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn measured_steps(
            plan: &SelectedInstructionPlan,
            function: &SelectedFunction,
        ) -> Result<u64, AddressFoldError> {
            let largest_block = function
                .blocks
                .iter()
                .map(|block| block.instructions.len())
                .max()
                .unwrap_or(0);
            let steps = admission::rewrite_scan_steps(plan, function)?
                .checked_add(largest_block)
                .and_then(|total| total.checked_add(largest_block))
                .ok_or(AddressFoldError::IdentityOverflow)?;
            u64::try_from(steps).map_err(|_| AddressFoldError::IdentityOverflow)
        }

        pub(crate) fn seal_address_fold(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            access: SelectedInstructionId,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedAddressFold, AddressFoldError> {
            Ok(
                sealed!(ValidatedAddressFold, AddressFoldReceipt, source, proposed, function_index: function_index, access: access),
            )
        }
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{
        AddressFoldIdentity, SelectedInstructionId, SelectedInstructionPlan,
        SelectedInstructionPlanIdentity,
    };
    use semantic_vocabulary::WorkScheduleIdentity;

    pub(crate) use rewrite::fold_selected_address;
    pub(crate) use validation::measured_steps;

    validated_transformed_receipt!(ValidatedAddressFold, AddressFoldReceipt);

    rewrite_receipt!(AddressFoldReceipt {
        function_index: usize,
        access: SelectedInstructionId,
    });

    rewrite_receipt_identity!(
        address_fold_identity,
        AddressFoldIdentity,
        AddressFoldReceipt,
        b"omega.terminal-address-fold.v1\0",
        "address-fold",
        access,
    );

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AddressFoldError {
        SourceMismatch,
        UnsupportedInstruction,
        UnsupportedProducer,
        UnsupportedOffset,
        UnsupportedUse,
        ConstraintMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    failed_display!(AddressFoldError, "invalid address fold");
}

pub(crate) mod load_forwarding {
    pub(crate) mod admission {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
        use selected_instructions::{
            FrameStorageSlotId, SelectedCasePayloadTransport, SelectedFunction,
            SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance, SelectedMemoryAccess, SelectedMemoryAccessRole,
            SelectedStructuralTransport, SelectedSuccessor, SelectedValueTransport,
            VirtualRegisterId, VirtualRegisterOrigin, terminator_instruction,
        };
        use semantic_vocabulary::PlaceId;
        use terminal_psi::StructuralPlaceDeclaration;

        use super::StoredLoadForwardingError;
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::block_edges::{member_position, terminator_successors};
        use crate::rewrites::place_storage::{
            constant_index, extent_intersects, extent_reached_by, local_slot_is_place_storage,
            structural_place_declarations,
        };
        use crate::rewrites::window_hazards::{UnrosteredReach, unrostered_reach};

        pub(crate) struct Admission<'source> {
            pub block_index: usize,
            pub load_index: usize,
            pub load_id: SelectedInstructionId,
            pub provenance: SelectedInstructionProvenance,
            pub value: VirtualRegisterId,
            pub output: VirtualRegisterId,
            pub load_access: usize,
            pub copy: &'source RegisterInstructionConstraint,
            pub kind: SelectedInstructionKind,
        }

        struct Forwarded {
            place: PlaceId,
            byte_offset: u32,
            byte_count: u32,
            sequence_index: Option<semantic_vocabulary::ValueId>,
        }

        impl Forwarded {
            fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
                extent_intersects(
                    self.byte_offset,
                    self.byte_count,
                    self.sequence_index.is_some(),
                    access,
                )
            }

            fn reached_by(
                &self,
                access: &SelectedMemoryAccess,
                function: &SelectedFunction,
            ) -> bool {
                extent_reached_by(
                    self.byte_offset,
                    self.byte_count,
                    self.sequence_index.is_some(),
                    access,
                    function,
                )
            }
        }

        pub(crate) fn admit<'source>(
            source: &'source impl ValidatedSelectedAnalysis,
            function_index: usize,
            load: SelectedInstructionId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<Admission<'source>, StoredLoadForwardingError> {
            let (plan, function) =
                crate::rewrites::plan_function(source, function_index, environment, || {
                    StoredLoadForwardingError::SourceMismatch
                })?;
            let (block_index, load_index) =
                member_position(function, load).ok_or(StoredLoadForwardingError::SourceMismatch)?;
            let block = &function.blocks[block_index];
            let forwarded_load = &block.instructions[load_index];
            let indexed = matches!(forwarded_load.kind, SelectedInstructionKind::Load8Indexed);
            let (encoded_offset, width): (u32, u32) = match forwarded_load.kind {
                SelectedInstructionKind::Load8 { byte_offset } => (byte_offset, 1),
                SelectedInstructionKind::Load16 { byte_offset } => (byte_offset, 2),
                SelectedInstructionKind::Load32 { byte_offset } => (byte_offset, 4),
                SelectedInstructionKind::Load64 { byte_offset } => (byte_offset, 8),
                SelectedInstructionKind::Load8Indexed => (0, 1),
                _ => return Err(StoredLoadForwardingError::UnsupportedInstruction),
            };
            let mut rows = function
                .memory_accesses
                .iter()
                .enumerate()
                .filter(|(_, access)| access.instruction == load);
            let (load_access, read) = rows
                .next()
                .ok_or(StoredLoadForwardingError::UnsupportedInstruction)?;
            if rows.next().is_some() {
                return Err(StoredLoadForwardingError::UnsupportedPair);
            }
            let sequence_index = match read.role {
                SelectedMemoryAccessRole::ReadPlace
                    if !indexed
                        && read.byte_offset == encoded_offset
                        && read.byte_count == width =>
                {
                    None
                }
                SelectedMemoryAccessRole::ReadByteSequence { index, .. }
                    if indexed && read.byte_count == width =>
                {
                    Some(index)
                }
                _ => return Err(StoredLoadForwardingError::UnsupportedPair),
            };
            let mut forwarded = Forwarded {
                place: read.place,
                byte_offset: read.byte_offset,
                byte_count: width,
                sequence_index,
            };
            if let Some(index) = forwarded.sequence_index
                && let Some(landed) = constant_index(function, index)
                && let Some(position) = u64::from(forwarded.byte_offset).checked_add(landed)
                && let Ok(position) = u32::try_from(position)
            {
                forwarded.byte_offset = position;
                forwarded.byte_count = 1;
                forwarded.sequence_index = None;
            }
            let output = single_def(forwarded_load)?;
            let output_register = function
                .virtual_register(output)
                .ok_or(StoredLoadForwardingError::UnsupportedPair)?;
            let VirtualRegisterOrigin::InstructionResult {
                instruction: output_instruction,
                ..
            } = output_register.origin
            else {
                return Err(StoredLoadForwardingError::UnsupportedUse);
            };
            if output_instruction != load {
                return Err(StoredLoadForwardingError::UnsupportedUse);
            }
            let load_row = environment
                .constraint(forwarded_load.constraint)
                .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
            let load_shape = if indexed {
                load_row.operands.len() == 3
                    && load_row.operands[0].operand == 0
                    && load_row.operands[0].access == RegisterOperandAccess::Use
                    && load_row.operands[1].operand == 1
                    && load_row.operands[1].access == RegisterOperandAccess::Use
                    && load_row.operands[2].operand == 2
                    && load_row.operands[2].access == RegisterOperandAccess::Def
                    && load_row.operands[2].class == output_register.class
            } else {
                load_row.operands.len() == 2
                    && load_row.operands[0].operand == 0
                    && load_row.operands[0].access == RegisterOperandAccess::Use
                    && load_row.operands[1].operand == 1
                    && load_row.operands[1].access == RegisterOperandAccess::Def
                    && load_row.operands[1].class == output_register.class
            };
            if !load_shape {
                return Err(StoredLoadForwardingError::ConstraintMismatch);
            }
            let structural_places = structural_place_declarations(function);
            let mut visited = vec![false; function.blocks.len()];
            let mut resolved = vec![None; function.blocks.len()];
            let mut deferred = Vec::new();
            let mut walked = Vec::new();
            let mut between = Vec::new();
            let mut crossed = Vec::new();
            let mut pending = vec![(block_index, load_index)];
            while let Some((cursor, cursor_end)) = pending.pop() {
                if visited[cursor] {
                    continue;
                }
                visited[cursor] = true;
                walked.push(cursor);
                let current = &function.blocks[cursor];
                let mut found = None;
                for candidate_index in (0..cursor_end).rev() {
                    let candidate = &current.instructions[candidate_index];
                    reject_barrier(candidate)?;
                    let (has_row, interfered) =
                        candidate_accesses(function, &forwarded, candidate, structural_places);
                    if interfered {
                        found = Some(forwarding_source(
                            candidate,
                            &forwarded,
                            function,
                            environment,
                        )?);
                        between.push((cursor, candidate_index + 1, cursor_end));
                        break;
                    }
                    if !has_row {
                        reject_unaccounted(candidate)?;
                    }
                }
                if let Some(value) = found {
                    resolved[cursor] = Some(value);
                    continue;
                }
                between.push((cursor, 0, cursor_end));
                if current.id == function.entry_block {
                    return Err(StoredLoadForwardingError::UnsupportedPair);
                }
                let mut predecessors = Vec::new();
                for (predecessor_index, predecessor) in function.blocks.iter().enumerate() {
                    let edges: Vec<&SelectedSuccessor> =
                        terminator_successors(&predecessor.terminator)
                            .into_iter()
                            .filter(|successor| successor.block == current.id)
                            .collect();
                    if edges.is_empty() {
                        continue;
                    }
                    let terminator = terminator_instruction(&predecessor.terminator);
                    if function.memory_accesses.iter().any(|access| {
                        access.instruction == terminator.id
                            && interferes(&forwarded, access, structural_places, function)
                    }) {
                        return Err(StoredLoadForwardingError::AliasingWrite);
                    }
                    crossed.extend(edges.iter().map(|edge| (predecessor_index, cursor, *edge)));
                    predecessors.push(predecessor_index);
                    if !visited[predecessor_index] {
                        pending.push((predecessor_index, predecessor.instructions.len()));
                    }
                }
                if predecessors.is_empty() {
                    return Err(StoredLoadForwardingError::UnsupportedPair);
                }
                deferred.push((cursor, predecessors));
            }
            let mut merged = vec![None; function.blocks.len()];
            let mut merge_work = 0usize;
            loop {
                if resolved[block_index].is_some() {
                    break;
                }
                let mut progressed = false;
                for (block, predecessors) in &deferred {
                    if resolved[*block].is_some() {
                        continue;
                    }
                    let mut values = predecessors
                        .iter()
                        .map(|predecessor| resolved[*predecessor]);
                    if let Some(Some(first)) = values.next()
                        && values.all(|value| value == Some(first))
                    {
                        resolved[*block] = Some(first);
                        progressed = true;
                    }
                }
                if !progressed {
                    break;
                }
            }
            let mut open = vec![false; function.blocks.len()];
            for (block, _) in &deferred {
                open[*block] = resolved[*block].is_none();
            }
            let mut conflicted = vec![false; function.blocks.len()];
            loop {
                let mut progressed = false;
                for (block, predecessors) in &deferred {
                    if !open[*block] || conflicted[*block] {
                        continue;
                    }
                    if predecessors
                        .iter()
                        .all(|predecessor| resolved[*predecessor].is_some())
                    {
                        let mut legs = predecessors
                            .iter()
                            .map(|predecessor| resolved[*predecessor].unwrap());
                        let first = legs.next().unwrap();
                        if legs.all(|leg| leg == first) {
                            if resolved[*block] != Some(first) {
                                resolved[*block] = Some(first);
                                progressed = true;
                            }
                            merged[*block] = None;
                        } else if let Some(parameter) = merged_parameter(
                            *block,
                            predecessors,
                            &resolved,
                            function,
                            &mut merge_work,
                        ) {
                            if resolved[*block] != Some(parameter) {
                                resolved[*block] = Some(parameter);
                                progressed = true;
                            }
                            merged[*block] = Some(parameter);
                        } else {
                            conflicted[*block] = true;
                            resolved[*block] = None;
                            merged[*block] = None;
                            progressed = true;
                        }
                        continue;
                    }
                    let mut candidate = None;
                    let mut conflict = false;
                    for predecessor in predecessors {
                        if conflicted[*predecessor] {
                            conflict = true;
                            break;
                        }
                        if let Some(register) = resolved[*predecessor] {
                            match candidate {
                                None => candidate = Some(register),
                                Some(current) if current == register => {}
                                Some(_) => {
                                    conflict = true;
                                    break;
                                }
                            }
                        }
                    }
                    if conflict {
                        conflicted[*block] = true;
                        resolved[*block] = None;
                        merged[*block] = None;
                        progressed = true;
                    } else if resolved[*block] != candidate {
                        resolved[*block] = candidate;
                        merged[*block] = None;
                        progressed = true;
                    }
                }
                if !progressed {
                    break;
                }
            }
            let value = resolved[block_index].ok_or(StoredLoadForwardingError::UnsupportedPair)?;
            let head_looped = {
                let mut deferred_block = vec![false; function.blocks.len()];
                for (block, _) in &deferred {
                    deferred_block[*block] = true;
                }
                let mut seen = vec![false; function.blocks.len()];
                let mut stack: Vec<usize> = deferred
                    .iter()
                    .find(|(block, _)| *block == block_index)
                    .map(|(_, predecessors)| {
                        predecessors
                            .iter()
                            .copied()
                            .filter(|predecessor| deferred_block[*predecessor])
                            .collect()
                    })
                    .unwrap_or_default();
                let mut loops = false;
                while let Some(block) = stack.pop() {
                    if block == block_index {
                        loops = true;
                        break;
                    }
                    if seen[block] {
                        continue;
                    }
                    seen[block] = true;
                    if let Some((_, predecessors)) =
                        deferred.iter().find(|(entry, _)| *entry == block)
                    {
                        stack.extend(
                            predecessors
                                .iter()
                                .copied()
                                .filter(|predecessor| deferred_block[*predecessor]),
                        );
                    }
                }
                loops
            };
            if head_looped {
                let tail = &function.blocks[block_index].instructions[load_index + 1..];
                for candidate in tail {
                    reject_barrier(candidate)?;
                    let (has_row, interfered) =
                        candidate_accesses(function, &forwarded, candidate, structural_places);
                    if interfered {
                        if forwarding_source(candidate, &forwarded, function, environment)? != value
                        {
                            return Err(StoredLoadForwardingError::AliasingWrite);
                        }
                        continue;
                    }
                    if !has_row {
                        reject_unaccounted(candidate)?;
                    }
                }
                between.push((
                    block_index,
                    load_index + 1,
                    function.blocks[block_index].instructions.len(),
                ));
            }
            if value == output {
                return Err(StoredLoadForwardingError::UnsupportedUse);
            }
            let value_register = function
                .virtual_register(value)
                .ok_or(StoredLoadForwardingError::UnsupportedPair)?;
            for (predecessor, target, successor) in &crossed {
                edge_preserves(
                    successor,
                    &forwarded,
                    resolved[*predecessor],
                    merged[*target],
                    output,
                )?;
            }
            for (walked_block, start, end) in &between {
                for instruction in &function.blocks[*walked_block].instructions[*start..*end] {
                    registers_untouched(instruction, resolved[*walked_block], output)?;
                }
            }
            for walked_block in &walked {
                if *walked_block != block_index || head_looped {
                    registers_untouched(
                        terminator_instruction(&function.blocks[*walked_block].terminator),
                        resolved[*walked_block],
                        output,
                    )?;
                }
            }
            let copy = crate::rewrites::integer_copy_row(
                environment,
                value_register.class,
                output_register.class,
                || StoredLoadForwardingError::ConstraintMismatch,
            )?;
            let kind = match forwarded.byte_count {
                8 => SelectedInstructionKind::CopyI64,
                4 => SelectedInstructionKind::ZeroExtendU32,
                2 => SelectedInstructionKind::ZeroExtendU16,
                _ => SelectedInstructionKind::ZeroExtendU8,
            };
            let interval = between
                .iter()
                .try_fold(0usize, |distance, (_, start, end)| {
                    distance.checked_add(end - start)
                })
                .and_then(|total| total.checked_add(crossed.len()))
                .and_then(|total| total.checked_add(merge_work))
                .ok_or(StoredLoadForwardingError::IdentityOverflow)?;
            let steps = crate::rewrites::rewrite_scan(plan, function)
                .and_then(|total| total.checked_add(interval))
                .and_then(|total| total.checked_add(function.memory_accesses.len()))
                .ok_or(StoredLoadForwardingError::IdentityOverflow)?;
            crate::rewrites::charge_steps(
                steps,
                budget,
                || StoredLoadForwardingError::IdentityOverflow,
                || StoredLoadForwardingError::WorkBudgetExceeded,
            )?;
            Ok(Admission {
                block_index,
                load_index,
                load_id: load,
                provenance: forwarded_load.provenance.clone(),
                value,
                output,
                load_access,
                copy,
                kind,
            })
        }

        fn single_def(
            instruction: &SelectedInstruction,
        ) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
            let mut defs = instruction
                .operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Def);
            let result = defs
                .next()
                .ok_or(StoredLoadForwardingError::UnsupportedInstruction)?;
            if defs.next().is_some()
                || result.fixed_view.is_some()
                || result.tied_to.is_some()
                || result.early_clobber
            {
                return Err(StoredLoadForwardingError::UnsupportedUse);
            }
            Ok(result.virtual_register)
        }

        fn interferes(
            forwarded: &Forwarded,
            access: &SelectedMemoryAccess,
            structural_places: &[StructuralPlaceDeclaration],
            function: &SelectedFunction,
        ) -> bool {
            match access.role {
                SelectedMemoryAccessRole::WriteBacking
                | SelectedMemoryAccessRole::WriteBackingSpan => true,
                SelectedMemoryAccessRole::WritePlace => {
                    access.place != forwarded.place || forwarded.intersects(access)
                }
                SelectedMemoryAccessRole::WriteByteSequence { .. }
                | SelectedMemoryAccessRole::WriteByteSpan { .. }
                | SelectedMemoryAccessRole::WriteElementView { .. }
                | SelectedMemoryAccessRole::WriteIndexedPrimitive { .. }
                | SelectedMemoryAccessRole::WriteIndexedStructural { .. } => {
                    access.place != forwarded.place || forwarded.reached_by(access, function)
                }
                SelectedMemoryAccessRole::WriteLocal { slot } => {
                    (access.place != forwarded.place
                        && local_slot_is_place_storage(slot, access.place, structural_places))
                        || (local_slot_is_place_storage(slot, forwarded.place, structural_places)
                            && forwarded.intersects(access))
                }
                SelectedMemoryAccessRole::AddressLocal { slot } => {
                    local_slot_is_place_storage(slot, access.place, structural_places)
                }
                SelectedMemoryAccessRole::ReadReturnedView { .. }
                | SelectedMemoryAccessRole::ReadBacking
                | SelectedMemoryAccessRole::ReadBackingSpan
                | SelectedMemoryAccessRole::ReadLocal { .. }
                | SelectedMemoryAccessRole::ReadPlace
                | SelectedMemoryAccessRole::ReadByteSpan { .. }
                | SelectedMemoryAccessRole::ReadByteSequence { .. }
                | SelectedMemoryAccessRole::ReadMeasuredSpan { .. }
                | SelectedMemoryAccessRole::ReadElementView { .. }
                | SelectedMemoryAccessRole::ReadIndexedPrimitive { .. }
                | SelectedMemoryAccessRole::WriteOutgoing { .. }
                | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
            }
        }

        fn forwarding_source(
            instruction: &SelectedInstruction,
            forwarded: &Forwarded,
            function: &SelectedFunction,
            environment: &ValidatedTargetRegisterEnvironment,
        ) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
            let reject = || StoredLoadForwardingError::AliasingWrite;
            let structural_places = structural_place_declarations(function);
            let mut rows = function
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == instruction.id);
            let Some(row) = rows.next() else {
                return Err(reject());
            };
            if rows.next().is_some() || row.place != forwarded.place {
                return Err(reject());
            }
            if let SelectedMemoryAccessRole::WriteByteSequence { index, .. } = row.role {
                return sequence_source(instruction, forwarded, row, index, function, environment);
            }
            if row.byte_offset != forwarded.byte_offset || row.byte_count != forwarded.byte_count {
                return Err(reject());
            }
            match instruction.kind {
                SelectedInstructionKind::Store {
                    byte_offset,
                    byte_size,
                } => {
                    let place_route = match (forwarded.sequence_index, row.role) {
                        (None, SelectedMemoryAccessRole::WritePlace) => {
                            byte_offset == forwarded.byte_offset
                                && u32::from(byte_size) == forwarded.byte_count
                        }
                        (None, SelectedMemoryAccessRole::WriteLocal { slot }) => {
                            byte_offset == forwarded.byte_offset
                                && u32::from(byte_size) == forwarded.byte_count
                                && local_slot_is_place_storage(
                                    slot,
                                    forwarded.place,
                                    structural_places,
                                )
                        }
                        _ => false,
                    };
                    if !place_route {
                        return Err(reject());
                    }
                    place_store_value(instruction, environment)
                }
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset,
                } => {
                    if byte_offset != forwarded.byte_offset || forwarded.byte_count != 8 {
                        return Err(reject());
                    }
                    if row.role != (SelectedMemoryAccessRole::WriteLocal { slot })
                        || !local_slot_is_place_storage(slot, forwarded.place, structural_places)
                    {
                        return Err(reject());
                    }
                    if environment.selected_keys().store64 != Some(instruction.constraint) {
                        return Err(StoredLoadForwardingError::ConstraintMismatch);
                    }
                    let row_constraint = environment
                        .constraint(instruction.constraint)
                        .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
                    let mut stored = row_constraint
                        .operands
                        .iter()
                        .filter(|operand| operand.access == RegisterOperandAccess::Use);
                    if stored.next().is_none_or(|operand| operand.operand != 0)
                        || stored.next().is_some()
                    {
                        return Err(StoredLoadForwardingError::ConstraintMismatch);
                    }
                    let mut uses = instruction
                        .operands
                        .iter()
                        .filter(|operand| operand.access == RegisterOperandAccess::Use);
                    let value = uses.next().ok_or_else(reject)?;
                    if uses.next().is_some() || value.operand != 0 {
                        return Err(reject());
                    }
                    Ok(value.virtual_register)
                }
                _ => Err(reject()),
            }
        }

        fn place_store_value(
            instruction: &SelectedInstruction,
            environment: &ValidatedTargetRegisterEnvironment,
        ) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
            let reject = || StoredLoadForwardingError::AliasingWrite;
            let row_constraint = environment
                .constraint(instruction.constraint)
                .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
            if !crate::rewrites::declares_pair(
                row_constraint,
                RegisterOperandAccess::Use,
                RegisterOperandAccess::Use,
            ) {
                return Err(StoredLoadForwardingError::ConstraintMismatch);
            }
            let mut uses = instruction
                .operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Use);
            let pointer = uses.next().ok_or_else(reject)?;
            let value = uses.next().ok_or_else(reject)?;
            if uses.next().is_some() || pointer.operand != 0 || value.operand != 1 {
                return Err(reject());
            }
            Ok(value.virtual_register)
        }

        fn sequence_source(
            instruction: &SelectedInstruction,
            forwarded: &Forwarded,
            row: &SelectedMemoryAccess,
            written: semantic_vocabulary::ValueId,
            function: &SelectedFunction,
            environment: &ValidatedTargetRegisterEnvironment,
        ) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
            let reject = || StoredLoadForwardingError::AliasingWrite;
            if !matches!(
                instruction.kind,
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 1
                }
            ) || row.byte_count != 1
            {
                return Err(reject());
            }
            let lands_on_read = match forwarded.sequence_index {
                None => {
                    forwarded.byte_count == 1
                        && constant_index(function, written)
                            .and_then(|landed| u64::from(row.byte_offset).checked_add(landed))
                            == Some(u64::from(forwarded.byte_offset))
                }
                Some(index) if written == index => row.byte_offset == forwarded.byte_offset,
                Some(index) => match (
                    constant_index(function, written),
                    constant_index(function, index),
                ) {
                    (Some(written), Some(read)) => {
                        u64::from(row.byte_offset).checked_add(written)
                            == u64::from(forwarded.byte_offset).checked_add(read)
                    }
                    _ => false,
                },
            };
            if !lands_on_read {
                return Err(reject());
            }
            place_store_value(instruction, environment)
        }

        fn edge_preserves(
            successor: &SelectedSuccessor,
            _forwarded: &Forwarded,
            carried: Option<VirtualRegisterId>,
            defined: Option<VirtualRegisterId>,
            output: VirtualRegisterId,
        ) -> Result<(), StoredLoadForwardingError> {
            for binding in &successor.bindings {
                if let SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } = binding.transport
                {
                    if Some(parameter) == defined {
                        if Some(argument) != carried {
                            return Err(StoredLoadForwardingError::UnsupportedUse);
                        }
                        continue;
                    }
                    if Some(parameter) == carried || parameter == output {
                        return Err(StoredLoadForwardingError::UnsupportedUse);
                    }
                }
            }
            for binding in &successor.structural_bindings {
                let destination = match binding.transport {
                    SelectedStructuralTransport::Unused => continue,
                    SelectedStructuralTransport::WholeValue { destination, .. }
                    | SelectedStructuralTransport::ScalarReference { destination, .. }
                    | SelectedStructuralTransport::Address { destination, .. } => destination,
                };
                if destination.structural_place().is_some() {
                    return Err(StoredLoadForwardingError::AliasingWrite);
                }
            }
            if let Some(case) = &successor.structural_case {
                if case.source.place().is_some() || !case.cleanup_actions.is_empty() {
                    return Err(StoredLoadForwardingError::AliasingWrite);
                }
                for payload in &case.payloads {
                    let parameter = match payload.transport {
                        SelectedCasePayloadTransport::Unused => continue,
                        SelectedCasePayloadTransport::Unmaterialized { parameter }
                        | SelectedCasePayloadTransport::Registers { parameter, .. } => parameter,
                    };
                    if Some(parameter) == carried
                        || Some(parameter) == defined
                        || parameter == output
                    {
                        return Err(StoredLoadForwardingError::UnsupportedUse);
                    }
                }
            }
            Ok(())
        }

        fn registers_untouched(
            instruction: &SelectedInstruction,
            carried: Option<VirtualRegisterId>,
            output: VirtualRegisterId,
        ) -> Result<(), StoredLoadForwardingError> {
            for operand in &instruction.operands {
                if operand.access != RegisterOperandAccess::Use
                    && (Some(operand.virtual_register) == carried
                        || operand.virtual_register == output)
                {
                    return Err(StoredLoadForwardingError::UnsupportedUse);
                }
            }
            Ok(())
        }

        fn merged_parameter(
            block: usize,
            predecessors: &[usize],
            resolved: &[Option<VirtualRegisterId>],
            function: &SelectedFunction,
            work: &mut usize,
        ) -> Option<VirtualRegisterId> {
            let block_id = function.blocks[block].id;
            for register in &function.virtual_registers {
                let VirtualRegisterOrigin::BlockParameter {
                    block: parameter_block,
                    ..
                } = register.origin
                else {
                    continue;
                };
                if parameter_block != block_id {
                    continue;
                }
                let mut binds = true;
                for predecessor in predecessors {
                    for edge in terminator_successors(&function.blocks[*predecessor].terminator) {
                        if edge.block != block_id {
                            continue;
                        }
                        *work += 1;
                        let mut arguments =
                            edge.bindings
                                .iter()
                                .filter_map(|binding| match binding.transport {
                                    SelectedValueTransport::Registers {
                                        argument,
                                        parameter,
                                    } if parameter == register.id => Some(argument),
                                    _ => None,
                                });
                        let Some(argument) = arguments.next() else {
                            binds = false;
                            break;
                        };
                        if arguments.next().is_some() || Some(argument) != resolved[*predecessor] {
                            binds = false;
                            break;
                        }
                    }
                    if !binds {
                        break;
                    }
                }
                if binds {
                    return Some(register.id);
                }
            }
            None
        }

        fn candidate_accesses(
            function: &SelectedFunction,
            forwarded: &Forwarded,
            candidate: &SelectedInstruction,
            structural_places: &[StructuralPlaceDeclaration],
        ) -> (bool, bool) {
            let mut has_row = false;
            let mut interfered = false;
            for access in function
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == candidate.id)
            {
                has_row = true;
                interfered |= interferes(forwarded, access, structural_places, function);
            }
            (has_row, interfered)
        }

        fn reject_barrier(
            instruction: &SelectedInstruction,
        ) -> Result<(), StoredLoadForwardingError> {
            use SelectedInstructionKind::*;
            match instruction.kind {
                CallUnit { .. }
                | CallScalar { .. }
                | CallAggregate { .. }
                | HostedReadByte { .. }
                | HostedWriteByteI32 { .. }
                | HostedExitProcessI32
                | ReturnScalar
                | ReturnAggregate { .. }
                | ReturnUnit
                | Jump
                | ConditionalBranchNonZero
                | ConditionalBranchU64LessThan
                | ConditionalBranchI64LessThan => {
                    Err(StoredLoadForwardingError::UnsupportedInstruction)
                }
                _ => Ok(()),
            }
        }

        fn reject_unaccounted(
            instruction: &SelectedInstruction,
        ) -> Result<(), StoredLoadForwardingError> {
            match unrostered_reach(instruction) {
                UnrosteredReach::Private => Ok(()),
                UnrosteredReach::Store => Err(StoredLoadForwardingError::AliasingWrite),
                UnrosteredReach::Unaccounted => {
                    Err(StoredLoadForwardingError::UnsupportedInstruction)
                }
            }
        }

        pub(crate) fn forwarded(admitted: &Admission<'_>) -> SelectedInstruction {
            crate::rewrites::selected_operands::copy_shaped(
                admitted.load_id,
                admitted.kind,
                admitted.copy,
                admitted.value,
                admitted.output,
                admitted.provenance.clone(),
            )
        }

        #[cfg(test)]
        mod backing_tests {
            use super::*;

            #[test]
            fn backing_write_cannot_be_disjoint_by_descriptor_identity_or_offset() {
                let function = SelectedFunction {
                    machine: semantic_vocabulary::MachineId::new(1).unwrap(),
                    attachment: None,
                    nominal_cleanup_subject: None,
                    provenance: target_operations::TerminalPsiProvenance {
                        operations: vec![],
                        edges: vec![],
                    },
                    structural: None,
                    local_storage_slots: vec![],
                    outgoing_arguments: vec![],
                    calls: vec![],
                    normalized_foreign_calls: vec![],
                    memory_accesses: vec![],
                    boundary_settlements: vec![],
                    entry_block: selected_instructions::SelectedBlockId(0),
                    virtual_registers: vec![],
                    blocks: vec![].into(),
                };
                let forwarded = Forwarded {
                    place: PlaceId::new(2).unwrap(),
                    byte_offset: 0,
                    byte_count: 8,
                    sequence_index: None,
                };
                for place in [forwarded.place, PlaceId::new(3).unwrap()] {
                    let mut access = SelectedMemoryAccess {
                        instruction: SelectedInstructionId(0),
                        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                            semantic_vocabulary::OperationId::new(1).unwrap(),
                        ),
                        place,
                        byte_offset: 4096,
                        byte_count: 8,
                        role: SelectedMemoryAccessRole::WriteBacking,
                    };
                    assert!(interferes(&forwarded, &access, &[], &function));
                    access.role = SelectedMemoryAccessRole::ReadBacking;
                    assert!(!interferes(&forwarded, &access, &[], &function));
                }
            }
        }
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::SelectedInstructionId;

        use super::{StoredLoadForwardingError, ValidatedStoredLoadForwarding, admission};
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn forward_selected_stored_load(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            load: SelectedInstructionId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedStoredLoadForwarding, StoredLoadForwardingError> {
            let admitted = admission::admit(source, function_index, load, environment, budget)?;
            let mut transformed = source.selected_plan().clone();
            let function = &mut transformed.functions[function_index];
            function.blocks[admitted.block_index].instructions[admitted.load_index] =
                admission::forwarded(&admitted);
            function.memory_accesses.remove(admitted.load_access);
            super::validation::seal_stored_load_forwarding(
                source,
                function_index,
                load,
                transformed,
            )
        }
    }
    pub(crate) mod validation {
        use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};

        use super::{
            StoredLoadForwardingError, StoredLoadForwardingReceipt, ValidatedStoredLoadForwarding,
        };
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn seal_stored_load_forwarding(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            load: SelectedInstructionId,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedStoredLoadForwarding, StoredLoadForwardingError> {
            let transformed_selected = selected_instruction_plan_identity(&proposed);
            let mut canonical = b"omega.stored-load-forwarding.v1".to_vec();
            canonical.extend_from_slice(&source.selected_identity().bytes());
            canonical.extend_from_slice(&transformed_selected.bytes());
            canonical.extend_from_slice(&source.optimization_unit_identity().bytes());
            crate::codec_bytes::push_encoded(
                &mut canonical,
                source.work_schedule_identity().marker().to_le_bytes(),
            );
            crate::codec_bytes::push_encoded(&mut canonical, (function_index as u64).to_le_bytes());
            crate::codec_bytes::push_encoded(&mut canonical, load.0.to_le_bytes());
            Ok(sealed!(ValidatedStoredLoadForwarding,
                StoredLoadForwardingReceipt,
                source,
                proposed,
                identity: selected_instructions::StoredLoadForwardingIdentity::from_canonical_bytes(
                    &canonical
                ),
                function_index: function_index,
                load: load
            ))
        }
        use crate::rewrites::sealed;
        use selected_instructions::selected_instruction_plan_identity;
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{
        SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
        StoredLoadForwardingIdentity,
    };
    use semantic_vocabulary::WorkScheduleIdentity;

    pub(crate) use rewrite::forward_selected_stored_load;

    validated_transformed_receipt!(ValidatedStoredLoadForwarding, StoredLoadForwardingReceipt);

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StoredLoadForwardingReceipt {
        identity: StoredLoadForwardingIdentity,
        function_index: usize,
        load: SelectedInstructionId,
        source_selected: SelectedInstructionPlanIdentity,
        transformed_selected: SelectedInstructionPlanIdentity,
        optimization_unit: OptimizationUnitIdentity,
        work_schedule: WorkScheduleIdentity,
    }

    impl StoredLoadForwardingReceipt {
        receipt_accessor_rows!(ref StoredLoadForwardingReceipt {
            identity: StoredLoadForwardingIdentity,
            function_index: usize,
            load: SelectedInstructionId,
        });
    }

    receipt_selected_identities!(StoredLoadForwardingReceipt);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum StoredLoadForwardingError {
        SourceMismatch,
        UnsupportedInstruction,
        UnsupportedPair,
        UnsupportedUse,
        AliasingWrite,
        ConstraintMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    failed_display!(StoredLoadForwardingError, "invalid stored-load forwarding");
}
