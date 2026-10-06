//! The terminal control-flow envelope: each terminator kind behind its tag.

use super::super::CodecError;
use super::super::contract_wire::{
    decode_crash_predicate, decode_successor_edge, encode_crash_predicate, encode_successor_edge,
};
use super::super::wire::{Reader, Writer};
use crate::sections::semantic_module::proof_term_wire::{decode_proof_terms, encode_proof_terms};
use crate::sections::semantic_module::scalar_term_wire::{
    decode_scalar_terms, encode_scalar_terms,
};
use crate::sections::semantic_module::structural_place_wire::{
    decode_affine_cleanup_action, encode_affine_cleanup_action,
};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_arguments, encode_structural_arguments,
};
use crate::sections::semantic_module::wire::{decode_counted, decode_ids};
use terminal_psi::{CrashCause, StructuralCaseSuccessorEdge, Terminator};

pub(super) fn encode_terminator(
    writer: &mut Writer,
    terminator: &Terminator,
) -> Result<(), CodecError> {
    match terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            cleanup_actions,
        } => {
            // The ordered envelope must not reinterpret retired split-list tags.
            writer.u8(14);
            writer.id(*edge);
            writer.id(*target);
            writer.len("jump arguments", arguments.len())?;
            for argument in arguments {
                writer.id(*argument);
            }
            encode_scalar_terms(writer, erased_arguments)?;
            encode_proof_terms(writer, erased_proof_arguments)?;
            encode_structural_arguments(writer, structural_arguments)?;
            writer.len("jump cleanup actions", cleanup_actions.len())?;
            for action in cleanup_actions {
                encode_affine_cleanup_action(writer, action)?;
            }
        }
        Terminator::Return {
            edge,
            value,
            cleanup_actions,
        } => {
            writer.u8(2);
            writer.id(*edge);
            writer.id(*value);
            writer.len("scalar return cleanup actions", cleanup_actions.len())?;
            for action in cleanup_actions {
                encode_affine_cleanup_action(writer, action)?;
            }
        }
        Terminator::ReturnUnit {
            edge,
            cleanup_actions,
        } => {
            // Tag 13 replaces the retired split Unit cleanup envelopes (5, 7, 8).
            writer.u8(13);
            writer.id(*edge);
            writer.len("Unit return cleanup actions", cleanup_actions.len())?;
            for action in cleanup_actions {
                encode_affine_cleanup_action(writer, action)?;
            }
        }
        Terminator::ReturnStructural {
            edge,
            source,
            returned_claims,
            cleanup_actions,
        } => {
            writer.u8(6);
            writer.id(*edge);
            writer.id(*source);
            writer.len("structural return claims", returned_claims.len())?;
            for claim in returned_claims {
                writer.id(*claim);
            }
            writer.len("structural return cleanup actions", cleanup_actions.len())?;
            for action in cleanup_actions {
                encode_affine_cleanup_action(writer, action)?;
            }
        }
        Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            writer.u8(15);
            writer.id(*condition);
            for edge in [when_true, when_false] {
                encode_successor_edge(writer, edge)?;
            }
        }
        Terminator::StructuralCase { source, cases } => {
            writer.u8(16);
            writer.id(*source);
            writer.len("structural case successors", cases.len())?;
            for case in cases {
                writer.id(case.edge);
                writer.id(case.target);
                writer.id(case.case);
                writer.len("structural case payload fields", case.payload_fields.len())?;
                for field in &case.payload_fields {
                    writer.id(*field);
                }
                writer.len("case successor cleanup actions", case.cleanup_actions.len())?;
                for action in &case.cleanup_actions {
                    encode_affine_cleanup_action(writer, action)?;
                }
            }
        }
        Terminator::Crash {
            edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => {
            writer.u8(4);
            writer.id(*edge);
            writer.u8(match cause {
                CrashCause::Trap => 1,
                CrashCause::Abort => 2,
            });
            writer.len("crash site guard", site_guard.len())?;
            for predicate in site_guard {
                encode_crash_predicate(writer, predicate)?;
            }
            writer.len("crash frontier lower bound", frontier_lower_bound.len())?;
            for claim in frontier_lower_bound {
                writer.id(*claim);
            }
        }
    }
    Ok(())
}

pub(super) fn decode_terminator(reader: &mut Reader<'_>) -> Result<Terminator, CodecError> {
    Ok(match reader.u8()? {
        14 => {
            let edge = reader.id("EdgeId")?;
            let target = reader.id("BlockId")?;
            let argument_count = reader.count()?;
            let mut arguments = Vec::new();
            for _ in 0..argument_count {
                arguments.push(reader.id("ValueId")?);
            }
            Terminator::Jump {
                edge,
                target,
                arguments,
                erased_arguments: decode_scalar_terms(reader)?,
                erased_proof_arguments: decode_proof_terms(reader)?,
                structural_arguments: decode_structural_arguments(reader)?,
                cleanup_actions: decode_counted(reader, decode_affine_cleanup_action)?,
            }
        }
        2 => Terminator::Return {
            edge: reader.id("EdgeId")?,
            value: reader.id("ValueId")?,
            cleanup_actions: decode_counted(reader, decode_affine_cleanup_action)?,
        },
        15 => Terminator::Conditional {
            condition: reader.id("ValueId")?,
            when_true: decode_successor_edge(reader)?,
            when_false: decode_successor_edge(reader)?,
        },
        4 => {
            let edge = reader.id("EdgeId")?;
            let cause = match reader.u8()? {
                1 => CrashCause::Trap,
                2 => CrashCause::Abort,
                tag => return Err(CodecError::InvalidTag("CrashCause", tag)),
            };
            let guard_count = reader.count()?;
            let mut site_guard = Vec::with_capacity(guard_count as usize);
            for _ in 0..guard_count {
                site_guard.push(decode_crash_predicate(reader)?);
            }
            let claim_count = reader.count()?;
            let mut frontier_lower_bound = Vec::with_capacity(claim_count as usize);
            for _ in 0..claim_count {
                frontier_lower_bound.push(reader.id("ClaimId")?);
            }
            Terminator::Crash {
                edge,
                cause,
                site_guard,
                frontier_lower_bound,
            }
        }
        6 => Terminator::ReturnStructural {
            edge: reader.id("EdgeId")?,
            source: reader.id("PlaceId")?,
            returned_claims: decode_counted(reader, |reader| reader.id("ClaimId"))?,
            cleanup_actions: decode_counted(reader, decode_affine_cleanup_action)?,
        },
        13 => Terminator::ReturnUnit {
            edge: reader.id("EdgeId")?,
            cleanup_actions: decode_counted(reader, decode_affine_cleanup_action)?,
        },
        16 => {
            let source = reader.id("PlaceId")?;
            let cases = decode_counted(reader, |reader| {
                Ok(StructuralCaseSuccessorEdge {
                    edge: reader.id("EdgeId")?,
                    target: reader.id("BlockId")?,
                    case: reader.id("StructuralCaseId")?,
                    payload_fields: decode_ids(reader, "StructuralFieldId")?,
                    cleanup_actions: decode_counted(reader, decode_affine_cleanup_action)?,
                })
            })?;
            Terminator::StructuralCase { source, cases }
        }
        tag => return Err(CodecError::InvalidTag("Terminator", tag)),
    })
}
