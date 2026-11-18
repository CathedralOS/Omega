//! Read a selected carrier as storage after reading the program. Expressions
//! only see the opaque declaration. An owned place or an ABI application
//! needs its bytes and retains the exact selection that supplied them.

use super::{Program, malformed};
use crate::lowering::body::ValueType;
use crate::lowering::read::services::unavailable;
use semantic_vocabulary::{MachineId, StructuralCaseId, StructuralFieldId, StructuralTypeId};
use terminal_check_tables::Checked;
use terminal_psi::{
    BoundaryMachineResult, CallingAbiStorage, LayoutApplicationId, NativeLayoutCatalog,
    NativeLayoutFieldIdentity, StructuralAccess, StructuralParameterDeclaration,
    StructuralRecordStorage, StructuralTypeShape, TerminalMachineResult, TerminalModule,
    TerminalOpaqueRepresentation,
};
use crate::source_state::opaque_representations::OpaqueRepresentationSelection;

#[cfg(test)]
mod tests;

pub(crate) struct OpaqueCarrier<'t> {
    opaque: StructuralTypeId,
    carrier: StructuralTypeId,
    selection: &'t OpaqueRepresentationSelection,
}

impl<'t> Program<'t> {
    pub(crate) fn prepare_opaque_carriers(
        &mut self,
        machine: MachineId,
    ) -> Checked<Vec<OpaqueCarrier<'t>>> {
        let selections = &self.typed.opaque_representation_selections;
        if selections.is_empty() {
            return Ok(Vec::new());
        }
        let key = |s: symbols::SymbolHandle| (s.arena_index(), s.generation());
        let occurrences = self
            .record_sources
            .iter()
            .filter_map(|(opaque, source)| {
                selections
                    .binary_search_by_key(&key(source.declaration), |s| key(s.opaque()))
                    .ok()
                    .map(|index| (opaque, &selections[index]))
            })
            .collect::<Vec<_>>();
        let mut carriers = Vec::with_capacity(occurrences.len());
        for (opaque, selection) in occurrences {
            let ValueType::Record(carrier, _, _) =
                self.record_at(machine, selection.carrier_type())?
            else {
                return Err(malformed(
                    machine,
                    "selected opaque carrier lost its data declaration",
                ));
            };
            if self.source_declaration(machine, carrier)? != selection.carrier()
                || carrier == opaque
            {
                return Err(malformed(
                    machine,
                    "selected opaque carrier has another source owner",
                ));
            }
            carriers.push(OpaqueCarrier {
                opaque,
                carrier,
                selection,
            });
        }
        Ok(carriers)
    }

    pub(crate) fn publish_opaque_carriers<'a>(
        &self,
        machine: MachineId,
        carriers: Vec<OpaqueCarrier<'_>>,
        selected: impl Iterator<Item = &'a CallingAbiStorage>,
        module: &mut TerminalModule,
    ) -> Checked<()> {
        if carriers.is_empty() {
            return Ok(());
        }
        let graph = terminal_semantics::storage::type_graph::StorageTypeGraph::new(
            &module.structural_types,
        )
        .map_err(|why| unavailable(machine, format!("opaque storage declarations: {why:?}")))?;
        let demand = graph
            .storage_reachable_from(by_value_roots(module, selected))
            .map_err(|why| unavailable(machine, format!("opaque storage demand: {why:?}")))?;
        let mut next_field = self.fields_minted;
        let mut next_case = self.cases_minted;
        for OpaqueCarrier {
            opaque,
            carrier,
            selection,
        } in carriers
        {
            if demand.binary_search(&opaque).is_err() {
                continue;
            }
            let carrier_decl = module
                .structural_type(carrier)
                .ok_or_else(|| malformed(machine, "selected carrier lost its storage"))?;
            let carrier_storage_identity = carrier_decl.identity;
            let mut shape = carrier_decl.shape.clone();
            let scalar_restriction = carrier_decl.scalar_restriction.clone();
            let mut fields = Vec::new();
            for field in shape.fields_mut() {
                next_field += 1;
                let id = StructuralFieldId::new(next_field)
                    .ok_or_else(|| malformed(machine, "opaque field identity overflow"))?;
                fields.push((field.id, id));
                field.id = id;
            }
            for case in shape.cases_mut() {
                next_case += 1;
                case.id = StructuralCaseId::new(next_case)
                    .ok_or_else(|| malformed(machine, "opaque case identity overflow"))?;
            }
            if let StructuralTypeShape::Record {
                storage: StructuralRecordStorage::TargetLayout(application),
                ..
            } = &mut shape
            {
                let mut layout = module
                    .target_layout_applications
                    .iter()
                    .find(|row| row.id == *application)
                    .ok_or_else(|| malformed(machine, "opaque carrier lost its target layout"))?
                    .clone();
                let next = module
                    .target_layout_applications
                    .last()
                    .map_or(0, |row| row.id.0)
                    .checked_add(1)
                    .ok_or_else(|| malformed(machine, "opaque layout identity overflow"))?;
                *application = LayoutApplicationId(next);
                layout.id = *application;
                layout.record = opaque;
                fields.sort_unstable_by_key(|pair| pair.0);
                let rebase = |field: StructuralFieldId| {
                    fields
                        .binary_search_by_key(&field, |pair| pair.0)
                        .map(|i| fields[i].1)
                        .map_err(|_| {
                            malformed(machine, "opaque layout field belongs to another record")
                        })
                };
                for realization in &mut layout.realizations {
                    for row in &mut realization.schema_argument.fields {
                        row.record_field = rebase(row.record_field)?;
                    }
                    for field in &mut realization.fields {
                        field.field = rebase(field.field)?;
                    }
                }
                module.target_layout_applications.push(layout);
            }
            let position = module
                .structural_types
                .binary_search_by_key(&opaque, |row| row.id)
                .map_err(|_| malformed(machine, "opaque declaration lost its storage"))?;
            let declaration = &mut module.structural_types[position];
            if declaration.shape != StructuralTypeShape::Opaque {
                return Err(malformed(
                    machine,
                    "representation selects a type with visible contents",
                ));
            }
            declaration.shape = shape;
            declaration.scalar_restriction = scalar_restriction;
            module
                .opaque_representations
                .push(TerminalOpaqueRepresentation {
                    opaque,
                    carrier,
                    opaque_storage_identity: declaration.identity,
                    carrier_storage_identity,
                    application: selection
                        .portable_application(self.typed)
                        .map_err(|why| unavailable(machine, why))?,
                });
        }
        publish_native_layouts(machine, module)
    }
}

