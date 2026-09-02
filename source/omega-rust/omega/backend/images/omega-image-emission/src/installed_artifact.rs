use omega_executable_installation::{
    ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId,
};
use omega_function_identity::MachineFunctionIdentity;
use omega_installation_evidence::InstalledArtifactOccurrenceDigest;
use psi_layout_plans::EntryStubId;

use crate::{
    ExecutableImage, InstallationRecord, InstalledCompilerPrivateFunction, ObjectArtifact,
    validate_installation_record,
};

/// Exact join between one canonical terminal installation record and the
/// installed code occurrence containing that record's compiler-authored text.
///
/// The record's fingerprints remain report identities. This opaque carrier is
/// produced only after replaying the complete record against the emitted image
/// and comparing both unrelocated and materialized text with `InstalledCode`.
#[derive(Debug)]
pub struct InstalledArtifact {
    object: ObjectArtifact,
    image: ExecutableImage,
    installation: InstallationRecord,
    installed: InstalledCode,
}

impl InstalledArtifact {
    pub const fn object(&self) -> &ObjectArtifact {
        &self.object
    }

    pub const fn image(&self) -> &ExecutableImage {
        &self.image
    }

    pub const fn installation(&self) -> &InstallationRecord {
        &self.installation
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed.identity()
    }

    pub fn artifact(&self) -> ArtifactId {
        self.installed.artifact()
    }

    /// Borrow the linear installed-code occurrence retained by this join.
    /// Callers cannot retire it while the joined artifact remains live.
    pub const fn installed(&self) -> &InstalledCode {
        &self.installed
    }

    /// Compare complete opaque installation evidence, not only report IDs.
    pub fn binds_installed_code(&self, installed: &InstalledCode) -> bool {
        self.installed.receipt_context() == installed.receipt_context()
    }

    /// Release the canonical installation record and exact installed-code
    /// custody after the higher-level runnable lifecycle has retired.
    pub fn into_parts(
        self,
    ) -> (
        ObjectArtifact,
        ExecutableImage,
        InstallationRecord,
        InstalledCode,
    ) {
        (self.object, self.image, self.installation, self.installed)
    }
}

/// Transactional rejection from the terminal-record/artifact join. The
/// canonical record is returned so orchestration may correct another input and
/// retry without reconstructing installation metadata.
#[derive(Debug)]
pub struct InstalledArtifactBindingError {
    object: ObjectArtifact,
    image: ExecutableImage,
    installation: InstallationRecord,
    installed: InstalledCode,
    diagnostic: String,
}

impl InstalledArtifactBindingError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ObjectArtifact,
        ExecutableImage,
        InstallationRecord,
        InstalledCode,
    ) {
        (self.object, self.image, self.installation, self.installed)
    }
}

impl std::fmt::Display for InstalledArtifactBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for InstalledArtifactBindingError {}

/// Exact logical text/data image consumed by the normalized installation
/// ladder. The section gap is retained as canonical zero padding so a single
/// frozen placement can reproduce the target addresses used by final
/// relocation replay without erasing section boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledArtifactMemoryImages {
    layout: omega_image::FinalImageLayout,
    data_offset: Option<usize>,
    encoded: Vec<u8>,
    materialized: Vec<u8>,
}

impl InstalledArtifactMemoryImages {
    pub const fn layout(&self) -> omega_image::FinalImageLayout {
        self.layout
    }

    pub const fn data_offset(&self) -> Option<usize> {
        self.data_offset
    }

    pub fn encoded(&self) -> &[u8] {
        &self.encoded
    }

