//! Canonical, source-free Terminal-Psi handoff artifact.
//!
//! This is the exact ownership seam between Psi semantic production and Omega
//! target realization. It contains canonical semantic, pre-Terminal
//! optimization, and optional debug bytes plus their manifest identity. Target selection,
//! provider realization, deployment policy, output paths, and installation
//! authority are deliberately absent.

use crate::cursor::Cursor;
use terminal_psi::TerminalModule;

use crate::{
    ArtifactManifestError, CodecError, DebugMapError, PsiOptimizationExecutionRecord,
    PsiOptimizationExecutionRecordDecodeError, TerminalArtifactManifest, TerminalDebugMap,
    build_artifact_manifest, decode_debug_map, decode_module,
    decode_psi_optimization_execution_record, encode_debug_map, encode_module,
    encode_psi_optimization_execution_record,
};

const ARTIFACT_MAGIC: &[u8; 8] = b"PSIART\0\0";
const ARTIFACT_FORMAT_MARKER: u16 = 3;

/// An immutable semantic module paired with the identity of its canonical
/// artifact. This view borrows either producer-retained storage or storage
/// admitted at external ingress; consumers do not reconstruct it from bytes.
///
/// Private fields prevent pairing an unrelated module with an artifact's
/// identity. Canonical encoding establishes wire order, not structural
/// validity, proof validity or permission to install a provider.
#[derive(Debug, Clone, Copy)]
pub struct TerminalArtifactView<'artifact> {
    module: &'artifact TerminalModule,
    identity: terminal_psi::TerminalPsiIdentity,
}

impl<'artifact> TerminalArtifactView<'artifact> {
    pub const fn module(self) -> &'artifact TerminalModule {
        self.module
    }

    pub const fn identity(self) -> terminal_psi::TerminalPsiIdentity {
        self.identity
    }
}

/// One exact canonical Terminal-Psi semantic artifact.
///
/// Construction establishes canonical sections and retains their semantic
/// storage. Readers borrow that storage directly; there is no lazy decoding or
/// replay state. `Clone` explicitly copies both bytes and semantic storage;
/// in-process consumers should borrow a view instead.
#[derive(Clone)]
#[must_use = "a canonical Terminal artifact owns the Psi-to-Omega handoff bytes"]
pub struct CanonicalTerminalArtifact {
    semantic_bytes: Vec<u8>,
    optimization_bytes: Vec<u8>,
    optimization: PsiOptimizationExecutionRecord,
    debug_bytes: Option<Vec<u8>>,
    manifest: TerminalArtifactManifest,
    semantic_module: TerminalModule,
}

// Canonical bytes determine the retained module. Comparing or printing that
// second representation would traverse the program again without adding data.
impl PartialEq for CanonicalTerminalArtifact {
    fn eq(&self, other: &Self) -> bool {
        self.manifest == other.manifest
            && self.semantic_bytes == other.semantic_bytes
            && self.optimization_bytes == other.optimization_bytes
            && self.debug_bytes == other.debug_bytes
    }
}

impl Eq for CanonicalTerminalArtifact {}

impl std::fmt::Debug for CanonicalTerminalArtifact {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CanonicalTerminalArtifact")
            .field("manifest", &self.manifest)
            .field("semantic_byte_count", &self.semantic_bytes.len())
            .field("optimization", &self.optimization)
            .field("debug_byte_count", &self.debug_bytes.as_ref().map(Vec::len))
            .finish_non_exhaustive()
    }
}

