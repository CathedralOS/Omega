//! The key a compiled-review record is filed under, and the provenance
//! digests a record stores so a lookup can prove it was filed for the live
//! package key and source resolution.

use super::record_codec::purpose_tag;
use crate::declarations::DependencyPurpose;
use package_compilation::{
    PackageCompilationInputs, review_cache_inputs_digest, review_cache_snapshot_digest,
};
use package_source::{ImmutableSourceResolution, SourceLineage};
use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};
use std::path::Path;

const RECORD_KEY_DOMAIN: &[u8] = b"OMEGA-COMPILED-REVIEW-CACHE-KEY-V2\0";

/// The compiler toolchain stamp folded into every record key. Release builds
/// stamp their exact version and commit; dev builds stamp the package
/// version plus `unrecorded` — records remain self-verifying either way.
pub(super) fn stamp_string() -> String {
    let version =
        option_env!("OMEGA_RELEASE_VERSION").unwrap_or(concat!(env!("CARGO_PKG_VERSION"), "-dev"));
    let commit = option_env!("OMEGA_RELEASE_COMMIT").unwrap_or("unrecorded");
    format!("{version}|{commit}")
}

/// The SHA-256 key over everything a persisted review depends on: package
/// identity, occurrence purpose, the checked context (target plus admitted
/// build profile), the checked entry path, the toolchain that minted the
/// record, and the content digests of the complete compilation inputs and
/// the build snapshot.
pub(crate) fn compiled_review_key(
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    checked_target: target::TargetProfile,
    build_execution_profile: Option<target::TargetProfile>,
    entry: &Path,
    inputs: &PackageCompilationInputs,
    snapshot: &build_evaluation::BuildSnapshotRequest,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(RECORD_KEY_DOMAIN);
    digest.update(package.digest());
    digest.update([purpose_tag(purpose)]);
    hash_bytes(&mut digest, checked_target.target_name().as_bytes());
    match build_execution_profile {
        Some(profile) => {
            digest.update([1]);
            hash_bytes(&mut digest, profile.target_name().as_bytes());
        }
        None => digest.update([0]),
    }
    hash_bytes(&mut digest, entry.as_os_str().as_encoded_bytes());
    hash_bytes(&mut digest, stamp_string().as_bytes());
    digest.update(review_cache_inputs_digest(inputs));
    digest.update(review_cache_snapshot_digest(snapshot));
    digest.finalize().into()
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

pub(super) fn lineage_digest(lineage: &SourceLineage) -> [u8; 32] {
    let mut digest = Sha256::new();
    lineage.hash_canonical(&mut digest);
    digest.finalize().into()
}

pub(super) fn resolution_digest(resolution: &ImmutableSourceResolution) -> [u8; 32] {
    let mut digest = Sha256::new();
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            digest.update([0]);
            hash_bytes(&mut digest, commit.to_hex().as_bytes());
            hash_bytes(&mut digest, tree.to_hex().as_bytes());
            digest.update(content.digest());
        }
        ImmutableSourceResolution::Workspace { content } => {
            digest.update([1]);
            digest.update(content.digest());
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            digest.update([2]);
            digest.update(content.digest());
        }
    }
    digest.finalize().into()
}
