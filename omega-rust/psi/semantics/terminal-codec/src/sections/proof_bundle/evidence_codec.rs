//! Evidence producers, evidence routes, component certificates and
//! admission kinds on the wire.

use crate::sections::proof_bundle::ProofCodecError;
use crate::sections::proof_bundle::proof_node_codec::{decode_proof_node, encode_proof_node};
use crate::sections::proof_bundle::scalar_term_codec::{decode_primitive, encode_primitive};
use crate::sections::proof_bundle::wire::{Reader, Writer};
use proof_admission::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, EvidenceRoute, ProofSystemMarker,
    RecursiveComponentCertificate, RecursiveEdgeCertificate,
};
use terminal_psi::{CrashCause, CrashObligationOwner};
use terminal_verifier::{
    CrashCertificate, CrashObligationEvidence, EvidenceProducerProvenance,
    EvidenceProducerRealization, EvidenceProducerRowSource, ObligationEvidence,
    RecursiveComponentEvidence,
};

pub(crate) fn encode_evidence_producer(
    writer: &mut Writer,
    producer: &EvidenceProducerProvenance,
) -> Result<(), ProofCodecError> {
    writer.id(producer.id);
    writer.id(producer.term);
    writer.string(
        "evidence producer conformance",
        &producer.conformance_identity,
    )?;
    writer.string("evidence producer trait", &producer.evidence_trait_identity)?;
    writer.len("evidence producer rows", producer.rows.len())?;
    for row in &producer.rows {
        writer.string(
            "evidence producer declaring trait",
            &row.declaring_trait_identity,
        )?;
        writer.len(
            "evidence producer declaring trait arguments",
            row.declaring_trait_arguments.len(),
        )?;
        for argument in &row.declaring_trait_arguments {
            writer.string("evidence producer declaring trait argument", argument)?;
        }
        writer.string("evidence producer requirement", &row.requirement_identity)?;
        writer.string(
            "evidence producer machine",
            &row.realization_machine_identity,
        )?;
        writer.string("evidence producer state", &row.realization_state_identity)?;
        writer.u8(match row.source {
            EvidenceProducerRowSource::Inline => 1,
            EvidenceProducerRowSource::Reference => 2,
            EvidenceProducerRowSource::TraitDefault => 3,
        });
    }
    Ok(())
}

pub(crate) fn decode_evidence_producer(
    reader: &mut Reader<'_>,
) -> Result<EvidenceProducerProvenance, ProofCodecError> {
    let id = reader.id("EvidenceIdentity")?;
    let term = reader.id("EvidenceTermId")?;
    let conformance_identity = reader.string("evidence producer conformance")?;
    let evidence_trait_identity = reader.string("evidence producer trait")?;
    let row_count = reader.count()?;
    let mut rows = Vec::new();
    for _ in 0..row_count {
        let declaring_trait_identity = reader.string("evidence producer declaring trait")?;
        let argument_count = reader.count()?;
        let mut declaring_trait_arguments = Vec::new();
        for _ in 0..argument_count {
            declaring_trait_arguments
                .push(reader.string("evidence producer declaring trait argument")?);
        }
        rows.push(EvidenceProducerRealization {
            declaring_trait_identity,
            declaring_trait_arguments,
            requirement_identity: reader.string("evidence producer requirement")?,
            realization_machine_identity: reader.string("evidence producer machine")?,
            realization_state_identity: reader.string("evidence producer state")?,
            source: match reader.u8()? {
                1 => EvidenceProducerRowSource::Inline,
                2 => EvidenceProducerRowSource::Reference,
                3 => EvidenceProducerRowSource::TraitDefault,
                tag => {
                    return Err(ProofCodecError::InvalidTag(
                        "EvidenceProducerRowSource",
                        tag,
                    ));
                }
            },
        });
    }
    Ok(EvidenceProducerProvenance {
        id,
        term,
        conformance_identity,
        evidence_trait_identity,
        rows,
    })
}

pub(crate) fn encode_evidence(
    writer: &mut Writer,
    evidence: &ObligationEvidence,
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    writer.id(evidence.obligation);
    encode_evidence_route(writer, &evidence.route, format_marker)
}

