//! One block's operand and successor-binding checks.

use super::definitions::DefinitionSites;
use crate::validation::operations::{require_defined, validate_operation_operands};
use crate::validation::{
    BTreeMap, BTreeSet, BlockId, BoundaryMachineDeclaration, ContractClauseKind, MachineId,
    ModuleError, OperationKind, ScalarType, StructuralAccess, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueId, contracts,
};
use semantic_vocabulary::PlaceId;

use super::{validate_structural_case_successors, validate_successor_bindings};

/// One block's checks: the values and places available at its entry (those
/// defined globally, by dominating blocks, and by its own parameters), each
/// operation's operands against them, and its terminator's successor
/// bindings.
pub(super) fn validate_block(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    boundary_machines: &[BoundaryMachineDeclaration],
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    sites: &DefinitionSites,
    dominators: &crate::control_graph::DominatorTree,
    mutable_views: &BTreeMap<BlockId, BTreeSet<PlaceId>>,
    block_id: BlockId,
) -> Result<(), ModuleError> {
    let DefinitionSites {
        bare_value_types,
        globally_defined,
        structural_definitions,
        primitive_local_definitions,
        scalar_array_definitions,
        by_block,
        ..
    } = sites;
    let block = blocks
        .get(&block_id)
        .copied()
        .expect("validation order contains known blocks");
    let mut defined = globally_defined.clone();
    defined.extend(block.parameters.iter().map(|parameter| parameter.id));
    // Available at entry: everything defined in a strictly dominating block.
    let mut available_structural = BTreeSet::new();
    let mut available_primitives = BTreeSet::new();
    let mut available_arrays = BTreeSet::new();
    for dominator in dominators.strict_dominators(block_id) {
        let Some(definitions) = by_block.get(&dominator) else {
            continue;
        };
        defined.extend(definitions.values.iter().copied());
        available_structural.extend(definitions.structural.iter().copied());
        available_primitives.extend(definitions.primitive_locals.iter().copied());
        available_arrays.extend(definitions.scalar_arrays.iter().copied());
    }
    available_structural.extend(
        block
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place),
    );
    if let Some(mutable) = mutable_views.get(&block_id) {
        available_structural
            .retain(|place| !crate::validation::block_views::is_mutable_parameter(machine, *place));
        available_structural.extend(mutable.iter().copied());
    }
    for operation in &block.operations {
        crate::validation::structural::case_membership::validate_available(
            machine,
            operation,
            &available_structural,
        )?;
        crate::validation::structural::leaf_copy::validate_available(
            machine,
            operation,
            &available_structural,
        )?;
        crate::validation::byte_sequence::subslice::validate_uses(
            module,
            machine,
            operation,
            &available_structural,
        )?;
        crate::validation::element_view::establishment::validate_uses(
            machine,
            operation,
            &available_structural,
        )?;
        crate::validation::element_view::subslice::validate_uses(
            module,
            machine,
            operation,
            &available_structural,
        )?;
        crate::validation::primitive_storage::validate_uses(
            machine,
            operation,
            &available_primitives,
        )?;
        crate::validation::record::validate_uses(
            module,
            machine,
            operation,
            &available_structural,
        )?;
        crate::validation::scalar::case::validate_uses(
            module,
            machine,
            operation,
            &available_structural,
        )?;
        crate::validation::references::validate_uses(machine, operation, &available_structural)?;
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
        crate::validation::structural::runtime_indexes::validate_operands(
            operation,
            value_types,
            &defined,
        )?;
        crate::validation::scalar::array::validate_uses(
            operation,
            scalar_array_definitions,
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
        if let OperationKind::EstablishByteSequenceLiteral { destination, .. } = operation.kind {
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
            validate_successor_bindings(*edge, *target, arguments, blocks, value_types, &defined)?;
            crate::validation::block_views::validate_successor(
                module,
                machine,
                *edge,
                blocks[target],
                structural_arguments,
                &available_structural,
                dominators,
                block_id,
                true,
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
                crate::validation::block_views::validate_successor(
                    module,
                    machine,
                    successor.edge,
                    blocks[&successor.target],
                    &successor.structural_arguments,
                    &available_structural,
                    dominators,
                    block_id,
                    false,
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
                parameter.place == *source && parameter.access == StructuralAccess::WriteOnlyBorrow
            }) {
                return Err(ModuleError::StructuralCaseRequiresReadableAccess {
                    machine: machine.id,
                    block: block.id,
                    source: *source,
                });
            }
            for successor in cases {
                crate::validation::block_views::validate_successor(
                    module,
                    machine,
                    successor.edge,
                    blocks[&successor.target],
                    &[],
                    &available_structural,
                    dominators,
                    block_id,
                    false,
                )?;
            }
            let source_signature =
                crate::validation::structural::result_contracts::source_signature(machine, *source)
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
                definition != block.id && !dominators.dominates(definition, block_id)
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
        Terminator::ReturnUnitPartialAffine { .. } | Terminator::ReturnUnitNominalAffine { .. } => {
            if !matches!(machine.result, TerminalMachineResult::Unit) {
                return Err(ModuleError::UnitReturnFromScalarMachine {
                    machine: machine.id,
                    block: block.id,
                });
            }
        }
        Terminator::ReturnStructural { source, .. } => {
            let block_parameter = crate::validation::block_views::parameter(machine, *source);
            // A result can be copyable without being available on this path.
            // Every local producer, not only arrays or block parameters,
            // must dominate the return independently of disposal custody.
            if structural_definitions.contains_key(source) && !available_structural.contains(source)
            {
                return Err(ModuleError::StructuralReturnSourceNotLive {
                    machine: machine.id,
                    block: block.id,
                    place: *source,
                });
            }
            if scalar_array_definitions.contains_key(source) && !available_arrays.contains(source) {
                return Err(ModuleError::StructuralReturnSourceNotLive {
                    machine: machine.id,
                    block: block.id,
                    place: *source,
                });
            }
            if crate::validation::byte_sequence::subslice::borrowed_result(machine, *source)
                .is_some()
                || crate::validation::element_view::subslice::borrowed_result(machine, *source)
                    .is_some()
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
    Ok(())
}
