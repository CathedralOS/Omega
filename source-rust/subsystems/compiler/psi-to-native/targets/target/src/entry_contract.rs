//! Canonical type identities and source commitments for the installed entry packages.

use crate::ProgramEntryPhysicalContractPackage as Package;

pub const WINDOWS_X86_64_U32_TYPE_IDENTITY: &str = "named(name(u32))";
pub const WINDOWS_ARM64_U32_TYPE_IDENTITY: &str = "named(name(u32))";
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
            0xc2, 0x9b, 0x32, 0xc7, 0x91, 0x49, 0xab, 0x82, 0xfc, 0x8a, 0xe8, 0x7a, 0x2f, 0x2c,
            0xd0, 0xa9, 0xd4, 0x6b, 0x01, 0x0f, 0xc5, 0x1d, 0xb2, 0x21, 0x58, 0x5a, 0x2f, 0x1b,
            0x7f, 0xda, 0x09, 0x9b,
        ],
        Package::MacosArm64 => [
            0x81, 0xfc, 0xea, 0xf1, 0x95, 0x5a, 0xdc, 0x36, 0x14, 0x5a, 0x5e, 0xd4, 0x60, 0xe7,
            0xe8, 0xd0, 0x8f, 0xb6, 0xc1, 0xff, 0xc3, 0xfc, 0xb2, 0x59, 0x22, 0x9c, 0x9f, 0x14,
            0x78, 0x18, 0xda, 0x2e,
        ],
        Package::MacosX64 => [
            0xd2, 0x17, 0x84, 0x1a, 0x5e, 0xc8, 0xfc, 0x39, 0xb1, 0x7c, 0xb1, 0x11, 0x67, 0x19,
            0x41, 0x07, 0xc9, 0xb2, 0xd3, 0x6b, 0xa4, 0x30, 0xfc, 0x24, 0x9b, 0xd6, 0x61, 0x01,
            0xd3, 0xde, 0xcc, 0xa2,
        ],
        Package::LinuxX86_64 => [
            0x96, 0x7f, 0xaf, 0x56, 0xad, 0xa8, 0x53, 0x24, 0xf3, 0xcb, 0x0f, 0x1f, 0x26, 0x4a,
            0xc8, 0x80, 0xbe, 0x4f, 0x5b, 0xac, 0xb9, 0x1f, 0xd8, 0x78, 0x6e, 0xcf, 0xe7, 0x09,
            0x2c, 0xc6, 0x28, 0xe9,
        ],
        Package::LinuxArm64 => [
            0xad, 0x88, 0x8e, 0x8f, 0x95, 0x57, 0x83, 0x0e, 0x38, 0xdb, 0xc5, 0xcd, 0x3e, 0xf1,
            0x73, 0x51, 0x33, 0x18, 0xc0, 0x3f, 0xe3, 0xa0, 0xe6, 0xb1, 0x68, 0x36, 0x77, 0x92,
            0x94, 0x2f, 0xa4, 0xe6,
        ],
        Package::WindowsX64 => [
            0x5d, 0x1e, 0x54, 0x2a, 0xb0, 0xcc, 0x18, 0x46, 0x0f, 0xc1, 0xcd, 0x02, 0xcd, 0x46,
            0x3c, 0xea, 0x0a, 0x0d, 0x6a, 0x46, 0xe9, 0xd3, 0x7b, 0x88, 0x02, 0x97, 0x41, 0xdf,
            0x9a, 0x42, 0x2b, 0xcf,
        ],
        Package::WindowsArm64 => [
            0x0b, 0x6c, 0xee, 0xd7, 0x51, 0xdc, 0x07, 0x6e, 0x28, 0x6e, 0x9a, 0x6b, 0xe0, 0xdc,
            0x7b, 0x6a, 0xf9, 0x32, 0x59, 0x56, 0xd1, 0x86, 0x10, 0xcd, 0x1b, 0xc4, 0x04, 0x01,
            0xbd, 0x38, 0x68, 0x3b,
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
            Package::WindowsArm64 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/std/targets/windows_arm64/entry.omg"
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
            Package::WindowsArm64,
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
