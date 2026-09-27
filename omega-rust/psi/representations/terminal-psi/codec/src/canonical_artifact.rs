//! Canonical, source-free Terminal-Psi handoff artifact.
//!
//! This is the exact ownership seam between Psi semantic production and Omega
//! target realization. It contains canonical semantic, proof, pre-Terminal
//! optimization, and optional debug bytes plus their manifest identity. Target selection,
//! provider realization, deployment policy, output paths, and installation
//! authority are deliberately absent.

use terminal_psi::TerminalModule;
use terminal_verifier::ProofBundle;

use crate::{
    ArtifactManifestError, CodecError, DebugMapError, ProofCodecError,
    PsiOptimizationExecutionRecord, PsiOptimizationExecutionRecordDecodeError,
    TerminalArtifactManifest, TerminalDebugMap,
    build_artifact_manifest_with_mathematical_signature, decode_debug_map,
    decode_mathematical_signature, decode_module, decode_proof_section_for,
    decode_psi_optimization_execution_record, encode_debug_map, encode_module,
    encode_proof_section, encode_psi_optimization_execution_record,
    validate_artifact_manifest_with_mathematical_signature,
};

const ARTIFACT_MAGIC: &[u8; 8] = b"PSIART\0\0";
const ARTIFACT_FORMAT_MARKER: u16 = 2;

/// One exact canonical Terminal-Psi semantic/proof artifact.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "a canonical Terminal artifact owns the Psi-to-Omega handoff bytes"]
pub struct CanonicalTerminalArtifact {
    semantic_bytes: Vec<u8>,
    proof_bytes: Vec<u8>,
    optimization_bytes: Vec<u8>,
    optimization: PsiOptimizationExecutionRecord,
    debug_bytes: Option<Vec<u8>>,
    /// The canonical mathematical-signature wire, present exactly when the
    /// checked program carried mathematical declarations. It is evidence
    /// about the checked cohort — not module state — so it rides the
    /// envelope as an optional trailing section bound by the manifest
    /// fingerprint rather than joining `ProofBundle`.
    mathematical_signature_bytes: Option<Vec<u8>>,
    manifest: TerminalArtifactManifest,
    replayed: SectionReplay,
}

/// Set once `validate` has replayed an artifact's sections successfully.
/// The sections are private and never change after construction, so a later
/// replay of the same artifact reaches the same verdict; the compiler's
/// custody checks revalidate one artifact many times per compilation.
/// Equality ignores it, and its debug form carries no state.
#[derive(Default)]
struct SectionReplay(std::sync::OnceLock<()>);

impl PartialEq for SectionReplay {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for SectionReplay {}

impl std::fmt::Debug for SectionReplay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SectionReplay")
    }
}

impl CanonicalTerminalArtifact {
    /// Encode and independently replay one source-free Terminal artifact.
    /// The artifact carries no mathematical-signature section; a program
    /// whose checked cohort held mathematical declarations must publish
    /// through [`Self::from_parts_with_mathematical_signature`].
    pub fn from_parts(
        semantic_module: &TerminalModule,
        proof_bundle: &ProofBundle,
        optimization: &PsiOptimizationExecutionRecord,
        debug_map: Option<&TerminalDebugMap>,
    ) -> Result<Self, CanonicalTerminalArtifactError> {
        Self::from_parts_with_mathematical_signature(
            semantic_module,
            proof_bundle,
            optimization,
            debug_map,
            None,
        )
    }

