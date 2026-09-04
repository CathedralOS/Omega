//! Closed target-feature requirements for x86-64 instruction fragments.
//!
//! A requirement is not an admission. In particular, constructing the scalar
//! FMA requirement records that AVX and FMA3 must be discharged for one exact
//! deployment profile; it does not claim that the selected hardware provides
//! either feature.

use sha2::{Digest, Sha256};

use crate::{Architecture, TargetProfile};

const X86_FEATURE_REQUIREMENT_SCHEMA: &[u8] = b"omega.x86-feature-requirement.v1";
const X86_DEPLOYMENT_FEATURES_SCHEMA: &[u8] = b"omega.x86-deployment-features.v1";
const X86_SCALAR_FMA_DIFFERENTIAL_SCHEMA: &[u8] = b"omega.x86-scalar-fma-differential.v1";
const X86_SCALAR_FMA_PROVIDER_SCHEMA: &[u8] = b"omega.x86-scalar-fma-provider.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum X86TargetFeature {
    Avx,
    Fma3,
}

/// Generic nearest-even scalar FMA slots that an admitted x86 provider may
/// satisfy. These are semantic slots, not instruction mnemonics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum X86ScalarFmaSlot {
    Binary32,
    Binary64,
}

impl X86ScalarFmaSlot {
    pub const ALL: [Self; 2] = [Self::Binary32, Self::Binary64];

    pub const fn requirement_identity(self) -> &'static str {
        match self {
            Self::Binary32 => "F32::fused_multiply_add",
            Self::Binary64 => "F64::fused_multiply_add",
        }
    }

    pub const fn realization_identity(self) -> &'static str {
        match self {
            Self::Binary32 => "x86_64::VFMADD132SS",
            Self::Binary64 => "x86_64::VFMADD132SD",
        }
    }

    /// Exact normalized operator requirement carried by the selected
    /// `ProviderPlan` row for this semantic slot. This is deliberately
    /// distinct from the stable human-facing slot identity above.
    pub const fn selected_plan_requirement_identity(self) -> &'static str {
        match self {
            Self::Binary32 => {
                "operator::F32::fused_multiply_add(named(name(f32)),named(name(f32)),named(name(f32)))->named(name(f32))"
            }
            Self::Binary64 => {
                "operator::F64::fused_multiply_add(named(name(f64)),named(name(f64)),named(name(f64)))->named(name(f64))"
            }
        }
    }
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

/// Explicit deployment-owned x86 feature inventory.
///
/// Construction records an input claim; it does not inspect the compiler host.
/// The provider admission below is the boundary that checks the claim against
/// one exact requirement and the retained differential receipts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86DeploymentFeatures {
    profile: TargetProfile,
    features: [X86TargetFeature; 2],
    identity: [u8; 32],
}

impl X86DeploymentFeatures {
    pub fn scalar_fma(
        profile: TargetProfile,
        features: &[X86TargetFeature],
    ) -> Result<Self, X86ScalarFmaAdmissionError> {
        if profile.native_target().architecture != Architecture::X86_64 {
            return Err(X86ScalarFmaAdmissionError::UnsupportedProfile(profile));
        }
        if features != X86_SCALAR_FMA_REQUIRED_FEATURES {
            return Err(X86ScalarFmaAdmissionError::MissingExactFeatureSet);
        }
        let features = X86_SCALAR_FMA_REQUIRED_FEATURES;
        Ok(Self {
            profile,
            features,
            identity: deployment_features_identity(profile, &features),
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
            && self.identity == deployment_features_identity(self.profile, &self.features)
    }
}

/// Checked differential evidence for one generic scalar FMA slot.
///
/// The canonical cancellation vector distinguishes one fused rounding from
/// multiply-then-add. The receipt retains raw bits so it is independent of the
/// compiler host's floating-point environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86ScalarFmaDifferentialReceipt {
    slot: X86ScalarFmaSlot,
    operand_bits: [u64; 3],
    fused_result_bits: u64,
    multiply_then_add_result_bits: u64,
    identity: [u8; 32],
}

impl X86ScalarFmaDifferentialReceipt {
    pub fn admit(
        slot: X86ScalarFmaSlot,
        operand_bits: [u64; 3],
        fused_result_bits: u64,
        multiply_then_add_result_bits: u64,
    ) -> Result<Self, X86ScalarFmaAdmissionError> {
        let (expected_operands, expected_fused, expected_unfused) = differential_vector(slot);
        if operand_bits != expected_operands
            || fused_result_bits != expected_fused
            || multiply_then_add_result_bits != expected_unfused
            || fused_result_bits == multiply_then_add_result_bits
        {
            return Err(X86ScalarFmaAdmissionError::InvalidDifferentialReceipt(slot));
        }
        let identity = differential_identity(
            slot,
            &operand_bits,
            fused_result_bits,
            multiply_then_add_result_bits,
        );
        Ok(Self {
            slot,
            operand_bits,
            fused_result_bits,
            multiply_then_add_result_bits,
            identity,
        })
    }

