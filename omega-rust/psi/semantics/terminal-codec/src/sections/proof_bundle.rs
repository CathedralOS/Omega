//! The proof-bundle section: encoding and decoding terminal proof bundles
//! and their fingerprints.
//!
//! This file owns the bundle envelope and the section entry points.
//! `evidence_codec.rs` carries evidence producers, routes and component
//! certificates, `proof_node_codec.rs` the proof nodes and rules,
//! `proposition_codec.rs` propositions and content terms,
//! `scalar_term_codec.rs` scalar terms, types and values, `codec_error.rs`
//! the error, `synopsis.rs` the rendered synopsis, `validation.rs` the
//! decoded-bundle validation and `wire.rs` the byte reader and writer.

mod codec_error;
mod evidence_codec;
mod proof_node_codec;
mod proposition_codec;
mod scalar_term_codec;
mod signature_codec;
mod synopsis;
mod validation;
mod wire;

pub use codec_error::ProofCodecError;
pub use signature_codec::{
    DecodedMathematicalSignature, decode_mathematical_signature, encode_mathematical_signature,
};
pub use synopsis::render_verified_proof_synopsis;

use crate::sections::proof_bundle::evidence_codec::{
    decode_component_certificate, decode_evidence, decode_evidence_producer,
    decode_recursive_component_evidence, encode_component_certificate, encode_evidence,
    encode_evidence_producer,
};
use sha2::{Digest, Sha256};
use terminal_psi::{
    ControlCycleEvidence, SemanticFingerprint, TerminalModule, TerminalPsiIdentity,
    VocabularyMarker,
};
use terminal_verifier::ProofBundle;
use validation::validate_bundle;
use wire::{Reader, Writer};

const MAGIC: &[u8; 8] = b"PSIPRF\0\0";

/// Single current pre-release proof vocabulary marker.
pub(crate) const FORMAT_MARKER: u16 = 33;

/// Subject-sealed canonical proof section: the artifact-bound form of a proof
/// bundle. The section header names the exact semantic subject the bundle was
/// admitted for so a sealed proof cannot be replayed for another subject even
/// when compact obligation coordinates coincide.
const SECTION_MAGIC: &[u8; 8] = b"PSIPSC\0\0";

const SECTION_FORMAT_MARKER: u16 = 1;

const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-proof-bundle-fingerprint\0";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofBundleFingerprint([u8; 32]);

