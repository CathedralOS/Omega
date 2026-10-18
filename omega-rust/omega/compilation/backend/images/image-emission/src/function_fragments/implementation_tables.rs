//! Materialize complete selector tables and their exact address relocations.
//!
//! Every canonical row occupies eight bytes, including zero trap rows. The
//! initialized Data section is required by Mach-O's existing PIE pointer
//! rebases. This compiler-owned payload has no source store operation; using
//! initialized data does not promise physically read-only host pages.

use super::{Error, host};
use crate::ObjectFunction;
use machine_code::{ImageAddressFields, PlacedImageAddress};
use object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
};
use terminal_psi::TerminalDynamicDispatchCatalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectImplementationTablePool {
    pub catalog: TerminalDynamicDispatchCatalog,
    pub addresses: Vec<PlacedImageAddress>,
    pub tables: Vec<ObjectImplementationTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectImplementationTable {
    pub table: u32,
    pub symbol: ObjectSymbolHandle,
    pub data_offset: usize,
    pub byte_count: usize,
}

pub(super) fn place(
    catalog: &TerminalDynamicDispatchCatalog,
    addresses: &[PlacedImageAddress],
    object: &mut ObjectPlan,
) -> Result<(Option<ObjectImplementationTablePool>, Vec<u8>), Error> {
    let addresses = addresses
        .iter()
        .filter(|address| address.implementation_table().is_some())
        .copied()
        .collect::<Vec<_>>();
    if catalog.tables.is_empty() {
        if !addresses.is_empty() {
            return Err(Error::Mismatch(
                "implementation addresses have no table catalog",
            ));
        }
        return Ok((None, Vec::new()));
    }
    let mut data = Vec::new();
    let mut tables = Vec::new();
    for (ordinal, table) in catalog.tables.iter().enumerate() {
        let interface = catalog
            .interface(table.interface)
            .ok_or(Error::Mismatch("implementation table interface is absent"))?;
        if usize::try_from(table.id).ok() != Some(ordinal)
            || table.rows.len() != interface.slots.len()
        {
            return Err(Error::Mismatch("implementation table roster changed"));
        }
        let byte_count = table
            .rows
            .len()
            .max(1)
            .checked_mul(8)
            .ok_or(Error::Overflow)?;
        let data_offset = data.len();
        data.resize(
            data_offset.checked_add(byte_count).ok_or(Error::Overflow)?,
            0,
        );
        let name = format!("omega_implementation_table_{}", table.id);
        if object
            .layout
            .symbols
            .iter()
            .any(|(_, symbol)| symbol.name == name)
        {
            return Err(Error::Mismatch("implementation table symbol collision"));
        }
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name,
            section: SymbolSection::Section(SectionKind::Data),
            offset: data_offset,
            size: byte_count,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        tables.push(ObjectImplementationTable {
            table: table.id,
            symbol,
            data_offset,
            byte_count,
        });
    }
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Data,
        size: data.len(),
        alignment: 8,
    });
    Ok((
        Some(ObjectImplementationTablePool {
            catalog: catalog.clone(),
            addresses: addresses.to_vec(),
            tables,
        }),
        data,
    ))
}

pub(super) fn publish_relocations(
    pool: Option<&ObjectImplementationTablePool>,
    functions: &[ObjectFunction],
    relocations: &mut RelocationPlan,
) -> Result<(), Error> {
    if let Some(pool) = pool {
        for record in records(pool, functions)? {
            relocations.push_record(record);
        }
    }
    Ok(())
}

fn records(
    pool: &ObjectImplementationTablePool,
    functions: &[ObjectFunction],
) -> Result<Vec<RelocationRecord>, Error> {
    records_for_symbols(pool, |machine| {
        crate::object_artifact::find_object_function(functions, machine)
            .map(|function| function.symbol)
    })
}

