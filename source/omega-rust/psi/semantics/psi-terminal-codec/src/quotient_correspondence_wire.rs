//! Canonical wire form for proof-only quotient correspondence rows.

use psi_language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientCallableIdentity, QuotientCongruenceCorrespondence,
    QuotientContractFactCoordinate, QuotientContractOwner, QuotientCorrespondenceOperationKind,
    QuotientCrashCertificate, QuotientDefineRuntimePosition, QuotientDirectResultFlow,
    QuotientForwardPreconditionTransportCorrespondence, QuotientForwardPreconditionTransportFact,
    QuotientMachineApplication, QuotientPositionalRelation, QuotientPurityCertificate,
    QuotientRelationIdentity, QuotientRepresentativeApplication, QuotientRepresentativeEligibility,
    QuotientStaticApplication, QuotientTerminationCertificate, QuotientTheoremApplicationSide,
    QuotientTheoremConclusion, QuotientTheoremCorrespondence, QuotientTheoremEligibility,
    QuotientTheoremEvidence, QuotientTheoremParameter, QuotientTheoremParameterRole,
    QuotientTheoremRelationPremise, QuotientTheoremRole,
};
use psi_terminal::{RetainedQuotientCorrespondence, retain_non_executable_quotient_correspondence};

use super::wire::{Reader, Writer};
use super::{CodecError, decode_counted};

pub(super) fn encode_quotient_correspondence(
    writer: &mut Writer,
    retained: &RetainedQuotientCorrespondence,
) -> Result<(), CodecError> {
    let certificate = &retained.certificate;
    writer.u8(match certificate.operation_kind {
        QuotientCorrespondenceOperationKind::Lift => 1,
        QuotientCorrespondenceOperationKind::Define => 2,
        QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport => 3,
    });
    encode_callable(writer, &certificate.public_operation)?;
    encode_application(writer, &certificate.representative)?;
    writer.len(
        "quotient input relations",
        certificate.input_relations.len(),
    )?;
    for relation in &certificate.input_relations {
        match relation {
            QuotientPositionalRelation::Quotient(relation) => {
                writer.u8(1);
                encode_relation(writer, relation)?;
            }
            QuotientPositionalRelation::ExactEquality {
                public_type,
                representative_type,
            } => {
                writer.u8(2);
                writer.string("quotient public exact type", public_type)?;
                writer.string("quotient representative exact type", representative_type)?;
            }
        }
    }
    encode_relation(writer, &certificate.result_relation)?;
    writer.len(
        "quotient runtime positions",
        certificate.runtime_positions.len(),
    )?;
    for position in &certificate.runtime_positions {
        writer.u32(position.public_position);
        writer.u32(position.representative_position);
    }
    writer.len(
        "quotient theorem evidence",
        certificate.theorem_evidence.len(),
    )?;
    for evidence in &certificate.theorem_evidence {
        writer.u8(match evidence.role {
            QuotientTheoremRole::Congruence => 1,
            QuotientTheoremRole::ForwardPreconditionTransport => 2,
        });
        encode_application(writer, &evidence.selected_application)?;
        match &evidence.correspondence {
            QuotientTheoremCorrespondence::Congruence(congruence) => {
                writer.u8(1);
                encode_congruence(writer, congruence)?;
            }
            QuotientTheoremCorrespondence::ForwardPreconditionTransport(transport) => {
                writer.u8(2);
                writer.len(
                    "quotient transport public premises",
                    transport.public_premises.len(),
                )?;
                for fact in &transport.public_premises {
                    encode_transport_fact(writer, *fact);
                }
                writer.len(
                    "quotient transport representative conclusions",
                    transport.representative_conclusions.len(),
                )?;
                for fact in &transport.representative_conclusions {
                    encode_transport_fact(writer, *fact);
                }
            }
        }
        writer.u8(match evidence.eligibility.purity {
            QuotientPurityCertificate::PureClosure => 1,
        });
        writer.u8(match evidence.eligibility.termination {
            QuotientTerminationCertificate::Unconditional => 1,
        });
        writer.u8(match evidence.eligibility.crash {
            QuotientCrashCertificate::CrashFree => 1,
        });
    }
    writer.u8(match certificate.representative_eligibility.purity {
        QuotientPurityCertificate::PureClosure => 1,
    });
    writer.u8(match certificate.representative_eligibility.termination {
        QuotientTerminationCertificate::Unconditional => 1,
    });
    writer.u32(certificate.result_flow.state_position);
    writer.u32(certificate.result_flow.statement_position);
    Ok(())
}

