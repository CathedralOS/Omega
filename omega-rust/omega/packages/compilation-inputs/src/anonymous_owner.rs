//! Canonical ownership for a completed default-build source invocation.
//!
//! A default Build has no dependency requests. Its empty edge roster is
//! committed explicitly, alongside the complete captured namespace and exact
//! applicable core declarations. Supplied package owners bypass this operation.
//! The anonymous record uses a different digest domain from named lineage;
//! PackageKeyIdentity carries the resulting opaque commitment through the
//! existing declaration, specialization and portable-artifact representations.
//! Root paths route capture only. Host/target checked instances, output paths,
//! debug settings, caches and backend optimization choices do not enter the key.

use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};
use source::{CapturedSourceNode, CapturedSourceTree};

fn field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

pub(crate) fn derive(
    capture: &CapturedSourceTree,
    core: impl IntoIterator<Item = [u8; 32]>,
) -> Result<PackageKeyIdentity, String> {
    let mut digest = Sha256::new();
    field(&mut digest, b"OMEGA-ANONYMOUS-CONTENT-OWNER-V1\0");
    // Version the language/default-build source semantics independently from
    // the compiler's native implementation and target code generation.
    field(&mut digest, b"OMEGA-LANGUAGE-DEFAULT-BUILD-SEMANTICS-V1\0");
    snapshot(&mut digest, capture)?;
    digest.update(0u64.to_le_bytes()); // completed dependency edge roster
    let mut core = core.into_iter().collect::<Vec<_>>();
    core.sort();
    core.dedup();
    digest.update((core.len() as u64).to_le_bytes());
    for identity in core {
        field(&mut digest, &identity);
    }
    PackageKeyIdentity::from_digest(digest.finalize().into())
        .ok_or_else(|| "anonymous owner has the reserved zero digest".to_owned())
}

/// The complete applicable core snapshot is one semantic dependency.
pub(crate) fn core_snapshot_identity(capture: &CapturedSourceTree) -> Result<[u8; 32], String> {
    let mut digest = Sha256::new();
    field(&mut digest, b"OMEGA-ANONYMOUS-CORE-SNAPSHOT-V1\0");
    snapshot(&mut digest, capture)?;
    Ok(digest.finalize().into())
}

