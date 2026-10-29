//! Immutable source custody for the language-coupled Core Build occurrence.
//!
//! Core keeps Toolchain declaration identity. Its content key routes an actual
//! Build activation and handoff; it is never an ordinary package identity.

use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct CoreSourceIdentity([u8; 32]);

impl CoreSourceIdentity {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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

impl compiler_storage::Record for CapturedCoreSource {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        use compiler_storage::Record;
        output.record(|output| {
            self.root.write(output)?;
            self.tree.write(output)?;
            self.identity.write(output)
        })
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        use compiler_storage::Record;
        input.record(|input| {
            let root = PathBuf::read(input)?;
            let tree = Arc::<source::CapturedSourceTree>::read(input)?;
            let retained_identity = CoreSourceIdentity::read(input)?;
            let captured = Self::new(root, tree).map_err(|reason| input.error(reason))?;
            if captured.identity != retained_identity {
                return Err(
                    input.error("captured Core identity differs from its retained complete source")
                );
            }
            Ok(captured)
        })
    }
}
