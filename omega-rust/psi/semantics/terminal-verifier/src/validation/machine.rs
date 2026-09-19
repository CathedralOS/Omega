//! Registers and validates one terminal machine in canonical source order.
//!
//! `validate_machine` registers the machine's structural places
//! (`structural_places`) and scalar parameters, then each block
//! (`blocks`, which registers every operation through
//! `custody_operations` and `scalar_result_operations`), then checks the
//! entry block, builds the proposition context, validates content claims,
//! crash frontiers and affine cleanups, the contract clauses
//! (`contract_clauses`), and finally control flow, structural frontiers
//! and natural cycles.

mod blocks;
mod contract_clauses;
mod custody_operations;
mod scalar_result_operations;
mod structural_places;

use super::affine_cleanup::{
    nominal_cleanup_contract_receiver, validate_nominal_affine_cleanup_shape,
    validate_partial_affine_cleanup_shape,
};
use super::crash::validate_crash_frontiers;
use super::{
    BTreeMap, BTreeSet, IdRegistry, MachineId, ModuleError, PropositionContext,
    StructuralPlaceKind, TerminalMachine, TerminalModule, content, control_flow, frontier,
    insert_value,
};

pub(super) fn validate_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    registry: &mut IdRegistry,
) -> Result<(), ModuleError> {
    if machine.blocks.is_empty() {
        return Err(ModuleError::MachineHasNoBlocks(machine.id));
    }
    super::references::validate_machine(module, machine)?;

    let contract_receiver = nominal_cleanup_contract_receiver(module, machine.id);
    let mut blocks = BTreeMap::new();
    let mut value_types = BTreeMap::new();
    let structural_place_kinds = structural_places::register_structural_places(machine, registry)?;
    for declaration in machine
        .parameters
        .iter()
        .chain(machine.contract.erased_scalar_formals.iter())
        .chain(machine.result.scalar_ref())
    {
        insert_value(
            &mut value_types,
            &mut registry.values,
            declaration.id,
            declaration.scalar_type,
        )?;
    }
    for block in &machine.blocks {
        blocks::register_block(
            module,
            machine,
            machines,
            block,
            registry,
            &mut blocks,
            &mut value_types,
        )?;
    }
    let Some(entry) = blocks.get(&machine.entry) else {
        return Err(ModuleError::UnknownEntryBlock {
            machine: machine.id,
            block: machine.entry,
        });
    };
    if !entry.parameters.is_empty() || !entry.structural_parameters.is_empty() {
        return Err(ModuleError::EntryBlockCannotHaveParameters(machine.entry));
    }

    let context = PropositionContext::from_value_types_and_places(
        value_types.iter().map(|(id, ty)| (*id, *ty)),
        machine
            .structural_places
            .iter()
            .map(|place| (place.id, place.kind))
            .chain(contract_receiver.map(|receiver| {
                (
                    receiver,
                    StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: true,
                    },
                )
            })),
    )
    .map_err(ModuleError::MalformedProposition)?;
    content::validate_content_entry_claims(machine, registry, &structural_place_kinds, &context)?;
    content::validate_content_identity_reshuffles(
        machine,
        registry,
        &structural_place_kinds,
        &context,
    )?;
    content::validate_content_partition_compositions(
        module,
        machine,
        machines,
        registry,
        &structural_place_kinds,
        &context,
    )?;
    let requires_values = machine
        .parameters
        .iter()
        .chain(machine.contract.erased_scalar_formals.iter())
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    validate_crash_frontiers(module, machine, &context, &requires_values)?;
    validate_partial_affine_cleanup_shape(module, machine, machines)?;
    validate_nominal_affine_cleanup_shape(module, machine, machines)?;
    contract_clauses::validate_contract_clauses(
        module,
        machine,
        registry,
        &context,
        contract_receiver,
        &requires_values,
    )?;

    let dominators = control_flow::validate_control_flow(
        module,
        machine,
        machines,
        &module.boundary_machines,
        &blocks,
        &value_types,
    )?;
    frontier::validate_structural_frontier(module, machine, machines, &blocks, &dominators)?;
    crate::control_cycles::validate_natural_cycles(machine, &dominators)?;
    Ok(())
}
