use omega_register_model::{RegisterConstraintFamily, RegisterConstraintKey, RegisterUnitId};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternative, TerminalMachineAlternativeApplicability,
    TerminalMachineAlternativeFamily, TerminalMachineBarrier, TerminalMachineEncodedControlEffect,
    TerminalMachineEncodedEffects, TerminalMachineEncodedMemoryEffect,
    TerminalMachineEncodedStackEffect, TerminalMachineEncodedTrapBehavior,
    TerminalMachineSizeKnowledge, TerminalSelectedInstructionKind,
    TerminalStructuralUnitCallBarrier, TerminalStructuralUnitCallEffect,
    TerminalStructuralUnitCallFrameEffect, TerminalStructuralUnitCallMemoryEffect,
};

use crate::{TerminalPreAllocationMachineEffectIdentity, TerminalPreAllocationMachineEffectPlan};

pub fn terminal_pre_allocation_machine_effect_identity(
    plan: &TerminalPreAllocationMachineEffectPlan,
) -> TerminalPreAllocationMachineEffectIdentity {
    use sha2::{Digest, Sha256};

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-preallocation-machine-effects.v5\0");
    bytes.extend_from_slice(&encode_terminal_pre_allocation_machine_effect_content(plan));
    TerminalPreAllocationMachineEffectIdentity::from_bytes(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_pre_allocation_machine_effect_content(
    plan: &TerminalPreAllocationMachineEffectPlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.register_constraints.bytes());
    bytes.extend_from_slice(&plan.machine_effect_catalog.bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.blocks.len());
        for block in &function.blocks {
            bytes.extend_from_slice(&block.block.0.to_le_bytes());
            encode_len(&mut bytes, block.instructions.len());
            for instruction in &block.instructions {
                bytes.extend_from_slice(&instruction.instruction.0.to_le_bytes());
                encode_kind(&mut bytes, instruction.kind);
                encode_constraint_key(&mut bytes, instruction.constraint);
                encode_units(&mut bytes, &instruction.unit_uses);
                encode_units(&mut bytes, &instruction.unit_defs);
                encode_units(&mut bytes, &instruction.unit_clobbers);
                bytes.push(0); // memory: NoneV1
                bytes.push(0); // trap: NeverV1
                bytes.push(match instruction.barrier {
                    TerminalMachineBarrier::None => 0,
                    TerminalMachineBarrier::ControlFlow => 1,
                });
                bytes.push(0); // call: NoneV1
                bytes.push(0); // cleanup: NoneV1
                encode_provenance(&mut bytes, &instruction.provenance);
                encode_len(&mut bytes, instruction.alternatives.len());
                for alternative in &instruction.alternatives {
                    encode_alternative(&mut bytes, alternative);
                }
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
                bytes.extend_from_slice(&call.instruction.0.to_le_bytes());
                bytes.extend_from_slice(&call.operation.get().to_le_bytes());
                bytes.extend_from_slice(&call.callee.get().to_le_bytes());
                encode_constraint_key(&mut bytes, call.constraint);
                encode_units(&mut bytes, &call.unit_uses);
                encode_units(&mut bytes, &call.unit_defs);
                encode_units(&mut bytes, &call.unit_clobbers);
                encode_structural_layout(&mut bytes, call.layout);
                encode_effect_link(&mut bytes, call.effect);
                encode_ownership(&mut bytes, &call.ownership);
                encode_len(&mut bytes, call.claim_transfers.len());
                for transfer in &call.claim_transfers {
                    bytes.extend_from_slice(&transfer.claim.get().to_le_bytes());
                    bytes.extend_from_slice(&transfer.argument_index.to_le_bytes());
                }
                encode_provenance(&mut bytes, &call.provenance);
                encode_structural_declaration(&mut bytes, call.declaration);
            }
        }
        encode_ordinary_instruction(&mut bytes, &function.return_instruction);
        encode_effect_link(&mut bytes, function.return_effect);
        encode_ownership(&mut bytes, &function.return_ownership);
    }
    bytes
}

