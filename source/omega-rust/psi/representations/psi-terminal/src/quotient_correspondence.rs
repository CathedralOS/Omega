//! Proof-only retention for canonical quotient correspondence.
//!
//! [`crate::TerminalModule`] owns these rows and its canonical codec retains
//! them. A row still grants no machine, operation, or executable authority.

use psi_language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientContractFactCoordinate, QuotientContractOwner,
    QuotientCorrespondenceOperationKind, QuotientPositionalRelation, QuotientTheoremParameterRole,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotientCorrespondenceIdentity(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedQuotientCorrespondence {
    pub certificate: CanonicalQuotientCorrespondence,
    pub identity: QuotientCorrespondenceIdentity,
}

/// Retain one source-free aggregate without adding it to executable Terminal
/// Psi. Independent semantic replay remains the verifier's responsibility.
pub fn retain_non_executable_quotient_correspondence(
    certificate: CanonicalQuotientCorrespondence,
) -> RetainedQuotientCorrespondence {
    let identity = quotient_correspondence_identity(&certificate);
    RetainedQuotientCorrespondence {
        certificate,
        identity,
    }
}

fn quotient_correspondence_identity(
    certificate: &CanonicalQuotientCorrespondence,
) -> QuotientCorrespondenceIdentity {
    let mut writer = IdentityWriter::new();
    writer.string("omega.quotient-correspondence.total-direct-define.v1");
    writer.byte(match certificate.operation_kind {
        QuotientCorrespondenceOperationKind::Lift => 1,
        QuotientCorrespondenceOperationKind::Define => 2,
    });
    writer.callable(&certificate.public_operation);
    writer.application(&certificate.representative);
    writer.application(&certificate.selected_theorem);
    writer.len(certificate.input_relations.len());
    for relation in &certificate.input_relations {
        match relation {
            QuotientPositionalRelation::Quotient(relation) => {
                writer.byte(1);
                writer.relation(relation);
            }
            QuotientPositionalRelation::ExactEquality {
                public_type,
                representative_type,
            } => {
                writer.byte(2);
                writer.string(public_type);
                writer.string(representative_type);
            }
        }
    }
    writer.relation(&certificate.result_relation);
    writer.len(certificate.runtime_positions.len());
    for position in &certificate.runtime_positions {
        writer.u32(position.public_position);
        writer.u32(position.representative_position);
    }
    writer.len(certificate.theorem.parameters.len());
    for parameter in &certificate.theorem.parameters {
        writer.u32(parameter.theorem_position);
        match parameter.role {
            QuotientTheoremParameterRole::QuotientLeft { input_position } => {
                writer.byte(1);
                writer.u32(input_position);
            }
            QuotientTheoremParameterRole::QuotientRight { input_position } => {
                writer.byte(2);
                writer.u32(input_position);
            }
            QuotientTheoremParameterRole::Shared { input_position } => {
                writer.byte(3);
                writer.u32(input_position);
            }
        }
    }
    writer.len(certificate.theorem.relation_premises.len());
    for premise in &certificate.theorem.relation_premises {
        writer.u32(premise.expected_position);
        writer.coordinate(premise.actual);
        writer.string(&premise.relation);
        writer.u32(premise.left_parameter);
        writer.u32(premise.right_parameter);
    }
    writer.len(certificate.theorem.legality_premises.len());
    for premise in &certificate.theorem.legality_premises {
        writer.coordinate(*premise);
    }
    writer.coordinate(certificate.theorem.conclusion.actual);
    writer.string(&certificate.theorem.conclusion.relation);
    writer.len(certificate.theorem.conclusion.left.arguments.len());
    for argument in &certificate.theorem.conclusion.left.arguments {
        writer.u32(*argument);
    }
    writer.len(certificate.theorem.conclusion.right.arguments.len());
    for argument in &certificate.theorem.conclusion.right.arguments {
        writer.u32(*argument);
    }
    writer.byte(1); // representative pure closure
    writer.byte(1); // representative unconditional termination
    writer.byte(1); // theorem pure closure
    writer.byte(1); // theorem unconditional termination
    writer.byte(1); // theorem crash free
    writer.u32(certificate.result_flow.state_position);
    writer.u32(certificate.result_flow.statement_position);
    QuotientCorrespondenceIdentity(writer.finish())
}

struct IdentityWriter {
    bytes: Vec<u8>,
}

impl IdentityWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn byte(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn len(&mut self, value: usize) {
        self.bytes(&(value as u64).to_le_bytes());
    }

    fn string(&mut self, value: &str) {
        self.len(value.len());
        self.bytes(value.as_bytes());
    }

    fn callable(
        &mut self,
        callable: &psi_language_semantics::quotient_correspondence::QuotientCallableIdentity,
    ) {
        self.string(&callable.declaration);
        self.string(&callable.overload);
    }

    fn application(
        &mut self,
        application: &psi_language_semantics::quotient_correspondence::QuotientMachineApplication,
    ) {
        self.callable(&application.callable);
        self.len(application.static_application.bindings.len());
        for binding in &application.static_application.bindings {
            self.string(binding);
        }
    }

    fn relation(
        &mut self,
        relation: &psi_language_semantics::quotient_correspondence::QuotientRelationIdentity,
    ) {
        self.string(&relation.quotient_declaration);
        self.string(&relation.quotient_type);
        self.string(&relation.carrier_type);
        self.string(&relation.relation);
    }

    fn coordinate(&mut self, coordinate: QuotientContractFactCoordinate) {
        self.byte(match coordinate.owner {
            QuotientContractOwner::Machine => 1,
            QuotientContractOwner::State => 2,
        });
        self.u32(coordinate.contract_position);
        self.u32(coordinate.fact_position);
    }
}
