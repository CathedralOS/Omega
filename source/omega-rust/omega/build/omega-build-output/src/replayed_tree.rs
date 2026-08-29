use super::{
    canonical_symlink_target, commitment_for_retained_entries, diagnostics, reserve_path_bytes,
    retained_native_path, validate_retained_tree, BuildStagedOutputTree, RetainedStagedOutputEntry,
    RetainedStagedOutputEntryKind, MAX_STAGED_OUTPUT_ENTRIES, MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES,
};
use psi_diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Inert compiler replay input for one receipted `Output` namespace entry.
///
/// These values are not staged-tree authority. The constructor below validates
/// the complete namespace and issues the private [`BuildStagedOutputTree`]
/// carrier. Symbolic links carry only their exact link path and target
/// spelling; replay grants them no filesystem authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayedBuildOutputEntry<'entry> {
    Directory {
        relative_path: &'entry [u8],
    },
    RegularFile {
        relative_path: &'entry [u8],
        bytes: &'entry [u8],
        executable: bool,
    },
    SymbolicLink {
        relative_path: &'entry [u8],
        target: &'entry [u8],
    },
}

impl<'entry> ReplayedBuildOutputEntry<'entry> {
    pub const fn directory(relative_path: &'entry [u8]) -> Self {
        Self::Directory { relative_path }
    }

    pub const fn regular_file(
        relative_path: &'entry [u8],
        bytes: &'entry [u8],
        executable: bool,
    ) -> Self {
        Self::RegularFile {
            relative_path,
            bytes,
            executable,
        }
    }

    pub const fn symbolic_link(relative_path: &'entry [u8], target: &'entry [u8]) -> Self {
        Self::SymbolicLink {
            relative_path,
            target,
        }
    }

