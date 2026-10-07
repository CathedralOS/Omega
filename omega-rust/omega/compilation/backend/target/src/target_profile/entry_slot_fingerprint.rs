//! Entry-slot fingerprint bytes; layout callers keep their own state and
//! domains. Raw tags and length-prefixed payloads remain distinct operations.

pub(crate) use crate::compatibility_fingerprint::Fnv1a;

use super::{
    ProgramEntryCallingConvention, ProgramEntryPhysicalContractPackage,
    ProgramEntryReceiverProvisioning, ProgramEntrySchema, ProgramEntrySlotDeclaration,
    ProgramEntryVisibleParameters,
};

pub(crate) fn hash_entry_slot(hash: &mut Fnv1a, entry_slot: ProgramEntrySlotDeclaration) {
    hash.bytes(entry_slot.owner.target_name().as_bytes());
    hash.bytes(entry_slot.slot_name.as_bytes());
    hash.byte(match entry_slot.schema {
        ProgramEntrySchema::HostedApplication => 1,
        ProgramEntrySchema::ProgramStorageApplication => 2,
    });
    hash.bytes(entry_slot.semantic_arrival_requirement.as_bytes());
    hash_optional_str(hash, entry_slot.physical_arrival_requirement);
    hash.byte(
        entry_slot
            .physical_contract_package
            .map_or(0, |package| match package {
                ProgramEntryPhysicalContractPackage::UefiX64 => 1,
                ProgramEntryPhysicalContractPackage::MacosArm64 => 2,
                ProgramEntryPhysicalContractPackage::MacosX64 => 6,
                ProgramEntryPhysicalContractPackage::LinuxX86_64 => 3,
                ProgramEntryPhysicalContractPackage::LinuxArm64 => 4,
                ProgramEntryPhysicalContractPackage::WindowsX64 => 5,
            }),
    );
    hash_optional_str(hash, entry_slot.boundary_schema);
    hash.byte(
        entry_slot
            .physical_calling_convention
            .map_or(0, |calling| match calling {
                ProgramEntryCallingConvention::MicrosoftX64 => 1,
                ProgramEntryCallingConvention::Aapcs64 => 2,
                ProgramEntryCallingConvention::SystemVAMD64 => 3,
            }),
    );
    hash.byte(
        entry_slot
            .semantic_calling_convention
            .map_or(0, |calling| match calling {
                ProgramEntryCallingConvention::MicrosoftX64 => 1,
                ProgramEntryCallingConvention::Aapcs64 => 2,
                ProgramEntryCallingConvention::SystemVAMD64 => 3,
            }),
    );
    hash.byte(match entry_slot.visible_parameters {
        ProgramEntryVisibleParameters::None => 1,
        ProgramEntryVisibleParameters::ImageAndInitialStorage => 2,
    });
    hash.byte(match entry_slot.receiver {
        ProgramEntryReceiverProvisioning::NoneOrProvisionedZii => 1,
    });
}

fn hash_optional_str(hash: &mut Fnv1a, value: Option<&str>) {
    match value {
        Some(value) => {
            hash.byte(1);
            hash.bytes(value.as_bytes());
        }
        None => hash.byte(0),
    }
}
