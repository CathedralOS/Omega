//! Independent replay of the standalone quotient-correspondence bridge.

use std::collections::BTreeSet;

use psi_language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientContractFactCoordinate, QuotientContractOwner,
    QuotientCorrespondenceOperationKind, QuotientPositionalRelation,
    QuotientTheoremApplicationSide, QuotientTheoremCorrespondence, QuotientTheoremParameter,
    QuotientTheoremParameterRole, QuotientTheoremRole,
};
use psi_terminal::{QuotientCorrespondenceIdentity, RetainedQuotientCorrespondence};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotientCorrespondenceReplayError {
    UnsupportedOperationKind,
    NonCanonicalTheoremRoleCollection,
    TheoremRolePayloadMismatch,
    NonHermeticIdentity,
    NonEmptyStaticApplication,
    CallableIdentityCollision,
    InvalidRelationIdentity,
    RuntimeArityMismatch,
    NonCanonicalRuntimePosition {
        expected: u32,
    },
    TheoremParameterMismatch,
    TheoremRelationPremiseMismatch,
    NonEmptyLegalityPremises,
    TransportFactRosterMismatch,
    DuplicateTheoremCoordinate,
    TheoremConclusionMismatch,
    InvalidResultFlow,
    IdentityMismatch {
        expected: QuotientCorrespondenceIdentity,
        actual: QuotientCorrespondenceIdentity,
    },
}

