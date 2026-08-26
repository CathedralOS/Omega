use sha2::{Digest, Sha256};

use crate::{TerminalLiteralFoldIdentity, TerminalLiteralFoldPlan, TerminalLiteralFoldPolicy};

pub fn terminal_literal_fold_identity(
    plan: &TerminalLiteralFoldPlan,
) -> TerminalLiteralFoldIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-literal-fold.v1\0");
    bytes.extend_from_slice(&encode_terminal_literal_fold_content(plan));
    TerminalLiteralFoldIdentity(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_literal_fold_content(plan: &TerminalLiteralFoldPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.spill_choices.bytes());
    bytes.extend_from_slice(&plan.recovery_classifications.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        match function.action {
            None => bytes.push(0),
            Some(action) => {
                bytes.push(1);
                bytes.extend_from_slice(&action.block.0.to_le_bytes());
                bytes.extend_from_slice(&action.pressure_point.0.to_le_bytes());
                bytes.extend_from_slice(&action.literal_instruction.0.to_le_bytes());
                bytes.extend_from_slice(&action.victim.0.to_le_bytes());
                bytes.extend_from_slice(&action.consumer_instruction.0.to_le_bytes());
                bytes.extend_from_slice(&action.left.0.to_le_bytes());
                bytes.extend_from_slice(&action.result.0.to_le_bytes());
                bytes.extend_from_slice(&action.immediate.to_le_bytes());
                bytes.push(match action.immediate_constraint.family {
                    omega_register_model::RegisterConstraintFamily::Call => 0,
                    omega_register_model::RegisterConstraintFamily::Return => 1,
                    omega_register_model::RegisterConstraintFamily::SystemCall => 2,
                    omega_register_model::RegisterConstraintFamily::InlineAssembly => 3,
                    omega_register_model::RegisterConstraintFamily::Instruction => 4,
                });
                bytes.extend_from_slice(&action.immediate_constraint.variant.to_le_bytes());
            }
        }
    }
    bytes.extend_from_slice(&plan.transformed_selected.bytes());
    bytes
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("literal-fold identity length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{
        OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    };
    use omega_register_model::{
        RegisterConstraintFamily, RegisterConstraintKey, TargetRegisterEnvironmentIdentity,
    };
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalSelectedInstructionId,
        TerminalSelectedInstructionPlanIdentity, TerminalVirtualRegisterId,
    };
    use psi_core::{FuelScheduleIdentity, MachineId};

    use crate::{
        TerminalAllocationLegalityIdentity, TerminalAllocatorAvailabilityIdentity,
        TerminalFunctionLiteralFold, TerminalLiteralFoldAction, TerminalLiteralFoldDecodeError,
        TerminalLiteralFoldPlan, TerminalLiteralFoldPolicy, TerminalLiveRangeIdentity,
        TerminalLiveRangePoint, TerminalRecoveryClassificationIdentity,
        TerminalSpillChoiceIdentity, terminal_literal_fold_identity,
    };

    fn plan() -> TerminalLiteralFoldPlan {
        TerminalLiteralFoldPlan {
            source_selected: TerminalSelectedInstructionPlanIdentity::from_bytes([1; 32]),
            spill_choices: TerminalSpillChoiceIdentity::from_bytes([2; 32]),
            recovery_classifications: TerminalRecoveryClassificationIdentity::from_bytes([3; 32]),
            ranges: TerminalLiveRangeIdentity::from_bytes([4; 32]),
            legality: TerminalAllocationLegalityIdentity::from_bytes([5; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([6; 32]),
            allocator_availability: TerminalAllocatorAvailabilityIdentity::from_bytes([7; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([8; 32]),
            fuel_schedule: FuelScheduleIdentity::new(9).unwrap(),
            policy: TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
            budget: OptimizationWorkBudget::new(10, 10, 20, 10, 1).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 1,
                validation_steps: 12,
                commits: 1,
                iterations: 1,
            },
            functions: vec![TerminalFunctionLiteralFold {
                machine: MachineId::new(10).unwrap(),
                action: Some(TerminalLiteralFoldAction {
                    block: TerminalSelectedBlockId(1),
                    pressure_point: TerminalLiveRangePoint(8),
                    literal_instruction: TerminalSelectedInstructionId(2),
                    victim: TerminalVirtualRegisterId(3),
                    consumer_instruction: TerminalSelectedInstructionId(4),
                    left: TerminalVirtualRegisterId(2),
                    result: TerminalVirtualRegisterId(4),
                    immediate: 12,
                    immediate_constraint: RegisterConstraintKey {
                        family: RegisterConstraintFamily::Instruction,
                        variant: 5,
                    },
                }),
            }],
            transformed_selected: TerminalSelectedInstructionPlanIdentity::from_bytes([11; 32]),
        }
    }

    #[test]
    fn identity_roots_every_recipe_field_and_codec_is_strict() {
        let baseline = plan();
        let identity = terminal_literal_fold_identity(&baseline);
        let mut changed = baseline.clone();
        changed.functions[0].action.as_mut().unwrap().immediate += 1;
        assert_ne!(terminal_literal_fold_identity(&changed), identity);
        changed = baseline.clone();
        changed.transformed_selected =
            TerminalSelectedInstructionPlanIdentity::from_bytes([12; 32]);
        assert_ne!(terminal_literal_fold_identity(&changed), identity);

        let encoded = baseline.encode();
        assert_eq!(TerminalLiteralFoldPlan::decode(&encoded).unwrap(), baseline);
        assert_eq!(
            TerminalLiteralFoldPlan::decode(&encoded[..encoded.len() - 1]),
            Err(TerminalLiteralFoldDecodeError::Truncated)
        );
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            TerminalLiteralFoldPlan::decode(&trailing),
            Err(TerminalLiteralFoldDecodeError::TrailingBytes)
        );
    }
}
