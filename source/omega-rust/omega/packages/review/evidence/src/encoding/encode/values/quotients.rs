use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use psi_language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientCallableIdentity, QuotientContractFactCoordinate,
    QuotientContractOwner, QuotientCorrespondenceOperationKind, QuotientCrashCertificate,
    QuotientForwardPreconditionTransportFact, QuotientMachineApplication,
    QuotientPositionalRelation, QuotientPurityCertificate, QuotientTerminationCertificate,
    QuotientTheoremApplicationSide, QuotientTheoremCorrespondence, QuotientTheoremParameterRole,
    QuotientTheoremRole,
};

pub(crate) fn encode_quotient_correspondence_key(
    encoder: &mut Encoder,
    certificate: &CanonicalQuotientCorrespondence,
) -> Result<(), PackageReviewEncodingError> {
    encode_callable(encoder, &certificate.public_operation)?;
    encoder.u32(certificate.result_flow.state_position);
    encoder.u32(certificate.result_flow.statement_position);
    Ok(())
}

pub(crate) fn encode_quotient_correspondence(
    encoder: &mut Encoder,
    certificate: &CanonicalQuotientCorrespondence,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match certificate.operation_kind {
        QuotientCorrespondenceOperationKind::Lift => 1,
        QuotientCorrespondenceOperationKind::Define => 2,
        QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport => 3,
    });
    encode_callable(encoder, &certificate.public_operation)?;
    encode_application(encoder, &certificate.representative)?;
    encoder.sequence(&certificate.input_relations, |encoder, relation| {
        match relation {
            QuotientPositionalRelation::Quotient(relation) => {
                encoder.byte(1);
                encode_relation(encoder, relation)?;
            }
            QuotientPositionalRelation::ExactEquality {
                public_type,
                representative_type,
            } => {
                encoder.byte(2);
                encoder.string(public_type)?;
                encoder.string(representative_type)?;
            }
        }
        Ok(())
    })?;
    encode_relation(encoder, &certificate.result_relation)?;
    encoder.sequence(&certificate.runtime_positions, |encoder, position| {
        encoder.u32(position.public_position);
        encoder.u32(position.representative_position);
        Ok(())
    })?;
    encoder.sequence(&certificate.theorem_evidence, |encoder, evidence| {
        encoder.byte(match evidence.role {
            QuotientTheoremRole::Congruence => 1,
            QuotientTheoremRole::ForwardPreconditionTransport => 2,
        });
        encode_application(encoder, &evidence.selected_application)?;
        match &evidence.correspondence {
            QuotientTheoremCorrespondence::Congruence(congruence) => {
                encoder.byte(1);
                encoder.sequence(&congruence.parameters, |encoder, parameter| {
                    encoder.u32(parameter.theorem_position);
                    match parameter.role {
                        QuotientTheoremParameterRole::QuotientLeft { input_position } => {
                            encoder.byte(1);
                            encoder.u32(input_position);
                        }
                        QuotientTheoremParameterRole::QuotientRight { input_position } => {
                            encoder.byte(2);
                            encoder.u32(input_position);
                        }
                        QuotientTheoremParameterRole::Shared { input_position } => {
                            encoder.byte(3);
                            encoder.u32(input_position);
                        }
                    }
                    Ok(())
                })?;
                encoder.sequence(&congruence.relation_premises, |encoder, premise| {
                    encoder.u32(premise.expected_position);
                    encode_coordinate(encoder, premise.actual);
                    encoder.string(&premise.relation)?;
                    encoder.u32(premise.left_parameter);
                    encoder.u32(premise.right_parameter);
                    Ok(())
                })?;
                encoder.sequence(&congruence.legality_premises, |encoder, premise| {
                    encode_transport_fact(encoder, *premise);
                    Ok(())
                })?;
                encode_coordinate(encoder, congruence.conclusion.actual);
                encoder.string(&congruence.conclusion.relation)?;
                encoder.sequence(
                    &congruence.conclusion.left.arguments,
                    |encoder, argument| {
                        encoder.u32(*argument);
                        Ok(())
                    },
                )?;
                encoder.sequence(
                    &congruence.conclusion.right.arguments,
                    |encoder, argument| {
                        encoder.u32(*argument);
                        Ok(())
                    },
                )?;
            }
            QuotientTheoremCorrespondence::ForwardPreconditionTransport(transport) => {
                encoder.byte(2);
                encoder.sequence(&transport.public_premises, |encoder, fact| {
                    encode_transport_fact(encoder, *fact);
                    Ok(())
                })?;
                encoder.sequence(&transport.representative_conclusions, |encoder, fact| {
                    encode_transport_fact(encoder, *fact);
                    Ok(())
                })?;
            }
        }
        encoder.byte(match evidence.eligibility.purity {
            QuotientPurityCertificate::PureClosure => 1,
        });
        encoder.byte(match evidence.eligibility.termination {
            QuotientTerminationCertificate::Unconditional => 1,
        });
        encoder.byte(match evidence.eligibility.crash {
            QuotientCrashCertificate::CrashFree => 1,
        });
        Ok(())
    })?;
    encoder.byte(match certificate.representative_eligibility.purity {
        QuotientPurityCertificate::PureClosure => 1,
    });
    encoder.byte(match certificate.representative_eligibility.termination {
        QuotientTerminationCertificate::Unconditional => 1,
    });
    encoder.u32(certificate.result_flow.state_position);
    encoder.u32(certificate.result_flow.statement_position);
    Ok(())
}

fn encode_callable(
    encoder: &mut Encoder,
    callable: &QuotientCallableIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&callable.declaration)?;
    encoder.string(&callable.overload)
}

fn encode_application(
    encoder: &mut Encoder,
    application: &QuotientMachineApplication,
) -> Result<(), PackageReviewEncodingError> {
    encode_callable(encoder, &application.callable)?;
    encoder.sequence(
        &application.static_application.bindings,
        |encoder, binding| encoder.string(binding),
    )
}

fn encode_relation(
    encoder: &mut Encoder,
    relation: &psi_language_semantics::quotient_correspondence::QuotientRelationIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&relation.quotient_declaration)?;
    encoder.string(&relation.quotient_type)?;
    encoder.string(&relation.carrier_type)?;
    encoder.string(&relation.relation)
}

fn encode_coordinate(encoder: &mut Encoder, coordinate: QuotientContractFactCoordinate) {
    encoder.byte(match coordinate.owner {
        QuotientContractOwner::Machine => 1,
        QuotientContractOwner::State => 2,
    });
    encoder.u32(coordinate.contract_position);
    encoder.u32(coordinate.fact_position);
}

fn encode_transport_fact(encoder: &mut Encoder, fact: QuotientForwardPreconditionTransportFact) {
    encoder.byte(match fact.application {
        QuotientTheoremApplicationSide::Left => 1,
        QuotientTheoremApplicationSide::Right => 2,
    });
    encode_coordinate(encoder, fact.source);
    encode_coordinate(encoder, fact.actual);
}
