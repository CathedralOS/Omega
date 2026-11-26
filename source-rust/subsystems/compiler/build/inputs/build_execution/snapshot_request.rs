//! Immutable source and output grants for one Build activation.

use crate::BuildSourceCaptureRequest;
use build_output::CapturedBuildSourceInput;
use diagnostics::Diagnostic;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSnapshotRequest {
    required_outputs: Vec<Vec<u8>>,
    product_source_directory: Option<Vec<u8>>,
    capture: BuildSnapshotCapture,
    inputs: BTreeMap<Vec<u8>, CapturedBuildSourceInput>,
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
        Self {
            required_outputs: Vec::new(),
            capture: if dependency_activation {
                BuildSnapshotCapture::PackageInventory
            } else {
                BuildSnapshotCapture::RequiredSources
            },
            inputs: BTreeMap::new(),
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
            inputs: BTreeMap::new(),
            product_source_directory: None,
        }
    }

    pub fn with_inputs(
        mut self,
        inputs: impl IntoIterator<Item = (Vec<u8>, CapturedBuildSourceInput)>,
    ) -> Result<Self, Vec<Diagnostic>> {
        for (name, input) in inputs {
            insert_named_input(&mut self.inputs, name, input)?;
        }
        Ok(self)
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

    pub fn inputs(&self) -> &BTreeMap<Vec<u8>, CapturedBuildSourceInput> {
        &self.inputs
    }

    pub fn required_outputs(&self) -> &[Vec<u8>] {
        &self.required_outputs
    }

    pub const fn capture(&self) -> &BuildSnapshotCapture {
        &self.capture
    }
}

fn insert_named_input(
    slots: &mut BTreeMap<Vec<u8>, CapturedBuildSourceInput>,
    name: Vec<u8>,
    input: CapturedBuildSourceInput,
) -> Result<(), Vec<Diagnostic>> {
    if std::str::from_utf8(&name).is_err()
        || !filesystem_authority::canonical_filesystem_metadata_path_is_canonical(&name, false)
    {
        return Err(vec![Diagnostic::error(format!(
            "named build input slot is not a canonical UTF-8 relative name: {name:?}"
        ))]);
    }
    if slots.contains_key(&name) {
        return Err(vec![Diagnostic::error(format!(
            "named build input slot {name:?} is declared twice for one dependency occurrence"
        ))]);
    }
    let entries = slots
        .values()
        .try_fold(input.entry_count(), |total, input| {
            total.checked_add(input.entry_count())
        });
    let bytes = slots.values().try_fold(input.file_bytes(), |total, input| {
        total.checked_add(input.file_bytes())
    });
    let names = slots
        .keys()
        .try_fold(name.len(), |total, name| total.checked_add(name.len()));
    if slots.len() >= 4096
        || entries.is_none_or(|entries| {
            entries > filesystem_authority::CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT as u64
        })
        || bytes.is_none_or(|bytes| bytes > 512 * 1024 * 1024)
        || names.is_none_or(|names| {
            names > filesystem_authority::FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT
        })
    {
        return Err(vec![Diagnostic::error(
            "named build inputs exceed the activation's captured-inventory limits",
        )]);
    }
    input.validate_snapshot_paths()?;
    slots.insert(name, input);
    Ok(())
}
