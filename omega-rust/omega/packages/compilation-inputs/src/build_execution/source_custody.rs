//! Acquired input bytes for an ordinary Build activation, independently of
//! whether declaration ownership is anonymous, package-supplied, or Toolchain.

use std::path::{Path, PathBuf};

#[derive(Clone, compiler_storage::Record)]
pub struct CapturedBuildSource {
    root: PathBuf,
    filesystem: source::SourceFileSystem,
    input: build_output::CapturedBuildSourceInput,
}

impl CapturedBuildSource {
    pub(crate) fn new(
        root: PathBuf,
        tree: &source::CapturedSourceTree,
        filesystem: source::SourceFileSystem,
    ) -> Result<Self, String> {
        Ok(Self {
            root,
            filesystem,
            input: crate::source_snapshot::retained_source_input(tree)?,
        })
    }

    pub(crate) fn anonymous(source: &crate::AnonymousSourceCapture) -> Result<Self, String> {
        Self::new(
            source.root.clone(),
            &source.capture,
            source.filesystem.clone(),
        )
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn filesystem(&self) -> &source::SourceFileSystem {
        &self.filesystem
    }
    pub fn build_input(&self) -> &build_output::CapturedBuildSourceInput {
        &self.input
    }
}
