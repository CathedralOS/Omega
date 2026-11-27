//! Immutable source and output grants for one Build activation.

use crate::BuildSourceCaptureRequest;
use diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSnapshotRequest {
    directories: super::BuildDirectoryMappings,
    required_outputs: Vec<Vec<u8>>,
    product_source_directory: Option<Vec<u8>>,
    capture: BuildSnapshotCapture,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildSnapshotCapture {
    RequiredSources,                   // Local Build entry and its admitted source files.
    PackageInventory,                  // Resolver-supplied dependency snapshot.
    Scoped(BuildSourceCaptureRequest), // Explicit invocation inventory of files and subtrees.
}

impl BuildSnapshotRequest {
    /// Resolved dependencies receive their sealed package inventory. A local
    /// invocation without an explicit inventory can read its required source.
    pub fn for_activation(dependency_activation: bool) -> Self {
        Self::new(if dependency_activation {
            BuildSnapshotCapture::PackageInventory
        } else {
            BuildSnapshotCapture::RequiredSources
        })
    }

    pub fn new(capture: BuildSnapshotCapture) -> Self {
        Self {
            required_outputs: Vec::new(),
            capture,
            directories: Default::default(),
            product_source_directory: None,
        }
    }

    pub fn scoped(
        required_outputs: impl IntoIterator<Item = Vec<u8>>,
        capture: BuildSourceCaptureRequest,
    ) -> Self {
        Self {
            required_outputs: required_outputs.into_iter().collect(),
            capture: BuildSnapshotCapture::Scoped(capture),
            directories: Default::default(),
            product_source_directory: None,
        }
    }

    /// Extra live directories belong only to this Build activation.
    pub fn with_directories(
        mut self,
        directories: platform_custody::directory_mappings::DirectoryMappings,
    ) -> Self {
        self.directories = directories.map_backings(super::BuildDirectory::caller);
        self
    }

    pub fn with_successor_directories(
        mut self,
        directories: super::BuildDirectoryMappings,
    ) -> Self {
        self.directories = directories;
        self
    }

    pub fn directories(&self) -> &super::BuildDirectoryMappings {
        &self.directories
    }

    /// The caller selects a directory of this activation's private staging as
    /// a source overlay for its immediate Product. An empty path selects the
    /// staging root. Other staged files remain outside the source view.
    pub fn with_product_source_directory(
        mut self,
        directory: Vec<u8>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if !filesystem_authority::canonical_filesystem_metadata_path_is_canonical(&directory, true)
        {
            return Err(vec![Diagnostic::error(
                "Product source directory must be relative to the activation's private staging",
            )]);
        }
        self.product_source_directory = Some(directory);
        Ok(self)
    }

    pub fn product_source_directory(&self) -> Option<&[u8]> {
        self.product_source_directory.as_deref()
    }


    pub fn required_outputs(&self) -> &[Vec<u8>] {
        &self.required_outputs
    }

    pub const fn capture(&self) -> &BuildSnapshotCapture {
        &self.capture
    }
}
