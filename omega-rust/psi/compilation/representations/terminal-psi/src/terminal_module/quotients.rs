//! Source-free quotient identity and the exact executable edges it licenses.
//!
//! Shape equality supplies no authority. A copy keeps its nominal endpoints;
//! these rows bind formation or a checked operation to the actual copy/call
//! occurrence. The ordinary storage and call operations perform computation.
use language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientRelationIdentity,
};
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
    /// Source-checked binding from hermetic semantic formation to the actual
    /// compilation-local storage declarations. Neither identity is rewritten.
    pub quotient_storage_identity: crate::TerminalIdentityIndex,
    pub carrier_storage_identity: crate::TerminalIdentityIndex,
    pub formation: QuotientRelationIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalQuotientOperation {
    pub public_machine: MachineId,
    pub representative_machine: MachineId,
    pub call: OperationId,
    pub correspondence: CanonicalQuotientCorrespondence,
    /// Ordered selected theorem declarations. The normalized schemas and
    /// exact fact-coordinate mapping are in correspondence.theorem_evidence.
    pub theorems: Vec<TerminalQuotientTheorem>,
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

/// Source admission is explicit; this is not an independently checked PCC proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalQuotientTheoremAdmission {
    SourceCheckedBodyfulResultless,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalQuotientTheorem {
    pub declaration: crate::CheckedMachineDeclaration,
    pub admission: TerminalQuotientTheoremAdmission,
}
