use super::{Error, ObjectFunctionLocation, find_object_function, refuse_symbol_collision};
use crate::{
    ObjectPlan, ObjectSymbolHandle, RelocationOrigin, RelocationRecord, SymbolKind, SymbolPlan,
    SymbolSection,
};
use machine_code::{ImageAddressFields, PlacedImageAddress};
use selected_instructions::SelectedImageAddressReference;
pub fn observes_residence(address: &PlacedImageAddress) -> bool {
    address.reference == SelectedImageAddressReference::RetainedEntryStorageStart
}

pub(super) fn declare(
    addresses: &[PlacedImageAddress],
    object: &mut ObjectPlan,
    profile: Option<target::TargetProfile>,
) -> Result<ObjectSymbolHandle, Error> {
    if addresses.iter().any(observes_residence) {
        if profile != Some(target::TargetProfile::UefiX64)
            || profile.and_then(|profile| profile.native_realization()) != Some(object.target)
        {
            return Err(Error::Unsupported(
                "image residence requires the selected UEFI image",
            ));
        }
        declare_symbol(object)
    } else {
        Ok(Default::default())
    }
}

pub fn declare_symbol(object: &mut ObjectPlan) -> Result<ObjectSymbolHandle, Error> {
    let name = "omega_retained_entry_storage";
    refuse_symbol_collision(object, name, "image residence symbol collision")?;
    Ok(object.layout.symbols.insert(SymbolPlan {
        name: name.into(),
        section: SymbolSection::None,
        offset: 0,
        size: 0,
        kind: SymbolKind::Object,
    }))
}

pub fn records(
    addresses: &[PlacedImageAddress],
    destination: ObjectSymbolHandle,
    functions: &[impl AsRef<ObjectFunctionLocation>],
) -> Result<Vec<RelocationRecord>, Error> {
    let mut records = Vec::new();
    for address in addresses
        .iter()
        .filter(|address| observes_residence(address))
    {
        if !destination.is_valid() {
            return Err(Error::Mismatch("image residence has no object referent"));
        }
        let owner = find_object_function(functions, address.machine).ok_or(Error::Mismatch(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_residence_requires_selected_firmware_not_matching_windows_geometry() {
        let addresses = [PlacedImageAddress {
            machine: semantic_vocabulary::MachineId::new(1).unwrap(),
            block: selected_instructions::SelectedBlockId(0),
            instruction: selected_instructions::SelectedInstructionId(0),
            reference: SelectedImageAddressReference::RetainedEntryStorageStart,
            operation: semantic_vocabulary::OperationId::new(1).unwrap(),
            instruction_function_offset: 0,
            instruction_section_offset: 0,
            instruction_byte_count: 7,
            fields: ImageAddressFields::X86_64Relative32 { patch_offset: 3 },
        }];
        assert_eq!(
            target::NativeTarget::windows_x64(),
            target::NativeTarget::uefi_x64()
        );
        for profile in [
            Some(target::TargetProfile::WindowsX64),
            None,
            Some(target::TargetProfile::UefiX64),
        ] {
            let mut object = ObjectPlan::with_capacity(target::NativeTarget::windows_x64(), 0, 1);
            let result = declare(&addresses, &mut object, profile);
            assert_eq!(
                result.is_ok(),
                profile == Some(target::TargetProfile::UefiX64)
            );
            match result {
                Ok(symbol) => assert!(symbol.is_valid()),
                Err(error) => assert!(matches!(
                    error,
                    Error::Unsupported("image residence requires the selected UEFI image")
                )),
            }
        }
        let mut incompatible = ObjectPlan::with_capacity(target::NativeTarget::linux_arm64(), 0, 1);
        assert!(
            declare(
                &addresses,
                &mut incompatible,
                Some(target::TargetProfile::UefiX64)
            )
            .is_err()
        );
        assert!(!declare(&[], &mut incompatible, None).unwrap().is_valid());
    }
}
