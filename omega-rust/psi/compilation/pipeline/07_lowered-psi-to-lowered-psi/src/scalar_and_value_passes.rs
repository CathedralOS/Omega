/*
Target-neutral scalar and value optimization passes: sparse conditional
constant propagation, copy propagation, global value numbering, and dead pure
scalar elimination.
*/

pub(crate) mod sparse_conditional_constant_propagation {
    pub(crate) mod folding {
        use semantic_vocabulary::ValueId;
        use std::collections::BTreeMap;
        use terminal_psi::{OperationKind, TerminalMachine};
        use terminal_semantics::{ScalarLeafLiteral, constant_goal_free_scalar_leaf};

        pub(crate) fn fold(
            machine: &mut TerminalMachine,
            demands: &[terminal_psi::TerminalBoundaryApplicationDemand],
        ) {
            if machine.ranked_scc.is_some() {
                return;
            }
            let value_types = super::machine_scalar_value_types(machine);
            let mut literals: BTreeMap<ValueId, ScalarLeafLiteral> = machine
                .operations()
                .filter_map(|operation| {
                    let result = operation.result.scalar()?;
                    let literal = match &operation.kind {
                        OperationKind::IntegerConstant { value } => {
                            ScalarLeafLiteral::Integer(*value)
                        }
                        OperationKind::BooleanConstant { value } => {
                            ScalarLeafLiteral::Boolean(*value)
                        }
                        _ => return None,
                    };
                    Some((result.id, literal))
                })
                .collect();
            loop {
                let mut changed = false;
                for block in &mut machine.blocks {
                    for operation in &mut block.operations {
                        let Some(result) = operation.result.scalar() else {
                            continue;
                        };
                        if literals.contains_key(&result.id) {
                            continue;
                        }
                        if operation.static_reach_binding.is_some()
                            || crate::retained_identities::has_boundary_application(
                                demands,
                                operation.id,
                            )
                        {
                            continue;
                        }
                        let Some(literal) =
                            constant_goal_free_scalar_leaf(operation, &literals, &value_types)
                        else {
                            continue;
                        };
                        operation.kind = match literal {
                            ScalarLeafLiteral::Integer(value) => {
                                OperationKind::IntegerConstant { value }
                            }
                            ScalarLeafLiteral::Boolean(value) => {
                                OperationKind::BooleanConstant { value }
                            }
                        };
                        literals.insert(result.id, literal);
                        changed = true;
                    }
                }
                if !changed {
                    return;
                }
            }
        }
    }

    use crate::PsiOptimizationStageError;
    use lowered_psi::LoweredPsi;

    fn machine_scalar_value_types(
        machine: &terminal_psi::TerminalMachine,
    ) -> std::collections::BTreeMap<semantic_vocabulary::ValueId, semantic_vocabulary::ScalarType>
    {
        let mut value_types = std::collections::BTreeMap::new();
        for parameter in &machine.parameters {
            value_types.insert(parameter.id, parameter.scalar_type);
        }
        if let Some(result) = machine.result.scalar() {
            value_types.insert(result.id, result.scalar_type);
        }
        for block in &machine.blocks {
            for parameter in &block.parameters {
                value_types.insert(parameter.id, parameter.scalar_type);
            }
            for operation in &block.operations {
                if let Some(result) = operation.result.scalar() {
                    value_types.insert(result.id, result.scalar_type);
                }
            }
        }
        value_types
    }

    pub(crate) fn propagate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
        let mut after = before.clone();
        for machine in &mut after.semantic_module.machines {
            folding::fold(
                machine,
                &before.semantic_module.boundary_application_demands,
            );
        }
        if let Some(debug) = after.debug_map.as_mut() {
            debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
                .map_err(PsiOptimizationStageError::InvalidSemantic)?;
        }
        Ok(after)
    }
}

