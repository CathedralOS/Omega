//! Normalized target/deployment input for an ELF interpreter selection.
//!
//! This carrier seals only the exact `PT_INTERP` pathname bytes and Linux
//! target profile. It does not construct an ELF image, choose dynamic-link
//! tables, or grant loader/admission authority.
//!
//! Build output configuration selects the pathname per target
//! (wiki/spec/build/configuration.md#elf-executable-loader-selection). When it
//! selects none, [`declared_elf_interpreter_default`] supplies the loader the
//! bundled Linux target environment declares. That default belongs to the
//! toolchain's target environment, never to the architecture, and nothing here
//! probes the build host's loader.

use crate::TargetProfile;
use crate::compatibility_fingerprint::Fnv1a;
use std::fmt;

/// Every deployment profile whose dynamically linked output carries a
/// `PT_INTERP` pathname, in canonical target-profile identity order.
pub const ELF_INTERPRETER_TARGET_PROFILES: [TargetProfile; 2] =
    [TargetProfile::LinuxArm64, TargetProfile::LinuxX64];

/// The loader pathname the bundled Linux target environment declares for a
/// dynamically linked executable.
///
/// The toolchain's Linux target environment binds glibc's versioned exports
/// (`libc.so.6` at `GLIBC_*` versions), so it declares the glibc psABI
/// interpreter. A build deploying another C runtime (musl, a relocated loader)
/// overrides this per target; it is a deployment default, not a fact derived
/// from the architecture or read from the build host.
const fn declared_elf_interpreter_default_path(target: TargetProfile) -> Option<&'static [u8]> {
    match target {
        TargetProfile::LinuxX64 => Some(b"/lib64/ld-linux-x86-64.so.2"),
        TargetProfile::LinuxArm64 => Some(b"/lib/ld-linux-aarch64.so.1"),
        TargetProfile::MacosArm64
        | TargetProfile::MacosX64
        | TargetProfile::WindowsX64
        | TargetProfile::UefiX64
        | TargetProfile::CrossPlatformCli
        | TargetProfile::LocalUnchecked
        | TargetProfile::AlphaBootstrap => None,
    }
}

/// The normalized default loader of one target environment, or `None` when
/// the environment declares none (every non-ELF profile). The declared bytes
/// pass the same validation an explicit build selection does.
pub fn declared_elf_interpreter_default(
    target: TargetProfile,
) -> Option<NormalizedElfInterpreterPlan> {
    declared_elf_interpreter_default_path(target).map(|path| {
        normalize_elf_interpreter_plan(path.to_vec(), target)
            .expect("a declared target-environment loader is a valid interpreter plan")
    })
}

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
    non_authoritative_compatibility_fingerprint: u64,
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
    pub const fn non_authoritative_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_compatibility_fingerprint
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
    if !ELF_INTERPRETER_TARGET_PROFILES.contains(&target) {
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

    let non_authoritative_compatibility_fingerprint =
        non_authoritative_compatibility_fingerprint(target, &interpreter_path);
    Ok(NormalizedElfInterpreterPlan {
        target,
        interpreter_path,
        non_authoritative_compatibility_fingerprint,
    })
}

fn non_authoritative_compatibility_fingerprint(
    target: TargetProfile,
    interpreter_path: &[u8],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-interpreter-plan.v1");
    hash.bytes(target.target_name().as_bytes());
    hash.bytes(interpreter_path);
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::{
        ELF_INTERPRETER_TARGET_PROFILES, ElfInterpreterPlanValidationError,
        NormalizedElfInterpreterPlan, TargetProfile, declared_elf_interpreter_default,
        normalize_elf_interpreter_plan,
    };

    fn normalize(path: &[u8], target: TargetProfile) -> NormalizedElfInterpreterPlan {
        normalize_elf_interpreter_plan(path.to_vec(), target).expect("valid interpreter plan")
    }

    #[test]
    fn exact_linux_profile_and_raw_non_utf8_path_are_retained() {
        let path = b"/lib64/ld-linux-\xff-x86-64.so.2";
        let plan = normalize(path, TargetProfile::LinuxX64);

        assert_eq!(plan.target(), TargetProfile::LinuxX64);
        assert_eq!(plan.interpreter_path(), path);
        assert_ne!(plan.non_authoritative_compatibility_fingerprint(), 0);
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
    fn compatibility_fingerprint_covers_profile_path_bytes_and_length_framing() {
        let baseline = normalize(b"/ab", TargetProfile::LinuxX64);
        let mutations = [
            normalize(b"/a", TargetProfile::LinuxX64),
            normalize(b"/abc", TargetProfile::LinuxX64),
            normalize(b"/ac", TargetProfile::LinuxX64),
            normalize(b"/ab", TargetProfile::LinuxArm64),
        ];

        for mutation in mutations {
            assert_ne!(
                baseline.non_authoritative_compatibility_fingerprint(),
                mutation.non_authoritative_compatibility_fingerprint()
            );
        }
        assert_eq!(
            baseline,
            normalize(b"/ab", TargetProfile::LinuxX64),
            "the exact input normalizes deterministically",
        );
    }

    #[test]
    fn linux_target_environments_declare_glibc_loader_defaults() {
        let x86_64 = declared_elf_interpreter_default(TargetProfile::LinuxX64)
            .expect("the Linux x86-64 environment declares a loader");
        assert_eq!(x86_64.target(), TargetProfile::LinuxX64);
        assert_eq!(x86_64.interpreter_path(), b"/lib64/ld-linux-x86-64.so.2");
        let arm64 = declared_elf_interpreter_default(TargetProfile::LinuxArm64)
            .expect("the Linux ARM64 environment declares a loader");
        assert_eq!(arm64.target(), TargetProfile::LinuxArm64);
        assert_eq!(arm64.interpreter_path(), b"/lib/ld-linux-aarch64.so.1");
    }

    #[test]
    fn only_elf_interpreter_profiles_declare_a_default() {
        for target in [
            TargetProfile::LinuxArm64,
            TargetProfile::LinuxX64,
            TargetProfile::MacosArm64,
            TargetProfile::MacosX64,
            TargetProfile::WindowsX64,
            TargetProfile::UefiX64,
            TargetProfile::CrossPlatformCli,
            TargetProfile::LocalUnchecked,
            TargetProfile::AlphaBootstrap,
        ] {
            assert_eq!(
                declared_elf_interpreter_default(target).is_some(),
                ELF_INTERPRETER_TARGET_PROFILES.contains(&target),
                "{} declares a default exactly when it carries PT_INTERP",
                target.target_name(),
            );
        }
    }
}
