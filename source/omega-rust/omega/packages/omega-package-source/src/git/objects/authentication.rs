//! Commit and tree graph authentication against declared Git object identities.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ffi::OsStr;

use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::identity::GitObjectIdAlgorithm;

use super::identity::{
    decode_git_object_id, git_object_algorithm, git_object_identity, git_object_invalid,
    is_object_id, verify_git_object_identity,
};
use super::tree::git_tree_invalid;
use super::{GitTreeEntry, GitTreeEntryKind};

#[derive(Debug)]
enum AuthenticatedGitTreeNode {
    Blob {
        mode: &'static [u8],
        oid: String,
    },
    Tree {
        expected_oid: String,
        directory: AuthenticatedGitDirectory,
    },
}

#[derive(Debug, Default)]
struct AuthenticatedGitDirectory {
    entries: BTreeMap<Vec<u8>, AuthenticatedGitTreeNode>,
}

pub(crate) fn verify_exact_git_revision(
    requested_rev: &str,
    selected_commit: &str,
) -> Result<(), SourceResolveError> {
    if is_object_id(requested_rev) && !requested_rev.eq_ignore_ascii_case(selected_commit) {
        return Err(git_object_invalid(
            selected_commit,
            "selected commit does not match the exact requested revision",
        ));
    }
    Ok(())
}

pub(crate) fn authenticate_git_commit(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    commit: &str,
    tree: &str,
) -> Result<(), SourceResolveError> {
    let payload = repository.run_git_bytes_stdout(
        executor,
        [
            OsStr::new("cat-file"),
            OsStr::new("commit"),
            OsStr::new(commit),
        ],
    )?;
    authenticate_git_commit_payload(commit, tree, &payload)
}

pub(crate) fn authenticate_git_commit_payload(
    commit: &str,
    reported_tree: &str,
    payload: &[u8],
) -> Result<(), SourceResolveError> {
    let algorithm = git_object_algorithm(commit)?;
    if git_object_algorithm(reported_tree)? != algorithm {
        return Err(git_object_invalid(
            commit,
            "commit and root tree use different object formats",
        ));
    }
    verify_git_object_identity(commit, b"commit", payload, algorithm)?;

    let first_line = payload
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or(payload);
    let Some(commit_tree) = first_line.strip_prefix(b"tree ") else {
        return Err(git_object_invalid(
            commit,
            "commit payload does not begin with one root tree edge",
        ));
    };
    let commit_tree = std::str::from_utf8(commit_tree)
        .map_err(|_| git_object_invalid(commit, "commit tree ID is not ASCII"))?;
    if git_object_algorithm(commit_tree)? != algorithm || commit_tree != reported_tree {
        return Err(git_object_invalid(
            commit,
            "commit root tree edge does not match the selected tree",
        ));
    }
    Ok(())
}

pub(crate) fn authenticate_git_tree(
    expected_tree: &str,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    authenticate_git_tree_graph(expected_tree, entries)?;
    authenticate_git_tree_payloads(expected_tree, entries)
}

/// Authenticate every mode/name/object edge back to the selected root tree.
///
/// Blob object IDs commit to their payloads, so graph authentication does not
/// need to open every blob. Any blob that is later consumed or materialized
/// must still pass [`authenticate_git_tree_payloads`].
pub(crate) fn authenticate_git_tree_graph(
    expected_tree: &str,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    let algorithm = git_object_algorithm(expected_tree)?;
    let mut root = AuthenticatedGitDirectory::default();
    for entry in entries {
        insert_authenticated_git_entry(&mut root, entry)?;
    }
    let actual_tree = authenticate_git_directory(&root, algorithm)?;
    if actual_tree != expected_tree {
        return Err(git_object_invalid(
            expected_tree,
            "authenticated tree graph does not reconstruct the selected root tree",
        ));
    }
    Ok(())
}

/// Authenticate the bytes of every populated blob entry against its graph ID.
pub(crate) fn authenticate_git_tree_payloads(
    expected_tree: &str,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    let algorithm = git_object_algorithm(expected_tree)?;
    for entry in entries {
        match &entry.kind {
            GitTreeEntryKind::Tree => {}
            GitTreeEntryKind::File { bytes, .. } => {
                verify_git_object_identity(&entry.oid, b"blob", bytes.as_slice(), algorithm)?;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                verify_git_object_identity(
                    &entry.oid,
                    b"blob",
                    target_bytes.as_slice(),
                    algorithm,
                )?;
            }
        }
    }
    Ok(())
}

