use sha2::{Digest, Sha256};

use crate::{LiteralFoldIdentity, LiteralFoldPlan};

pub fn literal_fold_identity(plan: &LiteralFoldPlan) -> LiteralFoldIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-literal-fold.v3\0");
    bytes.extend_from_slice(&encode_terminal_literal_fold_content(plan));
    LiteralFoldIdentity(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_literal_fold_content(plan: &LiteralFoldPlan) -> Vec<u8> {
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
    bytes.push(plan.policy.canonical_bits());
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
    use omega_selected_instructions::{
        SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
    };
    use psi_core::{FuelScheduleIdentity, MachineId};

    use crate::{
        AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FunctionLiteralFold,
        LiteralFoldAction, LiteralFoldDecodeError, LiteralFoldPlan, LiteralFoldPolicy,
        LiveRangeIdentity, LiveRangePoint, RecoveryClassificationIdentity, SpillChoiceIdentity,
        literal_fold_identity,
    };

    fn plan() -> LiteralFoldPlan {
        LiteralFoldPlan {
            source_selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            spill_choices: SpillChoiceIdentity::from_bytes([2; 32]),
            recovery_classifications: RecoveryClassificationIdentity::from_bytes([3; 32]),
            ranges: LiveRangeIdentity::from_bytes([4; 32]),
            legality: AllocationLegalityIdentity::from_bytes([5; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([6; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([7; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([8; 32]),
            fuel_schedule: FuelScheduleIdentity::new(9).unwrap(),
            policy: LiteralFoldPolicy::EXACT_ADD_V1,
            budget: OptimizationWorkBudget::new(10, 10, 20, 10, 1).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 1,
                validation_steps: 12,
                commits: 1,
                iterations: 1,
            },
            functions: vec![FunctionLiteralFold {
                machine: MachineId::new(10).unwrap(),
                action: Some(LiteralFoldAction {
                    block: SelectedBlockId(1),
                    pressure_point: LiveRangePoint(8),
                    literal_instruction: SelectedInstructionId(2),
                    victim: VirtualRegisterId(3),
                    consumer_instruction: SelectedInstructionId(4),
                    left: VirtualRegisterId(2),
                    result: VirtualRegisterId(4),
                    immediate: 12,
                    immediate_constraint: RegisterConstraintKey {
                        family: RegisterConstraintFamily::Instruction,
                        variant: 5,
                    },
                }),
            }],
            transformed_selected: SelectedInstructionPlanIdentity::from_bytes([11; 32]),
        }
    }

    #[test]
    fn identity_roots_every_recipe_field_and_codec_is_strict() {
        let baseline = plan();
        let identity = literal_fold_identity(&baseline);
        let mut changed = baseline.clone();
        changed.functions[0].action.as_mut().unwrap().immediate += 1;
        assert_ne!(literal_fold_identity(&changed), identity);
        changed = baseline.clone();
        changed.transformed_selected = SelectedInstructionPlanIdentity::from_bytes([12; 32]);
        assert_ne!(literal_fold_identity(&changed), identity);

        let encoded = baseline.encode();
        assert_eq!(LiteralFoldPlan::decode(&encoded).unwrap(), baseline);
        assert_eq!(
            LiteralFoldPlan::decode(&encoded[..encoded.len() - 1]),
            Err(LiteralFoldDecodeError::Truncated)
        );
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            LiteralFoldPlan::decode(&trailing),
            Err(LiteralFoldDecodeError::TrailingBytes)
        );
    }
}