    const fn relative_path(self) -> &'entry [u8] {
        match self {
            Self::Directory { relative_path }
            | Self::RegularFile { relative_path, .. }
            | Self::SymbolicLink { relative_path, .. } => relative_path,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamespaceEntryKind {
    Directory,
    RegularFile,
    SymbolicLink,
}

/// Reconstruct one complete mixed receipted `Output` tree.
///
/// Entries use canonical root-relative paths. Every nested entry must follow
/// its exact parent directory in the replay sequence; no directory is inferred.
/// Siblings may retain operation order because the issued tree is sorted into
/// canonical path order. Any exact path collision, including a directory/file
/// collision, rejects. Entry, aggregate path, per-file, and unique-content byte
/// ceilings are the same limits used by physical staged-output capture.
pub fn replayed_output_tree(
    entries: &[ReplayedBuildOutputEntry<'_>],
) -> Result<BuildStagedOutputTree, Vec<Diagnostic>> {
    if entries.len() > MAX_STAGED_OUTPUT_ENTRIES {
        return Err(diagnostics(format!(
            "receipted build output exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
        )));
    }

    let mut retained = Vec::new();
    retained.try_reserve_exact(entries.len()).map_err(|_| {
        diagnostics("receipted build output entry allocation failed on this compiler host")
    })?;
    let mut namespace = BTreeMap::<Vec<u8>, NamespaceEntryKind>::new();
    let mut distinct_content = BTreeSet::new();
    let mut unique_file_bytes = 0u64;
    let mut total_path_bytes = 0usize;

    for entry in entries.iter().copied() {
        let relative_path = entry.relative_path();
        let relative_native = retained_native_path(relative_path).map_err(|error| {
            diagnostics(format!(
                "receipted build output path is not canonical: {error}"
            ))
        })?;
        total_path_bytes = reserve_path_bytes(total_path_bytes, relative_path.len())?;
        if namespace.contains_key(relative_path) {
            return Err(diagnostics(format!(
                "receipted build output namespace contains more than one entry at `{}`",
                String::from_utf8_lossy(relative_path),
            )));
        }
        if let Some(separator) = relative_path.iter().rposition(|byte| *byte == b'/') {
            let parent = &relative_path[..separator];
            match namespace.get(parent) {
                Some(NamespaceEntryKind::Directory) => {}
                Some(NamespaceEntryKind::RegularFile | NamespaceEntryKind::SymbolicLink) => {
                    return Err(diagnostics(format!(
                        "receipted build output entry `{}` has a non-directory parent",
                        String::from_utf8_lossy(relative_path),
                    )));
                }
                None => {
                    return Err(diagnostics(format!(
                        "receipted nested build output entry `{}` must follow its exact parent directory",
                        String::from_utf8_lossy(relative_path),
                    )));
                }
            }
        }

        let (namespace_kind, retained_kind) = match entry {
            ReplayedBuildOutputEntry::Directory { .. } => (
                NamespaceEntryKind::Directory,
                RetainedStagedOutputEntryKind::Directory,
            ),
            ReplayedBuildOutputEntry::RegularFile {
                bytes, executable, ..
            } => {
                let length = u64::try_from(bytes.len()).map_err(|_| {
                    diagnostics("receipted build output length cannot be represented canonically")
                })?;
                if length > MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES {
                    return Err(diagnostics(format!(
                        "receipted build output exceeds its {MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES}-byte object ceiling"
                    )));
                }
                let digest: [u8; 32] = Sha256::digest(bytes).into();
                if distinct_content.insert((digest, length)) {
                    unique_file_bytes = unique_file_bytes
                        .checked_add(length)
                        .filter(|total| *total <= MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES)
                        .ok_or_else(|| {
                            diagnostics(format!(
                                "receipted build output exceeds its {MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES}-byte unique-content ceiling"
                            ))
                        })?;
                }
                (
                    NamespaceEntryKind::RegularFile,
                    RetainedStagedOutputEntryKind::File {
                        bytes: Arc::from(bytes),
                        executable,
                    },
                )
            }
            ReplayedBuildOutputEntry::SymbolicLink { target, .. } => {
                let target_spelling = std::str::from_utf8(target).map_err(|_| {
                    diagnostics(format!(
                        "receipted build output symlink `{}` has a non-UTF-8 target",
                        String::from_utf8_lossy(relative_path),
                    ))
                })?;
                let canonical_target = canonical_symlink_target(
                    std::path::Path::new(target_spelling),
                    relative_path,
                    &relative_native,
                )?;
                total_path_bytes = reserve_path_bytes(total_path_bytes, canonical_target.len())?;
                (
                    NamespaceEntryKind::SymbolicLink,
                    RetainedStagedOutputEntryKind::Symlink {
                        target: canonical_target,
                    },
                )
            }
        };
        namespace.insert(relative_path.to_vec(), namespace_kind);
        retained.push(RetainedStagedOutputEntry {
            relative_path: relative_path.to_vec(),
            kind: retained_kind,
        });
    }

    retained.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let commitment = commitment_for_retained_entries(&retained).ok_or_else(|| {
        diagnostics("receipted build output exceeds the staged-output unique-content ceiling")
    })?;
    debug_assert_eq!(commitment.file_bytes(), unique_file_bytes);
    let tree = BuildStagedOutputTree {
        commitment,
        entries: retained,
    };
    validate_retained_tree(&tree).map_err(|error| {
        diagnostics(format!(
            "receipted build output failed canonical tree validation: {error}"
        ))
    })?;
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{empty, replayed_empty_directories, replayed_files};

    #[test]
    fn reconstructs_mixed_parent_before_child_output_trees_canonically() {
        let mixed = replayed_output_tree(&[
            ReplayedBuildOutputEntry::directory(b"generated"),
            ReplayedBuildOutputEntry::regular_file(b"generated/data.bin", b"data", false),
            ReplayedBuildOutputEntry::regular_file(b"tool", b"executable", true),
        ])
        .expect("mixed receipted tree");
        let reordered_siblings = replayed_output_tree(&[
            ReplayedBuildOutputEntry::regular_file(b"tool", b"executable", true),
            ReplayedBuildOutputEntry::directory(b"generated"),
            ReplayedBuildOutputEntry::regular_file(b"generated/data.bin", b"data", false),
        ])
        .expect("operation order does not become tree identity");

        assert_eq!(mixed, reordered_siblings);
        assert_eq!(mixed.entry_count(), 3);
        assert_eq!(mixed.file_bytes(), 14);
    }

    #[test]
    fn retains_inert_symbolic_links_with_exact_target_spelling() {
        let mixed = replayed_output_tree(&[
            ReplayedBuildOutputEntry::directory(b"generated"),
            ReplayedBuildOutputEntry::regular_file(b"generated/data.bin", b"data", false),
            ReplayedBuildOutputEntry::symbolic_link(b"generated/current", b"data.bin"),
            ReplayedBuildOutputEntry::regular_file(b"tool", b"executable", true),
            ReplayedBuildOutputEntry::symbolic_link(b"generated/tool", b"../tool"),
        ])
        .expect("receipted tree with symbolic links");
        let reordered_siblings = replayed_output_tree(&[
            ReplayedBuildOutputEntry::regular_file(b"tool", b"executable", true),
            ReplayedBuildOutputEntry::directory(b"generated"),
            ReplayedBuildOutputEntry::symbolic_link(b"generated/tool", b"../tool"),
            ReplayedBuildOutputEntry::regular_file(b"generated/data.bin", b"data", false),
            ReplayedBuildOutputEntry::symbolic_link(b"generated/current", b"data.bin"),
        ])
        .expect("sibling order does not become tree identity");

        assert_eq!(mixed, reordered_siblings);
        assert_eq!(mixed.entry_count(), 5);
        assert_eq!(mixed.file_bytes(), 14);
        assert!(matches!(
            &mixed.entries[1].kind,
            RetainedStagedOutputEntryKind::Symlink { target } if target == b"data.bin"
        ));
        assert!(matches!(
            &mixed.entries[3].kind,
            RetainedStagedOutputEntryKind::Symlink { target } if target == b"../tool"
        ));
    }

    #[test]
    fn rejects_noncanonical_or_output_escaping_symbolic_link_targets() {
        for target in [
            b"".as_slice(),
            b"/absolute",
            b"./artifact",
            b"generated//artifact",
            b"generated\\artifact",
            b"artifact\0suffix",
            b"\xffartifact",
            b"../outside",
        ] {
            assert!(
                replayed_output_tree(&[ReplayedBuildOutputEntry::symbolic_link(b"link", target,)])
                    .is_err(),
                "accepted invalid symbolic-link target {:?}",
                String::from_utf8_lossy(target),
            );
        }

        assert!(replayed_output_tree(&[
            ReplayedBuildOutputEntry::directory(b"generated"),
            ReplayedBuildOutputEntry::symbolic_link(b"generated/link", b"../../outside",),
        ])
        .is_err());
    }

    #[test]
    fn rejects_symbolic_links_as_namespace_parents() {
        assert!(replayed_output_tree(&[
            ReplayedBuildOutputEntry::symbolic_link(b"alias", b"target"),
            ReplayedBuildOutputEntry::regular_file(b"alias/child", b"bytes", false),
        ])
        .is_err());
    }

    #[test]
    fn charges_symbolic_link_targets_to_the_shared_path_byte_ceiling() {
        let target = vec![b'a'; crate::MAX_STAGED_OUTPUT_PATH_BYTES];
        assert!(
            replayed_output_tree(&[ReplayedBuildOutputEntry::symbolic_link(b"link", &target)])
                .is_err()
        );
    }

    #[test]
    fn rejects_namespace_collisions_missing_parents_and_file_parents() {
        assert!(
            replayed_output_tree(&[
                ReplayedBuildOutputEntry::directory(b"generated"),
                ReplayedBuildOutputEntry::regular_file(b"generated", b"data", false),
            ])
            .is_err()
        );
        assert!(
            replayed_output_tree(&[ReplayedBuildOutputEntry::regular_file(
                b"generated/data.bin",
                b"data",
                false,
            )])
            .is_err()
        );
        assert!(
            replayed_output_tree(&[
                ReplayedBuildOutputEntry::regular_file(b"generated", b"data", false),
                ReplayedBuildOutputEntry::regular_file(b"generated/nested.bin", b"nested", false,),
            ])
            .is_err()
        );
        assert!(
            replayed_output_tree(&[
                ReplayedBuildOutputEntry::regular_file(b"generated/data.bin", b"data", false,),
                ReplayedBuildOutputEntry::directory(b"generated"),
            ])
            .is_err()
        );
    }

    #[test]
    fn enforces_entry_ceiling_and_preserves_compatibility_facades() {
        let repeated =
            vec![ReplayedBuildOutputEntry::directory(b"entry"); MAX_STAGED_OUTPUT_ENTRIES + 1];
        assert!(replayed_output_tree(&repeated).is_err());
        assert_eq!(replayed_output_tree(&[]).unwrap(), empty());

        assert_eq!(
            replayed_files(&[(b"artifact", b"bytes", false)]).unwrap(),
            replayed_output_tree(&[ReplayedBuildOutputEntry::regular_file(
                b"artifact",
                b"bytes",
                false,
            )])
            .unwrap(),
        );
        assert_eq!(
            replayed_empty_directories(&[b"generated", b"generated/nested"]).unwrap(),
            replayed_output_tree(&[
                ReplayedBuildOutputEntry::directory(b"generated"),
                ReplayedBuildOutputEntry::directory(b"generated/nested"),
            ])
            .unwrap(),
        );
    }
}