fn snapshot(digest: &mut Sha256, capture: &CapturedSourceTree) -> Result<(), String> {
    let mut entries = capture
        .entries()
        .map(|(path, entry)| {
            crate::source_snapshot::capture_relative_path_bytes(path).map(|path| (path, entry))
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    digest.update((entries.len() as u64).to_le_bytes());
    for (path, entry) in entries {
        field(digest, &path);
        match entry {
            CapturedSourceNode::Directory => digest.update([0]),
            CapturedSourceNode::File { bytes, executable } => {
                digest.update([1, u8::from(*executable)]);
                field(digest, bytes);
            }
            CapturedSourceNode::Symlink { target } => {
                digest.update([2]);
                field(
                    digest,
                    &crate::canonical_fs::os_str_bytes(target.as_os_str())?,
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    fn snapshot(path: &str, bytes: &[u8]) -> CapturedSourceTree {
        CapturedSourceTree::new([(
            PathBuf::from(path),
            CapturedSourceNode::File {
                bytes: Arc::from(bytes),
                executable: false,
            },
        )])
        .expect("closed source snapshot")
    }

    #[test]
    fn canonical_record_retains_source_and_core_without_instance_order() {
        let source = snapshot("library.omg", b"public data Token {}");
        let owner = derive(&source, [[1; 32], [2; 32]]).unwrap();
        // Independent V1 framing vector: u64 little-endian lengths/counts,
        // file tag 1, executable tag 0, then the sorted core commitments.
        assert_eq!(
            owner.digest(),
            [
                53, 179, 79, 52, 161, 231, 70, 176, 53, 17, 97, 63, 133, 76, 192, 249, 53, 186,
                154, 205, 196, 211, 104, 141, 69, 99, 254, 91, 79, 148, 35, 9
            ]
        );
        assert_eq!(owner, derive(&source, [[2; 32], [1; 32], [2; 32]]).unwrap());
        assert_ne!(owner, derive(&source, [[1; 32], [3; 32]]).unwrap());
        assert_ne!(
            owner,
            derive(
                &snapshot("other.omg", b"public data Token {}"),
                [[1; 32], [2; 32]]
            )
            .unwrap()
        );
        assert_ne!(
            owner,
            derive(
                &snapshot("library.omg", b"public data Token { value: u8; }"),
                [[1; 32], [2; 32]]
            )
            .unwrap()
        );
        let expanded = CapturedSourceTree::new([
            (
                PathBuf::from("library.omg"),
                CapturedSourceNode::File {
                    bytes: Arc::from(b"public data Token {}".as_slice()),
                    executable: false,
                },
            ),
            (
                PathBuf::from("unused.omg"),
                CapturedSourceNode::File {
                    bytes: Arc::from(b"const SEMANTIC_INPUT: u8 = 1;".as_slice()),
                    executable: false,
                },
            ),
        ])
        .unwrap();
        assert_ne!(
            owner,
            derive(&expanded, [[1; 32], [2; 32]]).unwrap(),
            "the entire capture participates, including unselected files"
        );
    }

    #[test]
    fn closed_core_commitment_includes_undemanded_semantic_inputs() {
        let source = snapshot("library.omg", b"public data Token {}");
        let core = snapshot("core.omg", b"public data Core {}");
        let expanded = CapturedSourceTree::new([
            (
                PathBuf::from("core.omg"),
                CapturedSourceNode::File {
                    bytes: Arc::from(b"public data Core {}".as_slice()),
                    executable: false,
                },
            ),
            (
                PathBuf::from("other.omg"),
                CapturedSourceNode::File {
                    bytes: Arc::from(b"public data Other {}".as_slice()),
                    executable: false,
                },
            ),
        ])
        .unwrap();
        let before = core_snapshot_identity(&core).unwrap();
        let after = core_snapshot_identity(&expanded).unwrap();
        assert_ne!(before, after);
        assert_ne!(
            derive(&source, [before]).unwrap(),
            derive(&source, [after]).unwrap()
        );
    }

    #[test]
    fn source_binding_keeps_supplied_and_core_owners_and_checked_instances() {
        let identity = derive(&snapshot("library.omg", b"public data Token {}"), []).unwrap();
        let supplied = PackageKeyIdentity::from_digest([23; 32]).unwrap();
        let root = PathBuf::from("/anonymous");
        let mut sources = source::SourceMap::default();
        for scope in [
            source::DependencyScope::Product,
            source::DependencyScope::Build,
        ] {
            sources.add_checked_instance(
                root.join("library.omg"),
                "public data Token {}".to_owned(),
                root.clone(),
                None,
                source::SourceOrigin::User,
                source::SourceResolutionStratum::Base,
                scope,
            );
        }
        sources.add_with_metadata(
            PathBuf::from("/supplier/types.omg"),
            "public data Foreign {}".to_owned(),
            PathBuf::from("/supplier"),
            Some(supplied),
            source::SourceOrigin::User,
        );
        sources.add_with_metadata(
            PathBuf::from("/toolchain/core.omg"),
            "public data Core {}".to_owned(),
            PathBuf::from("/toolchain"),
            None,
            source::SourceOrigin::Toolchain,
        );
        sources
            .bind_default_anonymous_owner(&root, identity)
            .unwrap();
        assert_eq!(
            sources.get(source::SourceId(0)).unwrap().package_identity,
            Some(identity)
        );
        assert_eq!(
            sources.get(source::SourceId(1)).unwrap().package_identity,
            Some(identity)
        );
        assert_eq!(
            sources.get(source::SourceId(2)).unwrap().package_identity,
            Some(supplied)
        );
        assert_eq!(
            sources.get(source::SourceId(3)).unwrap().package_identity,
            None
        );
        assert_ne!(
            sources.get(source::SourceId(0)).unwrap().dependency_scope,
            sources.get(source::SourceId(1)).unwrap().dependency_scope
        );
        let before = sources.clone();
        sources
            .bind_default_anonymous_owner(&root, supplied)
            .unwrap();
        assert_eq!(sources, before, "a supplied owner is never rebound");
    }

    #[test]
    fn missing_foreign_provenance_rejects_without_partial_binding() {
        let owner = derive(&snapshot("library.omg", b"public data Token {}"), []).unwrap();
        let root = PathBuf::from("/anonymous");
        let mut sources = source::SourceMap::default();
        sources.add_with_metadata(
            root.join("library.omg"),
            String::new(),
            root.clone(),
            None,
            source::SourceOrigin::User,
        );
        sources.add_with_metadata(
            PathBuf::from("/foreign/copied.omg"),
            String::new(),
            PathBuf::from("/foreign"),
            None,
            source::SourceOrigin::User,
        );
        let before = sources.clone();
        let error = sources
            .bind_default_anonymous_owner(&root, owner)
            .unwrap_err();
        assert!(error.contains("no supplied owner outside the anonymous capture"));
        assert_eq!(sources, before);
    }
}
