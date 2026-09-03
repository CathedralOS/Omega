//! Compiler-owned symbolic target-semantic observation applications.
//!
//! An application names the selected projection, subject, semantics version,
//! and deployment profile. It deliberately carries no runtime observation or
//! stack address. Target closure separately turns that symbolic application
//! into one exact numeric guarantee while preserving the distinction between
//! a selected target contract and conformance of a physical invocation.

use sha2::{Digest, Sha256};

use crate::TargetProfile;

const GUARANTEED_ENTRY_STACK_PROJECTION: &str = "TargetSemantics::guaranteed_entry_stack";
const UEFI_X86_64_SUBJECT: &str = "UefiX86_64";
const PROJECTION_SEMANTICS_VERSION: u32 = 1;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

mod sealed {
    pub trait EntryStackSubject {}
}

/// Closed subject vocabulary accepted by `guaranteed_entry_stack`.
pub trait TargetEntryStackSubject: sealed::EntryStackSubject {
    const SUBJECT_IDENTITY: &'static str;
    const REQUIRED_PROFILE: TargetProfile;
    const GUARANTEED_AVAILABLE_BYTES: u64;
    const REQUIRED_ALIGNMENT: u64;
}

/// Compiler-owned subject of the UEFI x86-64 physical-entry stack guarantee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UefiX86_64;

impl sealed::EntryStackSubject for UefiX86_64 {}

impl TargetEntryStackSubject for UefiX86_64 {
    const SUBJECT_IDENTITY: &'static str = UEFI_X86_64_SUBJECT;
    const REQUIRED_PROFILE: TargetProfile = TargetProfile::UefiX64;
    // UEFI Specification 2.11 section 2.3.4 requires x64 boot-services
    // execution to have at least 128 KiB of available, 16-byte-aligned stack.
    const GUARANTEED_AVAILABLE_BYTES: u64 = 128 * 1024;
    const REQUIRED_ALIGNMENT: u64 = 16;
}

/// Closed symbolic target-observation application retained by compatibility
/// evidence. Exact fields are normative; the compact identity is report-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicTargetObservationApplication {
    projection: &'static str,
    subject: &'static str,
    projection_semantics_version: u32,
    selected_profile: TargetProfile,
    non_authoritative_compatibility_report_identity: u64,
    compatibility_commitment: [u8; 32],
}

impl SymbolicTargetObservationApplication {
    pub const fn projection(&self) -> &'static str {
        self.projection
    }

    pub const fn subject(&self) -> &'static str {
        self.subject
    }

    pub const fn projection_semantics_version(&self) -> u32 {
        self.projection_semantics_version
    }

    pub const fn selected_profile(&self) -> TargetProfile {
        self.selected_profile
    }

    /// Deterministic compatibility/report coordinate over the retained exact
    /// application. It is not authority and cannot replace structural replay.
    pub const fn non_authoritative_compatibility_report_identity(&self) -> u64 {
        self.non_authoritative_compatibility_report_identity
    }

    /// Domain-separated commitment to the complete symbolic application.
    pub const fn compatibility_commitment(&self) -> &[u8; 32] {
        &self.compatibility_commitment
    }

    pub fn matches_exact_uefi_x64_entry_stack_application(&self) -> bool {
        TargetSemantics::guaranteed_entry_stack::<UefiX86_64>(TargetProfile::UefiX64)
            .is_ok_and(|expected| self == &expected)
    }
}

/// Numeric closure of one symbolic entry-stack target observation.
///
/// This is target-contract compatibility evidence only. It neither observes a
/// runtime stack nor admits that the current firmware invocation conforms to
/// the selected profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetEntryStackGuarantee {
    application: SymbolicTargetObservationApplication,
    guaranteed_available_bytes: u64,
    required_alignment: u64,
    non_authoritative_compatibility_report_identity: u64,
    compatibility_commitment: [u8; 32],
}

impl TargetEntryStackGuarantee {
    pub const fn application(&self) -> &SymbolicTargetObservationApplication {
        &self.application
    }

    pub const fn guaranteed_available_bytes(&self) -> u64 {
        self.guaranteed_available_bytes
    }

    pub const fn required_alignment(&self) -> u64 {
        self.required_alignment
    }

    /// Deterministic report coordinate over the complete numeric closure. It
    /// is not authority and cannot replace exact field replay.
    pub const fn non_authoritative_compatibility_report_identity(&self) -> u64 {
        self.non_authoritative_compatibility_report_identity
    }

    /// Domain-separated commitment to the symbolic application and exact
    /// numeric target guarantee.
    pub const fn compatibility_commitment(&self) -> &[u8; 32] {
        &self.compatibility_commitment
    }

