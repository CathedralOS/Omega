//! The terminal control-flow envelope: each terminator kind behind its tag.

use super::super::CodecError;
use super::super::contract_wire::{
    decode_crash_predicate, decode_successor_edge, encode_crash_predicate, encode_successor_edge,
};
use super::super::wire::{Reader, Writer};
use crate::sections::semantic_module::scalar_term_wire::{
    decode_scalar_terms, encode_scalar_terms,
};
use crate::sections::semantic_module::structural_place_wire::{
    decode_affine_cleanup_action, decode_structural_path, encode_affine_cleanup_action,
    encode_obligation_ids, encode_structural_path,
};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_arguments, encode_structural_arguments,
};
use crate::sections::semantic_module::wire::{
    decode_counted, decode_ids, decode_optional_id, encode_optional_id,
};
use terminal_psi::{
    CrashCause, NominalAffineCleanup, StructuralAffineDiscard, StructuralCaseSuccessorEdge,
    Terminator,
};

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
            trivial_affine_discards,
        } => {
            writer.u8(5);
            writer.id(*edge);
            writer.len(
                "return Unit trivial affine discards",
                trivial_affine_discards.len(),
            )?;
            for place in trivial_affine_discards {
                writer.id(*place);
            }
        }
        Terminator::ReturnUnitPartialAffine {
            edge,
            trivial_affine_discards,
            residual_affine_discards,
        } => {
            writer.u8(7);
            writer.id(*edge);
            writer.len(
                "partial Unit return trivial affine discards",
                trivial_affine_discards.len(),
            )?;
            for place in trivial_affine_discards {
                writer.id(*place);
            }
            writer.len(
                "partial Unit return residual affine discards",
                residual_affine_discards.len(),
            )?;
            for discard in residual_affine_discards {
                writer.id(discard.place);
                encode_structural_path(writer, "partial affine discard path", &discard.path)?;
                writer.id(discard.structural_type);
            }
        }
        Terminator::ReturnUnitNominalAffine { edge, cleanups } => {
            writer.u8(8);
            writer.id(*edge);
            writer.len("nominal affine cleanups", cleanups.len())?;
            for cleanup in cleanups {
                writer.id(cleanup.place);
                writer.id(cleanup.structural_type);
                writer.id(cleanup.cleanup_machine);
                encode_optional_id(writer, cleanup.cleanup_receiver);
                encode_obligation_ids(writer, &cleanup.requirement_obligations)?;
            }
        }
        Terminator::ReturnStructural {
            edge,
            source,
            returned_claims,
            trivial_affine_discards,
        } => {
            writer.u8(6);
            writer.id(*edge);
            writer.id(*source);
            writer.len("structural return claims", returned_claims.len())?;
            for claim in returned_claims {
                writer.id(*claim);
            }
            writer.len(
                "structural return trivial affine discards",
                trivial_affine_discards.len(),
            )?;
            for place in trivial_affine_discards {
                writer.id(*place);
            }
        }
        Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            writer.u8(3);
            writer.id(*condition);
            encode_successor_edge(writer, when_true)?;
            encode_successor_edge(writer, when_false)?;
        }
        Terminator::StructuralCase { source, cases } => {
            writer.u8(9);
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
        3 => Terminator::Conditional {
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
        5 => Terminator::ReturnUnit {
            edge: reader.id("EdgeId")?,
            trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
        },
        6 => Terminator::ReturnStructural {
            edge: reader.id("EdgeId")?,
            source: reader.id("PlaceId")?,
            returned_claims: decode_counted(reader, |reader| reader.id("ClaimId"))?,
            trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
        },
        7 => Terminator::ReturnUnitPartialAffine {
            edge: reader.id("EdgeId")?,
            trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
            residual_affine_discards: decode_counted(reader, |reader| {
                Ok(StructuralAffineDiscard {
                    place: reader.id("PlaceId")?,
                    path: decode_structural_path(reader)?,
                    structural_type: reader.id("StructuralTypeId")?,
                })
            })?,
        },
        8 => Terminator::ReturnUnitNominalAffine {
            edge: reader.id("EdgeId")?,
            cleanups: decode_counted(reader, |reader| {
                Ok(NominalAffineCleanup {
                    place: reader.id("PlaceId")?,
                    structural_type: reader.id("StructuralTypeId")?,
                    cleanup_machine: reader.id("MachineId")?,
                    cleanup_receiver: decode_optional_id(reader, "PlaceId")?,
                    requirement_obligations: decode_ids(reader, "ObligationId")?,
                })
            })?,
        },
        9 => Terminator::StructuralCase {
            source: reader.id("PlaceId")?,
            cases: decode_counted(reader, |reader| {
                Ok(StructuralCaseSuccessorEdge {
                    edge: reader.id("EdgeId")?,
                    target: reader.id("BlockId")?,
                    case: reader.id("StructuralCaseId")?,
                    payload_fields: decode_ids(reader, "StructuralFieldId")?,
                    trivial_affine_discards: decode_ids(reader, "PlaceId")?,
                })
            })?,
        },
        tag => return Err(CodecError::InvalidTag("Terminator", tag)),
    })
}