fn encode_congruence(
    writer: &mut Writer,
    congruence: &QuotientCongruenceCorrespondence,
) -> Result<(), CodecError> {
    writer.len("quotient theorem parameters", congruence.parameters.len())?;
    for parameter in &congruence.parameters {
        writer.u32(parameter.theorem_position);
        match parameter.role {
            QuotientTheoremParameterRole::QuotientLeft { input_position } => {
                writer.u8(1);
                writer.u32(input_position);
            }
            QuotientTheoremParameterRole::QuotientRight { input_position } => {
                writer.u8(2);
                writer.u32(input_position);
            }
            QuotientTheoremParameterRole::Shared { input_position } => {
                writer.u8(3);
                writer.u32(input_position);
            }
        }
    }
    writer.len(
        "quotient theorem relation premises",
        congruence.relation_premises.len(),
    )?;
    for premise in &congruence.relation_premises {
        writer.u32(premise.expected_position);
        encode_coordinate(writer, premise.actual);
        writer.string("quotient theorem premise relation", &premise.relation)?;
        writer.u32(premise.left_parameter);
        writer.u32(premise.right_parameter);
    }
    writer.len(
        "quotient theorem legality premises",
        congruence.legality_premises.len(),
    )?;
    for premise in &congruence.legality_premises {
        encode_transport_fact(writer, *premise);
    }
    encode_coordinate(writer, congruence.conclusion.actual);
    writer.string(
        "quotient theorem conclusion relation",
        &congruence.conclusion.relation,
    )?;
    encode_representative_application(writer, &congruence.conclusion.left)?;
    encode_representative_application(writer, &congruence.conclusion.right)?;
    Ok(())
}

pub(super) fn decode_quotient_correspondence(
    reader: &mut Reader<'_>,
) -> Result<RetainedQuotientCorrespondence, CodecError> {
    let operation_kind = match reader.u8()? {
        1 => QuotientCorrespondenceOperationKind::Lift,
        2 => QuotientCorrespondenceOperationKind::Define,
        3 => QuotientCorrespondenceOperationKind::LiftWithForwardPreconditionTransport,
        tag => {
            return Err(CodecError::InvalidTag(
                "QuotientCorrespondenceOperationKind",
                tag,
            ));
        }
    };
    let public_operation = decode_callable(reader)?;
    let representative = decode_application(reader)?;
    let input_relations = decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(QuotientPositionalRelation::Quotient(decode_relation(
            reader,
        )?)),
        2 => Ok(QuotientPositionalRelation::ExactEquality {
            public_type: reader.string("quotient public exact type")?,
            representative_type: reader.string("quotient representative exact type")?,
        }),
        tag => Err(CodecError::InvalidTag("QuotientPositionalRelation", tag)),
    })?;
    let result_relation = decode_relation(reader)?;
    let runtime_positions = decode_counted(reader, |reader| {
        Ok(QuotientDefineRuntimePosition {
            public_position: reader.u32()?,
            representative_position: reader.u32()?,
        })
    })?;
    let theorem_evidence = decode_counted(reader, decode_theorem_evidence)?;
    let representative_eligibility = QuotientRepresentativeEligibility {
        purity: decode_purity(reader)?,
        termination: decode_termination(reader)?,
    };
    let certificate = CanonicalQuotientCorrespondence {
        operation_kind,
        public_operation,
        representative,
        input_relations,
        result_relation,
        runtime_positions,
        theorem_evidence,
        representative_eligibility,
        result_flow: QuotientDirectResultFlow {
            state_position: reader.u32()?,
            statement_position: reader.u32()?,
        },
    };
    Ok(retain_non_executable_quotient_correspondence(certificate))
}

