//! Normalized target/deployment input for an ELF interpreter selection.
//!
//! This carrier seals only the exact `PT_INTERP` pathname bytes and Linux
//! target profile. It does not construct an ELF image, choose dynamic-link
//! tables, or grant loader/admission authority.

use crate::TargetProfile;
use std::fmt;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Read-only, structurally validated input for one exact Linux ELF
/// interpreter.
///
/// The path remains raw bytes so target/deployment policy never has to
/// reconstruct foreign coordinates through UTF-8 text. The eventual ELF
/// emitter is responsible for adding the `PT_INTERP` segment terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedElfInterpreterPlan {
    target: TargetProfile,
    interpreter_path: Vec<u8>,
    normalized_identity: u64,
}

impl NormalizedElfInterpreterPlan {
    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub fn interpreter_path(&self) -> &[u8] {
        &self.interpreter_path
    }

    /// Compatibility fingerprint of the exact target and length-framed raw
    /// path bytes. This is deterministic artifact identity, not loader or
    /// admission authority.
    pub const fn normalized_identity(&self) -> u64 {
        self.normalized_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfInterpreterPlanValidationError {
    UnsupportedTarget { target: TargetProfile },
    EmptyPath,
    RelativePath,
    ForbiddenNul { offset: usize },
}

impl fmt::Display for ElfInterpreterPlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedTarget { target } => write!(
                formatter,
                "ELF interpreter plan is not applicable to selected target `{}`",
                target.target_name(),
            ),
            Self::EmptyPath => write!(formatter, "ELF interpreter plan has an empty path"),
            Self::RelativePath => {
                write!(formatter, "ELF interpreter plan path is not absolute")
            }
            Self::ForbiddenNul { offset } => write!(
                formatter,
                "ELF interpreter plan path contains a forbidden NUL byte at offset {offset}",
            ),
        }
    }
}

impl std::error::Error for ElfInterpreterPlanValidationError {}

/// Validate and seal one target/deployment-selected ELF interpreter input.
///
/// Construction grants no publication or executable-loader authority. Dynamic
/// ELF emission remains unavailable until a later owner joins this input to
/// the complete dynamic-link structures and validates the resulting image.
pub fn normalize_elf_interpreter_plan(
    interpreter_path: Vec<u8>,
    target: TargetProfile,
) -> Result<NormalizedElfInterpreterPlan, ElfInterpreterPlanValidationError> {
    if !matches!(target, TargetProfile::LinuxArm64 | TargetProfile::LinuxX64) {
        return Err(ElfInterpreterPlanValidationError::UnsupportedTarget { target });
    }
    if interpreter_path.is_empty() {
        return Err(ElfInterpreterPlanValidationError::EmptyPath);
    }
    if interpreter_path.first() != Some(&b'/') {
        return Err(ElfInterpreterPlanValidationError::RelativePath);
    }
    if let Some(offset) = interpreter_path.iter().position(|byte| *byte == 0) {
        return Err(ElfInterpreterPlanValidationError::ForbiddenNul { offset });
    }

    let normalized_identity = fingerprint(target, &interpreter_path);
    Ok(NormalizedElfInterpreterPlan {
        target,
        interpreter_path,
        normalized_identity,
    })
}

fn fingerprint(target: TargetProfile, interpreter_path: &[u8]) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-interpreter-plan.v1");
    hash.bytes(target.target_name().as_bytes());
    hash.bytes(interpreter_path);
    hash.finish()
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

    fn normalize(path: &[u8], target: TargetProfile) -> NormalizedElfInterpreterPlan {
        normalize_elf_interpreter_plan(path.to_vec(), target).expect("valid interpreter plan")
    }

    #[test]
    fn exact_linux_profile_and_raw_non_utf8_path_are_retained() {
        let path = b"/lib64/ld-linux-\xff-x86-64.so.2";
        let plan = normalize(path, TargetProfile::LinuxX64);

        assert_eq!(plan.target(), TargetProfile::LinuxX64);
        assert_eq!(plan.interpreter_path(), path);
        assert_ne!(plan.normalized_identity(), 0);
    }

    #[test]
    fn non_linux_profiles_reject() {
        for target in [
            TargetProfile::MacosArm64,
            TargetProfile::WindowsX64,
            TargetProfile::UefiX64,
            TargetProfile::CrossPlatformCli,
            TargetProfile::LocalUnchecked,
        ] {
            assert_eq!(
                normalize_elf_interpreter_plan(b"/loader".to_vec(), target),
                Err(ElfInterpreterPlanValidationError::UnsupportedTarget { target }),
            );
        }
    }

    #[test]
    fn empty_relative_and_nul_containing_paths_reject() {
        assert_eq!(
            normalize_elf_interpreter_plan(Vec::new(), TargetProfile::LinuxX64),
            Err(ElfInterpreterPlanValidationError::EmptyPath),
        );
        assert_eq!(
            normalize_elf_interpreter_plan(
                b"lib64/ld-linux-x86-64.so.2".to_vec(),
                TargetProfile::LinuxX64,
            ),
            Err(ElfInterpreterPlanValidationError::RelativePath),
        );
        assert_eq!(
            normalize_elf_interpreter_plan(
                b"/lib/ld-linux\0-aarch64.so.1".to_vec(),
                TargetProfile::LinuxArm64,
            ),
            Err(ElfInterpreterPlanValidationError::ForbiddenNul { offset: 13 }),
        );
    }

    #[test]
    fn identity_binds_profile_path_bytes_and_length_framing() {
        let baseline = normalize(b"/ab", TargetProfile::LinuxX64);
        let mutations = [
            normalize(b"/a", TargetProfile::LinuxX64),
            normalize(b"/abc", TargetProfile::LinuxX64),
            normalize(b"/ac", TargetProfile::LinuxX64),
            normalize(b"/ab", TargetProfile::LinuxArm64),
        ];

        for mutation in mutations {
            assert_ne!(
                baseline.normalized_identity(),
                mutation.normalized_identity()
            );
        }
        assert_eq!(
            baseline,
            normalize(b"/ab", TargetProfile::LinuxX64),
            "the exact input normalizes deterministically",
        );
    }
}
