//! Validates terminal control-flow structure, dominance, and successor bindings.

use super::operations::{require_defined, validate_operation_operands};
use super::*;

mod unranked_cycles;

pub(super) fn validate_control_flow(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    boundary_machines: &[BoundaryMachineDeclaration],
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    representation_backedges: &BTreeSet<EdgeId>,
) -> Result<(), ModuleError> {
    // Ordinary scalar operators, storage and boundary presentation currently
    // consume bare carriers. Reuse their complete operand checks with a bare
    // namespace; direct calls instead transport their full checked signature.
    // Build this projection once per machine, not once per operation.
    let bare_value_types = super::scalar_qualifications::declarations(machine)
        .any(|value| !value.qualifications.is_empty())
        .then(|| {
            super::scalar_qualifications::declarations(machine)
                .filter(|value| value.qualifications.is_empty())
                .map(|value| (value.id, value.scalar_type))
                .collect::<BTreeMap<_, _>>()
        });
    let globally_defined = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let mut definition_blocks = BTreeMap::new();
    // Unrestricted constructed values still require dominance even though they
    // never enter the affine ownership frontier. Track structural establishment
    // independently; each operation separately checks its permitted source shape.
    let mut structural_definitions = BTreeMap::new();
    let mut primitive_local_definitions = BTreeMap::new();
    let mut scalar_array_definitions = BTreeMap::new();
    for block in blocks.values() {
        for parameter in &block.structural_parameters {
            structural_definitions.insert(parameter.place, block.id);
        }
        for parameter in &block.parameters {
            definition_blocks.insert(parameter.id, block.id);
        }
        for operation in &block.operations {
            if let Some(result) = operation.result.structural() {
                structural_definitions.insert(result.place, block.id);
            }
            if let Some(result) = operation.result.structural()
                && super::scalar_array::plain_return_source(module, machine, result.place)
            {
                scalar_array_definitions.insert(result.place, block.id);
            }
            if let Some(result) = operation.result.structural()
                && super::primitive_storage::local_result(machine, result.place).is_some()
            {
                primitive_local_definitions.insert(result.place, block.id);
            }
            if let OperationKind::EstablishByteSequenceLiteral { destination, .. } = operation.kind
            {
                structural_definitions.insert(destination, block.id);
            }
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
            Terminator::StructuralCase { cases, .. } => {
                cases.iter().map(|case| (case.edge, case.target)).collect()
            }
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
    let cyclic = order.len() != blocks.len();
    if cyclic {
        if !representation_backedges.is_empty() || !unranked_cycles::eligible(module, machine) {
            let block = indegree
                .iter()
                .find_map(|(block, count)| (*count != 0).then_some(*block))
                .expect("a cyclic graph leaves positive indegree");
            return Err(ModuleError::ControlCycle(block));
        }
        // Every target and reachable block has been checked before the shared
        // full-graph analysis. Validation below is independent of visit order.
        order = blocks.keys().copied().collect();
    }

    // Array payload slots currently establish once per activation. Keep loop
    // re-establishment unsupported while allowing an outside definition to
    // dominate ordinary uses within a loop.
    if !scalar_array_definitions.is_empty() && (cyclic || !representation_backedges.is_empty()) {
        for component in crate::control_graph::cyclic_components(machine) {
            if let Some(block) = scalar_array_definitions
                .values()
                .find(|block| component.contains(block))
            {
                return Err(ModuleError::ControlCycle(*block));
            }
        }
    }
    let dominators = if cyclic {
        crate::control_graph::dominators(machine)
    } else {
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

        dominators
    };

    let mutable_views = super::block_views::mutable_availability(module, machine);
    for block_id in order {
        let block = blocks
            .get(&block_id)
            .copied()
            .expect("validation order contains known blocks");
        let block_dominators = dominators
            .get(&block_id)
            .expect("every ordered block has dominators");
        let mut defined = globally_defined.clone();
        defined.extend(block.parameters.iter().map(|parameter| parameter.id));
        defined.extend(definition_blocks.iter().filter_map(|(value, definition)| {
            (*definition != block_id && block_dominators.contains(definition)).then_some(*value)
        }));
        let mut available_structural = structural_definitions
            .iter()
            .filter_map(|(place, definition)| {
                (*definition != block_id && block_dominators.contains(definition)).then_some(*place)
            })
            .collect::<BTreeSet<_>>();
        let mut available_primitives = primitive_local_definitions
            .iter()
            .filter_map(|(place, definition)| {
                (*definition != block_id && block_dominators.contains(definition)).then_some(*place)
            })
            .collect::<BTreeSet<_>>();
        let mut available_arrays = scalar_array_definitions
            .iter()
            .filter_map(|(place, definition)| {
                (*definition != block_id && block_dominators.contains(definition)).then_some(*place)
            })
            .collect::<BTreeSet<_>>();
        available_structural.extend(
            block
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place),
        );
        if let Some(mutable) = mutable_views.get(&block_id) {
            available_structural
                .retain(|place| !super::block_views::is_mutable_parameter(machine, *place));
            available_structural.extend(mutable.iter().copied());
        }
        for operation in &block.operations {
            super::structural_case_membership::validate_available(
                machine,
                operation,
                &available_structural,
            )?;
            super::byte_sequence_subslice::validate_uses(
                module,
                machine,
                operation,
                &available_structural,
            )?;
            super::primitive_storage::validate_uses(machine, operation, &available_primitives)?;
            super::record::validate_uses(module, machine, operation, &available_structural)?;
            validate_operation_operands(
                module,
                machine,
                operation,
                machines,
                boundary_machines,
                if matches!(
                    operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::CallUnit { .. }
                        | OperationKind::CallStructuralScalar { .. }
                        | OperationKind::CallStructuralWithScalarArguments { .. }
                ) {
                    value_types
                } else {
                    bare_value_types.as_ref().unwrap_or(value_types)
                },
                &defined,
            )?;
            super::scalar_array::validate_uses(
                operation,
                &scalar_array_definitions,
                &available_arrays,
            )?;
            if let Some(result) = operation.result.structural()
                && scalar_array_definitions.contains_key(&result.place)
            {
                available_arrays.insert(result.place);
            }
            if let Some(result) = operation.result.scalar() {
                defined.insert(result.id);
            }
            if let Some(result) = operation.result.structural()
                && primitive_local_definitions.contains_key(&result.place)
            {
                available_primitives.insert(result.place);
            }
            if let OperationKind::EstablishByteSequenceLiteral { destination, .. } = operation.kind
            {
                available_structural.insert(destination);
            }
            if let Some(result) = operation.result.structural()
                && structural_definitions.contains_key(&result.place)
            {
                available_structural.insert(result.place);
            }
        }
        match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                arguments,
                structural_arguments,
                ..
            } => {
                validate_successor_bindings(
                    *edge,
                    *target,
                    arguments,
                    blocks,
                    value_types,
                    &defined,
                )?;
                super::block_views::validate_successor(
                    module,
                    machine,
                    *edge,
                    blocks[target],
                    structural_arguments,
                    &available_structural,
                    block_dominators,
                )?;
            }
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
                    super::block_views::validate_successor(
                        module,
                        machine,
                        successor.edge,
                        blocks[&successor.target],
                        &successor.structural_arguments,
                        &available_structural,
                        block_dominators,
                    )?;
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
            Terminator::StructuralCase { source, cases } => {
                // Dispatch observes the stored tag. Logical entry refinements
                // do not grant an executable read through a write-only loan.
                // Operation results retain their independently validated owned
                // origin and are not narrowed by this parameter access check.
                if machine.structural_parameters.iter().any(|parameter| {
                    parameter.place == *source
                        && parameter.access == StructuralAccess::WriteOnlyBorrow
                }) {
                    return Err(ModuleError::StructuralCaseRequiresReadableAccess {
                        machine: machine.id,
                        block: block.id,
                        source: *source,
                    });
                }
                for successor in cases {
                    super::block_views::validate_successor(
                        module,
                        machine,
                        successor.edge,
                        blocks[&successor.target],
                        &[],
                        &available_structural,
                        block_dominators,
                    )?;
                }
                let source_signature = super::structural_result_contracts::source_signature(
                    machine, *source,
                )
                .ok_or(ModuleError::StructuralCaseSourceUnknown {
                    machine: machine.id,
                    block: block.id,
                    place: *source,
                })?;
                let source_definition = machine.blocks.iter().find_map(|candidate| {
                    if candidate
                        .structural_parameters
                        .iter()
                        .any(|parameter| parameter.place == *source)
                    {
                        return Some(candidate.id);
                    }
                    candidate.operations.iter().find_map(|operation| {
                        operation
                            .result
                            .structural()
                            .is_some_and(|result| result.place == *source)
                            .then_some(candidate.id)
                    })
                });
                if source_definition.is_some_and(|definition| {
                    definition != block.id && !block_dominators.contains(&definition)
                }) {
                    return Err(ModuleError::StructuralCaseSourceUnknown {
                        machine: machine.id,
                        block: block.id,
                        place: *source,
                    });
                }
                validate_structural_case_successors(
                    module,
                    machine,
                    block.id,
                    source_signature.structural_type,
                    cases,
                    blocks,
                )?;
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
                let block_parameter = super::block_views::parameter(machine, *source);
                // A result can be copyable without being available on this path.
                // Every local producer, not only arrays or block parameters,
                // must dominate the return independently of disposal custody.
                if structural_definitions.contains_key(source)
                    && !available_structural.contains(source)
                {
                    return Err(ModuleError::StructuralReturnSourceNotLive {
                        machine: machine.id,
                        block: block.id,
                        place: *source,
                    });
                }
                if scalar_array_definitions.contains_key(source)
                    && !available_arrays.contains(source)
                {
                    return Err(ModuleError::StructuralReturnSourceNotLive {
                        machine: machine.id,
                        block: block.id,
                        place: *source,
                    });
                }
                if super::byte_sequence_subslice::borrowed_result(machine, *source).is_some()
                    || block_parameter
                        .is_some_and(|parameter| parameter.access != StructuralAccess::Owned)
                {
                    return Err(ModuleError::ByteSequenceSubsliceReturnUnsupported {
                        machine: machine.id,
                        place: *source,
                    });
                }
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
                    && block_parameter.is_none()
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
                        module,
                        machine,
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

fn validate_structural_case_successors(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: BlockId,
    structural_type: semantic_vocabulary::StructuralTypeId,
    successors: &[terminal_psi::StructuralCaseSuccessorEdge],
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
) -> Result<(), ModuleError> {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .ok_or(ModuleError::UnknownStructuralType(structural_type))?;
    let cases = match &declaration.shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => {
            return Err(ModuleError::StructuralCaseRequiresClosedSum {
                machine: machine.id,
                block,
                structural_type,
            });
        }
    };
    if successors.len() != cases.len()
        || successors
            .iter()
            .zip(cases)
            .any(|(successor, case)| successor.case != case.id)
    {
        return Err(ModuleError::StructuralCaseRosterMismatch {
            machine: machine.id,
            block,
        });
    }
    for (successor, case) in successors.iter().zip(cases) {
        let target = blocks
            .get(&successor.target)
            .copied()
            .ok_or(ModuleError::UnknownTargetBlock(successor.target))?;
        if successor.payload_fields.len() != target.parameters.len() {
            return Err(ModuleError::StructuralCasePayloadMismatch {
                edge: successor.edge,
                case: successor.case,
                field: None,
            });
        }
        for (field_id, parameter) in successor.payload_fields.iter().zip(&target.parameters) {
            let Some(field) = case.fields.iter().find(|field| field.id == *field_id) else {
                return Err(ModuleError::StructuralCasePayloadMismatch {
                    edge: successor.edge,
                    case: successor.case,
                    field: Some(*field_id),
                });
            };
            if field.relevance.is_erased()
                || !match field.field_type {
                    StructuralFieldType::Scalar(actual) => actual == parameter.scalar_type,
                    StructuralFieldType::BoundedInteger(bounded) => {
                        ScalarType::Integer(bounded.integer_type()) == parameter.scalar_type
                    }
                    _ => false,
                }
            {
                return Err(ModuleError::StructuralCasePayloadMismatch {
                    edge: successor.edge,
                    case: successor.case,
                    field: Some(*field_id),
                });
            }
        }
    }
    Ok(())
}

fn validate_successor_bindings(
    edge: EdgeId,
    target: BlockId,
    arguments: &[ValueId],
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
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
