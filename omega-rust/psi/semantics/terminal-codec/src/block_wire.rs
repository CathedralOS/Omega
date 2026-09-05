//! Canonical terminal block, operation, and terminator wire format.
//!
//! This module owns operation-result and operation-kind rows plus the exact
//! terminal control-flow envelope. Shared structural paths, call arguments,
//! contracts, and declaration primitives remain sibling- or parent-owned.

use terminal_psi::{
    Block, ClaimTransfer, CompletionReceipt, CrashCause, NominalAffineCleanup, Operation,
    OperationKind, OperationResult, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificGuard, StructuralAffineDiscard, StructuralCaseSuccessorEdge,
    StructuralResultClaimTransfer, Terminator,
};

use super::contract_wire::{
    decode_crash_predicate, decode_crash_routes, decode_successor_edge, encode_crash_predicate,
    encode_crash_routes, encode_successor_edge,
};
use super::machine_wire::{
    decode_declaration, decode_declarations, encode_declaration, encode_declarations,
};
use super::proof_declaration_wire::{decode_evidence_interface, encode_evidence_interface};
use super::scalar_wire::{
    decode_ieee_float_value, decode_integer_value, encode_ieee_float_value, encode_integer_value,
};
use super::structural_result_wire::{
    ResultPathWireFormat, decode_operation_result, encode_operation_result,
};
use super::wire::{Reader, Writer};
use super::{
    CodecError, decode_affine_cleanup_action, decode_counted, decode_ids, decode_optional_id,
    decode_structural_arguments, decode_structural_path, encode_affine_cleanup_action,
    encode_obligation_ids, encode_optional_id, encode_structural_arguments, encode_structural_path,
};

#[cfg(test)]
pub(super) fn encode_block(writer: &mut Writer, block: &Block) -> Result<(), CodecError> {
    encode_block_for_result_paths(writer, block, ResultPathWireFormat::Current)
}

