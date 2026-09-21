//! The optional batch manifest over one staged multi-target invocation.
//!
//! The manifest binds only the explicit configured target set and each
//! child's commitment/outcome, in canonical configured order. It carries no
//! support, test, deployment-matrix, or completeness claim: a named row is
//! the exact outcome this invocation retained, not evidence that a target is
//! supported.

use diagnostics::{Diagnostic, DiagnosticSeverity};
use sha2::{Digest, Sha256};
use target::TargetProfile;

use crate::{ProductionArtifactIdentity, ProductionCompilationManifestIdentity};

const BATCH_MANIFEST_DOMAIN: &[u8] = b"OMEGA-BATCH-COMPILATION-MANIFEST-V1\0";
const BATCH_DIAGNOSTICS_DOMAIN: &[u8] = b"OMEGA-BATCH-CHILD-DIAGNOSTICS-V1\0";

/// Digest identifying one batch manifest's canonical bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BatchCompilationManifestIdentity([u8; 32]);

impl BatchCompilationManifestIdentity {
    /// Substitute a different claimed identity into a stored manifest so
    /// custody coverage can prove the digest is consulted on replay.
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub const fn for_test(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// What one succeeded child's retained report commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchChildCommitment {
    /// The child's own production manifest — the strongest package-aware
    /// custody, which already binds the produced artifact.
    Manifest(ProductionCompilationManifestIdentity),
    /// A produced artifact identity without package-aware production custody.
    Artifact(ProductionArtifactIdentity),
    /// A checking-only child commits no artifact.
    Checked,
}

/// One configured child's outcome bound by the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchChildOutcome {
    /// The child's report committed the named product.
    Succeeded { commitment: BatchChildCommitment },
    /// The child rejected; the row binds a digest over its exact ordered
    /// diagnostics rather than recording a bare failure flag.
    Rejected { diagnostics_digest: [u8; 32] },
}

impl BatchChildOutcome {
    /// A rejection row binds its ordered diagnostics; a child carrying no
    /// diagnostics has no outcome to bind and refuses construction.
    pub fn rejected(diagnostics: &[Diagnostic]) -> Result<Self, &'static str> {
        if diagnostics.is_empty() {
            return Err("a rejected batch child must carry its diagnostics");
        }
        Ok(Self::Rejected {
            diagnostics_digest: digest_diagnostics(diagnostics),
        })
    }
}

/// One row of the batch manifest: the exact configured target plus the
/// child's bound outcome. `target` is `None` only for the target-neutral
/// singleton configuration; multi-configuration batches name exact profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchChildRow {
    target: Option<TargetProfile>,
    outcome: BatchChildOutcome,
}

impl BatchChildRow {
    pub const fn new(target: Option<TargetProfile>, outcome: BatchChildOutcome) -> Self {
        Self { target, outcome }
    }

    pub const fn target(&self) -> Option<TargetProfile> {
        self.target
    }

    pub const fn outcome(&self) -> &BatchChildOutcome {
        &self.outcome
    }
}

/// The explicit-set binding over one invocation's children.
#[derive(Debug, Clone)]
pub struct BatchCompilationManifest {
    rows: Box<[BatchChildRow]>,
    canonical_bytes: Vec<u8>,
    identity: BatchCompilationManifestIdentity,
}