fn decode_theorem_evidence(reader: &mut Reader<'_>) -> Result<QuotientTheoremEvidence, CodecError> {
    let role = match reader.u8()? {
        1 => QuotientTheoremRole::Congruence,
        2 => QuotientTheoremRole::ForwardPreconditionTransport,
        tag => return Err(CodecError::InvalidTag("QuotientTheoremRole", tag)),
    };
    let selected_application = decode_application(reader)?;
    let correspondence = match reader.u8()? {
        1 => QuotientTheoremCorrespondence::Congruence(decode_congruence(reader)?),
        2 => QuotientTheoremCorrespondence::ForwardPreconditionTransport(
            QuotientForwardPreconditionTransportCorrespondence {
                public_premises: decode_counted(reader, decode_transport_fact)?,
                representative_conclusions: decode_counted(reader, decode_transport_fact)?,
            },
        ),
        tag => return Err(CodecError::InvalidTag("QuotientTheoremCorrespondence", tag)),
    };
    Ok(QuotientTheoremEvidence {
        role,
        selected_application,
        correspondence,
        eligibility: QuotientTheoremEligibility {
            purity: decode_purity(reader)?,
            termination: decode_termination(reader)?,
            crash: match reader.u8()? {
                1 => QuotientCrashCertificate::CrashFree,
                tag => return Err(CodecError::InvalidTag("QuotientCrashCertificate", tag)),
            },
        },
    })
}

fn encode_transport_fact(writer: &mut Writer, fact: QuotientForwardPreconditionTransportFact) {
    writer.u8(match fact.application {
        QuotientTheoremApplicationSide::Left => 1,
        QuotientTheoremApplicationSide::Right => 2,
    });
    encode_coordinate(writer, fact.source);
    encode_coordinate(writer, fact.actual);
}

fn decode_transport_fact(
    reader: &mut Reader<'_>,
) -> Result<QuotientForwardPreconditionTransportFact, CodecError> {
    let application = match reader.u8()? {
        1 => QuotientTheoremApplicationSide::Left,
        2 => QuotientTheoremApplicationSide::Right,
        tag => {
            return Err(CodecError::InvalidTag(
                "QuotientTheoremApplicationSide",
                tag,
            ));
        }
    };
    Ok(QuotientForwardPreconditionTransportFact {
        application,
        source: decode_coordinate(reader)?,
        actual: decode_coordinate(reader)?,
    })
}

fn decode_congruence(
    reader: &mut Reader<'_>,
) -> Result<QuotientCongruenceCorrespondence, CodecError> {
    let parameters = decode_counted(reader, |reader| {
        let theorem_position = reader.u32()?;
        let role = match reader.u8()? {
            1 => QuotientTheoremParameterRole::QuotientLeft {
                input_position: reader.u32()?,
            },
            2 => QuotientTheoremParameterRole::QuotientRight {
                input_position: reader.u32()?,
            },
            3 => QuotientTheoremParameterRole::Shared {
                input_position: reader.u32()?,
            },
            tag => return Err(CodecError::InvalidTag("QuotientTheoremParameterRole", tag)),
        };
        Ok(QuotientTheoremParameter {
            theorem_position,
            role,
        })
    })?;
    let relation_premises = decode_counted(reader, |reader| {
        Ok(QuotientTheoremRelationPremise {
            expected_position: reader.u32()?,
            actual: decode_coordinate(reader)?,
            relation: reader.string("quotient theorem premise relation")?,
            left_parameter: reader.u32()?,
            right_parameter: reader.u32()?,
        })
    })?;
    let legality_premises = decode_counted(reader, decode_transport_fact)?;
    let conclusion = QuotientTheoremConclusion {
        actual: decode_coordinate(reader)?,
        relation: reader.string("quotient theorem conclusion relation")?,
        left: decode_representative_application(reader)?,
        right: decode_representative_application(reader)?,
    };
    Ok(QuotientCongruenceCorrespondence {
        parameters,
        relation_premises,
        legality_premises,
        conclusion,
    })
}

