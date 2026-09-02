//! Source-independent proof-value vocabulary.
//!
//! Proof values have no runtime `ValueId`, storage, ABI, or execution result.
//! This first closed carrier retains only `FloatMeaning` projections from one
//! landed IEEE input through the shared format-specific projection catalog.

use psi_core::{BlockId, IeeeFloatFormat, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofPropositionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatProjectionInputId(pub u32);

/// Closed artifact descriptor corresponding to the rooted-checker D40 tuple.
/// `declaration` covers the exact toolchain operation ownership, parameter
/// shape, and FloatMeaning result identity; the commitment also binds the two
/// sealed source owners used by frontend recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatProjectionContractIdentity {
    pub format: u16,
    pub operation: u8,
    pub declaration: u8,
    pub catalog_version: u16,
    pub commitment: [u8; 32],
}

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

/// Exact artifact-relative identity of one direct IEEE machine parameter.
///
/// The owner and parameter are Terminal semantic identities, not frontend
/// symbol handles or producer-local coordinates. The independent verifier
/// rejoins this row to the owner's direct parameter table and checks the exact
/// IEEE format before accepting the projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectMachineFloatParameter {
    pub owner: MachineId,
    pub parameter: ValueId,
    pub format: IeeeFloatFormat,
}

/// Exact artifact-relative identity of one top-level machine's scalar IEEE
/// result. `result` is a reserved contract pseudo-name and has no frontend
/// symbol identity, so Terminal binds it directly to the owner's declared
/// scalar result value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectMachineFloatResult {
    pub owner: MachineId,
    pub result: ValueId,
    pub format: IeeeFloatFormat,
}

/// Exact artifact-relative identity of one scalar IEEE block parameter.
///
/// The block must occur in `owner` and declare `parameter` directly in its
/// parameter table. Machine parameters, machine results, and operation results
/// remain distinct source classes even when their scalar declarations have the
/// same IEEE format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectBlockFloatParameter {
    pub owner: MachineId,
    pub block: BlockId,
    pub parameter: ValueId,
    pub format: IeeeFloatFormat,
}

/// Exact artifact-relative identity of one scalar IEEE non-call operation
/// result.
///
/// The producer operation must occur in `owner` and declare `result` as its
/// direct scalar result. The independent verifier rejoins all three semantic
/// coordinates and the IEEE format; a machine result, parameter, block
/// parameter, call result, Unit result, or structural result is not
/// interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectOperationFloatResult {
    pub owner: MachineId,
    pub producer: OperationId,
    pub result: ValueId,
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
    DirectMachineParameter(DirectMachineFloatParameter),
    DirectMachineResult(DirectMachineFloatResult),
    DirectBlockParameter(DirectBlockFloatParameter),
    DirectOperationResult(DirectOperationFloatResult),
    ExactBinary32Literal(u32),
    ExactBinary64Literal(u64),
}

impl FloatMeaningSource {
    pub const fn format(self) -> IeeeFloatFormat {
        match self {
            Self::TransitionalInput(input) => input.format,
            Self::DirectMachineParameter(parameter) => parameter.format,
            Self::DirectMachineResult(result) => result.format,
            Self::DirectBlockParameter(parameter) => parameter.format,
            Self::DirectOperationResult(result) => result.format,
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
    pub contract: FloatProjectionContractIdentity,
}

/// One source-independent proof-only equality over retained projection
/// results. This row is not a runtime Boolean and cannot occur in a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatMeaningEqualityProposition {
    pub id: ProofPropositionId,
    pub left: ProofValueId,
    pub right: ProofValueId,
}