pub(crate) mod copy_propagation {
    pub(crate) mod copies {
        use crate::edge_arguments::drop_removed_positions_across_edges;
        use crate::retained_identities::proof_values::{
            call_erased_arguments, crash_continuations, retain_contract_values,
            retain_crash_routes, retain_erased_arguments, retain_proposition,
        };
        use semantic_vocabulary::{BlockId, ValueId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{TerminalMachine, Terminator, ValueDeclaration};

        pub(crate) fn propagate(
            machine: &mut TerminalMachine,
            source_calls: &[lowered_psi::LoweredSourceCallOccurrence],
            retained_values: &mut BTreeSet<ValueId>,
        ) {
            let Some(coverage) =
                crate::retained_identities::ranked_coverage::ranked_coverage(machine)
            else {
                return;
            };
            retained_values.extend(coverage.values.iter().copied());
            retain_contract_values(&machine.contract, retained_values);
            let operations = machine.operations().collect::<Vec<_>>();
            for operation in &operations {
                retain_crash_routes(crash_continuations(&operation.kind), retained_values);
                retain_erased_arguments(call_erased_arguments(&operation.kind), retained_values);
            }
            for occurrence in source_calls {
                if crate::retained_identities::contains_source_call(machine, occurrence) {
                    occurrence.frontier_before_call.visit_value_ids(|value| {
                        retained_values.insert(value);
                    });
                }
            }
            let mut incoming_arguments: BTreeMap<BlockId, Vec<Vec<ValueId>>> = BTreeMap::new();
            let mut structural_case_targets = BTreeSet::new();
            for block in &machine.blocks {
                match &block.terminator {
                    Terminator::Jump {
                        target,
                        arguments,
                        erased_arguments,
                        ..
                    } => {
                        incoming_arguments
                            .entry(*target)
                            .or_default()
                            .push(arguments.clone());
                        retain_erased_arguments(erased_arguments, retained_values);
                    }
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for edge in [when_true, when_false] {
                            incoming_arguments
                                .entry(edge.target)
                                .or_default()
                                .push(edge.arguments.clone());
                            retain_erased_arguments(&edge.erased_arguments, retained_values);
                        }
                    }
                    Terminator::StructuralCase { cases, .. } => {
                        structural_case_targets.extend(cases.iter().map(|case| case.target));
                    }
                    Terminator::Crash { site_guard, .. } => {
                        for term in site_guard {
                            retain_proposition(term.proposition(), retained_values);
                        }
                    }
                    Terminator::InvokeNoNormal { .. }
                    | terminal_psi::terminator_return_pair_pattern!(..)
                    | Terminator::ReturnStructural { .. }
                    | Terminator::ExternalCompletion { .. } => {}
                }
            }
            let mut raw: BTreeMap<ValueId, Vec<ValueId>> = BTreeMap::new();
            let mut covered_parameters: BTreeSet<ValueId> = BTreeSet::new();
            let mut declarations: BTreeMap<ValueId, ValueDeclaration> = BTreeMap::new();
            for parameter in &machine.parameters {
                declarations.insert(parameter.id, *parameter);
            }
            if let Some(result) = machine.result.scalar() {
                declarations.insert(result.id, result);
            }
            for block in &machine.blocks {
                for parameter in &block.parameters {
                    declarations.insert(parameter.id, *parameter);
                }
                for operation in &block.operations {
                    if let Some(result) = operation.result.scalar() {
                        declarations.insert(result.id, result);
                    }
                }
                if structural_case_targets.contains(&block.id) {
                    continue;
                }
                if let Some(edges) = incoming_arguments.get(&block.id) {
                    for (position, parameter) in block.parameters.iter().enumerate() {
                        raw.insert(
                            parameter.id,
                            edges.iter().map(|arguments| arguments[position]).collect(),
                        );
                        if coverage.blocks.contains(&block.id) {
                            covered_parameters.insert(parameter.id);
                        }
                    }
                }
            }
            let mut memo: BTreeMap<ValueId, ValueId> = BTreeMap::new();
            let mut collapse: BTreeMap<ValueId, ValueId> = BTreeMap::new();
            for parameter in raw.keys() {
                if covered_parameters.contains(parameter)
                    || retained_values.contains(parameter)
                    || coverage.member_uses.contains(parameter)
                {
                    continue;
                }
                let source = resolve(*parameter, &raw, &mut memo);
                let declaration = &declarations[parameter];
                if source != *parameter
                    && declarations.get(&source).is_some_and(|source_declaration| {
                        source_declaration.scalar_type == declaration.scalar_type
                            && source_declaration.qualifications == declaration.qualifications
                    })
                {
                    collapse.insert(*parameter, source);
                }
            }
            if collapse.is_empty() {
                return;
            }
            let mut removed_positions: BTreeMap<BlockId, Vec<usize>> = BTreeMap::new();
            for block in &mut machine.blocks {
                let removed = block
                    .parameters
                    .iter()
                    .enumerate()
                    .filter_map(|(position, parameter)| {
                        collapse.contains_key(&parameter.id).then_some(position)
                    })
                    .collect::<Vec<_>>();
                if !removed.is_empty() {
                    removed_positions.insert(block.id, removed);
                }
                block
                    .parameters
                    .retain(|parameter| !collapse.contains_key(&parameter.id));
                for operation in &mut block.operations {
                    operation.kind.map_scalar_uses(&mut |value| {
                        collapse.get(&value).copied().unwrap_or(value)
                    });
                }
                block
                    .terminator
                    .map_scalar_uses(&mut |value| collapse.get(&value).copied().unwrap_or(value));
            }
            drop_removed_positions_across_edges(machine, &removed_positions);
        }