    pub const fn slot(self) -> X86ScalarFmaSlot {
        self.slot
    }

    pub const fn operand_bits(self) -> [u64; 3] {
        self.operand_bits
    }

    pub const fn fused_result_bits(self) -> u64 {
        self.fused_result_bits
    }

    pub const fn multiply_then_add_result_bits(self) -> u64 {
        self.multiply_then_add_result_bits
    }

    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub fn has_canonical_identity(self) -> bool {
        Self::admit(
            self.slot,
            self.operand_bits,
            self.fused_result_bits,
            self.multiply_then_add_result_bits,
        )
        .is_ok_and(|expected| expected == self)
    }
}

/// Feature-qualified selection of both generic nearest-even x86 FMA slots.
///
/// This is the first authority-bearing rung. Requirement, deployment claim,
/// both semantic slots, and both differential receipts are inseparable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedX86ScalarFmaProvider {
    requirement: X86FeatureRequirement,
    deployment: X86DeploymentFeatures,
    differential_receipts: [X86ScalarFmaDifferentialReceipt; 2],
    identity: [u8; 32],
}

impl AdmittedX86ScalarFmaProvider {
    /// Admit one exact compiler-retained deployment claim for the canonical
    /// scalar FMA feature pair.
    ///
    /// This constructor replays the fixed semantic cancellation vectors used
    /// by the source-free custody seam. It does not execute the selected
    /// target, inspect the compiler host, or manufacture a native execution
    /// receipt. Native differential execution remains separate evidence.
    pub fn from_deployment_claim(
        profile: TargetProfile,
        features: &[X86TargetFeature],
    ) -> Result<Self, X86ScalarFmaAdmissionError> {
        let requirement = X86FeatureRequirement::scalar_fma(profile)
            .ok_or(X86ScalarFmaAdmissionError::UnsupportedProfile(profile))?;
        let deployment = X86DeploymentFeatures::scalar_fma(profile, features)?;
        let differential_receipts = X86ScalarFmaSlot::ALL.map(|slot| {
            let (operand_bits, fused_result_bits, multiply_then_add_result_bits) =
                differential_vector(slot);
            X86ScalarFmaDifferentialReceipt::admit(
                slot,
                operand_bits,
                fused_result_bits,
                multiply_then_add_result_bits,
            )
            .expect("closed scalar FMA semantic vector must remain canonical")
        });
        Self::admit(requirement, deployment, differential_receipts)
    }

    pub fn admit(
        requirement: X86FeatureRequirement,
        deployment: X86DeploymentFeatures,
        differential_receipts: [X86ScalarFmaDifferentialReceipt; 2],
    ) -> Result<Self, X86ScalarFmaAdmissionError> {
        if !requirement.has_canonical_identity() || !deployment.has_canonical_identity() {
            return Err(X86ScalarFmaAdmissionError::NonCanonicalIdentity);
        }
        if requirement.profile() != deployment.profile() {
            return Err(X86ScalarFmaAdmissionError::ProfileMismatch);
        }
        for (expected_slot, receipt) in X86ScalarFmaSlot::ALL.into_iter().zip(differential_receipts)
        {
            if receipt.slot() != expected_slot || !receipt.has_canonical_identity() {
                return Err(X86ScalarFmaAdmissionError::InvalidDifferentialReceipt(
                    expected_slot,
                ));
            }
        }
        let identity = provider_identity(requirement, deployment, &differential_receipts);
        Ok(Self {
            requirement,
            deployment,
            differential_receipts,
            identity,
        })
    }

    pub const fn profile(self) -> TargetProfile {
        self.requirement.profile()
    }

    pub const fn requirement(self) -> X86FeatureRequirement {
        self.requirement
    }

    pub const fn deployment(self) -> X86DeploymentFeatures {
        self.deployment
    }

