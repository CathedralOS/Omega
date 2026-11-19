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
            0x7f, 0x40, 0xc1, 0x02, 0x2d, 0x63, 0xfe, 0xf3, 0x94, 0xac, 0x9f, 0xd5, 0xfd, 0xd6,
            0xd2, 0xe5, 0xb0, 0x6a, 0x19, 0x4c, 0xd1, 0x26, 0x96, 0x47, 0x99, 0xff, 0x50, 0x90,
            0x46, 0x89, 0x82, 0xde,
        ],
        Package::MacosArm64 => [
            0xd5, 0xb1, 0xc5, 0x6d, 0xe3, 0xad, 0x41, 0xdc, 0x5d, 0x71, 0xb8, 0xf0, 0x4c, 0x74,
            0x2a, 0x78, 0x73, 0x67, 0xbe, 0xc2, 0x4e, 0xb1, 0x9e, 0x1d, 0x4d, 0x27, 0x81, 0x84,
            0xde, 0x68, 0xa8, 0x9a,
        ],
        Package::MacosX64 => [
            0xd6, 0x6b, 0x72, 0xf9, 0xfd, 0xfa, 0xe0, 0x4c, 0xfa, 0x06, 0x7d, 0x41, 0xa0, 0x83,
            0xa5, 0x68, 0x69, 0xb1, 0x4f, 0xa1, 0x78, 0x5d, 0x70, 0xbe, 0xb2, 0x98, 0x39, 0xf3,
            0x1a, 0x06, 0xaf, 0x87,
        ],
        Package::LinuxX86_64 => [
            0x78, 0x69, 0x9a, 0x13, 0xdb, 0x3a, 0xd6, 0x26, 0x34, 0xcd, 0x95, 0xb4, 0x20, 0x01,
            0x19, 0xc4, 0x3b, 0x67, 0x2b, 0xac, 0x89, 0xe3, 0x07, 0x46, 0x9f, 0x0f, 0x02, 0xae,
            0xc0, 0x38, 0xf4, 0xee,
        ],
        Package::LinuxArm64 => [
            0x03, 0x16, 0x52, 0x52, 0x73, 0xb1, 0x83, 0x26, 0xe6, 0xfc, 0x1c, 0xd5, 0x11, 0x65,
            0x2c, 0xa1, 0x45, 0xce, 0x88, 0xb5, 0x05, 0xd0, 0xf7, 0x59, 0xe0, 0x98, 0x29, 0x6c,
            0x35, 0x5d, 0x55, 0x77,
        ],
        Package::WindowsX64 => [
            0x87, 0xd8, 0x74, 0x14, 0x3b, 0x91, 0xe9, 0x09, 0x30, 0xfc, 0x97, 0x20, 0x7b, 0xfb,
            0x36, 0xf9, 0x3b, 0x61, 0x31, 0x56, 0xf1, 0xb8, 0xdb, 0x88, 0xfb, 0xda, 0x09, 0xb5,
            0x66, 0x64, 0x50, 0xc8,
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
                "/../../../../../../source/library/std/uefi_x86_64_entry.omg"
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
