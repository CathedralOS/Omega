//! Independent replay of the standalone quotient-correspondence bridge.

use std::collections::BTreeSet;

use language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientContractFactCoordinate, QuotientContractOwner,
    QuotientCorrespondenceOperationKind, QuotientPositionalRelation, QuotientResultFlow,
    QuotientTheoremApplicationSide, QuotientTheoremCorrespondence, QuotientTheoremParameter,
    QuotientTheoremParameterRole, QuotientTheoremRole,
};
use terminal_psi::{QuotientCorrespondenceIdentity, RetainedQuotientCorrespondence};

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
    InconsistentRuntimePosition {
        position: u32,
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
    match certificate.operation_kind {
        // `define` is faithful and declaration-order preserving.
        QuotientCorrespondenceOperationKind::Define => {
            for (position, runtime) in certificate.runtime_positions.iter().enumerate() {
                let expected = u32::try_from(position)
                    .map_err(|_| QuotientCorrespondenceReplayError::RuntimeArityMismatch)?;
                if runtime.public_position != expected
                    || runtime.representative_position != expected
                {
                    return Err(
                        QuotientCorrespondenceReplayError::NonCanonicalRuntimePosition { expected },
                    );
                }
            }
        }
        // `lift` rows are in representative order and may adapt arguments:
        // `public_position` selects, permutes, repeats, or omits direct
        // public parameters. Rows that select the same public parameter must
        // carry the same input relation, or the retained map is inconsistent.
        QuotientCorrespondenceOperationKind::Lift
        | QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport => {
            for (position, runtime) in certificate.runtime_positions.iter().enumerate() {
                let expected = u32::try_from(position)
                    .map_err(|_| QuotientCorrespondenceReplayError::RuntimeArityMismatch)?;
                if runtime.representative_position != expected {
                    return Err(
                        QuotientCorrespondenceReplayError::NonCanonicalRuntimePosition { expected },
                    );
                }
                if certificate.runtime_positions[..position]
                    .iter()
                    .enumerate()
                    .any(|(earlier, candidate)| {
                        candidate.public_position == runtime.public_position
                            && certificate.input_relations[earlier]
                                != certificate.input_relations[position]
                    })
                {
                    return Err(
                        QuotientCorrespondenceReplayError::InconsistentRuntimePosition {
                            position: expected,
                        },
                    );
                }
            }
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
    validate_result_flow(&certificate.result_flow)?;

    let expected = independent_identity(certificate);
    if retained.identity != expected {
        return Err(QuotientCorrespondenceReplayError::IdentityMismatch {
            expected,
            actual: retained.identity.clone(),
        });
    }
    Ok(())
}

/// Re-derive the retained result-flow shape without source. A `Direct` row
/// asserts a single-state owner, so only `Forwarded` carries a graph: every
/// non-result state must contribute exactly one edge, and every chain must
/// converge on the result state without a cycle. `machine_state_count` is
/// retained precisely so replay can check the coverage claim rather than
/// trust it.
fn validate_result_flow(
    result_flow: &QuotientResultFlow,
) -> Result<(), QuotientCorrespondenceReplayError> {
    let QuotientResultFlow::Forwarded {
        machine_state_count,
        result_state_position,
        forwarding,
        ..
    } = result_flow
    else {
        return Ok(());
    };
    let state_count = usize::try_from(*machine_state_count)
        .map_err(|_| QuotientCorrespondenceReplayError::InvalidResultFlow)?;
    let result_position = usize::try_from(*result_state_position)
        .map_err(|_| QuotientCorrespondenceReplayError::InvalidResultFlow)?;
    if state_count < 2 || result_position >= state_count || forwarding.len() + 1 != state_count {
        return Err(QuotientCorrespondenceReplayError::InvalidResultFlow);
    }
    let mut sources = BTreeSet::new();
    for edge in forwarding {
        let source = usize::try_from(edge.source_position)
            .map_err(|_| QuotientCorrespondenceReplayError::InvalidResultFlow)?;
        let target = usize::try_from(edge.target_position)
            .map_err(|_| QuotientCorrespondenceReplayError::InvalidResultFlow)?;
        if source >= state_count
            || target >= state_count
            || source == result_position
            || !sources.insert(source)
        {
            return Err(QuotientCorrespondenceReplayError::InvalidResultFlow);
        }
    }
    // Every non-result position is a unique source, so each chain either
    // reaches the result state or cycles: follow it to decide.
    for edge in forwarding {
        let mut current = edge.source_position;
        let mut visited = BTreeSet::new();
        while current != *result_state_position {
            if !visited.insert(current) {
                return Err(QuotientCorrespondenceReplayError::InvalidResultFlow);
            }
            current = forwarding
                .iter()
                .find(|candidate| candidate.source_position == current)
                .map(|candidate| candidate.target_position)
                .ok_or(QuotientCorrespondenceReplayError::InvalidResultFlow)?;
        }
    }
    Ok(())
}

fn validate_transport_roster(
    facts: &[language_semantics::quotient_correspondence::QuotientForwardPreconditionTransportFact],
) -> Result<(), QuotientCorrespondenceReplayError> {
    if !facts.len().is_multiple_of(2) {
        return Err(QuotientCorrespondenceReplayError::TransportFactRosterMismatch);
    }
    let mut previous_source = None;
    let mut actuals = BTreeSet::new();
    let mut previous_actual = None;
    for pair in facts.as_chunks::<2>().0 {
        if pair[0].application != QuotientTheoremApplicationSide::Left
            || pair[1].application != QuotientTheoremApplicationSide::Right
            || pair[0].source != pair[1].source
            || previous_source.is_some_and(|previous| previous >= pair[0].source)
            || !actuals.insert(pair[0].actual)
            || !actuals.insert(pair[1].actual)
            || previous_actual.is_some_and(|previous| previous >= pair[0].actual)
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
    callable: &language_semantics::quotient_correspondence::QuotientCallableIdentity,
) -> Result<(), QuotientCorrespondenceReplayError> {
    if !hermetic(&callable.declaration) || callable.overload.is_empty() {
        return Err(QuotientCorrespondenceReplayError::NonHermeticIdentity);
    }
    Ok(())
}

fn validate_relation(
    relation: &language_semantics::quotient_correspondence::QuotientRelationIdentity,
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
    writer.string("omega.quotient-correspondence.transport-coordinates.v4");
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
    write_result_flow_identity(&mut writer, &certificate.result_flow);
    QuotientCorrespondenceIdentity(writer.finish())
}

fn write_result_flow_identity(writer: &mut ReplayIdentityWriter, result_flow: &QuotientResultFlow) {
    match result_flow {
        QuotientResultFlow::Direct {
            statement_position,
            immutable_alias_count,
        } => {
            writer.byte(1);
            writer.u32(*statement_position);
            writer.u32(*immutable_alias_count);
        }
        QuotientResultFlow::Forwarded {
            machine_state_count,
            result_state_position,
            statement_position,
            immutable_alias_count,
            forwarding,
        } => {
            writer.byte(2);
            writer.u32(*machine_state_count);
            writer.u32(*result_state_position);
            writer.u32(*statement_position);
            writer.u32(*immutable_alias_count);
            writer.len(forwarding.len());
            for edge in forwarding {
                writer.u32(edge.source_position);
                writer.u32(edge.target_position);
            }
        }
    }
}

fn write_congruence_identity(
    writer: &mut ReplayIdentityWriter,
    congruence: &language_semantics::quotient_correspondence::QuotientCongruenceCorrespondence,
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
        callable: &language_semantics::quotient_correspondence::QuotientCallableIdentity,
    ) {
        self.string(&callable.declaration);
        self.string(&callable.overload);
    }
    fn application(
        &mut self,
        application: &language_semantics::quotient_correspondence::QuotientMachineApplication,
    ) {
        self.callable(&application.callable);
        self.len(application.static_application.bindings.len());
        for binding in &application.static_application.bindings {
            self.string(binding);
        }
    }
    fn relation(
        &mut self,
        relation: &language_semantics::quotient_correspondence::QuotientRelationIdentity,
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
        fact: language_semantics::quotient_correspondence::QuotientForwardPreconditionTransportFact,
    ) {
        self.byte(match fact.application {
            QuotientTheoremApplicationSide::Left => 1,
            QuotientTheoremApplicationSide::Right => 2,
        });
        self.coordinate(fact.source);
        self.coordinate(fact.actual);
    }
}
