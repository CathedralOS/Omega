//! Canonical record of the target-neutral optimization phase that produced a
//! Terminal-Psi artifact.

use optimization::{
    PsiOptimizationSelectionDecodeError, PsiOptimizationSelectionIdentity,
    PsiOptimizationSelections,
};
use sha2::{Digest, Sha256};
use terminal_psi::TerminalModule;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
use terminal_verifier::ProofBundle;

use crate::{
    CodecError, ProofBundleFingerprint, ProofCodecError, proof_bundle_fingerprint,
    terminal_psi_identity,
};

const MAGIC: &[u8; 8] = b"PSIOEXE\0";
const FORMAT_MARKER: u16 = 1;
const IDENTITY_DOMAIN: &[u8] = b"psi.preterminal-optimization-execution.v1\0";

/// Strong identity of one selected pre-Terminal optimization execution.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PsiOptimizationExecutionIdentity([u8; 32]);

impl PsiOptimizationExecutionIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for PsiOptimizationExecutionIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Canonical identities before and after one target-neutral optimization
/// stage, including the exact selected pass set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationExecutionRecord {
    selections: PsiOptimizationSelections,
    input_semantic: TerminalPsiIdentity,
    input_proof: ProofBundleFingerprint,
    output_semantic: TerminalPsiIdentity,
    output_proof: ProofBundleFingerprint,
    identity: PsiOptimizationExecutionIdentity,
}

impl PsiOptimizationExecutionRecord {
    pub fn new(
        selections: PsiOptimizationSelections,
        input_semantic: TerminalPsiIdentity,
        input_proof: ProofBundleFingerprint,
        output_semantic: TerminalPsiIdentity,
        output_proof: ProofBundleFingerprint,
    ) -> Result<Self, PsiOptimizationExecutionRecordError> {
        if selections.is_empty()
            && (input_semantic != output_semantic || input_proof != output_proof)
        {
            return Err(PsiOptimizationExecutionRecordError::IdentityStageChangedProduct);
        }
        let identity = execution_identity(
            selections.identity(),
            input_semantic,
            input_proof,
            output_semantic,
            output_proof,
        );
        Ok(Self {
            selections,
            input_semantic,
            input_proof,
            output_semantic,
            output_proof,
            identity,
        })
    }

    pub const fn selections(&self) -> &PsiOptimizationSelections {
        &self.selections
    }

    pub fn selection_identity(&self) -> PsiOptimizationSelectionIdentity {
        self.selections.identity()
    }

    pub fn selection(&self) -> PsiOptimizationSelectionIdentity {
        self.selection_identity()
    }

    pub const fn input_semantic(&self) -> TerminalPsiIdentity {
        self.input_semantic
    }

    pub const fn input_proof(&self) -> ProofBundleFingerprint {
        self.input_proof
    }

    pub const fn output_semantic(&self) -> TerminalPsiIdentity {
        self.output_semantic
    }

    pub const fn output_proof(&self) -> ProofBundleFingerprint {
        self.output_proof
    }

    pub const fn identity(&self) -> PsiOptimizationExecutionIdentity {
        self.identity
    }

    pub fn validate_output(
        &self,
        semantic: TerminalPsiIdentity,
        proof: ProofBundleFingerprint,
    ) -> Result<(), PsiOptimizationExecutionRecordError> {
        if self.output_semantic != semantic || self.output_proof != proof {
            return Err(PsiOptimizationExecutionRecordError::OutputMismatch);
        }
        Ok(())
    }
}

/// Build the canonical record for an explicitly executed identity stage.
///
/// This is useful for source-free fixtures and importers. Production lowering
/// still crosses the pipeline stage whose output is required by publication.
pub fn build_identity_optimization_execution_record(
    module: &TerminalModule,
    proof: &ProofBundle,
) -> Result<PsiOptimizationExecutionRecord, PsiOptimizationExecutionRecordBuildError> {
    let semantic = terminal_psi_identity(module)
        .map_err(PsiOptimizationExecutionRecordBuildError::Semantic)?;
    let proof =
        proof_bundle_fingerprint(proof).map_err(PsiOptimizationExecutionRecordBuildError::Proof)?;
    PsiOptimizationExecutionRecord::new(
        PsiOptimizationSelections::default(),
        semantic,
        proof,
        semantic,
        proof,
    )
    .map_err(PsiOptimizationExecutionRecordBuildError::Record)
}

