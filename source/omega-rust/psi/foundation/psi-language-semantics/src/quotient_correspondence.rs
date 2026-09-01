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
    /// Two-static-argument `Quotient::lift<F, Congruence>`.
    Lift,
    /// Three-static-argument
    /// `Quotient::lift<F, Congruence, ForwardPreconditionTransport>`.
    LiftWithForwardPreconditionTransport,
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
pub enum QuotientTheoremApplicationSide {
    Left,
    Right,
}

/// One exact source-to-theorem fact coordinate in a forward-precondition
/// transport proof.  Retaining both coordinates prevents source erasure from
/// turning a theorem row into unscoped proof authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientForwardPreconditionTransportFact {
    pub application: QuotientTheoremApplicationSide,
    pub source: QuotientContractFactCoordinate,
    pub actual: QuotientContractFactCoordinate,
}

pub type QuotientTheoremLegalityFact = QuotientForwardPreconditionTransportFact;

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
pub struct QuotientCongruenceCorrespondence {
    pub parameters: Vec<QuotientTheoremParameter>,
    pub relation_premises: Vec<QuotientTheoremRelationPremise>,
    /// Faithful `define` requires this roster to be empty. A transport-backed
    /// lift retains every representative-P source, application side, and
    /// selected congruence-theorem coordinate so replay can join it exactly to
    /// the transport theorem's P conclusions.
    pub legality_premises: Vec<QuotientTheoremLegalityFact>,
    pub conclusion: QuotientTheoremConclusion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuotientTheoremRole {
    Congruence,
    ForwardPreconditionTransport,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientForwardPreconditionTransportCorrespondence {
    /// Complete ordered public-Q source/theorem premises cited by the selected
    /// transport, source-fact-major with adjacent Left/Right applications.
    pub public_premises: Vec<QuotientForwardPreconditionTransportFact>,
    /// Complete ordered representative-P source/theorem conclusions proved by
    /// the transport, source-fact-major with adjacent Left/Right applications.
    pub representative_conclusions: Vec<QuotientForwardPreconditionTransportFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuotientTheoremCorrespondence {
    Congruence(QuotientCongruenceCorrespondence),
    ForwardPreconditionTransport(QuotientForwardPreconditionTransportCorrespondence),
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientTheoremEvidence {
    pub role: QuotientTheoremRole,
    pub selected_application: QuotientMachineApplication,
    pub correspondence: QuotientTheoremCorrespondence,
    pub eligibility: QuotientTheoremEligibility,
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
    pub input_relations: Vec<QuotientPositionalRelation>,
    pub result_relation: QuotientRelationIdentity,
    pub runtime_positions: Vec<QuotientDefineRuntimePosition>,
    pub theorem_evidence: Vec<QuotientTheoremEvidence>,
    pub representative_eligibility: QuotientRepresentativeEligibility,
    pub result_flow: QuotientDirectResultFlow,
}
