//! The terminal control-flow envelope: each terminator kind behind its tag.

use super::super::CodecError;
use super::super::contract_wire::{
    decode_crash_predicate, decode_residual_affine_discards, decode_successor_edge,
    encode_crash_predicate, encode_residual_affine_discards, encode_successor_edge,
};
use super::super::wire::{Reader, Writer};
use crate::sections::semantic_module::proof_term_wire::{decode_proof_terms, encode_proof_terms};
use crate::sections::semantic_module::scalar_term_wire::{
    decode_scalar_terms, encode_scalar_terms,
};
use crate::sections::semantic_module::structural_place_wire::{
    decode_affine_cleanup_action, decode_structural_path, encode_affine_cleanup_action,
    encode_structural_path,
};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_arguments, encode_structural_arguments,
};
use crate::sections::semantic_module::wire::{decode_counted, decode_ids};
use terminal_psi::{CrashCause, StructuralAffineDiscard, StructuralCaseSuccessorEdge, Terminator};

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
            trivial_affine_discards,
            residual_affine_discards,
        } => {
            // Preserve the established root-only encoding byte for byte.
            writer.u8(if residual_affine_discards.is_empty() {
                1
            } else {
                10
            });
            writer.id(*edge);
            writer.id(*target);
            writer.len("jump arguments", arguments.len())?;
            for argument in arguments {
                writer.id(*argument);
            }
            encode_scalar_terms(writer, erased_arguments)?;
            encode_proof_terms(writer, erased_proof_arguments)?;
            encode_structural_arguments(writer, structural_arguments)?;
            writer.len(
                "jump trivial affine discards",
                trivial_affine_discards.len(),
            )?;
            for place in trivial_affine_discards {
                writer.id(*place);
            }
            if !residual_affine_discards.is_empty() {
                writer.len(
                    "jump residual affine discards",
                    residual_affine_discards.len(),
                )?;
                for discard in residual_affine_discards {
                    writer.id(discard.place);
                    encode_structural_path(writer, "partial affine discard path", &discard.path)?;
                    writer.id(discard.structural_type);
                }
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
            let projected = !when_true.residual_affine_discards.is_empty()
                || !when_false.residual_affine_discards.is_empty();
            writer.u8(if projected { 11 } else { 3 });
            writer.id(*condition);
            for edge in [when_true, when_false] {
                encode_successor_edge(writer, edge)?;
                if projected {
                    encode_residual_affine_discards(writer, &edge.residual_affine_discards)?;
                }
            }
        }
        Terminator::StructuralCase { source, cases } => {
            let projected = cases
                .iter()
                .any(|edge| !edge.residual_affine_discards.is_empty());
            writer.u8(if projected { 12 } else { 9 });
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
                writer.len(
                    "structural case trivial affine discards",
                    case.trivial_affine_discards.len(),
                )?;
                for place in &case.trivial_affine_discards {
                    writer.id(*place);
                }
                if projected {
                    encode_residual_affine_discards(writer, &case.residual_affine_discards)?;
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
        tag @ (1 | 10) => {
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
                trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
                residual_affine_discards: if tag == 10 {
                    let residuals = decode_counted(reader, |reader| {
                        Ok(StructuralAffineDiscard {
                            place: reader.id("PlaceId")?,
                            path: decode_structural_path(reader)?,
                            structural_type: reader.id("StructuralTypeId")?,
                        })
                    })?;
                    if residuals.is_empty() {
                        return Err(CodecError::NonCanonicalEncoding);
                    }
                    residuals
                } else {
                    Vec::new()
                },
            }
        }
        2 => Terminator::Return {
            edge: reader.id("EdgeId")?,
            value: reader.id("ValueId")?,
            cleanup_actions: decode_counted(reader, decode_affine_cleanup_action)?,
        },
        tag @ (3 | 11) => {
            let condition = reader.id("ValueId")?;
            let mut decode_edge = || {
                let mut edge = decode_successor_edge(reader)?;
                if tag == 11 {
                    edge.residual_affine_discards = decode_residual_affine_discards(reader)?;
                }
                Ok::<_, CodecError>(edge)
            };
            let when_true = decode_edge()?;
            let when_false = decode_edge()?;
            if tag == 11
                && when_true.residual_affine_discards.is_empty()
                && when_false.residual_affine_discards.is_empty()
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            }
        }
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
        tag @ (9 | 12) => {
            let source = reader.id("PlaceId")?;
            let cases = decode_counted(reader, |reader| {
                Ok(StructuralCaseSuccessorEdge {
                    edge: reader.id("EdgeId")?,
                    target: reader.id("BlockId")?,
                    case: reader.id("StructuralCaseId")?,
                    payload_fields: decode_ids(reader, "StructuralFieldId")?,
                    trivial_affine_discards: decode_ids(reader, "PlaceId")?,
                    residual_affine_discards: if tag == 12 {
                        decode_residual_affine_discards(reader)?
                    } else {
                        Vec::new()
                    },
                })
            })?;
            if tag == 12
                && cases
                    .iter()
                    .all(|edge| edge.residual_affine_discards.is_empty())
            {
                return Err(CodecError::NonCanonicalEncoding);
            }
            Terminator::StructuralCase { source, cases }
        }
        tag => return Err(CodecError::InvalidTag("Terminator", tag)),
    })
}