fn encode_ordinary_instruction(
    bytes: &mut Vec<u8>,
    instruction: &crate::TerminalInstructionMachineEffects,
) {
    bytes.extend_from_slice(&instruction.instruction.0.to_le_bytes());
    encode_kind(bytes, instruction.kind);
    encode_constraint_key(bytes, instruction.constraint);
    encode_units(bytes, &instruction.unit_uses);
    encode_units(bytes, &instruction.unit_defs);
    encode_units(bytes, &instruction.unit_clobbers);
    bytes.push(match instruction.memory {
        omega_terminal_selected_instructions::TerminalMachineMemoryEffect::NoneV1 => 0,
    });
    bytes.push(match instruction.trap {
        omega_terminal_selected_instructions::TerminalMachineTrapBehavior::NeverV1 => 0,
        omega_terminal_selected_instructions::TerminalMachineTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    bytes.push(match instruction.barrier {
        TerminalMachineBarrier::None => 0,
        TerminalMachineBarrier::ControlFlow => 1,
    });
    bytes.push(match instruction.call {
        omega_terminal_selected_instructions::TerminalMachineCallEffect::NoneV1 => 0,
    });
    bytes.push(match instruction.cleanup {
        omega_terminal_selected_instructions::TerminalMachineCleanupEffect::NoneV1 => 0,
    });
    encode_provenance(bytes, &instruction.provenance);
    encode_len(bytes, instruction.alternatives.len());
    for alternative in &instruction.alternatives {
        encode_alternative(bytes, alternative);
    }
}

fn encode_structural_layout(
    bytes: &mut Vec<u8>,
    layout: omega_terminal_selected_instructions::TerminalSelectedMicrosoftX64OwnedIndirectPairLayout,
) {
    bytes.extend_from_slice(&layout.shadow_byte_count.to_le_bytes());
    bytes.extend_from_slice(&layout.outgoing_frame_byte_count.to_le_bytes());
    bytes.extend_from_slice(&layout.pre_call_stack_alignment.to_le_bytes());
    for binding in layout.bindings {
        bytes.extend_from_slice(&(binding.parameter_index as u64).to_le_bytes());
        encode_machine_register(bytes, binding.pointer);
        bytes.extend_from_slice(&binding.copy_stack_byte_offset.to_le_bytes());
        bytes.extend_from_slice(&binding.byte_count.to_le_bytes());
        bytes.extend_from_slice(&binding.alignment.to_le_bytes());
    }
}

fn encode_machine_register(
    bytes: &mut Vec<u8>,
    register: omega_terminal_target_operations::MachineRegister,
) {
    use omega_terminal_target_operations::MachineRegister as R;
    let tag = match register {
        R::X86Rax => 0,
        R::X86Rcx => 1,
        R::X86Rdx => 2,
        R::X86Rbx => 3,
        R::X86Rsp => 4,
        R::X86Rbp => 5,
        R::X86Rsi => 6,
        R::X86Rdi => 7,
        R::X86R8 => 8,
        R::X86R9 => 9,
        R::X86R10 => 10,
        R::X86R11 => 11,
        R::X86R12 => 12,
        R::X86R13 => 13,
        R::X86R14 => 14,
        R::X86R15 => 15,
        R::X86Xmm(index) => {
            bytes.push(16);
            bytes.push(index);
            return;
        }
        R::Aarch64X(index) => {
            bytes.push(17);
            bytes.push(index);
            return;
        }
        R::Aarch64V(index) => {
            bytes.push(18);
            bytes.push(index);
            return;
        }
    };
    bytes.push(tag);
}

fn encode_effect_link(bytes: &mut Vec<u8>, effect: omega_optimization_unit::EffectLink) {
    bytes.extend_from_slice(&effect.input.to_le_bytes());
    bytes.extend_from_slice(&effect.output.to_le_bytes());
}

fn encode_ownership(bytes: &mut Vec<u8>, ownership: &[omega_optimization_unit::OwnershipEvent]) {
    use omega_optimization_unit::OwnershipEvent;
    encode_len(bytes, ownership.len());
    for event in ownership {
        match event {
            OwnershipEvent::ClaimTransfer(claims) => {
                bytes.push(1);
                encode_ids(bytes, claims.iter().map(|id| id.get()));
            }
            OwnershipEvent::ClaimCompletion(claims) => {
                bytes.push(2);
                encode_ids(bytes, claims.iter().map(|id| id.get()));
            }
            OwnershipEvent::Cleanup(actions) => {
                bytes.push(3);
                encode_len(bytes, actions.len());
                for action in actions {
                    encode_cleanup(bytes, action);
                }
            }
            OwnershipEvent::StructuralReturn(claims) => {
                bytes.push(4);
                encode_ids(bytes, claims.iter().map(|id| id.get()));
            }
            OwnershipEvent::CrashFrontier(claims) => {
                bytes.push(5);
                encode_ids(bytes, claims.iter().map(|id| id.get()));
            }
        }
    }
}

fn encode_cleanup(bytes: &mut Vec<u8>, action: &psi_terminal::TerminalAffineCleanupAction) {
    match action {
        psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
            bytes.push(1);
            bytes.extend_from_slice(&place.get().to_le_bytes());
        }
        psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
            bytes.push(2);
            bytes.extend_from_slice(&discard.place.get().to_le_bytes());
            encode_path(bytes, &discard.path);
            bytes.extend_from_slice(&discard.structural_type.get().to_le_bytes());
        }
        psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            bytes.push(3);
            bytes.extend_from_slice(&cleanup.place.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup.structural_type.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup.cleanup_machine.get().to_le_bytes());
            match cleanup.cleanup_receiver {
                None => bytes.push(0),
                Some(place) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&place.get().to_le_bytes());
                }
            }
            encode_ids(
                bytes,
                cleanup.requirement_obligations.iter().map(|id| id.get()),
            );
        }
    }
}