fn by_value_roots<'a>(
    module: &TerminalModule,
    selected: impl Iterator<Item = &'a CallingAbiStorage>,
) -> Vec<StructuralTypeId> {
    fn owned(
        parameters: &[StructuralParameterDeclaration],
    ) -> impl Iterator<Item = StructuralTypeId> + '_ {
        parameters
            .iter()
            .filter(|p| p.access == StructuralAccess::Owned)
            .map(|p| p.structural_type)
    }
    fn abi(storage: &CallingAbiStorage) -> impl Iterator<Item = StructuralTypeId> + '_ {
        storage
            .parameters
            .iter()
            .filter(|p| p.access == StructuralAccess::Owned)
            .map(|p| p.structural_type)
            .chain(storage.result)
    }
    let mut roots = Vec::new();
    for machine in &module.machines {
        roots.extend(
            machine
                .structural_places
                .iter()
                .filter_map(|place| place.kind.produced_structural_type()),
        );
        for block in &machine.blocks {
            roots.extend(owned(&block.structural_parameters));
        }
    }
    for contract in module
        .machines
        .iter()
        .map(|machine| &machine.contract)
        .chain(&module.declaration_contracts)
    {
        roots.extend(owned(&contract.structural_parameters));
        if let TerminalMachineResult::Structural(result) = &contract.result {
            roots.push(result.structural_type);
        }
    }
    for boundary in &module.boundary_machines {
        roots.extend(owned(&boundary.structural_parameters));
        if let BoundaryMachineResult::Structural(result) = &boundary.result {
            roots.push(result.structural_type);
        }
    }
    roots.extend(
        module
            .calling_abi_signatures
            .iter()
            .filter_map(|row| row.storage.as_ref().ok())
            .flat_map(abi),
    );
    roots.extend(selected.flat_map(abi));
    roots
}

fn publish_native_layouts(machine: MachineId, module: &mut TerminalModule) -> Checked<()> {
    if module.opaque_representations.is_empty() {
        return Ok(());
    }
    let previous = std::mem::take(&mut module.native_layouts);
    let old = previous.records().collect::<Vec<_>>();
    let mut layouts = NativeLayoutCatalog::default();
    for declaration in &module.structural_types {
        let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
            continue;
        };
        let owner = module
            .opaque_representations
            .binary_search_by_key(&declaration.id, |row| row.opaque)
            .map_or(declaration.id, |i| module.opaque_representations[i].carrier);
        let index = old
            .binary_search_by_key(&owner, |row| row.record)
            .map_err(|_| malformed(machine, "record lost its carrier's native layout"))?;
        let layout = old[index];
        let nominal = previous
            .fields(layout)
            .map_err(|why| unavailable(machine, why))?;
        if nominal.len() != fields.len() {
            return Err(malformed(
                machine,
                "native layout and selected carrier field rosters differ",
            ));
        }
        layouts.insert(
            declaration.id,
            layout.data_identity,
            layout.policy_identity,
            layout.semantic_identity,
            fields
                .iter()
                .zip(nominal)
                .map(|(field, definition)| NativeLayoutFieldIdentity {
                    field: field.id,
                    declaration_identity: definition.declaration_identity,
                }),
            previous
                .realizations(layout)
                .map_err(|why| unavailable(machine, why))?
                .iter()
                .cloned(),
        );
    }
    module.native_layouts = layouts;
    Ok(())
}
