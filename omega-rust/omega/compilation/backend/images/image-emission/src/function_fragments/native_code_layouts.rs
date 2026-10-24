//! Final-placement native metadata, with relocations owned by ordinary image
//! construction. The query never recursively compiles Psi or predicts sizes.
use super::{Error, host};
use crate::ObjectFunction;
use machine_code::{ImageAddressFields, PlacedImageAddress};
use machine_emission::code_layout::{NativeCodeLayout, describe_native_code_layout};
use object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
};
use selected_instructions::SelectedImageAddressReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeCodeLayoutPool {
    pub addresses: Vec<PlacedImageAddress>,
    pub layouts: Vec<NativeLayoutObject>,
    pub data_start: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeLayoutObject {
    pub layout: NativeCodeLayout,
    pub symbol: ObjectSymbolHandle,
    pub data_offset: usize,
    pub byte_count: usize,
}

pub(super) fn place(
    source: &machine_emission::StagedOptimizedFixedFrameTextSection,
    addresses: &[PlacedImageAddress],
    object: &mut ObjectPlan,
    data: &mut Vec<u8>,
) -> Result<Option<NativeCodeLayoutPool>, Error> {
    let addresses = addresses
        .iter()
        .filter(|address| {
            matches!(
                address.reference,
                SelectedImageAddressReference::NativeCodeLayout { .. }
            )
        })
        .copied()
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Ok(None);
    }
    let data_start = data.len();
    if !data_start.is_multiple_of(8) {
        return Err(Error::Mismatch("native metadata data alignment changed"));
    }
    let mut layouts: Vec<NativeLayoutObject> = Vec::new();
    for address in &addresses {
        let SelectedImageAddressReference::NativeCodeLayout { machine } = address.reference else {
            unreachable!()
        };
        if layouts.iter().any(|row| row.layout.machine == machine) {
            continue;
        }
        let layout =
            describe_native_code_layout(source, machine).map_err(Error::NativeCodeLayout)?;
        let data_offset = data.len();
        let byte_count = 24usize
            .checked_add(
                layout
                    .regions
                    .len()
                    .checked_mul(16)
                    .ok_or(Error::Overflow)?,
            )
            .ok_or(Error::Overflow)?;
        data.resize(
            data_offset.checked_add(byte_count).ok_or(Error::Overflow)?,
            0,
        );
        data[data_offset + 16..data_offset + 24].copy_from_slice(
            &u64::try_from(layout.regions.len())
                .map_err(|_| Error::Overflow)?
                .to_le_bytes(),
        );
        for (ordinal, region) in layout.regions.iter().enumerate() {
            let offset = data_offset + 24 + ordinal * 16;
            data[offset + 8..offset + 16].copy_from_slice(&region.length.to_le_bytes());
        }
        let name = format!("omega_native_code_layout_{}", machine.get());
        if object
            .layout
            .symbols
            .iter()
            .any(|(_, symbol)| symbol.name == name)
        {
            return Err(Error::Mismatch("native metadata symbol collision"));
        }
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name,
            section: SymbolSection::Section(SectionKind::Data),
            offset: data_offset,
            size: byte_count,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        layouts.push(NativeLayoutObject {
            layout,
            symbol,
            data_offset,
            byte_count,
        });
    }
    let section = object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Data)
        .map(|(handle, _)| handle);
    if let Some(section) = section {
        object.layout.sections.get_mut(section).size = data.len();
    } else {
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Data,
            size: data.len(),
            alignment: 8,
        });
    }
    Ok(Some(NativeCodeLayoutPool {
        addresses,
        layouts,
        data_start,
    }))
}