    pub fn materialized(&self) -> &[u8] {
        &self.materialized
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledArtifactMemoryProjectionError(String);

impl InstalledArtifactMemoryProjectionError {
    pub fn diagnostic(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InstalledArtifactMemoryProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for InstalledArtifactMemoryProjectionError {}

/// Reconstruct the exact compiler-authored pre/post-relocation section images
/// expected by the installation provider. Image-writer thunks and their mutable
/// binding slots, other mutable initialized data, and BSS remain outside this
/// bounded lane.
pub fn project_installed_artifact_memory_images(
    object: &ObjectArtifact,
    image: &ExecutableImage,
) -> Result<InstalledArtifactMemoryImages, InstalledArtifactMemoryProjectionError> {
    if object.psi() != image.psi() || object.target() != image.target() {
        return Err(InstalledArtifactMemoryProjectionError(
            "terminal object and image identity differ".into(),
        ));
    }
    crate::validate_executable_image(object, image).map_err(|diagnostic| {
        InstalledArtifactMemoryProjectionError(format!(
            "terminal executable image replay failed: {diagnostic}"
        ))
    })?;
    let output = image.output();
    if output.final_text_bytes.len() < object.text_bytes().len()
        || output.final_data_bytes.len() < object.data_bytes().len()
        || output.final_image_layout.text_address == 0
    {
        return Err(InstalledArtifactMemoryProjectionError(
            "terminal object/image section byte counts or text placement differ".into(),
        ));
    }
    let data_offset = if object.data_bytes().is_empty() {
        None
    } else {
        let offset = output
            .final_image_layout
            .data_address
            .checked_sub(output.final_image_layout.text_address)
            .and_then(|offset| usize::try_from(offset).ok())
            .filter(|offset| *offset >= object.text_bytes().len())
            .ok_or_else(|| {
                InstalledArtifactMemoryProjectionError(
                    "terminal initialized-data placement overlaps text or is not representable"
                        .into(),
                )
            })?;
        Some(offset)
    };
    let encoded =
        flatten_installed_sections(object.text_bytes(), object.data_bytes(), data_offset)?;
    let materialized = flatten_installed_sections(
        &output.final_text_bytes[..object.text_bytes().len()],
        &output.final_data_bytes[..object.data_bytes().len()],
        data_offset,
    )?;
    Ok(InstalledArtifactMemoryImages {
        layout: output.final_image_layout,
        data_offset,
        encoded,
        materialized,
    })
}

fn flatten_installed_sections(
    text: &[u8],
    data: &[u8],
    data_offset: Option<usize>,
) -> Result<Vec<u8>, InstalledArtifactMemoryProjectionError> {
    let Some(data_offset) = data_offset else {
        if !data.is_empty() {
            return Err(InstalledArtifactMemoryProjectionError(
                "initialized data has no retained placement".into(),
            ));
        }
        return Ok(text.to_vec());
    };
    let total = data_offset.checked_add(data.len()).ok_or_else(|| {
        InstalledArtifactMemoryProjectionError("installed section extent overflows".into())
    })?;
    let mut bytes = vec![0; total];
    bytes[..text.len()].copy_from_slice(text);
    bytes[data_offset..].copy_from_slice(data);
    Ok(bytes)
}

/// Exact attribution of one already-admitted entry to one compiler-private
/// function in one installed artifact occurrence.
///
/// This carrier is descriptive custody only. It exposes neither a resolved
/// address nor external-root, registrar, capacity, or lifetime authority.
pub struct InstalledCompilerPrivateFunctionEntry {
    private_function: InstalledCompilerPrivateFunction,
    entry: EntryStubId,
    artifact: ArtifactId,
    installed_code: InstalledCodeId,
    installed_context: InstalledCodeContext,
}

impl std::fmt::Debug for InstalledCompilerPrivateFunctionEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstalledCompilerPrivateFunctionEntry")
            .field("private_function", &self.private_function)
            .field("entry", &self.entry)
            .field("artifact", &self.artifact)
            .field("installed_code", &self.installed_code)
            .field("occurrence_digest", &self.occurrence_digest())
            .finish_non_exhaustive()
    }
}

impl InstalledCompilerPrivateFunctionEntry {
    pub const fn private_function(&self) -> &InstalledCompilerPrivateFunction {
        &self.private_function
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub fn occurrence_digest(&self) -> InstalledArtifactOccurrenceDigest {
        self.installed_context.occurrence_digest()
    }

    /// Compare complete opaque installation evidence, not compact IDs.
    pub fn binds_installed_code(&self, installed: &InstalledCode) -> bool {
        self.installed_context == installed.receipt_context()
    }
}

/// Fail-closed diagnostic from private-function entry attribution. The
/// caller-supplied identities are retained so orchestration can retry without
/// deriving or minting replacements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCompilerPrivateFunctionEntryBindingError {
    private_function: MachineFunctionIdentity,
    entry: EntryStubId,
    diagnostic: String,
}

impl InstalledCompilerPrivateFunctionEntryBindingError {
    pub const fn private_function(&self) -> MachineFunctionIdentity {
        self.private_function
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
}

impl std::fmt::Display for InstalledCompilerPrivateFunctionEntryBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for InstalledCompilerPrivateFunctionEntryBindingError {}

/// Bind a caller-supplied, already-admitted entry to the exact compiler-private
/// function row and installed occurrence that contain its executable bytes.
/// This gate never derives an entry identity or resolves an executable address.
pub fn bind_installed_compiler_private_function_entry(
    installed_artifact: &InstalledArtifact,
    private_function: MachineFunctionIdentity,
    entry: EntryStubId,
) -> Result<InstalledCompilerPrivateFunctionEntry, InstalledCompilerPrivateFunctionEntryBindingError>
{
    let reject = |diagnostic: &str| InstalledCompilerPrivateFunctionEntryBindingError {
        private_function,
        entry,
        diagnostic: diagnostic.into(),
    };
    let mut matches = installed_artifact
        .installation
        .private_functions()
        .iter()
        .filter(|candidate| candidate.identity == private_function);
    let Some(candidate) = matches.next() else {
        return Err(reject(
            "installed artifact does not retain the requested compiler-private function",
        ));
    };
    if matches.next().is_some() {
        return Err(reject(
            "installed artifact retains the compiler-private function more than once",
        ));
    }
    let Some(end) = candidate.text_offset.checked_add(candidate.byte_count) else {
        return Err(reject(
            "compiler-private function text interval is not representable",
        ));
    };
    let Some(expected_bytes) = installed_artifact
        .image
        .output()
        .final_text_bytes
        .get(candidate.text_offset..end)
        .filter(|bytes| !bytes.is_empty())
    else {
        return Err(reject(
            "compiler-private function text interval is empty or outside the final image",
        ));
    };
    let Ok(expected_offset) = u64::try_from(candidate.text_offset) else {
        return Err(reject(
            "compiler-private function entry offset is not representable",
        ));
    };
    if installed_artifact
        .installed
        .selected_entry_target(entry)
        .is_err()
    {
        return Err(reject(
            "entry is not admitted by the exact installed artifact",
        ));
    }
    if !installed_artifact
        .installed
        .binds_entry_offset(entry, expected_offset)
    {
        return Err(reject(
            "entry does not begin at the compiler-private function text offset",
        ));
    }
    if !installed_artifact
        .installed
        .binds_exact_materialized_entry_bytes(entry, expected_bytes)
    {
        return Err(reject(
            "entry does not retain the compiler-private function's exact final bytes",
        ));
    }

    Ok(InstalledCompilerPrivateFunctionEntry {
        private_function: candidate.clone(),
        entry,
        artifact: installed_artifact.installed.artifact(),
        installed_code: installed_artifact.installed.identity(),
        installed_context: installed_artifact.installed.receipt_context(),
    })
}

/// Bind canonical terminal installation metadata to one exact installed code
/// occurrence. Neither an image fingerprint nor an artifact ID can substitute
/// for the byte-bearing values replayed here.
pub fn bind_installed_artifact(
    object: ObjectArtifact,
    image: ExecutableImage,
    installation: InstallationRecord,
    installed: InstalledCode,
) -> Result<InstalledArtifact, Box<InstalledArtifactBindingError>> {
    let reject = |object, image, installation, installed, diagnostic: String| {
        Err(Box::new(InstalledArtifactBindingError {
            object,
            image,
            installation,
            installed,
            diagnostic,
        }))
    };

    if object.psi() != image.psi() || object.target() != image.target() {
        return reject(
            object,
            image,
            installation,
            installed,
            "terminal object and executable image have different semantic or target identity"
                .into(),
        );
    }
    if let Err(error) = validate_installation_record(&installation, &image) {
        return reject(
            object,
            image,
            installation,
            installed,
            format!("terminal installation record does not bind the exact image: {error}"),
        );
    }
    if installed.architecture() != object.target().architecture {
        return reject(
            object,
            image,
            installation,
            installed,
            "installed code architecture differs from the terminal artifact target".into(),
        );
    }
    let memory = match project_installed_artifact_memory_images(&object, &image) {
        Ok(memory) => memory,
        Err(error) => {
            return reject(object, image, installation, installed, error.to_string());
        }
    };
    if !installed.binds_exact_materialized_artifact_bytes(memory.encoded(), memory.materialized()) {
        return reject(
            object,
            image,
            installation,
            installed,
            "installed code does not contain the exact unrelocated and materialized terminal text/data sections"
                .into(),
        );
    }

    Ok(InstalledArtifact {
        object,
        image,
        installation,
        installed,
    })
}
