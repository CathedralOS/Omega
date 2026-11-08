// Caller-selected immutable build snapshot requests (`BuildSnapshotRequest`,
// `BuildSnapshotCapture`) and named input slots. When package inputs carry
// validated canonical `Source` metadata, `automatic_package_inventory` selects
// `BuildSnapshotCapture::PackageInventory` automatically unless overridden by
// an explicit snapshot request.

use crate::BuildSourceCaptureRequest;
use build_output::CapturedBuildSourceInput;
use diagnostics::Diagnostic;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSnapshotRequest {
    required_outputs: Vec<Vec<u8>>,
    capture: BuildSnapshotCapture,
    inputs: BTreeMap<Vec<u8>, CapturedBuildSourceInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildSnapshotCapture {
    PackageInventory,                  // Root package's complete sealed inventory.
    Scoped(BuildSourceCaptureRequest), // Explicit invocation inventory of files and subtrees.
}

impl BuildSnapshotRequest {
    pub fn new(required_outputs: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            required_outputs: required_outputs.into_iter().collect(),
            capture: BuildSnapshotCapture::PackageInventory,
            inputs: BTreeMap::new(),
        }
    }

    pub fn automatic_package_inventory(
        package_inputs: Option<&crate::PackageCompilationInputs>,
    ) -> Option<Self> {
        package_inputs
            .filter(|inputs| inputs.canonical_source_metadata(inputs.root()).is_some())
            .map(|_| Self::new(std::iter::empty::<Vec<u8>>()))
    }

    pub fn scoped(
        required_outputs: impl IntoIterator<Item = Vec<u8>>,
        capture: BuildSourceCaptureRequest,
    ) -> Self {
        Self {
            required_outputs: required_outputs.into_iter().collect(),
            capture: BuildSnapshotCapture::Scoped(capture),
            inputs: BTreeMap::new(),
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
    slots.insert(name, input);
    Ok(())
}
