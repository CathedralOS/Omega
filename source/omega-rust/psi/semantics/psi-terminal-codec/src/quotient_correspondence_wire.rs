//! Canonical wire form for proof-only quotient correspondence rows.

use psi_language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientCallableIdentity, QuotientContractFactCoordinate,
    QuotientContractOwner, QuotientCorrespondenceOperationKind, QuotientCrashCertificate,
    QuotientDefineRuntimePosition, QuotientDirectResultFlow, QuotientMachineApplication,
    QuotientPositionalRelation, QuotientPurityCertificate, QuotientRelationIdentity,
    QuotientRepresentativeApplication, QuotientRepresentativeEligibility,
    QuotientStaticApplication, QuotientTerminationCertificate, QuotientTheoremConclusion,
    QuotientTheoremCorrespondence, QuotientTheoremEligibility, QuotientTheoremParameter,
    QuotientTheoremParameterRole, QuotientTheoremRelationPremise,
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
    });
    encode_callable(writer, &certificate.public_operation)?;
    encode_application(writer, &certificate.representative)?;
    encode_application(writer, &certificate.selected_theorem)?;
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
        "quotient theorem parameters",
        certificate.theorem.parameters.len(),
    )?;
    for parameter in &certificate.theorem.parameters {
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
        certificate.theorem.relation_premises.len(),
    )?;
    for premise in &certificate.theorem.relation_premises {
        writer.u32(premise.expected_position);
        encode_coordinate(writer, premise.actual);
        writer.string("quotient theorem premise relation", &premise.relation)?;
        writer.u32(premise.left_parameter);
        writer.u32(premise.right_parameter);
    }
    writer.len(
        "quotient theorem legality premises",
        certificate.theorem.legality_premises.len(),
    )?;
    for premise in &certificate.theorem.legality_premises {
        encode_coordinate(writer, *premise);
    }
    encode_coordinate(writer, certificate.theorem.conclusion.actual);
    writer.string(
        "quotient theorem conclusion relation",
        &certificate.theorem.conclusion.relation,
    )?;
    encode_representative_application(writer, &certificate.theorem.conclusion.left)?;
    encode_representative_application(writer, &certificate.theorem.conclusion.right)?;
    writer.u8(match certificate.representative_eligibility.purity {
        QuotientPurityCertificate::PureClosure => 1,
    });
    writer.u8(match certificate.representative_eligibility.termination {
        QuotientTerminationCertificate::Unconditional => 1,
    });
    writer.u8(match certificate.theorem_eligibility.purity {
        QuotientPurityCertificate::PureClosure => 1,
    });
    writer.u8(match certificate.theorem_eligibility.termination {
        QuotientTerminationCertificate::Unconditional => 1,
    });
    writer.u8(match certificate.theorem_eligibility.crash {
        QuotientCrashCertificate::CrashFree => 1,
    });
    writer.u32(certificate.result_flow.state_position);
    writer.u32(certificate.result_flow.statement_position);
    Ok(())
}

pub(super) fn decode_quotient_correspondence(
    reader: &mut Reader<'_>,
) -> Result<RetainedQuotientCorrespondence, CodecError> {
    let operation_kind = match reader.u8()? {
        1 => QuotientCorrespondenceOperationKind::Lift,
        2 => QuotientCorrespondenceOperationKind::Define,
        tag => {
            return Err(CodecError::InvalidTag(
                "QuotientCorrespondenceOperationKind",
                tag,
            ));
        }
    };
    let public_operation = decode_callable(reader)?;
    let representative = decode_application(reader)?;
    let selected_theorem = decode_application(reader)?;
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
    let legality_premises = decode_counted(reader, decode_coordinate)?;
    let conclusion = QuotientTheoremConclusion {
        actual: decode_coordinate(reader)?,
        relation: reader.string("quotient theorem conclusion relation")?,
        left: decode_representative_application(reader)?,
        right: decode_representative_application(reader)?,
    };
    let representative_eligibility = QuotientRepresentativeEligibility {
        purity: decode_purity(reader)?,
        termination: decode_termination(reader)?,
    };
    let theorem_eligibility = QuotientTheoremEligibility {
        purity: decode_purity(reader)?,
        termination: decode_termination(reader)?,
        crash: match reader.u8()? {
            1 => QuotientCrashCertificate::CrashFree,
            tag => return Err(CodecError::InvalidTag("QuotientCrashCertificate", tag)),
        },
    };
    let certificate = CanonicalQuotientCorrespondence {
        operation_kind,
        public_operation,
        representative,
        selected_theorem,
        input_relations,
        result_relation,
        runtime_positions,
        theorem: QuotientTheoremCorrespondence {
            parameters,
            relation_premises,
            legality_premises,
            conclusion,
        },
        representative_eligibility,
        theorem_eligibility,
        result_flow: QuotientDirectResultFlow {
            state_position: reader.u32()?,
            statement_position: reader.u32()?,
        },
    };
    Ok(retain_non_executable_quotient_correspondence(certificate))
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
