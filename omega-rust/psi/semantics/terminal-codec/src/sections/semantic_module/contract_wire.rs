//! Canonical control-flow contract and crash-route wire format.
//!
//! This module owns successor-edge envelopes, machine contracts, ordered crash
//! buckets/guards, and crash-predicate envelopes. Recursive proposition bytes
//! remain in the parent codec.

use terminal_psi::{
    ContractClause, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
    MachineContract, OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificGuard,
    SuccessorEdge,
};

use super::wire::{Reader, Writer};
use super::{CodecError, decode_proposition, encode_proposition};
use crate::sections::semantic_module::machine_wire::{decode_declarations, encode_declarations};
use crate::sections::semantic_module::scalar_term_wire::{
    decode_scalar_terms, encode_scalar_terms,
};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_arguments, encode_structural_arguments,
};
use crate::sections::semantic_module::wire::decode_counted;

pub(crate) fn encode_successor_edge(
    writer: &mut Writer,
    successor: &SuccessorEdge,
) -> Result<(), CodecError> {
    writer.id(successor.edge);
    writer.id(successor.target);
    writer.len("conditional successor arguments", successor.arguments.len())?;
    for argument in &successor.arguments {
        writer.id(*argument);
    }
    encode_scalar_terms(writer, &successor.erased_arguments)?;
    encode_structural_arguments(writer, &successor.structural_arguments)?;
    writer.len(
        "conditional successor trivial affine discards",
        successor.trivial_affine_discards.len(),
    )?;
    for place in &successor.trivial_affine_discards {
        writer.id(*place);
    }
    Ok(())
}

