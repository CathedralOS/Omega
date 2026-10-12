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
    LinuxKernel,
}

pub fn descriptor_import(
    locator: &NormalizedForeignLocator,
) -> Option<(DescriptorAbi, DescriptorOperation)> {
    let target = locator.target();
    let abi = match target {
        TargetProfile::MacosArm64 | TargetProfile::MacosX64 => DescriptorAbi::DarwinLibSystem,
        TargetProfile::WindowsX64 => DescriptorAbi::WindowsCrt,
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
            DescriptorAbi::LinuxKernel => return None,
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
