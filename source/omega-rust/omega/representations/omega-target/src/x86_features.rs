//! Closed target-feature requirements for x86-64 instruction fragments.
//!
//! A requirement is not an admission. In particular, constructing the scalar
//! FMA requirement records that AVX and FMA3 must be discharged for one exact
//! deployment profile; it does not claim that the selected hardware provides
//! either feature.

use sha2::{Digest, Sha256};

use crate::{Architecture, TargetProfile};

const X86_FEATURE_REQUIREMENT_SCHEMA: &[u8] = b"omega.x86-feature-requirement.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum X86TargetFeature {
    Avx,
    Fma3,
}

/// The exact feature pair required by canonical scalar FMA3 encodings.
pub const X86_SCALAR_FMA_REQUIRED_FEATURES: [X86TargetFeature; 2] =
    [X86TargetFeature::Avx, X86TargetFeature::Fma3];

/// Immutable requirement emitted by one target-owned x86 instruction model.
///
/// The private identity is recomputed by every downstream custody boundary.
/// It binds the deployment profile as well as the canonical feature set so a
/// receipt for one profile cannot be transplanted to another profile sharing
/// the same [`crate::NativeTarget`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86FeatureRequirement {
    profile: TargetProfile,
    features: [X86TargetFeature; 2],
    identity: [u8; 32],
}

impl X86FeatureRequirement {
    /// Produce the target-owned AVX+FMA3 requirement for one x86-64 profile.
    ///
    /// This function deliberately does not inspect the host or admit hardware.
    pub fn scalar_fma(profile: TargetProfile) -> Option<Self> {
        if profile.native_target().architecture != Architecture::X86_64 {
            return None;
        }
        let features = X86_SCALAR_FMA_REQUIRED_FEATURES;
        Some(Self {
            profile,
            features,
            identity: requirement_identity(profile, &features),
        })
    }

    pub const fn profile(self) -> TargetProfile {
        self.profile
    }

    pub const fn features(self) -> &'static [X86TargetFeature; 2] {
        &X86_SCALAR_FMA_REQUIRED_FEATURES
    }

    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub fn has_canonical_identity(self) -> bool {
        self.features == X86_SCALAR_FMA_REQUIRED_FEATURES
            && self.identity == requirement_identity(self.profile, &self.features)
    }
}

fn requirement_identity(profile: TargetProfile, features: &[X86TargetFeature; 2]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(X86_FEATURE_REQUIREMENT_SCHEMA);
    let profile = profile.identity().as_str().as_bytes();
    hasher.update((profile.len() as u64).to_le_bytes());
    hasher.update(profile);
    hasher.update((features.len() as u64).to_le_bytes());
    for feature in features {
        hasher.update([match feature {
            X86TargetFeature::Avx => 1,
            X86TargetFeature::Fma3 => 2,
        }]);
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_fma_requires_exact_avx_and_fma3_without_host_inference() {
        for profile in [
            TargetProfile::LinuxX64,
            TargetProfile::WindowsX64,
            TargetProfile::UefiX64,
        ] {
            let requirement = X86FeatureRequirement::scalar_fma(profile).unwrap();
            assert_eq!(requirement.profile(), profile);
            assert_eq!(requirement.features(), &X86_SCALAR_FMA_REQUIRED_FEATURES);
            assert!(requirement.has_canonical_identity());
        }
        assert!(X86FeatureRequirement::scalar_fma(TargetProfile::LinuxArm64).is_none());
        assert!(X86FeatureRequirement::scalar_fma(TargetProfile::MacosArm64).is_none());
    }

    #[test]
    fn scalar_fma_requirement_identity_binds_exact_profile() {
        let linux = X86FeatureRequirement::scalar_fma(TargetProfile::LinuxX64).unwrap();
        let windows = X86FeatureRequirement::scalar_fma(TargetProfile::WindowsX64).unwrap();
        let uefi = X86FeatureRequirement::scalar_fma(TargetProfile::UefiX64).unwrap();
        assert_ne!(linux.identity(), windows.identity());
        assert_ne!(windows.identity(), uefi.identity());
        assert_ne!(linux.identity(), uefi.identity());
    }
}