pub(crate) fn records(
    pool: &NativeCodeLayoutPool,
    functions: &[ObjectFunction],
) -> Result<Vec<RelocationRecord>, Error> {
    let mut rows = Vec::new();
    for descriptor in &pool.layouts {
        let function = functions
            .iter()
            .find(|function| function.machine == descriptor.layout.machine)
            .ok_or(Error::Mismatch(
                "native descriptor exact function is absent",
            ))?;
        let mut data_pointer = |offset, symbol_handle, addend| {
            rows.push(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: descriptor.symbol,
                },
                section: SectionKind::Data,
                offset,
                byte_width: 8,
                symbol_handle,
                addend,
                kind: RelocationKind::Absolute64,
            })
        };
        data_pointer(descriptor.data_offset, function.symbol, 0);
        data_pointer(descriptor.data_offset + 8, descriptor.symbol, 24);
        for (ordinal, region) in descriptor.layout.regions.iter().enumerate() {
            let offset = region
                .section_offset
                .checked_sub(u64::try_from(function.text_offset).map_err(|_| Error::Overflow)?)
                .ok_or(Error::Mismatch(
                    "native region is outside its selected function attribution",
                ))?;
            data_pointer(
                descriptor.data_offset + 24 + ordinal * 16,
                function.symbol,
                i64::try_from(offset).map_err(|_| Error::Overflow)?,
            );
        }
    }
    for address in &pool.addresses {
        let SelectedImageAddressReference::NativeCodeLayout { machine } = address.reference else {
            return Err(Error::Mismatch("native address names another image object"));
        };
        let descriptor = pool
            .layouts
            .iter()
            .find(|row| row.layout.machine == machine)
            .ok_or(Error::Mismatch("native descriptor is absent"))?;
        let function = functions
            .iter()
            .find(|function| function.machine == address.machine)
            .ok_or(Error::Mismatch("native query owner is absent"))?;
        let origin = RelocationOrigin::SemanticOperation {
            function_symbol_handle: function.symbol,
            operation_identity: address.operation.get(),
        };
        let mut pointer = |offset, kind| -> Result<(), Error> {
            rows.push(RelocationRecord {
                origin,
                section: SectionKind::Text,
                offset: host(offset)?,
                byte_width: 4,
                symbol_handle: descriptor.symbol,
                addend: 0,
                kind,
            });
            Ok(())
        };
        match address.fields {
            ImageAddressFields::X86_64Relative32 { patch_offset } => {
                pointer(patch_offset, RelocationKind::X86_64Relative32)?
            }
            ImageAddressFields::Aarch64PageAddress {
                page_patch_offset,
                page_offset_patch_offset,
            } => {
                pointer(page_patch_offset, RelocationKind::Aarch64Page21)?;
                pointer(
                    page_offset_patch_offset,
                    RelocationKind::Aarch64PageOffset12,
                )?;
            }
        }
    }
    Ok(rows)
}

pub(super) fn publish_relocations(
    pool: Option<&NativeCodeLayoutPool>,
    functions: &[ObjectFunction],
    plan: &mut RelocationPlan,
) -> Result<(), Error> {
    if let Some(pool) = pool {
        for row in records(pool, functions)? {
            plan.push_record(row);
        }
    }
    Ok(())
}

