//! Domain-separated authority digests and report-only fingerprints.

use crate::executable_installation::InstallationDiagnostic;
use layout_plans::PlacementConstraints;
use sha2::Digest;
use sha2::Sha256;

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
normalized_id!(PlacementPlanId, "placement-plan");
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
            pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn digest(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

normalized_digest!(ArtifactContentDigest);
normalized_digest!(ProofPayloadDigest);
normalized_digest!(FinalBytesDigest);
normalized_digest!(InstallationFactDigest);
normalized_digest!(RetirementFactDigest);

macro_rules! canonical_authority_digest {
    ($name:ident, $domain:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            fn from_report_identity_and_canonical_bytes(
                report_identity: u64,
                canonical: &[u8],
            ) -> Self {
                let mut digest = Sha256::new();
                digest.update($domain);
                digest.update(report_identity.to_le_bytes());
                digest.update((canonical.len() as u64).to_le_bytes());
                digest.update(canonical);
                Self(digest.finalize().into())
            }

            pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

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
pub struct ArtifactAuthorityCommitments {
    imported_contract_report_identity: u64,
    imported_contracts: ImportedContractSetDigest,
    declared_footprint_report_identity: u64,
    declared_footprint: DeclaredFootprintDigest,
    machine_regime_report_identity: u64,
    machine_regime: MachineRegimeDigest,
    installation_scope_report_identity: u64,
    installation_scope: InstallationScopeDigest,
}

impl ArtifactAuthorityCommitments {
    #[allow(clippy::too_many_arguments)]
    pub fn from_canonical_evidence(
        contracts: MachineContractSetId,
        contract_bytes: &[u8],
        footprint: MachineFootprintId,
        footprint_bytes: &[u8],
        regime: Option<(layout_plans::MachineRegimeId, &[u8])>,
        scope: Option<(layout_plans::ArtifactInstallationScopeId, &[u8])>,
    ) -> Self {
        let (regime_identity, regime_bytes) = regime
            .map(|(identity, bytes)| (identity.normalized_identity(), bytes))
            .unwrap_or((0, &[]));
        let (scope_identity, scope_bytes) = scope
            .map(|(identity, bytes)| (identity.normalized_identity(), bytes))
            .unwrap_or((0, &[]));
        Self {
            imported_contract_report_identity: contracts.normalized_identity(),
            imported_contracts: ImportedContractSetDigest::from_report_identity_and_canonical_bytes(
                contracts.normalized_identity(),
                contract_bytes,
            ),
            declared_footprint_report_identity: footprint.normalized_identity(),
            declared_footprint: DeclaredFootprintDigest::from_report_identity_and_canonical_bytes(
                footprint.normalized_identity(),
                footprint_bytes,
            ),
            machine_regime_report_identity: regime_identity,
            machine_regime: MachineRegimeDigest::from_report_identity_and_canonical_bytes(
                regime_identity,
                regime_bytes,
            ),
            installation_scope_report_identity: scope_identity,
            installation_scope: InstallationScopeDigest::from_report_identity_and_canonical_bytes(
                scope_identity,
                scope_bytes,
            ),
        }
    }

    pub const fn imported_contracts(&self) -> ImportedContractSetDigest {
        self.imported_contracts
    }

    pub const fn declared_footprint(&self) -> DeclaredFootprintDigest {
        self.declared_footprint
    }

    pub const fn machine_regime(&self) -> MachineRegimeDigest {
        self.machine_regime
    }

    pub const fn installation_scope(&self) -> InstallationScopeDigest {
        self.installation_scope
    }

    pub(crate) fn from_decoded_digests(
        contracts: MachineContractSetId,
        footprint: MachineFootprintId,
        placement: PlacementConstraints,
        imported_contracts: [u8; 32],
        declared_footprint: [u8; 32],
        machine_regime: [u8; 32],
        installation_scope: [u8; 32],
    ) -> Result<Self, InstallationDiagnostic> {
        if [
            imported_contracts,
            declared_footprint,
            machine_regime,
            installation_scope,
        ]
        .contains(&[0; 32])
        {
            return Err(InstallationDiagnostic(
                "executable-container authority commitments cannot be zero".into(),
            ));
        }
        Ok(Self {
            imported_contract_report_identity: contracts.normalized_identity(),
            imported_contracts: ImportedContractSetDigest::from_digest(imported_contracts),
            declared_footprint_report_identity: footprint.normalized_identity(),
            declared_footprint: DeclaredFootprintDigest::from_digest(declared_footprint),
            machine_regime_report_identity: placement
                .machine_regime()
                .map_or(0, |identity| identity.normalized_identity()),
            machine_regime: MachineRegimeDigest::from_digest(machine_regime),
            installation_scope_report_identity: placement
                .installation_scope()
                .map_or(0, |identity| identity.normalized_identity()),
            installation_scope: InstallationScopeDigest::from_digest(installation_scope),
        })
    }

    pub(crate) fn matches_report_coordinates(
        &self,
        contracts: MachineContractSetId,
        footprint: MachineFootprintId,
        placement: PlacementConstraints,
    ) -> bool {
        self.imported_contract_report_identity == contracts.normalized_identity()
            && self.declared_footprint_report_identity == footprint.normalized_identity()
            && self.machine_regime_report_identity
                == placement
                    .machine_regime()
                    .map_or(0, |identity| identity.normalized_identity())
            && self.installation_scope_report_identity
                == placement
                    .installation_scope()
                    .map_or(0, |identity| identity.normalized_identity())
    }
}

impl InstallationFactDigest {
    /// Derive one provider-defined installation fact from its canonical bytes.
    ///
    /// Install gates compare this complete domain-separated digest rather
    /// than a compact provider-selected integer. The canonical bytes remain
    /// provider vocabulary — a receiver can demand, for example, the
    /// provider's exact cache-order or instruction-fetch completion fact —
    /// and this layer assigns them no ambient meaning.
    pub fn from_canonical_bytes(canonical: &[u8]) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.installation-fact.sha256.v1\0");
        digest.update(
            u64::try_from(canonical.len())
                .expect("installation-fact canonical byte length fits u64")
                .to_le_bytes(),
        );
        digest.update(canonical);
        Self::from_digest(digest.finalize().into())
    }
}

impl RetirementFactDigest {
    /// Derive one provider-defined completion fact from its canonical bytes.
    ///
    /// Retirement gates compare this complete domain-separated digest rather
    /// than a compact provider-selected integer. The canonical bytes remain
    /// provider vocabulary; this layer assigns them no ambient meaning.
    pub fn from_canonical_bytes(canonical: &[u8]) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.retirement-fact.sha256.v1\0");
        digest.update(
            u64::try_from(canonical.len())
                .expect("retirement-fact canonical byte length fits u64")
                .to_le_bytes(),
        );
        digest.update(canonical);
        Self::from_digest(digest.finalize().into())
    }
}

macro_rules! non_authoritative_fingerprint64 {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_compatibility_value(value: u64) -> Result<Self, InstallationDiagnostic> {
                if value == 0 {
                    return Err(InstallationDiagnostic(format!(
                        "non-authoritative {} fingerprint cannot be zero",
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
