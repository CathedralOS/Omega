/*
We drive instruction selection (`select_instructions`), validating target
register constraint keys, preparing foreign scalar backings and installed
interrupt helpers, and sealing the constructed `SelectedInstructionPlan` with
its `SelectedInstructionValidationReceipt`.
*/

pub(crate) use crate::selection_inputs::activation_storage_input;
pub(crate) use crate::selection_inputs::address_join_input;
pub(crate) use crate::selection_inputs::aggregate_result_input;
pub(crate) use crate::selection_inputs::arrival_parameter_input;
pub(crate) mod atomic_access {
    use crate::structural_inputs::structural_reference_input::{
        PrimitiveAccessGeometry, atomic_cell_geometry,
    };
    use legalized_operations::{LegalizedScalarInstruction, LegalizedScalarInstructionKind};
    use selected_instructions::AtomicForm;
    use semantic_vocabulary::ScalarType;
    use terminal_psi::StructuralAccess;

    pub(crate) fn form(
        source: &crate::native_geometry::NativeFunction<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Option<(AtomicForm, PrimitiveAccessGeometry, ScalarType)> {
        let LegalizedScalarInstructionKind::AtomicAccess {
            location,
            scalar_type,
            field_byte_offset,
            event,
            outcome,
        } = &row.kind
        else {
            return None;
        };
        let form = AtomicForm::from_access(*scalar_type, *event)?;
        let signature = source.structural.as_ref()?;
        let root = super::reference_input::root_type(source, location.place)?;
        let root_access = if let Some(parameter) =
            super::arrival_parameter_input::parameter(source, location.place)
        {
            parameter.access
        } else {
            super::aggregate_result_input::home(source, location.place)?;
            StructuralAccess::Owned
        };
        if root_access != location.access
            || event.observes_resident() && root_access == StructuralAccess::WriteOnlyBorrow
            || row.result.is_some()
                != (event.observes_resident() && event.single_attempt_custody().is_none())
        {
            return None;
        }
        match (event.single_attempt_custody(), outcome) {
            (None, None) => {}
            (Some(custody), Some((result, layout)))
                if terminal_psi::types::atomic_once_result_is_plain(result, custody)
                    && signature
                        .structural_types
                        .iter()
                        .find(|row| row.id == result.structural_type)
                        .and_then(|row| {
                            target_operations::conventional_atomic_once_layout(
                                row,
                                *scalar_type,
                                custody,
                            )
                        })
                        .as_ref()
                        == Some(layout) => {}
            _ => return None,
        }
        let cell = terminal_semantics::atomic_cell_type(
            signature.structural_types.iter(),
            root,
            &location.path,
        )?;
        let logical_type = cell.scalar_type();
        if target_operations::operations::atomic_storage_integer_type(*event, logical_type)?
            != *scalar_type
            || row
                .result
                .is_some_and(|result| result.scalar_type != logical_type)
        {
            return None;
        }
        let geometry = atomic_cell_geometry(root, &location.path, logical_type, source.geometry())?;
        (geometry.byte_offset == *field_byte_offset
            && geometry.byte_size == u32::from(form.width.bytes())
            && geometry.byte_offset % geometry.byte_size == 0)
            .then_some((form, geometry, logical_type))
    }
}
pub(crate) mod block_order {
    use super::SelectedInstructionError;
    use legalized_operations::LegalizedScalarTerminator;

