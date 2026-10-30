//! The exact bundled source document behind each closed physical-entry
//! contract package.
//!
//! One table owns the package-to-document association. `include_bytes!` needs a
//! literal path, so each arm spells its own document; every consumer reads this
//! table rather than a per-package function of its own.

use target::ProgramEntryPhysicalContractPackage;

/// Exact bundled source bytes of one closed contract package.
///
/// `target` owns the package-relative spelling consumers resolve against
/// (`package_relative_source`); this table pins the bytes the compiler itself
/// admitted for that package.
pub(super) fn exact_package_source(package: ProgramEntryPhysicalContractPackage) -> &'static [u8] {
    match package {
        ProgramEntryPhysicalContractPackage::UefiX64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/uefi_x86_64/entry.omg"
        )),
        ProgramEntryPhysicalContractPackage::MacosArm64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/macos_arm64/entry.omg"
        )),
        ProgramEntryPhysicalContractPackage::MacosX64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/macos_x86_64/entry.omg"
        )),
        ProgramEntryPhysicalContractPackage::LinuxX86_64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/linux_x86_64/entry.omg"
        )),
        ProgramEntryPhysicalContractPackage::LinuxArm64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/linux_arm64/entry.omg"
        )),
        ProgramEntryPhysicalContractPackage::WindowsX64 => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/std/targets/windows_x86_64/entry.omg"
        )),
    }
}