impl CanonicalTerminalArtifact {
    /// Encode one source-free Terminal artifact.
    ///
    /// The sections are this call's own encoding of these inputs, so the
    /// artifact owns the module it encoded without copying it. External
    /// bytes cross a trust boundary and are read back through
    /// [`Self::from_bytes`], which decodes and re-decides every section.
    pub fn from_parts(
        semantic_module: TerminalModule,
        optimization: PsiOptimizationExecutionRecord,
        debug_map: Option<&TerminalDebugMap>,
    ) -> Result<Self, CanonicalTerminalArtifactError> {
        let semantic_bytes =
            encode_module(&semantic_module).map_err(CanonicalTerminalArtifactError::Semantic)?;
        let optimization_bytes = encode_psi_optimization_execution_record(&optimization);
        let debug_bytes = debug_map
            .map(|debug_map| {
                encode_debug_map(&semantic_module, debug_map)
                    .map_err(CanonicalTerminalArtifactError::Debug)
            })
            .transpose()?;
        let manifest = build_artifact_manifest(
            &semantic_module,
            &optimization,
            None,
            debug_bytes.as_deref(),
        )
        .map_err(CanonicalTerminalArtifactError::Manifest)?;
        Ok(Self {
            semantic_bytes,
            optimization_bytes,
            optimization,
            debug_bytes,
            manifest,
            semantic_module,
        })
    }

    /// Serialize every canonical Terminal-Psi section into one source-free
    /// transport envelope. The manifest is reconstructed by the receiver from
    /// the exact section bytes rather than trusted as redundant input.
    pub fn to_bytes(&self) -> Vec<u8> {
        let debug_header_bytes = if self.debug_bytes.is_some() { 9 } else { 1 };
        let capacity = ARTIFACT_MAGIC
            .len()
            .saturating_add(2)
            .saturating_add(16)
            .saturating_add(debug_header_bytes)
            .saturating_add(self.semantic_bytes.len())
            .saturating_add(self.optimization_bytes.len())
            .saturating_add(self.debug_bytes.as_ref().map_or(0, Vec::len));
        let mut bytes = Vec::with_capacity(capacity);
        bytes.extend_from_slice(ARTIFACT_MAGIC);
        crate::codec_bytes::push_encoded(&mut bytes, ARTIFACT_FORMAT_MARKER.to_le_bytes());
        encode_section_len(&mut bytes, self.semantic_bytes.len());
        encode_section_len(&mut bytes, self.optimization_bytes.len());
        match &self.debug_bytes {
            None => bytes.push(0),
            Some(debug) => {
                bytes.push(1);
                encode_section_len(&mut bytes, debug.len());
            }
        }
        bytes.extend_from_slice(&self.semantic_bytes);
        bytes.extend_from_slice(&self.optimization_bytes);
        if let Some(debug) = &self.debug_bytes {
            bytes.extend_from_slice(debug);
        }
        bytes
    }

