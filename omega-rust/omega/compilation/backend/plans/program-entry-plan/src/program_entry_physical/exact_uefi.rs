const UEFI_X64_TARGET_PACKAGE_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/std/targets/uefi_x86_64/entry.omg"
));

pub fn exact_uefi_x64_physical_contract_package_source_digest()
-> super::ProgramEntryPhysicalContractPackageSourceDigest {
    super::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
        target::ProgramEntryPhysicalContractPackage::UefiX64,
        UEFI_X64_TARGET_PACKAGE_SOURCE,
    )
}
