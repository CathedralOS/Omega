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
use checked_trees::{CheckedDynamicSlotPlan, CheckedTrees};
use symbols::SymbolHandle;
use terminal_psi::{
    TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog, TerminalDynamicInterface,
    TerminalDynamicSlot, TerminalDynamicStructuralFormal, TerminalDynamicTable, TerminalModule,
};

/// Identity-to-id maps for one program's interfaces and tables.
#[derive(Debug, Clone, Default)]
pub(crate) struct DynamicDispatchIndex {
    /// Sorted, unique interface identities; position is the interface id.
    interfaces: Vec<typed_trees::type_identity::TypeIdentityId>,
    /// Sorted, unique (interface identity, implementation); position is the
    /// table id.
    tables: Vec<(typed_trees::type_identity::TypeIdentityId, SymbolHandle)>,
    /// Per interface id: each slot's requirement identity and, for a
    /// finite-family row, the canonical tuple identities it commits; one
    /// slot per canonical row.
    slots: Vec<Vec<(typed_trees::type_identity::CallableIdentityId, Vec<String>)>>,
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
                        .map(|row| {
                            (
                                row.requirement_identity.clone(),
                                row.family_tuple.as_ref().to_vec(),
                            )
                        })
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
                            .into_iter()
                            .map(|identity| (identity, Vec::new()))
                            .collect()
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

    pub(crate) fn interface(
        &self,
        identity: &typed_trees::type_identity::TypeIdentityId,
    ) -> Option<u32> {
        let position = self.interfaces.binary_search(identity).ok()?;
        u32::try_from(position).ok()
    }

    pub(crate) fn table(
        &self,
        interface: &typed_trees::type_identity::TypeIdentityId,
        implementation: SymbolHandle,
    ) -> Option<u32> {
        let position = self
            .tables
            .binary_search_by(|(candidate, symbol)| {
                candidate
                    .cmp(interface)
                    .then(symbol.arena_index().cmp(&implementation.arena_index()))
            })
            .ok()?;
        u32::try_from(position).ok()
    }

    /// One interface's slots with each slot's requirement identity and
    /// canonical tuple roster entry (`Vec::new()` for nongeneric rows); a
    /// family call reads the contiguous tuple run starting at its base slot.
    pub(crate) fn interface_slots(
        &self,
        interface: u32,
    ) -> Option<&[(typed_trees::type_identity::CallableIdentityId, Vec<String>)]> {
        self.slots
            .get(usize::try_from(interface).ok()?)
            .map(Vec::as_slice)
    }

    /// The slot a requirement occupies in an interface.
    pub(crate) fn slot(
        &self,
        interface: u32,
        requirement_identity: &typed_trees::type_identity::CallableIdentityId,
    ) -> Option<u32> {
        let slots = self.slots.get(usize::try_from(interface).ok()?)?;
        let position = slots
            .iter()
            .position(|(candidate, _)| candidate == requirement_identity)?;
        u32::try_from(position).ok()
    }

    /// The `(interface, slot)` a selector call names, with the one refusal
    /// every selector-call emitter reports for an unselected interface or an
    /// absent requirement.
    pub(crate) fn resolve_slot(
        &self,
        interface_identity: &typed_trees::type_identity::TypeIdentityId,
        requirement_identity: &typed_trees::type_identity::CallableIdentityId,
    ) -> Result<(u32, u32), LoweringError> {
        let interface = self
            .interface(interface_identity)
            .ok_or(LoweringError::Unsupported(
                "selector call names an interface no implementation is selected for",
            ))?;
        let slot = self
            .slot(interface, requirement_identity)
            .ok_or(LoweringError::Unsupported(
                "selector call names a requirement absent from its interface",
            ))?;
        Ok((interface, slot))
    }

    /// The tuple roster of a finite-family call: the contiguous run of the
    /// requirement's tuple rows starting at its base slot, which dispatch
    /// matches against the evaluated family actuals in order.
    pub(crate) fn family_roster(
        &self,
        interface: u32,
        slot: u32,
        requirement_identity: &typed_trees::type_identity::CallableIdentityId,
    ) -> Result<Vec<Vec<String>>, LoweringError> {
        let slots = self
            .interface_slots(interface)
            .ok_or(LoweringError::Unsupported(
                "selector call names an interface with no slot roster",
            ))?;
        let base = usize::try_from(slot)
            .map_err(|_| LoweringError::Unsupported("dynamic family slot overflows"))?;
        Ok(slots
            .iter()
            .skip(base)
            .take_while(|(requirement, tuple)| {
                requirement == requirement_identity && !tuple.is_empty()
            })
            .map(|(_, tuple)| tuple.clone())
            .collect())
    }
}