fn encode_path(bytes: &mut Vec<u8>, path: &[psi_terminal::StructuralPathSegment]) {
    encode_len(bytes, path.len());
    for segment in path {
        match segment {
            psi_terminal::StructuralPathSegment::Field(name) => {
                bytes.push(1);
                encode_len(bytes, name.len());
                bytes.extend_from_slice(name.as_bytes());
            }
            psi_terminal::StructuralPathSegment::FixedIndex(index) => {
                bytes.push(2);
                bytes.extend_from_slice(&index.to_le_bytes());
            }
        }
    }
}

fn encode_structural_declaration(
    bytes: &mut Vec<u8>,
    declaration: omega_terminal_selected_instructions::TerminalStructuralUnitCallEffectDeclaration,
) {
    encode_constraint_key(bytes, declaration.constraint);
    match declaration.memory {
        TerminalStructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
            root_byte_count,
            copy_stack_byte_offsets,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&root_byte_count.to_le_bytes());
            for offset in copy_stack_byte_offsets {
                bytes.extend_from_slice(&offset.to_le_bytes());
            }
        }
    }
    match declaration.frame {
        TerminalStructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
            frame_byte_count,
            shadow_byte_count,
            pre_call_stack_alignment,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&frame_byte_count.to_le_bytes());
            bytes.extend_from_slice(&shadow_byte_count.to_le_bytes());
            bytes.extend_from_slice(&pre_call_stack_alignment.to_le_bytes());
        }
    }
    bytes.push(match declaration.trap {
        omega_terminal_selected_instructions::TerminalMachineTrapBehavior::NeverV1 => 0,
        omega_terminal_selected_instructions::TerminalMachineTrapBehavior::MayArchitecturalFaultV1 => 1,
    });
    bytes.push(match declaration.barrier {
        TerminalStructuralUnitCallBarrier::CallV1 => 1,
    });
    bytes.push(match declaration.call {
        TerminalStructuralUnitCallEffect::DirectInternalUnitV1 => 1,
    });
    bytes.push(match declaration.cleanup {
        omega_terminal_selected_instructions::TerminalMachineCleanupEffect::NoneV1 => 0,
    });
}

