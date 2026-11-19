#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct QuotientRelationIdentity {
    pub quotient_declaration: String,
    pub quotient_type: String,
    pub carrier_type: String,
    pub relation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum QuotientPositionalRelation {
    Quotient(QuotientRelationIdentity),
    ExactEquality {
        public_type: String,
        representative_type: String,
    },
}

use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalQuotientCatalog {
    pub representations: Vec<TerminalQuotientRepresentation>,
    pub operations: Vec<TerminalQuotientOperation>,
    pub adaptations: Vec<TerminalQuotientAdaptation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalQuotientRepresentation {
    pub quotient: StructuralTypeId,
    pub carrier: StructuralTypeId,
    pub quotient_storage_identity: crate::TerminalIdentityIndex,
    pub carrier_storage_identity: crate::TerminalIdentityIndex,
    pub formation: QuotientRelationIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalQuotientOperation {
    pub public_machine: MachineId,
    pub representative_machine: MachineId,
    pub call: OperationId,
    pub input_relations: Vec<QuotientPositionalRelation>,
    pub result_relation: QuotientPositionalRelation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalQuotientAdaptationLicense {
    Mint,
    Input { operation: u32, position: u32 },
    Result { operation: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalQuotientAdaptation {
    pub machine: MachineId,
    pub copy: OperationId,
    pub source: PlaceId,
    pub result: PlaceId,
    pub source_type: StructuralTypeId,
    pub result_type: StructuralTypeId,
    pub representation: u32,
    pub license: TerminalQuotientAdaptationLicense,
}
