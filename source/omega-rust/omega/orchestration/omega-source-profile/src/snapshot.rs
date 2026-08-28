use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const SOURCE_CLOSURE_SNAPSHOT_SCHEMA: &str = "omega.source-closure-snapshot.v4";

/// Exact package-resolution custody joined to a source-discovery observation.
///
/// The canonical bytes are produced by the package system's bounded
/// `CanonicalSourceClosureSubject`. Retaining them here makes the package graph
/// reconstructible instead of reducing it to a fingerprint. This remains a
/// diagnostic question, not an accepted lock, package instance, or admission
/// result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackageSourceClosureCustodySnapshot {
    pub subject_encoding_version: u16,
    pub subject_fingerprint: String,
    pub canonical_subject_bytes_hex: String,
}

/// Domain-separated identity of the exact source-discovery question observed
/// by this snapshot. It commits custody and source bytes, not compiler
/// correctness, package admission, or an emitted artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceClosureSnapshotFingerprint([u8; 32]);

impl SourceClosureSnapshotFingerprint {
    pub fn to_hex(&self) -> String {
        encode_hex(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceClosureSnapshotEntry {
    pub source_id: usize,
    pub identity: String,
    /// Exact reconciled package identity for package-authored source. Toolchain
    /// and unmanaged standalone source have no package identity.
    pub package_identity: Option<String>,
    /// Canonical path beneath the reconciled package root. This remains
    /// independent from the human-facing repository identity above.
    pub package_relative_path: Option<String>,
    pub origin: &'static str,
    pub byte_length: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceClosureSnapshot {
    pub schema: &'static str,
    pub entry_source: String,
    /// Present only when the observer resolved the build root through package
    /// custody. A companion-free focused compilation remains standalone.
    pub package_source_closure: Option<PackageSourceClosureCustodySnapshot>,
    pub selected_target: Option<String>,
    pub native_provider_substitution: bool,
    pub sources: Vec<SourceClosureSnapshotEntry>,
    pub syntax: psi_syntax_trees::SyntaxTreesSnapshot,
}

impl SourceClosureSnapshot {
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn feature_census(&self) -> crate::SourceFeatureCensus {
        crate::census_source_closure(self)
    }

    pub fn fingerprint(&self) -> SourceClosureSnapshotFingerprint {
        let mut digest = Sha256::new();
        hash_field(&mut digest, b"OMEGA-SOURCE-CLOSURE-SNAPSHOT\0");
        hash_field(&mut digest, self.schema.as_bytes());
        hash_field(&mut digest, self.entry_source.as_bytes());
        match &self.package_source_closure {
            Some(custody) => {
                digest.update([1]);
                digest.update(custody.subject_encoding_version.to_le_bytes());
                hash_field(&mut digest, custody.subject_fingerprint.as_bytes());
                hash_field(&mut digest, custody.canonical_subject_bytes_hex.as_bytes());
            }
            None => digest.update([0]),
        }
        match &self.selected_target {
            Some(target) => {
                digest.update([1]);
                hash_field(&mut digest, target.as_bytes());
            }
            None => digest.update([0]),
        }
        digest.update([u8::from(self.native_provider_substitution)]);
        digest.update(
            u64::try_from(self.sources.len())
                .expect("source closure count fits u64")
                .to_le_bytes(),
        );
        for source in &self.sources {
            hash_field(&mut digest, source.identity.as_bytes());
            hash_optional_field(
                &mut digest,
                source.package_identity.as_deref().map(str::as_bytes),
            );
            hash_optional_field(
                &mut digest,
                source.package_relative_path.as_deref().map(str::as_bytes),
            );
            hash_field(&mut digest, source.origin.as_bytes());
            digest.update(
                u64::try_from(source.byte_length)
                    .expect("source byte length fits u64")
                    .to_le_bytes(),
            );
            hash_field(&mut digest, source.sha256.as_bytes());
        }
        SourceClosureSnapshotFingerprint(digest.finalize().into())
    }
}

fn hash_optional_field(digest: &mut Sha256, bytes: Option<&[u8]>) {
    match bytes {
        Some(bytes) => {
            digest.update([1]);
            hash_field(digest, bytes);
        }
        None => digest.update([0]),
    }
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("source closure field length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

/// An immutable source root and the logical identity used in snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInspectionRoot {
    physical_root: PathBuf,
    logical_root: PathBuf,
}

impl SourceInspectionRoot {
    pub fn new(physical_root: impl Into<PathBuf>, logical_root: impl Into<PathBuf>) -> Self {
        Self {
            physical_root: physical_root.into(),
            logical_root: logical_root.into(),
        }
    }

    pub fn physical_root(&self) -> &Path {
        &self.physical_root
    }

    pub fn logical_root(&self) -> &Path {
        &self.logical_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> SourceClosureSnapshot {
        SourceClosureSnapshot {
            schema: SOURCE_CLOSURE_SNAPSHOT_SCHEMA,
            entry_source: "source/omega/main.omg".to_owned(),
            package_source_closure: Some(PackageSourceClosureCustodySnapshot {
                subject_encoding_version: 1,
                subject_fingerprint: "11".repeat(32),
                canonical_subject_bytes_hex: "010203".to_owned(),
            }),
            selected_target: Some("linux_x64".to_owned()),
            native_provider_substitution: false,
            sources: vec![SourceClosureSnapshotEntry {
                source_id: 7,
                identity: format!("package:{}/main.omg", "22".repeat(32)),
                package_identity: Some("22".repeat(32)),
                package_relative_path: Some("main.omg".to_owned()),
                origin: "repository",
                byte_length: 15,
                sha256: "33".repeat(32),
            }],
            syntax: psi_syntax_trees::SyntaxTrees::default().snapshot(),
        }
    }

    #[test]
    fn fingerprint_binds_package_custody_and_loaded_source_content() {
        let baseline = snapshot();
        let mut changed_package = baseline.clone();
        changed_package
            .package_source_closure
            .as_mut()
            .expect("package custody")
            .subject_fingerprint = "44".repeat(32);
        let mut changed_source = baseline.clone();
        changed_source.sources[0].sha256 = "55".repeat(32);

        assert_ne!(baseline.fingerprint(), changed_package.fingerprint());
        assert_ne!(baseline.fingerprint(), changed_source.fingerprint());
    }

    #[test]
    fn feature_census_names_its_exact_source_closure() {
        let snapshot = snapshot();
        let census = snapshot.feature_census();

        assert_eq!(
            census.source_closure_fingerprint,
            snapshot.fingerprint().to_hex()
        );
        assert_eq!(
            census.package_source_closure_fingerprint,
            snapshot
                .package_source_closure
                .as_ref()
                .map(|custody| custody.subject_fingerprint.clone())
        );
    }
}
