use omega_register_model::{RegisterOperandAccess, RegisterUnitId, RegisterWriteSemantics};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternative, TerminalMachineAlternativeApplicability,
    TerminalMachineAlternativeFamily, TerminalMachineEncodedControlEffect,
    TerminalMachineEncodedEffects, TerminalMachineEncodedMemoryEffect,
    TerminalMachineEncodedStackEffect, TerminalMachineEncodedTrapBehavior,
    TerminalMachineLatencyKnowledge, TerminalMachineSizeKnowledge,
};

use crate::{TerminalPostAllocationMachineIdentity, TerminalPostAllocationMachinePlan};

pub fn terminal_post_allocation_machine_identity(
    plan: &TerminalPostAllocationMachinePlan,
) -> TerminalPostAllocationMachineIdentity {
    use sha2::{Digest, Sha256};

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-postallocation-machine.v2\0");
    bytes.extend_from_slice(&encode_terminal_post_allocation_machine_content(plan));
    TerminalPostAllocationMachineIdentity::from_bytes(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_post_allocation_machine_content(
    plan: &TerminalPostAllocationMachinePlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.effects.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.homes.bytes());
    bytes.extend_from_slice(&plan.post_allocation_manifest.bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.physical_register_model.bytes());
    bytes.extend_from_slice(&plan.register_constraints.bytes());
    bytes.extend_from_slice(&plan.machine_effect_catalog.bytes());
    bytes.push(match plan.choice_rule {
        crate::TerminalMachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1 => 0,
    });
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.blocks.len());
        for block in &function.blocks {
            bytes.extend_from_slice(&block.block.0.to_le_bytes());
            encode_len(&mut bytes, block.instructions.len());
            for instruction in &block.instructions {
                bytes.extend_from_slice(&instruction.instruction.0.to_le_bytes());
                encode_alternative(&mut bytes, &instruction.alternative);
                encode_len(&mut bytes, instruction.operands.len());
                for operand in &instruction.operands {
                    bytes.extend_from_slice(&operand.operand.to_le_bytes());
                    bytes.extend_from_slice(&operand.virtual_register.0.to_le_bytes());
                    bytes.extend_from_slice(&operand.class.0.to_le_bytes());
                    bytes.extend_from_slice(&operand.view.0.to_le_bytes());
                    bytes.push(match operand.access {
                        RegisterOperandAccess::Use => 0,
                        RegisterOperandAccess::Def => 1,
                        RegisterOperandAccess::UseDef => 2,
                    });
                    encode_units(&mut bytes, &operand.storage_units);
                    encode_units(&mut bytes, &operand.read_units);
                    encode_units(&mut bytes, &operand.write_units);
                    match operand.write_semantics {
                        None => bytes.push(0),
                        Some(semantics) => {
                            bytes.push(1);
                            bytes.push(match semantics {
                                RegisterWriteSemantics::ExactView => 0,
                                RegisterWriteSemantics::PreservesUnwritten => 1,
                                RegisterWriteSemantics::ZeroExtendsParent => 2,
                                RegisterWriteSemantics::ZeroExtendsWithinUnit => 3,
                                RegisterWriteSemantics::Discards => 4,
                                RegisterWriteSemantics::InstructionDefined => 5,
                            });
                        }
                    }
                }
                encode_units(&mut bytes, &instruction.implicit_unit_uses);
                encode_units(&mut bytes, &instruction.implicit_unit_defs);
                encode_units(&mut bytes, &instruction.implicit_unit_clobbers);
                encode_units(&mut bytes, &instruction.unit_uses);
                encode_units(&mut bytes, &instruction.unit_defs);
                encode_units(&mut bytes, &instruction.unit_clobbers);
            }
        }
    }
    bytes
}

