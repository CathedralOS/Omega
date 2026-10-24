//! Canonical record of the target-neutral optimization phase that produced a
//! Terminal-Psi artifact.

use crate::cursor::Cursor;
use optimization::{
    PsiOptimizationSelectionDecodeError, PsiOptimizationSelectionIdentity,
    PsiOptimizationSelections,
};
use terminal_psi::TerminalModule;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{CodecError, terminal_psi_identity};

const MAGIC: &[u8; 8] = b"PSIOEXE\0";
const FORMAT_MARKER: u16 = 2;
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
    output_semantic: TerminalPsiIdentity,
    identity: PsiOptimizationExecutionIdentity,
}

impl PsiOptimizationExecutionRecord {
    pub fn new(
        selections: PsiOptimizationSelections,
        input_semantic: TerminalPsiIdentity,
        output_semantic: TerminalPsiIdentity,
    ) -> Result<Self, PsiOptimizationExecutionRecordError> {
        if selections.is_empty() && input_semantic != output_semantic {
            return Err(PsiOptimizationExecutionRecordError::IdentityStageChangedProduct);
        }
        let identity = execution_identity(selections.identity(), input_semantic, output_semantic);
        Ok(Self {
            selections,
            input_semantic,
            output_semantic,
            identity,
        })
    }

    pub const fn selections(&self) -> &PsiOptimizationSelections {
        &self.selections
    }

    pub fn selection_identity(&self) -> PsiOptimizationSelectionIdentity {
        self.selections.identity()
    }

    pub const fn input_semantic(&self) -> TerminalPsiIdentity {
        self.input_semantic
    }

    pub const fn output_semantic(&self) -> TerminalPsiIdentity {
        self.output_semantic
    }

    pub const fn identity(&self) -> PsiOptimizationExecutionIdentity {
        self.identity
    }

    pub fn validate_output(
        &self,
        semantic: TerminalPsiIdentity,
    ) -> Result<(), PsiOptimizationExecutionRecordError> {
        if self.output_semantic != semantic {
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
) -> Result<PsiOptimizationExecutionRecord, PsiOptimizationExecutionRecordBuildError> {
    let semantic = terminal_psi_identity(module)
        .map_err(PsiOptimizationExecutionRecordBuildError::Semantic)?;
    PsiOptimizationExecutionRecord::new(PsiOptimizationSelections::default(), semantic, semantic)
        .map_err(PsiOptimizationExecutionRecordBuildError::Record)
}

pub fn encode_psi_optimization_execution_record(
    record: &PsiOptimizationExecutionRecord,
) -> Vec<u8> {
    let selections = record.selections.encode();
    let mut bytes = Vec::with_capacity(10 + 8 + selections.len() + 132);
    bytes.extend_from_slice(MAGIC);
    crate::codec_bytes::push_encoded(&mut bytes, FORMAT_MARKER.to_le_bytes());
    bytes.extend_from_slice(
        &u64::try_from(selections.len())
            .expect("the in-memory optimization selection fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&selections);
    append_terminal_identity(&mut bytes, record.input_semantic);
    append_terminal_identity(&mut bytes, record.output_semantic);
    bytes
}

pub fn decode_psi_optimization_execution_record(
    bytes: &[u8],
) -> Result<PsiOptimizationExecutionRecord, PsiOptimizationExecutionRecordDecodeError> {
    let mut cursor = Cursor::new(bytes);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(PsiOptimizationExecutionRecordDecodeError::InvalidMagic);
    }
    let marker = cursor.u16()?;
    if marker != FORMAT_MARKER {
        return Err(PsiOptimizationExecutionRecordDecodeError::UnsupportedFormatMarker(marker));
    }
    let selections_len = usize::try_from(cursor.u64()?)
        .map_err(|_| PsiOptimizationExecutionRecordDecodeError::UnexpectedEnd)?;
    let selections = PsiOptimizationSelections::decode(cursor.take(selections_len)?)
        .map_err(PsiOptimizationExecutionRecordDecodeError::Selection)?;
    let input_semantic = decode_terminal_identity(&mut cursor)?;
    let output_semantic = decode_terminal_identity(&mut cursor)?;
    if cursor.remaining() != 0 {
        return Err(PsiOptimizationExecutionRecordDecodeError::TrailingBytes(
            cursor.remaining(),
        ));
    }
    let record = PsiOptimizationExecutionRecord::new(selections, input_semantic, output_semantic)
        .map_err(PsiOptimizationExecutionRecordDecodeError::InvalidRecord)?;
    if encode_psi_optimization_execution_record(&record) != bytes {
        return Err(PsiOptimizationExecutionRecordDecodeError::NonCanonicalEncoding);
    }
    Ok(record)
}

fn execution_identity(
    selection: PsiOptimizationSelectionIdentity,
    input_semantic: TerminalPsiIdentity,
    output_semantic: TerminalPsiIdentity,
) -> PsiOptimizationExecutionIdentity {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(IDENTITY_DOMAIN);
    digest.raw(&selection.bytes());
    append_identity_to_digest(&mut digest, input_semantic);
    append_identity_to_digest(&mut digest, output_semantic);
    PsiOptimizationExecutionIdentity(digest.finish())
}

fn append_identity_to_digest(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    identity: TerminalPsiIdentity,
) {
    digest.raw(&identity.vocabulary_marker.get().to_le_bytes());
    digest.raw(identity.program_fingerprint.as_bytes());
}

/// The canonical wire fragment for a `TerminalPsiIdentity`: vocabulary marker
/// then fingerprint bytes.
pub fn append_terminal_identity(bytes: &mut Vec<u8>, identity: TerminalPsiIdentity) {
    crate::codec_bytes::push_encoded(bytes, identity.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(identity.program_fingerprint.as_bytes());
}

fn decode_terminal_identity(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalPsiIdentity, PsiOptimizationExecutionRecordDecodeError> {
    let marker = cursor.u16()?;
    let vocabulary_marker = VocabularyMarker::new(marker)
        .ok_or(PsiOptimizationExecutionRecordDecodeError::UnsupportedVocabularyMarker(marker))?;
    Ok(TerminalPsiIdentity {
        vocabulary_marker,
        program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiOptimizationExecutionRecordError {
    IdentityStageChangedProduct,
    OutputMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiOptimizationExecutionRecordBuildError {
    Semantic(CodecError),
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

optimization::truncated_from_adapter!(PsiOptimizationExecutionRecordDecodeError => Self::UnexpectedEnd);
