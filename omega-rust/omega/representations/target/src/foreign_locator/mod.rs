//! Normalized physical locators for evaluated foreign bindings.
//!
//! A locator is ordinary evaluated data, not binding authority. Validation
//! only seals its atomic coordinates and selected-target applicability so a
//! later provider-plan join does not reconstruct or independently pair raw
//! object, symbol, version, or ordinal values.

use crate::TargetProfile;
use sha2::{Digest, Sha256};
use std::fmt;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Untrusted owned coordinates produced by future ordinary `Binding` value
/// evaluation. Vector lengths retain the fixed-array widths of that value;
/// unused coordinates are absent by construction rather than normalized from
/// an independently supplied field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignLocatorCandidate {
    PeByName {
        library: Vec<u8>,
        export: Vec<u8>,
    },
    PeByOrdinal {
        library: Vec<u8>,
        ordinal: u16,
    },
    ElfVersioned {
        object: Vec<u8>,
        symbol: Vec<u8>,
        version: Vec<u8>,
    },
    /// One exact two-level-namespace Mach-O import. `install_name` is the raw
    /// `LC_LOAD_DYLIB` identity; `symbol` is the raw dyld symbol spelling. The
    /// image-local dylib ordinal is derived from load-command order and is not
    /// locator identity.
    MachODylibSymbol {
        install_name: Vec<u8>,
        symbol: Vec<u8>,
    },
}

impl ForeignLocatorCandidate {
    const fn case_name(&self) -> &'static str {
        match self {
            Self::PeByName { .. } => "PeByName",
            Self::PeByOrdinal { .. } => "PeByOrdinal",
            Self::ElfVersioned { .. } => "ElfVersioned",
            Self::MachODylibSymbol { .. } => "MachODylibSymbol",
        }
    }
}

/// Read-only, structurally validated locator for one exact selected target.
///
/// Construction grants no provider authority and performs no source
/// evaluation. The complete candidate remains private so its coordinates
/// cannot drift after its normalized identity has been derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedForeignLocator {
    target: TargetProfile,
    locator: ForeignLocatorCandidate,
    non_authoritative_compatibility_fingerprint: u64,
    identity_digest: ForeignLocatorIdentityDigest,
}

/// Collision-resistant identity of one complete normalized physical binding.
/// Import locators and target-scoped syscall values share this dependency-light
/// receipt join; it grants no provider, loader, or execution authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ForeignLocatorIdentityDigest([u8; 32]);

impl ForeignLocatorIdentityDigest {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Collision-resistant identity of one target-scoped ordinary syscall value.
/// This uses the existing locator digest carrier only as the dependency-light
/// binding-receipt join; it grants no provider or execution authority.
#[doc(hidden)]
pub fn evaluated_syscall_identity_digest(
    target: TargetProfile,
    number: u32,
) -> ForeignLocatorIdentityDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.evaluated-syscall-binding.sha256.v1\0");
    hash_bytes(&mut digest, target.target_name().as_bytes());
    digest.update(number.to_le_bytes());
    ForeignLocatorIdentityDigest(digest.finalize().into())
}

impl NormalizedForeignLocator {
    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub const fn locator(&self) -> &ForeignLocatorCandidate {
        &self.locator
    }

    /// Compatibility fingerprint of the exact case, target, coordinate
    /// lengths, coordinate bytes, and ordinal. This is deterministic artifact
    /// identity, not collision-resistant admission evidence.
    pub const fn non_authoritative_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_compatibility_fingerprint
    }

    pub const fn identity_digest(&self) -> ForeignLocatorIdentityDigest {
        self.identity_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignLocatorValidationError {
    UnsupportedTarget {
        locator_case: &'static str,
        target: TargetProfile,
    },
    EmptyCoordinate {
        locator_case: &'static str,
        coordinate: &'static str,
    },
    ForbiddenNul {
        locator_case: &'static str,
        coordinate: &'static str,
        offset: usize,
    },
    ZeroPeOrdinal,
}

impl fmt::Display for ForeignLocatorValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTarget {
                locator_case,
                target,
            } => write!(
                formatter,
                "foreign locator {locator_case} is not applicable to selected target `{}`",
                target.target_name(),
            ),
            Self::EmptyCoordinate {
                locator_case,
                coordinate,
            } => write!(
                formatter,
                "foreign locator {locator_case} has an empty {coordinate} coordinate",
            ),
            Self::ForbiddenNul {
                locator_case,
                coordinate,
                offset,
            } => write!(
                formatter,
                "foreign locator {locator_case} {coordinate} coordinate contains a forbidden NUL byte at offset {offset}",
            ),
            Self::ZeroPeOrdinal => write!(
                formatter,
                "foreign locator PeByOrdinal uses reserved zero ordinal",
            ),
        }
    }
}