    /// Decode and independently replay a complete source-free transport
    /// envelope. No producer-owned module or manifest crosses
    /// this boundary.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CanonicalTerminalArtifactError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(ARTIFACT_MAGIC.len())? != ARTIFACT_MAGIC {
            return Err(CanonicalTerminalArtifactEnvelopeError::InvalidMagic.into());
        }
        let marker = cursor.u16()?;
        if marker != ARTIFACT_FORMAT_MARKER {
            return Err(
                CanonicalTerminalArtifactEnvelopeError::UnsupportedFormatMarker(marker).into(),
            );
        }
        let semantic_len = section_len(&mut cursor, "semantic")?;
        let optimization_len = section_len(&mut cursor, "optimization")?;
        let debug_len = match cursor.byte()? {
            0 => None,
            1 => Some(section_len(&mut cursor, "debug")?),
            tag => return Err(CanonicalTerminalArtifactEnvelopeError::InvalidDebugTag(tag).into()),
        };
        let semantic_bytes = cursor.take(semantic_len)?;
        let optimization_bytes = cursor.take(optimization_len)?;
        let debug_bytes = debug_len.map(|len| cursor.take(len)).transpose()?;
        if cursor.remaining() != 0 {
            return Err(
                CanonicalTerminalArtifactEnvelopeError::TrailingBytes(cursor.remaining()).into(),
            );
        }

        let semantic_module =
            decode_module(semantic_bytes).map_err(CanonicalTerminalArtifactError::Semantic)?;
        let optimization = decode_psi_optimization_execution_record(optimization_bytes)
            .map_err(CanonicalTerminalArtifactError::Optimization)?;
        let debug_map = debug_bytes
            .map(|debug| {
                decode_debug_map(&semantic_module, debug)
                    .map_err(CanonicalTerminalArtifactError::Debug)
            })
            .transpose()?;
        let artifact = Self::from_parts(semantic_module, optimization, debug_map.as_ref())?;
        if artifact.semantic_bytes() != semantic_bytes
            || artifact.optimization_bytes() != optimization_bytes
            || artifact.debug_bytes() != debug_bytes
        {
            return Err(CanonicalTerminalArtifactEnvelopeError::NonCanonicalSections.into());
        }
        Ok(artifact)
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        &self.semantic_bytes
    }

    /// Borrow the canonical module and its already established identity.
    /// This does not re-encode, clone or re-admit the artifact.
    pub fn view(&self) -> TerminalArtifactView<'_> {
        TerminalArtifactView {
            module: &self.semantic_module,
            identity: self.manifest.semantic(),
        }
    }

    /// Transfer retained semantic storage to an execution owner without a
    /// second byte load. All constructors retain this module before returning.
    pub fn into_semantics(self) -> (TerminalModule, terminal_psi::TerminalPsiIdentity) {
        (self.semantic_module, self.manifest.semantic())
    }

    /// The canonical semantic module these bytes decode to.
    ///
    /// Producer construction retains the module serialized by `encode_module`;
    /// external ingress retains the module reconstructed by `decode_module`.
    /// Only external decoding validates structural foundations. Neither route
    /// establishes semantic proof validity. Readers share that immutable module
    /// rather than repeating its encoding or decoding.
    pub fn semantic_module(&self) -> &TerminalModule {
        &self.semantic_module
    }

    pub fn optimization_bytes(&self) -> &[u8] {
        &self.optimization_bytes
    }

    pub const fn optimization(&self) -> &PsiOptimizationExecutionRecord {
        &self.optimization
    }

    pub fn debug_bytes(&self) -> Option<&[u8]> {
        self.debug_bytes.as_deref()
    }

    pub const fn manifest(&self) -> TerminalArtifactManifest {
        self.manifest
    }
}

fn encode_section_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(
        &u64::try_from(len)
            .expect("an in-memory Terminal artifact section fits u64")
            .to_le_bytes(),
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalTerminalArtifactError {
    Semantic(CodecError),
    Optimization(PsiOptimizationExecutionRecordDecodeError),
    Debug(DebugMapError),
    Manifest(ArtifactManifestError),
    Envelope(CanonicalTerminalArtifactEnvelopeError),
}

optimization::truncated_from_adapter!(CanonicalTerminalArtifactError => Self::Envelope(CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd));

impl From<CanonicalTerminalArtifactEnvelopeError> for CanonicalTerminalArtifactError {
    fn from(error: CanonicalTerminalArtifactEnvelopeError) -> Self {
        Self::Envelope(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalTerminalArtifactEnvelopeError {
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    InvalidDebugTag(u8),
    SectionTooLong(&'static str),
    UnexpectedEnd,
    TrailingBytes(usize),
    NonCanonicalSections,
}

impl std::fmt::Display for CanonicalTerminalArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CanonicalTerminalArtifactError {}

optimization::truncated_from_adapter!(CanonicalTerminalArtifactEnvelopeError => Self::UnexpectedEnd);

fn section_len(
    cursor: &mut Cursor,
    section: &'static str,
) -> Result<usize, CanonicalTerminalArtifactEnvelopeError> {
    usize::try_from(cursor.u64()?)
        .map_err(|_| CanonicalTerminalArtifactEnvelopeError::SectionTooLong(section))
}
