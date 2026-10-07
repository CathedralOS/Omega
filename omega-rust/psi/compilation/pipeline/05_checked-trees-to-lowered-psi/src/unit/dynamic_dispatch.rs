//! Runtime implementation selection: the module's interface/table catalog.
//!
//! Checking fixes the implementation each `Impl as dyn Trait<Args>` selects
//! (`DynamicSelectionFact`), and stage 04 publishes the call shape of every
//! slot a selection or selector call reaches (`CheckedDynamicSlotPlan`).
//! Every distinct selector type is one interface and every distinct
//! (interface, implementation) pair is one table. Ids come from sorting those
//! program-wide identities, so a selection and a dynamic call emitted while
//! one body lowers agree with the catalog assembled after the whole closure
//! lowers, without shared mutable state.
//!
//! An interface's slot order is the canonical row order its selected
//! implementations share; an interface no implementation is selected for
//! orders its called slots by requirement identity, since no table can
//! exist for it. Slot call shapes come from the checked slot plans, never
//! from one implementation, and assembly refuses a row that does not realize
//! its slot's exact shape. A table whose selection lies outside this module's
//! closure keeps absent rows: no selector naming it can be produced here.

use crate::LoweringError;
use checked_trees::{CheckedBoundaryMachineResultPlan, CheckedDynamicSlotPlan, CheckedTrees};
use symbols::SymbolHandle;
use terminal_psi::{
    TerminalDynamicDispatchCatalog, TerminalDynamicInterface, TerminalDynamicSlot,
    TerminalDynamicStructuralFormal, TerminalDynamicTable, TerminalMachineResult, TerminalModule,
};

/// Identity-to-id maps for one program's interfaces and tables.
#[derive(Debug, Clone, Default)]
pub(crate) struct DynamicDispatchIndex {
    /// Sorted, unique interface identities; position is the interface id.
    interfaces: Vec<String>,
    /// Sorted, unique (interface identity, implementation); position is the
    /// table id.
    tables: Vec<(String, SymbolHandle)>,
    /// Per interface id: the canonical requirement identities, by slot.
    slots: Vec<Vec<String>>,
}

impl DynamicDispatchIndex {
    pub(crate) fn new(checked: &CheckedTrees) -> Self {
        let selections = &checked.facts.dynamic_conformances.selections;
        let slot_plans = &checked.facts.flow.terminal_unit_effects.dynamic_slots;
        let mut interfaces = selections
            .iter()
            .map(|selection| selection.interface_identity.clone())
            .chain(
                slot_plans
                    .iter()
                    .map(|plan| plan.interface_identity.clone()),
            )
            .collect::<Vec<_>>();
        interfaces.sort();
        interfaces.dedup();
        let mut tables = selections
            .iter()
            .map(|selection| {
                (
                    selection.interface_identity.clone(),
                    selection.implementation,
                )
            })
            .collect::<Vec<_>>();
        tables.sort_by(|(left, left_symbol), (right, right_symbol)| {
            left.cmp(right)
                .then(left_symbol.arena_index().cmp(&right_symbol.arena_index()))
        });
        tables.dedup();
        let slots = interfaces
            .iter()
            .map(|interface| {
                match selections
                    .iter()
                    .find(|selection| &selection.interface_identity == interface)
                {
                    Some(selection) => selection
                        .rows
                        .iter()
                        .map(|row| row.requirement_identity.clone())
                        .collect(),
                    None => {
                        let mut called = slot_plans
                            .iter()
                            .filter(|plan| &plan.interface_identity == interface)
                            .map(|plan| plan.requirement_identity.clone())
                            .collect::<Vec<_>>();
                        called.sort();
                        called.dedup();
                        called
                    }
                }
            })
            .collect();
        Self {
            interfaces,
            tables,
            slots,
        }
    }

    pub(crate) fn interface(&self, identity: &str) -> Option<u32> {
        let position = self
            .interfaces
            .binary_search_by(|candidate| candidate.as_str().cmp(identity))
            .ok()?;
        u32::try_from(position).ok()
    }