impl std::error::Error for ForeignLocatorValidationError {}

/// Validate and seal one evaluated locator against its exact selected target.
///
/// PE import locators are hosted Windows data; sharing COFF with the UEFI
/// image format does not make an OS import applicable there. Versioned ELF
/// locators are admitted for the two Linux profiles. Mach-O dylib-symbol
/// locators are admitted only for macOS AArch64. Target-opaque profiles require
/// distinct future cases rather than reinterpretation.
pub fn normalize_foreign_locator(
    locator: ForeignLocatorCandidate,
    target: TargetProfile,
) -> Result<NormalizedForeignLocator, ForeignLocatorValidationError> {
    validate_target(&locator, target)?;
    validate_coordinates(&locator)?;
    let non_authoritative_compatibility_fingerprint =
        non_authoritative_compatibility_fingerprint(&locator, target);
    let identity_digest = identity_digest(&locator, target);
    Ok(NormalizedForeignLocator {
        target,
        locator,
        non_authoritative_compatibility_fingerprint,
        identity_digest,
    })
}

fn validate_target(
    locator: &ForeignLocatorCandidate,
    target: TargetProfile,
) -> Result<(), ForeignLocatorValidationError> {
    let applicable = match locator {
        ForeignLocatorCandidate::PeByName { .. } | ForeignLocatorCandidate::PeByOrdinal { .. } => {
            target == TargetProfile::WindowsX64
        }
        ForeignLocatorCandidate::ElfVersioned { .. } => {
            matches!(target, TargetProfile::LinuxArm64 | TargetProfile::LinuxX64)
        }
        ForeignLocatorCandidate::MachODylibSymbol { .. } => target == TargetProfile::MacosArm64,
    };
    if !applicable {
        return Err(ForeignLocatorValidationError::UnsupportedTarget {
            locator_case: locator.case_name(),
            target,
        });
    }
    Ok(())
}

fn validate_coordinates(
    locator: &ForeignLocatorCandidate,
) -> Result<(), ForeignLocatorValidationError> {
    match locator {
        ForeignLocatorCandidate::PeByName { library, export } => {
            validate_coordinate(locator.case_name(), "library", library)?;
            validate_coordinate(locator.case_name(), "export", export)
        }
        ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
            validate_coordinate(locator.case_name(), "library", library)?;
            if *ordinal == 0 {
                return Err(ForeignLocatorValidationError::ZeroPeOrdinal);
            }
            Ok(())
        }
        ForeignLocatorCandidate::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            validate_coordinate(locator.case_name(), "object", object)?;
            validate_coordinate(locator.case_name(), "symbol", symbol)?;
            validate_coordinate(locator.case_name(), "version", version)
        }
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            validate_coordinate(locator.case_name(), "install_name", install_name)?;
            validate_coordinate(locator.case_name(), "symbol", symbol)
        }
    }
}

fn validate_coordinate(
    locator_case: &'static str,
    coordinate: &'static str,
    bytes: &[u8],
) -> Result<(), ForeignLocatorValidationError> {
    if bytes.is_empty() {
        return Err(ForeignLocatorValidationError::EmptyCoordinate {
            locator_case,
            coordinate,
        });
    }
    if let Some(offset) = bytes.iter().position(|byte| *byte == 0) {
        return Err(ForeignLocatorValidationError::ForbiddenNul {
            locator_case,
            coordinate,
            offset,
        });
    }
    Ok(())
}

