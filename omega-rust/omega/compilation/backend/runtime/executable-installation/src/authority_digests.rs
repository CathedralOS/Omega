//! Domain-separated authority digests and report-only fingerprints.

use crate::installation::InstallationDiagnostic;

macro_rules! normalized_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, InstallationDiagnostic> {
                if identity == 0 {
                    return Err(InstallationDiagnostic(format!(
                        "normalized {} identity cannot be zero",
                        $label
                    )));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

normalized_id!(ArtifactId, "artifact");
normalized_id!(MachineContractSetId, "machine-contract-set");
normalized_id!(MachineFootprintId, "machine-footprint");
normalized_id!(AccessPlanId, "placement-plan");
normalized_id!(EntrySetId, "entry-set");
normalized_id!(AdmissionReceiptId, "admission-receipt");
normalized_id!(CodePlacementId, "code-placement");
normalized_id!(InstallationScopeId, "installation-scope");
normalized_id!(FinalValidationId, "final-validation");
normalized_id!(InstalledCodeId, "installed-code");
normalized_id!(MappingQuarantineId, "mapping-quarantine");
normalized_id!(RelocationSetId, "relocation-set");
normalized_id!(
    DestinationPreparationReceiptId,
    "destination-preparation-receipt"
);

macro_rules! normalized_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn digest(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

normalized_digest!(ArtifactContentDigest);
normalized_digest!(FinalBytesDigest);
normalized_digest!(EntryContractDigest);
normalized_digest!(EntryReferenceFactDigest);
normalized_digest!(InstallationFactDigest);
normalized_digest!(ReplacementFactDigest);
normalized_digest!(RetirementFactDigest);

macro_rules! canonical_authority_digest {
    ($name:ident, $domain:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

canonical_authority_digest!(
    ImportedContractSetDigest,
    b"omega.imported-contract-set.sha256.v1\0"
);
canonical_authority_digest!(
    DeclaredFootprintDigest,
    b"omega.declared-machine-footprint.sha256.v1\0"
);
canonical_authority_digest!(MachineRegimeDigest, b"omega.machine-regime.sha256.v1\0");
canonical_authority_digest!(
    InstallationScopeDigest,
    b"omega.artifact-installation-scope.sha256.v1\0"
);

/// Collision-resistant commitments to the exact authority-bearing values
/// imported by an executable artifact. The compact normalized identities
/// remain report coordinates and are included in each digest's domain-framed
/// preimage; they are never sufficient on their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ArtifactAuthorityCommitments {
    imported_contract_report_identity: u64,
    imported_contracts: ImportedContractSetDigest,
    declared_footprint_report_identity: u64,
    declared_footprint: DeclaredFootprintDigest,
    machine_regime_report_identity: u64,
    machine_regime: MachineRegimeDigest,
    installation_scope_report_identity: u64,
    installation_scope: InstallationScopeDigest,
}

impl InstallationFactDigest {}

impl ReplacementFactDigest {}

impl EntryContractDigest {}

impl EntryReferenceFactDigest {}

impl RetirementFactDigest {}

macro_rules! non_authoritative_fingerprint64 {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_compatibility_value(value: u64) -> Result<Self, InstallationDiagnostic> {
                if value == 0 {
                    return Err(InstallationDiagnostic(format!(
                        "{} compatibility value cannot be zero",
                        $label
                    )));
                }
                Ok(Self(value))
            }

            pub const fn compatibility_value(self) -> u64 {
                self.0
            }
        }
    };
}

non_authoritative_fingerprint64!(
    NonAuthoritativeContainerFingerprint64,
    "container-v1 compatibility"
);
non_authoritative_fingerprint64!(
    NonAuthoritativeInformationalFingerprint64,
    "informational-section"
);
non_authoritative_fingerprint64!(
    NonAuthoritativeWriterContextFingerprint64,
    "writer-context replay"
);
