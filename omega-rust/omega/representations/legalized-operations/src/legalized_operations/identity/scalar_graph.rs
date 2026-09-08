use super::calling::{encode_call_plan, encode_placement};
use super::scalar::{
    encode_bindings, encode_definition_site, encode_integer, encode_integer_type,
    encode_scalar_type,
};
use super::shared::*;
use super::structural::{encode_effect, encode_ownership_roster};
pub(super) fn encode(bytes: &mut Vec<u8>, function: &LegalizedScalarFunction) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    encode_option_id(bytes, function.attachment.map(|value| value.get()));
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|value| value.get()),
    );
    encode_call_plan(bytes, &function.call_plan);
    match &function.ranked {
        None => bytes.push(0),
        Some(custody) => match abstract_operations::encode_ranked_u32_countdown_custody(custody) {
            Ok(encoded) => {
                bytes.push(1);
                encode_len(bytes, encoded.len());
                bytes.extend_from_slice(&encoded);
            }
            Err(_) => bytes.push(2),
        },
    }
    match &function.structural {
        Some(signature) => {
            bytes.push(1);
            super::plan::encode_structural_contract(bytes, signature);
        }
        None => bytes.push(0),
    }

    encode_len(bytes, function.parameters.len());
    for parameter in &function.parameters {
        bytes.extend_from_slice(&parameter.value.get().to_le_bytes());
        encode_scalar_type(bytes, parameter.scalar_type);
        encode_definition_site(bytes, parameter.definition_site);
        encode_placement(bytes, &parameter.placement);
    }
    bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
    encode_len(bytes, function.blocks.len());
    for block in &function.blocks {
        bytes.extend_from_slice(&block.id.get().to_le_bytes());
        encode_len(bytes, block.parameters.len());
        for parameter in &block.parameters {
            bytes.extend_from_slice(&parameter.value.get().to_le_bytes());
            encode_scalar_type(bytes, parameter.scalar_type);
            encode_definition_site(bytes, parameter.site);
        }
        encode_len(bytes, block.instructions.len());
        for instruction in &block.instructions {
            bytes.extend_from_slice(&instruction.operation.get().to_le_bytes());
            match instruction.result {
                Some(result) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&result.value.get().to_le_bytes());
                    encode_scalar_type(bytes, result.scalar_type);
                    encode_definition_site(bytes, result.definition_site);
                }
                None => bytes.push(0),
            }
            match &instruction.kind {
                LegalizedScalarInstructionKind::ByteSequenceSubslice {
                    result,
                    source,
                    start,
                    end,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(9);
                    super::projected_structural_call_return::encode_operation_result(bytes, result);
                    for identity in [
                        source.get(),
                        start.get(),
                        end.get(),
                        length.get(),
                        obligation.get(),
                    ] {
                        bytes.extend_from_slice(&identity.to_le_bytes());
                    }
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ByteSequenceRead {
                    source,
                    index,
                    length,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(8);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    bytes.extend_from_slice(&index.get().to_le_bytes());
                    bytes.extend_from_slice(&length.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::ByteSequenceLength {
                    source,
                    length_byte_offset,
                } => {
                    bytes.push(7);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    bytes.extend_from_slice(&length_byte_offset.to_le_bytes());
                }
                LegalizedScalarInstructionKind::BooleanNot { operand } => {
                    bytes.push(4);
                    bytes.extend_from_slice(&operand.get().to_le_bytes());
                }
                LegalizedScalarInstructionKind::IntegerWiden {
                    operand,
                    source_type,
                } => {
                    bytes.push(5);
                    bytes.extend_from_slice(&operand.get().to_le_bytes());
                    encode_integer_type(bytes, *source_type);
                }
                LegalizedScalarInstructionKind::Constant(value) => {
                    bytes.push(0);
                    encode_integer(bytes, *value);
                }
                LegalizedScalarInstructionKind::Call(call) => {
                    bytes.push(1);
                    super::ordinary_calls::encode_call(bytes, call);
                }
                LegalizedScalarInstructionKind::BoundarySettlement(settlement) => {
                    bytes.push(6);
                    super::structural::encode_boundary_settlement(bytes, settlement);
                }
                LegalizedScalarInstructionKind::ExactBinary {
                    operator,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    bytes.push(2);
                    bytes.push(match operator {
                        LegalizedExactIntegerOperator::Add => 0,
                        LegalizedExactIntegerOperator::Subtract => 1,
                    });
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                    bytes.extend_from_slice(&obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&accepted_fact.bytes());
                }
                LegalizedScalarInstructionKind::Compare {
                    predicate,
                    operand_type,
                    left,
                    right,
                } => {
                    bytes.push(3);
                    bytes.push(match predicate {
                        LegalizedScalarComparison::Equal => 0,
                        LegalizedScalarComparison::LessThan => 1,
                        LegalizedScalarComparison::LessOrEqual => 2,
                    });
                    encode_integer_type(bytes, *operand_type);
                    bytes.extend_from_slice(&left.get().to_le_bytes());
                    bytes.extend_from_slice(&right.get().to_le_bytes());
                }
            }
            encode_fuel(bytes, &instruction.fuel);
            encode_effect(bytes, instruction.effect);
            encode_ownership_roster(bytes, &instruction.ownership);
        }
        encode_terminator(bytes, &block.terminator);
    }
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &LegalizedScalarSuccessor) {
    bytes.extend_from_slice(&successor.edge.get().to_le_bytes());
    bytes.extend_from_slice(&successor.target.get().to_le_bytes());
    encode_bindings(bytes, &successor.bindings);
    encode_fuel(bytes, &successor.fuel);
}

fn encode_terminator(bytes: &mut Vec<u8>, terminator: &LegalizedScalarTerminator) {
    match terminator {
        LegalizedScalarTerminator::Return(returned) => {
            bytes.push(0);
            bytes.extend_from_slice(&returned.edge.get().to_le_bytes());
            match returned.value {
                LegalizedScalarReturnValue::Unit => bytes.push(0),
                LegalizedScalarReturnValue::Value { value, scalar_type } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&value.get().to_le_bytes());
                    encode_integer_type(bytes, scalar_type);
                }
            }
            encode_fuel(bytes, &returned.fuel);
            encode_effect(bytes, returned.effect);
            encode_ownership_roster(bytes, &returned.ownership);
        }
        LegalizedScalarTerminator::Jump {
            successor,
            effect,
            ownership,
        } => {
            bytes.push(1);
            encode_successor(bytes, successor);
            encode_effect(bytes, *effect);
            encode_ownership_roster(bytes, ownership);
        }
        LegalizedScalarTerminator::Conditional {
            condition,
            when_true,
            when_false,
            effect,
            ownership,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&condition.get().to_le_bytes());
            encode_successor(bytes, when_true);
            encode_successor(bytes, when_false);
            encode_effect(bytes, *effect);
            encode_ownership_roster(bytes, ownership);
        }
    }
}