    /// Encode and independently replay one source-free Terminal artifact that
    /// also carries the checked mathematical signature.
    ///
    /// `mathematical_signature` is the canonical `PSISIG` wire produced by
    /// [`crate::encode_mathematical_signature`], `Some` exactly when the
    /// checked program carried mathematical declarations. The bytes are
    /// decoded and kernel re-decided during the construction replay, so an
    /// artifact whose signature evidence does not verify is never built.
    pub fn from_parts_with_mathematical_signature(
        semantic_module: &TerminalModule,
        proof_bundle: &ProofBundle,
        optimization: &PsiOptimizationExecutionRecord,
        debug_map: Option<&TerminalDebugMap>,
        mathematical_signature: Option<&[u8]>,
    ) -> Result<Self, CanonicalTerminalArtifactError> {
        let semantic_bytes =
            encode_module(semantic_module).map_err(CanonicalTerminalArtifactError::Semantic)?;
        // The proof section is sealed to this module's reconstructed identity
        // by the codec; the producer cannot choose the verifier's subject.
        let proof_bytes = encode_proof_section(semantic_module, proof_bundle)
            .map_err(CanonicalTerminalArtifactError::Proof)?;
        let optimization_bytes = encode_psi_optimization_execution_record(optimization);
        let debug_bytes = debug_map
            .map(|debug_map| {
                encode_debug_map(semantic_module, debug_map)
                    .map_err(CanonicalTerminalArtifactError::Debug)
            })
            .transpose()?;
        let manifest = build_artifact_manifest_with_mathematical_signature(
            semantic_module,
            proof_bundle,
            optimization,
            None,
            debug_bytes.as_deref(),
            mathematical_signature,
        )
        .map_err(CanonicalTerminalArtifactError::Manifest)?;
        let artifact = Self {
            semantic_bytes,
            proof_bytes,
            optimization_bytes,
            optimization: optimization.clone(),
            debug_bytes,
            mathematical_signature_bytes: mathematical_signature.map(<[u8]>::to_vec),
            manifest,
            replayed: SectionReplay::default(),
        };
        artifact.validate()?;
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
        let semantic_module = decode_module(&self.semantic_bytes)
            .map_err(CanonicalTerminalArtifactError::Semantic)?;
        let proof_bundle = decode_proof_section_for(&semantic_module, &self.proof_bytes)
            .map_err(CanonicalTerminalArtifactError::Proof)?;
        let optimization = decode_psi_optimization_execution_record(&self.optimization_bytes)
            .map_err(CanonicalTerminalArtifactError::Optimization)?;
        if optimization != self.optimization {
            return Err(CanonicalTerminalArtifactEnvelopeError::NonCanonicalSections.into());
        }
        if let Some(debug_bytes) = self.debug_bytes.as_deref() {
            decode_debug_map(&semantic_module, debug_bytes)
                .map_err(CanonicalTerminalArtifactError::Debug)?;
        }
        // The signature wire decodes into a fresh arena, must re-encode to
        // its exact bytes, and is re-decided by the kernel's `check_signature`
        // inside the decoder — no producer claim is trusted.
        if let Some(signature_bytes) = self.mathematical_signature_bytes.as_deref() {
            decode_mathematical_signature(signature_bytes)
                .map_err(CanonicalTerminalArtifactError::MathematicalSignature)?;
        }
        validate_artifact_manifest_with_mathematical_signature(
            &semantic_module,
            &proof_bundle,
            &optimization,
            None,
            self.debug_bytes.as_deref(),
            self.mathematical_signature_bytes.as_deref(),
            self.manifest,
        )
        .map_err(CanonicalTerminalArtifactError::Manifest)
    }

    /// Serialize every canonical Terminal-Psi section into one source-free
    /// transport envelope. The manifest is reconstructed by the receiver from
    /// the exact section bytes rather than trusted as redundant input.
    pub fn to_bytes(&self) -> Vec<u8> {
        let debug_header_bytes = if self.debug_bytes.is_some() { 9 } else { 1 };
        // The signature section, when present, is an optional trailing
        // `tag + length + payload` after the four fixed sections: a
        // signature-free artifact emits byte-identical marker-2 output, and
        // a receiver without signature support rejects a signed artifact on
        // its trailing bytes rather than silently dropping the evidence.
        let signature_tail_bytes = if self.mathematical_signature_bytes.is_some() {
            9
        } else {
            0
        };
        let capacity = ARTIFACT_MAGIC
            .len()
            .saturating_add(2)
            .saturating_add(24)
            .saturating_add(debug_header_bytes)
            .saturating_add(signature_tail_bytes)
            .saturating_add(self.semantic_bytes.len())
            .saturating_add(self.proof_bytes.len())
            .saturating_add(self.optimization_bytes.len())
            .saturating_add(self.debug_bytes.as_ref().map_or(0, Vec::len))
            .saturating_add(
                self.mathematical_signature_bytes
                    .as_ref()
                    .map_or(0, Vec::len),
            );
        let mut bytes = Vec::with_capacity(capacity);
        bytes.extend_from_slice(ARTIFACT_MAGIC);
        bytes.extend_from_slice(&ARTIFACT_FORMAT_MARKER.to_le_bytes());
        encode_section_len(&mut bytes, self.semantic_bytes.len());
        encode_section_len(&mut bytes, self.proof_bytes.len());
        encode_section_len(&mut bytes, self.optimization_bytes.len());
        match &self.debug_bytes {
            None => bytes.push(0),
            Some(debug) => {
                bytes.push(1);
                encode_section_len(&mut bytes, debug.len());
            }
        }
        bytes.extend_from_slice(&self.semantic_bytes);
        bytes.extend_from_slice(&self.proof_bytes);
        bytes.extend_from_slice(&self.optimization_bytes);
        if let Some(debug) = &self.debug_bytes {
            bytes.extend_from_slice(debug);
        }
        if let Some(signature) = &self.mathematical_signature_bytes {
            bytes.push(1);
            encode_section_len(&mut bytes, signature.len());
            bytes.extend_from_slice(signature);
        }
        bytes
    }

