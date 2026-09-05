use super::limits::ORDINARY_PACKAGE_OBLIGATION_SCHEMA_VERSION;
use crate::record::{PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk};
use package_compilation::PackageDependencyClosure;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

/// Exact semantic vocabulary under which the ordinary obligation question was
/// reconstructed. This is intentionally distinct from both the outer ledger
/// codec and the package-review row encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrdinaryPackageObligationSchemaIdentity {
    version: u16,
}

impl OrdinaryPackageObligationSchemaIdentity {
    pub const fn current() -> Self {
        Self {
            version: ORDINARY_PACKAGE_OBLIGATION_SCHEMA_VERSION,
        }
    }

    pub const fn version(self) -> u16 {
        self.version
    }

    pub(super) fn decode(
        version: u16,
    ) -> Result<Self, OrdinaryPackageObligationLedgerRecoveryError> {
        if version != ORDINARY_PACKAGE_OBLIGATION_SCHEMA_VERSION {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "unsupported ordinary package obligation schema",
            ));
        }
        Ok(Self { version })
    }
}

/// Collision-resistant identity of one canonical ordinary obligation ledger.
/// It identifies a replay question; it is not a discharge result or package
/// admission.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrdinaryPackageObligationLedgerFingerprint([u8; 32]);

impl OrdinaryPackageObligationLedgerFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub(super) const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }
}

impl std::fmt::Debug for OrdinaryPackageObligationLedgerFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for OrdinaryPackageObligationLedgerFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// One source-handle-free semantic row in the current ordinary package-review
/// vocabulary. Explanatory source coordinates and compiler derivation notes are
/// deliberately separate provenance and do not enter ledger equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageObligationRow {
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    key_bytes: Vec<u8>,
    canonical_bytes: Vec<u8>,
}

impl OrdinaryPackageObligationRow {
    pub const fn kind(&self) -> PackageReviewCanonicalRowKind {
        self.kind
    }

    pub const fn risk(&self) -> PackageReviewCanonicalRowRisk {
        self.risk
    }

    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub(super) fn from_parts(
        kind: PackageReviewCanonicalRowKind,
        risk: PackageReviewCanonicalRowRisk,
        key_bytes: Vec<u8>,
        canonical_bytes: Vec<u8>,
    ) -> Self {
        Self {
            kind,
            risk,
            key_bytes,
            canonical_bytes,
        }
    }
}

/// Complete locally ordered row set for the current ordinary package-review
/// vocabulary under one exact package, target, and dependency closure.
///
/// This is not yet accepted package evidence: exact source/artifact subjects,
/// certificates, transitive open obligations, schema migration, and local
/// admission decisions remain separate unfinished joins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageObligationLedger {
    schema: OrdinaryPackageObligationSchemaIdentity,
    package: PackageKeyIdentity,
    target: TargetProfile,
    dependency_closure: PackageDependencyClosure,
    rows: Vec<OrdinaryPackageObligationRow>,
}

impl OrdinaryPackageObligationLedger {
    pub const fn schema(&self) -> OrdinaryPackageObligationSchemaIdentity {
        self.schema
    }

    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub const fn dependency_closure(&self) -> &PackageDependencyClosure {
        &self.dependency_closure
    }

    pub fn rows(&self) -> &[OrdinaryPackageObligationRow] {
        &self.rows
    }

    pub(super) fn from_parts(
        schema: OrdinaryPackageObligationSchemaIdentity,
        package: PackageKeyIdentity,
        target: TargetProfile,
        dependency_closure: PackageDependencyClosure,
        rows: Vec<OrdinaryPackageObligationRow>,
    ) -> Self {
        Self {
            schema,
            package,
            target,
            dependency_closure,
            rows,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageObligationLedgerRecoveryError {
    message: &'static str,
}

impl OrdinaryPackageObligationLedgerRecoveryError {
    pub(super) const fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for OrdinaryPackageObligationLedgerRecoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for OrdinaryPackageObligationLedgerRecoveryError {}