fn encode_kind(bytes: &mut Vec<u8>, kind: TerminalSelectedInstructionKind) {
    bytes.push(match kind {
        TerminalSelectedInstructionKind::CompareI64Zero => 0,
        TerminalSelectedInstructionKind::MaterializeI64 { .. } => 1,
        TerminalSelectedInstructionKind::CopyI64 => 2,
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => 3,
        TerminalSelectedInstructionKind::ExactAddI64Immediate { .. } => 4,
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => 5,
        TerminalSelectedInstructionKind::ConditionalBranchNonZero => 6,
        TerminalSelectedInstructionKind::ReturnI64 => 7,
        TerminalSelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        TerminalSelectedInstructionKind::ReturnUnit => 9,
    });
    match kind {
        TerminalSelectedInstructionKind::MaterializeI64 { value } => encode_integer(bytes, value),
        TerminalSelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        }
        | TerminalSelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        }
        | TerminalSelectedInstructionKind::ExactSubtractI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        } => {
            encode_integer(bytes, immediate);
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        TerminalSelectedInstructionKind::CompareI64Zero
        | TerminalSelectedInstructionKind::CopyI64
        | TerminalSelectedInstructionKind::ConditionalBranchNonZero
        | TerminalSelectedInstructionKind::ReturnI64
        | TerminalSelectedInstructionKind::ReturnUnit => {}
    }
}

