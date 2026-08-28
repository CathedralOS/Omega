use omega_register_model::{RegisterConstraintFamily, RegisterConstraintKey, RegisterUnitId};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternative, TerminalMachineAlternativeApplicability,
    TerminalMachineAlternativeFamily, TerminalMachineBarrier, TerminalMachineEncodedControlEffect,
    TerminalMachineEncodedEffects, TerminalMachineEncodedMemoryEffect,
    TerminalMachineEncodedStackEffect, TerminalMachineEncodedTrapBehavior,
    TerminalMachineSizeKnowledge, TerminalSelectedInstructionKind,
};

use crate::{TerminalPreAllocationMachineEffectIdentity, TerminalPreAllocationMachineEffectPlan};

pub fn terminal_pre_allocation_machine_effect_identity(
    plan: &TerminalPreAllocationMachineEffectPlan,
) -> TerminalPreAllocationMachineEffectIdentity {
    use sha2::{Digest, Sha256};

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-preallocation-machine-effects.v3\0");
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
    bytes
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
        | TerminalSelectedInstructionKind::ReturnI64 => {}
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
