//! Canonical control-flow contract and crash-route wire format.
//!
//! This module owns successor-edge envelopes, machine contracts, ordered crash
//! buckets/guards, and crash-predicate envelopes. Recursive proposition bytes
//! remain in the parent codec.

use psi_terminal::{
    ContractClause, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
    MachineContract, SuccessorEdge,
};

use super::wire::{Reader, Writer};
use super::{CodecError, decode_counted, decode_proposition, encode_proposition};

pub(super) fn encode_successor_edge(
    writer: &mut Writer,
    successor: &SuccessorEdge,
) -> Result<(), CodecError> {
    writer.id(successor.edge);
    writer.id(successor.target);
    writer.len("conditional successor arguments", successor.arguments.len())?;
    for argument in &successor.arguments {
        writer.id(*argument);
    }
    writer.len(
        "conditional successor trivial affine discards",
        successor.trivial_affine_discards.len(),
    )?;
    for place in &successor.trivial_affine_discards {
        writer.id(*place);
    }
    Ok(())
}

pub(super) fn encode_contract(
    writer: &mut Writer,
    contract: &MachineContract,
) -> Result<(), CodecError> {
    writer.id(contract.id);
    encode_crash_routes(writer, &contract.crash_routes)?;
    writer.len("requires", contract.requires.len())?;
    for proposition in &contract.requires {
        encode_proposition(writer, proposition, 0)?;
    }
    writer.len("ensures", contract.ensures.len())?;
    for clause in &contract.ensures {
        writer.id(clause.obligation);
        encode_proposition(writer, &clause.proposition, 0)?;
    }
    Ok(())
}

pub(super) fn encode_crash_routes(
    writer: &mut Writer,
    crash_routes: &[CrashRouteBucket],
) -> Result<(), CodecError> {
    writer.len("crash route buckets", crash_routes.len())?;
    for bucket in crash_routes {
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
    }
    Ok(())
}

pub(super) fn encode_crash_predicate(
    writer: &mut Writer,
    predicate: &CrashPredicateTerm,
) -> Result<(), CodecError> {
    encode_proposition(writer, predicate.proposition(), 0)
}

pub(super) fn decode_successor_edge(reader: &mut Reader<'_>) -> Result<SuccessorEdge, CodecError> {
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
        trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
    })
}

pub(super) fn decode_contract(reader: &mut Reader<'_>) -> Result<MachineContract, CodecError> {
    let id = reader.id("ContractId")?;
    let crash_routes = decode_crash_routes(reader)?;
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
    Ok(MachineContract {
        id,
        crash_routes,
        requires,
        ensures,
    })
}

pub(super) fn decode_crash_routes(
    reader: &mut Reader<'_>,
) -> Result<Vec<CrashRouteBucket>, CodecError> {
    let count = reader.count()?;
    let mut crash_routes = Vec::with_capacity(count as usize);
    for _ in 0..count {
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
        crash_routes.push(CrashRouteBucket {
            cause,
            alternatives,
        });
    }
    Ok(crash_routes)
}

pub(super) fn decode_crash_predicate(
    reader: &mut Reader<'_>,
) -> Result<CrashPredicateTerm, CodecError> {
    Ok(CrashPredicateTerm::new(decode_proposition(reader, 0)?))
}