impl BatchCompilationManifest {
    /// Bind the explicit configured set in the order the invocation retained
    /// it. An empty set is not a batch and rejects.
    pub fn new(rows: Vec<BatchChildRow>) -> Result<Self, &'static str> {
        if rows.is_empty() {
            return Err("a batch manifest requires at least one child outcome");
        }
        let rows = rows.into_boxed_slice();
        let canonical_bytes = canonical_batch_manifest_bytes(&rows);
        let identity = BatchCompilationManifestIdentity(manifest_identity(&canonical_bytes));
        Ok(Self {
            rows,
            canonical_bytes,
            identity,
        })
    }

    /// Mint the honest manifest over arbitrary retained parts so custody
    /// coverage can drive stale or substituted rows through validate().
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn for_test(
        rows: Vec<BatchChildRow>,
        canonical_bytes: Vec<u8>,
        identity: BatchCompilationManifestIdentity,
    ) -> Self {
        Self {
            rows: rows.into_boxed_slice(),
            canonical_bytes,
            identity,
        }
    }

    pub const fn rows(&self) -> &[BatchChildRow] {
        &self.rows
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> BatchCompilationManifestIdentity {
        self.identity
    }

    /// Recompute the canonical bytes and identity from the retained rows; a
    /// manifest whose stored bytes or digest drifted does not validate.
    pub fn validate(&self) -> bool {
        let canonical = canonical_batch_manifest_bytes(&self.rows);
        if canonical != self.canonical_bytes {
            return false;
        }
        manifest_identity(&canonical) == self.identity.0
    }
}

fn manifest_identity(canonical_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(canonical_bytes);
    digest.finalize().into()
}

fn canonical_batch_manifest_bytes(rows: &[BatchChildRow]) -> Vec<u8> {
    let mut bytes = BATCH_MANIFEST_DOMAIN.to_vec();
    append_count(&mut bytes, rows.len());
    for row in rows {
        match row.target {
            Some(profile) => {
                bytes.push(1);
                append_field(&mut bytes, profile.target_name().as_bytes());
            }
            None => bytes.push(0),
        }
        match &row.outcome {
            BatchChildOutcome::Succeeded { commitment } => {
                bytes.push(0);
                match commitment {
                    BatchChildCommitment::Checked => bytes.push(0),
                    BatchChildCommitment::Artifact(artifact) => {
                        bytes.push(1);
                        append_artifact_identity(&mut bytes, *artifact);
                    }
                    BatchChildCommitment::Manifest(identity) => {
                        bytes.push(2);
                        bytes.extend_from_slice(identity.as_bytes());
                    }
                }
            }
            BatchChildOutcome::Rejected { diagnostics_digest } => {
                bytes.push(1);
                bytes.extend_from_slice(diagnostics_digest);
            }
        }
    }
    bytes
}

fn append_artifact_identity(bytes: &mut Vec<u8>, artifact: ProductionArtifactIdentity) {
    match artifact {
        ProductionArtifactIdentity::Terminal(identity) => {
            bytes.push(0);
            bytes.extend_from_slice(identity.as_bytes());
        }
        ProductionArtifactIdentity::Native(identity) => {
            bytes.push(1);
            bytes.extend_from_slice(identity.as_bytes());
        }
        ProductionArtifactIdentity::BuildOutputs(identity) => {
            bytes.push(2);
            bytes.extend_from_slice(&identity);
        }
    }
}

fn digest_diagnostics(diagnostics: &[Diagnostic]) -> [u8; 32] {
    let mut bytes = BATCH_DIAGNOSTICS_DOMAIN.to_vec();
    append_count(&mut bytes, diagnostics.len());
    for diagnostic in diagnostics {
        bytes.push(match diagnostic.severity {
            DiagnosticSeverity::Error => 0,
            DiagnosticSeverity::Warning => 1,
        });
        append_field(&mut bytes, diagnostic.message.as_bytes());
        match &diagnostic.source_span {
            Some(span) => {
                bytes.push(1);
                append_field(&mut bytes, format!("{span:?}").as_bytes());
            }
            None => bytes.push(0),
        }
    }
    let mut digest = Sha256::new();
    digest.update(&bytes);
    digest.finalize().into()
}

fn append_field(bytes: &mut Vec<u8>, field: &[u8]) {
    append_count(bytes, field.len());
    bytes.extend_from_slice(field);
}

fn append_count(bytes: &mut Vec<u8>, count: usize) {
    bytes.extend_from_slice(&(count as u64).to_le_bytes());
}

#[cfg(test)]
mod tests;