fn records_for_symbols(
    pool: &ObjectImplementationTablePool,
    symbol_for_machine: impl Fn(semantic_vocabulary::MachineId) -> Option<ObjectSymbolHandle>,
) -> Result<Vec<RelocationRecord>, Error> {
    let mut records = Vec::new();
    for table in &pool.tables {
        let rows = &pool
            .catalog
            .table(table.table)
            .ok_or(Error::Mismatch("implementation table is absent"))?
            .rows;
        for (slot, machine) in rows.iter().enumerate() {
            let Some(machine) = machine else {
                continue;
            };
            let symbol = symbol_for_machine(*machine)
                .ok_or(Error::Mismatch("implementation row function is absent"))?;
            records.push(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: table.symbol,
                },
                section: SectionKind::Data,
                offset: table
                    .data_offset
                    .checked_add(slot.checked_mul(8).ok_or(Error::Overflow)?)
                    .ok_or(Error::Overflow)?,
                byte_width: 8,
                symbol_handle: symbol,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }
    }
    for address in &pool.addresses {
        let table = pool
            .tables
            .iter()
            .find(|table| Some(table.table) == address.implementation_table())
            .ok_or(Error::Mismatch(
                "implementation address names an absent table",
            ))?;
        let symbol = symbol_for_machine(address.machine)
            .ok_or(Error::Mismatch("implementation address owner is absent"))?;
        let origin = RelocationOrigin::SemanticOperation {
            function_symbol_handle: symbol,
            operation_identity: address.operation.get(),
        };
        let mut record = |offset, kind| -> Result<(), Error> {
            records.push(RelocationRecord {
                origin,
                section: SectionKind::Text,
                offset: host(offset)?,
                byte_width: 4,
                symbol_handle: table.symbol,
                addend: 0,
                kind,
            });
            Ok(())
        };
        match address.fields {
            ImageAddressFields::X86_64Relative32 { patch_offset } => {
                record(patch_offset, RelocationKind::X86_64Relative32)?
            }
            ImageAddressFields::Aarch64PageAddress {
                page_patch_offset,
                page_offset_patch_offset,
            } => {
                record(page_patch_offset, RelocationKind::Aarch64Page21)?;
                record(
                    page_offset_patch_offset,
                    RelocationKind::Aarch64PageOffset12,
                )?;
            }
        }
    }
    Ok(records)
}

pub(super) fn relocation_count(pool: Option<&ObjectImplementationTablePool>) -> usize {
    pool.map_or(0, |pool| {
        pool.catalog
            .tables
            .iter()
            .map(|table| table.rows.iter().flatten().count())
            .sum::<usize>()
            + pool
                .addresses
                .iter()
                .map(|address| match address.fields {
                    ImageAddressFields::X86_64Relative32 { .. } => 1,
                    ImageAddressFields::Aarch64PageAddress { .. } => 2,
                })
                .sum::<usize>()
    })
}

pub(super) fn validate(
    source: &object_file::StagedOptimizedRelocationFreeObjectContainer,
    artifact: &crate::ObjectArtifact,
) -> Result<(), Error> {
    let plan = source
        .source()
        .source()
        .source()
        .source()
        .optimized_target()
        .optimized()
        .plan();
    let Some(pool) = &artifact.implementation_table_pool else {
        if !artifact.data_bytes.is_empty() || !plan.dynamic_dispatch.tables.is_empty() {
            return Err(Error::Mismatch(
                "initialized data has no implementation table owner",
            ));
        }
        return Ok(());
    };
    if pool.catalog != plan.dynamic_dispatch
        || pool.addresses
            != source
                .source()
                .text_section()
                .unresolved_image_addresses
                .iter()
                .filter(|address| address.implementation_table().is_some())
                .copied()
                .collect::<Vec<_>>()
    {
        return Err(Error::Mismatch(
            "implementation table inputs differ from admitted source",
        ));
    }
    for address in &pool.addresses {
        let (function, _) = super::source::function(source, address.machine)?;
        if function
            .operations
            .iter()
            .filter(|operation| {
                matches!(operation,
            abstract_operations::AbstractOperation::SelectImplementation {psi_operation,table,..}
            if *psi_operation==address.operation && Some(*table)==address.implementation_table())
            })
            .count()
            != 1
        {
            return Err(Error::Mismatch(
                "implementation table address differs from its selection",
            ));
        }
    }
    let mut expected = ObjectPlan::with_capacity(artifact.target, 1, pool.tables.len());
    let (rebuilt, data) = place(&pool.catalog, &pool.addresses, &mut expected)?;
    let rebuilt = rebuilt.ok_or(Error::Mismatch("implementation table pool disappeared"))?;
    if artifact.data_bytes != data || rebuilt.tables.len() != pool.tables.len() {
        return Err(Error::Mismatch("implementation table bytes changed"));
    }
    let data_section = artifact
        .object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Data)
        .ok_or(Error::Mismatch(
            "implementation table data section is absent",
        ))?
        .1;
    if data_section.size != data.len() || data_section.alignment != 8 {
        return Err(Error::Mismatch(
            "implementation table data geometry changed",
        ));
    }
    for (actual, expected_table) in pool.tables.iter().zip(&rebuilt.tables) {
        if actual.table != expected_table.table
            || actual.data_offset != expected_table.data_offset
            || actual.byte_count != expected_table.byte_count
            || !artifact.object.layout.symbols.is_valid(actual.symbol)
            || artifact.object.layout.symbols.get(actual.symbol)
                != expected.layout.symbols.get(expected_table.symbol)
        {
            return Err(Error::Mismatch("implementation table symbol changed"));
        }
    }
    let actual = artifact
        .relocations
        .records()
        .filter(|(_, record)| {
            pool.tables.iter().any(|table| {
                record.symbol_handle == table.symbol
                    || record.origin
                        == RelocationOrigin::Materialization {
                            object_symbol_handle: table.symbol,
                        }
            })
        })
        .map(|(_, record)| record.clone())
        .collect::<Vec<_>>();
    if actual != records(pool, &artifact.functions)? {
        return Err(Error::Mismatch("implementation table relocations changed"));
    }
    Ok(())
}

