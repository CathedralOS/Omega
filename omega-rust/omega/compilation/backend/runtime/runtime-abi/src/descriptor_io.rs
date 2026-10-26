//! Exact native descriptor I/O entry identities shared by admission and guards.
//! Symbol text alone is insufficient: the normalized target, library identity,
//! entry mechanism and ABI remain part of the offered implementation.
use target::{ForeignLocatorCandidate, NormalizedForeignLocator, TargetProfile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorOperation {
    Read,
    Write,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorAbi {
    WindowsCrt,
    DarwinLibSystem,
    LinuxLibc,
    LinuxKernel,
}

pub fn descriptor_import(
    locator: &NormalizedForeignLocator,
) -> Option<(DescriptorAbi, DescriptorOperation)> {
    let identity = descriptor_import_identity(locator)?;
    // The finite private-output adapter offers libc write. Libc read
    // requires an input/errno model that it does not yet supply. Recognition
    // remains separate so other execution routes cannot silently run it with
    // a different input capability.
    (identity != (DescriptorAbi::LinuxLibc, DescriptorOperation::Read)).then_some(identity)
}

/// Recognize the exact descriptor operation independently of whether the host
/// adapter offers its environment. This is not permission to execute it.
pub fn descriptor_import_identity(
    locator: &NormalizedForeignLocator,
) -> Option<(DescriptorAbi, DescriptorOperation)> {
    let target = locator.target();
    let abi = match target {
        TargetProfile::MacosArm64 | TargetProfile::MacosX64 => DescriptorAbi::DarwinLibSystem,
        TargetProfile::WindowsX64 => DescriptorAbi::WindowsCrt,
        TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 => DescriptorAbi::LinuxLibc,
        _ => return None,
    };
    for (symbol, operation) in [
        (b"_read".as_slice(), DescriptorOperation::Read),
        (b"_write".as_slice(), DescriptorOperation::Write),
    ] {
        let candidate = match abi {
            DescriptorAbi::DarwinLibSystem => ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                symbol: symbol.to_vec(),
            },
            DescriptorAbi::WindowsCrt => ForeignLocatorCandidate::PeByName {
                library: b"msvcrt.dll".to_vec(),
                export: symbol.to_vec(),
            },
            DescriptorAbi::LinuxLibc => {
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: match operation {
                        DescriptorOperation::Read => b"read".to_vec(),
                        DescriptorOperation::Write => b"write".to_vec(),
                    },
                    version: match target {
                        TargetProfile::LinuxX64 => b"GLIBC_2.2.5".to_vec(),
                        TargetProfile::LinuxArm64 => b"GLIBC_2.17".to_vec(),
                        _ => return None,
                    },
                }
            }
            DescriptorAbi::LinuxKernel => continue,
        };
        if target::normalize_foreign_locator(candidate, target)
            .ok()
            .as_ref()
            == Some(locator)
        {
            return Some((abi, operation));
        }
    }
    None
}

pub fn descriptor_system_call(
    target: TargetProfile,
    number: i64,
) -> Option<(DescriptorAbi, DescriptorOperation)> {
    let operation = match (target, number) {
        (TargetProfile::LinuxX64, 0) | (TargetProfile::LinuxArm64, 63) => DescriptorOperation::Read,
        (TargetProfile::LinuxX64, 1) | (TargetProfile::LinuxArm64, 64) => {
            DescriptorOperation::Write
        }
        _ => return None,
    };
    Some((DescriptorAbi::LinuxKernel, operation))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn libc(
        target: TargetProfile,
        object: &[u8],
        symbol: &[u8],
        version: &[u8],
    ) -> NormalizedForeignLocator {
        target::normalize_foreign_locator(
            ForeignLocatorCandidate::ElfVersioned {
                object: object.to_vec(),
                symbol: symbol.to_vec(),
                version: version.to_vec(),
            },
            target,
        )
        .unwrap()
    }

    #[test]
    fn libc_write_keeps_the_selected_version_and_syscall_contract_distinct() {
        for (target, version, number) in [
            (TargetProfile::LinuxX64, b"GLIBC_2.2.5".as_slice(), 1),
            (TargetProfile::LinuxArm64, b"GLIBC_2.17".as_slice(), 64),
        ] {
            assert_eq!(
                descriptor_import(&libc(target, b"libc.so.6", b"write", version)),
                Some((DescriptorAbi::LinuxLibc, DescriptorOperation::Write))
            );
            assert_eq!(
                descriptor_system_call(target, number),
                Some((DescriptorAbi::LinuxKernel, DescriptorOperation::Write))
            );
            assert_eq!(
                descriptor_import(&libc(target, b"libc.so.6", b"read", version)),
                None
            );
            assert_eq!(
                descriptor_import_identity(&libc(target, b"libc.so.6", b"read", version)),
                Some((DescriptorAbi::LinuxLibc, DescriptorOperation::Read))
            );
        }
    }

    #[test]
    fn a_lookalike_export_or_other_libc_version_is_not_an_offer() {
        for (object, symbol, version) in [
            (
                b"other.so.6".as_slice(),
                b"write".as_slice(),
                b"GLIBC_2.2.5".as_slice(),
            ),
            (
                b"libc.so.6".as_slice(),
                b"_write".as_slice(),
                b"GLIBC_2.2.5".as_slice(),
            ),
            (
                b"libc.so.6".as_slice(),
                b"write".as_slice(),
                b"GLIBC_2.17".as_slice(),
            ),
        ] {
            assert_eq!(
                descriptor_import(&libc(TargetProfile::LinuxX64, object, symbol, version)),
                None
            );
            assert_eq!(
                descriptor_import_identity(&libc(TargetProfile::LinuxX64, object, symbol, version)),
                None
            );
        }
    }
}
