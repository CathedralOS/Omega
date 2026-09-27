//! Validates terminal control-flow structure, dominance, and successor bindings.
//!
//! `validate_control_flow` collects the machine's definition sites
//! (`definitions`), orders its blocks (`block_graph`), computes dominance
//! and mutable-view availability, then checks each block's operands and
//! successor bindings (`block_checks`).

use super::operations::require_defined;
use super::{
    BTreeMap, BTreeSet, BlockId, BoundaryMachineDeclaration, EdgeId, MachineId, ModuleError,
    OperationKind, ScalarType, StructuralAccess, StructuralFieldType, StructuralTypeShape,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueId,
};

mod block_checks;
mod block_graph;
mod definitions;
mod unranked_cycles;

pub(super) fn validate_control_flow(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    boundary_machines: &[BoundaryMachineDeclaration],
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<crate::control_graph::DominatorTree, ModuleError> {
    let sites = definitions::definition_sites(module, machine, blocks);
    let order = block_graph::block_order(module, machine, blocks)?;
    // Availability is a property of the complete graph, including backedges.
    // Retain the same analysis for the following ownership and ranking checks.
    let dominators = crate::control_graph::dominators(machine);

    let mutable_views = super::block_views::mutable_availability(module, machine);
    for block_id in order {
        block_checks::validate_block(
            module,
            machine,
            machines,
            boundary_machines,
            blocks,
            value_types,
            &sites,
            &dominators,
            &mutable_views,
            block_id,
        )?;
    }
    Ok(dominators)
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