/// Final-image preparation preserves table symbols and the complete canonical
/// Data relocation roster. Pool presence alone grants no initialized-data
/// authority: bind its catalog, bytes, geometry and slots to retained source.
pub(crate) fn validate_image_data_relocations(
    artifact: &crate::ObjectArtifact,
    object: &ObjectPlan,
    relocations: &RelocationPlan,
) -> Result<(), Error> {
    let mut expected = Vec::new();
    for namespace in crate::private_object_composition::namespaces(artifact) {
        let local = namespace.artifact;
        let Some(pool) = &local.implementation_table_pool else {
            if !local.data_bytes.is_empty() {
                return Err(Error::Mismatch(
                    "initialized image data has no implementation table owner",
                ));
            }
            continue;
        };
        let replay = local.fragment_replay.as_ref().ok_or(Error::Mismatch(
            "implementation table data requires admitted fragment source",
        ))?;
        validate(&replay.0, local)?;
        for symbol in pool
            .tables
            .iter()
            .map(|table| table.symbol)
            .chain(local.functions.iter().map(|function| function.symbol))
        {
            let symbol = namespace.symbol(symbol)?;
            if !object.layout.symbols.is_valid(symbol)
                || object.layout.symbols.get(symbol) != artifact.object.layout.symbols.get(symbol)
            {
                return Err(Error::Mismatch(
                    "image changed implementation table symbol custody",
                ));
            }
        }
        // Replay each local table before embedding its exact row coordinates.
        // The root and callback bodies may reuse machine/table handles; only
        // their retained symbol bindings identify the final-image occurrences.
        for mut record in records(pool, &local.functions)? {
            record.symbol_handle = namespace.symbol(record.symbol_handle)?;
            record.origin = RelocationOrigin::Materialization {
                object_symbol_handle: namespace.symbol(record.origin.symbol_handle())?,
            };
            record.offset = record
                .offset
                .checked_add(namespace.data_offset)
                .ok_or(Error::Overflow)?;
            expected.push(record);
        }
    }
    validate_data_relocation_roster(&expected, relocations)
}