    /// Decode and independently replay a complete source-free transport
    /// envelope. No producer-owned module, proof object, or manifest crosses
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
        let proof_len = cursor.section_len("proof")?;
        let optimization_len = cursor.section_len("optimization")?;
        let debug_len = match cursor.byte()? {
            0 => None,
            1 => Some(cursor.section_len("debug")?),
            tag => return Err(CanonicalTerminalArtifactEnvelopeError::InvalidDebugTag(tag).into()),
        };
        let semantic_bytes = cursor.take(semantic_len)?;
        let proof_bytes = cursor.take(proof_len)?;
        let optimization_bytes = cursor.take(optimization_len)?;
        let debug_bytes = debug_len.map(|len| cursor.take(len)).transpose()?;
        // After the four fixed payloads, a `1`-tagged tail carries the
        // canonical mathematical-signature section; bytes still remaining at
        // a signature-free envelope's end are the existing trailing-bytes
        // rejection, unchanged for marker-2 artifacts that predate it.
        let mathematical_signature_bytes = if cursor.remaining() == 0 {
            None
        } else {
            match cursor.byte()? {
                1 => {
                    let signature_len = cursor.section_len("mathematical signature")?;
                    Some(cursor.take(signature_len)?)
                }
                tag => {
                    return Err(
                        CanonicalTerminalArtifactEnvelopeError::InvalidMathematicalSignatureTag(
                            tag,
                        )
                        .into(),
                    );
                }
            }
        };
        if cursor.remaining() != 0 {
            return Err(
                CanonicalTerminalArtifactEnvelopeError::TrailingBytes(cursor.remaining()).into(),
            );
        }

        let semantic_module =
            decode_module(semantic_bytes).map_err(CanonicalTerminalArtifactError::Semantic)?;
        let proof_bundle = decode_proof_section_for(&semantic_module, proof_bytes)
            .map_err(CanonicalTerminalArtifactError::Proof)?;
        let optimization = decode_psi_optimization_execution_record(optimization_bytes)
            .map_err(CanonicalTerminalArtifactError::Optimization)?;
        let debug_map = debug_bytes
            .map(|debug| {
                decode_debug_map(&semantic_module, debug)
                    .map_err(CanonicalTerminalArtifactError::Debug)
            })
            .transpose()?;
        let artifact = Self::from_parts_with_mathematical_signature(
            &semantic_module,
            &proof_bundle,
            &optimization,
            debug_map.as_ref(),
            mathematical_signature_bytes,
        )?;
        if artifact.semantic_bytes() != semantic_bytes
            || artifact.proof_bytes() != proof_bytes
            || artifact.optimization_bytes() != optimization_bytes
            || artifact.debug_bytes() != debug_bytes
            || artifact.mathematical_signature_bytes() != mathematical_signature_bytes
        {
            return Err(CanonicalTerminalArtifactEnvelopeError::NonCanonicalSections.into());
        }
        Ok(artifact)
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        &self.semantic_bytes
    }

    pub fn proof_bytes(&self) -> &[u8] {
        &self.proof_bytes
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

    /// The canonical mathematical-signature wire the artifact carries —
    /// `Some` exactly when the checked program's signature was published.
    /// The bytes have already decoded and kernel-verified once; receivers
    /// re-decide them through [`crate::decode_mathematical_signature`].
    pub fn mathematical_signature_bytes(&self) -> Option<&[u8]> {
        self.mathematical_signature_bytes.as_deref()
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
    Proof(ProofCodecError),
    Optimization(PsiOptimizationExecutionRecordDecodeError),
    Debug(DebugMapError),
    /// The mathematical-signature section failed canonical decoding or the
    /// kernel's independent `check_signature` re-decision — producer
    /// signature evidence that does not verify is never admitted.
    MathematicalSignature(ProofCodecError),
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
    InvalidMathematicalSignatureTag(u8),
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
