use super::ObjectFunctionLocation;
use super::{Error, refuse_symbol_collision};
use crate::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationRecord,
    SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
};
use machine_code::PlacedImageAddress;
use machine_code::code_layout::NativeCodeLayout;
use selected_instructions::SelectedImageAddressReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCodeLayoutPool {
    pub addresses: Vec<PlacedImageAddress>,
    pub layouts: Vec<NativeLayoutObject>,
    pub data_start: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLayoutObject {
    pub layout: NativeCodeLayout,
    pub symbol: ObjectSymbolHandle,
    pub data_offset: usize,
    pub byte_count: usize,
}

pub fn place(
    native_layouts: &[NativeCodeLayout],
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
        let mut matching = native_layouts
            .iter()
            .filter(|layout| layout.machine == machine);
        let layout = matching
            .next()
            .ok_or(Error::Mismatch(
                "native metadata lacks its final code layout",
            ))?
            .clone();
        if matching.next().is_some() {
            return Err(Error::Mismatch(
                "native metadata has competing final code layouts",
            ));
        }
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
        refuse_symbol_collision(object, &name, "native metadata symbol collision")?;
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

pub fn records(
    pool: &NativeCodeLayoutPool,
    functions: &[impl AsRef<ObjectFunctionLocation>],
) -> Result<Vec<RelocationRecord>, Error> {
    let mut rows = Vec::new();
    for descriptor in &pool.layouts {
        let function = functions
            .iter()
            .map(AsRef::as_ref)
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
            .map(AsRef::as_ref)
            .find(|function| function.machine == address.machine)
            .ok_or(Error::Mismatch("native query owner is absent"))?;
        let origin = RelocationOrigin::SemanticOperation {
            function_symbol_handle: function.symbol,
            operation_identity: address.operation.get(),
        };
        rows.extend(super::image_addresses::records(
            address.fields,
            origin,
            descriptor.symbol,
            0,
        )?);
    }
    Ok(rows)
}
