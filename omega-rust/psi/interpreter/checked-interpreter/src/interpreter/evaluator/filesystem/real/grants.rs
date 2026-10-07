use crate::{CanonicalFilesystemMetadataIndex, FilesystemGrantRootIdentity};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Canonicalized [`crate::FsGrants`]: the roots a scoped run may read/write
/// under, resolved once at construction so prefix checks compare real paths.
#[derive(Debug)]
pub(super) struct Grants {
    pub(super) read_roots: Vec<GrantRoot>,
    pub(super) write_roots: Vec<GrantRoot>,
}

#[derive(Debug)]
pub(super) struct GrantRoot {
    pub(super) identity: FilesystemGrantRootIdentity,
    pub(super) path: PathBuf,
    pub(super) canonical_metadata: Option<CanonicalFilesystemMetadataIndex>,
}

/// Canonicalize a grant root before execution. Scoped evidence needs one exact
/// physical root for both authorization and rooted-path identity, so an absent
/// or unresolvable root is an invalid compiler grant rather than a path-level
/// errno observed by package code.
pub(super) fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    root.canonicalize().map_err(|error| {
        format!(
            "filesystem grant root `{}` cannot be resolved: {error}",
            root.display()
        )
    })
}

impl Grants {
    pub(super) fn matching_root(&self, resolved: &Path, write: bool) -> Option<&GrantRoot> {
        // A write root grants read-back. Prefer the most specific root so a
        // build/output root nested under a source root keeps its output
        // identity. Grant validation rejects equal physical roots, so no
        // caller-order or numeric tie-break can select evidence identity.
        self.write_roots
            .iter()
            .chain(if write {
                [].iter()
            } else {
                self.read_roots.as_slice().iter()
            })
            .filter(|root| resolved.starts_with(&root.path))
            .max_by(|left, right| {
                left.path
                    .components()
                    .count()
                    .cmp(&right.path.components().count())
            })
    }

    pub(super) fn root(&self, identity: FilesystemGrantRootIdentity) -> Option<&GrantRoot> {
        self.write_roots
            .iter()
            .chain(self.read_roots.iter())
            .find(|root| root.identity == identity)
    }
}

pub(super) fn canonical_grants(grants: crate::FsGrants) -> Result<Grants, String> {
    let mut identities = BTreeSet::new();
    let mut physical_roots = BTreeMap::<PathBuf, FilesystemGrantRootIdentity>::new();
    let mut canonicalize =
        |root: crate::FilesystemGrantRoot, write: bool| -> Result<GrantRoot, String> {
            if write && root.canonical_metadata().is_some() {
                return Err(format!(
                    "writable filesystem grant root `{}` cannot carry immutable canonical metadata",
                    root.identity().get()
                ));
            }
            if !identities.insert(root.identity()) {
                return Err(format!(
                    "filesystem grant-root identity `{}` is duplicated",
                    root.identity().get()
                ));
            }
            let path = canonical_root(root.path())?;
            if let Some(previous) = physical_roots.insert(path.clone(), root.identity()) {
                return Err(format!(
                    "filesystem grant root `{}` has conflicting identities `{}` and `{}`",
                    path.display(),
                    previous.get(),
                    root.identity().get()
                ));
            }
            Ok(GrantRoot {
                identity: root.identity(),
                path,
                canonical_metadata: root.canonical_metadata().cloned(),
            })
        };
    let read_roots = grants
        .read_roots
        .into_iter()
        .map(|root| canonicalize(root, false))
        .collect::<Result<Vec<_>, _>>()?;
    let write_roots = grants
        .write_roots
        .into_iter()
        .map(|root| canonicalize(root, true))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Grants {
        read_roots,
        write_roots,
    })
}
