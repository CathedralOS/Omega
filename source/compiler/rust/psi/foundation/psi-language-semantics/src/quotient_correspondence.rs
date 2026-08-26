//! Source-handle-free vocabulary for non-executable quotient correspondence.
//!
//! These rows carry proof-only semantic identity. They do not admit a source
//! operation, create a checked runtime operation, or authorize Terminal-Psi
//! machine emission.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientCallableIdentity {
    /// Hermetic declaration owner, including package or toolchain identity.
    pub declaration: String,
    /// Canonical overload identity, including the complete runtime signature.
    pub overload: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuotientCorrespondenceOperationKind {
    Lift,
    Define,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientStaticApplication {
    /// Canonical closed bindings in declaration order. The first proof-only
    /// bridge admits only the empty application, but emptiness is retained as
    /// semantic evidence rather than inferred by a consumer.
    pub bindings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientMachineApplication {
    pub callable: QuotientCallableIdentity,
    pub static_application: QuotientStaticApplication,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientRelationIdentity {
    pub quotient_declaration: String,
    pub quotient_type: String,
    pub carrier_type: String,
    pub relation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuotientPositionalRelation {
    Quotient(QuotientRelationIdentity),
    ExactEquality {
        public_type: String,
        representative_type: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientDefineRuntimePosition {
    pub public_position: u32,
    pub representative_position: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuotientContractOwner {
    Machine,
    State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientContractFactCoordinate {
    pub owner: QuotientContractOwner,
    pub contract_position: u32,
    pub fact_position: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuotientTheoremParameterRole {
    QuotientLeft { input_position: u32 },
    QuotientRight { input_position: u32 },
    Shared { input_position: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientTheoremParameter {
    pub theorem_position: u32,
    pub role: QuotientTheoremParameterRole,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientTheoremRelationPremise {
    pub expected_position: u32,
    pub actual: QuotientContractFactCoordinate,
    pub relation: String,
    pub left_parameter: u32,
    pub right_parameter: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientRepresentativeApplication {
    pub arguments: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientTheoremConclusion {
    pub actual: QuotientContractFactCoordinate,
    pub relation: String,
    pub left: QuotientRepresentativeApplication,
    pub right: QuotientRepresentativeApplication,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientTheoremCorrespondence {
    pub parameters: Vec<QuotientTheoremParameter>,
    pub relation_premises: Vec<QuotientTheoremRelationPremise>,
    /// The total direct bridge requires this roster to be empty. Retaining it
    /// prevents a replay consumer from silently treating partial legality as
    /// already discharged.
    pub legality_premises: Vec<QuotientContractFactCoordinate>,
    pub conclusion: QuotientTheoremConclusion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuotientPurityCertificate {
    PureClosure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuotientTerminationCertificate {
    Unconditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuotientCrashCertificate {
    CrashFree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientRepresentativeEligibility {
    pub purity: QuotientPurityCertificate,
    pub termination: QuotientTerminationCertificate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientTheoremEligibility {
    pub purity: QuotientPurityCertificate,
    pub termination: QuotientTerminationCertificate,
    pub crash: QuotientCrashCertificate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientDirectResultFlow {
    pub state_position: u32,
    pub statement_position: u32,
}

/// Complete source-free input to the first standalone Terminal replay seam.
///
/// Construction is non-authoritative: normal validation continues to reject
/// every executable quotient request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalQuotientCorrespondence {
    pub operation_kind: QuotientCorrespondenceOperationKind,
    pub public_operation: QuotientCallableIdentity,
    pub representative: QuotientMachineApplication,
    pub selected_theorem: QuotientMachineApplication,
    pub input_relations: Vec<QuotientPositionalRelation>,
    pub result_relation: QuotientRelationIdentity,
    pub runtime_positions: Vec<QuotientDefineRuntimePosition>,
    pub theorem: QuotientTheoremCorrespondence,
    pub representative_eligibility: QuotientRepresentativeEligibility,
    pub theorem_eligibility: QuotientTheoremEligibility,
    pub result_flow: QuotientDirectResultFlow,
}
