//! The canonical little-endian byte layout that binds one production to its
//! package, build machine, target and artifact, hashed under a versioned domain
//! so adding a field is a new domain rather than a silent reinterpretation.

use native_artifact::{
    DynamicElfNativeArtifact, DynamicElfNativeArtifactIdentity, NativeArtifact,
    NativeArtifactIdentity, NativePhysicalEvidence, NativePhysicalEvidenceGap,
};
use package_compilation::ProductionCompilationSubject;
use sha2::{Digest, Sha256};
use terminal_codec::{CanonicalTerminalArtifact, TerminalArtifactIdentity};

const MANIFEST_DOMAIN: &[u8] = b"OMEGA-PRODUCTION-COMPILATION-MANIFEST-V11\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProductionCompilationManifestIdentity([u8; 32]);

impl ProductionCompilationManifestIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionArtifactIdentity {
    Terminal(TerminalArtifactIdentity),
    Native(NativeArtifactIdentity),
    BuildOutputs([u8; 32]),
    /// A dynamically linked ELF product. Its identity domain is separate from
    /// [`Self::Native`], so equal bytes never collapse the two image classes.
    DynamicElf(DynamicElfNativeArtifactIdentity),
}

/// Fail-closed rejection from the narrow package/native physical-evidence
/// join. This does not claim that a package was audited or that the complete
/// executable is correct; it only admits the exact native physical evidence
/// already carried by the matching artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalRealizationEvidenceError {
    InvalidReportCustody,
    RetainedNativeArtifactRequired,
    PackageProductionManifestRequired,
    InvalidProductionManifest,
    InvalidNativeArtifact,
    NativeTargetMismatch,
    NativeArtifactMismatch,
    NativePhysicalEvidenceUnavailable,
    /// The artifact's scoped derivation stopped at one exact subject, which
    /// the payload names. Consumers learn which occurrence or retained record
    /// failed instead of receiving a bare absence.
    NativePhysicalEvidenceBlocked(NativePhysicalEvidenceGap),
}

impl std::fmt::Display for FinalRealizationEvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NativePhysicalEvidenceBlocked(gap) => {
                write!(
                    formatter,
                    "native artifact physical evidence stops at {gap}"
                )
            }
            Self::InvalidReportCustody => {
                formatter.write_str("compiler report has inconsistent production custody")
            }
            Self::RetainedNativeArtifactRequired => formatter
                .write_str("final-realization evidence requires a retained native artifact"),
            Self::PackageProductionManifestRequired => {
                formatter.write_str("final-realization evidence requires package-aware production")
            }
            Self::InvalidProductionManifest => {
                formatter.write_str("production compilation manifest is invalid")
            }
            Self::InvalidNativeArtifact => formatter.write_str("native artifact is invalid"),
            Self::NativeTargetMismatch => formatter
                .write_str("production compilation manifest and native artifact target disagree"),
            Self::NativeArtifactMismatch => formatter
                .write_str("production compilation manifest and native artifact identity disagree"),
            Self::NativePhysicalEvidenceUnavailable => formatter
                .write_str("native artifact carries no physical evidence for this realization"),
        }
    }
}

impl std::error::Error for FinalRealizationEvidenceError {}

/// One canonical package-source/build/target/artifact join retained by the
/// real production report. Human views may decode this authority; they cannot
/// define a parallel schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionCompilationManifest {
    subject: ProductionCompilationSubject,
    artifact: ProductionArtifactIdentity,
    canonical_bytes: Vec<u8>,
    identity: ProductionCompilationManifestIdentity,
}