    pub(crate) fn table(&self, interface: &str, implementation: SymbolHandle) -> Option<u32> {
        let position = self
            .tables
            .binary_search_by(|(candidate, symbol)| {
                candidate
                    .as_str()
                    .cmp(interface)
                    .then(symbol.arena_index().cmp(&implementation.arena_index()))
            })
            .ok()?;
        u32::try_from(position).ok()
    }

    /// The slot a requirement occupies in an interface.
    pub(crate) fn slot(&self, interface: u32, requirement_identity: &str) -> Option<u32> {
        let slots = self.slots.get(usize::try_from(interface).ok()?)?;
        let position = slots
            .iter()
            .position(|candidate| candidate == requirement_identity)?;
        u32::try_from(position).ok()
    }
}

/// Publish the dispatch catalog of one lowered closure. `source_machines`
/// names the source owner of each `module.machines` entry, in order.
pub(crate) fn lower_dynamic_dispatch_catalog(
    checked: &CheckedTrees,
    source_machines: &[SymbolHandle],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    let index = DynamicDispatchIndex::new(checked);
    if index.interfaces.is_empty() {
        module.dynamic_dispatch = TerminalDynamicDispatchCatalog::default();
        return Ok(());
    }
    if source_machines.len() != module.machines.len() {
        return unsupported("dynamic dispatch rows need the closure's source owners");
    }
    let owners = source_machines
        .iter()
        .zip(&module.machines)
        .map(|(source, machine)| (*source, machine.id))
        .collect::<Vec<_>>();
    let type_ids = module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let slot_plans = &checked.facts.flow.terminal_unit_effects.dynamic_slots;
    let mut catalog = TerminalDynamicDispatchCatalog::default();
    for (interface_id, interface_identity) in index.interfaces.iter().enumerate() {
        let interface_id = u32::try_from(interface_id)
            .map_err(|_| LoweringError::Unsupported("dynamic interface id overflows"))?;
        let mut slots = Vec::new();
        for (slot, requirement_identity) in index.slots[interface_id as usize].iter().enumerate() {
            let Some(plan) = slot_plans.iter().find(|plan| {
                &plan.interface_identity == interface_identity
                    && &plan.requirement_identity == requirement_identity
            }) else {
                return unsupported("dynamic interface slot has no checked call shape");
            };
            slots.push(lower_slot(
                checked,
                plan,
                u32::try_from(slot)
                    .map_err(|_| LoweringError::Unsupported("dynamic slot id overflows"))?,
                &type_ids,
            )?);
        }
        catalog.interfaces.push(TerminalDynamicInterface {
            id: interface_id,
            identity: interface_identity.clone(),
            slots,
        });
    }
    for (table_id, (interface_identity, implementation)) in index.tables.iter().enumerate() {
        let table_id = u32::try_from(table_id)
            .map_err(|_| LoweringError::Unsupported("dynamic table id overflows"))?;
        let interface = index
            .interface(interface_identity)
            .ok_or(LoweringError::Unsupported(
                "dynamic table names no interface",
            ))?;
        let selections = checked
            .facts
            .dynamic_conformances
            .selections
            .iter()
            .filter(|selection| {
                &selection.interface_identity == interface_identity
                    && selection.implementation == *implementation
            })
            .collect::<Vec<_>>();
        let Some(selection) = selections.first() else {
            return unsupported("dynamic table lost its selection");
        };
        // A selector naming this table exists in this module only when one
        // of its selections does; then every row must be lowered here.
        let selected_here = selections
            .iter()
            .any(|selection| source_machines.contains(&selection.machine));
        let slots = &catalog.interfaces[interface as usize].slots;
        if selection.rows.len() != slots.len() {
            return unsupported("implementation rows disagree with their interface slots");
        }
        let mut rows = Vec::with_capacity(slots.len());
        for (row, slot) in selection.rows.iter().zip(slots) {
            if row.requirement_identity != slot.requirement_identity {
                return unsupported("implementation row is out of its interface's slot order");
            }
            if !selected_here {
                rows.push(None);
                continue;
            }
            let id =
                crate::terminal_identities::lookup_machine_id(&owners, row.realization_machine)?;
            let realization = module
                .machines
                .iter()
                .find(|machine| machine.id == id)
                .ok_or(LoweringError::Unsupported(
                    "dynamic table row names a machine outside the lowered closure",
                ))?;
            if !realizes_slot(realization, slot) {
                return unsupported("implementation row does not realize its slot's call shape");
            }
            rows.push(Some(id));
        }
        catalog.tables.push(TerminalDynamicTable {
            id: table_id,
            interface,
            implementation_identity: checked.symbols.display_path(*implementation, "::"),
            rows,
        });
    }
    module.dynamic_dispatch = catalog;
    Ok(())
}