/// Publish the dispatch catalog of one lowered closure. `source_machines`
/// names the source owner of each `module.machines` entry, in order.
pub(crate) fn lower_dynamic_dispatch_catalog(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
        .map(|declaration| {
            (
                checked.type_identity_by_text(
                    module
                        .identities
                        .text(declaration.identity)
                        .unwrap_or_default(),
                ),
                declaration.id,
            )
        })
        .collect::<Vec<_>>();
    // The emitted domain catalog retains the exact source semantic identity
    // and storage carrier; no implementation or diagnostic spelling selects it.
    let domain_ids = module
        .structural_domains
        .iter()
        .map(|domain| {
            let semantic_domain = checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_domains
                .iter()
                .find(|plan| {
                    u64::from(plan.domain.0) == domain.semantic_domain.get()
                        && type_ids.iter().any(|(identity, carrier)| {
                            *identity == plan.carrier_type_identity && *carrier == domain.carrier
                        })
                })
                .map(|plan| plan.domain)
                .ok_or(LoweringError::Unsupported(
                    "dynamic domain lost its exact source carrier",
                ))?;
            Ok(crate::terminal_identities::StructuralDomainBinding {
                byte_predicate: if domain.content_projection.is_none()
                    && domain.establishment_routes.is_empty()
                {
                    domain.byte_predicate
                } else {
                    None
                },
                semantic_domain,
                carrier: domain.carrier,
                domain: domain.id,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let slot_plans = &checked.facts.flow.terminal_unit_effects.dynamic_slots;
    let mut catalog = TerminalDynamicDispatchCatalog::default();
    for (interface_id, interface_identity) in index.interfaces.iter().enumerate() {
        let interface_id = u32::try_from(interface_id)
            .map_err(|_| LoweringError::Unsupported("dynamic interface id overflows"))?;
        let mut slots = Vec::new();
        for (slot, (requirement_identity, family_tuple)) in
            index.slots[interface_id as usize].iter().enumerate()
        {
            let Some(plan) = slot_plans.iter().find(|plan| {
                plan.interface_identity == *interface_identity
                    && plan.requirement_identity == *requirement_identity
            }) else {
                return unsupported("dynamic interface slot has no checked call shape");
            };
            slots.push(lower_slot(
                identities,
                checked,
                plan,
                u32::try_from(slot)
                    .map_err(|_| LoweringError::Unsupported("dynamic slot id overflows"))?,
                family_tuple,
                &type_ids,
                &module.structural_types,
                &domain_ids,
            )?);
        }
        catalog.interfaces.push(TerminalDynamicInterface {
            id: interface_id,
            identity: identities.intern(
                &checked
                    .type_identity_text(*interface_identity)
                    .unwrap_or_default(),
            ),
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
            if identities.intern(
                &checked
                    .typed
                    .callable_identity_text(row.requirement_identity),
            ) != slot.requirement_identity
            {
                return unsupported("implementation row is out of its interface's slot order");
            }
            if !selected_here {
                rows.push(None);
                continue;
            }
            let id =
                crate::terminal_identities::lookup_machine_id(&owners, row.realization_machine)?;
            let realization = module.machine(id).ok_or(LoweringError::Unsupported(
                "dynamic table row names a machine outside the lowered closure",
            ))?;
            if !realizes_slot(realization, slot, &module.structural_types) {
                return unsupported("implementation row does not realize its slot's call shape");
            }
            rows.push(Some(id));
        }
        catalog.tables.push(TerminalDynamicTable {
            id: table_id,
            interface,
            implementation_identity: identities
                .intern(&checked.symbols.display_path(*implementation, "::")),
            rows,
        });
    }
    module.dynamic_dispatch = catalog;
    Ok(())
}

/// One slot's call shape from its checked plan. The selector is operand 0
/// of the plan and no implementation receives it. Terminal scalar and
/// structural formals have independent dense rosters, just as ordinary
/// machine lowering does: source positions minus the selector are not
/// structural-lane positions when scalar arguments precede a borrow.
fn lower_slot(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plan: &CheckedDynamicSlotPlan,
    slot: u32,
    family_tuple: &[String],
    type_ids: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    domain_ids: &[crate::terminal_identities::StructuralDomainBinding],
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
        .enumerate()
        .map(|(position, parameter)| {
            if parameter.position == 0 {
                return unsupported("dynamic slot structural formal precedes its selector");
            }
            Ok(TerminalDynamicStructuralFormal {
                position: u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported(
                        "dynamic slot structural parameter count exceeds u32",
                    )
                })?,
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
    let mut entry_claim_paths = plan.entry_claim_paths.iter().map(|claim| {
        let formal = structural_parameters.get(claim.argument_index as usize).ok_or(
            LoweringError::Unsupported("dynamic slot claim has no structural formal"),
        )?;
        Ok(TerminalDynamicClaimPath {
            argument_index: claim.argument_index,
            path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(identities, &claim.path, formal.structural_type, structural_types)?,
        })
    }).collect::<Result<Vec<_>, LoweringError>>()?;
    entry_claim_paths.sort();
    let result = super::attached_unit::lower_boundary_result(
        identities,
        &plan.result,
        type_ids,
        structural_types,
        domain_ids,
    )?;
    let mut result_claim_paths = match &result {
        terminal_psi::BoundaryMachineResult::Structural(result) => plan.result_claim_paths.iter()
            .map(|path| crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                identities, path, result.structural_type, structural_types,
            )).collect::<Result<Vec<_>, _>>()?,
        _ if plan.result_claim_paths.is_empty() => Vec::new(),
        _ => return unsupported("nonstructural dynamic result carries claims"),
    };
    result_claim_paths.sort();
    let mut result_reference_regions = Vec::new();
    for reference in &plan.result_reference_sources {
        if reference.source.extent != semantic_vocabulary::ReferenceOriginExtent::WithinRegion {
            return unsupported("dynamic result contract contains a private precise origin");
        }
        let terminal_psi::BoundaryMachineResult::Structural(result) = &result else {
            return unsupported("nonstructural dynamic result carries reference origins");
        };
        let formal = structural_parameters
            .get(reference.source.parameter_index as usize)
            .ok_or(LoweringError::Unsupported(
                "dynamic result region has no structural input",
            ))?;
        let source_path =
            crate::expression_preparation::bindings::structural_paths::lower_reference_origin_path(
                identities,
                checked,
                &reference.source.path,
                formal.structural_type,
                structural_types,
            )?
            .into_iter()
            .map(|projection| match projection {
                terminal_psi::ReferenceOriginProjection::Exact(segment) => Ok(segment),
                terminal_psi::ReferenceOriginProjection::Element => unsupported(
                    "dynamic result input region requires an exact supplying descriptor",
                ),
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        result_reference_regions.push(terminal_psi::TerminalDynamicReferenceRegion {
            path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                identities, &reference.path, result.structural_type, structural_types,
            )?,
            argument_index: reference.source.parameter_index,
            source_path,
            access: crate::expression_preparation::bindings::structural_paths::terminal_access(reference.source.access),
        });
    }
    result_reference_regions.sort();
    result_reference_regions.dedup();
    Ok(TerminalDynamicSlot {
        slot,
        declaring_trait_identity: identities
            .intern(&checked.symbols.display_path(plan.declaring_trait, "::")),
        requirement_identity: identities.intern(
            &checked
                .typed
                .callable_identity_text(plan.requirement_identity),
        ),
        family_tuple: family_tuple.to_vec(),
        scalar_parameters,
        structural_parameters,
        entry_claim_paths,
        result,
        result_claim_paths,
        result_reference_regions,
    })
}

/// Whether one lowered implementation takes exactly the slot's formals and
/// returns exactly its result.
fn realizes_slot(
    machine: &terminal_psi::TerminalMachine,
    slot: &TerminalDynamicSlot,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
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
    let result_matches = slot.result.matches_machine_result(&machine.result)
        && machine.result.structural().is_none_or(|result| {
            terminal_semantics::reference_result_refines_regions(
                structural_types.iter(),
                &machine.structural_parameters,
                result,
                &slot.result_reference_regions,
            )
        })
        && terminal_semantics::result_claim_paths_match_returns(
            &slot.result_claim_paths,
            machine
                .blocks
                .iter()
                .filter_map(|block| match &block.terminator {
                    terminal_psi::Terminator::ReturnStructural {
                        returned_claims, ..
                    } => Some(returned_claims.as_slice()),
                    _ => None,
                }),
        );
    let Some(mut entry_claim_paths) = machine
        .entry_claims
        .iter()
        .map(|claim| {
            let argument_index = machine
                .structural_parameters
                .iter()
                .position(|parameter| parameter.place == claim.input)?;
            Some(TerminalDynamicClaimPath {
                argument_index: u32::try_from(argument_index).ok()?,
                path: claim.path.clone(),
            })
        })
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    entry_claim_paths.sort();
    scalars_match
        && structurals_match
        && result_matches
        && entry_claim_paths == slot.entry_claim_paths
}

fn unsupported<T>(reason: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(reason))
}