fn non_authoritative_compatibility_fingerprint(
    locator: &ForeignLocatorCandidate,
    target: TargetProfile,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.foreign-locator.v1");
    hash.bytes(target.target_name().as_bytes());
    match locator {
        ForeignLocatorCandidate::PeByName { library, export } => {
            hash.byte(1);
            hash.bytes(library);
            hash.bytes(export);
        }
        ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
            hash.byte(2);
            hash.bytes(library);
            hash.bytes(&ordinal.to_le_bytes());
        }
        ForeignLocatorCandidate::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            hash.byte(3);
            hash.bytes(object);
            hash.bytes(symbol);
            hash.bytes(version);
        }
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            hash.byte(4);
            hash.bytes(install_name);
            hash.bytes(symbol);
        }
    }
    hash.finish()
}

fn identity_digest(
    locator: &ForeignLocatorCandidate,
    target: TargetProfile,
) -> ForeignLocatorIdentityDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.foreign-locator.sha256.v1\0");
    hash_bytes(&mut digest, target.target_name().as_bytes());
    match locator {
        ForeignLocatorCandidate::PeByName { library, export } => {
            digest.update([1]);
            hash_bytes(&mut digest, library);
            hash_bytes(&mut digest, export);
        }
        ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
            digest.update([2]);
            hash_bytes(&mut digest, library);
            hash_bytes(&mut digest, &ordinal.to_le_bytes());
        }
        ForeignLocatorCandidate::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            digest.update([3]);
            hash_bytes(&mut digest, object);
            hash_bytes(&mut digest, symbol);
            hash_bytes(&mut digest, version);
        }
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            digest.update([4]);
            hash_bytes(&mut digest, install_name);
            hash_bytes(&mut digest, symbol);
        }
    }
    ForeignLocatorIdentityDigest(digest.finalize().into())
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("normalized foreign-locator coordinate length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pe_name() -> ForeignLocatorCandidate {
        ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"WriteFile".to_vec(),
        }
    }

    fn pe_ordinal() -> ForeignLocatorCandidate {
        ForeignLocatorCandidate::PeByOrdinal {
            library: b"user32.dll".to_vec(),
            ordinal: 17,
        }
    }

    fn elf_versioned() -> ForeignLocatorCandidate {
        ForeignLocatorCandidate::ElfVersioned {
            object: b"libc.so.6".to_vec(),
            symbol: b"memcpy".to_vec(),
            version: b"GLIBC_2.14".to_vec(),
        }
    }

    fn macho_dylib_symbol() -> ForeignLocatorCandidate {
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
            symbol: b"_write".to_vec(),
        }
    }

    #[test]
    fn every_settled_locator_case_seals_exact_owned_coordinates() {
        let cases = [
            (pe_name(), TargetProfile::WindowsX64),
            (pe_ordinal(), TargetProfile::WindowsX64),
            (elf_versioned(), TargetProfile::LinuxX64),
            (macho_dylib_symbol(), TargetProfile::MacosArm64),
        ];
        for (candidate, target) in cases {
            let expected = candidate.clone();
            let normalized = normalize_foreign_locator(candidate, target).expect("valid locator");
            assert_eq!(normalized.target(), target);
            assert_eq!(normalized.locator(), &expected);
            assert_ne!(normalized.non_authoritative_compatibility_fingerprint(), 0);
        }
        assert_eq!(
            normalize_foreign_locator(pe_name(), TargetProfile::WindowsX64)
                .unwrap()
                .non_authoritative_compatibility_fingerprint(),
            0xd6c3_45dc_fd2c_fba0,
            "the canonical PeByName fingerprint is stable",
        );
    }

    #[test]
    fn evaluated_syscall_identity_binds_exact_target_and_number() {
        let baseline = evaluated_syscall_identity_digest(TargetProfile::LinuxX64, 60);
        assert_ne!(
            baseline,
            evaluated_syscall_identity_digest(TargetProfile::LinuxX64, 61)
        );
        assert_ne!(
            baseline,
            evaluated_syscall_identity_digest(TargetProfile::LinuxArm64, 60)
        );
    }

    #[test]
    fn applicability_is_exact_and_does_not_treat_all_coff_as_pe_import_hosts() {
        for target in [
            TargetProfile::LinuxArm64,
            TargetProfile::LinuxX64,
            TargetProfile::MacosArm64,
            TargetProfile::UefiX64,
            TargetProfile::CrossPlatformCli,
            TargetProfile::LocalUnchecked,
        ] {
            assert!(matches!(
                normalize_foreign_locator(pe_name(), target),
                Err(ForeignLocatorValidationError::UnsupportedTarget {
                    locator_case: "PeByName",
                    target: rejected,
                }) if rejected == target
            ));
        }
        for target in [
            TargetProfile::MacosArm64,
            TargetProfile::WindowsX64,
            TargetProfile::UefiX64,
            TargetProfile::CrossPlatformCli,
            TargetProfile::LocalUnchecked,
        ] {
            assert!(matches!(
                normalize_foreign_locator(elf_versioned(), target),
                Err(ForeignLocatorValidationError::UnsupportedTarget {
                    locator_case: "ElfVersioned",
                    target: rejected,
                }) if rejected == target
            ));
        }
        normalize_foreign_locator(elf_versioned(), TargetProfile::LinuxArm64)
            .expect("versioned ELF is applicable to Linux AArch64");
        for target in [
            TargetProfile::LinuxArm64,
            TargetProfile::LinuxX64,
            TargetProfile::WindowsX64,
            TargetProfile::UefiX64,
            TargetProfile::CrossPlatformCli,
            TargetProfile::LocalUnchecked,
        ] {
            assert!(matches!(
                normalize_foreign_locator(macho_dylib_symbol(), target),
                Err(ForeignLocatorValidationError::UnsupportedTarget {
                    locator_case: "MachODylibSymbol",
                    target: rejected,
                }) if rejected == target
            ));
        }
        normalize_foreign_locator(macho_dylib_symbol(), TargetProfile::MacosArm64)
            .expect("Mach-O dylib-symbol locator is applicable to macOS AArch64");
    }

    #[test]
    fn every_required_coordinate_rejects_empty_or_nul_bytes() {
        let empty_cases = [
            (
                ForeignLocatorCandidate::PeByName {
                    library: Vec::new(),
                    export: b"WriteFile".to_vec(),
                },
                TargetProfile::WindowsX64,
                "library",
            ),
            (
                ForeignLocatorCandidate::PeByName {
                    library: b"kernel32.dll".to_vec(),
                    export: Vec::new(),
                },
                TargetProfile::WindowsX64,
                "export",
            ),
            (
                ForeignLocatorCandidate::PeByOrdinal {
                    library: Vec::new(),
                    ordinal: 17,
                },
                TargetProfile::WindowsX64,
                "library",
            ),
            (
                ForeignLocatorCandidate::ElfVersioned {
                    object: Vec::new(),
                    symbol: b"memcpy".to_vec(),
                    version: b"GLIBC_2.14".to_vec(),
                },
                TargetProfile::LinuxX64,
                "object",
            ),
            (
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: Vec::new(),
                    version: b"GLIBC_2.14".to_vec(),
                },
                TargetProfile::LinuxX64,
                "symbol",
            ),
            (
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: b"memcpy".to_vec(),
                    version: Vec::new(),
                },
                TargetProfile::LinuxX64,
                "version",
            ),
            (
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: Vec::new(),
                    symbol: b"_write".to_vec(),
                },
                TargetProfile::MacosArm64,
                "install_name",
            ),
            (
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                    symbol: Vec::new(),
                },
                TargetProfile::MacosArm64,
                "symbol",
            ),
        ];
        for (candidate, target, coordinate) in empty_cases {
            assert!(matches!(
                normalize_foreign_locator(candidate, target),
                Err(ForeignLocatorValidationError::EmptyCoordinate {
                    coordinate: rejected,
                    ..
                }) if rejected == coordinate
            ));
        }
        for (candidate, target, locator_case, coordinate, offset) in [
            (
                ForeignLocatorCandidate::PeByName {
                    library: b"kernel\0base.dll".to_vec(),
                    export: b"WriteFile".to_vec(),
                },
                TargetProfile::WindowsX64,
                "PeByName",
                "library",
                6,
            ),
            (
                ForeignLocatorCandidate::PeByName {
                    library: b"kernel32.dll".to_vec(),
                    export: b"Write\0File".to_vec(),
                },
                TargetProfile::WindowsX64,
                "PeByName",
                "export",
                5,
            ),
            (
                ForeignLocatorCandidate::PeByOrdinal {
                    library: b"user\0.dll".to_vec(),
                    ordinal: 17,
                },
                TargetProfile::WindowsX64,
                "PeByOrdinal",
                "library",
                4,
            ),
            (
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc\0.so.6".to_vec(),
                    symbol: b"memcpy".to_vec(),
                    version: b"GLIBC_2.14".to_vec(),
                },
                TargetProfile::LinuxX64,
                "ElfVersioned",
                "object",
                4,
            ),
            (
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: b"mem\0cpy".to_vec(),
                    version: b"GLIBC_2.14".to_vec(),
                },
                TargetProfile::LinuxX64,
                "ElfVersioned",
                "symbol",
                3,
            ),
            (
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: b"memcpy".to_vec(),
                    version: b"GLIBC\0_2.14".to_vec(),
                },
                TargetProfile::LinuxX64,
                "ElfVersioned",
                "version",
                5,
            ),
            (
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: b"/usr/lib/lib\0System.B.dylib".to_vec(),
                    symbol: b"_write".to_vec(),
                },
                TargetProfile::MacosArm64,
                "MachODylibSymbol",
                "install_name",
                12,
            ),
            (
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                    symbol: b"_wr\0ite".to_vec(),
                },
                TargetProfile::MacosArm64,
                "MachODylibSymbol",
                "symbol",
                3,
            ),
        ] {
            assert_eq!(
                normalize_foreign_locator(candidate, target),
                Err(ForeignLocatorValidationError::ForbiddenNul {
                    locator_case,
                    coordinate,
                    offset,
                })
            );
        }
    }

    #[test]
    fn pe_ordinal_zero_is_reserved_and_full_u16_range_is_otherwise_retained() {
        assert_eq!(
            normalize_foreign_locator(
                ForeignLocatorCandidate::PeByOrdinal {
                    library: b"user32.dll".to_vec(),
                    ordinal: 0,
                },
                TargetProfile::WindowsX64,
            ),
            Err(ForeignLocatorValidationError::ZeroPeOrdinal)
        );
        normalize_foreign_locator(
            ForeignLocatorCandidate::PeByOrdinal {
                library: b"user32.dll".to_vec(),
                ordinal: u16::MAX,
            },
            TargetProfile::WindowsX64,
        )
        .expect("the highest PE ordinal remains representable");
    }

    #[test]
    fn compatibility_fingerprint_changes_for_every_physical_coordinate_and_target_axis() {
        fn id(candidate: ForeignLocatorCandidate, target: TargetProfile) -> u64 {
            normalize_foreign_locator(candidate, target)
                .expect("mutation fixture remains valid")
                .non_authoritative_compatibility_fingerprint()
        }

        let baseline = id(elf_versioned(), TargetProfile::LinuxX64);
        let mutations = [
            id(
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.7".to_vec(),
                    symbol: b"memcpy".to_vec(),
                    version: b"GLIBC_2.14".to_vec(),
                },
                TargetProfile::LinuxX64,
            ),
            id(
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: b"memmove".to_vec(),
                    version: b"GLIBC_2.14".to_vec(),
                },
                TargetProfile::LinuxX64,
            ),
            id(
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: b"memcpy".to_vec(),
                    version: b"GLIBC_2.17".to_vec(),
                },
                TargetProfile::LinuxX64,
            ),
            id(elf_versioned(), TargetProfile::LinuxArm64),
            id(pe_name(), TargetProfile::WindowsX64),
            id(pe_ordinal(), TargetProfile::WindowsX64),
            id(
                ForeignLocatorCandidate::PeByOrdinal {
                    library: b"user32.dll".to_vec(),
                    ordinal: 18,
                },
                TargetProfile::WindowsX64,
            ),
        ];
        for mutation in mutations {
            assert_ne!(baseline, mutation);
        }

        let pe_name_baseline = id(pe_name(), TargetProfile::WindowsX64);
        assert_ne!(
            pe_name_baseline,
            id(
                ForeignLocatorCandidate::PeByName {
                    library: b"kernelbase.dll".to_vec(),
                    export: b"WriteFile".to_vec(),
                },
                TargetProfile::WindowsX64,
            ),
        );
        assert_ne!(
            pe_name_baseline,
            id(
                ForeignLocatorCandidate::PeByName {
                    library: b"kernel32.dll".to_vec(),
                    export: b"ReadFile".to_vec(),
                },
                TargetProfile::WindowsX64,
            ),
        );

        let pe_ordinal_baseline = id(pe_ordinal(), TargetProfile::WindowsX64);
        assert_ne!(
            pe_name_baseline, pe_ordinal_baseline,
            "case tag is identity"
        );
        assert_ne!(
            pe_ordinal_baseline,
            id(
                ForeignLocatorCandidate::PeByOrdinal {
                    library: b"kernel32.dll".to_vec(),
                    ordinal: 17,
                },
                TargetProfile::WindowsX64,
            ),
        );

        let macho_baseline = id(macho_dylib_symbol(), TargetProfile::MacosArm64);
        assert_ne!(baseline, macho_baseline, "case tag is identity");
        assert_ne!(
            macho_baseline,
            id(
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: b"/usr/lib/libobjc.A.dylib".to_vec(),
                    symbol: b"_write".to_vec(),
                },
                TargetProfile::MacosArm64,
            )
        );
        assert_ne!(
            macho_baseline,
            id(
                ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                    symbol: b"_read".to_vec(),
                },
                TargetProfile::MacosArm64,
            )
        );
        assert_ne!(
            pe_ordinal_baseline,
            id(
                ForeignLocatorCandidate::PeByOrdinal {
                    library: b"user32.dll".to_vec(),
                    ordinal: 18,
                },
                TargetProfile::WindowsX64,
            ),
        );
    }

    #[test]
    fn length_prefixes_prevent_coordinate_boundary_aliases() {
        let first = normalize_foreign_locator(
            ForeignLocatorCandidate::PeByName {
                library: b"ab".to_vec(),
                export: b"c".to_vec(),
            },
            TargetProfile::WindowsX64,
        )
        .unwrap();
        let second = normalize_foreign_locator(
            ForeignLocatorCandidate::PeByName {
                library: b"a".to_vec(),
                export: b"bc".to_vec(),
            },
            TargetProfile::WindowsX64,
        )
        .unwrap();
        assert_ne!(
            first.non_authoritative_compatibility_fingerprint(),
            second.non_authoritative_compatibility_fingerprint()
        );

        let first = normalize_foreign_locator(
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"ab".to_vec(),
                symbol: b"c".to_vec(),
            },
            TargetProfile::MacosArm64,
        )
        .unwrap();
        let second = normalize_foreign_locator(
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"a".to_vec(),
                symbol: b"bc".to_vec(),
            },
            TargetProfile::MacosArm64,
        )
        .unwrap();
        assert_ne!(
            first.non_authoritative_compatibility_fingerprint(),
            second.non_authoritative_compatibility_fingerprint()
        );
    }

    #[test]
    fn macho_locator_retains_non_utf8_coordinates_without_text_reconstruction() {
        let expected = ForeignLocatorCandidate::MachODylibSymbol {
            install_name: vec![b'/', b'l', b'i', b'b', 0xff],
            symbol: vec![b'_', 0xfe],
        };
        let normalized = normalize_foreign_locator(expected.clone(), TargetProfile::MacosArm64)
            .expect("non-UTF-8 bytes are valid structural locator coordinates");
        assert_eq!(normalized.locator(), &expected);
    }
}
