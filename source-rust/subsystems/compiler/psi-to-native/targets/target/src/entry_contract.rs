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
            0x70, 0x0d, 0x3a, 0xbf, 0x0d, 0xa1, 0xcc, 0x64, 0x94, 0x65, 0xab, 0x4a, 0x80, 0x5d,
            0x42, 0xda, 0xcf, 0xce, 0x82, 0x77, 0x6f, 0x5f, 0xc8, 0x8a, 0xcd, 0x6b, 0x21, 0x8a,
            0x6b, 0xad, 0xbb, 0x68,
        ],
        Package::MacosArm64 => [
            0x64, 0xde, 0x2f, 0xba, 0xc5, 0x28, 0x25, 0x5c, 0xea, 0x1d, 0x21, 0xac, 0x51, 0x17,
            0x81, 0x6e, 0xad, 0x7b, 0x25, 0xae, 0xa3, 0x3d, 0x88, 0x72, 0x44, 0x89, 0x23, 0x08,
            0x4f, 0xe8, 0x80, 0x66,
        ],
        Package::MacosX64 => [
            0x16, 0xb3, 0x9f, 0x85, 0x70, 0x31, 0x39, 0x04, 0xcf, 0x56, 0xea, 0xb7, 0x6c, 0xd2,
            0x9f, 0x2e, 0x69, 0xc3, 0x1d, 0x7b, 0x91, 0x97, 0x37, 0xb5, 0xf9, 0xff, 0xf9, 0x9d,
            0xe4, 0x9a, 0x33, 0xe3,
        ],
        Package::LinuxX86_64 => [
            0xeb, 0xcf, 0xed, 0xa6, 0x9c, 0x53, 0xd4, 0xb1, 0xa1, 0x65, 0x8c, 0xc2, 0xf2, 0xb2,
            0x3b, 0x2b, 0x8f, 0x57, 0xa3, 0x6b, 0xaa, 0xce, 0x09, 0x5b, 0x73, 0x91, 0xc7, 0x10,
            0x24, 0x55, 0x26, 0x36,
        ],
        Package::LinuxArm64 => [
            0x39, 0x96, 0xe6, 0x14, 0x81, 0xd8, 0xe9, 0x5b, 0xc9, 0xcc, 0x3e, 0x0e, 0x05, 0x21,
            0x53, 0x37, 0x99, 0x21, 0x39, 0x7f, 0x41, 0x50, 0x10, 0xf0, 0xf8, 0x97, 0x16, 0x74,
            0xe8, 0x0c, 0x8e, 0x24,
        ],
        Package::WindowsX64 => [
            0xd5, 0x6c, 0xb1, 0x50, 0x54, 0xbc, 0x32, 0x85, 0x2e, 0x7c, 0x72, 0x40, 0x86, 0xe2,
            0xbd, 0xbe, 0x3b, 0x5d, 0x9a, 0x04, 0x0b, 0xec, 0x53, 0xf7, 0x23, 0xe4, 0x71, 0x2e,
            0x21, 0xd5, 0xc1, 0xcc,
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
