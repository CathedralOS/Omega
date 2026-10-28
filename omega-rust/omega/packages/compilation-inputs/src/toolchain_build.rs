//! Immutable source custody for the language-coupled Core Build occurrence.
//!
//! Core keeps Toolchain declaration identity. Its content key routes an actual
//! Build activation and handoff; it is never an ordinary package identity.

use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CoreSourceIdentity([u8; 32]);

impl CoreSourceIdentity {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildSourceOwner {
    Package(semantic_vocabulary::PackageKeyIdentity),
    ToolchainCore(CoreSourceIdentity),
}

/// Only source admission constructs this input from its complete captured
/// Core mount. Consumers cannot retag an arbitrary local capture as Toolchain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedCoreSource {
    root: PathBuf,
    tree: Arc<source::CapturedSourceTree>,
    identity: CoreSourceIdentity,
}

impl CapturedCoreSource {
    pub(crate) fn new(
        root: PathBuf,
        tree: Arc<source::CapturedSourceTree>,
    ) -> Result<Self, String> {
        let identity = CoreSourceIdentity(crate::anonymous_owner::core_snapshot_identity(&tree)?);
        if !tree.is_file(Path::new("build.omg")) {
            return Err("captured Core has no ordinary Build entry".to_owned());
        }
        Ok(Self {
            root,
            tree,
            identity,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn build_path(&self) -> PathBuf {
        self.root.join("build.omg")
    }
    pub fn identity(&self) -> CoreSourceIdentity {
        self.identity
    }
    pub(crate) fn contains_source(&self, source: &source::SourceFile) -> bool {
        source.origin == source::SourceOrigin::Toolchain
            && source.package_identity.is_none()
            && source
                .path
                .strip_prefix(&source.package_root)
                .ok()
                .and_then(|relative| self.tree.read_to_string(relative).ok())
                .is_some_and(|bytes| bytes == source.source)
    }

    pub(crate) fn tree(&self) -> &Arc<source::CapturedSourceTree> {
        &self.tree
    }
}