pub fn encode_psi_optimization_execution_record(
    record: &PsiOptimizationExecutionRecord,
) -> Vec<u8> {
    let selections = record.selections.encode();
    let mut bytes = Vec::with_capacity(10 + 8 + selections.len() + 132);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&FORMAT_MARKER.to_le_bytes());
    bytes.extend_from_slice(
        &u64::try_from(selections.len())
            .expect("the in-memory optimization selection fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&selections);
    append_terminal_identity(&mut bytes, record.input_semantic);
    bytes.extend_from_slice(record.input_proof.as_bytes());
    append_terminal_identity(&mut bytes, record.output_semantic);
    bytes.extend_from_slice(record.output_proof.as_bytes());
    bytes
}

pub fn decode_psi_optimization_execution_record(
    bytes: &[u8],
) -> Result<PsiOptimizationExecutionRecord, PsiOptimizationExecutionRecordDecodeError> {
    let mut cursor = RecordCursor::new(bytes);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(PsiOptimizationExecutionRecordDecodeError::InvalidMagic);
    }
    let marker = u16::from_le_bytes(cursor.array()?);
    if marker != FORMAT_MARKER {
        return Err(PsiOptimizationExecutionRecordDecodeError::UnsupportedFormatMarker(marker));
    }
    let selections_len = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| PsiOptimizationExecutionRecordDecodeError::UnexpectedEnd)?;
    let selections = PsiOptimizationSelections::decode(cursor.take(selections_len)?)
        .map_err(PsiOptimizationExecutionRecordDecodeError::Selection)?;
    let input_semantic = decode_terminal_identity(&mut cursor)?;
    let input_proof = ProofBundleFingerprint::from_bytes(cursor.array()?);
    let output_semantic = decode_terminal_identity(&mut cursor)?;
    let output_proof = ProofBundleFingerprint::from_bytes(cursor.array()?);
    if cursor.remaining() != 0 {
        return Err(PsiOptimizationExecutionRecordDecodeError::TrailingBytes(
            cursor.remaining(),
        ));
    }
    let record = PsiOptimizationExecutionRecord::new(
        selections,
        input_semantic,
        input_proof,
        output_semantic,
        output_proof,
    )
    .map_err(PsiOptimizationExecutionRecordDecodeError::InvalidRecord)?;
    if encode_psi_optimization_execution_record(&record) != bytes {
        return Err(PsiOptimizationExecutionRecordDecodeError::NonCanonicalEncoding);
    }
    Ok(record)
}

fn execution_identity(
    selection: PsiOptimizationSelectionIdentity,
    input_semantic: TerminalPsiIdentity,
    input_proof: ProofBundleFingerprint,
    output_semantic: TerminalPsiIdentity,
    output_proof: ProofBundleFingerprint,
) -> PsiOptimizationExecutionIdentity {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN);
    digest.update(selection.bytes());
    append_identity_to_digest(&mut digest, input_semantic);
    digest.update(input_proof.as_bytes());
    append_identity_to_digest(&mut digest, output_semantic);
    digest.update(output_proof.as_bytes());
    PsiOptimizationExecutionIdentity(digest.finalize().into())
}

fn append_identity_to_digest(digest: &mut Sha256, identity: TerminalPsiIdentity) {
    digest.update(identity.vocabulary_marker.get().to_le_bytes());
    digest.update(identity.program_fingerprint.as_bytes());
}