    pub const fn selected_slots(self) -> &'static [X86ScalarFmaSlot; 2] {
        &X86ScalarFmaSlot::ALL
    }

    pub const fn differential_receipts(&self) -> &[X86ScalarFmaDifferentialReceipt; 2] {
        &self.differential_receipts
    }

    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub fn admits(self, requirement: X86FeatureRequirement, slot: X86ScalarFmaSlot) -> bool {
        self.has_canonical_identity()
            && requirement == self.requirement
            && X86ScalarFmaSlot::ALL.contains(&slot)
    }

    pub fn has_canonical_identity(self) -> bool {
        Self::admit(
            self.requirement,
            self.deployment,
            self.differential_receipts,
        )
        .is_ok_and(|expected| expected == self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86ScalarFmaAdmissionError {
    UnsupportedProfile(TargetProfile),
    MissingExactFeatureSet,
    ProfileMismatch,
    InvalidDifferentialReceipt(X86ScalarFmaSlot),
    NonCanonicalIdentity,
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

fn deployment_features_identity(
    profile: TargetProfile,
    features: &[X86TargetFeature; 2],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(X86_DEPLOYMENT_FEATURES_SCHEMA);
    hash_profile_and_features(&mut hasher, profile, features);
    hasher.finalize().into()
}

fn hash_profile_and_features(
    hasher: &mut Sha256,
    profile: TargetProfile,
    features: &[X86TargetFeature; 2],
) {
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
}

fn differential_vector(slot: X86ScalarFmaSlot) -> ([u64; 3], u64, u64) {
    match slot {
        X86ScalarFmaSlot::Binary32 => ([0x3f80_0001, 0x3f7f_fffe, 0xbf80_0000], 0xa880_0000, 0),
        X86ScalarFmaSlot::Binary64 => (
            [
                0x3ff0_0000_0000_0001,
                0x3fef_ffff_ffff_fffe,
                0xbff0_0000_0000_0000,
            ],
            0xb970_0000_0000_0000,
            0,
        ),
    }
}

fn differential_identity(
    slot: X86ScalarFmaSlot,
    operand_bits: &[u64; 3],
    fused_result_bits: u64,
    multiply_then_add_result_bits: u64,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(X86_SCALAR_FMA_DIFFERENTIAL_SCHEMA);
    hasher.update([slot_tag(slot)]);
    for bits in operand_bits {
        hasher.update(bits.to_le_bytes());
    }
    hasher.update(fused_result_bits.to_le_bytes());
    hasher.update(multiply_then_add_result_bits.to_le_bytes());
    hasher.finalize().into()
}

fn provider_identity(
    requirement: X86FeatureRequirement,
    deployment: X86DeploymentFeatures,
    differential_receipts: &[X86ScalarFmaDifferentialReceipt; 2],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(X86_SCALAR_FMA_PROVIDER_SCHEMA);
    hasher.update(requirement.identity());
    hasher.update(deployment.identity());
    for (slot, receipt) in X86ScalarFmaSlot::ALL.into_iter().zip(differential_receipts) {
        hasher.update([slot_tag(slot)]);
        hasher.update(slot.requirement_identity().as_bytes());
        hasher.update(slot.realization_identity().as_bytes());
        hasher.update(receipt.identity());
    }
    hasher.finalize().into()
}

const fn slot_tag(slot: X86ScalarFmaSlot) -> u8 {
    match slot {
        X86ScalarFmaSlot::Binary32 => 1,
        X86ScalarFmaSlot::Binary64 => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn differential_receipts() -> [X86ScalarFmaDifferentialReceipt; 2] {
        [
            X86ScalarFmaDifferentialReceipt::admit(
                X86ScalarFmaSlot::Binary32,
                [0x3f80_0001, 0x3f7f_fffe, 0xbf80_0000],
                0xa880_0000,
                0,
            )
            .unwrap(),
            X86ScalarFmaDifferentialReceipt::admit(
                X86ScalarFmaSlot::Binary64,
                [
                    0x3ff0_0000_0000_0001,
                    0x3fef_ffff_ffff_fffe,
                    0xbff0_0000_0000_0000,
                ],
                0xb970_0000_0000_0000,
                0,
            )
            .unwrap(),
        ]
    }

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

    #[test]
    fn deployment_admission_selects_both_generic_slots_only_for_exact_features() {
        let profile = TargetProfile::LinuxX64;
        let requirement = X86FeatureRequirement::scalar_fma(profile).unwrap();
        let deployment = X86DeploymentFeatures::scalar_fma(
            profile,
            &[X86TargetFeature::Avx, X86TargetFeature::Fma3],
        )
        .unwrap();
        let admitted =
            AdmittedX86ScalarFmaProvider::admit(requirement, deployment, differential_receipts())
                .unwrap();

        assert!(admitted.has_canonical_identity());
        assert_eq!(admitted.selected_slots(), &X86ScalarFmaSlot::ALL);
        for slot in X86ScalarFmaSlot::ALL {
            assert!(admitted.admits(requirement, slot));
            assert!(slot.requirement_identity().contains("fused_multiply_add"));
            assert!(slot.realization_identity().contains("VFMADD132"));
        }
        assert_eq!(
            X86DeploymentFeatures::scalar_fma(profile, &[X86TargetFeature::Avx]),
            Err(X86ScalarFmaAdmissionError::MissingExactFeatureSet)
        );
        assert_eq!(
            X86DeploymentFeatures::scalar_fma(
                profile,
                &[X86TargetFeature::Fma3, X86TargetFeature::Avx],
            ),
            Err(X86ScalarFmaAdmissionError::MissingExactFeatureSet)
        );
    }

    #[test]
    fn admission_rejects_cross_profile_and_incomplete_differential_evidence() {
        let linux_requirement = X86FeatureRequirement::scalar_fma(TargetProfile::LinuxX64).unwrap();
        let windows_deployment = X86DeploymentFeatures::scalar_fma(
            TargetProfile::WindowsX64,
            &X86_SCALAR_FMA_REQUIRED_FEATURES,
        )
        .unwrap();
        assert_eq!(
            AdmittedX86ScalarFmaProvider::admit(
                linux_requirement,
                windows_deployment,
                differential_receipts(),
            ),
            Err(X86ScalarFmaAdmissionError::ProfileMismatch)
        );

        assert_eq!(
            X86ScalarFmaDifferentialReceipt::admit(
                X86ScalarFmaSlot::Binary32,
                [0x3f80_0001, 0x3f7f_fffe, 0xbf80_0000],
                0,
                0,
            ),
            Err(X86ScalarFmaAdmissionError::InvalidDifferentialReceipt(
                X86ScalarFmaSlot::Binary32
            ))
        );
        let mut reversed = differential_receipts();
        reversed.swap(0, 1);
        let linux_deployment = X86DeploymentFeatures::scalar_fma(
            TargetProfile::LinuxX64,
            &X86_SCALAR_FMA_REQUIRED_FEATURES,
        )
        .unwrap();
        assert_eq!(
            AdmittedX86ScalarFmaProvider::admit(linux_requirement, linux_deployment, reversed,),
            Err(X86ScalarFmaAdmissionError::InvalidDifferentialReceipt(
                X86ScalarFmaSlot::Binary32
            ))
        );
    }

    #[test]
    fn provider_identity_binds_the_exact_deployment_profile() {
        let admit = |profile| {
            let requirement = X86FeatureRequirement::scalar_fma(profile).unwrap();
            let deployment =
                X86DeploymentFeatures::scalar_fma(profile, &X86_SCALAR_FMA_REQUIRED_FEATURES)
                    .unwrap();
            AdmittedX86ScalarFmaProvider::admit(requirement, deployment, differential_receipts())
                .unwrap()
        };
        assert_ne!(
            admit(TargetProfile::LinuxX64).identity(),
            admit(TargetProfile::WindowsX64).identity()
        );
    }

    #[test]
    fn deployment_claim_factory_is_profile_bound_and_not_host_inferred() {
        let linux = AdmittedX86ScalarFmaProvider::from_deployment_claim(
            TargetProfile::LinuxX64,
            &X86_SCALAR_FMA_REQUIRED_FEATURES,
        )
        .unwrap();
        let windows = AdmittedX86ScalarFmaProvider::from_deployment_claim(
            TargetProfile::WindowsX64,
            &X86_SCALAR_FMA_REQUIRED_FEATURES,
        )
        .unwrap();

        assert!(linux.has_canonical_identity());
        assert!(windows.has_canonical_identity());
        assert_ne!(linux.identity(), windows.identity());
        assert_eq!(
            AdmittedX86ScalarFmaProvider::from_deployment_claim(
                TargetProfile::LinuxX64,
                &[X86TargetFeature::Avx],
            ),
            Err(X86ScalarFmaAdmissionError::MissingExactFeatureSet)
        );
        assert_eq!(
            AdmittedX86ScalarFmaProvider::from_deployment_claim(
                TargetProfile::LinuxArm64,
                &X86_SCALAR_FMA_REQUIRED_FEATURES,
            ),
            Err(X86ScalarFmaAdmissionError::UnsupportedProfile(
                TargetProfile::LinuxArm64
            ))
        );
    }
}