        fn resolve(
            value: ValueId,
            raw: &BTreeMap<ValueId, Vec<ValueId>>,
            memo: &mut BTreeMap<ValueId, ValueId>,
        ) -> ValueId {
            resolve_in(value, raw, memo, &mut BTreeSet::new())
        }

        fn resolve_in(
            value: ValueId,
            raw: &BTreeMap<ValueId, Vec<ValueId>>,
            memo: &mut BTreeMap<ValueId, ValueId>,
            visiting: &mut BTreeSet<ValueId>,
        ) -> ValueId {
            let Some(arguments) = raw.get(&value) else {
                return value;
            };
            if let Some(resolved) = memo.get(&value) {
                return *resolved;
            }
            if !visiting.insert(value) {
                return value;
            }
            let mut resolved = arguments
                .iter()
                .map(|argument| resolve_in(*argument, raw, memo, visiting));
            let first = resolved
                .next()
                .expect("an inventoried edge argument list is never empty");
            let result = if resolved.all(|other| other == first) {
                first
            } else {
                value
            };
            visiting.remove(&value);
            memo.insert(value, result);
            result
        }
    }

    use crate::PsiOptimizationStageError;
    use lowered_psi::LoweredPsi;

    pub(crate) fn propagate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
        let mut after = before.clone();
        let mut retained_values =
            crate::retained_identities::record_named_rows(&after.semantic_module).values;
        for invariant in &after.semantic_module.scalar_block_invariants {
            invariant.predicate.visit_value_ids(|value| {
                retained_values.insert(value);
            });
        }
        // A qualification row names its jump argument by position, so the
        // receiving block keeps every parameter: collapsing an earlier one
        // would shift that position. When those blocks cannot be identified
        // the pass leaves the module unchanged.
        let module = &after.semantic_module;
        let Some(qualified) =
            terminal_semantics::scalar_demand::qualification_parameter_blocks(module)
        else {
            return Ok(before);
        };
        for machine in &module.machines {
            let first = qualified.partition_point(|(owner, _)| *owner < machine.id);
            let last = qualified.partition_point(|(owner, _)| *owner <= machine.id);
            for (_, block) in &qualified[first..last] {
                if let Some(block) = machine.block(block) {
                    retained_values.extend(block.parameters.iter().map(|parameter| parameter.id));
                }
            }
        }
        // A return row also binds its source by identity. That source is often
        // itself a parameter fed by one same-typed argument, which this pass
        // would otherwise replace in the jump while the row kept the old name.
        for returned in &module.scalar_qualifications.returns {
            retained_values.extend([returned.source, returned.result]);
        }
        for machine in &mut after.semantic_module.machines {
            copies::propagate(
                machine,
                &before.source_call_occurrences,
                &mut retained_values,
            );
        }
        crate::retained_identities::prune_debug_map(&before, &mut after)?;
        Ok(after)
    }
}

pub(crate) mod global_value_numbering {
    pub(crate) mod equivalents {
        use crate::retained_identities::proof_values::{
            call_erased_arguments, crash_continuations, retain_contract_values,
            retain_crash_routes, retain_erased_arguments, retain_proposition,
        };
        use semantic_vocabulary::{BlockId, ValueId};
        use std::collections::{BTreeMap, BTreeSet};
        use terminal_psi::{OperationKind, TerminalMachine, Terminator, ValueDeclaration};