pub fn replay_non_executable_quotient_correspondence(
    retained: &RetainedQuotientCorrespondence,
) -> Result<(), QuotientCorrespondenceReplayError> {
    let certificate = &retained.certificate;
    validate_theorem_roles(certificate)?;
    if !matches!(
        certificate.operation_kind,
        QuotientCorrespondenceOperationKind::Define
            | QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport
    ) {
        return Err(QuotientCorrespondenceReplayError::UnsupportedOperationKind);
    }
    let theorem_evidence = &certificate.theorem_evidence[0];
    let QuotientTheoremCorrespondence::Congruence(theorem) = &theorem_evidence.correspondence
    else {
        return Err(QuotientCorrespondenceReplayError::TheoremRolePayloadMismatch);
    };
    validate_callable(&certificate.public_operation)?;
    validate_callable(&certificate.representative.callable)?;
    for evidence in &certificate.theorem_evidence {
        validate_callable(&evidence.selected_application.callable)?;
    }
    if !certificate
        .representative
        .static_application
        .bindings
        .is_empty()
        || certificate.theorem_evidence.iter().any(|evidence| {
            !evidence
                .selected_application
                .static_application
                .bindings
                .is_empty()
        })
    {
        return Err(QuotientCorrespondenceReplayError::NonEmptyStaticApplication);
    }
    if certificate.public_operation == certificate.representative.callable
        || certificate.theorem_evidence.iter().any(|evidence| {
            certificate.public_operation == evidence.selected_application.callable
                || certificate.representative.callable == evidence.selected_application.callable
        })
        || certificate
            .theorem_evidence
            .iter()
            .enumerate()
            .any(|(position, evidence)| {
                certificate.theorem_evidence[..position]
                    .iter()
                    .any(|earlier| {
                        earlier.selected_application.callable
                            == evidence.selected_application.callable
                    })
            })
    {
        return Err(QuotientCorrespondenceReplayError::CallableIdentityCollision);
    }

    for relation in &certificate.input_relations {
        match relation {
            QuotientPositionalRelation::Quotient(relation) => validate_relation(relation)?,
            QuotientPositionalRelation::ExactEquality {
                public_type,
                representative_type,
            } if valid_type(public_type)
                && valid_type(representative_type)
                && public_type == representative_type => {}
            QuotientPositionalRelation::ExactEquality { .. } => {
                return Err(QuotientCorrespondenceReplayError::InvalidRelationIdentity);
            }
        }
    }
    validate_relation(&certificate.result_relation)?;

    if certificate.runtime_positions.len() != certificate.input_relations.len() {
        return Err(QuotientCorrespondenceReplayError::RuntimeArityMismatch);
    }
    for (position, runtime) in certificate.runtime_positions.iter().enumerate() {
        let expected = u32::try_from(position)
            .map_err(|_| QuotientCorrespondenceReplayError::RuntimeArityMismatch)?;
        if runtime.public_position != expected || runtime.representative_position != expected {
            return Err(
                QuotientCorrespondenceReplayError::NonCanonicalRuntimePosition { expected },
            );
        }
    }

    let (parameters, left_arguments, right_arguments, relation_rows) =
        expected_theorem_shape(&certificate.input_relations)?;
    if theorem.parameters != parameters {
        return Err(QuotientCorrespondenceReplayError::TheoremParameterMismatch);
    }
    if theorem.relation_premises.len() != relation_rows.len() {
        return Err(QuotientCorrespondenceReplayError::TheoremRelationPremiseMismatch);
    }
    let mut coordinates = BTreeSet::new();
    for (actual, expected) in theorem.relation_premises.iter().zip(relation_rows) {
        if actual.expected_position != expected.0
            || actual.relation != expected.1
            || actual.left_parameter != expected.2
            || actual.right_parameter != expected.3
        {
            return Err(QuotientCorrespondenceReplayError::TheoremRelationPremiseMismatch);
        }
        if !coordinates.insert(actual.actual) {
            return Err(QuotientCorrespondenceReplayError::DuplicateTheoremCoordinate);
        }
    }
    match certificate.operation_kind {
        QuotientCorrespondenceOperationKind::Define => {
            if !theorem.legality_premises.is_empty() {
                return Err(QuotientCorrespondenceReplayError::NonEmptyLegalityPremises);
            }
        }
        QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport => {
            let QuotientTheoremCorrespondence::ForwardPreconditionTransport(transport) =
                &certificate.theorem_evidence[1].correspondence
            else {
                return Err(QuotientCorrespondenceReplayError::TheoremRolePayloadMismatch);
            };
            validate_transport_roster(&transport.public_premises)?;
            validate_transport_roster(&transport.representative_conclusions)?;
            if theorem.legality_premises.len() != transport.representative_conclusions.len() {
                return Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch);
            }
            validate_transport_roster(&theorem.legality_premises)?;
            for fact in &theorem.legality_premises {
                if !coordinates.insert(fact.actual) {
                    return Err(QuotientCorrespondenceReplayError::DuplicateTheoremCoordinate);
                }
            }
            if theorem
                .legality_premises
                .iter()
                .zip(&transport.representative_conclusions)
                .any(|(legality, transport)| {
                    legality.application != transport.application
                        || legality.source != transport.source
                })
            {
                return Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch);
            }
        }
        QuotientCorrespondenceOperationKind::Lift => unreachable!(),
    }
    if !coordinates.insert(theorem.conclusion.actual) {
        return Err(QuotientCorrespondenceReplayError::DuplicateTheoremCoordinate);
    }
    if theorem.conclusion.relation != certificate.result_relation.relation
        || theorem.conclusion.left.arguments != left_arguments
        || theorem.conclusion.right.arguments != right_arguments
    {
        return Err(QuotientCorrespondenceReplayError::TheoremConclusionMismatch);
    }
    if certificate.result_flow.state_position != 0 {
        return Err(QuotientCorrespondenceReplayError::InvalidResultFlow);
    }

    let expected = independent_identity(certificate);
    if retained.identity != expected {
        return Err(QuotientCorrespondenceReplayError::IdentityMismatch {
            expected,
            actual: retained.identity.clone(),
        });
    }
    Ok(())
}

fn validate_transport_roster(
    facts: &[psi_language_semantics::quotient_correspondence::QuotientForwardPreconditionTransportFact],
) -> Result<(), QuotientCorrespondenceReplayError> {
    if facts.len() % 2 != 0 {
        return Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch);
    }
    let mut previous_source = None;
    let mut actuals = BTreeSet::new();
    let mut previous_actual = None;
    for pair in facts.chunks_exact(2) {
        if pair[0].application != QuotientTheoremApplicationSide::Left
            || pair[1].application != QuotientTheoremApplicationSide::Right
            || pair[0].source != pair[1].source
            || previous_source.map_or(false, |previous| previous >= pair[0].source)
            || !actuals.insert(pair[0].actual)
            || !actuals.insert(pair[1].actual)
            || previous_actual.map_or(false, |previous| previous >= pair[0].actual)
            || pair[0].actual >= pair[1].actual
        {
            return Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch);
        }
        previous_source = Some(pair[0].source);
        previous_actual = Some(pair[1].actual);
    }
    Ok(())
}

