//! Canonical type identities and source commitments for the installed entry packages.

use crate::ProgramEntryPhysicalContractPackage as Package;

pub const WINDOWS_X86_64_U32_TYPE_IDENTITY: &str = "named(name(u32))";
pub const MACOS_ARM64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
pub const MACOS_ARM64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";
pub const MACOS_X86_64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
pub const MACOS_X86_64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";
pub const LINUX_X86_64_ADDRESS_TYPE_IDENTITY: &str = "named(name(addr))";
pub const LINUX_X86_64_I32_TYPE_IDENTITY: &str = "named(name(i32))";
pub const LINUX_ARM64_U64_TYPE_IDENTITY: &str = "named(name(u64))";
pub const LINUX_ARM64_I32_TYPE_IDENTITY: &str = "named(name(i32))";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramEntryPhysicalContractPackageSourceDigest {
    package: Package,
    bytes: [u8; 32],
}

pub fn exact_physical_contract_package_source_digest(
    package: Package,
) -> ProgramEntryPhysicalContractPackageSourceDigest {
    // The receiver compares the published entry to the installed target's
    // identity. The test below derives each identity from its package.
    let digest = match package {
        Package::UefiX64 => [
            0x2d, 0x95, 0x4b, 0xf9, 0xd7, 0x8f, 0x30, 0x20, 0x4b, 0x9c, 0x38, 0x15, 0x6e, 0xf5,
            0x0a, 0x89, 0x6e, 0x31, 0xd1, 0x01, 0x4f, 0x45, 0x48, 0xf0, 0xef, 0x5f, 0x56, 0x9b,
            0x79, 0xac, 0x14, 0xb4,
        ],
        Package::MacosArm64 => [
            0xca, 0x73, 0xec, 0x7b, 0x86, 0x6c, 0xe2, 0x00, 0x17, 0x09, 0x7e, 0x02, 0x75, 0x14,
            0xb5, 0x9a, 0x55, 0x77, 0xc8, 0x5b, 0x43, 0xd1, 0x91, 0x22, 0x6a, 0xc2, 0x9c, 0xb7,
            0x57, 0x83, 0x2a, 0x55,
        ],
        Package::MacosX64 => [
            0xfd, 0x93, 0xab, 0x44, 0xba, 0x48, 0xf1, 0x31, 0xf4, 0xb7, 0xee, 0x6d, 0x10, 0x02,
            0x46, 0xb6, 0xfd, 0x08, 0x0b, 0x4e, 0x41, 0x71, 0xb4, 0xa1, 0x1a, 0x19, 0x62, 0x73,
            0x4a, 0xa2, 0x63, 0x91,
        ],
        Package::LinuxX86_64 => [
            0xc8, 0xbd, 0x64, 0x3e, 0x84, 0xef, 0x79, 0x55, 0x62, 0xe4, 0xa5, 0x9c, 0x79, 0xd9,
            0x21, 0x0b, 0x6d, 0x7a, 0x84, 0x32, 0xc9, 0x26, 0x25, 0xdc, 0x4c, 0xf2, 0xb6, 0xd7,
            0x05, 0xb6, 0xb3, 0x0b,
        ],
        Package::LinuxArm64 => [
            0x8e, 0x77, 0x8f, 0x60, 0x38, 0xe5, 0x15, 0x82, 0x27, 0x0d, 0xe9, 0xf4, 0x83, 0xda,
            0xd2, 0xd5, 0x06, 0xb9, 0x04, 0x55, 0x0d, 0xe9, 0xf1, 0x7f, 0x56, 0x0d, 0x96, 0xaf,
            0x66, 0x0d, 0x50, 0x23,
        ],
        Package::WindowsX64 => [
            0x88, 0x26, 0xfa, 0xc8, 0x97, 0x6d, 0x8b, 0x6b, 0x72, 0xc2, 0x5d, 0xb4, 0x51, 0x64,
            0xc9, 0x43, 0x5a, 0x62, 0x38, 0xe5, 0xad, 0x7d, 0x30, 0x6a, 0x7b, 0xc0, 0x6a, 0x63,
            0x71, 0x01, 0x3c, 0x59,
        ],
    };
    ProgramEntryPhysicalContractPackageSourceDigest::from_retained_digest(package, digest)
}

impl ProgramEntryPhysicalContractPackageSourceDigest {
    pub const fn from_retained_digest(package: Package, bytes: [u8; 32]) -> Self {
        Self { package, bytes }
    }

    pub const fn package(&self) -> Package {
        self.package
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn exact_package_source(package: Package) -> &'static [u8] {
        match package {
            Package::UefiX64 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/std/targets/uefi_x86_64/entry.omg"
            )),
            Package::MacosArm64 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/std/targets/macos_arm64/entry.omg"
            )),
            Package::MacosX64 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/std/targets/macos_x86_64/entry.omg"
            )),
            Package::LinuxX86_64 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/std/targets/linux_x86_64/entry.omg"
            )),
            Package::LinuxArm64 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/std/targets/linux_arm64/entry.omg"
            )),
            Package::WindowsX64 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/std/targets/windows_x86_64/entry.omg"
            )),
        }
    }

    #[test]
    fn target_entry_revisions_match_the_bundled_package_sources() {
        for package in [
            Package::UefiX64,
            Package::MacosArm64,
            Package::MacosX64,
            Package::LinuxX86_64,
            Package::LinuxArm64,
            Package::WindowsX64,
        ] {
            assert_eq!(
                exact_physical_contract_package_source_digest(package),
                ProgramEntryPhysicalContractPackageSourceDigest::from_retained_digest(
                    package,
                    source_provenance::entry_contract_digest(
                        package.manifest_identity(),
                        package.package_relative_source(),
                        exact_package_source(package),
                    ),
                )
            );
        }
    }
}