pub(crate) fn encode_contract(
    writer: &mut Writer,
    contract: &MachineContract,
) -> Result<(), CodecError> {
    writer.id(contract.id);
    encode_crash_routes(writer, &contract.crash_routes)?;
    encode_declarations(
        writer,
        "erased scalar formals",
        &contract.erased_scalar_formals,
    )?;
    writer.len("requires", contract.requires.len())?;
    for proposition in &contract.requires {
        encode_proposition(writer, proposition, 0)?;
    }
    writer.len("ensures", contract.ensures.len())?;
    for clause in &contract.ensures {
        writer.id(clause.obligation);
        encode_proposition(writer, &clause.proposition, 0)?;
    }
    writer.len(
        "outcome-specific ensures",
        contract.outcome_specific_ensures.len(),
    )?;
    for row in &contract.outcome_specific_ensures {
        encode_outcome_guard(writer, row.guard);
        writer.u32(row.position);
        writer.id(row.obligation);
        encode_proposition(writer, &row.proposition, 0)?;
        writer.boolean(row.evidence.is_some());
        if let Some(evidence) = &row.evidence {
            writer.id(evidence.term);
            writer.string(
                "outcome-specific evidence output field",
                &evidence.output_field,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn encode_crash_routes(
    writer: &mut Writer,
    crash_routes: &[CrashRouteBucket],
) -> Result<(), CodecError> {
    writer.len("crash route buckets", crash_routes.len())?;
    for bucket in crash_routes {
        encode_crash_route_bucket(writer, bucket)?;
    }
    Ok(())
}

pub(crate) fn encode_crash_route_bucket(
    writer: &mut Writer,
    bucket: &CrashRouteBucket,
) -> Result<(), CodecError> {
    writer.u8(match bucket.cause {
        CrashCause::Trap => 1,
        CrashCause::Abort => 2,
    });
    writer.len("crash route alternatives", bucket.alternatives.len())?;
    for guard in &bucket.alternatives {
        match guard {
            CrashRouteGuard::Truth => writer.u8(0),
            CrashRouteGuard::Predicate(predicate) => {
                writer.u8(1);
                encode_crash_predicate(writer, predicate)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn encode_crash_predicate(
    writer: &mut Writer,
    predicate: &CrashPredicateTerm,
) -> Result<(), CodecError> {
    encode_proposition(writer, predicate.proposition(), 0)
}

pub(crate) fn decode_successor_edge(reader: &mut Reader<'_>) -> Result<SuccessorEdge, CodecError> {
    let edge = reader.id("EdgeId")?;
    let target = reader.id("BlockId")?;
    let argument_count = reader.count()?;
    let mut arguments = Vec::new();
    for _ in 0..argument_count {
        arguments.push(reader.id("ValueId")?);
    }
    Ok(SuccessorEdge {
        edge,
        target,
        arguments,
        erased_arguments: decode_scalar_terms(reader)?,
        structural_arguments: decode_structural_arguments(reader)?,
        trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
    })
}

pub(crate) fn decode_contract(reader: &mut Reader<'_>) -> Result<MachineContract, CodecError> {
    let id = reader.id("ContractId")?;
    let crash_routes = decode_crash_routes(reader)?;
    let erased_scalar_formals = decode_declarations(reader)?;
    let requires_count = reader.count()?;
    let mut requires = Vec::new();
    for _ in 0..requires_count {
        requires.push(decode_proposition(reader, 0)?);
    }
    let ensures_count = reader.count()?;
    let mut ensures = Vec::new();
    for _ in 0..ensures_count {
        ensures.push(ContractClause {
            obligation: reader.id("ObligationId")?,
            proposition: decode_proposition(reader, 0)?,
        });
    }
    let outcome_specific_ensures = decode_counted(reader, |reader| {
        let guard = decode_outcome_guard(reader)?;
        let position = reader.u32()?;
        let obligation = reader.id("ObligationId")?;
        let proposition = decode_proposition(reader, 0)?;
        let evidence = reader
            .boolean()?
            .then(|| {
                Ok(OutcomeSpecificEvidence {
                    term: reader.id("EvidenceTermId")?,
                    output_field: reader.string("outcome-specific evidence output field")?,
                })
            })
            .transpose()?;
        Ok(OutcomeSpecificEnsure {
            guard,
            position,
            obligation,
            proposition,
            evidence,
        })
    })?;
    Ok(MachineContract {
        id,
        crash_routes,
        erased_scalar_formals,
        requires,
        ensures,
        outcome_specific_ensures,
    })
}

fn encode_outcome_guard(writer: &mut Writer, guard: OutcomeSpecificGuard) {
    writer.id(guard.result_type);
    writer.id(guard.result_case);
}

fn decode_outcome_guard(reader: &mut Reader<'_>) -> Result<OutcomeSpecificGuard, CodecError> {
    Ok(OutcomeSpecificGuard {
        result_type: reader.id("StructuralTypeId")?,
        result_case: reader.id("StructuralCaseId")?,
    })
}

pub(crate) fn decode_crash_routes(
    reader: &mut Reader<'_>,
) -> Result<Vec<CrashRouteBucket>, CodecError> {
    let count = reader.count()?;
    let mut crash_routes = Vec::with_capacity(count as usize);
    for _ in 0..count {
        crash_routes.push(decode_crash_route_bucket(reader)?);
    }
    Ok(crash_routes)
}

pub(crate) fn decode_crash_route_bucket(
    reader: &mut Reader<'_>,
) -> Result<CrashRouteBucket, CodecError> {
    let cause = match reader.u8()? {
        1 => CrashCause::Trap,
        2 => CrashCause::Abort,
        tag => return Err(CodecError::InvalidTag("CrashCause", tag)),
    };
    let alternative_count = reader.count()?;
    let mut alternatives = Vec::with_capacity(alternative_count as usize);
    for _ in 0..alternative_count {
        alternatives.push(match reader.u8()? {
            0 => CrashRouteGuard::Truth,
            1 => CrashRouteGuard::Predicate(decode_crash_predicate(reader)?),
            tag => return Err(CodecError::InvalidTag("CrashRouteGuard", tag)),
        });
    }
    Ok(CrashRouteBucket {
        cause,
        alternatives,
    })
}

pub(crate) fn decode_crash_predicate(
    reader: &mut Reader<'_>,
) -> Result<CrashPredicateTerm, CodecError> {
    Ok(CrashPredicateTerm::new(decode_proposition(reader, 0)?))
}
