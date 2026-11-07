//! Keep only the scalar values a completed graph actually needs to transport.
//!
//! Removing a private parameter also removes its matching incoming edge slot;
//! it never suppresses an evaluated read. Producers keep every operation, while
//! the optional optimizer may separately remove unused total scalar operations.
//! Both consume this same backward demand, including every incoming alternative.

mod operation_inputs;
mod ranked_coordinates;
mod retained_rows;

pub use retained_rows::{record_named_rows, visit_invocation_evidence_values};
use semantic_vocabulary::{BlockId, ValueId};
use terminal_psi::{TerminalMachine, Terminator};

#[derive(Clone, Copy)]
pub enum ScalarDemandMode {
    KeepAllOperations,
    DropUnusedTotalOperations,
}

#[derive(Clone, Copy)]
enum Definition {
    MachineParameter,
    BlockParameter(usize, usize),
    Operation(usize, BlockId),
}

/// Unchanged semantic IDs, sorted only to index this immutable input. No ID is
/// replaced by a dense public identity, and high singleton IDs allocate one row.
struct ScalarDemand {
    live: Vec<ValueId>,
    removed: Vec<(BlockId, Vec<usize>)>,
}

pub fn ranked_coordinates(machine: &TerminalMachine) -> Option<(Vec<BlockId>, Vec<ValueId>)> {
    let coordinates = ranked_coordinates::ranked_coordinates(machine)?;
    Some((coordinates.blocks, coordinates.values))
}

/// Qualification evidence also names an edge argument position. Preserve its
/// complete target telescope instead of silently changing that coordinate.
pub fn qualification_parameter_blocks(
    module: &terminal_psi::TerminalModule,
) -> Option<Vec<(semantic_vocabulary::MachineId, BlockId)>> {
    if module.scalar_qualifications.coercions.is_empty() {
        return Some(Vec::new());
    }
    let mut targets = Vec::new();
    for machine in &module.machines {
        for block in &machine.blocks {
            match &block.terminator {
                Terminator::Jump { edge, target, .. } => {
                    targets.push(((machine.id, *edge), *target))
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        targets.push(((machine.id, edge.edge), edge.target));
                    }
                }
                _ => {}
            }
        }
    }
    targets.sort_unstable_by_key(|(key, _)| *key);
    if targets.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return None;
    }
    let mut protected = Vec::new();
    for row in &module.scalar_qualifications.coercions {
        let position = targets
            .binary_search_by_key(&(row.machine, row.edge), |(key, _)| *key)
            .ok()?;
        protected.push((row.machine, targets[position].1));
    }
    protected.sort_unstable();
    protected.dedup();
    Some(protected)
}

/// Partition the one module-owned retention roster once. Per-body demand then
/// reads only its own values instead of scanning every other body's metadata.
pub fn retained_values_by_machine(
    module: &terminal_psi::TerminalModule,
    values: impl IntoIterator<Item = ValueId>,
) -> Option<Vec<(semantic_vocabulary::MachineId, Vec<ValueId>)>> {
    let mut namespace = Vec::new();
    for machine in &module.machines {
        namespace.extend(
            machine
                .parameters
                .iter()
                .map(|value| (value.id, machine.id)),
        );
        for block in &machine.blocks {
            namespace.extend(block.parameters.iter().map(|value| (value.id, machine.id)));
            namespace.extend(block.operations.iter().filter_map(|operation| {
                operation
                    .result
                    .scalar()
                    .map(|value| (value.id, machine.id))
            }));
        }
    }
    namespace.sort_unstable_by_key(|(value, _)| *value);
    if namespace.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return None;
    }
    let mut roots = values
        .into_iter()
        .filter_map(|value| {
            namespace
                .binary_search_by_key(&value, |(value, _)| *value)
                .ok()
                .map(|position| (namespace[position].1, value))
        })
        .collect::<Vec<_>>();
    roots.sort_unstable();
    roots.dedup();
    let mut result: Vec<(semantic_vocabulary::MachineId, Vec<ValueId>)> = Vec::new();
    for (machine, value) in roots {
        if let Some((previous, values)) = result.last_mut()
            && *previous == machine
        {
            values.push(value);
        } else {
            result.push((machine, vec![value]));
        }
    }
    Some(result)
}

