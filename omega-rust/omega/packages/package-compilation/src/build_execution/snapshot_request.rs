//! Caller-selected immutable build inputs and required output names.

use crate::BuildSourceCaptureRequest;
use build_output::CapturedBuildSourceInput;
use diagnostics::Diagnostic;
use std::collections::BTreeMap;

/// Invocation-level request to execute one build occurrence against a
/// captured immutable source snapshot and to require the named outputs to
/// complete as sealed regular files before the build result may publish.
///
/// The request is a plain carrier: it expresses intent but grants nothing by
/// itself. The caller binds the captured input inventory and the roster onto
/// the build's filesystem scope; admission then serves the retained inventory directly through
/// an exact non-following memory view and completion checking enforces the roster against
/// retained staged-output custody. Capture requires the source root's
/// canonical sealed form (the same physical inventory validated for canonical
/// Source metadata); an unsealed or mutating root fails capture rather than
/// producing a falsely attributed snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSnapshotRequest {
    required_outputs: Vec<Vec<u8>>,
    capture: BuildSnapshotCapture,
    inputs: BTreeMap<Vec<u8>, CapturedBuildSourceInput>,
}

/// The caller-authorized source inventory one build activation captures
/// before execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildSnapshotCapture {
    /// The root package's complete sealed inventory — the exact custody the
    /// resolver already validated. Capture reads no member the package
    /// binding did not commit.
    PackageInventory,
    /// An explicit invocation inventory of files and subtrees. A package
    /// root retains full resolver provenance while its build receives only
    /// the selected view; a standalone root captures only declared members.
    Scoped(BuildSourceCaptureRequest),
}

impl BuildSnapshotRequest {
    /// A snapshot over the root package's sealed inventory. Package custody
    /// is itself the caller-authorized inventory — the resolver's canonical
    /// index fixes membership — so this form needs no capture request.
    pub fn new(required_outputs: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            required_outputs: required_outputs.into_iter().collect(),
            capture: BuildSnapshotCapture::PackageInventory,
            inputs: BTreeMap::new(),
        }
    }

    /// The capture a root package selects on its own, with no caller intent.
    ///
    /// Compiler-validated canonical Source metadata means the resolver has
    /// already fixed this package's membership, so the activation captures
    /// that sealed inventory into immutable memory storage and requires no
    /// invocation roster — the same source custody package review uses. A
    /// binding carrying no such index stays live and captures nothing.
    ///
    /// This belongs to the custody the package inputs carry, not to the
    /// entrypoint a caller happened to use, so every route that assembles a
    /// checked activation applies it and an explicitly supplied snapshot
    /// always wins. Deriving it on one route only is what let a build machine
    /// reach the filesystem with no staged-output custody behind it.
    pub fn automatic_package_inventory(
        package_inputs: Option<&crate::PackageCompilationInputs>,
    ) -> Option<Self> {
        package_inputs
            .filter(|inputs| inputs.canonical_source_metadata(inputs.root()).is_some())
            .map(|_| Self::new(std::iter::empty::<Vec<u8>>()))
    }

    /// A snapshot over exactly the files and subtrees the caller declared.
    /// Package capture additionally verifies this view against the complete
    /// resolver-owned source inventory; it never replaces package provenance.
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

    /// Supply immutable slots to this activation, separately from its source
    /// inventory. The caller owns capture; these names never select host paths.
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

    /// Required sealed output paths in canonical slash-separated form. An
    /// empty roster still runs against the captured source snapshot.
    pub fn required_outputs(&self) -> &[Vec<u8>] {
        &self.required_outputs
    }

    /// The caller-authorized inventory this snapshot captures.
    pub const fn capture(&self) -> &BuildSnapshotCapture {
        &self.capture
    }
}

// These are implementation resource ceilings, not extra authority. Count
// complete inventories, including unread files and names, before issuing views.
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
