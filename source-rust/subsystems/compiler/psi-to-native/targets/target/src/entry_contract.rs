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
    let (filename, source) = package.contract_source();
    let bytes =
        source_provenance::entry_contract_digest(package.manifest_identity(), filename, source);
    ProgramEntryPhysicalContractPackageSourceDigest { package, bytes }
}

impl ProgramEntryPhysicalContractPackageSourceDigest {
    /// Recover the installed profile's source identity from a published entry.
    pub fn verify(package: Package, published: [u8; 32]) -> Result<Self, String> {
        let expected = exact_physical_contract_package_source_digest(package);
        if published != expected.bytes {
            return Err(format!(
                "physical entry source differs from the designated {} contract",
                package.contract_name()
            ));
        }
        Ok(expected)
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
            let installed = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../../../../source/library/system")
                .join(package.package_relative_source());
            let published = source_provenance::entry_contract_digest(
                package.manifest_identity(),
                package.package_relative_source(),
                &std::fs::read(installed).unwrap(),
            );
            assert_eq!(
                ProgramEntryPhysicalContractPackageSourceDigest::verify(package, published)
                    .unwrap(),
                exact_physical_contract_package_source_digest(package),
            );
            let mut changed = published;
            changed[31] ^= 1;
            assert!(
                ProgramEntryPhysicalContractPackageSourceDigest::verify(package, changed).is_err()
            );
        }
    }
}