/// Replay the finalized attribution and exact relocation roster for retained
/// readers. Source and query identity remain independent of descriptor bytes.
pub(super) fn validate(
    source: &object_file::StagedOptimizedRelocationFreeObjectContainer,
    artifact: &crate::ObjectArtifact,
) -> Result<(), Error> {
    let addresses = source
        .source()
        .text_section()
        .unresolved_image_addresses
        .iter()
        .filter(|row| {
            matches!(
                row.reference,
                SelectedImageAddressReference::NativeCodeLayout { .. }
            )
        })
        .copied()
        .collect::<Vec<_>>();
    let prefix = artifact
        .implementation_table_pool
        .as_ref()
        .map_or(0, |pool| {
            pool.tables
                .last()
                .map_or(0, |row| row.data_offset + row.byte_count)
        });
    let Some(pool) = &artifact.native_code_layout_pool else {
        if !addresses.is_empty() || artifact.data_bytes.len() != prefix {
            return Err(Error::Mismatch(
                "native metadata or initialized-data owner is absent",
            ));
        }
        return Ok(());
    };
    if pool.addresses != addresses || pool.data_start != prefix {
        return Err(Error::Mismatch(
            "native metadata selection or prefix differs from admitted source",
        ));
    }
    for address in &addresses {
        let SelectedImageAddressReference::NativeCodeLayout { machine } = address.reference else {
            unreachable!()
        };
        let (function, _) = super::source::function(source, address.machine)?;
        if function.operations.iter().filter(|operation| matches!(operation, abstract_operations::AbstractOperation::EstablishNativeCodeLayout { psi_operation, machine: target, .. } if *psi_operation == address.operation && *target == machine)).count() != 1 { return Err(Error::Mismatch("native metadata address differs from its exact query")); }
    }
    let mut object = ObjectPlan::with_capacity(artifact.target, 1, pool.layouts.len());
    let mut data = artifact
        .data_bytes
        .get(..prefix)
        .ok_or(Error::Mismatch("native metadata prefix is truncated"))?
        .to_vec();
    let rebuilt = place(source.source(), &addresses, &mut object, &mut data)?
        .ok_or(Error::Mismatch("native metadata pool disappeared"))?;
    if data != artifact.data_bytes || rebuilt.layouts.len() != pool.layouts.len() {
        return Err(Error::Mismatch(
            "native metadata bytes or realization roster changed",
        ));
    }
    for (actual, rebuilt) in pool.layouts.iter().zip(&rebuilt.layouts) {
        if actual.layout != rebuilt.layout
            || actual.data_offset != rebuilt.data_offset
            || actual.byte_count != rebuilt.byte_count
            || !artifact.object.layout.symbols.is_valid(actual.symbol)
            || artifact.object.layout.symbols.get(actual.symbol)
                != object.layout.symbols.get(rebuilt.symbol)
        {
            return Err(Error::Mismatch(
                "native metadata symbol or final instruction attribution changed",
            ));
        }
    }
    let section = artifact
        .object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Data)
        .ok_or(Error::Mismatch("native metadata has no data section"))?
        .1;
    if section.size != data.len() || section.alignment != 8 {
        return Err(Error::Mismatch("native metadata data geometry changed"));
    }
    let actual = super::image_addresses::rows_for_symbols(
        &artifact.relocations,
        pool.layouts.iter().map(|descriptor| descriptor.symbol),
    );
    if actual != records(pool, &artifact.functions)? {
        return Err(Error::Mismatch("native metadata relocation roster changed"));
    }
    Ok(())
}

pub(crate) fn image_data_records(
    namespace: &crate::private_object_composition::ObjectNamespace<'_>,
    root: &crate::ObjectArtifact,
    object: &ObjectPlan,
) -> Result<Vec<RelocationRecord>, Error> {
    let local = namespace.artifact;
    let Some(pool) = &local.native_code_layout_pool else {
        return Ok(Vec::new());
    };
    let replay = local.fragment_replay.as_ref().ok_or(Error::Mismatch(
        "native metadata requires admitted final placement",
    ))?;
    validate(&replay.0, local)?;
    for symbol in pool
        .layouts
        .iter()
        .map(|row| row.symbol)
        .chain(local.functions.iter().map(|function| function.symbol))
    {
        let symbol = namespace.symbol(symbol)?;
        if !object.layout.symbols.is_valid(symbol)
            || object.layout.symbols.get(symbol) != root.object.layout.symbols.get(symbol)
        {
            return Err(Error::Mismatch(
                "image changed native metadata symbol custody",
            ));
        }
    }
    records(pool, &local.functions)?
        .into_iter()
        .filter(|row| row.section == SectionKind::Data)
        .map(|mut row| {
            row.symbol_handle = namespace.symbol(row.symbol_handle)?;
            row.origin = RelocationOrigin::Materialization {
                object_symbol_handle: namespace.symbol(row.origin.symbol_handle())?,
            };
            row.offset = row
                .offset
                .checked_add(namespace.data_offset)
                .ok_or(Error::Overflow)?;
            Ok(row)
        })
        .collect()
}