fn encode_alternative(bytes: &mut Vec<u8>, alternative: &TerminalMachineAlternative) {
    bytes.push(match alternative.key.family {
        TerminalMachineAlternativeFamily::CompareI64Zero => 0,
        TerminalMachineAlternativeFamily::MaterializeI64 => 1,
        TerminalMachineAlternativeFamily::CopyI64 => 2,
        TerminalMachineAlternativeFamily::ExactAddI64 => 3,
        TerminalMachineAlternativeFamily::ExactAddI64Immediate => 4,
        TerminalMachineAlternativeFamily::ExactSubtractI64 => 5,
        TerminalMachineAlternativeFamily::ConditionalBranchNonZero => 6,
        TerminalMachineAlternativeFamily::ReturnI64 => 7,
        TerminalMachineAlternativeFamily::ExactSubtractI64Immediate => 8,
    });
    bytes.extend_from_slice(&alternative.key.variant.to_le_bytes());
    match alternative.applicability {
        TerminalMachineAlternativeApplicability::Always => bytes.push(0),
        TerminalMachineAlternativeApplicability::ResultAliasesOperand { result, operand } => {
            bytes.push(1);
            bytes.extend_from_slice(&result.to_le_bytes());
            bytes.extend_from_slice(&operand.to_le_bytes());
        }
        TerminalMachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&result.to_le_bytes());
            bytes.extend_from_slice(&aliased_operand.to_le_bytes());
            bytes.extend_from_slice(&distinct_operand.to_le_bytes());
        }
        TerminalMachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&result.to_le_bytes());
            bytes.extend_from_slice(&left.to_le_bytes());
            bytes.extend_from_slice(&right.to_le_bytes());
        }
        TerminalMachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&result.to_le_bytes());
            bytes.extend_from_slice(&left.to_le_bytes());
            bytes.extend_from_slice(&right.to_le_bytes());
        }
        TerminalMachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
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
    match alternative.size {
        TerminalMachineSizeKnowledge::ExactBytes(size) => {
            bytes.push(0);
            bytes.extend_from_slice(&size.to_le_bytes());
        }
        TerminalMachineSizeKnowledge::EncoderResolved {
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
    bytes.push(match alternative.latency {
        TerminalMachineLatencyKnowledge::StableBaselineUnavailable => 0,
    });
    encode_encoded_effects(bytes, &alternative.encoded);
}

fn encode_encoded_effects(bytes: &mut Vec<u8>, effects: &TerminalMachineEncodedEffects) {
    encode_u16s(bytes, &effects.external_operand_reads);
    encode_u16s(bytes, &effects.external_operand_writes);
    encode_units(bytes, &effects.implicit_unit_uses);
    encode_units(bytes, &effects.implicit_unit_defs);
    encode_units(bytes, &effects.implicit_unit_clobbers);
    match effects.memory {
        TerminalMachineEncodedMemoryEffect::NoneV1 => bytes.push(0),
        TerminalMachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
            bytes.extend_from_slice(&byte_count.to_le_bytes());
        }
    }
    match effects.stack {
        TerminalMachineEncodedStackEffect::UnchangedV1 => bytes.push(0),
        TerminalMachineEncodedStackEffect::PopBytesV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
            bytes.extend_from_slice(&byte_count.to_le_bytes());
        }
    }
    bytes.push(match effects.trap {
        TerminalMachineEncodedTrapBehavior::NeverV1 => 0,
        TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    match effects.control {
        TerminalMachineEncodedControlEffect::FallThroughV1 => bytes.push(0),
        TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1 => bytes.push(1),
        TerminalMachineEncodedControlEffect::ReturnFromActivationStackV1 => bytes.push(2),
        TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 { target } => {
            bytes.push(3);
            bytes.extend_from_slice(&target.0.to_le_bytes());
        }
    }
}

fn encode_u16s(bytes: &mut Vec<u8>, values: &[u16]) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 0,
        ObjectFormat::MachO => 1,
        ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_size)
            .expect("supported pointer size fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_alignment)
            .expect("supported pointer alignment fits u64")
            .to_le_bytes(),
    );
}

fn encode_units(bytes: &mut Vec<u8>, units: &[RegisterUnitId]) {
    encode_len(bytes, units.len());
    for unit in units {
        bytes.extend_from_slice(&unit.0.to_le_bytes());
    }
}

fn encode_len(bytes: &mut Vec<u8>, length: usize) {
    bytes.extend_from_slice(
        &u64::try_from(length)
            .expect("in-memory artifact length fits u64")
            .to_le_bytes(),
    );
}