fn encode_evidence_route(
    writer: &mut Writer,
    route: &EvidenceRoute,
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    match route {
        EvidenceRoute::KernelDerived(judgment) => {
            writer.u8(1);
            encode_primitive(writer, *judgment);
        }
        EvidenceRoute::CertificateDerived(certificate) => {
            writer.u8(2);
            writer.id(certificate.identity);
            writer.u16(certificate.proof_system_marker.get());
            encode_proof_node(writer, &certificate.proof, 0, format_marker)?;
        }
        EvidenceRoute::Admitted(evidence) => {
            writer.u8(3);
            writer.id(evidence.site);
            encode_admission_kind(writer, evidence.kind);
            writer.id(evidence.authority_identity);
            writer.id(evidence.evidence_identity);
            writer.id(evidence.profile_decision);
        }
    }
    Ok(())
}

pub(crate) fn encode_component_certificate(
    writer: &mut Writer,
    certificate: &RecursiveComponentCertificate,
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    writer.id(certificate.identity);
    writer.id(certificate.ranking_relation);
    encode_evidence_route(writer, &certificate.well_foundedness, format_marker)?;
    writer.len("recursive component edge evidence", certificate.edges.len())?;
    for edge in &certificate.edges {
        writer.id(edge.obligation);
        encode_evidence_route(writer, &edge.evidence, format_marker)?;
    }
    Ok(())
}

fn encode_admission_kind(writer: &mut Writer, kind: AdmissionKind) {
    writer.u8(match kind {
        AdmissionKind::ForeignBoundaryGuarantee => 1,
        AdmissionKind::ProviderFact => 2,
        AdmissionKind::CheckedAssemblyClaim => 3,
    });
}

pub(crate) fn decode_evidence(
    reader: &mut Reader<'_>,
    format_marker: u16,
) -> Result<ObligationEvidence, ProofCodecError> {
    let obligation = reader.id("ObligationId")?;
    let route = decode_evidence_route(reader, format_marker)?;
    Ok(ObligationEvidence { obligation, route })
}

fn decode_evidence_route(
    reader: &mut Reader<'_>,
    format_marker: u16,
) -> Result<EvidenceRoute, ProofCodecError> {
    Ok(match reader.u8()? {
        1 => EvidenceRoute::KernelDerived(decode_primitive(reader)?),
        2 => {
            let identity = reader.id("EvidenceIdentity")?;
            let raw_marker = reader.u16()?;
            EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity,
                proof_system_marker: ProofSystemMarker::new(raw_marker)
                    .ok_or(ProofCodecError::UnsupportedProofSystemMarker(raw_marker))?,
                proof: decode_proof_node(reader, 0, format_marker)?,
            })
        }
        3 => EvidenceRoute::Admitted(AdmissionEvidence {
            site: reader.id("AdmissionSiteId")?,
            kind: decode_admission_kind(reader)?,
            authority_identity: reader.id("EvidenceIdentity")?,
            evidence_identity: reader.id("EvidenceIdentity")?,
            profile_decision: reader.id("ProfileDecisionId")?,
        }),
        tag => return Err(ProofCodecError::InvalidTag("EvidenceRoute", tag)),
    })
}

pub(crate) fn decode_recursive_component_evidence(
    reader: &mut Reader<'_>,
    format_marker: u16,
) -> Result<RecursiveComponentEvidence, ProofCodecError> {
    let component = reader.id("RecursiveComponentId")?;
    Ok(RecursiveComponentEvidence {
        component,
        certificate: decode_component_certificate(reader, format_marker)?,
    })
}

pub(crate) fn decode_component_certificate(
    reader: &mut Reader<'_>,
    format_marker: u16,
) -> Result<RecursiveComponentCertificate, ProofCodecError> {
    let identity = reader.id("EvidenceIdentity")?;
    let ranking_relation = reader.id("RankingRelationId")?;
    let well_foundedness = decode_evidence_route(reader, format_marker)?;
    let edge_count = reader.count()?;
    let mut edges = Vec::new();
    for _ in 0..edge_count {
        edges.push(RecursiveEdgeCertificate {
            obligation: reader.id("ObligationId")?,
            evidence: decode_evidence_route(reader, format_marker)?,
        });
    }
    Ok(RecursiveComponentCertificate {
        identity,
        ranking_relation,
        well_foundedness,
        edges,
    })
}

