//! Canonical terminal block, operation, and terminator wire format.
//!
//! This module owns operation-result and operation-kind rows plus the exact
//! terminal control-flow envelope. Shared structural paths, call arguments,
//! contracts, and declaration primitives remain sibling- or parent-owned.

use psi_terminal::{
    Block, ClaimTransfer, CompletionReceipt, CrashCause, NominalAffineCleanup, Operation,
    OperationKind, OperationResult, StructuralAffineDiscard, Terminator,
};

use super::contract_wire::{
    decode_crash_predicate, decode_crash_routes, decode_successor_edge, encode_crash_predicate,
    encode_crash_routes, encode_successor_edge,
};
use super::machine_wire::{
    decode_declaration, decode_declarations, encode_declaration, encode_declarations,
};
use super::scalar_wire::{decode_integer_value, encode_integer_value};
use super::wire::{Reader, Writer};
use super::{
    CodecError, decode_affine_cleanup_action, decode_counted, decode_ids, decode_optional_id,
    decode_structural_arguments, decode_structural_path, encode_affine_cleanup_action,
    encode_obligation_ids, encode_optional_id, encode_structural_arguments, encode_structural_path,
};

pub(super) fn encode_block(writer: &mut Writer, block: &Block) -> Result<(), CodecError> {
    writer.id(block.id);
    encode_declarations(writer, "block parameters", &block.parameters)?;
    writer.len("operations", block.operations.len())?;
    for operation in &block.operations {
        writer.id(operation.id);
        match operation.result {
            OperationResult::Unit => writer.u8(0),
            OperationResult::Scalar(result) => {
                writer.u8(1);
                encode_declaration(writer, result);
            }
        }
        match operation.kind.clone() {
            OperationKind::EstablishTrivialAffineLocal { destination } => {
                writer.u8(37);
                writer.id(destination);
            }
            OperationKind::Call {
                callee,
                arguments,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(33);
                writer.id(callee);
                writer.len("call arguments", arguments.len())?;
                for argument in arguments {
                    writer.id(argument);
                }
                writer.len(
                    "call requirement obligations",
                    requirement_obligations.len(),
                )?;
                for obligation in requirement_obligations {
                    writer.id(obligation);
                }
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::CallUnit {
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(34);
                writer.id(callee);
                encode_structural_arguments(writer, &structural_arguments)?;
                writer.len("unit-call claim transfers", claim_transfers.len())?;
                for transfer in claim_transfers {
                    writer.id(transfer.claim);
                    writer.u32(transfer.argument_index);
                }
                encode_obligation_ids(writer, &requirement_obligations)?;
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::CallStructuralScalar {
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(39);
                writer.id(callee);
                encode_structural_arguments(writer, &structural_arguments)?;
                writer.len(
                    "structural-scalar-call claim transfers",
                    claim_transfers.len(),
                )?;
                for transfer in claim_transfers {
                    writer.id(transfer.claim);
                    writer.u32(transfer.argument_index);
                }
                encode_obligation_ids(writer, &requirement_obligations)?;
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::BoundaryCall {
                boundary,
                structural_arguments,
                completion_receipts,
                requirement_obligations,
            } => {
                writer.u8(35);
                writer.id(boundary);
                encode_structural_arguments(writer, &structural_arguments)?;
                writer.len("boundary claim settlements", completion_receipts.len())?;
                for settlement in completion_receipts {
                    writer.id(settlement.claim);
                    writer.u32(settlement.argument_index);
                }
                encode_obligation_ids(writer, &requirement_obligations)?;
            }
            OperationKind::PortWrite {
                service,
                port,
                value,
            } => {
                writer.u8(36);
                writer.id(service);
                writer.u16(port);
                writer.u8(value);
            }
            OperationKind::IntegerConstant { value } => {
                writer.u8(1);
                encode_integer_value(writer, value);
            }
            OperationKind::BooleanConstant { value } => {
                writer.u8(2);
                writer.u8(u8::from(value));
            }
            OperationKind::BooleanStructuralField { source, field } => {
                writer.u8(38);
                writer.id(source);
                writer.id(field);
            }
            OperationKind::BooleanNot { operand } => {
                writer.u8(9);
                writer.id(operand);
            }
            OperationKind::BooleanEqual { left, right } => {
                writer.u8(10);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerEqual { left, right } => {
                writer.u8(11);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerLessThan { left, right } => {
                writer.u8(12);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerLessOrEqual { left, right } => {
                writer.u8(13);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerBitwiseNot { operand } => {
                writer.u8(19);
                writer.id(operand);
            }
            OperationKind::IntegerWiden { operand } => {
                writer.u8(20);
                writer.id(operand);
            }
            OperationKind::IntegerExactCast {
                operand,
                obligation,
            } => {
                writer.u8(21);
                writer.id(operand);
                writer.id(obligation);
            }
            OperationKind::IntegerBitwiseAnd { left, right } => {
                writer.u8(14);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerBitwiseOr { left, right } => {
                writer.u8(15);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::IntegerBitwiseXor { left, right } => {
                writer.u8(16);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::WrappingIntegerShiftLeft { value, count } => {
                writer.u8(17);
                writer.id(value);
                writer.id(count);
            }
            OperationKind::WrappingIntegerShiftRight { value, count } => {
                writer.u8(18);
                writer.id(value);
                writer.id(count);
            }
            OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation,
            } => {
                writer.u8(23);
                writer.id(value);
                writer.id(count);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation,
            } => {
                writer.u8(22);
                writer.id(value);
                writer.id(count);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } => {
                writer.u8(24);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } => {
                writer.u8(25);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation,
            } => {
                writer.u8(26);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation,
            } => {
                writer.u8(27);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation,
            } => {
                writer.u8(28);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation,
            } => {
                writer.u8(29);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation,
            } => {
                writer.u8(30);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation,
            } => {
                writer.u8(31);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation,
            } => {
                writer.u8(32);
                writer.id(left);
                writer.id(right);
                writer.id(obligation);
            }
            OperationKind::WrappingIntegerAdd { left, right } => {
                writer.u8(3);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::SaturatingIntegerAdd { left, right } => {
                writer.u8(4);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::WrappingIntegerSubtract { left, right } => {
                writer.u8(5);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::SaturatingIntegerSubtract { left, right } => {
                writer.u8(6);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::WrappingIntegerMultiply { left, right } => {
                writer.u8(7);
                writer.id(left);
                writer.id(right);
            }
            OperationKind::SaturatingIntegerMultiply { left, right } => {
                writer.u8(8);
                writer.id(left);
                writer.id(right);
            }
        }
    }
    match &block.terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
            trivial_affine_discards,
        } => {
            writer.u8(1);
            writer.id(*edge);
            writer.id(*target);
            writer.len("jump arguments", arguments.len())?;
            for argument in arguments {
                writer.id(*argument);
            }
            writer.len(
                "jump trivial affine discards",
                trivial_affine_discards.len(),
            )?;
            for place in trivial_affine_discards {
                writer.id(*place);
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

pub(super) fn decode_block(reader: &mut Reader<'_>) -> Result<Block, CodecError> {
    let id = reader.id("BlockId")?;
    let parameters = decode_declarations(reader)?;
    let operation_count = reader.count()?;
    let mut operations = Vec::new();
    for _ in 0..operation_count {
        let operation_id = reader.id("OperationId")?;
        let result = match reader.u8()? {
            0 => OperationResult::Unit,
            1 => OperationResult::Scalar(decode_declaration(reader)?),
            tag => return Err(CodecError::InvalidTag("OperationResult", tag)),
        };
        let kind = match reader.u8()? {
            1 => OperationKind::IntegerConstant {
                value: decode_integer_value(reader)?,
            },
            2 => OperationKind::BooleanConstant {
                value: reader.boolean()?,
            },
            38 => OperationKind::BooleanStructuralField {
                source: reader.id("PlaceId")?,
                field: reader.id("StructuralFieldId")?,
            },
            3 => OperationKind::WrappingIntegerAdd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            4 => OperationKind::SaturatingIntegerAdd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            5 => OperationKind::WrappingIntegerSubtract {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            6 => OperationKind::SaturatingIntegerSubtract {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            7 => OperationKind::WrappingIntegerMultiply {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            8 => OperationKind::SaturatingIntegerMultiply {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            9 => OperationKind::BooleanNot {
                operand: reader.id("ValueId")?,
            },
            10 => OperationKind::BooleanEqual {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            11 => OperationKind::IntegerEqual {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            12 => OperationKind::IntegerLessThan {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            13 => OperationKind::IntegerLessOrEqual {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            14 => OperationKind::IntegerBitwiseAnd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            15 => OperationKind::IntegerBitwiseOr {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            16 => OperationKind::IntegerBitwiseXor {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
            },
            17 => OperationKind::WrappingIntegerShiftLeft {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
            },
            18 => OperationKind::WrappingIntegerShiftRight {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
            },
            19 => OperationKind::IntegerBitwiseNot {
                operand: reader.id("ValueId")?,
            },
            20 => OperationKind::IntegerWiden {
                operand: reader.id("ValueId")?,
            },
            21 => OperationKind::IntegerExactCast {
                operand: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            22 => OperationKind::ExactIntegerShiftRight {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            23 => OperationKind::ExactIntegerShiftLeft {
                value: reader.id("ValueId")?,
                count: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            24 => OperationKind::ExactIntegerAdd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            25 => OperationKind::ExactIntegerSubtract {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            26 => OperationKind::ExactIntegerMultiply {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            27 => OperationKind::ExactIntegerDivide {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            28 => OperationKind::ExactIntegerRemainder {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            29 => OperationKind::WrappingIntegerDivide {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            30 => OperationKind::WrappingIntegerRemainder {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            31 => OperationKind::SaturatingIntegerDivide {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            32 => OperationKind::SaturatingIntegerRemainder {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                obligation: reader.id("ObligationId")?,
            },
            33 => {
                let callee = reader.id("MachineId")?;
                let argument_count = reader.count()?;
                let mut arguments = Vec::with_capacity(
                    usize::try_from(argument_count).expect("u32 count fits usize"),
                );
                for _ in 0..argument_count {
                    arguments.push(reader.id("ValueId")?);
                }
                let requirement_count = reader.count()?;
                let mut requirement_obligations = Vec::with_capacity(
                    usize::try_from(requirement_count).expect("u32 count fits usize"),
                );
                for _ in 0..requirement_count {
                    requirement_obligations.push(reader.id("ObligationId")?);
                }
                let crash_continuations = decode_crash_routes(reader)?;
                OperationKind::Call {
                    callee,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                }
            }
            34 => OperationKind::CallUnit {
                callee: reader.id("MachineId")?,
                structural_arguments: decode_structural_arguments(reader)?,
                claim_transfers: decode_counted(reader, |reader| {
                    Ok(ClaimTransfer {
                        claim: reader.id("ClaimId")?,
                        argument_index: reader.u32()?,
                    })
                })?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
                crash_continuations: decode_crash_routes(reader)?,
            },
            35 => OperationKind::BoundaryCall {
                boundary: reader.id("BoundaryMachineId")?,
                structural_arguments: decode_structural_arguments(reader)?,
                completion_receipts: decode_counted(reader, |reader| {
                    Ok(CompletionReceipt {
                        claim: reader.id("ClaimId")?,
                        argument_index: reader.u32()?,
                    })
                })?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
            },
            36 => OperationKind::PortWrite {
                service: reader.id("ServiceId")?,
                port: reader.u16()?,
                value: reader.u8()?,
            },
            37 => OperationKind::EstablishTrivialAffineLocal {
                destination: reader.id("PlaceId")?,
            },
            39 => OperationKind::CallStructuralScalar {
                callee: reader.id("MachineId")?,
                structural_arguments: decode_structural_arguments(reader)?,
                claim_transfers: decode_counted(reader, |reader| {
                    Ok(ClaimTransfer {
                        claim: reader.id("ClaimId")?,
                        argument_index: reader.u32()?,
                    })
                })?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
                crash_continuations: decode_crash_routes(reader)?,
            },
            tag => return Err(CodecError::InvalidTag("OperationKind", tag)),
        };
        operations.push(Operation {
            id: operation_id,
            result,
            kind,
        });
    }
    let terminator = match reader.u8()? {
        1 => {
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
                trivial_affine_discards: decode_counted(reader, |reader| reader.id("PlaceId"))?,
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
        tag => return Err(CodecError::InvalidTag("Terminator", tag)),
    };
    Ok(Block {
        id,
        parameters,
        operations,
        terminator,
    })
}