fn insert_authenticated_git_entry(
    directory: &mut AuthenticatedGitDirectory,
    entry: &GitTreeEntry,
) -> Result<(), SourceResolveError> {
    let components = entry
        .relative_bytes
        .split(|byte| *byte == b'/')
        .collect::<Vec<_>>();
    insert_authenticated_git_components(directory, &components, entry)
}

fn insert_authenticated_git_components(
    directory: &mut AuthenticatedGitDirectory,
    components: &[&[u8]],
    entry: &GitTreeEntry,
) -> Result<(), SourceResolveError> {
    let Some((name, rest)) = components.split_first() else {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "authenticated tree entry has no path component",
        ));
    };
    if rest.is_empty() {
        let node = match entry.kind {
            GitTreeEntryKind::Tree => AuthenticatedGitTreeNode::Tree {
                expected_oid: entry.oid.clone(),
                directory: AuthenticatedGitDirectory::default(),
            },
            GitTreeEntryKind::File {
                executable: false, ..
            } => AuthenticatedGitTreeNode::Blob {
                mode: b"100644".as_slice(),
                oid: entry.oid.clone(),
            },
            GitTreeEntryKind::File {
                executable: true, ..
            } => AuthenticatedGitTreeNode::Blob {
                mode: b"100755".as_slice(),
                oid: entry.oid.clone(),
            },
            GitTreeEntryKind::Symlink { .. } => AuthenticatedGitTreeNode::Blob {
                mode: b"120000".as_slice(),
                oid: entry.oid.clone(),
            },
        };
        if directory.entries.insert(name.to_vec(), node).is_some() {
            return Err(git_tree_invalid(
                &entry.relative_bytes,
                "authenticated tree contains a duplicate path",
            ));
        }
        return Ok(());
    }

    let Some(node) = directory.entries.get_mut(*name) else {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "authenticated tree path has no declared parent-tree edge",
        ));
    };
    let AuthenticatedGitTreeNode::Tree {
        directory: child, ..
    } = node
    else {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "authenticated tree path traverses a blob",
        ));
    };
    insert_authenticated_git_components(child, rest, entry)
}

fn authenticate_git_directory(
    directory: &AuthenticatedGitDirectory,
    algorithm: GitObjectIdAlgorithm,
) -> Result<String, SourceResolveError> {
    let mut ordered = directory.entries.iter().collect::<Vec<_>>();
    ordered.sort_by(git_tree_entry_order);
    let mut payload = Vec::new();
    for (name, node) in ordered {
        let (mode, oid) = match node {
            AuthenticatedGitTreeNode::Blob { mode, oid } => (*mode, oid.clone()),
            AuthenticatedGitTreeNode::Tree {
                expected_oid,
                directory,
            } => {
                if git_object_algorithm(expected_oid)? != algorithm {
                    return Err(git_object_invalid(
                        expected_oid,
                        "child tree uses a different hash algorithm than its graph",
                    ));
                }
                let actual_oid = authenticate_git_directory(directory, algorithm)?;
                if actual_oid != *expected_oid {
                    return Err(git_object_invalid(
                        expected_oid,
                        "child tree bytes do not match the declared tree edge",
                    ));
                }
                (b"40000".as_slice(), actual_oid)
            }
        };
        payload.extend_from_slice(mode);
        payload.push(b' ');
        payload.extend_from_slice(name);
        payload.push(0);
        payload.extend_from_slice(&decode_git_object_id(&oid, algorithm)?);
    }
    git_object_identity(b"tree", &payload, algorithm)
}

fn git_tree_entry_order(
    left: &(&Vec<u8>, &AuthenticatedGitTreeNode),
    right: &(&Vec<u8>, &AuthenticatedGitTreeNode),
) -> Ordering {
    let common = left.0.len().min(right.0.len());
    let prefix = left.0[..common].cmp(&right.0[..common]);
    if prefix != Ordering::Equal {
        return prefix;
    }
    let left_next = left.0.get(common).copied().unwrap_or({
        if matches!(left.1, AuthenticatedGitTreeNode::Tree { .. }) {
            b'/'
        } else {
            0
        }
    });
    let right_next = right.0.get(common).copied().unwrap_or({
        if matches!(right.1, AuthenticatedGitTreeNode::Tree { .. }) {
            b'/'
        } else {
            0
        }
    });
    left_next.cmp(&right_next)
}
