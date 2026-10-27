//! The terminal control-flow envelope: each terminator kind behind its tag.

use super::super::CodecError;
use super::super::contract_wire::{
    decode_crash_predicate, decode_crash_routes, decode_successor_edge, encode_crash_predicate,
    encode_successor_edge,
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
    decode_structural_arguments, decode_structural_path, encode_structural_arguments,
    encode_structural_path,
};
use crate::sections::semantic_module::structural_result_wire::{
    decode_claim_bindings, encode_claim_bindings,
};
use crate::sections::semantic_module::wire::{decode_counted, decode_ids};
use terminal_psi::{CrashCause, ExternalCompletionKind, StructuralCaseSuccessorEdge, Terminator};

pub(super) fn encode_terminator(
    writer: &mut Writer,
    terminator: &Terminator,
) -> Result<(), CodecError> {
    match terminator {
        Terminator::InvokeNoNormal {
            edge,
            static_reach_binding,
            ..
        } => {
            writer.u8(19);
            writer.id(*edge);
            writer.boolean(static_reach_binding.is_some());
            if let Some(binder) = static_reach_binding {
                writer.u32(*binder);
            }
            super::call_operations::encode_module_invocation_inputs(
                writer,
                terminator
                    .module_invocation()
                    .expect("matched module invocation"),
            )?;
        }
        Terminator::ExternalCompletion {
            edge,
            boundary,
            kind,
            arguments,
            structural_arguments,
            requirement_obligations,
            frontier_lower_bound,
        } => {
            // A distinct tag prevents legacy Unit calls from acquiring terminal
            // semantics by changing their declaration alone.
            writer.u8(17);
            writer.id(*edge);
            writer.id(*boundary);
            writer.u8(match kind {
                ExternalCompletionKind::ProcessExit => 1,
            });
            writer.len("external completion arguments", arguments.len())?;
            for argument in arguments {
                writer.id(*argument);
            }
            encode_structural_arguments(writer, structural_arguments)?;
            writer.len(
                "external completion requirements",
                requirement_obligations.len(),
            )?;
            for obligation in requirement_obligations {
                writer.id(*obligation);
            }
            writer.len(
                "external completion frontier lower bound",
                frontier_lower_bound.len(),
            )?;
            for claim in frontier_lower_bound {
                writer.id(*claim);
            }
        }
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
            // Tag 18 retains typed claim paths; the retired ID-only tag 6 is
            // never reinterpreted as a conditional payload custody envelope.
            writer.u8(18);
            writer.id(*edge);
            writer.id(*source);
            encode_claim_bindings(writer, returned_claims)?;
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
        Terminator::StructuralCase {
            source,
            path,
            cases,
        } => {
            writer.u8(16);
            writer.id(*source);
            encode_structural_path(writer, "structural case subject path", path)?;
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
        19 => Terminator::InvokeNoNormal {
            edge: reader.id("EdgeId")?,
            static_reach_binding: if reader.boolean()? {
                Some(reader.u32()?)
            } else {
                None
            },
            callee: reader.id("MachineId")?,
            arguments: decode_ids(reader, "ValueId")?,
            erased_arguments: decode_scalar_terms(reader)?,
            erased_proof_arguments: decode_proof_terms(reader)?,
            structural_arguments: decode_structural_arguments(reader)?,
            claim_transfers: super::call_operations::decode_claim_transfers(reader)?,
            requirement_obligations: decode_ids(reader, "ObligationId")?,
            crash_continuations: decode_crash_routes(reader)?,
        },
        17 => {
            let edge = reader.id("EdgeId")?;
            let boundary = reader.id("BoundaryMachineId")?;
            let kind = match reader.u8()? {
                1 => ExternalCompletionKind::ProcessExit,
                tag => return Err(CodecError::InvalidTag("ExternalCompletionKind", tag)),
            };
            Terminator::ExternalCompletion {
                edge,
                boundary,
                kind,
                arguments: decode_ids(reader, "ValueId")?,
                structural_arguments: decode_structural_arguments(reader)?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
                frontier_lower_bound: decode_ids(reader, "ClaimId")?,
            }
        }
        14 => {
            let edge = reader.id("EdgeId")?;
            let target = reader.id("BlockId")?;
            let arguments = decode_ids(reader, "ValueId")?;
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
            let site_guard = decode_counted(reader, decode_crash_predicate)?;
            let frontier_lower_bound = decode_ids(reader, "ClaimId")?;
            Terminator::Crash {
                edge,
                cause,
                site_guard,
                frontier_lower_bound,
            }
        }
        18 => Terminator::ReturnStructural {
            edge: reader.id("EdgeId")?,
            source: reader.id("PlaceId")?,
            returned_claims: decode_claim_bindings(reader)?,
            cleanup_actions: decode_counted(reader, decode_affine_cleanup_action)?,
        },
        13 => Terminator::ReturnUnit {
            edge: reader.id("EdgeId")?,
            cleanup_actions: decode_counted(reader, decode_affine_cleanup_action)?,
        },
        16 => {
            let source = reader.id("PlaceId")?;
            let path = decode_structural_path(reader)?;
            let cases = decode_counted(reader, |reader| {
                Ok(StructuralCaseSuccessorEdge {
                    edge: reader.id("EdgeId")?,
                    target: reader.id("BlockId")?,
                    case: reader.id("StructuralCaseId")?,
                    payload_fields: decode_ids(reader, "StructuralFieldId")?,
                    cleanup_actions: decode_counted(reader, decode_affine_cleanup_action)?,
                })
            })?;
            Terminator::StructuralCase {
                source,
                path,
                cases,
            }
        }
        tag => return Err(CodecError::InvalidTag("Terminator", tag)),
    })
}