fn encode_callable(
    writer: &mut Writer,
    callable: &QuotientCallableIdentity,
) -> Result<(), CodecError> {
    writer.string("quotient callable declaration", &callable.declaration)?;
    writer.string("quotient callable overload", &callable.overload)
}

fn decode_callable(reader: &mut Reader<'_>) -> Result<QuotientCallableIdentity, CodecError> {
    Ok(QuotientCallableIdentity {
        declaration: reader.string("quotient callable declaration")?,
        overload: reader.string("quotient callable overload")?,
    })
}

fn encode_application(
    writer: &mut Writer,
    application: &QuotientMachineApplication,
) -> Result<(), CodecError> {
    encode_callable(writer, &application.callable)?;
    writer.strings(
        "quotient static application bindings",
        &application.static_application.bindings,
    )
}

fn decode_application(reader: &mut Reader<'_>) -> Result<QuotientMachineApplication, CodecError> {
    Ok(QuotientMachineApplication {
        callable: decode_callable(reader)?,
        static_application: QuotientStaticApplication {
            bindings: reader.strings("quotient static application bindings")?,
        },
    })
}

fn encode_relation(
    writer: &mut Writer,
    relation: &QuotientRelationIdentity,
) -> Result<(), CodecError> {
    writer.string(
        "quotient relation declaration",
        &relation.quotient_declaration,
    )?;
    writer.string("quotient relation quotient type", &relation.quotient_type)?;
    writer.string("quotient relation carrier type", &relation.carrier_type)?;
    writer.string("quotient relation callable", &relation.relation)
}

fn decode_relation(reader: &mut Reader<'_>) -> Result<QuotientRelationIdentity, CodecError> {
    Ok(QuotientRelationIdentity {
        quotient_declaration: reader.string("quotient relation declaration")?,
        quotient_type: reader.string("quotient relation quotient type")?,
        carrier_type: reader.string("quotient relation carrier type")?,
        relation: reader.string("quotient relation callable")?,
    })
}

fn encode_coordinate(writer: &mut Writer, coordinate: QuotientContractFactCoordinate) {
    writer.u8(match coordinate.owner {
        QuotientContractOwner::Machine => 1,
        QuotientContractOwner::State => 2,
    });
    writer.u32(coordinate.contract_position);
    writer.u32(coordinate.fact_position);
}

fn decode_coordinate(
    reader: &mut Reader<'_>,
) -> Result<QuotientContractFactCoordinate, CodecError> {
    let owner = match reader.u8()? {
        1 => QuotientContractOwner::Machine,
        2 => QuotientContractOwner::State,
        tag => return Err(CodecError::InvalidTag("QuotientContractOwner", tag)),
    };
    Ok(QuotientContractFactCoordinate {
        owner,
        contract_position: reader.u32()?,
        fact_position: reader.u32()?,
    })
}

fn encode_representative_application(
    writer: &mut Writer,
    application: &QuotientRepresentativeApplication,
) -> Result<(), CodecError> {
    writer.len(
        "quotient representative arguments",
        application.arguments.len(),
    )?;
    for argument in &application.arguments {
        writer.u32(*argument);
    }
    Ok(())
}

fn decode_representative_application(
    reader: &mut Reader<'_>,
) -> Result<QuotientRepresentativeApplication, CodecError> {
    Ok(QuotientRepresentativeApplication {
        arguments: decode_counted(reader, |reader| reader.u32())?,
    })
}

fn decode_purity(reader: &mut Reader<'_>) -> Result<QuotientPurityCertificate, CodecError> {
    match reader.u8()? {
        1 => Ok(QuotientPurityCertificate::PureClosure),
        tag => Err(CodecError::InvalidTag("QuotientPurityCertificate", tag)),
    }
}

fn decode_termination(
    reader: &mut Reader<'_>,
) -> Result<QuotientTerminationCertificate, CodecError> {
    match reader.u8()? {
        1 => Ok(QuotientTerminationCertificate::Unconditional),
        tag => Err(CodecError::InvalidTag(
            "QuotientTerminationCertificate",
            tag,
        )),
    }
}