/// One slot's call shape from its checked plan. The selector is operand 0
/// of the plan and no implementation receives it, so formal positions shift
/// down by one into the implementations' own numbering.
fn lower_slot(
    checked: &CheckedTrees,
    plan: &CheckedDynamicSlotPlan,
    slot: u32,
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<TerminalDynamicSlot, LoweringError> {
    let Some((selector, scalars)) = plan.scalar_parameters.split_first() else {
        return unsupported("dynamic slot lost its selector operand");
    };
    if selector.source_position != 0 {
        return unsupported("dynamic slot selector is not its first operand");
    }
    let scalar_parameters = scalars
        .iter()
        .map(|parameter| {
            crate::emission::scalar_types::terminal_scalar_type(parameter.primitive_type)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let structural_parameters = plan
        .structural_parameters
        .iter()
        .map(|parameter| {
            Ok(TerminalDynamicStructuralFormal {
                position: parameter
                    .position
                    .checked_sub(1)
                    .ok_or(LoweringError::Unsupported(
                        "dynamic slot structural formal precedes its selector",
                    ))?,
                structural_type: crate::terminal_identities::lookup_type_id(
                    type_ids,
                    &parameter.type_identity,
                )?,
                access: crate::expression_preparation::bindings::structural_paths::terminal_access(
                    parameter.access,
                ),
                multiplicity: crate::terminal_identities::structural_multiplicity(
                    parameter.multiplicity,
                ),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let result = match plan.result {
        CheckedBoundaryMachineResultPlan::Unit => None,
        CheckedBoundaryMachineResultPlan::Scalar(primitive) => Some(
            crate::emission::scalar_types::terminal_scalar_type(primitive)?,
        ),
        _ => return unsupported("a selector call's structural result is not yet planned"),
    };
    Ok(TerminalDynamicSlot {
        slot,
        declaring_trait_identity: checked.symbols.display_path(plan.declaring_trait, "::"),
        requirement_identity: plan.requirement_identity.clone(),
        family_tuple: Vec::new(),
        scalar_parameters,
        structural_parameters,
        result,
    })
}

/// Whether one lowered implementation takes exactly the slot's formals and
/// returns exactly its result.
fn realizes_slot(machine: &terminal_psi::TerminalMachine, slot: &TerminalDynamicSlot) -> bool {
    let scalars_match = machine.parameters.len() == slot.scalar_parameters.len()
        && machine
            .parameters
            .iter()
            .zip(&slot.scalar_parameters)
            .all(|(parameter, expected)| parameter.scalar_type == *expected);
    let structurals_match = machine.structural_parameters.len() == slot.structural_parameters.len()
        && machine
            .structural_parameters
            .iter()
            .zip(&slot.structural_parameters)
            .all(|(parameter, expected)| {
                parameter.position == expected.position
                    && parameter.structural_type == expected.structural_type
                    && parameter.access == expected.access
                    && parameter.multiplicity == expected.multiplicity
            });
    let result_matches = match (&machine.result, slot.result) {
        (TerminalMachineResult::Unit, None) => true,
        (TerminalMachineResult::Scalar(result), Some(expected)) => result.scalar_type == expected,
        _ => false,
    };
    scalars_match && structurals_match && result_matches
}

fn unsupported<T>(reason: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(reason))
}
