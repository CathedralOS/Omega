//! The key a compiled-review record is filed under, and the provenance
//! digests a record stores so a lookup can prove it was filed for the live
//! package key and source resolution.

use crate::declarations::DependencyPurpose;
use package_compilation::review_record_encoding::purpose_tag;
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
/// build profile), the checked entry path, the compiler input identity the
/// lock records (build-produced compiler digest plus selected toolchain
/// library content), and the content digests of the complete compilation
/// inputs and the build snapshot.
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

pub(crate) fn compiled_review_key_with_identity(
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    checked_target: target::TargetProfile,
    build_execution_profile: Option<target::TargetProfile>,
    entry: &Path,
    inputs: &PackageCompilationInputs,
    snapshot: &build_evaluation::BuildSnapshotRequest,
    compiler_identity: [u8; 32],
) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(RECORD_KEY_DOMAIN);
    digest.raw(&package.digest());
    digest.byte(purpose_tag(purpose));
    digest.bytes(checked_target.target_name().as_bytes());
    match build_execution_profile {
        Some(profile) => {
            digest.byte(1);
            digest.bytes(profile.target_name().as_bytes());
        }
        None => digest.byte(0),
    }
    digest.bytes(entry.as_os_str().as_encoded_bytes());
    digest.raw(&compiler_identity);
    digest.raw(&review_cache_inputs_digest(inputs));
    digest.raw(&review_cache_snapshot_digest(snapshot));
    digest.finish().into()
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
            digest.update(commit.to_hex().as_bytes());
            digest.update(tree.to_hex().as_bytes());
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

#[cfg(test)]
mod tests {
    use super::{compiled_review_key, compiled_review_key_with_identity};
    use crate::declarations::DependencyPurpose;
    use package_compilation::{PackageCompilationInputs, PackageSourceBinding};
    use semantic_vocabulary::PackageKeyIdentity;
    use std::path::{Path, PathBuf};

    /// Records are content-named by this key, so a different key is a miss.
    /// The live key must bind exactly the compiler input identity the lock
    /// records: a review one compiler issued never stands in for another's,
    /// while the same compiler finds its own record.
    #[test]
    fn compiled_review_key_binds_the_locked_compiler_input_identity() {
        let package = PackageKeyIdentity::from_digest([7; 32]).unwrap();
        let inputs = PackageCompilationInputs::new_package(
            package,
            vec![PackageSourceBinding::new(
                package,
                "reviewed_package",
                PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            )],
            Vec::new(),
        )
        .unwrap();
        let snapshot = build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>());
        let entry = Path::new("main.omg");
        let key_for = |compiler_identity| {
            compiled_review_key_with_identity(
                package,
                DependencyPurpose::Product,
                target::TargetProfile::MacosArm64,
                None,
                entry,
                &inputs,
                &snapshot,
                compiler_identity,
            )
        };
        let locked_identity = crate::operations::compiler_input_identity()
            .expect("development builds identify their compiler and toolchain library");
        let live = compiled_review_key(
            package,
            DependencyPurpose::Product,
            target::TargetProfile::MacosArm64,
            None,
            entry,
            &inputs,
            &snapshot,
        )
        .expect("an identified compiler keys its records");

        assert_eq!(live, key_for(locked_identity));
        for byte in [0, 31] {
            let mut other_compiler = locked_identity;
            other_compiler[byte] ^= 1;
            assert_ne!(live, key_for(other_compiler));
        }
    }
}
