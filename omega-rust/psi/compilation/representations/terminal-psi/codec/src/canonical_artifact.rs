//! Canonical, source-free Terminal-Psi handoff artifact.
//!
//! This is the exact ownership seam between Psi semantic production and Omega
//! target realization. It contains canonical semantic, pre-Terminal
//! optimization, and optional debug bytes plus their manifest identity. Target selection,
//! provider realization, deployment policy, output paths, and installation
//! authority are deliberately absent.

use terminal_psi::TerminalModule;

use crate::{
    ArtifactManifestError, CodecError, DebugMapError,
    PsiOptimizationExecutionRecord, PsiOptimizationExecutionRecordDecodeError,
    TerminalArtifactManifest, TerminalDebugMap,
    build_artifact_manifest, decode_debug_map, decode_module,
    decode_psi_optimization_execution_record, encode_debug_map, encode_module,
    encode_psi_optimization_execution_record, validate_artifact_manifest,
};

const ARTIFACT_MAGIC: &[u8; 8] = b"PSIART\0\0";
const ARTIFACT_FORMAT_MARKER: u16 = 3;

/// One exact canonical Terminal-Psi semantic artifact.
///
/// `Clone` copies the private section bytes and the manifest verbatim. The
/// artifact's invariant is a property of exactly those bytes, so a clone
/// satisfies it for the same reason the original does, and it carries the
/// original's replay and decode conclusions rather than re-deriving them. A
/// caller that needs an owned artifact beside a borrowed one clones it; it
/// does not reproduce it through `from_bytes(&artifact.to_bytes())`, which
/// would re-run the whole envelope admission to reach the identical value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "a canonical Terminal artifact owns the Psi-to-Omega handoff bytes"]
pub struct CanonicalTerminalArtifact {
    semantic_bytes: Vec<u8>,
    optimization_bytes: Vec<u8>,
    optimization: PsiOptimizationExecutionRecord,
    debug_bytes: Option<Vec<u8>>,
    manifest: TerminalArtifactManifest,
    replayed: ReplayMemo<()>,
    /// The canonical semantic module this artifact's bytes decode to, retained
    /// from the first reader onwards. Decoding is the artifact's most
    /// expensive admission step and every custody, settlement and realization
    /// check needs the same module, so the decode is paid once per artifact
    /// rather than once per reader.
    semantic_module: ReplayMemo<TerminalModule>,
}

/// One conclusion derived from an artifact's sections and retained for later
/// readers. `replayed: ReplayMemo<()>` holds a successful section replay;
/// `semantic_module: ReplayMemo<TerminalModule>` holds the decoded module.
/// The sections are private and never change after construction, so deriving
/// the same conclusion from the same artifact again reaches the same answer;
/// the compiler's custody checks interrogate one artifact many times per
/// compilation. Equality ignores a memo, and its debug form carries no state.
struct ReplayMemo<Conclusion>(std::sync::OnceLock<Conclusion>);

impl<Conclusion> Default for ReplayMemo<Conclusion> {
    fn default() -> Self {
        Self(std::sync::OnceLock::new())
    }
}

impl<Conclusion> PartialEq for ReplayMemo<Conclusion> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<Conclusion> Eq for ReplayMemo<Conclusion> {}

impl<Conclusion> std::fmt::Debug for ReplayMemo<Conclusion> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ReplayMemo")
    }
}

impl<Conclusion: Clone> Clone for ReplayMemo<Conclusion> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl CanonicalTerminalArtifact {
    /// Encode one source-free Terminal artifact.
    ///
    /// The sections are this call's own encoding of these inputs, so the
    /// artifact starts replayed and holds the module it encoded. Retained
    /// bytes cross a trust boundary and are read back through
    /// [`Self::from_bytes`], which decodes and re-decides every section.
    pub fn from_parts(
        semantic_module: &TerminalModule,
        optimization: &PsiOptimizationExecutionRecord,
        debug_map: Option<&TerminalDebugMap>,
    ) -> Result<Self, CanonicalTerminalArtifactError> {
        let semantic_bytes =
            encode_module(semantic_module).map_err(CanonicalTerminalArtifactError::Semantic)?;
        let optimization_bytes = encode_psi_optimization_execution_record(optimization);
        let debug_bytes = debug_map
            .map(|debug_map| {
                encode_debug_map(semantic_module, debug_map)
                    .map_err(CanonicalTerminalArtifactError::Debug)
            })
            .transpose()?;
        let manifest = build_artifact_manifest(
            semantic_module,
            optimization,
            None,
            debug_bytes.as_deref(),
        )
        .map_err(CanonicalTerminalArtifactError::Manifest)?;
        let artifact = Self {
            semantic_bytes,
            optimization_bytes,
            optimization: optimization.clone(),
            debug_bytes,
            manifest,
            replayed: ReplayMemo::default(),
            semantic_module: ReplayMemo::default(),
        };
        let _ = artifact.replayed.0.set(());
        let _ = artifact.semantic_module.0.set(semantic_module.clone());
        Ok(artifact)
    }