impl ProductionCompilationManifest {
    pub fn for_build_outputs(
        subject: ProductionCompilationSubject,
        outputs: &crate::RetainedBuildOutputs,
    ) -> Result<Self, &'static str> {
        if subject.build_observation_identity() != outputs.observation_identity() {
            return Err("completed build outputs belong to another production observation");
        }
        Ok(Self::new(
            subject,
            ProductionArtifactIdentity::BuildOutputs(*outputs.identity()),
        ))
    }

    pub fn for_terminal(
        subject: ProductionCompilationSubject,
        artifact: &CanonicalTerminalArtifact,
    ) -> Result<Self, &'static str> {
        artifact
            .validate()
            .map_err(|_| "production manifest received invalid Terminal artifact")?;
        Ok(Self::new(
            subject,
            ProductionArtifactIdentity::Terminal(artifact.manifest().identity()),
        ))
    }

    pub fn for_native(
        subject: ProductionCompilationSubject,
        artifact: &NativeArtifact,
    ) -> Result<Self, &'static str> {
        artifact
            .validate()
            .map_err(|_| "production manifest received invalid native artifact")?;
        if subject.native_target() != artifact.target() {
            return Err("production manifest subject target disagrees with native artifact");
        }
        Ok(Self::new(
            subject,
            ProductionArtifactIdentity::Native(artifact.identity()),
        ))
    }

    pub fn for_dynamic_elf(
        subject: ProductionCompilationSubject,
        artifact: &DynamicElfNativeArtifact,
    ) -> Result<Self, &'static str> {
        artifact
            .validate()
            .map_err(|_| "production manifest received invalid dynamic ELF artifact")?;
        if subject.native_target() != artifact.target() {
            return Err("production manifest subject target disagrees with dynamic ELF artifact");
        }
        Ok(Self::new(
            subject,
            ProductionArtifactIdentity::DynamicElf(artifact.identity()),
        ))
    }

    fn new(subject: ProductionCompilationSubject, artifact: ProductionArtifactIdentity) -> Self {
        let canonical_bytes = canonical_manifest_bytes(&subject, artifact);
        let mut digest = Sha256::new();
        digest.update(&canonical_bytes);
        Self {
            subject,
            artifact,
            canonical_bytes,
            identity: ProductionCompilationManifestIdentity(digest.finalize().into()),
        }
    }

    pub const fn subject(&self) -> &ProductionCompilationSubject {
        &self.subject
    }

    pub const fn artifact(&self) -> ProductionArtifactIdentity {
        self.artifact
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> ProductionCompilationManifestIdentity {
        self.identity
    }

    pub fn validate(&self) -> bool {
        let canonical = canonical_manifest_bytes(&self.subject, self.artifact);
        if canonical != self.canonical_bytes {
            return false;
        }
        let mut digest = Sha256::new();
        digest.update(&canonical);
        self.identity == ProductionCompilationManifestIdentity(digest.finalize().into())
    }

    pub fn matches_terminal_artifact(&self, artifact: &CanonicalTerminalArtifact) -> bool {
        matches!(
            self.artifact,
            ProductionArtifactIdentity::Terminal(identity)
                if identity == artifact.manifest().identity()
        )
    }

    pub fn matches_native_artifact(&self, artifact: &NativeArtifact) -> bool {
        matches!(
            self.artifact,
            ProductionArtifactIdentity::Native(identity) if identity == artifact.identity()
        ) && self.subject.native_target() == artifact.target()
    }

    pub fn matches_dynamic_elf_artifact(&self, artifact: &DynamicElfNativeArtifact) -> bool {
        matches!(
            self.artifact,
            ProductionArtifactIdentity::DynamicElf(identity) if identity == artifact.identity()
        ) && self.subject.native_target() == artifact.target()
    }

    /// Borrow the exact physical evidence already owned by this manifest's
    /// native artifact. No receipt, digest proxy, or blanket Terminal-complete
    /// bit is minted here.
    pub fn require_native_physical_evidence<'artifact>(
        &self,
        artifact: &'artifact NativeArtifact,
    ) -> Result<&'artifact NativePhysicalEvidence, FinalRealizationEvidenceError> {
        if !self.validate() {
            return Err(FinalRealizationEvidenceError::InvalidProductionManifest);
        }
        artifact
            .validate()
            .map_err(|_| FinalRealizationEvidenceError::InvalidNativeArtifact)?;
        if self.subject.native_target() != artifact.target() {
            return Err(FinalRealizationEvidenceError::NativeTargetMismatch);
        }
        if !matches!(
            self.artifact,
            ProductionArtifactIdentity::Native(identity) if identity == artifact.identity()
        ) {
            return Err(FinalRealizationEvidenceError::NativeArtifactMismatch);
        }
        artifact.physical_evidence().ok_or_else(|| {
            artifact.physical_evidence_gap().map_or(
                FinalRealizationEvidenceError::NativePhysicalEvidenceUnavailable,
                |gap| FinalRealizationEvidenceError::NativePhysicalEvidenceBlocked(*gap),
            )
        })
    }
}

fn canonical_manifest_bytes(
    subject: &ProductionCompilationSubject,
    artifact: ProductionArtifactIdentity,
) -> Vec<u8> {
    let mut bytes = MANIFEST_DOMAIN.to_vec();
    subject.append_canonical_bytes(&mut bytes);
    match artifact {
        ProductionArtifactIdentity::Terminal(identity) => {
            bytes.push(1);
            bytes.extend_from_slice(identity.as_bytes());
        }
        ProductionArtifactIdentity::Native(identity) => {
            bytes.push(2);
            bytes.extend_from_slice(identity.as_bytes());
        }
        ProductionArtifactIdentity::BuildOutputs(identity) => {
            bytes.push(3);
            bytes.extend_from_slice(&identity);
        }
        ProductionArtifactIdentity::DynamicElf(identity) => {
            bytes.push(4);
            bytes.extend_from_slice(identity.as_bytes());
        }
    }
    bytes
}