fn validate_theorem_roles(
    certificate: &CanonicalQuotientCorrespondence,
) -> Result<(), QuotientCorrespondenceReplayError> {
    let expected: &[QuotientTheoremRole] = match certificate.operation_kind {
        QuotientCorrespondenceOperationKind::Lift | QuotientCorrespondenceOperationKind::Define => {
            &[QuotientTheoremRole::Congruence]
        }
        QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport => &[
            QuotientTheoremRole::Congruence,
            QuotientTheoremRole::ForwardPreconditionTransport,
        ],
    };
    if certificate
        .theorem_evidence
        .iter()
        .map(|evidence| evidence.role)
        .ne(expected.iter().copied())
    {
        return Err(QuotientCorrespondenceReplayError::NonCanonicalTheoremRoleCollection);
    }
    for evidence in &certificate.theorem_evidence {
        let matches = matches!(
            (evidence.role, &evidence.correspondence),
            (
                QuotientTheoremRole::Congruence,
                QuotientTheoremCorrespondence::Congruence(_)
            ) | (
                QuotientTheoremRole::ForwardPreconditionTransport,
                QuotientTheoremCorrespondence::ForwardPreconditionTransport(_)
            )
        );
        if !matches {
            return Err(QuotientCorrespondenceReplayError::TheoremRolePayloadMismatch);
        }
    }
    Ok(())
}

fn expected_theorem_shape(
    relations: &[QuotientPositionalRelation],
) -> Result<
    (
        Vec<QuotientTheoremParameter>,
        Vec<u32>,
        Vec<u32>,
        Vec<(u32, String, u32, u32)>,
    ),
    QuotientCorrespondenceReplayError,
> {
    let mut parameters = Vec::new();
    let mut left = Vec::with_capacity(relations.len());
    let mut right = Vec::with_capacity(relations.len());
    let mut premises = Vec::new();
    for (input, relation) in relations.iter().enumerate() {
        let input_position = u32::try_from(input)
            .map_err(|_| QuotientCorrespondenceReplayError::TheoremParameterMismatch)?;
        match relation {
            QuotientPositionalRelation::Quotient(relation) => {
                let left_parameter = u32::try_from(parameters.len())
                    .map_err(|_| QuotientCorrespondenceReplayError::TheoremParameterMismatch)?;
                parameters.push(QuotientTheoremParameter {
                    theorem_position: left_parameter,
                    role: QuotientTheoremParameterRole::QuotientLeft { input_position },
                });
                let right_parameter = u32::try_from(parameters.len())
                    .map_err(|_| QuotientCorrespondenceReplayError::TheoremParameterMismatch)?;
                parameters.push(QuotientTheoremParameter {
                    theorem_position: right_parameter,
                    role: QuotientTheoremParameterRole::QuotientRight { input_position },
                });
                left.push(left_parameter);
                right.push(right_parameter);
                premises.push((
                    u32::try_from(premises.len()).map_err(|_| {
                        QuotientCorrespondenceReplayError::TheoremRelationPremiseMismatch
                    })?,
                    relation.relation.clone(),
                    left_parameter,
                    right_parameter,
                ));
            }
            QuotientPositionalRelation::ExactEquality { .. } => {
                let shared = u32::try_from(parameters.len())
                    .map_err(|_| QuotientCorrespondenceReplayError::TheoremParameterMismatch)?;
                parameters.push(QuotientTheoremParameter {
                    theorem_position: shared,
                    role: QuotientTheoremParameterRole::Shared { input_position },
                });
                left.push(shared);
                right.push(shared);
            }
        }
    }
    Ok((parameters, left, right, premises))
}

fn validate_callable(
    callable: &psi_language_semantics::quotient_correspondence::QuotientCallableIdentity,
) -> Result<(), QuotientCorrespondenceReplayError> {
    if !hermetic(&callable.declaration) || callable.overload.is_empty() {
        return Err(QuotientCorrespondenceReplayError::NonHermeticIdentity);
    }
    Ok(())
}