pub(super) fn encode_block_for_result_paths(
    writer: &mut Writer,
    block: &Block,
    result_path_format: ResultPathWireFormat,
) -> Result<(), CodecError> {
    writer.id(block.id);
    encode_declarations(writer, "block parameters", &block.parameters)?;
    writer.len("operations", block.operations.len())?;
    for operation in &block.operations {
        writer.id(operation.id);
        match &operation.result {
            OperationResult::Unit => writer.u8(0),
            OperationResult::Scalar(result) => {
                writer.u8(1);
                encode_declaration(writer, *result);
            }
            OperationResult::Structural(result) => {
                writer.u8(2);
                encode_operation_result(writer, result, result_path_format)?;
            }
        }
        match operation.kind.clone() {
            OperationKind::WriteOnlyPrimitiveStore { destination, value } => {
                writer.u8(43);
                writer.id(destination);
                writer.id(value);
            }
            OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
            } => {
                writer.u8(46);
                writer.id(destination);
                encode_structural_path(writer, "structural scalar field store path", &path)?;
                writer.id(field);
                writer.id(value);
            }
            OperationKind::EstablishPayloadlessCase { result_case } => {
                writer.u8(42);
                writer.id(result_case);
            }
            OperationKind::EstablishByteSequenceLiteral { destination, bytes } => {
                writer.u8(40);
                writer.id(destination);
                writer.len("byte-sequence literal bytes", bytes.len())?;
                writer.bytes(&bytes);
            }
            OperationKind::EstablishTrivialAffineLocal { destination } => {
                writer.u8(37);
                writer.id(destination);
            }
            OperationKind::EstablishAffineScalarRecord { field, value } => {
                writer.u8(51);
                writer.id(field);
                encode_integer_value(writer, value);
            }
            OperationKind::StoreDynamicDescriptor { descriptor_ordinal } => {
                writer.u8(54);
                writer.u32(descriptor_ordinal);
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
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(34);
                writer.id(callee);
                writer.len("unit-call scalar arguments", arguments.len())?;
                for argument in arguments {
                    writer.id(argument);
                }
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
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(39);
                writer.id(callee);
                writer.len("structural-scalar-call scalar arguments", arguments.len())?;
                for argument in arguments {
                    writer.id(argument);
                }
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
            OperationKind::CallDynamicScalar {
                descriptor_ordinal,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(48);
                writer.u32(descriptor_ordinal);
                encode_obligation_ids(writer, &requirement_obligations)?;
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::CallDynamicParameterScalar {
                parameter_ordinal,
                requirement_slot,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(49);
                writer.u32(parameter_ordinal);
                writer.u32(requirement_slot);
                encode_obligation_ids(writer, &requirement_obligations)?;
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::CallDynamicUnit {
                descriptor_ordinal,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(52);
                writer.u32(descriptor_ordinal);
                encode_obligation_ids(writer, &requirement_obligations)?;
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::CallDynamicParameterUnit {
                parameter_ordinal,
                requirement_slot,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(53);
                writer.u32(parameter_ordinal);
                writer.u32(requirement_slot);
                encode_obligation_ids(writer, &requirement_obligations)?;
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::CallStructural {
                callee,
                structural_arguments,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
                selected_evidence,
            } => {
                writer.u8(41);
                writer.id(callee);
                encode_structural_arguments(writer, &structural_arguments)?;
                writer.len("structural-call claim transfers", claim_transfers.len())?;
                for transfer in claim_transfers {
                    writer.id(transfer.claim);
                    writer.u32(transfer.argument_index);
                }
                writer.len(
                    "structural-call returned claim transfers",
                    returned_claim_transfers.len(),
                )?;
                for transfer in returned_claim_transfers {
                    writer.id(transfer.callee_claim);
                    writer.id(transfer.caller_claim);
                }
                encode_obligation_ids(writer, &requirement_obligations)?;
                encode_crash_routes(writer, &crash_continuations)?;
                writer.len("guarded call selected evidence", selected_evidence.len())?;
                for binding in selected_evidence {
                    writer.id(binding.guard.result_type);
                    writer.id(binding.guard.result_case);
                    writer.u32(binding.position);
                    writer.id(binding.callee_obligation);
                    writer.id(binding.callee_term);
                    writer.string("guarded call output field", &binding.output_field)?;
                    writer.id(binding.callee_proposition);
                    writer.id(binding.instantiated_proposition);
                    writer.id(binding.output);
                    match binding.result_substitution {
                        None => writer.u8(0),
                        Some(substitution) => {
                            writer.u8(1);
                            writer.u32(substitution.argument_position);
                            writer.id(substitution.callee_result);
                            writer.id(substitution.caller_result);
                        }
                    }
                    writer.id(binding.validity.result);
                    writer.len(
                        "guarded call proposition dependencies",
                        binding.validity.proposition_dependencies.len(),
                    )?;
                    for dependency in &binding.validity.proposition_dependencies {
                        writer.id(*dependency);
                    }
                    encode_evidence_interface(writer, &binding.validity.evidence_interface)?;
                    writer.len(
                        "guarded call interface dependencies",
                        binding.validity.interface_dependencies.len(),
                    )?;
                    for dependency in &binding.validity.interface_dependencies {
                        writer.id(*dependency);
                    }
                    writer.u32(binding.expected_use_count);
                    writer.len("guarded selected evidence uses", binding.uses.len())?;
                    for use_ in &binding.uses {
                        writer.id(use_.target);
                        writer.u32(use_.input_position);
                        writer.id(use_.target_requirement);
                        writer.id(use_.target_term);
                        writer.id(use_.source);
                        writer.id(use_.instantiated_proposition);
                        writer.id(use_.target_parameter);
                        writer.id(use_.caller_result);
                    }
                }
            }
            OperationKind::CallStructuralWithScalarArguments {
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => {
                writer.u8(50);
                writer.id(callee);
                writer.len("mixed structural-call scalar arguments", arguments.len())?;
                for argument in arguments {
                    writer.id(argument);
                }
                encode_structural_arguments(writer, &structural_arguments)?;
                writer.len(
                    "mixed structural-call claim transfers",
                    claim_transfers.len(),
                )?;
                for transfer in claim_transfers {
                    writer.id(transfer.claim);
                    writer.u32(transfer.argument_index);
                }
                writer.len(
                    "mixed structural-call returned claim transfers",
                    returned_claim_transfers.len(),
                )?;
                for transfer in returned_claim_transfers {
                    writer.id(transfer.callee_claim);
                    writer.id(transfer.caller_claim);
                }
                encode_obligation_ids(writer, &requirement_obligations)?;
                encode_crash_routes(writer, &crash_continuations)?;
            }
            OperationKind::BoundaryCall {
                boundary,
                arguments,
                structural_arguments,
                completion_receipts,
            } => {
                writer.u8(35);
                writer.id(boundary);
                writer.len("boundary scalar arguments", arguments.len())?;
                for argument in arguments {
                    writer.id(argument);
                }
                encode_structural_arguments(writer, &structural_arguments)?;
                writer.len("boundary claim settlements", completion_receipts.len())?;
                for settlement in completion_receipts {
                    writer.id(settlement.claim);
                    writer.u32(settlement.argument_index);
                }
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
            OperationKind::IeeeFloatConstant { value } => {
                writer.u8(44);
                encode_ieee_float_value(writer, value);
            }
            OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                left,
                right,
                addend,
            } => {
                writer.u8(45);
                writer.id(left);
                writer.id(right);
                writer.id(addend);
            }
            OperationKind::BooleanStructuralField { source, field } => {
                writer.u8(38);
                writer.id(source);
                writer.id(field);
            }
            OperationKind::IntegerStructuralField { source, field } => {
                writer.u8(47);
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

#[cfg(test)]
pub(super) fn decode_block(reader: &mut Reader<'_>) -> Result<Block, CodecError> {
    decode_block_for_result_paths(reader, ResultPathWireFormat::Current)
}

pub(super) fn decode_block_for_result_paths(
    reader: &mut Reader<'_>,
    result_path_format: ResultPathWireFormat,
) -> Result<Block, CodecError> {
    let id = reader.id("BlockId")?;
    let parameters = decode_declarations(reader)?;
    let operation_count = reader.count()?;
    let mut operations = Vec::new();
    for _ in 0..operation_count {
        let operation_id = reader.id("OperationId")?;
        let result = match reader.u8()? {
            0 => OperationResult::Unit,
            1 => OperationResult::Scalar(decode_declaration(reader)?),
            2 => OperationResult::Structural(decode_operation_result(reader, result_path_format)?),
            tag => return Err(CodecError::InvalidTag("OperationResult", tag)),
        };
        let kind = match reader.u8()? {
            43 => OperationKind::WriteOnlyPrimitiveStore {
                destination: reader.id("PlaceId")?,
                value: reader.id("ValueId")?,
            },
            46 => OperationKind::StructuralScalarFieldStore {
                destination: reader.id("PlaceId")?,
                path: decode_structural_path(reader)?,
                field: reader.id("StructuralFieldId")?,
                value: reader.id("ValueId")?,
            },
            42 => OperationKind::EstablishPayloadlessCase {
                result_case: reader.id("StructuralCaseId")?,
            },
            40 => OperationKind::EstablishByteSequenceLiteral {
                destination: reader.id("PlaceId")?,
                bytes: {
                    let len =
                        usize::try_from(reader.count()?).map_err(|_| CodecError::UnexpectedEnd)?;
                    reader.take(len)?.to_vec()
                },
            },
            1 => OperationKind::IntegerConstant {
                value: decode_integer_value(reader)?,
            },
            2 => OperationKind::BooleanConstant {
                value: reader.boolean()?,
            },
            44 => OperationKind::IeeeFloatConstant {
                value: decode_ieee_float_value(reader)?,
            },
            45 => OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                left: reader.id("ValueId")?,
                right: reader.id("ValueId")?,
                addend: reader.id("ValueId")?,
            },
            38 => OperationKind::BooleanStructuralField {
                source: reader.id("PlaceId")?,
                field: reader.id("StructuralFieldId")?,
            },
            47 => OperationKind::IntegerStructuralField {
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
                arguments: decode_ids(reader, "ValueId")?,
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
                arguments: decode_ids(reader, "ValueId")?,
                structural_arguments: decode_structural_arguments(reader)?,
                completion_receipts: decode_counted(reader, |reader| {
                    Ok(CompletionReceipt {
                        claim: reader.id("ClaimId")?,
                        argument_index: reader.u32()?,
                    })
                })?,
            },
            36 => OperationKind::PortWrite {
                service: reader.id("ServiceId")?,
                port: reader.u16()?,
                value: reader.u8()?,
            },
            37 => OperationKind::EstablishTrivialAffineLocal {
                destination: reader.id("PlaceId")?,
            },
            51 => OperationKind::EstablishAffineScalarRecord {
                field: reader.id("StructuralFieldId")?,
                value: decode_integer_value(reader)?,
            },
            39 => OperationKind::CallStructuralScalar {
                callee: reader.id("MachineId")?,
                arguments: decode_ids(reader, "ValueId")?,
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
            48 => OperationKind::CallDynamicScalar {
                descriptor_ordinal: reader.u32()?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
                crash_continuations: decode_crash_routes(reader)?,
            },
            49 => OperationKind::CallDynamicParameterScalar {
                parameter_ordinal: reader.u32()?,
                requirement_slot: reader.u32()?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
                crash_continuations: decode_crash_routes(reader)?,
            },
            52 => OperationKind::CallDynamicUnit {
                descriptor_ordinal: reader.u32()?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
                crash_continuations: decode_crash_routes(reader)?,
            },
            53 => OperationKind::CallDynamicParameterUnit {
                parameter_ordinal: reader.u32()?,
                requirement_slot: reader.u32()?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
                crash_continuations: decode_crash_routes(reader)?,
            },
            54 => OperationKind::StoreDynamicDescriptor {
                descriptor_ordinal: reader.u32()?,
            },
            41 => OperationKind::CallStructural {
                callee: reader.id("MachineId")?,
                structural_arguments: decode_structural_arguments(reader)?,
                claim_transfers: decode_counted(reader, |reader| {
                    Ok(ClaimTransfer {
                        claim: reader.id("ClaimId")?,
                        argument_index: reader.u32()?,
                    })
                })?,
                returned_claim_transfers: decode_counted(reader, |reader| {
                    Ok(StructuralResultClaimTransfer {
                        callee_claim: reader.id("ClaimId")?,
                        caller_claim: reader.id("ClaimId")?,
                    })
                })?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
                crash_continuations: decode_crash_routes(reader)?,
                selected_evidence: decode_counted(reader, |reader| {
                    Ok(OutcomeSpecificCallEvidence {
                        guard: OutcomeSpecificGuard {
                            result_type: reader.id("StructuralTypeId")?,
                            result_case: reader.id("StructuralCaseId")?,
                        },
                        position: reader.u32()?,
                        callee_obligation: reader.id("ObligationId")?,
                        callee_term: reader.id("EvidenceTermId")?,
                        output_field: reader.string("guarded call output field")?,
                        callee_proposition: reader.id("PropositionId")?,
                        instantiated_proposition: reader.id("PropositionId")?,
                        output: reader.id("EvidenceTermId")?,
                        result_substitution: match reader.u8()? {
                            0 => None,
                            1 => Some(OutcomeSpecificCallResultSubstitution {
                                argument_position: reader.u32()?,
                                callee_result: reader.id("PlaceId")?,
                                caller_result: reader.id("PlaceId")?,
                            }),
                            tag => {
                                return Err(CodecError::InvalidTag(
                                    "OutcomeSpecificCallResultSubstitution",
                                    tag,
                                ));
                            }
                        },
                        validity: OutcomeSpecificCallEvidenceValidity {
                            result: reader.id("PlaceId")?,
                            proposition_dependencies: decode_ids(reader, "PlaceId")?,
                            evidence_interface: decode_evidence_interface(reader)?,
                            interface_dependencies: decode_ids(reader, "PlaceId")?,
                        },
                        expected_use_count: reader.u32()?,
                        uses: decode_counted(reader, |reader| {
                            Ok(terminal_psi::OutcomeSpecificEvidenceUse {
                                target: reader.id("MachineId")?,
                                input_position: reader.u32()?,
                                target_requirement: reader.id("PropositionId")?,
                                target_term: reader.id("EvidenceTermId")?,
                                source: reader.id("EvidenceTermId")?,
                                instantiated_proposition: reader.id("PropositionId")?,
                                target_parameter: reader.id("PlaceId")?,
                                caller_result: reader.id("PlaceId")?,
                            })
                        })?,
                    })
                })?,
            },
            50 => OperationKind::CallStructuralWithScalarArguments {
                callee: reader.id("MachineId")?,
                arguments: decode_ids(reader, "ValueId")?,
                structural_arguments: decode_structural_arguments(reader)?,
                claim_transfers: decode_counted(reader, |reader| {
                    Ok(ClaimTransfer {
                        claim: reader.id("ClaimId")?,
                        argument_index: reader.u32()?,
                    })
                })?,
                returned_claim_transfers: decode_counted(reader, |reader| {
                    Ok(StructuralResultClaimTransfer {
                        callee_claim: reader.id("ClaimId")?,
                        caller_claim: reader.id("ClaimId")?,
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
    };
    Ok(Block {
        id,
        parameters,
        operations,
        terminator,
    })
}

#[cfg(test)]
mod tests {
    use semantic_vocabulary::{
        BlockId, ClaimId, EdgeId, EvidenceTermId, IntegerSign, IntegerType, MachineId,
        ObligationId, OperationId, PlaceId, PropositionId, ScalarType, StructuralCaseId,
        StructuralFieldId, StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        Block, EvidenceInterfaceIdentity, Operation, OperationKind, OperationResult,
        OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity, OutcomeSpecificGuard,
        StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
        StructuralPathSegment, StructuralResultClaimBinding, StructuralResultClaimTransfer,
        Terminator, ValueDeclaration,
    };

    use super::{decode_block, encode_block};
    use crate::{
        CodecError,
        wire::{Reader, Writer},
    };

    fn id<T: semantic_vocabulary::PsiSemanticId>(raw: u64) -> T {
        T::new(raw).expect("test ids are nonzero")
    }

    fn structural_call_block() -> Block {
        Block {
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: id::<OperationId>(1),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: id::<PlaceId>(2),
                    structural_type: id::<StructuralTypeId>(3),
                    multiplicity: StructuralMultiplicity::Linear,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: vec![StructuralResultClaimBinding {
                        claim: id::<ClaimId>(4),
                        path: Vec::new(),
                    }],
                }),
                kind: OperationKind::CallStructural {
                    callee: id::<MachineId>(5),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    returned_claim_transfers: vec![StructuralResultClaimTransfer {
                        callee_claim: id::<ClaimId>(6),
                        caller_claim: id::<ClaimId>(4),
                    }],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    selected_evidence: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(7),
                trivial_affine_discards: Vec::new(),
            },
        }
    }

    #[test]
    fn write_only_primitive_store_uses_exact_stable_wire_fields() {
        let block = Block {
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: id::<OperationId>(2),
                result: OperationResult::Unit,
                kind: OperationKind::WriteOnlyPrimitiveStore {
                    destination: id::<PlaceId>(3),
                    value: id(4),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(5),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).expect("write-only primitive store block encodes");
        let bytes = writer.finish();
        assert_eq!(bytes[24], 0, "Unit OperationResult wire tag");
        assert_eq!(bytes[25], 43, "WriteOnlyPrimitiveStore wire tag");
        assert_eq!(
            &bytes[26..34],
            &id::<PlaceId>(3).get().to_le_bytes(),
            "destination is the first exact operation field",
        );
        assert_eq!(
            &bytes[34..42],
            &id::<semantic_vocabulary::ValueId>(4).get().to_le_bytes(),
            "source value is the second exact operation field",
        );
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_block(&mut reader), Ok(block));
        assert_eq!(reader.remaining(), 0);

        let mut invalid = bytes;
        invalid[25] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&invalid)),
            Err(CodecError::InvalidTag("OperationKind", 255)),
        );
    }

    #[test]
    fn structural_scalar_field_operations_use_exact_stable_wire_fields() {
        let store = Block {
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: id::<OperationId>(2),
                result: OperationResult::Unit,
                kind: OperationKind::StructuralScalarFieldStore {
                    destination: id::<PlaceId>(3),
                    path: vec![StructuralPathSegment::Field("item".into())],
                    field: id::<StructuralFieldId>(4),
                    value: id::<ValueId>(5),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(6),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &store).expect("structural scalar-field store encodes");
        let bytes = writer.finish();
        assert_eq!(bytes[25], 46, "StructuralScalarFieldStore wire tag");
        assert_eq!(&bytes[26..34], &id::<PlaceId>(3).get().to_le_bytes());
        assert_eq!(&bytes[34..38], &1_u32.to_le_bytes());
        assert_eq!(bytes[38], 1, "Field structural-path segment wire tag");
        assert_eq!(
            &bytes[47..55],
            &id::<StructuralFieldId>(4).get().to_le_bytes()
        );
        assert_eq!(&bytes[55..63], &id::<ValueId>(5).get().to_le_bytes());
        assert_eq!(decode_block(&mut Reader::new(&bytes)), Ok(store));

        let mut invalid_path = bytes;
        invalid_path[38] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&invalid_path)),
            Err(CodecError::InvalidTag("StructuralPathSegment", 255)),
        );

        let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        let read = Block {
            id: id::<BlockId>(7),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: id::<OperationId>(8),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: id::<ValueId>(9),
                    scalar_type: integer,
                }),
                kind: OperationKind::IntegerStructuralField {
                    source: id::<PlaceId>(10),
                    field: id::<StructuralFieldId>(11),
                },
            }],
            terminator: Terminator::Return {
                edge: id::<EdgeId>(12),
                value: id::<ValueId>(9),
                cleanup_actions: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &read).expect("integer structural field read encodes");
        let bytes = writer.finish();
        let kind = bytes
            .iter()
            .position(|byte| *byte == 47)
            .expect("IntegerStructuralField wire tag");
        assert_eq!(
            &bytes[kind + 1..kind + 9],
            &id::<PlaceId>(10).get().to_le_bytes()
        );
        assert_eq!(
            &bytes[kind + 9..kind + 17],
            &id::<StructuralFieldId>(11).get().to_le_bytes(),
        );
        assert_eq!(decode_block(&mut Reader::new(&bytes)), Ok(read));
    }

    #[test]
    fn structural_scalar_call_round_trips_scalar_arguments() {
        let block = Block {
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: id::<OperationId>(2),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: id::<ValueId>(3),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::CallStructuralScalar {
                    callee: id::<MachineId>(4),
                    arguments: vec![id::<ValueId>(5)],
                    structural_arguments: vec![StructuralArgument {
                        place: id::<PlaceId>(6),
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(7),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).expect("mixed structural scalar call encodes");
        let bytes = writer.finish();
        let decoded = decode_block(&mut Reader::new(&bytes)).expect("mixed call decodes");
        let OperationKind::CallStructuralScalar { arguments, .. } = &decoded.operations[0].kind
        else {
            unreachable!()
        };
        assert_eq!(arguments, &[id::<ValueId>(5)]);
        assert_eq!(decoded, block);
    }

    #[test]
    fn structural_operation_result_and_call_use_stable_wire_tags() {
        let block = structural_call_block();
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).expect("structural call block encodes");
        let bytes = writer.finish();

        // Block id + parameter count + operation count + operation id.
        assert_eq!(bytes[24], 2, "structural OperationResult wire tag");
        // The fixture has no qualifications and one whole-root claim, so the
        // operation-kind tag follows its fixed-width result metadata here.
        assert_eq!(bytes[66], 41, "CallStructural wire tag");

        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_block(&mut reader), Ok(block));
        assert_eq!(reader.remaining(), 0);

        let mut invalid_result = bytes.clone();
        invalid_result[24] = 3;
        assert_eq!(
            decode_block(&mut Reader::new(&invalid_result)),
            Err(CodecError::InvalidTag("OperationResult", 3))
        );

        let mut invalid_call = bytes;
        invalid_call[66] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&invalid_call)),
            Err(CodecError::InvalidTag("OperationKind", 255))
        );
    }

    #[test]
    fn guarded_structural_call_selection_round_trips_exact_validity_carrier() {
        let mut block = structural_call_block();
        let OperationKind::CallStructural {
            selected_evidence, ..
        } = &mut block.operations[0].kind
        else {
            unreachable!()
        };
        selected_evidence.push(OutcomeSpecificCallEvidence {
            guard: OutcomeSpecificGuard {
                result_type: id::<StructuralTypeId>(3),
                result_case: id::<StructuralCaseId>(8),
            },
            position: 0,
            callee_obligation: id::<ObligationId>(9),
            callee_term: id::<EvidenceTermId>(10),
            output_field: "selected".into(),
            callee_proposition: id::<PropositionId>(11),
            instantiated_proposition: id::<PropositionId>(11),
            output: id::<EvidenceTermId>(12),
            result_substitution: None,
            validity: OutcomeSpecificCallEvidenceValidity {
                result: id::<PlaceId>(2),
                proposition_dependencies: vec![id::<PlaceId>(2)],
                evidence_interface: EvidenceInterfaceIdentity {
                    trait_identity: "ReadyEvidence".into(),
                    arguments: vec!["Outcome".into()],
                    requirements: Vec::new(),
                },
                interface_dependencies: Vec::new(),
            },
            expected_use_count: 0,
            uses: Vec::new(),
        });
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).expect("guarded call selection encodes");
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_block(&mut reader), Ok(block));
        assert_eq!(reader.remaining(), 0);

        let mut truncated = bytes;
        truncated.pop();
        assert!(decode_block(&mut Reader::new(&truncated)).is_err());
    }
}