    /// Re-decode every canonical section and reconstruct the manifest.
    pub fn validate(&self) -> Result<(), CanonicalTerminalArtifactError> {
        if self.replayed.0.get().is_some() {
            return Ok(());
        }
        let verdict = self.replay_sections();
        if verdict.is_ok() {
            let _ = self.replayed.0.set(());
        }
        verdict
    }

    fn replay_sections(&self) -> Result<(), CanonicalTerminalArtifactError> {
        let semantic_module = self
            .semantic_module()
            .map_err(CanonicalTerminalArtifactError::Semantic)?;
        let optimization = decode_psi_optimization_execution_record(&self.optimization_bytes)
            .map_err(CanonicalTerminalArtifactError::Optimization)?;
        if optimization != self.optimization {
            return Err(CanonicalTerminalArtifactEnvelopeError::NonCanonicalSections.into());
        }
        if let Some(debug_bytes) = self.debug_bytes.as_deref() {
            decode_debug_map(semantic_module, debug_bytes)
                .map_err(CanonicalTerminalArtifactError::Debug)?;
        }
        validate_artifact_manifest(
            semantic_module,
            &optimization,
            None,
            self.debug_bytes.as_deref(),
            self.manifest,
        )
        .map_err(CanonicalTerminalArtifactError::Manifest)
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
        bytes.extend_from_slice(&ARTIFACT_FORMAT_MARKER.to_le_bytes());
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
        let mut cursor = ArtifactCursor::new(bytes);
        if cursor.take(ARTIFACT_MAGIC.len())? != ARTIFACT_MAGIC {
            return Err(CanonicalTerminalArtifactEnvelopeError::InvalidMagic.into());
        }
        let marker = u16::from_le_bytes(cursor.array()?);
        if marker != ARTIFACT_FORMAT_MARKER {
            return Err(
                CanonicalTerminalArtifactEnvelopeError::UnsupportedFormatMarker(marker).into(),
            );
        }
        let semantic_len = cursor.section_len("semantic")?;
        let optimization_len = cursor.section_len("optimization")?;
        let debug_len = match cursor.byte()? {
            0 => None,
            1 => Some(cursor.section_len("debug")?),
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
        let artifact = Self::from_parts(&semantic_module, &optimization, debug_map.as_ref())?;
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

    /// The canonical semantic module these bytes decode to.
    ///
    /// The module is produced by [`crate::decode_module`], which replays every
    /// codec validation — scalar qualification, canonical ordering, the
    /// structural foundation, the module representation — and re-encodes the
    /// result to reject any non-canonical byte sequence. A reader of this
    /// module therefore holds exactly the evidence a direct
    /// `decode_module(artifact.semantic_bytes())` would establish; the only
    /// difference is that the artifact pays for that decode once and retains
    /// the conclusion, because its bytes are private and immutable.
    pub fn semantic_module(&self) -> Result<&TerminalModule, CodecError> {
        if let Some(module) = self.semantic_module.0.get() {
            return Ok(module);
        }
        let module = decode_module(&self.semantic_bytes)?;
        Ok(self.semantic_module.0.get_or_init(|| module))
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

struct ArtifactCursor<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> ArtifactCursor<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'bytes [u8], CanonicalTerminalArtifactEnvelopeError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], CanonicalTerminalArtifactEnvelopeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd)
    }

    fn byte(&mut self) -> Result<u8, CanonicalTerminalArtifactEnvelopeError> {
        Ok(self.array::<1>()?[0])
    }

    fn section_len(
        &mut self,
        section: &'static str,
    ) -> Result<usize, CanonicalTerminalArtifactEnvelopeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| CanonicalTerminalArtifactEnvelopeError::SectionTooLong(section))
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalTerminalArtifactError {
    Semantic(CodecError),
    Optimization(PsiOptimizationExecutionRecordDecodeError),
    Debug(DebugMapError),
    Manifest(ArtifactManifestError),
    Envelope(CanonicalTerminalArtifactEnvelopeError),
}

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