fn validate_relation(
    relation: &psi_language_semantics::quotient_correspondence::QuotientRelationIdentity,
) -> Result<(), QuotientCorrespondenceReplayError> {
    if !hermetic(&relation.quotient_declaration)
        || !hermetic(&relation.relation)
        || !valid_type(&relation.quotient_type)
        || !valid_type(&relation.carrier_type)
        || relation.quotient_type == relation.carrier_type
    {
        return Err(QuotientCorrespondenceReplayError::InvalidRelationIdentity);
    }
    Ok(())
}

fn hermetic(identity: &str) -> bool {
    identity.starts_with("package:") || identity.starts_with("toolchain::")
}

fn valid_type(identity: &str) -> bool {
    !identity.is_empty() && !identity.contains("unresolved-owner")
}

fn independent_identity(
    certificate: &CanonicalQuotientCorrespondence,
) -> QuotientCorrespondenceIdentity {
    let mut writer = ReplayIdentityWriter::new();
    writer.string("omega.quotient-correspondence.transport-coordinates.v3");
    writer.byte(match certificate.operation_kind {
        QuotientCorrespondenceOperationKind::Lift => 1,
        QuotientCorrespondenceOperationKind::Define => 2,
        QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport => 3,
    });
    writer.callable(&certificate.public_operation);
    writer.application(&certificate.representative);
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
    writer.len(certificate.theorem_evidence.len());
    for evidence in &certificate.theorem_evidence {
        writer.byte(match evidence.role {
            QuotientTheoremRole::Congruence => 1,
            QuotientTheoremRole::ForwardPreconditionTransport => 2,
        });
        writer.application(&evidence.selected_application);
        match &evidence.correspondence {
            QuotientTheoremCorrespondence::Congruence(congruence) => {
                writer.byte(1);
                write_congruence_identity(&mut writer, congruence);
            }
            QuotientTheoremCorrespondence::ForwardPreconditionTransport(transport) => {
                writer.byte(2);
                writer.len(transport.public_premises.len());
                for fact in &transport.public_premises {
                    writer.transport_fact(*fact);
                }
                writer.len(transport.representative_conclusions.len());
                for fact in &transport.representative_conclusions {
                    writer.transport_fact(*fact);
                }
            }
        }
        writer.byte(1);
        writer.byte(1);
        writer.byte(1);
    }
    writer.byte(1);
    writer.byte(1);
    writer.u32(certificate.result_flow.state_position);
    writer.u32(certificate.result_flow.statement_position);
    QuotientCorrespondenceIdentity(writer.finish())
}