fn append_terminal_identity(bytes: &mut Vec<u8>, identity: TerminalPsiIdentity) {
    bytes.extend_from_slice(&identity.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(identity.program_fingerprint.as_bytes());
}

fn decode_terminal_identity(
    cursor: &mut RecordCursor<'_>,
) -> Result<TerminalPsiIdentity, PsiOptimizationExecutionRecordDecodeError> {
    let marker = u16::from_le_bytes(cursor.array()?);
    let vocabulary_marker = VocabularyMarker::new(marker)
        .ok_or(PsiOptimizationExecutionRecordDecodeError::UnsupportedVocabularyMarker(marker))?;
    Ok(TerminalPsiIdentity {
        vocabulary_marker,
        program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
    })
}

struct RecordCursor<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> RecordCursor<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(
        &mut self,
        len: usize,
    ) -> Result<&'bytes [u8], PsiOptimizationExecutionRecordDecodeError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(PsiOptimizationExecutionRecordDecodeError::UnexpectedEnd)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(PsiOptimizationExecutionRecordDecodeError::UnexpectedEnd)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], PsiOptimizationExecutionRecordDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PsiOptimizationExecutionRecordDecodeError::UnexpectedEnd)
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiOptimizationExecutionRecordError {
    IdentityStageChangedProduct,
    OutputMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiOptimizationExecutionRecordBuildError {
    Semantic(CodecError),
    Proof(ProofCodecError),
    Record(PsiOptimizationExecutionRecordError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiOptimizationExecutionRecordDecodeError {
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    UnsupportedVocabularyMarker(u16),
    UnexpectedEnd,
    TrailingBytes(usize),
    Selection(PsiOptimizationSelectionDecodeError),
    InvalidRecord(PsiOptimizationExecutionRecordError),
    NonCanonicalEncoding,
}

impl std::fmt::Display for PsiOptimizationExecutionRecordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Psi optimization execution record: {self:?}"
        )
    }
}

impl std::fmt::Display for PsiOptimizationExecutionRecordBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot build Psi optimization execution record: {self:?}"
        )
    }
}

impl std::fmt::Display for PsiOptimizationExecutionRecordDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Psi optimization execution bytes: {self:?}"
        )
    }
}

impl std::error::Error for PsiOptimizationExecutionRecordError {}
impl std::error::Error for PsiOptimizationExecutionRecordBuildError {}
impl std::error::Error for PsiOptimizationExecutionRecordDecodeError {}

#[cfg(test)]
mod tests {
    use optimization::{PsiOptimization, PsiOptimizationSelections};
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;

    fn semantic(byte: u8) -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([byte; 32]),
        }
    }

    fn proof(byte: u8) -> ProofBundleFingerprint {
        ProofBundleFingerprint::from_bytes([byte; 32])
    }

    #[test]
    fn identity_execution_round_trips_canonically() {
        let record = PsiOptimizationExecutionRecord::new(
            PsiOptimizationSelections::default(),
            semantic(1),
            proof(2),
            semantic(1),
            proof(2),
        )
        .unwrap();
        let bytes = encode_psi_optimization_execution_record(&record);
        assert_eq!(decode_psi_optimization_execution_record(&bytes), Ok(record));
    }

    #[test]
    fn identity_execution_cannot_claim_changed_output() {
        assert_eq!(
            PsiOptimizationExecutionRecord::new(
                PsiOptimizationSelections::default(),
                semantic(1),
                proof(2),
                semantic(3),
                proof(2),
            ),
            Err(PsiOptimizationExecutionRecordError::IdentityStageChangedProduct)
        );
    }

    #[test]
    fn selected_execution_identity_binds_selection_and_both_products() {
        let selections = PsiOptimizationSelections::new([
            PsiOptimization::ControlFlowCleanup,
            PsiOptimization::CopyPropagation,
        ])
        .unwrap();
        let first = PsiOptimizationExecutionRecord::new(
            selections.clone(),
            semantic(1),
            proof(2),
            semantic(3),
            proof(4),
        )
        .unwrap();
        let changed_output = PsiOptimizationExecutionRecord::new(
            selections,
            semantic(1),
            proof(2),
            semantic(5),
            proof(4),
        )
        .unwrap();
        assert_ne!(first.identity(), changed_output.identity());
    }
}
