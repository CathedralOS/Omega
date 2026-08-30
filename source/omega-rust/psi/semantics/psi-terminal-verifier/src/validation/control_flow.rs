//! Validates terminal control-flow structure, dominance, and successor bindings.

use super::operations::{require_defined, validate_operation_operands};
use super::*;

pub(super) fn validate_control_flow(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    boundary_machines: &[BoundaryMachineDeclaration],
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    representation_backedges: &BTreeSet<EdgeId>,
) -> Result<(), ModuleError> {
    let globally_defined = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let mut definition_blocks = BTreeMap::new();
    for block in blocks.values() {
        for parameter in &block.parameters {
            definition_blocks.insert(parameter.id, block.id);
        }
        for operation in &block.operations {
            if let Some(result) = operation.result.scalar() {
                definition_blocks.insert(result.id, block.id);
            }
        }
    }

    let mut successors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut representation_successors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut predecessors = blocks
        .keys()
        .map(|block| (*block, Vec::<BlockId>::new()))
        .collect::<BTreeMap<_, _>>();
    for block in blocks.values() {
        let targets = match &block.terminator {
            Terminator::Jump { edge, target, .. } => vec![(*edge, *target)],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![
                (when_true.edge, when_true.target),
                (when_false.edge, when_false.target),
            ],
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        for (_, target) in &targets {
            if !blocks.contains_key(target) {
                return Err(ModuleError::UnknownTargetBlock(*target));
            }
        }
        let retained_targets = targets
            .iter()
            .filter_map(|(edge, target)| {
                (!representation_backedges.contains(edge)).then_some(*target)
            })
            .collect::<Vec<_>>();
        for target in &retained_targets {
            predecessors
                .get_mut(target)
                .expect("known target has a predecessor row")
                .push(block.id);
        }
        successors.insert(
            block.id,
            targets.into_iter().map(|(_, target)| target).collect(),
        );
        representation_successors.insert(block.id, retained_targets);
    }

    let mut reachable = BTreeSet::new();
    let mut pending = vec![machine.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(
                successors
                    .get(&block)
                    .expect("every block has successors")
                    .iter()
                    .copied(),
            );
        }
    }
    if reachable.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !reachable.contains(block))
            .copied()
            .expect("different set lengths guarantee an unreachable block");
        return Err(ModuleError::UnreachableBlock(block));
    }

    let mut indegree = predecessors
        .iter()
        .map(|(block, incoming)| (*block, incoming.len()))
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(block) = ready.pop_first() {
        order.push(block);
        for target in representation_successors
            .get(&block)
            .expect("every block has representation successors")
        {
            let count = indegree
                .get_mut(target)
                .expect("known target has an indegree");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    if order.len() != blocks.len() {
        let block = indegree
            .iter()
            .find_map(|(block, count)| (*count != 0).then_some(*block))
            .expect("a cyclic graph leaves positive indegree");
        return Err(ModuleError::ControlCycle(block));
    }

    let mut dominators = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();
    for block in &order {
        let incoming = predecessors
            .get(block)
            .expect("every block has predecessors");
        let mut set = if *block == machine.entry {
            BTreeSet::new()
        } else {
            let mut incoming = incoming.iter();
            let first = incoming
                .next()
                .expect("reachable non-entry block has a predecessor");
            let mut intersection = dominators
                .get(first)
                .expect("topological predecessor has dominators")
                .clone();
            for predecessor in incoming {
                intersection = intersection
                    .intersection(
                        dominators
                            .get(predecessor)
                            .expect("topological predecessor has dominators"),
                    )
                    .copied()
                    .collect();
            }
            intersection
        };
        set.insert(*block);
        dominators.insert(*block, set);
    }

    for block_id in order {
        let block = blocks
            .get(&block_id)
            .copied()
            .expect("topological order contains known blocks");
        let block_dominators = dominators
            .get(&block_id)
            .expect("every ordered block has dominators");
        let mut defined = globally_defined.clone();
        defined.extend(block.parameters.iter().map(|parameter| parameter.id));
        defined.extend(definition_blocks.iter().filter_map(|(value, definition)| {
            (*definition != block_id && block_dominators.contains(definition)).then_some(*value)
        }));
        for operation in &block.operations {
            validate_operation_operands(
                module,
                machine,
                operation,
                machines,
                boundary_machines,
                value_types,
                &defined,
            )?;
            if let Some(result) = operation.result.scalar() {
                defined.insert(result.id);
            }
        }
        match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                arguments,
                ..
            } => validate_successor_bindings(
                *edge,
                *target,
                arguments,
                blocks,
                value_types,
                &defined,
            )?,
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                require_defined(*condition, value_types, &defined)?;
                let actual = value_types[condition];
                if actual != ScalarType::Boolean {
                    return Err(ModuleError::ConditionalConditionTypeMismatch {
                        block: block.id,
                        condition: *condition,
                        actual,
                    });
                }
                for successor in [when_true, when_false] {
                    validate_successor_bindings(
                        successor.edge,
                        successor.target,
                        &successor.arguments,
                        blocks,
                        value_types,
                        &defined,
                    )?;
                }
            }
            Terminator::Return { value, .. } => {
                let Some(result) = machine.result.scalar() else {
                    return Err(ModuleError::ScalarReturnFromUnitMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                };
                require_defined(*value, value_types, &defined)?;
                let value_type = value_types[value];
                if value_type != result.scalar_type {
                    return Err(ModuleError::ReturnTypeMismatch {
                        machine: machine.id,
                        value: value_type,
                        result: result.scalar_type,
                    });
                }
            }
            Terminator::ReturnUnit { .. } => {
                if !matches!(machine.result, TerminalMachineResult::Unit) {
                    return Err(ModuleError::UnitReturnFromScalarMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                }
            }
            Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. } => {
                if !matches!(machine.result, TerminalMachineResult::Unit) {
                    return Err(ModuleError::UnitReturnFromScalarMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                }
            }
            Terminator::ReturnStructural { source, .. } => {
                if machine.result.structural().is_none() {
                    return Err(ModuleError::StructuralReturnFromNonStructuralMachine {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                if !machine
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == *source)
                    && !machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .any(|operation| {
                            operation
                                .result
                                .structural()
                                .is_some_and(|result| result.place == *source)
                        })
                {
                    return Err(ModuleError::StructuralReturnRequiresParameterSource {
                        machine: machine.id,
                        block: block.id,
                        place: *source,
                    });
                }
            }
            Terminator::Crash { site_guard, .. } => {
                for predicate in site_guard {
                    contracts::validate_contract_scope(
                        predicate.proposition(),
                        &defined,
                        machine.contract.id,
                        ContractClauseKind::Crash,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_successor_bindings(
    edge: EdgeId,
    target: BlockId,
    arguments: &[ValueId],
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let target_block = blocks
        .get(&target)
        .copied()
        .ok_or(ModuleError::UnknownTargetBlock(target))?;
    if target_block.parameters.len() != arguments.len() {
        return Err(ModuleError::JumpArityMismatch {
            edge,
            expected: target_block.parameters.len(),
            actual: arguments.len(),
        });
    }
    for (argument, parameter) in arguments.iter().zip(&target_block.parameters) {
        require_defined(*argument, value_types, defined)?;
        let argument_type = value_types[argument];
        if argument_type != parameter.scalar_type {
            return Err(ModuleError::JumpTypeMismatch {
                edge,
                argument: argument_type,
                parameter: parameter.scalar_type,
            });
        }
    }
    Ok(())
}
