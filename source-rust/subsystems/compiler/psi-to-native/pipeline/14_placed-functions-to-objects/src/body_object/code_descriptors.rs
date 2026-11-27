use super::ObjectFunctionLocation;
use super::{Error, refuse_symbol_collision};
use crate::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationRecord,
    SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
};
use machine_code::PlacedImageAddress;
use terminal_psi::TerminalDynamicDispatchCatalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectCodeDescriptorPool {
    pub catalogs: target_vocabulary::NativeCodeCatalogs,
    pub addresses: Vec<PlacedImageAddress>,
    pub descriptors: Vec<ObjectCodeDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectCodeDescriptor {
    pub source: CodeDescriptorSource,
    pub symbol: ObjectSymbolHandle,
    pub data_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeDescriptorSource {
    ImplementationTable(u32),
    Machine(semantic_vocabulary::MachineDescriptorId),
}

pub fn address_source(address: &PlacedImageAddress) -> Option<CodeDescriptorSource> {
    address
        .implementation_table()
        .map(CodeDescriptorSource::ImplementationTable)
        .or_else(|| {
            address
                .machine_descriptor()
                .map(CodeDescriptorSource::Machine)
        })
}

pub fn target_entries<'catalog>(
    catalog: &'catalog TerminalDynamicDispatchCatalog,
    callables: &'catalog terminal_psi::TerminalCallableCatalog,
    source: CodeDescriptorSource,
) -> Result<impl Iterator<Item = Option<semantic_vocabulary::MachineId>> + 'catalog, Error> {
    let (rows, single) = match source {
        CodeDescriptorSource::ImplementationTable(table) => {
            let table = catalog
                .table(table)
                .filter(|row| row.id == table)
                .ok_or(Error::Mismatch("code descriptor table is absent"))?;
            let interface = catalog
                .interface(table.interface)
                .ok_or(Error::Mismatch("code descriptor table interface is absent"))?;
            if table.rows.len() != interface.slots.len() {
                return Err(Error::Mismatch("code descriptor table call shape changed"));
            }
            (
                Some(table.rows.as_slice()),
                table.rows.is_empty().then_some(None),
            )
        }
        CodeDescriptorSource::Machine(descriptor) => {
            let selected = callables
                .descriptor(descriptor)
                .ok_or(Error::Mismatch("machine descriptor is absent"))?;
            if callables.contract(selected.contract).is_none() {
                return Err(Error::Mismatch(
                    "machine descriptor has no retained contract",
                ));
            }
            (None, Some(Some(selected.machine)))
        }
    };
    Ok(rows.into_iter().flatten().copied().chain(single))
}

pub fn place(
    catalogs: &target_vocabulary::NativeCodeCatalogs,
    addresses: &[PlacedImageAddress],
    object: &mut ObjectPlan,
) -> Result<(Option<ObjectCodeDescriptorPool>, Vec<u8>), Error> {
    let catalog = catalogs.dynamic_dispatch();
    let callables = catalogs.callables();
    let addresses = addresses
        .iter()
        .filter(|address| address_source(address).is_some())
        .copied()
        .collect::<Vec<_>>();
    let mut selected = addresses
        .iter()
        .filter_map(|address| address.machine_descriptor())
        .collect::<Vec<_>>();
    selected.sort();
    selected.dedup();
    let sources = catalog
        .tables
        .iter()
        .map(|table| CodeDescriptorSource::ImplementationTable(table.id))
        .chain(selected.into_iter().map(CodeDescriptorSource::Machine))
        .collect::<Vec<_>>();
    if sources.is_empty() {
        if !addresses.is_empty() {
            return Err(Error::Mismatch(
                "code descriptor addresses have no retained catalog",
            ));
        }
        return Ok((None, Vec::new()));
    }
    let mut data = Vec::new();
    let mut descriptors = Vec::new();
    for source in sources {
        let byte_count = target_entries(catalog, callables, source)?
            .count()
            .checked_mul(8)
            .ok_or(Error::Overflow)?;
        let data_offset = data.len();
        data.resize(
            data_offset.checked_add(byte_count).ok_or(Error::Overflow)?,
            0,
        );
        let name = match source {
            CodeDescriptorSource::ImplementationTable(table) => {
                format!("omega_implementation_table_{table}")
            }
            CodeDescriptorSource::Machine(descriptor) => {
                format!("omega_machine_descriptor_{}", descriptor.get())
            }
        };
        refuse_symbol_collision(object, &name, "code descriptor symbol collision")?;
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name,
            section: SymbolSection::Section(SectionKind::Data),
            offset: data_offset,
            size: byte_count,
            kind: SymbolKind::Object,
        });
        descriptors.push(ObjectCodeDescriptor {
            source,
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
        Some(ObjectCodeDescriptorPool {
            catalogs: catalogs.clone(),
            addresses,
            descriptors,
        }),
        data,
    ))
}

pub fn records(
    pool: &ObjectCodeDescriptorPool,
    functions: &[impl AsRef<ObjectFunctionLocation>],
) -> Result<Vec<RelocationRecord>, Error> {
    records_for_symbols(pool, |machine| {
        super::find_object_function(functions, machine).map(|function| function.symbol)
    })
}

pub fn records_for_symbols(
    pool: &ObjectCodeDescriptorPool,
    symbol_for_machine: impl Fn(semantic_vocabulary::MachineId) -> Option<ObjectSymbolHandle>,
) -> Result<Vec<RelocationRecord>, Error> {
    let mut records = Vec::new();
    for descriptor in &pool.descriptors {
        for (slot, machine) in target_entries(
            pool.catalogs.dynamic_dispatch(),
            pool.catalogs.callables(),
            descriptor.source,
        )?
        .enumerate()
        {
            let Some(machine) = machine else {
                continue;
            };
            let symbol = symbol_for_machine(machine)
                .ok_or(Error::Mismatch("code descriptor target function is absent"))?;
            records.push(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: descriptor.symbol,
                },
                section: SectionKind::Data,
                offset: descriptor
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
            .descriptors
            .iter()
            .find(|descriptor| Some(descriptor.source) == address_source(address))
            .ok_or(Error::Mismatch(
                "code descriptor address names an absent cell",
            ))?;
        let symbol = symbol_for_machine(address.machine)
            .ok_or(Error::Mismatch("code descriptor address owner is absent"))?;
        let origin = RelocationOrigin::SemanticOperation {
            function_symbol_handle: symbol,
            operation_identity: address.operation.get(),
        };
        records.extend(super::image_addresses::records(
            address.fields,
            origin,
            table.symbol,
            0,
        )?);
    }
    Ok(records)
}