    pub fn matches_exact_uefi_x64_entry_stack_guarantee(&self) -> bool {
        TargetSemantics::close_guaranteed_entry_stack::<UefiX86_64>(TargetProfile::UefiX64)
            .is_ok_and(|expected| self == &expected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSemanticObservationError {
    projection: &'static str,
    subject: &'static str,
    selected_profile: TargetProfile,
    required_profile: TargetProfile,
}

impl TargetSemanticObservationError {
    pub const fn projection(&self) -> &'static str {
        self.projection
    }

    pub const fn subject(&self) -> &'static str {
        self.subject
    }

    pub const fn selected_profile(&self) -> TargetProfile {
        self.selected_profile
    }

    pub const fn required_profile(&self) -> TargetProfile {
        self.required_profile
    }
}

impl std::fmt::Display for TargetSemanticObservationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "target observation `{}<{}>()` requires profile `{}` but `{}` was selected",
            self.projection,
            self.subject,
            self.required_profile.target_name(),
            self.selected_profile.target_name(),
        )
    }
}

impl std::error::Error for TargetSemanticObservationError {}

/// Compiler-owned target-semantic projection namespace.
pub struct TargetSemantics;

impl TargetSemantics {
    /// Form the symbolic UEFI entry-stack guarantee application for one exact
    /// selected profile. This does not select a numeric bound and does not
    /// establish that a runtime firmware invocation conforms to the profile.
    pub fn guaranteed_entry_stack<Subject>(
        selected_profile: TargetProfile,
    ) -> Result<SymbolicTargetObservationApplication, TargetSemanticObservationError>
    where
        Subject: TargetEntryStackSubject,
    {
        if selected_profile != Subject::REQUIRED_PROFILE {
            return Err(TargetSemanticObservationError {
                projection: GUARANTEED_ENTRY_STACK_PROJECTION,
                subject: Subject::SUBJECT_IDENTITY,
                selected_profile,
                required_profile: Subject::REQUIRED_PROFILE,
            });
        }
        let non_authoritative_compatibility_report_identity =
            report_identity(Subject::SUBJECT_IDENTITY, selected_profile);
        let compatibility_commitment = commitment(Subject::SUBJECT_IDENTITY, selected_profile);
        Ok(SymbolicTargetObservationApplication {
            projection: GUARANTEED_ENTRY_STACK_PROJECTION,
            subject: Subject::SUBJECT_IDENTITY,
            projection_semantics_version: PROJECTION_SEMANTICS_VERSION,
            selected_profile,
            non_authoritative_compatibility_report_identity,
            compatibility_commitment,
        })
    }

    /// Close one symbolic application against its compiler-owned target
    /// subject definition. This selects a numeric target-contract guarantee;
    /// it does not establish runtime environment conformance.
    pub fn close_guaranteed_entry_stack<Subject>(
        selected_profile: TargetProfile,
    ) -> Result<TargetEntryStackGuarantee, TargetSemanticObservationError>
    where
        Subject: TargetEntryStackSubject,
    {
        let application = Self::guaranteed_entry_stack::<Subject>(selected_profile)?;
        let non_authoritative_compatibility_report_identity =
            closed_report_identity::<Subject>(&application);
        let compatibility_commitment = closed_commitment::<Subject>(&application);
        Ok(TargetEntryStackGuarantee {
            application,
            guaranteed_available_bytes: Subject::GUARANTEED_AVAILABLE_BYTES,
            required_alignment: Subject::REQUIRED_ALIGNMENT,
            non_authoritative_compatibility_report_identity,
            compatibility_commitment,
        })
    }
}

fn report_identity(subject: &str, profile: TargetProfile) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.target-semantics.observation-application.v1");
    hash.bytes(GUARANTEED_ENTRY_STACK_PROJECTION.as_bytes());
    hash.bytes(subject.as_bytes());
    hash.bytes(&PROJECTION_SEMANTICS_VERSION.to_le_bytes());
    hash.bytes(profile.identity().as_str().as_bytes());
    hash.finish()
}

fn commitment(subject: &str, profile: TargetProfile) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(
        &mut digest,
        b"omega.target-semantics.observation-application.v1",
    );
    hash_field(&mut digest, GUARANTEED_ENTRY_STACK_PROJECTION.as_bytes());
    hash_field(&mut digest, subject.as_bytes());
    hash_field(&mut digest, &PROJECTION_SEMANTICS_VERSION.to_le_bytes());
    hash_field(&mut digest, profile.identity().as_str().as_bytes());
    digest.finalize().into()
}

fn closed_report_identity<Subject>(application: &SymbolicTargetObservationApplication) -> u64
where
    Subject: TargetEntryStackSubject,
{
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.target-semantics.closed-entry-stack-guarantee.v1");
    hash.bytes(
        &application
            .non_authoritative_compatibility_report_identity
            .to_le_bytes(),
    );
    hash.bytes(&Subject::GUARANTEED_AVAILABLE_BYTES.to_le_bytes());
    hash.bytes(&Subject::REQUIRED_ALIGNMENT.to_le_bytes());
    hash.finish()
}

