use omega_register_model::{RegisterOperandAccess, RegisterUnitId, RegisterWriteSemantics};
use omega_selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineSizeKnowledge,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

use crate::PostAllocationMachinePlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostAllocationMachineIdentity([u8; 32]);

impl PostAllocationMachineIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

pub fn post_allocation_machine_identity(
    plan: &PostAllocationMachinePlan,
) -> PostAllocationMachineIdentity {
    post_allocation_machine_identity_with_domain(
        plan,
        b"omega.terminal-postallocation-machine.v6\0",
    )
}

pub(crate) fn post_allocation_machine_identity_v5_legacy(
    plan: &PostAllocationMachinePlan,
) -> PostAllocationMachineIdentity {
    post_allocation_machine_identity_with_domain(
        plan,
        b"omega.terminal-postallocation-machine.v5\0",
    )
}

pub(crate) fn post_allocation_machine_identity_v4_legacy(
    plan: &PostAllocationMachinePlan,
) -> PostAllocationMachineIdentity {
    post_allocation_machine_identity_with_domain(
        plan,
        b"omega.terminal-postallocation-machine.v4\0",
    )
}

fn post_allocation_machine_identity_with_domain(
    plan: &PostAllocationMachinePlan,
    domain: &[u8],
) -> PostAllocationMachineIdentity {
    use sha2::{Digest, Sha256};

    let mut bytes = Vec::new();
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encode_terminal_post_allocation_machine_content(plan));
    PostAllocationMachineIdentity::from_bytes(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_post_allocation_machine_content(
    plan: &PostAllocationMachinePlan,
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
        crate::MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1 => 0,
    });
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.blocks.len());
        for block in &function.blocks {
            bytes.extend_from_slice(&block.block.0.to_le_bytes());
            encode_len(&mut bytes, block.instructions.len());
            for instruction in &block.instructions {
                encode_instruction(&mut bytes, instruction);
            }
        }
    }
    encode_len(&mut bytes, plan.structural_unit_functions.len());
    for function in &plan.structural_unit_functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.extend_from_slice(&function.block.0.to_le_bytes());
        match &function.call {
            None => bytes.push(0),
            Some(call) => {
                bytes.push(1);
                omega_selected_instructions::selected_instructions::effects::program::identity::encode_structural_call(
                    &mut bytes, call,
                );
            }
        }
        encode_instruction(&mut bytes, &function.return_instruction);
        omega_selected_instructions::selected_instructions::effects::program::identity::encode_provenance(
            &mut bytes,
            &function.return_provenance,
        );
        omega_selected_instructions::selected_instructions::effects::program::identity::encode_effect_link(
            &mut bytes,
            function.return_effect,
        );
        omega_selected_instructions::selected_instructions::effects::program::identity::encode_ownership(
            &mut bytes,
            &function.return_ownership,
        );
    }
    bytes
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: &crate::PostAllocationMachineInstruction) {
    bytes.extend_from_slice(&instruction.instruction.0.to_le_bytes());
    encode_alternative(bytes, &instruction.alternative);
    encode_len(bytes, instruction.operands.len());
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
        encode_units(bytes, &operand.storage_units);
        encode_units(bytes, &operand.read_units);
        encode_units(bytes, &operand.write_units);
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
    encode_units(bytes, &instruction.implicit_unit_uses);
    encode_units(bytes, &instruction.implicit_unit_defs);
    encode_units(bytes, &instruction.implicit_unit_clobbers);
    encode_units(bytes, &instruction.unit_uses);
    encode_units(bytes, &instruction.unit_defs);
    encode_units(bytes, &instruction.unit_clobbers);
}

fn encode_alternative(bytes: &mut Vec<u8>, alternative: &MachineAlternative) {
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
        MachineAlternativeFamily::ConditionalBranchI64LessThan => 12,
        MachineAlternativeFamily::CallI64 => 13,
    });
    bytes.extend_from_slice(&alternative.key.variant.to_le_bytes());
    match alternative.applicability {
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
    match alternative.size {
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
    bytes.push(match alternative.latency {
        MachineLatencyKnowledge::StableBaselineUnavailable => 0,
    });
    encode_encoded_effects(bytes, &alternative.encoded);
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
        MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer,
            byte_count,
        } => {
            bytes.push(2);
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
        MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer,
            return_address_byte_count,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&stack_pointer.0.to_le_bytes());
            bytes.extend_from_slice(&return_address_byte_count.to_le_bytes());
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
        MachineEncodedControlEffect::DirectRelativeCallV1 => bytes.push(4),
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