fn validate_data_relocation_roster(
    expected: &[RelocationRecord],
    actual: &RelocationPlan,
) -> Result<(), Error> {
    if !expected
        .iter()
        .filter(|record| record.section == SectionKind::Data)
        .eq(actual
            .records()
            .filter(|(_, record)| record.section == SectionKind::Data)
            .map(|(_, record)| record))
    {
        return Err(Error::Mismatch(
            "initialized image data relocations differ from exact implementation table rows",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{TerminalDynamicInterface, TerminalDynamicSlot, TerminalDynamicTable};

    fn catalog(
        rows: Vec<Option<semantic_vocabulary::MachineId>>,
    ) -> TerminalDynamicDispatchCatalog {
        TerminalDynamicDispatchCatalog {
            interfaces: vec![TerminalDynamicInterface {
                id: 0,
                identity: terminal_psi::TerminalIdentityIndex(1),
                slots: (0..rows.len())
                    .map(|slot| TerminalDynamicSlot {
                        slot: slot as u32,
                        declaring_trait_identity: terminal_psi::TerminalIdentityIndex(1),
                        requirement_identity: terminal_psi::TerminalIdentityIndex(slot as u32),
                        family_tuple: Vec::new(),
                        scalar_parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        entry_claim_paths: Vec::new(),
                        result: terminal_psi::BoundaryMachineResult::Unit,
                        result_claim_paths: Vec::new(),
                        result_reference_regions: Vec::new(),
                    })
                    .collect(),
            }],
            tables: vec![TerminalDynamicTable {
                id: 0,
                interface: 0,
                implementation_identity: terminal_psi::TerminalIdentityIndex(2),
                rows,
            }],
        }
    }

    #[test]
    fn trap_rows_keep_slot_offsets_and_callable_relocations() {
        let machine = semantic_vocabulary::MachineId::new(1).unwrap();
        let catalog = catalog(vec![None, Some(machine), None, Some(machine)]);
        let mut object = ObjectPlan::with_capacity(target::NativeTarget::macos_arm64(), 2, 2);
        let (pool, data) = place(&catalog, &[], &mut object).unwrap();
        let pool = pool.unwrap();
        assert_eq!(data, vec![0; 32]);
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: "function".into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 0,
            size: 4,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        let relocations =
            records_for_symbols(&pool, |selected| (selected == machine).then_some(symbol)).unwrap();
        assert_eq!(
            relocations
                .iter()
                .map(|record| record.offset)
                .collect::<Vec<_>>(),
            [8, 24]
        );
        assert!(
            relocations
                .iter()
                .all(|record| record.symbol_handle == symbol
                    && record.kind == RelocationKind::Absolute64
                    && record.section == SectionKind::Data
                    && record.byte_width == 8)
        );
        assert_eq!(relocation_count(Some(&pool)), 2);
        let relocation_plan = |rows: &[RelocationRecord]| {
            let mut plan = RelocationPlan::with_target(object.target);
            for row in rows {
                plan.push_record(row.clone());
            }
            plan
        };
        assert!(
            validate_data_relocation_roster(&relocations, &relocation_plan(&relocations),).is_ok()
        );
        // A trap row cannot acquire a pointer, and a callable row cannot name
        // another symbol or disappear during final-image preparation.
        let mut extra = relocations.clone();
        let mut trap = relocations[0].clone();
        trap.offset = 0;
        extra.push(trap);
        assert!(validate_data_relocation_roster(&relocations, &relocation_plan(&extra)).is_err());
        let mut substituted = relocations.clone();
        substituted[0].symbol_handle = pool.tables[0].symbol;
        assert!(
            validate_data_relocation_roster(&relocations, &relocation_plan(&substituted),).is_err()
        );
        assert!(
            validate_data_relocation_roster(&relocations, &relocation_plan(&relocations[..1]),)
                .is_err()
        );
        assert!(validate_data_relocation_roster(&[], &relocation_plan(&relocations)).is_err());
    }

    #[test]
    fn empty_tables_keep_distinct_nonzero_storage() {
        let mut catalog = catalog(Vec::new());
        catalog.tables.push(TerminalDynamicTable {
            id: 1,
            interface: 0,
            implementation_identity: terminal_psi::TerminalIdentityIndex(3),
            rows: Vec::new(),
        });
        let mut object = ObjectPlan::with_capacity(target::NativeTarget::macos_arm64(), 1, 2);
        let (pool, data) = place(&catalog, &[], &mut object).unwrap();
        let pool = pool.unwrap();
        assert_eq!(data, vec![0; 16]);
        assert_eq!(
            pool.tables
                .iter()
                .map(|table| table.data_offset)
                .collect::<Vec<_>>(),
            [0, 8]
        );
        assert_ne!(pool.tables[0].symbol, pool.tables[1].symbol);
        assert!(records_for_symbols(&pool, |_| None).unwrap().is_empty());
    }
}
