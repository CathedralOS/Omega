//! Source-independent proof-value vocabulary.
//!
//! Proof values have no runtime `ValueId`, storage, ABI, or execution result.
//! This first closed carrier retains only `FloatMeaning` projections from one
//! landed IEEE input through the shared format-specific projection catalog.

use psi_core::IeeeFloatFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofValueId(pub u32);

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

/// One source-independent projection-input coordinate. It deliberately omits
/// runtime bits and cannot be evaluated by Terminal Psi. A later producer must
/// bind the coordinate to one landed runtime value before emitting this fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatProjectionInput {
    pub id: FloatProjectionInputId,
    pub format: IeeeFloatFormat,
}

/// One total proof-only projection. This row is not an executable Terminal
/// operation and cannot appear in a runtime block's `Operation` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatMeaningProjection {
    pub result: ProofValueDeclaration,
    pub source: FloatProjectionInput,
    pub operation: FloatMeaningProjectionOperation,
}