/// Prepare and apply against the same exclusively borrowed body. A plan cannot
/// be reused with another graph or after a caller changes its operations.
/// False leaves the original graph untouched; it is not an admission verdict.
pub fn complete_scalar_demand(
    machine: &mut TerminalMachine,
    retained_values: impl IntoIterator<Item = ValueId>,
    protected_blocks: impl IntoIterator<Item = BlockId>,
    mode: ScalarDemandMode,
) -> bool {
    let Some(demand) = ScalarDemand::prepare(machine, retained_values, protected_blocks, mode)
    else {
        return false;
    };
    if matches!(mode, ScalarDemandMode::DropUnusedTotalOperations) {
        for block in &mut machine.blocks {
            block.operations.retain(|operation| {
                !crate::is_unconditionally_total_scalar(&operation.kind)
                    || operation
                        .result
                        .scalar()
                        .is_none_or(|result| demand.is_live(result.id))
            });
        }
    }
    demand.remove_unused_block_parameters(machine);
    true
}

impl ScalarDemand {
    /// Unknown indirect uses or inconsistent input coordinates retain the
    /// original graph. Partial demand is never sufficient to remove anything.
    fn prepare<'a>(
        machine: &'a TerminalMachine,
        retained_values: impl IntoIterator<Item = ValueId>,
        protected_blocks: impl IntoIterator<Item = BlockId>,
        mode: ScalarDemandMode,
    ) -> Option<Self> {
        let operations = machine.operations().collect::<Vec<_>>();
        let mut blocks = machine
            .blocks
            .iter()
            .enumerate()
            .map(|(position, block)| (block.id, position))
            .collect::<Vec<_>>();
        blocks.sort_unstable_by_key(|(id, _)| *id);
        if blocks.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return None;
        }
        let block_position = |id| {
            blocks
                .binary_search_by_key(&id, |(id, _)| *id)
                .ok()
                .map(|position| blocks[position].1)
        };
        // The existing dominator owner requires validated targets and a
        // reachable graph. Withhold rewriting rather than guessing a scope
        // for an unindexed or unreachable block.
        let mut successors = Vec::with_capacity(machine.blocks.len());
        for block in &machine.blocks {
            successors.push(
                block
                    .terminator
                    .successor_targets()
                    .into_iter()
                    .map(&block_position)
                    .collect::<Option<Vec<_>>>()?,
            );
        }
        let mut visited = vec![false; machine.blocks.len()];
        let mut pending_blocks = vec![block_position(machine.entry)?];
        while let Some(block) = pending_blocks.pop() {
            if std::mem::replace(&mut visited[block], true) {
                continue;
            }
            pending_blocks.extend(&successors[block]);
        }
        if visited.iter().any(|visited| !visited) {
            return None;
        }
        let dominators = crate::control_graph::dominators(machine);
        let operation_blocks = machine
            .blocks
            .iter()
            .flat_map(|block| std::iter::repeat_n(block.id, block.operations.len()))
            .collect::<Vec<_>>();
        let mut definitions = machine
            .parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.id,
                    Definition::MachineParameter,
                    parameter.scalar_type,
                )
            })
            .collect::<Vec<_>>();
        for (block, body) in machine.blocks.iter().enumerate() {
            definitions.extend(
                body.parameters
                    .iter()
                    .enumerate()
                    .map(|(position, parameter)| {
                        (
                            parameter.id,
                            Definition::BlockParameter(block, position),
                            parameter.scalar_type,
                        )
                    }),
            );
        }
        definitions.extend(
            operations
                .iter()
                .enumerate()
                .filter_map(|(position, operation)| {
                    operation.result.scalar().map(|value| {
                        (
                            value.id,
                            Definition::Operation(position, operation_blocks[position]),
                            value.scalar_type,
                        )
                    })
                }),
        );
        definitions.sort_unstable_by_key(|(id, _, _)| *id);
        if definitions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return None;
        }

        let (ranked_blocks, rank_values) = ranked_coordinates(machine)?;
        let mut protected = vec![false; machine.blocks.len()];
        for block in protected_blocks.into_iter().chain(ranked_blocks) {
            protected[block_position(block)?] = true;
        }
        let mut incoming = vec![Vec::<&[ValueId]>::new(); machine.blocks.len()];
        let mut pending = retained_values
            .into_iter()
            .map(|value| (value, false))
            .collect::<Vec<_>>();
        pending.extend(rank_values.into_iter().map(|value| (value, true)));
        let mut inputs = Vec::new();
        for operation in &operations {
            if (matches!(mode, ScalarDemandMode::KeepAllOperations)
                || !crate::is_unconditionally_total_scalar(&operation.kind))
                && !operation_inputs::inputs(&operation.kind, &mut inputs)
            {
                return None;
            }
        }
        pending.extend(inputs.drain(..).map(|value| (value, true)));
        for block in &machine.blocks {
            let mut edge = |target, arguments: &'a [ValueId]| -> Option<()> {
                let position = block_position(target)?;
                if arguments.len() != machine.blocks[position].parameters.len() {
                    return None;
                }
                // Even an unused slot is an original evaluated input. Check
                // every source identity and carrier before dropping any slot.
                for (actual, formal) in arguments.iter().zip(&machine.blocks[position].parameters) {
                    let source = definitions
                        .binary_search_by_key(actual, |(id, _, _)| *id)
                        .ok()?;
                    if definitions[source].2 != formal.scalar_type {
                        return None;
                    }
                    let available = match definitions[source].1 {
                        Definition::MachineParameter => true,
                        Definition::BlockParameter(owner, _) => {
                            dominators.dominates(machine.blocks[owner].id, block.id)
                        }
                        Definition::Operation(_, owner) => dominators.dominates(owner, block.id),
                    };
                    if !available {
                        return None;
                    }
                }
                incoming[position].push(arguments);
                Some(())
            };
            match &block.terminator {
                Terminator::Jump {
                    target,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    ..
                } => {
                    edge(*target, arguments)?;
                    retained_rows::visit_erased_value_arguments(
                        erased_arguments,
                        erased_proof_arguments,
                        |value| pending.push((value, true)),
                    );
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for incoming in [when_true, when_false] {
                        edge(incoming.target, &incoming.arguments)?;
                        retained_rows::visit_erased_value_arguments(
                            &incoming.erased_arguments,
                            &incoming.erased_proof_arguments,
                            |value| pending.push((value, true)),
                        );
                    }
                }
                Terminator::StructuralCase { cases, .. } => {
                    for case in cases {
                        protected[block_position(case.target)?] = true;
                    }
                }
                // The optimizer's existing conservative refusal is shared.
                // A partial guard inventory cannot authorize a rewrite.
                Terminator::Crash { .. } => return None,
                Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnStructural { .. }
                | Terminator::ExternalCompletion { .. } => {}
                Terminator::InvokeNoNormal {
                    erased_arguments,
                    erased_proof_arguments,
                    ..
                } => {
                    retained_rows::visit_erased_value_arguments(
                        erased_arguments,
                        erased_proof_arguments,
                        |value| pending.push((value, true)),
                    );
                }
            }
            // Runtime scalar use mapping excludes proof lanes and structural
            // selectors. The canonical rosters above/below retain those too.
            // Only ordinary transported runtime slots remain demand links.
            let mut terminator = block.terminator.clone();
            match &mut terminator {
                Terminator::Jump { arguments, .. } => arguments.clear(),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    when_true.arguments.clear();
                    when_false.arguments.clear();
                }
                _ => {}
            }
            terminator.map_scalar_uses(&mut |value| {
                pending.push((value, true));
                value
            });
            let mut path = |path: &[terminal_psi::StructuralPathSegment]| {
                for segment in path {
                    if let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } = segment
                    {
                        pending.push((*index, true));
                    }
                }
            };
            for projection in block.terminator.storage_uses() {
                path(projection.path);
            }
            // StructuralCase storage_uses covers the subject and cleanup;
            // its independent successor roster owns outgoing argument paths.
            for arguments in block.terminator.successor_block_arguments() {
                for argument in arguments {
                    path(&argument.path);
                }
            }
        }
        for (position, block) in machine.blocks.iter().enumerate() {
            if protected[position] || incoming[position].is_empty() {
                pending.extend(
                    block
                        .parameters
                        .iter()
                        .map(|parameter| (parameter.id, true)),
                );
            }
        }
        let mut live = vec![false; definitions.len()];
        while let Some((value, required)) = pending.pop() {
            let Ok(position) = definitions.binary_search_by_key(&value, |(id, _, _)| *id) else {
                if required {
                    return None;
                }
                // Module-owned retained rows can name another machine. Those
                // roots cannot rename or remove any definition in this body.
                continue;
            };
            if std::mem::replace(&mut live[position], true) {
                continue;
            }
            match definitions[position].1 {
                Definition::MachineParameter => {}
                Definition::Operation(operation, _) => {
                    if !operation_inputs::inputs(&operations[operation].kind, &mut inputs) {
                        return None;
                    }
                    pending.extend(inputs.drain(..).map(|value| (value, true)));
                }
                Definition::BlockParameter(block, position) => {
                    for arguments in &incoming[block] {
                        pending.push((arguments[position], true));
                    }
                }
            }
        }
        let live = definitions
            .iter()
            .zip(live)
            .filter_map(|((id, _, _), live)| live.then_some(*id))
            .collect::<Vec<_>>();
        let mut removed = Vec::new();
        for (position, block) in machine.blocks.iter().enumerate() {
            if protected[position] || incoming[position].is_empty() {
                continue;
            }
            let positions = block
                .parameters
                .iter()
                .enumerate()
                .filter_map(|(position, parameter)| {
                    live.binary_search(&parameter.id)
                        .is_err()
                        .then_some(position)
                })
                .collect::<Vec<_>>();
            if !positions.is_empty() {
                removed.push((block.id, positions));
            }
        }
        removed.sort_unstable_by_key(|(id, _)| *id);
        Some(Self { live, removed })
    }

    fn is_live(&self, value: ValueId) -> bool {
        self.live.binary_search(&value).is_ok()
    }

    fn remove_unused_block_parameters(&self, machine: &mut TerminalMachine) {
        let positions = |block| {
            self.removed
                .binary_search_by_key(&block, |(id, _)| *id)
                .ok()
                .map(|position| self.removed[position].1.as_slice())
        };
        let remove = |arguments: &mut Vec<ValueId>, removed: Option<&[usize]>| {
            if let Some(removed) = removed {
                let mut position = 0;
                arguments.retain(|_| {
                    let keep = removed.binary_search(&position).is_err();
                    position += 1;
                    keep
                });
            }
        };
        for block in &mut machine.blocks {
            if let Some(removed) = positions(block.id) {
                let mut position = 0;
                block.parameters.retain(|_| {
                    let keep = removed.binary_search(&position).is_err();
                    position += 1;
                    keep
                });
            }
            match &mut block.terminator {
                Terminator::Jump {
                    target, arguments, ..
                } => remove(arguments, positions(*target)),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        remove(&mut edge.arguments, positions(edge.target));
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests;
