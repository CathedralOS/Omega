//! One block's registration: its id, parameters, operations, edges and
//! nominal cleanup obligations.

use super::super::affine_cleanup::nominal_cleanups;
use super::super::{
    BTreeMap, IdRegistry, MachineId, ModuleError, ScalarType, TerminalMachine, TerminalModule,
    insert_unique, insert_value,
};
use super::{custody_operations, scalar_result_operations};
use semantic_vocabulary::{BlockId, ValueId};
use terminal_psi::Block;

/// Registers one block: its id, its scalar parameters' value types, each
/// operation (custody and obligations, then its scalar result type), its
/// terminator's edges and its nominal cleanups' obligations.
pub(super) fn register_block<'m>(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    block: &'m Block,
    registry: &mut IdRegistry,
    blocks: &mut BTreeMap<BlockId, &'m Block>,
    value_types: &mut BTreeMap<ValueId, ScalarType>,
) -> Result<(), ModuleError> {
    insert_unique(&mut registry.blocks, block.id, ModuleError::DuplicateBlock)?;
    if blocks.insert(block.id, block).is_some() {
        return Err(ModuleError::DuplicateBlock(block.id));
    }
    for parameter in &block.parameters {
        insert_value(
            value_types,
            &mut registry.values,
            parameter.id,
            parameter.scalar_type,
        )?;
    }
    for operation in &block.operations {
        insert_unique(
            &mut registry.operations,
            operation.id,
            ModuleError::DuplicateOperation,
        )?;
        // Each runtime index an operation's projections carry is an
        // obligation that operation owns, whatever the operation's kind.
        for (_, obligation) in operation.kind.runtime_indexes() {
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
        if custody_operations::register_custody_operation(
            module,
            machine,
            machines,
            operation,
            registry,
            value_types,
        )? {
            continue;
        }
        scalar_result_operations::register_scalar_result_operation(
            module,
            machine,
            machines,
            operation,
            registry,
            value_types,
        )?;
    }
    for edge in block.terminator.edges() {
        insert_unique(&mut registry.edges, edge, ModuleError::DuplicateEdge)?;
    }
    for cleanup in nominal_cleanups(&block.terminator) {
        for obligation in &cleanup.requirement_obligations {
            insert_unique(
                &mut registry.obligations,
                *obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
    }
    Ok(())
}