fn write_congruence_identity(
    writer: &mut ReplayIdentityWriter,
    congruence: &psi_language_semantics::quotient_correspondence::QuotientCongruenceCorrespondence,
) {
    writer.len(congruence.parameters.len());
    for parameter in &congruence.parameters {
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
    writer.len(congruence.relation_premises.len());
    for premise in &congruence.relation_premises {
        writer.u32(premise.expected_position);
        writer.coordinate(premise.actual);
        writer.string(&premise.relation);
        writer.u32(premise.left_parameter);
        writer.u32(premise.right_parameter);
    }
    writer.len(congruence.legality_premises.len());
    for premise in &congruence.legality_premises {
        writer.transport_fact(*premise);
    }
    writer.coordinate(congruence.conclusion.actual);
    writer.string(&congruence.conclusion.relation);
    writer.len(congruence.conclusion.left.arguments.len());
    for argument in &congruence.conclusion.left.arguments {
        writer.u32(*argument);
    }
    writer.len(congruence.conclusion.right.arguments.len());
    for argument in &congruence.conclusion.right.arguments {
        writer.u32(*argument);
    }
}

struct ReplayIdentityWriter {
    bytes: Vec<u8>,
}

impl ReplayIdentityWriter {
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
    fn transport_fact(
        &mut self,
        fact: psi_language_semantics::quotient_correspondence::QuotientForwardPreconditionTransportFact,
    ) {
        self.byte(match fact.application {
            QuotientTheoremApplicationSide::Left => 1,
            QuotientTheoremApplicationSide::Right => 2,
        });
        self.coordinate(fact.source);
        self.coordinate(fact.actual);
    }
}

#[cfg(test)]
mod tests {
    use psi_core::{BlockId, ContractId, EdgeId, MachineId};
    use psi_language_semantics::quotient_correspondence::{
        CanonicalQuotientCorrespondence, QuotientCallableIdentity,
        QuotientCongruenceCorrespondence, QuotientContractFactCoordinate, QuotientContractOwner,
        QuotientCorrespondenceOperationKind, QuotientCrashCertificate,
        QuotientDefineRuntimePosition, QuotientDirectResultFlow,
        QuotientForwardPreconditionTransportCorrespondence,
        QuotientForwardPreconditionTransportFact, QuotientMachineApplication,
        QuotientPositionalRelation, QuotientPurityCertificate, QuotientRelationIdentity,
        QuotientRepresentativeApplication, QuotientRepresentativeEligibility,
        QuotientStaticApplication, QuotientTerminationCertificate, QuotientTheoremApplicationSide,
        QuotientTheoremConclusion, QuotientTheoremCorrespondence, QuotientTheoremEligibility,
        QuotientTheoremEvidence, QuotientTheoremParameter, QuotientTheoremParameterRole,
        QuotientTheoremRelationPremise, QuotientTheoremRole,
    };
    use psi_terminal::{
        Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
        VocabularyMarker, retain_non_executable_quotient_correspondence,
    };

    use super::*;

    fn callable(name: &str) -> QuotientCallableIdentity {
        QuotientCallableIdentity {
            declaration: format!("package:{}::{name}", "01".repeat(32)),
            overload: format!("named-callable:{name}"),
        }
    }

    fn relation(name: &str) -> QuotientRelationIdentity {
        QuotientRelationIdentity {
            quotient_declaration: format!("package:{}::{name}", "02".repeat(32)),
            quotient_type: format!("package:{}::{name}", "02".repeat(32)),
            carrier_type: format!("package:{}::{name}Carrier", "03".repeat(32)),
            relation: format!("package:{}::{name}Relation", "04".repeat(32)),
        }
    }

    fn coordinate(fact_position: u32) -> QuotientContractFactCoordinate {
        QuotientContractFactCoordinate {
            owner: QuotientContractOwner::State,
            contract_position: 0,
            fact_position,
        }
    }

    fn certificate() -> CanonicalQuotientCorrespondence {
        let input = relation("Value");
        let result = relation("Result");
        CanonicalQuotientCorrespondence {
            operation_kind: QuotientCorrespondenceOperationKind::Define,
            public_operation: callable("Public::apply"),
            representative: QuotientMachineApplication {
                callable: callable("Carrier::apply"),
                static_application: QuotientStaticApplication { bindings: vec![] },
            },
            input_relations: vec![
                QuotientPositionalRelation::Quotient(input.clone()),
                QuotientPositionalRelation::ExactEquality {
                    public_type: "u32@exact".to_owned(),
                    representative_type: "u32@exact".to_owned(),
                },
            ],
            result_relation: result.clone(),
            runtime_positions: vec![
                QuotientDefineRuntimePosition {
                    public_position: 0,
                    representative_position: 0,
                },
                QuotientDefineRuntimePosition {
                    public_position: 1,
                    representative_position: 1,
                },
            ],
            theorem_evidence: vec![QuotientTheoremEvidence {
                role: QuotientTheoremRole::Congruence,
                selected_application: QuotientMachineApplication {
                    callable: callable("apply_respects"),
                    static_application: QuotientStaticApplication { bindings: vec![] },
                },
                correspondence: QuotientTheoremCorrespondence::Congruence(
                    QuotientCongruenceCorrespondence {
                        parameters: vec![
                            QuotientTheoremParameter {
                                theorem_position: 0,
                                role: QuotientTheoremParameterRole::QuotientLeft {
                                    input_position: 0,
                                },
                            },
                            QuotientTheoremParameter {
                                theorem_position: 1,
                                role: QuotientTheoremParameterRole::QuotientRight {
                                    input_position: 0,
                                },
                            },
                            QuotientTheoremParameter {
                                theorem_position: 2,
                                role: QuotientTheoremParameterRole::Shared { input_position: 1 },
                            },
                        ],
                        relation_premises: vec![QuotientTheoremRelationPremise {
                            expected_position: 0,
                            actual: coordinate(0),
                            relation: input.relation,
                            left_parameter: 0,
                            right_parameter: 1,
                        }],
                        legality_premises: vec![],
                        conclusion: QuotientTheoremConclusion {
                            actual: coordinate(1),
                            relation: result.relation,
                            left: QuotientRepresentativeApplication {
                                arguments: vec![0, 2],
                            },
                            right: QuotientRepresentativeApplication {
                                arguments: vec![1, 2],
                            },
                        },
                    },
                ),
                eligibility: QuotientTheoremEligibility {
                    purity: QuotientPurityCertificate::PureClosure,
                    termination: QuotientTerminationCertificate::Unconditional,
                    crash: QuotientCrashCertificate::CrashFree,
                },
            }],
            representative_eligibility: QuotientRepresentativeEligibility {
                purity: QuotientPurityCertificate::PureClosure,
                termination: QuotientTerminationCertificate::Unconditional,
            },
            result_flow: QuotientDirectResultFlow {
                state_position: 0,
                statement_position: 7,
            },
        }
    }

    fn congruence(
        certificate: &mut CanonicalQuotientCorrespondence,
    ) -> &mut QuotientCongruenceCorrespondence {
        let QuotientTheoremCorrespondence::Congruence(congruence) =
            &mut certificate.theorem_evidence[0].correspondence
        else {
            panic!("congruence fixture")
        };
        congruence
    }

    fn transport_evidence() -> QuotientTheoremEvidence {
        QuotientTheoremEvidence {
            role: QuotientTheoremRole::ForwardPreconditionTransport,
            selected_application: QuotientMachineApplication {
                callable: callable("apply_transports_preconditions"),
                static_application: QuotientStaticApplication { bindings: vec![] },
            },
            correspondence: QuotientTheoremCorrespondence::ForwardPreconditionTransport(
                QuotientForwardPreconditionTransportCorrespondence {
                    public_premises: [
                        transport_pair(coordinate(20), 2),
                        transport_pair(coordinate(22), 4),
                    ]
                    .concat(),
                    representative_conclusions: transport_pair(coordinate(21), 6),
                },
            ),
            eligibility: QuotientTheoremEligibility {
                purity: QuotientPurityCertificate::PureClosure,
                termination: QuotientTerminationCertificate::Unconditional,
                crash: QuotientCrashCertificate::CrashFree,
            },
        }
    }

    fn transport_pair(
        source: QuotientContractFactCoordinate,
        actual_start: u32,
    ) -> Vec<QuotientForwardPreconditionTransportFact> {
        vec![
            QuotientForwardPreconditionTransportFact {
                application: QuotientTheoremApplicationSide::Left,
                source,
                actual: coordinate(actual_start),
            },
            QuotientForwardPreconditionTransportFact {
                application: QuotientTheoremApplicationSide::Right,
                source,
                actual: coordinate(actual_start + 1),
            },
        ]
    }

    fn transport_certificate() -> CanonicalQuotientCorrespondence {
        let mut certificate = certificate();
        certificate.operation_kind =
            QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport;
        certificate.theorem_evidence.push(transport_evidence());
        congruence(&mut certificate).legality_premises = transport_pair(coordinate(21), 5);
        certificate
    }

    fn transport(
        certificate: &mut CanonicalQuotientCorrespondence,
    ) -> &mut QuotientForwardPreconditionTransportCorrespondence {
        let QuotientTheoremCorrespondence::ForwardPreconditionTransport(transport) =
            &mut certificate.theorem_evidence[1].correspondence
        else {
            panic!("transport fixture")
        };
        transport
    }

    fn replayed(
        certificate: CanonicalQuotientCorrespondence,
    ) -> Result<(), QuotientCorrespondenceReplayError> {
        replay_non_executable_quotient_correspondence(
            &retain_non_executable_quotient_correspondence(certificate),
        )
    }

    fn module_with(
        quotient_correspondences: Vec<RetainedQuotientCorrespondence>,
    ) -> TerminalModule {
        let machine = MachineId::new(1).unwrap();
        TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences,
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                structural_parameters: Vec::new(),
                ranked_scc: None,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(1).unwrap(),
                blocks: vec![Block {
                    id: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(1).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(1).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        }
    }

    #[test]
    fn replays_total_direct_define_without_executable_authority() {
        assert_eq!(replayed(certificate()), Ok(()));
    }

    #[test]
    fn theorem_role_collection_is_exact_ordered_and_identity_bearing() {
        let mut missing = certificate();
        missing.theorem_evidence.clear();
        assert_eq!(
            replayed(missing),
            Err(QuotientCorrespondenceReplayError::NonCanonicalTheoremRoleCollection)
        );

        let mut duplicate = certificate();
        duplicate
            .theorem_evidence
            .push(duplicate.theorem_evidence[0].clone());
        assert_eq!(
            replayed(duplicate),
            Err(QuotientCorrespondenceReplayError::NonCanonicalTheoremRoleCollection)
        );

        let mut missing_transport = certificate();
        missing_transport.operation_kind =
            QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport;
        assert_eq!(
            replayed(missing_transport),
            Err(QuotientCorrespondenceReplayError::NonCanonicalTheoremRoleCollection)
        );

        let mut reversed = certificate();
        reversed.operation_kind =
            QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport;
        reversed.theorem_evidence.push(transport_evidence());
        reversed.theorem_evidence.swap(0, 1);
        assert_eq!(
            replayed(reversed),
            Err(QuotientCorrespondenceReplayError::NonCanonicalTheoremRoleCollection)
        );

        assert_eq!(replayed(transport_certificate()), Ok(()));

        let mut mismatched_payload = certificate();
        mismatched_payload.theorem_evidence[0].correspondence = transport_evidence().correspondence;
        assert_eq!(
            replayed(mismatched_payload),
            Err(QuotientCorrespondenceReplayError::TheoremRolePayloadMismatch)
        );

        let canonical = retain_non_executable_quotient_correspondence(certificate());
        let mut role_mutation = certificate();
        role_mutation.theorem_evidence[0].role = QuotientTheoremRole::ForwardPreconditionTransport;
        let role_mutation = retain_non_executable_quotient_correspondence(role_mutation);
        assert_ne!(canonical.identity, role_mutation.identity);
    }

    #[test]
    fn transport_replay_rejects_side_source_theorem_and_roster_drift() {
        let mut changed = transport_certificate();
        transport(&mut changed).public_premises[0].application =
            QuotientTheoremApplicationSide::Right;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch)
        );

        let mut changed = transport_certificate();
        transport(&mut changed).public_premises[1].source = coordinate(99);
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch)
        );

        let mut changed = transport_certificate();
        let retained_transport = transport(&mut changed);
        retained_transport.representative_conclusions[1].actual =
            retained_transport.representative_conclusions[0].actual;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch)
        );

        let mut changed = transport_certificate();
        transport(&mut changed).representative_conclusions.pop();
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch)
        );

        let mut changed = transport_certificate();
        transport(&mut changed).public_premises.rotate_left(2);
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch)
        );

        let mut changed = transport_certificate();
        let facts = &mut transport(&mut changed).public_premises;
        let actual = facts[0].actual;
        facts[0].actual = facts[1].actual;
        facts[1].actual = actual;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch)
        );

        let mut changed = transport_certificate();
        congruence(&mut changed).legality_premises[0].source = coordinate(98);
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch)
        );
    }

    #[test]
    fn rejects_every_structural_drift_independently() {
        let mut changed = certificate();
        changed.operation_kind = QuotientCorrespondenceOperationKind::Lift;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::UnsupportedOperationKind)
        );

        let mut changed = certificate();
        changed.public_operation.declaration = "local::Public".to_owned();
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::NonHermeticIdentity)
        );

        let mut changed = certificate();
        changed
            .representative
            .static_application
            .bindings
            .push("T=u8".to_owned());
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::NonEmptyStaticApplication)
        );

        let mut changed = certificate();
        changed.theorem_evidence[0].selected_application.callable =
            changed.representative.callable.clone();
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::CallableIdentityCollision)
        );

        let mut changed = certificate();
        let QuotientPositionalRelation::ExactEquality {
            representative_type,
            ..
        } = &mut changed.input_relations[1]
        else {
            unreachable!()
        };
        *representative_type = "u64@exact".to_owned();
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::InvalidRelationIdentity)
        );

        let mut changed = certificate();
        changed.runtime_positions.pop();
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::RuntimeArityMismatch)
        );

        let mut changed = certificate();
        changed.runtime_positions[1].representative_position = 0;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::NonCanonicalRuntimePosition { expected: 1 })
        );

        let mut changed = certificate();
        congruence(&mut changed).parameters.swap(0, 1);
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TheoremParameterMismatch)
        );

        let mut changed = certificate();
        congruence(&mut changed).relation_premises[0].right_parameter = 2;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TheoremRelationPremiseMismatch)
        );

        let mut changed = certificate();
        congruence(&mut changed).legality_premises = transport_pair(coordinate(21), 8);
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::NonEmptyLegalityPremises)
        );

        let mut changed = certificate();
        congruence(&mut changed).conclusion.actual = coordinate(0);
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::DuplicateTheoremCoordinate)
        );

        let mut changed = certificate();
        congruence(&mut changed)
            .conclusion
            .left
            .arguments
            .swap(0, 1);
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::TheoremConclusionMismatch)
        );

        let mut changed = certificate();
        changed.result_flow.state_position = 1;
        assert_eq!(
            replayed(changed),
            Err(QuotientCorrespondenceReplayError::InvalidResultFlow)
        );

        let mut retained = retain_non_executable_quotient_correspondence(certificate());
        retained.identity.0.push(1);
        assert!(matches!(
            replay_non_executable_quotient_correspondence(&retained),
            Err(QuotientCorrespondenceReplayError::IdentityMismatch { .. })
        ));
    }

    #[test]
    fn length_delimited_identity_rejects_field_boundary_collisions() {
        let first = retain_non_executable_quotient_correspondence(certificate());
        let mut shifted = certificate();
        shifted.public_operation.declaration.push('x');
        shifted.public_operation.overload.remove(0);
        let shifted = retain_non_executable_quotient_correspondence(shifted);
        assert_ne!(first.identity, shifted.identity);
    }

    #[test]
    fn module_representation_replays_rows_but_execution_rejects_them() {
        let module = module_with(vec![retain_non_executable_quotient_correspondence(
            certificate(),
        )]);
        assert_eq!(crate::validate_module_representation(&module), Ok(()));
        assert_eq!(
            crate::validate_module(&module).unwrap_err(),
            crate::ModuleError::NonExecutableQuotientCorrespondence
        );

        let mut tampered = module;
        tampered.quotient_correspondences[0].identity.0.push(0);
        assert!(matches!(
            crate::validate_module_representation(&tampered),
            Err(crate::ModuleError::InvalidQuotientCorrespondence {
                error: QuotientCorrespondenceReplayError::IdentityMismatch { .. },
                ..
            })
        ));
    }

    #[test]
    fn transport_lift_is_representation_only_and_never_executable() {
        let module = module_with(vec![retain_non_executable_quotient_correspondence(
            transport_certificate(),
        )]);
        assert_eq!(crate::validate_module_representation(&module), Ok(()));
        assert_eq!(
            crate::validate_module(&module).unwrap_err(),
            crate::ModuleError::NonExecutableQuotientCorrespondence
        );
    }

    #[test]
    fn module_representation_rejects_order_uniqueness_and_owner_collisions() {
        let first = retain_non_executable_quotient_correspondence(certificate());
        let duplicate = module_with(vec![first.clone(), first.clone()]);
        assert_eq!(
            crate::validate_module_representation(&duplicate),
            Err(crate::ModuleError::DuplicateQuotientCorrespondenceIdentity)
        );

        let mut second_certificate = certificate();
        second_certificate.public_operation = callable("Public::other");
        let second = retain_non_executable_quotient_correspondence(second_certificate);
        let mut ordered = vec![first.clone(), second];
        ordered.sort_by(|left, right| left.identity.cmp(&right.identity));
        let mut reversed = ordered;
        reversed.reverse();
        assert_eq!(
            crate::validate_module_representation(&module_with(reversed)),
            Err(crate::ModuleError::NonCanonicalQuotientCorrespondenceOrder)
        );

        let mut collision_certificate = certificate();
        collision_certificate.result_flow.statement_position += 1;
        let collision = retain_non_executable_quotient_correspondence(collision_certificate);
        let mut collided = vec![first, collision];
        collided.sort_by(|left, right| left.identity.cmp(&right.identity));
        assert_eq!(
            crate::validate_module_representation(&module_with(collided)),
            Err(crate::ModuleError::DuplicateQuotientCorrespondenceOwner)
        );
    }
}