impl ProofBundleFingerprint {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for ProofBundleFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for ProofBundleFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

pub fn encode_proof_bundle(bundle: &ProofBundle) -> Result<Vec<u8>, ProofCodecError> {
    validate_bundle(bundle)?;
    encode_raw(bundle, FORMAT_MARKER)
}

/// Seal one proof bundle into its canonical artifact proof section, binding
/// the section to the module's exact reconstructed semantic identity. The
/// subject is computed from the module here; a producer cannot choose the
/// verifier's root subject.
pub fn encode_proof_section(
    module: &TerminalModule,
    bundle: &ProofBundle,
) -> Result<Vec<u8>, ProofCodecError> {
    let subject = crate::terminal_psi_identity(module).map_err(ProofCodecError::SubjectIdentity)?;
    let bundle_bytes = encode_proof_bundle(bundle)?;
    let mut bytes = Vec::with_capacity(SECTION_MAGIC.len() + 2 + 2 + 32 + bundle_bytes.len());
    bytes.extend_from_slice(SECTION_MAGIC);
    bytes.extend_from_slice(&SECTION_FORMAT_MARKER.to_le_bytes());
    bytes.extend_from_slice(&subject.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(subject.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&bundle_bytes);
    Ok(bytes)
}

/// Decode a sealed proof section into its claimed semantic subject and proof
/// bundle. The claim is not trusted by this decoder: a boundary pairing the
/// section with a module must require the claim to equal the identity
/// reconstructed from that module (see [`decode_proof_section_for`]).
pub fn decode_proof_section(
    bytes: &[u8],
) -> Result<(TerminalPsiIdentity, ProofBundle), ProofCodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(SECTION_MAGIC.len())? != SECTION_MAGIC {
        return Err(ProofCodecError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != SECTION_FORMAT_MARKER {
        return Err(ProofCodecError::UnsupportedFormatMarker(format_marker));
    }
    let raw_vocabulary = reader.u16()?;
    let vocabulary_marker = VocabularyMarker::new(raw_vocabulary).ok_or(
        ProofCodecError::UnsupportedProofSectionVocabulary(raw_vocabulary),
    )?;
    let program_fingerprint = SemanticFingerprint::from_bytes(reader.array::<32>()?);
    let bundle_offset = bytes.len() - reader.remaining();
    let bundle = decode_proof_bundle(&bytes[bundle_offset..])?;
    Ok((
        TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint,
        },
        bundle,
    ))
}

/// Decode a sealed proof section and require its claimed subject to equal the
/// identity reconstructed from this exact module. Unsealed proof bundles are
/// rejected here: every boundary that pairs a proof with a module uses this
/// subject-checking decode.
pub fn decode_proof_section_for(
    module: &TerminalModule,
    bytes: &[u8],
) -> Result<ProofBundle, ProofCodecError> {
    let (claimed, bundle) = decode_proof_section(bytes)?;
    let reconstructed =
        crate::terminal_psi_identity(module).map_err(ProofCodecError::SubjectIdentity)?;
    if claimed != reconstructed {
        return Err(ProofCodecError::ProofSubjectMismatch {
            claimed,
            reconstructed,
        });
    }
    Ok(bundle)
}

pub fn decode_proof_bundle(bytes: &[u8]) -> Result<ProofBundle, ProofCodecError> {
    if bytes.starts_with(SECTION_MAGIC) {
        // A sealed proof section decodes to its bundle here; the subject
        // claim is intentionally not inspected. Boundaries that pair a proof
        // section with a module must use decode_proof_section_for so the seal
        // is enforced.
        return decode_proof_section(bytes).map(|(_, bundle)| bundle);
    }
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(ProofCodecError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != FORMAT_MARKER {
        return Err(ProofCodecError::UnsupportedFormatMarker(format_marker));
    }
    let evidence_count = reader.count()?;
    let mut evidence = Vec::new();
    for _ in 0..evidence_count {
        evidence.push(decode_evidence(&mut reader, format_marker)?);
    }
    let recursive_component_count = reader.count()?;
    let mut recursive_components = Vec::new();
    for _ in 0..recursive_component_count {
        recursive_components.push(decode_recursive_component_evidence(
            &mut reader,
            format_marker,
        )?);
    }
    let control_cycle_count = reader.count()?;
    let mut control_cycles = Vec::new();
    for _ in 0..control_cycle_count {
        control_cycles.push(ControlCycleEvidence {
            component: reader.id("CycleComponentId")?,
            certificate: decode_component_certificate(&mut reader, format_marker)?,
        });
    }
    let producer_count = reader.count()?;
    let mut evidence_producers = Vec::new();
    for _ in 0..producer_count {
        evidence_producers.push(decode_evidence_producer(&mut reader)?);
    }
    if reader.remaining() != 0 {
        return Err(ProofCodecError::TrailingBytes(reader.remaining()));
    }
    let bundle = ProofBundle {
        evidence,
        recursive_components,
        control_cycles,
        evidence_producers,
    };
    validate_bundle(&bundle)?;
    if encode_raw(&bundle, format_marker)? != bytes {
        return Err(ProofCodecError::NonCanonicalEncoding);
    }
    Ok(bundle)
}

pub fn proof_bundle_fingerprint(
    bundle: &ProofBundle,
) -> Result<ProofBundleFingerprint, ProofCodecError> {
    let bytes = encode_proof_bundle(bundle)?;
    let mut digest = Sha256::new();
    digest.update(FINGERPRINT_DOMAIN);
    let byte_len = u64::try_from(bytes.len()).expect("proof-bundle bytes fit the digest domain");
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    Ok(ProofBundleFingerprint(digest.finalize().into()))
}

fn encode_raw(bundle: &ProofBundle, format_marker: u16) -> Result<Vec<u8>, ProofCodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(format_marker);
    writer.len("evidence", bundle.evidence.len())?;
    for evidence in &bundle.evidence {
        encode_evidence(&mut writer, evidence, format_marker)?;
    }
    writer.len(
        "recursive component evidence",
        bundle.recursive_components.len(),
    )?;
    for component in &bundle.recursive_components {
        writer.id(component.component);
        encode_component_certificate(&mut writer, &component.certificate, format_marker)?;
    }
    writer.len("control cycle evidence", bundle.control_cycles.len())?;
    for component in &bundle.control_cycles {
        writer.id(component.component);
        encode_component_certificate(&mut writer, &component.certificate, format_marker)?;
    }
    writer.len("evidence producers", bundle.evidence_producers.len())?;
    for producer in &bundle.evidence_producers {
        encode_evidence_producer(&mut writer, producer)?;
    }
    Ok(writer.finish())
}
