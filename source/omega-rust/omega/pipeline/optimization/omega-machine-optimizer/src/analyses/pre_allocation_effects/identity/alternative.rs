use omega_selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineSizeKnowledge,
};

use super::values::{encode_u16s, encode_units};

pub(super) fn encode_alternative(bytes: &mut Vec<u8>, alternative: &MachineAlternative) {
    bytes.push(match alternative.key.family {
        MachineAlternativeFamily::CompareI64Zero => 0,
        MachineAlternativeFamily::MaterializeI64 => 1,
        MachineAlternativeFamily::CopyI64 => 2,
        MachineAlternativeFamily::ExactAddI64 => 3,
        MachineAlternativeFamily::ExactAddI64Immediate => 4,
        MachineAlternativeFamily::ExactSubtractI64 => 5,
        MachineAlternativeFamily::ConditionalBranchNonZero => 6,
        MachineAlternativeFamily::ReturnI64 => 7,
        MachineAlternativeFamily::ExactSubtractI64Immediate => 8,
        MachineAlternativeFamily::ReturnUnit => 9,
        MachineAlternativeFamily::CompareI64 => 10,
        MachineAlternativeFamily::ConditionalBranchU64LessThan => 11,
    });
    bytes.extend_from_slice(&alternative.key.variant.to_le_bytes());
    encode_applicability(bytes, alternative.applicability);
    encode_size(bytes, alternative.size);
    bytes.push(0); // latency: StableBaselineUnavailable
    encode_encoded_effects(bytes, &alternative.encoded);
}

fn encode_applicability(bytes: &mut Vec<u8>, applicability: MachineAlternativeApplicability) {
    match applicability {
        MachineAlternativeApplicability::Always => bytes.push(0),
        MachineAlternativeApplicability::ResultAliasesOperand { result, operand } => {
            bytes.push(1);
            bytes.extend_from_slice(&result.to_le_bytes());
            bytes.extend_from_slice(&operand.to_le_bytes());
        }
        MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&result.to_le_bytes());
            bytes.extend_from_slice(&aliased_operand.to_le_bytes());
            bytes.extend_from_slice(&distinct_operand.to_le_bytes());
        }
        MachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&result.to_le_bytes());
            bytes.extend_from_slice(&left.to_le_bytes());
            bytes.extend_from_slice(&right.to_le_bytes());
        }
        MachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&result.to_le_bytes());
            bytes.extend_from_slice(&left.to_le_bytes());
            bytes.extend_from_slice(&right.to_le_bytes());
        }
        MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left,
            right,
            excluded_view,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&left.to_le_bytes());
            bytes.extend_from_slice(&right.to_le_bytes());
            bytes.extend_from_slice(&excluded_view.0.to_le_bytes());
        }
    }
}

fn encode_size(bytes: &mut Vec<u8>, size: MachineSizeKnowledge) {
    match size {
        MachineSizeKnowledge::ExactBytes(size) => {
            bytes.push(0);
            bytes.extend_from_slice(&size.to_le_bytes());
        }
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes,
            maximum_bytes,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&minimum_bytes.to_le_bytes());
            match maximum_bytes {
                None => bytes.push(0),
                Some(maximum) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&maximum.to_le_bytes());
                }
            }
        }
    }
}

fn encode_encoded_effects(bytes: &mut Vec<u8>, effects: &MachineEncodedEffects) {
    encode_u16s(bytes, &effects.external_operand_reads);
    encode_u16s(bytes, &effects.external_operand_writes);
    encode_units(bytes, &effects.implicit_unit_uses);
    encode_units(bytes, &effects.implicit_unit_defs);
    encode_units(bytes, &effects.implicit_unit_clobbers);
    match effects.memory {
        MachineEncodedMemoryEffect::NoneV1 => bytes.push(0),
        MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
            bytes.extend_from_slice(&byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        MachineEncodedStackEffect::UnchangedV1 => bytes.push(0),
        MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
            bytes.extend_from_slice(&byte_count.to_le_bytes());
        }
    }
    bytes.push(match effects.trap {
        MachineEncodedTrapBehavior::NeverV1 => 0,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    match effects.control {
        MachineEncodedControlEffect::FallThroughV1 => bytes.push(0),
        MachineEncodedControlEffect::ConditionalRelativeBranchV1 => bytes.push(1),
        MachineEncodedControlEffect::ReturnFromActivationStackV1 => bytes.push(2),
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            bytes.push(3);
            bytes.extend_from_slice(&target.0.to_le_bytes());
        }
    }
}
