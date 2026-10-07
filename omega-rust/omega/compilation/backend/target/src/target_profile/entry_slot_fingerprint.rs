//! Entry-slot fingerprint bytes; layout callers keep their own state and
//! domains. Raw tags and length-prefixed payloads remain distinct operations.

use super::{
    ProgramEntryCallingConvention, ProgramEntryPhysicalContractPackage,
    ProgramEntryReceiverProvisioning, ProgramEntrySchema, ProgramEntrySlotDeclaration,
    ProgramEntryVisibleParameters,
};

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

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

pub(crate) struct Fnv1a(u64);

impl Fnv1a {
    pub(crate) const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    pub(crate) fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    pub(crate) const fn finish(self) -> u64 {
        self.0
    }
}
