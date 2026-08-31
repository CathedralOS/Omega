//! Source-independent proof-value vocabulary.
//!
//! Proof values have no runtime `ValueId`, storage, ABI, or execution result.
//! This first closed carrier retains only `FloatMeaning` projections from one
//! landed IEEE input through the shared format-specific projection catalog.

use psi_core::IeeeFloatFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofPropositionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatProjectionInputId(pub u32);

/// Closed Terminal identity for the format-specific public projection. This
/// tag carries no source spelling; the verifier maps it independently to the
/// shared numeric catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatMeaningProjectionOperation {
    Meaning32,
    Meaning64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofOnlyValueType {
    FloatMeaning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofValueDeclaration {
    pub id: ProofValueId,
    pub value_type: ProofOnlyValueType,
}

/// One source-independent projection-input coordinate. Equal checked source
/// keys reuse one ID, assigned densely by first use; authored projection
/// occurrences and spans are retained separately and do not enter this row.
/// The ID still deliberately omits runtime bits and cannot be evaluated by
/// Terminal Psi. A later producer must bind it to an artifact-reconstructible
/// landed source before this becomes the complete D40 source carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatProjectionInput {
    pub id: FloatProjectionInputId,
    pub format: IeeeFloatFormat,
}

/// Verifier-reconstructible source of one float-meaning projection.
///
/// Exact literals own their raw landed bits and therefore need no producer ID.
/// The transitional coordinate is retained only for source forms whose
/// artifact-relative carrier is still open; it is not interchangeable with an
/// exact literal and cannot manufacture literal correspondence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatMeaningSource {
    TransitionalInput(FloatProjectionInput),
    ExactBinary32Literal(u32),
    ExactBinary64Literal(u64),
}

impl FloatMeaningSource {
    pub const fn format(self) -> IeeeFloatFormat {
        match self {
            Self::TransitionalInput(input) => input.format,
            Self::ExactBinary32Literal(_) => IeeeFloatFormat::Binary32,
            Self::ExactBinary64Literal(_) => IeeeFloatFormat::Binary64,
        }
    }
}

/// One total proof-only projection. This row is not an executable Terminal
/// operation and cannot appear in a runtime block's `Operation` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatMeaningProjection {
    pub result: ProofValueDeclaration,
    pub source: FloatMeaningSource,
    pub operation: FloatMeaningProjectionOperation,
}

/// One source-independent proof-only equality over retained projection
/// results. This row is not a runtime Boolean and cannot occur in a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatMeaningEqualityProposition {
    pub id: ProofPropositionId,
    pub left: ProofValueId,
    pub right: ProofValueId,
}