fn closed_commitment<Subject>(application: &SymbolicTargetObservationApplication) -> [u8; 32]
where
    Subject: TargetEntryStackSubject,
{
    let mut digest = Sha256::new();
    hash_field(
        &mut digest,
        b"omega.target-semantics.closed-entry-stack-guarantee.v1",
    );
    hash_field(&mut digest, application.compatibility_commitment());
    hash_field(
        &mut digest,
        &Subject::GUARANTEED_AVAILABLE_BYTES.to_le_bytes(),
    );
    hash_field(&mut digest, &Subject::REQUIRED_ALIGNMENT.to_le_bytes());
    digest.finalize().into()
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_uefi_application_retains_projection_subject_version_and_profile() {
        let application =
            TargetSemantics::guaranteed_entry_stack::<UefiX86_64>(TargetProfile::UefiX64).unwrap();
        assert_eq!(application.projection(), GUARANTEED_ENTRY_STACK_PROJECTION);
        assert_eq!(application.subject(), UEFI_X86_64_SUBJECT);
        assert_eq!(application.projection_semantics_version(), 1);
        assert_eq!(application.selected_profile(), TargetProfile::UefiX64);
        assert_ne!(
            application.non_authoritative_compatibility_report_identity(),
            0
        );
        assert_ne!(application.compatibility_commitment(), &[0; 32]);
        assert!(application.matches_exact_uefi_x64_entry_stack_application());
    }

    #[test]
    fn cross_target_application_rejects() {
        for profile in TargetProfile::ALL {
            let result = TargetSemantics::guaranteed_entry_stack::<UefiX86_64>(profile);
            assert_eq!(result.is_ok(), profile == TargetProfile::UefiX64);
            if let Err(error) = result {
                assert_eq!(error.selected_profile(), profile);
                assert_eq!(error.required_profile(), TargetProfile::UefiX64);
            }
        }
    }

    #[test]
    fn every_application_coordinate_tamper_fails_exact_replay() {
        let exact =
            TargetSemantics::guaranteed_entry_stack::<UefiX86_64>(TargetProfile::UefiX64).unwrap();

        let mut projection = exact.clone();
        projection.projection = "TargetSemantics::different_projection";
        assert!(!projection.matches_exact_uefi_x64_entry_stack_application());

        let mut subject = exact.clone();
        subject.subject = "DifferentSubject";
        assert!(!subject.matches_exact_uefi_x64_entry_stack_application());

        let mut version = exact.clone();
        version.projection_semantics_version += 1;
        assert!(!version.matches_exact_uefi_x64_entry_stack_application());

        let mut profile = exact.clone();
        profile.selected_profile = TargetProfile::WindowsX64;
        assert!(!profile.matches_exact_uefi_x64_entry_stack_application());

        let mut report = exact.clone();
        report.non_authoritative_compatibility_report_identity ^= 1;
        assert!(!report.matches_exact_uefi_x64_entry_stack_application());

        let mut commitment = exact;
        commitment.compatibility_commitment[0] ^= 1;
        assert!(!commitment.matches_exact_uefi_x64_entry_stack_application());
    }

    #[test]
    fn uefi_x64_target_closure_selects_the_normative_numeric_stack_guarantee() {
        let guarantee =
            TargetSemantics::close_guaranteed_entry_stack::<UefiX86_64>(TargetProfile::UefiX64)
                .unwrap();
        assert!(guarantee.matches_exact_uefi_x64_entry_stack_guarantee());
        assert_eq!(guarantee.guaranteed_available_bytes(), 128 * 1024);
        assert_eq!(guarantee.required_alignment(), 16);
        assert!(
            guarantee
                .application()
                .matches_exact_uefi_x64_entry_stack_application()
        );
        assert_ne!(
            guarantee.non_authoritative_compatibility_report_identity(),
            guarantee
                .application()
                .non_authoritative_compatibility_report_identity(),
        );
        assert_ne!(guarantee.compatibility_commitment(), &[0; 32]);
    }

    #[test]
    fn numeric_guarantee_rejects_cross_target_closure() {
        for profile in TargetProfile::ALL {
            let result = TargetSemantics::close_guaranteed_entry_stack::<UefiX86_64>(profile);
            assert_eq!(result.is_ok(), profile == TargetProfile::UefiX64);
        }
    }

    #[test]
    fn every_numeric_guarantee_coordinate_tamper_fails_exact_replay() {
        let exact =
            TargetSemantics::close_guaranteed_entry_stack::<UefiX86_64>(TargetProfile::UefiX64)
                .unwrap();

        let mut application = exact.clone();
        application.application.subject = "DifferentSubject";
        assert!(!application.matches_exact_uefi_x64_entry_stack_guarantee());

        let mut bytes = exact.clone();
        bytes.guaranteed_available_bytes += 1;
        assert!(!bytes.matches_exact_uefi_x64_entry_stack_guarantee());

        let mut alignment = exact.clone();
        alignment.required_alignment *= 2;
        assert!(!alignment.matches_exact_uefi_x64_entry_stack_guarantee());

        let mut report = exact.clone();
        report.non_authoritative_compatibility_report_identity ^= 1;
        assert!(!report.matches_exact_uefi_x64_entry_stack_guarantee());

        let mut commitment = exact;
        commitment.compatibility_commitment[0] ^= 1;
        assert!(!commitment.matches_exact_uefi_x64_entry_stack_guarantee());
    }
}
