//! Relocate numeric observations of the retained physical-entry residence.
//!
//! Ordinary function placement retains an unresolved object symbol. Physical
//! image preparation alone defines it, after all program storage is placed and
//! before private stack alignment. Private-object composition coalesces this
//! image-wide referent while preserving each occurrence's relocation origin.
//! Nothing here introduces memory access or source storage ownership.

use super::Error;
use machine_code::{ImageAddressFields, PlacedImageAddress};
use object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationOrigin, RelocationPlan, RelocationRecord, SymbolKind,
    SymbolPlan, SymbolSection,
};
use selected_instructions::SelectedImageAddressReference;

fn observes_residence(address: &PlacedImageAddress) -> bool {
    address.reference == SelectedImageAddressReference::RetainedEntryStorageStart
}

pub(super) fn declare(
    addresses: &[PlacedImageAddress],
    object: &mut ObjectPlan,
) -> Result<ObjectSymbolHandle, Error> {
    if addresses.iter().any(observes_residence) {
        if object.target != target::NativeTarget::uefi_x64() {
            return Err(Error::Unsupported(
                "image residence requires the selected UEFI image",
            ));
        }
        declare_symbol(object)
    } else {
        Ok(Default::default())
    }
}

pub(crate) fn declare_symbol(object: &mut ObjectPlan) -> Result<ObjectSymbolHandle, Error> {
    let name = "omega_retained_entry_storage";
    if object
        .layout
        .symbols
        .iter()
        .any(|(_, symbol)| symbol.name == name)
    {
        return Err(Error::Mismatch("image residence symbol collision"));
    }
    Ok(object.layout.symbols.insert(SymbolPlan {
        name: name.into(),
        section: SymbolSection::None,
        offset: 0,
        size: 0,
        kind: SymbolKind::Object,
        import_library: String::new(),
    }))
}

pub(super) fn publish_relocations(
    addresses: &[PlacedImageAddress],
    destination: ObjectSymbolHandle,
    functions: &[crate::ObjectFunction],
    relocations: &mut RelocationPlan,
) -> Result<(), Error> {
    for record in records(addresses, destination, functions)? {
        relocations.push_record(record);
    }
    Ok(())
}

/// The residence observation roster's complete relocation roster: one row per
/// observing address. Publication appends exactly these rows and replay
/// compares the plan against them.
pub(super) fn records(
    addresses: &[PlacedImageAddress],
    destination: ObjectSymbolHandle,
    functions: &[crate::ObjectFunction],
) -> Result<Vec<RelocationRecord>, Error> {
    let mut records = Vec::new();
    for address in addresses
        .iter()
        .filter(|address| observes_residence(address))
    {
        if !destination.is_valid() {
            return Err(Error::Mismatch("image residence has no object referent"));
        }
        let owner = crate::object_artifact::find_object_function(functions, address.machine)
            .ok_or(Error::Mismatch(
                "image residence observation owner is absent",
            ))?;
        let ImageAddressFields::X86_64Relative32 { .. } = address.fields else {
            return Err(Error::Unsupported(
                "image residence requires the UEFI x64 address form",
            ));
        };
        records.extend(super::image_addresses::records(
            address.fields,
            RelocationOrigin::SemanticOperation {
                function_symbol_handle: owner.symbol,
                operation_identity: address.operation.get(),
            },
            destination,
            0,
        )?);
    }
    Ok(records)
}

pub(super) fn validate(
    source: &object_file::StagedOptimizedRelocationFreeObjectContainer,
    artifact: &crate::ObjectArtifact,
) -> Result<(), Error> {
    let addresses = &source.source().text_section().unresolved_image_addresses;
    let present = addresses.iter().any(observes_residence);
    if present != artifact.retained_entry_storage.is_valid() {
        return Err(Error::Mismatch(
            "image residence observation roster changed",
        ));
    }
    if !present {
        return Ok(());
    }
    let symbol = artifact
        .object
        .layout
        .symbols
        .get(artifact.retained_entry_storage);
    if artifact.target != target::NativeTarget::uefi_x64()
        || symbol.section != SymbolSection::None
        || symbol.offset != 0
        || symbol.size != 0
        || symbol.kind != SymbolKind::Object
        || !symbol.import_library.is_empty()
    {
        return Err(Error::Mismatch(
            "image residence was defined before physical placement",
        ));
    }
    let expected = records(
        addresses,
        artifact.retained_entry_storage,
        &artifact.functions,
    )?;
    let actual = super::image_addresses::rows_for_symbols(
        &artifact.relocations,
        [artifact.retained_entry_storage],
    );
    if actual != expected {
        return Err(Error::Mismatch("image residence relocations changed"));
    }
    Ok(())
}