        struct Leader {
            kind: OperationKind,
            block: BlockId,
            result: ValueDeclaration,
        }

        pub(crate) fn deduplicate(
            machine: &mut TerminalMachine,
            source_calls: &[lowered_psi::LoweredSourceCallOccurrence],
            retained_values: &BTreeSet<ValueId>,
        ) {
            let Some(coverage) =
                crate::retained_identities::ranked_coverage::ranked_coverage(machine)
            else {
                return;
            };
            let mut retained_values = retained_values.clone();
            retained_values.extend(coverage.values.iter().copied());
            retain_contract_values(&machine.contract, &mut retained_values);
            let operations = machine.operations().collect::<Vec<_>>();
            for operation in &operations {
                retain_crash_routes(crash_continuations(&operation.kind), &mut retained_values);
                retain_erased_arguments(
                    call_erased_arguments(&operation.kind),
                    &mut retained_values,
                );
            }
            for block in &machine.blocks {
                if let Terminator::Crash { site_guard, .. } = &block.terminator {
                    for term in site_guard {
                        retain_proposition(term.proposition(), &mut retained_values);
                    }
                }
            }
            for occurrence in source_calls {
                if crate::retained_identities::contains_source_call(machine, occurrence) {
                    occurrence.frontier_before_call.visit_value_ids(|value| {
                        retained_values.insert(value);
                    });
                }
            }
            let order = reverse_postorder(machine);
            let dominators = dominators(machine);
            let mut representative: BTreeMap<ValueId, ValueId> = BTreeMap::new();
            let mut removed = BTreeSet::new();
            let mut leaders: Vec<Leader> = Vec::new();
            for block_id in order {
                let Some(block) = machine.block(&block_id) else {
                    continue;
                };
                for operation in &block.operations {
                    let Some(result) = operation.result.scalar() else {
                        continue;
                    };
                    if operation.static_reach_binding.is_some()
                        || !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
                    {
                        continue;
                    }
                    let mut kind = operation.kind.clone();
                    kind.map_scalar_uses(&mut |value| {
                        representative.get(&value).copied().unwrap_or(value)
                    });
                    if !retained_values.contains(&result.id)
                        && !coverage.blocks.contains(&block_id)
                        && !coverage.member_uses.contains(&result.id)
                        && let Some(leader) = leaders.iter().find(|leader| {
                            leader.kind == kind
                                && dominators
                                    .get(&block_id)
                                    .is_some_and(|blocks| blocks.contains(&leader.block))
                                && leader.result.scalar_type == result.scalar_type
                                && leader.result.qualifications == result.qualifications
                        })
                    {
                        representative.insert(result.id, leader.result.id);
                        removed.insert(operation.id);
                        continue;
                    }
                    leaders.push(Leader {
                        kind,
                        block: block_id,
                        result,
                    });
                }
            }
            if removed.is_empty() {
                return;
            }
            for block in &mut machine.blocks {
                block
                    .operations
                    .retain(|operation| !removed.contains(&operation.id));
                for operation in &mut block.operations {
                    operation.kind.map_scalar_uses(&mut |value| {
                        representative.get(&value).copied().unwrap_or(value)
                    });
                }
                block.terminator.map_scalar_uses(&mut |value| {
                    representative.get(&value).copied().unwrap_or(value)
                });
            }
        }

        fn reverse_postorder(machine: &TerminalMachine) -> Vec<BlockId> {
            let successors = machine
                .blocks
                .iter()
                .map(|block| {
                    let targets = block.terminator.successor_targets();
                    (block.id, targets)
                })
                .collect::<BTreeMap<_, _>>();
            let mut finished = terminal_semantics::control_graph::finished_order(
                machine.entry,
                |block, position| {
                    successors
                        .get(&block)
                        .and_then(|targets| targets.get(position))
                        .copied()
                },
            );
            finished.reverse();
            finished
        }