fn encode_integer(bytes: &mut Vec<u8>, value: psi_core::IntegerValue) {
    match value {
        psi_core::IntegerValue::Signed(value) => {
            bytes.push(0);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        psi_core::IntegerValue::Unsigned(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn encode_provenance(
    bytes: &mut Vec<u8>,
    provenance: &omega_terminal_selected_instructions::TerminalSelectedInstructionProvenance,
) {
    encode_ids(bytes, provenance.operations.iter().map(|id| id.get()));
    encode_ids(bytes, provenance.values.iter().map(|id| id.get()));
    encode_ids(bytes, provenance.edges.iter().map(|id| id.get()));
    encode_ids(bytes, provenance.obligations.iter().map(|id| id.get()));
    encode_len(bytes, provenance.fuel.len());
    for settlement in &provenance.fuel {
        match settlement.site {
            omega_optimization_unit::PsiProvenance::Operation(operation) => {
                bytes.push(0);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
            }
            omega_optimization_unit::PsiProvenance::Edge(edge) => {
                bytes.push(1);
                bytes.extend_from_slice(&edge.get().to_le_bytes());
            }
        }
        bytes.extend_from_slice(&settlement.units.to_le_bytes());
    }
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
        TerminalMachineAlternativeFamily::ReturnUnit => 9,
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
    bytes.push(0); // latency: StableBaselineUnavailable
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
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

fn encode_constraint_key(bytes: &mut Vec<u8>, key: RegisterConstraintKey) {
    bytes.push(match key.family {
        RegisterConstraintFamily::Call => 0,
        RegisterConstraintFamily::Return => 1,
        RegisterConstraintFamily::SystemCall => 2,
        RegisterConstraintFamily::InlineAssembly => 3,
        RegisterConstraintFamily::Instruction => 4,
    });
    bytes.extend_from_slice(&key.variant.to_le_bytes());
}

fn encode_units(bytes: &mut Vec<u8>, units: &[RegisterUnitId]) {
    encode_len(bytes, units.len());
    for unit in units {
        bytes.extend_from_slice(&unit.0.to_le_bytes());
    }
}

fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn encode_len(bytes: &mut Vec<u8>, length: usize) {
    bytes.extend_from_slice(&(length as u64).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::OptimizationUnitIdentity;
    use omega_register_model::{
        RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
        TargetRegisterEnvironmentIdentity,
    };
    use omega_terminal_selected_instructions::{
        TerminalMachineAlternativeKey, TerminalMachineCallEffect, TerminalMachineCleanupEffect,
        TerminalMachineEffectCatalogIdentity, TerminalMachineLatencyKnowledge,
        TerminalMachineMemoryEffect, TerminalMachineTrapBehavior, TerminalSelectedBlockId,
        TerminalSelectedInstructionId, TerminalSelectedInstructionPlanIdentity,
        TerminalSelectedInstructionProvenance,
    };
    use psi_core::{FuelScheduleIdentity, MachineId};

    use super::*;
    use crate::{
        TerminalBlockMachineEffects, TerminalFunctionMachineEffects,
        TerminalInstructionMachineEffects, TerminalPreAllocationMachineEffectPlan,
    };

    fn plan() -> TerminalPreAllocationMachineEffectPlan {
        let constraint = RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 4,
        };
        let mut plan = TerminalPreAllocationMachineEffectPlan {
            identity: TerminalPreAllocationMachineEffectIdentity::from_bytes([0; 32]),
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes([1; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: NativeTarget::linux_x64(),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes([4; 32]),
            machine_effect_catalog: TerminalMachineEffectCatalogIdentity::from_bytes([5; 32]),
            functions: vec![TerminalFunctionMachineEffects {
                machine: MachineId::new(1).unwrap(),
                blocks: vec![TerminalBlockMachineEffects {
                    block: TerminalSelectedBlockId(0),
                    instructions: vec![TerminalInstructionMachineEffects {
                        instruction: TerminalSelectedInstructionId(0),
                        kind: TerminalSelectedInstructionKind::CompareI64Zero,
                        constraint,
                        unit_uses: vec![RegisterUnitId(0)],
                        unit_defs: vec![RegisterUnitId(1)],
                        unit_clobbers: Vec::new(),
                        memory: TerminalMachineMemoryEffect::NoneV1,
                        trap: TerminalMachineTrapBehavior::NeverV1,
                        barrier: TerminalMachineBarrier::None,
                        call: TerminalMachineCallEffect::NoneV1,
                        cleanup: TerminalMachineCleanupEffect::NoneV1,
                        provenance: TerminalSelectedInstructionProvenance::default(),
                        alternatives: vec![TerminalMachineAlternative {
                            key: TerminalMachineAlternativeKey {
                                family: TerminalMachineAlternativeFamily::CompareI64Zero,
                                variant: 0,
                            },
                            applicability: TerminalMachineAlternativeApplicability::Always,
                            size: TerminalMachineSizeKnowledge::ExactBytes(3),
                            latency: TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
                            encoded: TerminalMachineEncodedEffects::fallthrough_v1(vec![0], vec![]),
                        }],
                    }],
                }],
            }],
            structural_unit_functions: Vec::new(),
        };
        plan.identity = terminal_pre_allocation_machine_effect_identity(&plan);
        plan
    }

    #[test]
    fn identity_binds_roots_effect_rows_provenance_and_alternatives() {
        let source = plan();
        let baseline = source.identity;
        assert_eq!(
            baseline,
            terminal_pre_allocation_machine_effect_identity(&source)
        );

        let mut changed = source.clone();
        changed.selected = TerminalSelectedInstructionPlanIdentity::from_bytes([9; 32]);
        assert_ne!(
            baseline,
            terminal_pre_allocation_machine_effect_identity(&changed)
        );
        let mut changed = source.clone();
        changed.functions[0].blocks[0].instructions[0]
            .unit_clobbers
            .push(RegisterUnitId(2));
        assert_ne!(
            baseline,
            terminal_pre_allocation_machine_effect_identity(&changed)
        );
        let mut changed = source.clone();
        changed.functions[0].blocks[0].instructions[0].barrier =
            TerminalMachineBarrier::ControlFlow;
        assert_ne!(
            baseline,
            terminal_pre_allocation_machine_effect_identity(&changed)
        );
        let mut changed = source.clone();
        changed.functions[0].blocks[0].instructions[0].alternatives[0].size =
            TerminalMachineSizeKnowledge::ExactBytes(4);
        assert_ne!(
            baseline,
            terminal_pre_allocation_machine_effect_identity(&changed)
        );
    }
}
