use semantic_vocabulary::{
    BlockId, IeeeFloatFormat, IeeeFloatStructuralField, MachineId, OperationId, ValueId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofPropositionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatProjectionInputId(pub u32);

pub use numerics::float_projection::FloatProjectionContractIdentity;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatProjectionInput {
    pub id: FloatProjectionInputId,
    pub format: IeeeFloatFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectMachineFloatParameter {
    pub owner: MachineId,
    pub parameter: ValueId,
    pub format: IeeeFloatFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectMachineFloatResult {
    pub owner: MachineId,
    pub result: ValueId,
    pub format: IeeeFloatFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectBlockFloatParameter {
    pub owner: MachineId,
    pub block: BlockId,
    pub parameter: ValueId,
    pub format: IeeeFloatFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectOperationFloatResult {
    pub owner: MachineId,
    pub producer: OperationId,
    pub result: ValueId,
    pub format: IeeeFloatFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectCallFloatResult {
    pub owner: MachineId,
    pub producer: OperationId,
    pub result: ValueId,
    pub format: IeeeFloatFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectStructuralFloatLeaf {
    pub owner: MachineId,
    pub field: IeeeFloatStructuralField,
    pub format: IeeeFloatFormat,
}

pub use numerics::float_semantics::FloatSemanticContractIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatSemanticApplicationOperand {
    Format(IeeeFloatFormat),
    Meaning(ProofValueId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatSemanticApplication {
    pub contract: FloatSemanticContractIdentity,
    pub format: IeeeFloatFormat,
    pub operands: Vec<FloatSemanticApplicationOperand>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatMeaningSource {
    TransitionalInput(FloatProjectionInput),
    DirectMachineParameter(DirectMachineFloatParameter),
    DirectMachineResult(DirectMachineFloatResult),
    DirectBlockParameter(DirectBlockFloatParameter),
    DirectOperationResult(DirectOperationFloatResult),
    DirectCallResult(DirectCallFloatResult),
    DirectStructuralLeaf(DirectStructuralFloatLeaf),
    ExactBinary32Literal(u32),
    ExactBinary64Literal(u64),
    SemanticApplication(FloatSemanticApplication),
}

impl FloatMeaningSource {
    pub const fn format(&self) -> IeeeFloatFormat {
        match self {
            Self::TransitionalInput(input) => input.format,
            Self::DirectMachineParameter(parameter) => parameter.format,
            Self::DirectMachineResult(result) => result.format,
            Self::DirectBlockParameter(parameter) => parameter.format,
            Self::DirectOperationResult(result) => result.format,
            Self::DirectCallResult(result) => result.format,
            Self::DirectStructuralLeaf(leaf) => leaf.format,
            Self::ExactBinary32Literal(_) => IeeeFloatFormat::Binary32,
            Self::ExactBinary64Literal(_) => IeeeFloatFormat::Binary64,
            Self::SemanticApplication(application) => application.format,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatMeaningProjection {
    pub result: ProofValueDeclaration,
    pub source: FloatMeaningSource,
    pub operation: FloatMeaningProjectionOperation,
    pub contract: FloatProjectionContractIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatMeaningEqualityProposition {
    pub id: ProofPropositionId,
    pub left: ProofValueId,
    pub right: ProofValueId,
}

pub fn float_meaning_equality_proposition_id(index: u32) -> semantic_vocabulary::PropositionId {
    let mut encoder = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.terminal_psi.float_meaning_equality_proposition.v1",
    );
    encoder.raw(&index.to_le_bytes());
    let digest = encoder.finish();
    let raw = u64::from_le_bytes(
        digest[..8]
            .try_into()
            .expect("sha256 digests cover at least eight bytes"),
    ) | 1;
    semantic_vocabulary::PropositionId::new(raw).expect("a digest bit-or 1 is nonzero")
}