        fn dominators(machine: &TerminalMachine) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
            let blocks = machine
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<BTreeSet<_>>();
            let mut predecessors = blocks
                .iter()
                .map(|block| (*block, BTreeSet::new()))
                .collect::<BTreeMap<_, _>>();
            for block in &machine.blocks {
                let targets = block.terminator.successor_targets();
                for target in targets {
                    if let Some(incoming) = predecessors.get_mut(&target) {
                        incoming.insert(block.id);
                    }
                }
            }
            let mut dominators = blocks
                .iter()
                .map(|block| {
                    (
                        *block,
                        if *block == machine.entry {
                            BTreeSet::from([*block])
                        } else {
                            blocks.clone()
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            loop {
                let mut changed = false;
                for block in blocks
                    .iter()
                    .copied()
                    .filter(|block| *block != machine.entry)
                {
                    let Some(incoming) = predecessors.get(&block) else {
                        continue;
                    };
                    let Some(first) = incoming.iter().next() else {
                        continue;
                    };
                    let mut common = dominators
                        .get(first)
                        .cloned()
                        .unwrap_or_else(|| BTreeSet::from([*first]));
                    for predecessor in incoming.iter().skip(1) {
                        let predecessor_dominators = dominators
                            .get(predecessor)
                            .cloned()
                            .unwrap_or_else(|| BTreeSet::from([*predecessor]));
                        common.retain(|candidate| predecessor_dominators.contains(candidate));
                    }
                    common.insert(block);
                    if dominators.get(&block) != Some(&common) {
                        dominators.insert(block, common);
                        changed = true;
                    }
                }
                if !changed {
                    return dominators;
                }
            }
        }
    }

    use crate::PsiOptimizationStageError;
    use crate::retained_identities::{boundary_application_results, record_named_rows};
    use lowered_psi::LoweredPsi;

    pub(crate) fn number(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
        let mut after = before.clone();
        let mut retained_values = record_named_rows(&before.semantic_module).values;
        retained_values.extend(boundary_application_results(&before.semantic_module));
        for machine in &mut after.semantic_module.machines {
            equivalents::deduplicate(machine, &before.source_call_occurrences, &retained_values);
        }
        crate::retained_identities::prune_debug_map(&before, &mut after)?;
        Ok(after)
    }
}

pub(crate) mod dead_scalar_elimination {
    pub(crate) mod liveness {
        use semantic_vocabulary::ValueId;
        use terminal_psi::TerminalMachine;
        use terminal_semantics::scalar_demand::{ScalarDemandMode, complete_scalar_demand};

        pub(crate) fn eliminate(
            machine: &mut TerminalMachine,
            retained_values: &[ValueId],
            protected_blocks: impl IntoIterator<Item = semantic_vocabulary::BlockId>,
        ) {
            complete_scalar_demand(
                machine,
                retained_values.iter().copied(),
                protected_blocks,
                ScalarDemandMode::DropUnusedTotalOperations,
            );
        }
    }

    use crate::PsiOptimizationStageError;
    use crate::retained_identities::{boundary_application_results, record_named_rows};
    use lowered_psi::LoweredPsi;

    pub(crate) fn eliminate(before: LoweredPsi) -> Result<LoweredPsi, PsiOptimizationStageError> {
        let Some(protected) = terminal_semantics::scalar_demand::qualification_parameter_blocks(
            &before.semantic_module,
        ) else {
            return Ok(before);
        };
        let mut after = before.clone();
        let mut retained_values = record_named_rows(&before.semantic_module).values;
        retained_values.extend(boundary_application_results(&before.semantic_module));
        for invariant in &before.semantic_module.scalar_block_invariants {
            invariant.predicate.visit_value_ids(|value| {
                retained_values.insert(value);
            });
        }
        for occurrence in &before.source_call_occurrences {
            occurrence.frontier_before_call.visit_value_ids(|value| {
                retained_values.insert(value);
            });
        }
        let Some(retained) = terminal_semantics::scalar_demand::retained_values_by_machine(
            &before.semantic_module,
            retained_values,
        ) else {
            return Ok(before);
        };
        for machine in &mut after.semantic_module.machines {
            let first = protected.partition_point(|(owner, _)| *owner < machine.id);
            let last = protected.partition_point(|(owner, _)| *owner <= machine.id);
            let values = retained
                .binary_search_by_key(&machine.id, |(owner, _)| *owner)
                .ok()
                .map_or(&[][..], |position| retained[position].1.as_slice());
            liveness::eliminate(
                machine,
                values,
                protected[first..last].iter().map(|(_, block)| *block),
            );
        }
        crate::retained_identities::prune_debug_map(&before, &mut after)?;
        Ok(after)
    }
}