fn decode_admission_kind(reader: &mut Reader<'_>) -> Result<AdmissionKind, ProofCodecError> {
    match reader.u8()? {
        1 => Ok(AdmissionKind::ForeignBoundaryGuarantee),
        2 => Ok(AdmissionKind::ProviderFact),
        3 => Ok(AdmissionKind::CheckedAssemblyClaim),
        tag => Err(ProofCodecError::InvalidTag("AdmissionKind", tag)),
    }
}

pub(crate) fn encode_crash_obligation_evidence(
    writer: &mut Writer,
    evidence: &CrashObligationEvidence,
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    encode_crash_obligation_owner(writer, evidence.owner);
    encode_crash_certificate_rosters(writer, &evidence.coverage, format_marker)?;
    encode_crash_certificate_rosters(writer, &evidence.refutation, format_marker)
}

fn encode_crash_obligation_owner(writer: &mut Writer, owner: CrashObligationOwner) {
    match owner {
        CrashObligationOwner::Site {
            machine,
            block,
            edge,
        } => {
            writer.u8(1);
            writer.id(machine);
            writer.id(block);
            writer.id(edge);
        }
        CrashObligationOwner::Continuation {
            machine,
            operation,
            cause,
        } => {
            writer.u8(2);
            writer.id(machine);
            writer.id(operation);
            encode_crash_cause(writer, cause);
        }
    }
}

fn encode_crash_cause(writer: &mut Writer, cause: CrashCause) {
    writer.u8(match cause {
        CrashCause::Trap => 1,
        CrashCause::Abort => 2,
    });
}

fn encode_crash_certificate_rosters(
    writer: &mut Writer,
    rosters: &[Vec<CrashCertificate>],
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    writer.len("crash certificate rosters", rosters.len())?;
    for roster in rosters {
        writer.len("crash certificates", roster.len())?;
        for certificate in roster {
            writer.u8(u8::from(certificate.with_value_equalities));
            encode_proof_node(writer, &certificate.proof, 0, format_marker)?;
        }
    }
    Ok(())
}

pub(crate) fn decode_crash_obligation_evidence(
    reader: &mut Reader<'_>,
    format_marker: u16,
) -> Result<CrashObligationEvidence, ProofCodecError> {
    let owner = decode_crash_obligation_owner(reader)?;
    let coverage = decode_crash_certificate_rosters(reader, format_marker)?;
    let refutation = decode_crash_certificate_rosters(reader, format_marker)?;
    Ok(CrashObligationEvidence {
        owner,
        coverage,
        refutation,
    })
}

fn decode_crash_obligation_owner(
    reader: &mut Reader<'_>,
) -> Result<CrashObligationOwner, ProofCodecError> {
    Ok(match reader.u8()? {
        1 => CrashObligationOwner::Site {
            machine: reader.id("MachineId")?,
            block: reader.id("BlockId")?,
            edge: reader.id("EdgeId")?,
        },
        2 => CrashObligationOwner::Continuation {
            machine: reader.id("MachineId")?,
            operation: reader.id("OperationId")?,
            cause: decode_crash_cause(reader)?,
        },
        tag => return Err(ProofCodecError::InvalidTag("CrashObligationOwner", tag)),
    })
}

fn decode_crash_cause(reader: &mut Reader<'_>) -> Result<CrashCause, ProofCodecError> {
    match reader.u8()? {
        1 => Ok(CrashCause::Trap),
        2 => Ok(CrashCause::Abort),
        tag => Err(ProofCodecError::InvalidTag("CrashCause", tag)),
    }
}

fn decode_crash_certificate_rosters(
    reader: &mut Reader<'_>,
    format_marker: u16,
) -> Result<Vec<Vec<CrashCertificate>>, ProofCodecError> {
    let roster_count = reader.count()?;
    let mut rosters = Vec::new();
    for _ in 0..roster_count {
        let certificate_count = reader.count()?;
        let mut roster = Vec::new();
        for _ in 0..certificate_count {
            roster.push(CrashCertificate {
                with_value_equalities: reader.boolean()?,
                proof: decode_proof_node(reader, 0, format_marker)?,
            });
        }
        rosters.push(roster);
    }
    Ok(rosters)
}
