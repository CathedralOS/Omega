//! The key a compiled-review record is filed under, and the provenance
//! digests a record stores so a lookup can prove it was filed for the live
//! package key and source resolution.

use crate::declarations::DependencyPurpose;
use package_compilation::review_record_encoding::{hash_framed, purpose_tag};
use package_compilation::{
    PackageCompilationInputs, review_cache_inputs_digest, review_cache_snapshot_digest,
};
use package_source::{ImmutableSourceResolution, SourceLineage};
use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};
use std::path::Path;

const RECORD_KEY_DOMAIN: &[u8] = b"OMEGA-COMPILED-REVIEW-CACHE-KEY-V3\0";

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
) -> Option<[u8; 32]> {
    // A cache is optional. Failure to identify exact compiler and injected
    // library inputs is a miss, never a weaker release/developer-name key.
    let compiler_identity = crate::operations::compiler_input_identity().ok()?;
    Some(compiled_review_key_with_identity(
        package,
        purpose,
        checked_target,
        build_execution_profile,
        entry,
        inputs,
        snapshot,
        compiler_identity,
    ))
}

pub(super) fn compiled_review_key_with_identity(
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    checked_target: target::TargetProfile,
    build_execution_profile: Option<target::TargetProfile>,
    entry: &Path,
    inputs: &PackageCompilationInputs,
    snapshot: &build_evaluation::BuildSnapshotRequest,
    compiler_identity: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(RECORD_KEY_DOMAIN);
    digest.update(package.digest());
    digest.update([purpose_tag(purpose)]);
    hash_framed(&mut digest, checked_target.target_name().as_bytes());
    match build_execution_profile {
        Some(profile) => {
            digest.update([1]);
            hash_framed(&mut digest, profile.target_name().as_bytes());
        }
        None => digest.update([0]),
    }
    hash_framed(&mut digest, entry.as_os_str().as_encoded_bytes());
    digest.update(compiler_identity);
    digest.update(review_cache_inputs_digest(inputs));
    digest.update(review_cache_snapshot_digest(snapshot));
    digest.finalize().into()
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
            hash_framed(&mut digest, commit.to_hex().as_bytes());
            hash_framed(&mut digest, tree.to_hex().as_bytes());
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