    pub(crate) fn derive(
        source: &crate::native_geometry::NativeFunction<'_>,
    ) -> Result<Vec<usize>, SelectedInstructionError> {
        let entry = source
            .block_index(source.entry_block)
            .ok_or(SelectedInstructionError::custody())?;
        let mut outgoing = Vec::new();
        for block in &source.blocks {
            let targets = match &block.terminator {
                LegalizedScalarTerminator::Crash { .. }
                | LegalizedScalarTerminator::InvokeNoNormal { .. }
                | LegalizedScalarTerminator::ExternalCompletion { .. }
                | LegalizedScalarTerminator::Return(_)
                | LegalizedScalarTerminator::ReturnCleanup { .. } => Vec::new(),
                LegalizedScalarTerminator::Jump { successor, .. } => vec![successor.target],
                LegalizedScalarTerminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![when_true.target, when_false.target],
                LegalizedScalarTerminator::StructuralCase { cases, .. } => {
                    cases.iter().map(|case| case.target).collect()
                }
            };
            outgoing.push(
                targets
                    .into_iter()
                    .map(|target| {
                        source
                            .block_index(target)
                            .ok_or(SelectedInstructionError::custody())
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        ordered_blocks(&outgoing, entry).ok_or(SelectedInstructionError::custody())
    }

    // Keep the existing preference for the lowest ready block. Cycles still use
    // the first unselected block in the authored depth-first fallback order.
    fn ordered_blocks(outgoing: &[Vec<usize>], entry: usize) -> Option<Vec<usize>> {
        let mut remaining = vec![0usize; outgoing.len()];
        for targets in outgoing {
            for target in targets {
                *remaining.get_mut(*target)? += 1;
            }
        }
        let mut visited = vec![false; outgoing.len()];
        let mut pending = vec![(entry, false)];
        let mut postorder = Vec::with_capacity(outgoing.len());
        while let Some((block, expanded)) = pending.pop() {
            if expanded {
                postorder.push(block);
                continue;
            }
            if *visited.get(block)? {
                continue;
            }
            visited[block] = true;
            pending.push((block, true));
            pending.extend(outgoing[block].iter().rev().map(|target| (*target, false)));
        }
        if postorder.len() != outgoing.len() {
            return None;
        }
        postorder.reverse();
        let mut ready: std::collections::BTreeSet<_> = remaining
            .iter()
            .enumerate()
            .filter_map(|(block, count)| (*count == 0).then_some(block))
            .collect();
        let mut selected = vec![false; outgoing.len()];
        let mut order = Vec::with_capacity(outgoing.len());
        let mut fallback_position = 0;
        let mut next = entry;
        loop {
            selected[next] = true;
            ready.remove(&next);
            order.push(next);
            // Count and retire parallel edges together. A predecessor is done
            // only after all of its outgoing edges have been accounted for.
            for target in &outgoing[next] {
                remaining[*target] -= 1;
                if remaining[*target] == 0 && !selected[*target] {
                    ready.insert(*target);
                }
            }
            if order.len() == outgoing.len() {
                return Some(order);
            }
            next = if let Some(block) = ready.first() {
                *block
            } else {
                while selected[*postorder.get(fallback_position)?] {
                    fallback_position += 1;
                }
                postorder[fallback_position]
            };
        }
    }

    #[cfg(test)]
    mod tests {
        use super::ordered_blocks;

        fn original_order(outgoing: &[Vec<usize>], entry: usize) -> Option<Vec<usize>> {
            let mut visited = vec![false; outgoing.len()];
            let mut pending = vec![(entry, false)];
            let mut postorder = Vec::new();
            while let Some((block, expanded)) = pending.pop() {
                if expanded {
                    postorder.push(block);
                    continue;
                }
                if *visited.get(block)? {
                    continue;
                }
                visited[block] = true;
                pending.push((block, true));
                pending.extend(outgoing[block].iter().rev().map(|target| (*target, false)));
            }
            if postorder.len() != outgoing.len() {
                return None;
            }
            postorder.reverse();
            let mut order = vec![entry];
            while order.len() < outgoing.len() {
                let next = (0..outgoing.len())
                    .find(|index| {
                        !order.contains(index)
                            && outgoing.iter().enumerate().all(|(predecessor, targets)| {
                                !targets.contains(index) || order.contains(&predecessor)
                            })
                    })
                    .or_else(|| {
                        postorder
                            .iter()
                            .copied()
                            .find(|index| !order.contains(index))
                    })?;
                order.push(next);
            }
            Some(order)
        }

        #[test]
        fn ready_blocks_match_original_order_for_all_small_graphs() {
            for count in 1..=4 {
                for edges in 0u32..(1 << (count * count)) {
                    let mut outgoing = vec![Vec::new(); count];
                    for (from, targets) in outgoing.iter_mut().enumerate() {
                        for to in 0..count {
                            if edges & (1 << (from * count + to)) != 0 {
                                targets.push(to);
                            }
                        }
                    }
                    for entry in 0..count {
                        assert_eq!(
                            ordered_blocks(&outgoing, entry),
                            original_order(&outgoing, entry),
                            "entry={entry}, outgoing={outgoing:?}"
                        );
                    }
                    for targets in &mut outgoing {
                        targets.reverse();
                        if let Some(first) = targets.first().copied() {
                            targets.push(first);
                        }
                    }
                    for entry in 0..count {
                        assert_eq!(
                            ordered_blocks(&outgoing, entry),
                            original_order(&outgoing, entry),
                            "entry={entry}, outgoing={outgoing:?}"
                        );
                    }
                }
            }
            assert_eq!(ordered_blocks(&[], 0), None);
            assert_eq!(ordered_blocks(&[vec![1]], 0), None);
            assert_eq!(ordered_blocks(&[vec![]], 1), None);
        }

        #[test]
        fn ready_blocks_preserve_a_long_reverse_chain() {
            let count: usize = 8192;
            let outgoing: Vec<_> = (0..count)
                .map(|position| position.checked_sub(1).into_iter().collect())
                .collect();
            assert_eq!(
                ordered_blocks(&outgoing, count - 1),
                Some((0..count).rev().collect())
            );
        }
    }
}
pub(crate) use crate::selection_inputs::byte_concatenation_input;
pub(crate) use crate::selection_inputs::byte_owner_input;
pub(crate) mod byte_view_homes {
    use legalized_operations::LegalizedScalarTerminator;
    use selected_instructions::VirtualRegisterId;
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind, ValueId};
    use std::collections::BTreeSet;

    use crate::SelectedInstructionError;

    #[derive(Clone, Copy)]
    pub(crate) struct ByteViewHomes {
        pub place: PlaceId,
        pub source: PlaceId,
        pub root: Option<PlaceId>,
        pub backing_pointer: VirtualRegisterId,
        pub byte_offset: VirtualRegisterId,
        pub byte_length: VirtualRegisterId,
        pub root_length: ValueId,
    }

    pub(crate) fn own_view_root(
        source: &crate::native_geometry::NativeFunction<'_>,
        place: PlaceId,
    ) -> Option<PlaceId> {
        match source
            .structural
            .as_ref()
            .and_then(|contract| contract.structural_place(place))
            .map(|declaration| declaration.kind)
        {
            Some(StructuralPlaceKind::BlockParameter { .. }) => None,
            _ => Some(place),
        }
    }

    pub(crate) fn view_backing_roots(
        source: &crate::native_geometry::NativeFunction<'_>,
        views: &[ByteViewHomes],
        place: PlaceId,
        extent: ValueId,
    ) -> Result<Option<Vec<(PlaceId, ValueId)>>, SelectedInstructionError> {
        if let Some(view) = views.iter().find(|view| view.place == place) {
            return match view.root {
                Some(root) => Ok(Some(vec![(root, view.root_length)])),
                None => bound_view_roots(
                    source,
                    views,
                    view.source,
                    view.root_length,
                    &mut BTreeSet::new(),
                )
                .map(Some),
            };
        }
        if crate::selection::established_view_input::view_type(source, place).is_none() {
            return Ok(None);
        }
        let Some(contract) = source.structural.as_ref() else {
            return Ok(None);
        };
        if !contract.structural_places.iter().any(|declaration| {
            declaration.id == place
                && matches!(declaration.kind, StructuralPlaceKind::BlockParameter { .. })
        }) {
            return Ok(None);
        }
        bound_view_roots(source, views, place, extent, &mut BTreeSet::new()).map(Some)
    }

    fn bound_view_roots(
        source: &crate::native_geometry::NativeFunction<'_>,
        views: &[ByteViewHomes],
        parameter: PlaceId,
        extent: ValueId,
        explored: &mut BTreeSet<PlaceId>,
    ) -> Result<Vec<(PlaceId, ValueId)>, SelectedInstructionError> {
        let roots = transitive_view_roots(source, views, parameter, extent, explored)?;
        if roots.is_empty() {
            return Err(SelectedInstructionError::custody());
        }
        Ok(roots)
    }

    fn transitive_view_roots(
        source: &crate::native_geometry::NativeFunction<'_>,
        views: &[ByteViewHomes],
        parameter: PlaceId,
        extent: ValueId,
        explored: &mut BTreeSet<PlaceId>,
    ) -> Result<Vec<(PlaceId, ValueId)>, SelectedInstructionError> {
        if !explored.insert(parameter) {
            return Ok(Vec::new());
        }
        if let Some(view) = views.iter().find(|view| view.place == parameter) {
            return match view.root {
                Some(root) => Ok(vec![(root, view.root_length)]),
                None => {
                    transitive_view_roots(source, views, view.source, view.root_length, explored)
                }
            };
        }
        bound_view_roots_recurse(source, views, parameter, extent, explored)
    }

    fn bound_view_roots_recurse(
        source: &crate::native_geometry::NativeFunction<'_>,
        views: &[ByteViewHomes],
        parameter: PlaceId,
        extent: ValueId,
        explored: &mut BTreeSet<PlaceId>,
    ) -> Result<Vec<(PlaceId, ValueId)>, SelectedInstructionError> {
        let Some(contract) = source.structural.as_ref() else {
            return Err(SelectedInstructionError::custody());
        };
        let Some(declaration) = contract.structural_place(parameter) else {
            return Err(SelectedInstructionError::custody());
        };
        let StructuralPlaceKind::BlockParameter { block, .. } = declaration.kind else {
            return Err(SelectedInstructionError::custody());
        };
        let bound_root = |place: PlaceId,
                          explored: &mut BTreeSet<PlaceId>|
         -> Result<Vec<(PlaceId, ValueId)>, SelectedInstructionError> {
            if views.iter().any(|view| view.place == place) {
                return transitive_view_roots(source, views, place, extent, explored);
            }
            let kind = contract
                .structural_place(place)
                .map(|declaration| declaration.kind);
            match kind {
                Some(
                    StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::ByteSequenceLiteral { .. }
                    | StructuralPlaceKind::OperationResult { .. },
                ) => Ok(vec![(place, extent)]),
                Some(StructuralPlaceKind::BlockParameter { .. }) => {
                    transitive_view_roots(source, views, place, extent, explored)
                }
                _ => Err(SelectedInstructionError::custody()),
            }
        };
        let mut bound_edges = 0usize;
        let mut unbound_edges = 0usize;
        let mut roots: Vec<(PlaceId, ValueId)> = Vec::new();
        for owner in &source.blocks {
            for (target, successor) in edge_successors(&owner.terminator) {
                if target != block {
                    continue;
                }
                let mut bound = false;
                for binding in successor {
                    let binding = &binding.semantic;
                    if binding.parameter != parameter {
                        continue;
                    }
                    if binding.argument.path.is_empty() {
                        bound = true;
                        for candidate in bound_root(binding.argument.place, explored)? {
                            if !roots.contains(&candidate) {
                                roots.push(candidate);
                            }
                        }
                    }
                }
                if bound {
                    bound_edges += 1;
                } else {
                    unbound_edges += 1;
                }
            }
        }
        if bound_edges == 0 || unbound_edges != 0 {
            return Err(SelectedInstructionError::custody());
        }
        Ok(roots)
    }

    fn edge_successors(
        terminator: &LegalizedScalarTerminator,
    ) -> Vec<(
        semantic_vocabulary::BlockId,
        &[legalized_operations::LegalizedStructuralBinding],
    )> {
        match terminator {
            LegalizedScalarTerminator::Jump { successor, .. } => {
                vec![(successor.target, &successor.structural_bindings)]
            }
            LegalizedScalarTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![
                (when_true.target, &when_true.structural_bindings),
                (when_false.target, &when_false.structural_bindings),
            ],
            LegalizedScalarTerminator::StructuralCase { cases, .. } => cases
                .iter()
                .map(|case| (case.target, case.structural_bindings.as_slice()))
                .collect(),
            _ => Vec::new(),
        }
    }
}
pub(crate) mod constraints {
    use crate::selection::model::SelectedInstructionError;
    use register_model::{
        RegisterConstraintKey, RegisterInstructionConstraint, ValidatedRegisterConstraintCatalog,
    };
    use selected_instructions::{
        SelectedConstraintKeys, SelectedFixedInputConstraint, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
        SelectedOperand, VirtualRegisterId,
    };

    pub(crate) fn instruction(
        id: SelectedInstructionId,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        registers: &[VirtualRegisterId],
        provenance: SelectedInstructionProvenance,
        catalog: &ValidatedRegisterConstraintCatalog,
    ) -> Result<SelectedInstruction, SelectedInstructionError> {
        let row = row(catalog, key)?;
        instruction_from_row(id, kind, registers, provenance, row)
    }

    pub(crate) fn instruction_from_row(
        id: SelectedInstructionId,
        kind: SelectedInstructionKind,
        registers: &[VirtualRegisterId],
        provenance: SelectedInstructionProvenance,
        row: &RegisterInstructionConstraint,
    ) -> Result<SelectedInstruction, SelectedInstructionError> {
        let key = row.key;
        if row.operands.len() != registers.len() {
            return Err(SelectedInstructionError::MissingConstraint(key));
        }
        Ok(SelectedInstruction {
            id,
            kind,
            constraint: key,
            operands: row
                .operands
                .iter()
                .zip(registers)
                .map(|(constraint, register)| SelectedOperand {
                    operand: constraint.operand,
                    virtual_register: *register,
                    access: constraint.access,
                    class: constraint.class,
                    fixed_view: constraint.fixed_view,
                    tied_to: constraint.tied_to,
                    early_clobber: constraint.early_clobber,
                })
                .collect(),
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance,
        })
    }

    pub(crate) fn require_key_rows(
        keys: &SelectedConstraintKeys,
        catalog: &ValidatedRegisterConstraintCatalog,
    ) -> Result<(), SelectedInstructionError> {
        for key in [
            keys.materialize_i64,
            keys.copy_i64,
            keys.add_i64,
            keys.add_i64_immediate,
            keys.compare_i64_zero,
            keys.compare_i64,
            keys.conditional_branch,
            keys.jump,
            keys.return_i64,
            keys.return_unit,
        ] {
            row(catalog, key)?;
        }
        Ok(())
    }

    pub(crate) fn row(
        catalog: &ValidatedRegisterConstraintCatalog,
        key: RegisterConstraintKey,
    ) -> Result<&RegisterInstructionConstraint, SelectedInstructionError> {
        catalog
            .constraint(key)
            .ok_or(SelectedInstructionError::MissingConstraint(key))
    }

    pub(crate) fn fixed_input_constraint(
        machine: semantic_vocabulary::MachineId,
        source_value: semantic_vocabulary::ValueId,
        parameter_index: usize,
        register: target_operations::MachineRegister,
        inputs: &[SelectedFixedInputConstraint],
    ) -> Option<&SelectedFixedInputConstraint> {
        let mut matches = inputs.iter().filter(|input| {
            input.machine == machine
                && input.source_value == source_value
                && input.parameter_index == parameter_index
                && input.register == register
        });
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    pub(crate) fn store_registers(
        instruction: SelectedInstructionId,
        inputs: &[VirtualRegisterId],
        row: &RegisterInstructionConstraint,
        registers: &mut Vec<selected_instructions::VirtualRegister>,
    ) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
        use register_model::RegisterOperandAccess;
        let reject = || SelectedInstructionError::MissingConstraint(row.key);
        let [value] = inputs else {
            return Err(reject());
        };
        if !matches!(row.operands.len(), 1 | 2)
            || row.operands[0].operand != 0
            || row.operands[0].access != RegisterOperandAccess::Use
        {
            return Err(reject());
        }
        let mut operands = vec![*value];
        if let Some(scratch) = row.operands.get(1) {
            if scratch.operand != 1
                || scratch.access != RegisterOperandAccess::Def
                || !scratch.early_clobber
                || scratch.tied_to.is_some()
                || scratch.fixed_view.is_some()
            {
                return Err(reject());
            }
            let id = VirtualRegisterId(registers.len().try_into().map_err(|_| reject())?);
            registers.push(selected_instructions::VirtualRegister {
                id,
                scalar_type: semantic_vocabulary::ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .map_err(|_| reject())?,
                ),
                class: scratch.class,
                origin: selected_instructions::VirtualRegisterOrigin::InstructionScratch {
                    instruction,
                    operand: scratch.operand,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            operands.push(id);
        }
        Ok(operands)
    }
}
pub(crate) mod construction {
    #[allow(unused_imports)]
    pub(crate) use crate::legalization::ValidatedLegalizedOperations;
    #[allow(unused_imports)]
    pub(crate) use crate::selection::model::SelectedInstructionError;
    pub(crate) use crate::selection_scalar_graph as scalar_graph;
    #[allow(unused_imports)]
    pub(crate) use selected_instructions::{SelectedInstructionPlan, SelectedSelectionConstraints};

    #[allow(unused_imports)]
    pub(crate) use crate::selection::constraints::require_key_rows;

    pub(crate) fn build_plan(
        legalized: &ValidatedLegalizedOperations,
        constraints: &SelectedSelectionConstraints,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
    ) -> Result<SelectedInstructionPlan, SelectedInstructionError> {
        let target = legalized.plan();
        if target.target != environment.target() {
            return Err(SelectedInstructionError::custody());
        }
        let catalog = environment.constraints();
        require_key_rows(&constraints.keys, catalog)?;
        let interrupt_helpers = crate::installed_interrupts::PreparedHelperCohort::new(
            legalized.installed_interrupts(),
        );
        let native_geometry = crate::native_geometry::NativeGeometries::new(
            target.scalar_functions.iter().map(|source| {
                source
                    .structural
                    .as_ref()
                    .map_or(&[][..], |contract| contract.structural_types.as_slice())
            }),
        )
        .map_err(|_| SelectedInstructionError::custody())?;
        let backings =
            super::foreign_scalar_backings::ForeignScalarBackings::new(target, &native_geometry);
        let mut functions = target
            .scalar_functions
            .iter()
            .enumerate()
            .map(|(index, source)| {
                scalar_graph::build_with_environment(
                    index,
                    source,
                    native_geometry
                        .get(
                            source
                                .structural
                                .as_ref()
                                .map_or(&[][..], |contract| contract.structural_types.as_slice()),
                        )
                        .ok_or_else(SelectedInstructionError::custody)?,
                    constraints,
                    environment,
                    &backings,
                    legalized.installed_interrupts(),
                    legalized.installed_stack_requirements(),
                    &interrupt_helpers,
                    legalized.installed_entry_execution(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        functions.sort_by_key(|function| function.machine);
        Ok(SelectedInstructionPlan {
            psi: target.psi,
            work_schedule: target.work_schedule,
            target: target.target,
            entry: target.entry,
            functions: functions.into(),
        })
    }
}
pub(crate) use crate::selection_edge_transfers as edge_transfers;
pub(crate) mod element_view_homes {
    use selected_instructions::VirtualRegisterId;
    use semantic_vocabulary::{PlaceId, ValueId};

    #[derive(Clone, Copy)]
    pub(crate) struct ElementViewHomes {
        pub place: PlaceId,
        pub backing_pointer: VirtualRegisterId,
        pub byte_offset: VirtualRegisterId,
        pub element_length: VirtualRegisterId,
        pub root_length: ValueId,
        pub element_stride: u32,
    }
}
pub(crate) use crate::selection_inputs::established_view_input;
pub(crate) mod extent_copy {
    use crate::SelectedInstructionError;
    use register_model::{
        RegisterConstraintKey, RegisterOperandAccess, ValidatedRegisterConstraintCatalog,
    };
    use selected_instructions::{
        SelectedInstructionId, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

    pub(crate) const INLINE_MAX_BYTES: u32 = 4096;

    pub(crate) fn chunks(byte_size: u32) -> impl Iterator<Item = (u32, u8)> {
        let mut offset = 0;
        std::iter::from_fn(move || {
            let width = [8u32, 4, 2, 1]
                .into_iter()
                .find(|width| *width <= byte_size - offset)?;
            let chunk = (offset, width as u8);
            offset += width;
            Some(chunk)
        })
    }

    pub(crate) fn bulk_operands(
        instruction: SelectedInstructionId,
        inputs: [VirtualRegisterId; 3],
        key: RegisterConstraintKey,
        catalog: &ValidatedRegisterConstraintCatalog,
        registers: &mut Vec<VirtualRegister>,
    ) -> Result<[VirtualRegisterId; 5], SelectedInstructionError> {
        let row = super::constraints::row(catalog, key)?;
        let reject = || SelectedInstructionError::MissingConstraint(key);
        if row.operands.len() != 5
            || row.operands.iter().enumerate().any(|(index, operand)| {
                operand.operand as usize != index
                    || operand.access
                        != if index < 3 {
                            RegisterOperandAccess::Use
                        } else {
                            RegisterOperandAccess::Def
                        }
                    || operand.tied_to.is_some()
                    || (index >= 3 && (!operand.early_clobber || operand.fixed_view.is_some()))
            })
        {
            return Err(reject());
        }
        let mut operands = [inputs[0], inputs[1], inputs[2], inputs[0], inputs[0]];
        for index in 3..5 {
            let id = VirtualRegisterId(registers.len().try_into().map_err(|_| reject())?);
            registers.push(VirtualRegister {
                id,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| reject())?,
                ),
                class: row.operands[index].class,
                origin: VirtualRegisterOrigin::InstructionScratch {
                    instruction,
                    operand: index as u16,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            operands[index] = id;
        }
        Ok(operands)
    }
}
pub(crate) mod field_bytes {
    use super::established_view_input::{element_view_type, view_access};

    pub(crate) struct FieldBytes {
        offset: u32,
        pub(crate) is_descriptor: bool,
        pub(crate) pointer_slots:
            Vec<crate::structural_inputs::structural_reference_input::PointerSlot>,
    }

    impl FieldBytes {
        pub(crate) fn resolve(
            function: &crate::native_geometry::NativeFunction<'_>,
            source: &terminal_psi::StructuralArgument,
            field: semantic_vocabulary::StructuralFieldId,
            indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
        ) -> Option<Self> {
            let structural_type = root_type(function, source)?;
            function.structural.as_ref()?;
            if source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return None;
            }
            let geometry =
                crate::structural_inputs::structural_reference_input::projected_byte_field(
                    structural_type,
                    &source.path,
                    field,
                    function.geometry(),
                )?;
            if !crate::structural_inputs::runtime_indices_match(indices, &geometry.elements) {
                return None;
            }
            let offset = if let Some(offset) = geometry.descriptor_offset {
                offset
            } else {
                geometry.capacity?;
                geometry.length_offset.checked_add(8)?
            };
            Some(Self {
                offset,
                is_descriptor: geometry.descriptor_offset.is_some(),
                pointer_slots: geometry.pointer_slots,
            })
        }

        pub(crate) const fn offset(&self) -> u32 {
            self.offset
        }
    }

    pub(crate) fn root_type(
        function: &crate::native_geometry::NativeFunction<'_>,
        source: &terminal_psi::StructuralArgument,
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        use terminal_psi::{StructuralAccess, StructuralMultiplicity};
        if let Some(parameter) =
            super::primitive_local_input::read_parameter(function, source.place)
        {
            if !access_covers(parameter.access, source.access)
                || parameter.multiplicity == StructuralMultiplicity::Linear
            {
                return None;
            }
            return Some(parameter.structural_type);
        }
        if let Some(structural_type) = element_view_type(function, source.place) {
            return access_covers(view_access(function, source.place)?, source.access)
                .then_some(structural_type);
        }
        let (_, result) = super::aggregate_result_input::home(function, source.place)?;
        (access_covers(StructuralAccess::Owned, source.access)
            && result.multiplicity != StructuralMultiplicity::Linear
            && result.claims.is_empty())
        .then_some(result.structural_type)
    }

    fn access_covers(
        available: terminal_psi::StructuralAccess,
        requested: terminal_psi::StructuralAccess,
    ) -> bool {
        use terminal_psi::StructuralAccess;
        available == requested
            || requested == StructuralAccess::SharedBorrow
                && matches!(available, terminal_psi::mutating_access_pair_pattern!())
    }
}
pub(crate) mod foreign_scalar_backings {
    use super::reference_input::{borrowed_scalar_source, root_type};
    use crate::SelectedInstructionError;
    use crate::structural_inputs::structural_reference_input::projected_scalar_codec;
    use legalized_operations::{
        LegalizedOperationPlan, LegalizedScalarArgument, LegalizedScalarCall,
        LegalizedScalarInstructionKind as I, LegalizedScalarTerminator as T,
    };
    use semantic_vocabulary::{MachineId, StructuralPlaceKind};
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{StructuralArgument, StructuralTypeShape};

    pub(crate) struct ForeignScalarBackings<'program> {
        program: &'program LegalizedOperationPlan,
        geometries: &'program crate::native_geometry::NativeGeometries<'program>,
        incoming: BTreeMap<MachineId, Vec<(usize, &'program LegalizedScalarCall)>>,
        opaque_ingress: BTreeSet<MachineId>,
    }
    struct Coordinate {
        function: usize,
        source: StructuralArgument,
        dependencies: Vec<usize>,
        backing: bool,
    }
    impl<'program> ForeignScalarBackings<'program> {
        pub(crate) fn new(
            program: &'program LegalizedOperationPlan,
            geometries: &'program crate::native_geometry::NativeGeometries<'program>,
        ) -> Self {
            let mut context = Self {
                program,
                geometries,
                incoming: BTreeMap::new(),
                opaque_ingress: BTreeSet::from([program.entry]),
            };
            for (position, function) in program.scalar_functions.iter().enumerate() {
                for instruction in function.instructions() {
                    if let I::Call(call) = &instruction.kind {
                        context.retain_call(position, call);
                    }
                }
                for block in &function.blocks {
                    if let T::InvokeNoNormal { call, .. } = &block.terminator {
                        context.retain_call(position, call);
                    }
                }
            }
            context
        }
        fn retain_call(&mut self, caller: usize, call: &'program LegalizedScalarCall) {
            match call.target {
                target_operations::NativeCallTarget::Machine(callee) => {
                    self.incoming
                        .entry(callee)
                        .or_default()
                        .push((caller, call));
                }
                target_operations::NativeCallTarget::Dynamic { .. } => {
                    self.opaque_ingress.extend(
                        self.program
                            .scalar_functions
                            .iter()
                            .filter(|function| function.call_plan == call.call_plan)
                            .map(|function| function.machine),
                    );
                }
            }
        }
        pub(crate) fn matches(
            &self,
            function: &crate::native_geometry::NativeFunction<'_>,
            source: &StructuralArgument,
            width: u32,
        ) -> Result<bool, SelectedInstructionError> {
            let function = self
                .program
                .scalar_functions
                .iter()
                .position(|candidate| std::ptr::eq(candidate, &**function))
                .ok_or_else(|| {
                    SelectedInstructionError::custody_because(
                        "foreign backing query lost its exact closed function",
                    )
                })?;
            let mut coordinates = vec![Coordinate {
                function,
                source: source.clone(),
                dependencies: Vec::new(),
                backing: false,
            }];
            let mut cursor = 0;
            while cursor < coordinates.len() {
                let function_index = coordinates[cursor].function;
                let function = &self.program.scalar_functions[function_index];
                let declarations = function
                    .structural
                    .as_ref()
                    .map_or(&[][..], |signature| signature.structural_types.as_slice());
                let geometry = self.geometries.get(declarations).ok_or_else(|| {
                    SelectedInstructionError::custody_because(
                        "foreign backing query lost its prepared native geometry",
                    )
                })?;
                let function = &crate::native_geometry::NativeInput::new(function, geometry);
                let Some(origins) = super::storage_origin_input::scalar_storage_origins(
                    function,
                    &coordinates[cursor].source,
                )?
                else {
                    return Ok(false);
                };
                let signature = function
                    .structural
                    .as_ref()
                    .ok_or_else(SelectedInstructionError::custody)?;
                for origin in origins {
                    let origin = match origin {
                        super::storage_origin_input::ScalarStorageOrigin::Width(actual) => {
                            if actual != width {
                                return Ok(false);
                            }
                            coordinates[cursor].backing = true;
                            continue;
                        }
                        super::storage_origin_input::ScalarStorageOrigin::Source(origin) => origin,
                    };
                    let declaration = signature
                        .structural_place(origin.place)
                        .ok_or_else(SelectedInstructionError::custody)?;
                    let pointer_ingress = borrowed_scalar_source(function, origin.place).is_some()
                        || origin
                            .path
                            .contains(&terminal_psi::StructuralPathSegment::Referent);
                    if !pointer_ingress {
                        let root = root_type(function, origin.place)
                            .ok_or_else(SelectedInstructionError::custody)?;
                        let footprint = inline_footprint(function, root, &origin.path);
                        if let Some(actual) = footprint {
                            if actual != width {
                                return Ok(false);
                            }
                            coordinates[cursor].backing = true;
                            continue;
                        }
                    }
                    if let StructuralPlaceKind::Parameter { position } = declaration.kind {
                        if self.opaque_ingress.contains(&function.machine) {
                            return Ok(false);
                        }
                        let Some(calls) = self
                            .incoming
                            .get(&function.machine)
                            .filter(|calls| !calls.is_empty())
                        else {
                            return Ok(false);
                        };
                        for (caller, call) in calls {
                            let actual = call
                                .arguments
                                .iter()
                                .filter_map(|argument| match argument {
                                    LegalizedScalarArgument::Structural { semantic, .. } => {
                                        Some(semantic)
                                    }
                                    _ => None,
                                })
                                .nth(position as usize)
                                .ok_or_else(|| {
                                    SelectedInstructionError::custody_because(
                                        "foreign backing ingress lost its actual structural argument",
                                    )
                                })?;
                            let mut source = actual.clone();
                            source.path.extend_from_slice(&origin.path);
                            let existing = coordinates.iter().position(|previous| {
                                previous.function == *caller
                                    && terminal_semantics::same_descriptor_coordinate(
                                        &previous.source,
                                        &source,
                                    )
                            });
                            let dependency = if let Some(existing) = existing {
                                existing
                            } else {
                                if coordinates.iter().any(|previous| {
                                    previous.function == *caller
                                        && previous.source.place == source.place
                                        && source.path.len() > previous.source.path.len()
                                        && source.path.starts_with(&previous.source.path)
                                }) {
                                    return Ok(false);
                                }
                                let dependency = coordinates.len();
                                coordinates.push(Coordinate {
                                    function: *caller,
                                    source,
                                    dependencies: Vec::new(),
                                    backing: false,
                                });
                                dependency
                            };
                            coordinates[cursor].dependencies.push(dependency);
                        }
                    } else {
                        return Ok(false);
                    }
                }
                cursor += 1;
            }
            let mut grounded = coordinates
                .iter()
                .map(|coordinate| coordinate.backing)
                .collect::<Vec<_>>();
            loop {
                let mut changed = false;
                for (position, coordinate) in coordinates.iter().enumerate() {
                    if !grounded[position]
                        && coordinate
                            .dependencies
                            .iter()
                            .any(|dependency| grounded[*dependency])
                    {
                        grounded[position] = true;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            Ok(grounded.into_iter().all(|grounded| grounded))
        }
    }

    fn inline_footprint(
        function: &crate::native_geometry::NativeFunction<'_>,
        root: semantic_vocabulary::StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Option<u32> {
        let signature = function.structural.as_ref()?;
        let codec = projected_scalar_codec(root, path, function.geometry());
        if let Some(codec) = codec {
            return Some(u32::from(codec.byte_size()));
        }
        if !path.is_empty() {
            return None;
        }
        let StructuralTypeShape::FixedArray { element, .. } =
            signature.structural_type(root)?.shape
        else {
            return None;
        };
        projected_scalar_codec(element, &[], function.geometry())
            .map(|codec| u32::from(codec.byte_size()))
    }
}
pub(crate) mod identity {
    use crate::legalization::ValidatedLegalizedOperations;
    use crate::selection::model::SelectedInstructionValidationReceipt;
    use selected_instructions::SelectedInstructionPlan;
    use selected_instructions::selected_instruction_plan_identity;

    pub(crate) fn receipt(
        plan: &SelectedInstructionPlan,
        legalized: &ValidatedLegalizedOperations,
    ) -> SelectedInstructionValidationReceipt {
        let function_count = plan.functions.len();
        let block_count = plan
            .functions
            .iter()
            .map(|function| function.blocks.len())
            .sum::<usize>();
        let virtual_register_count = plan
            .functions
            .iter()
            .map(|function| function.virtual_registers.len())
            .sum();
        let instruction_count = plan
            .blocks()
            .map(|block| block.instructions.len() + 1)
            .sum::<usize>();
        SelectedInstructionValidationReceipt {
            identity: selected_instruction_plan_identity(plan),
            legalized: legalized.receipt().identity(),
            legalization_validator: legalized.receipt().validator(),
            optimization_unit: legalized.receipt().optimization_unit(),
            work_schedule: legalized.receipt().work_schedule(),
            function_count,
            block_count,
            virtual_register_count,
            instruction_count,
        }
    }
}
pub(crate) mod integer_conversion {
    use legalized_operations::LegalizedScalarInstructionKind;
    use semantic_vocabulary::ScalarType;

    pub(crate) fn normalization(
        kind: &LegalizedScalarInstructionKind,
        scalar_type: ScalarType,
    ) -> Option<selected_instructions::SelectedInstructionKind> {
        if let LegalizedScalarInstructionKind::ScalarRepresentationView { source_type, .. } = kind {
            terminal_semantics::ScalarRepresentationViewPair::classify(*source_type, scalar_type)?;
            return Some(register_form_normalization(scalar_type));
        }
        let target = scalar_type.as_integer()?;
        let fixed = |integer: semantic_vocabulary::IntegerType| {
            integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                && matches!(integer.bits(), 8 | 16 | 32 | 64)
        };
        let narrower = match kind {
            LegalizedScalarInstructionKind::IntegerWiden { source_type, .. }
                if fixed(*source_type) && fixed(target) && source_type.can_widen_to(target) =>
            {
                *source_type
            }
            LegalizedScalarInstructionKind::IntegerExactCast { source_type, .. }
                if crate::legalization::exact_cast_has_native_carriers(*source_type, target) =>
            {
                if source_type.bits() < target.bits() {
                    *source_type
                } else {
                    target
                }
            }
            _ => return None,
        };
        Some(
            crate::selection::scalar_call_abi::integer_carrier_normalization(ScalarType::Integer(
                narrower,
            )),
        )
    }

    fn register_form_normalization(
        scalar_type: ScalarType,
    ) -> selected_instructions::SelectedInstructionKind {
        match scalar_type {
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
                selected_instructions::SelectedInstructionKind::ZeroExtendU32
            }
            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
                selected_instructions::SelectedInstructionKind::CopyI64
            }
            other => crate::selection::scalar_call_abi::integer_carrier_normalization(other),
        }
    }
}
pub(crate) use crate::selection_inputs::literal_compare_input;
pub(crate) mod model {
    use legalized_operations::LegalizedOperationPlanIdentity;
    use optimization_core::OptimizationValidatorIdentity;
    use register_model::RegisterConstraintKey;
    use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedSelectedInstructions {
        pub(crate) plan: std::sync::Arc<SelectedInstructionPlan>,
        pub(crate) receipt: SelectedInstructionValidationReceipt,
    }

    impl ValidatedSelectedInstructions {
        pub fn shared_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
            std::sync::Arc::clone(&self.plan)
        }

        pub fn plan(&self) -> &SelectedInstructionPlan {
            &self.plan
        }

        pub const fn receipt(&self) -> SelectedInstructionValidationReceipt {
            self.receipt
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SelectedInstructionValidationReceipt {
        pub(crate) identity: SelectedInstructionPlanIdentity,
        pub(crate) legalized: LegalizedOperationPlanIdentity,
        pub(crate) legalization_validator: OptimizationValidatorIdentity,
        pub(crate) optimization_unit: optimization_core::OptimizationUnitIdentity,
        pub(crate) work_schedule: semantic_vocabulary::WorkScheduleIdentity,
        pub(crate) function_count: usize,
        pub(crate) block_count: usize,
        pub(crate) virtual_register_count: usize,
        pub(crate) instruction_count: usize,
    }

    impl SelectedInstructionValidationReceipt {
        pub const fn identity(self) -> SelectedInstructionPlanIdentity {
            self.identity
        }

        pub const fn legalized(self) -> LegalizedOperationPlanIdentity {
            self.legalized
        }

        pub const fn legalization_validator(self) -> OptimizationValidatorIdentity {
            self.legalization_validator
        }

        pub const fn optimization_unit(self) -> optimization_core::OptimizationUnitIdentity {
            self.optimization_unit
        }

        pub const fn work_schedule(self) -> semantic_vocabulary::WorkScheduleIdentity {
            self.work_schedule
        }

        pub const fn function_count(self) -> usize {
            self.function_count
        }

        pub const fn block_count(self) -> usize {
            self.block_count
        }

        pub const fn virtual_register_count(self) -> usize {
            self.virtual_register_count
        }

        pub const fn instruction_count(self) -> usize {
            self.instruction_count
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SelectedInstructionError {
        SourceCustodyMismatch {
            machine: Option<semantic_vocabulary::MachineId>,
            operation: Option<semantic_vocabulary::OperationId>,
            site: optimization_core::CompilerCheckSite,
            reason: Option<&'static str>,
        },
        TargetRegisterArchitectureMismatch,
        UnsupportedSourceShape {
            function: usize,
            machine: Option<semantic_vocabulary::MachineId>,
            operation: Option<semantic_vocabulary::OperationId>,
            site: optimization_core::CompilerCheckSite,
        },
        AtomicFormUnavailable {
            target: target::NativeTarget,
            form: selected_instructions::AtomicForm,
            function: usize,
            machine: semantic_vocabulary::MachineId,
            operation: semantic_vocabulary::OperationId,
        },
        AmbiguousSourceShape {
            function: usize,
            first: &'static str,
            second: &'static str,
        },
        MissingConstraint(RegisterConstraintKey),
        MissingInputRegisterView {
            function: usize,
        },
        NonCanonicalVirtualRegisters {
            function: usize,
        },
        NonCanonicalBlocks {
            function: usize,
        },
        NonCanonicalInstructions {
            function: usize,
        },
        FunctionProjectionMismatch {
            function: usize,
        },
        VirtualRegisterProjectionMismatch {
            function: usize,
            register: u32,
        },
        BlockProjectionMismatch {
            function: usize,
            block: u32,
        },
        InstructionProjectionMismatch {
            function: usize,
            instruction: u32,
        },
        ConstraintOperandMismatch {
            function: usize,
            instruction: u32,
        },
        ConstraintEffectMismatch {
            function: usize,
            instruction: u32,
        },
        SuccessorProjectionMismatch {
            function: usize,
            block: u32,
        },
        UseBeforeDefinition {
            function: usize,
            instruction: u32,
            register: u32,
        },
        MultipleDefinitions {
            function: usize,
            register: u32,
        },
        ProvenancePartitionMismatch {
            function: usize,
        },
    }

    impl SelectedInstructionError {
        #[track_caller]
        pub(crate) fn unsupported_shape(function: usize) -> Self {
            Self::UnsupportedSourceShape {
                function,
                machine: None,
                operation: None,
                site: optimization_core::CompilerCheckSite::caller(),
            }
        }

        pub(crate) fn attributed(
            self,
            machine: semantic_vocabulary::MachineId,
            operation: Option<semantic_vocabulary::OperationId>,
        ) -> Self {
            match self {
                Self::UnsupportedSourceShape {
                    function,
                    machine: None,
                    operation: None,
                    site,
                } => Self::UnsupportedSourceShape {
                    function,
                    machine: Some(machine),
                    operation,
                    site,
                },
                Self::SourceCustodyMismatch {
                    machine: None,
                    operation: None,
                    site,
                    reason,
                } => Self::SourceCustodyMismatch {
                    machine: Some(machine),
                    operation,
                    site,
                    reason,
                },
                other => other,
            }
        }
    }

    failed_display!(
        SelectedInstructionError,
        "Terminal instruction selection failed"
    );

    impl std::error::Error for SelectedInstructionError {}

    impl SelectedInstructionError {
        #[track_caller]
        pub fn custody() -> Self {
            Self::SourceCustodyMismatch {
                machine: None,
                operation: None,
                site: optimization_core::CompilerCheckSite::caller(),
                reason: None,
            }
        }

        #[track_caller]
        pub fn custody_because(reason: &'static str) -> Self {
            Self::SourceCustodyMismatch {
                machine: None,
                operation: None,
                site: optimization_core::CompilerCheckSite::caller(),
                reason: Some(reason),
            }
        }
    }
}
pub(crate) use crate::selection_inputs::native_code_layout_input;
pub(crate) use crate::selection_inputs::owned_block_home_input;
pub(crate) mod parameter_use {
    use legalized_operations::{
        LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarInstructionKind,
    };
    use semantic_vocabulary::PlaceId;

    fn call_uses_referent(call: &LegalizedScalarCall, place: PlaceId) -> bool {
        call.arguments.iter().any(|argument| {
            matches!(argument, LegalizedScalarArgument::Structural { semantic, .. } if semantic.place == place)
        })
    }

    pub(crate) fn referent_used(
        source: &crate::native_geometry::NativeFunction<'_>,
        place: PlaceId,
    ) -> bool {
        super::established_view_input::transferred(source, place)
            || source.blocks.iter().any(|block| block.terminator.cleanup_calls().any(|(_, call)|
                call_uses_referent(call, place)))
            || source.blocks.iter().any(|block| {
                match &block.terminator {
                    legalized_operations::LegalizedScalarTerminator::InvokeNoNormal { call, .. } => call_uses_referent(call, place),
                    legalized_operations::LegalizedScalarTerminator::StructuralCase { source, .. } => source.place() == place,
                    legalized_operations::LegalizedScalarTerminator::Return(returned)
                    | legalized_operations::LegalizedScalarTerminator::ReturnCleanup { returned, .. } => matches!(
                        returned.value,
                        legalized_operations::LegalizedScalarReturnValue::StructuralParameter { place: returned }
                        | legalized_operations::LegalizedScalarReturnValue::SharedDescriptor { place: returned } if returned == place
                    ),
                    _ => false,
                }
            })
            || source.instructions()
                .any(|row| match &row.kind {
                    LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
                    | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. }
                    | LegalizedScalarInstructionKind::ElementViewSubslice { source, .. }
                    | LegalizedScalarInstructionKind::EstablishReference { source, .. }
                    | LegalizedScalarInstructionKind::EstablishAccessReference { source, .. } => source.place == place,
                    LegalizedScalarInstructionKind::AtomicAccess { location, .. }
                    | LegalizedScalarInstructionKind::AccessorScalarAccess { location, .. } => location.place == place,
                    LegalizedScalarInstructionKind::StaticInterrupt { token, .. } => token.place == place,
                    LegalizedScalarInstructionKind::InstallStaticInterrupt { arguments, .. } =>
                        arguments.iter().any(|argument| argument.place == place),
                    LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation { left, right, .. } => left.source() == place || right.source() == place,
                    LegalizedScalarInstructionKind::ByteSequenceEqual { left, right } => left.source() == place || right.source() == place,
                    LegalizedScalarInstructionKind::ByteSequencePredicate { operand, .. } => operand.source() == place,
                    LegalizedScalarInstructionKind::ByteSequenceStore { destination, source, .. } => *destination == place || *source == place,
                    LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
                        destination,
                        source,
                        ..
                    }
                    | LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
                        destination,
                        source,
                        ..
                    } => destination.place == place || *source == place,
                    LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                        source: argument,
                        ..
                    }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                        source: argument,
                        ..
                    }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
                        source: argument,
                        ..
                    } => argument.place == place,
                    LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
                        destination,
                        ..
                    } => destination.place == place,
                    LegalizedScalarInstructionKind::StoreStructuralField { destination, .. } => {
                        destination.place() == place
                    }
                    LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                        destination, ..
                    } => destination.place() == place,
                    LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { destination, .. } => {
                        destination.place() == place
                    }
                    LegalizedScalarInstructionKind::ActivateTaskRegister { gdt, task_state, scratch, .. } =>
                        [gdt, task_state, scratch].into_iter().any(|argument| argument.place == place),
    LegalizedScalarInstructionKind::ReloadSegmentDescriptors { gdt, scratch, .. } =>
                        [gdt, scratch].into_iter().any(|argument| argument.place == place),
                    LegalizedScalarInstructionKind::StorageAddress { source, .. }
                    | LegalizedScalarInstructionKind::GlobalDescriptorTable { source, .. }
                    | LegalizedScalarInstructionKind::PrimitiveScalarRead { source, .. }
                    | LegalizedScalarInstructionKind::StructuralCaseMembership { source, .. }
                    | LegalizedScalarInstructionKind::StructuralLeafCopy { source, .. }
                    | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. }
                    | LegalizedScalarInstructionKind::ElementViewRead { source, .. } => {
                        *source == place
                    }
                    LegalizedScalarInstructionKind::EstablishElementView { source, .. }
                    | LegalizedScalarInstructionKind::ElementViewLength { source, .. } => {
                        source.place == place
                    }
                    LegalizedScalarInstructionKind::Call(call) => call_uses_referent(call, place),
                    LegalizedScalarInstructionKind::NormalizedForeignCall(call) => call
                        .structural_arguments
                        .iter()
                        .any(|argument| argument.place == place),
                    _ => false,
                })
    }
}
pub(crate) mod place_declaration {
    use legalized_operations::LegalizedStructuralContract;
    use semantic_vocabulary::{
        BlockId, OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId,
    };
    use terminal_psi::StructuralParameterDeclaration;

    pub(crate) fn declares_operation_result(
        contract: &LegalizedStructuralContract,
        place: PlaceId,
        producer: OperationId,
        structural_type: StructuralTypeId,
    ) -> bool {
        contract
            .structural_place(place)
            .and_then(|declaration| declaration.kind.as_operation_result())
            == Some((producer, structural_type))
    }

    pub(crate) fn declares_block_parameter(
        contract: &LegalizedStructuralContract,
        place: PlaceId,
        block: BlockId,
        position: u32,
    ) -> bool {
        contract.structural_place(place).is_some_and(|declaration| {
            declaration.kind == StructuralPlaceKind::BlockParameter { block, position }
        })
    }

    pub(crate) fn arrivals<'a>(
        source: &'a crate::native_geometry::NativeFunction<'a>,
        place: PlaceId,
    ) -> impl Iterator<Item = (BlockId, &'a StructuralParameterDeclaration)> + 'a {
        source.blocks.iter().flat_map(move |block| {
            block
                .structural_parameters
                .iter()
                .filter(move |parameter| parameter.place == place)
                .map(move |parameter| (block.id, parameter))
        })
    }
}
pub(crate) use crate::selection_call_abi as scalar_call_abi;
pub(crate) use crate::selection_inputs::primitive_local_input;
pub(crate) use crate::selection_inputs::record_input;
pub(crate) use crate::selection_inputs::reference_input;
pub(crate) use crate::selection_inputs::scalar_array_input;
pub(crate) use crate::selection_inputs::storage_origin_input;
pub(crate) use crate::selection_inputs::structural_array_input;
pub(crate) use crate::selection_inputs::structural_local_input;
pub(crate) use scalar_call_abi::scalar_shape;
pub(crate) mod value_transport {
    use std::collections::{BTreeMap, BTreeSet};

    use legalized_operations::{
        LegalizedScalarReturnValue, LegalizedScalarTerminator as Terminator,
    };
    use semantic_vocabulary::ValueId;

    pub(crate) fn required_values(
        function: &legalized_operations::LegalizedScalarFunction,
    ) -> BTreeSet<ValueId> {
        let mut pending = Vec::new();
        let mut incoming = BTreeMap::<ValueId, Vec<ValueId>>::new();
        for block in &function.blocks {
            for instruction in &block.instructions {
                instruction.visit_scalar_operands(|value| pending.push(value));
            }
            let mut bind =
                |successor: &legalized_operations::LegalizedScalarSuccessor,
                 pending: &mut Vec<ValueId>| {
                    pending.extend(successor.structural_bindings.iter().flat_map(|binding| {
                        binding.indices.iter().map(|index| index.operand.value)
                    }));
                    for binding in &successor.bindings {
                        incoming
                            .entry(binding.parameter)
                            .or_default()
                            .push(binding.argument);
                    }
                };
            match &block.terminator {
                Terminator::InvokeNoNormal { call, .. } => {
                    pending.extend(
                        call.arguments
                            .iter()
                            .filter_map(|argument| argument.scalar_source()),
                    );
                    pending.extend(call.arguments.iter().flat_map(|argument| {
                        match argument {
                            legalized_operations::LegalizedScalarArgument::Structural {
                                indices,
                                ..
                            } => indices
                                .iter()
                                .map(|index| index.operand.value)
                                .collect::<Vec<_>>(),
                            _ => Vec::new(),
                        }
                    }));
                }
                Terminator::ExternalCompletion { arguments, .. } => {
                    pending.extend(
                        arguments
                            .iter()
                            .map(|argument| argument.source.source_value()),
                    );
                }
                Terminator::Return(returned) | Terminator::ReturnCleanup { returned, .. } => {
                    if let LegalizedScalarReturnValue::Value { value, .. } = returned.value {
                        pending.push(value);
                    }
                }
                Terminator::Jump { successor, .. } => bind(successor, &mut pending),
                Terminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => {
                    pending.push(*condition);
                    bind(when_true, &mut pending);
                    bind(when_false, &mut pending);
                }
                Terminator::StructuralCase { cases, .. } => {
                    pending.extend(
                        cases
                            .iter()
                            .flat_map(|case| &case.structural_bindings)
                            .flat_map(|binding| {
                                binding.indices.iter().map(|index| index.operand.value)
                            }),
                    );
                }
                Terminator::Crash { .. } => {}
            }
        }
        let mut required = BTreeSet::new();
        while let Some(value) = pending.pop() {
            if required.insert(value)
                && let Some(arguments) = incoming.get(&value)
            {
                pending.extend(arguments);
            }
        }
        required
    }
}
pub(crate) use crate::selection_inputs::zeroed_structural_input;

pub(crate) use model::SelectedInstructionError;
pub use model::ValidatedSelectedInstructions;
pub use selected_instructions::selected_instruction_plan_identity;

#[allow(unused_imports)]
pub(crate) use selected_instructions::SelectedSelectionConstraints;

#[allow(unused_imports)]
pub(crate) use crate::legalization::ValidatedLegalizedOperations;
#[allow(unused_imports)]
pub(crate) use construction::build_plan;

pub(crate) fn select_instructions(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let plan = build_plan(legalized, constraints, environment)?;
    let receipt = identity::receipt(&plan, legalized);
    Ok(ValidatedSelectedInstructions {
        plan: plan.into(),
        receipt,
    })
}
