use sha2::{Digest, Sha256};

use crate::{
    TerminalPressureRematerializationIdentity, TerminalPressureRematerializationPlan,
    TerminalPressureRematerializationPolicy,
};

pub fn terminal_pressure_rematerialization_identity(
    plan: &TerminalPressureRematerializationPlan,
) -> TerminalPressureRematerializationIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-pressure-rematerialization.v2\0");
    bytes.extend_from_slice(&encode_terminal_pressure_rematerialization_content(plan));
    TerminalPressureRematerializationIdentity(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_pressure_rematerialization_content(
    plan: &TerminalPressureRematerializationPlan,
) -> Vec<u8> {
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
        TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 => 0,
        TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1 => 1,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    bytes.extend_from_slice(
        &u64::try_from(plan.functions.len())
            .expect("function count fits u64")
            .to_le_bytes(),
    );
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        match &function.action {
            None => bytes.push(0),
            Some(action) => {
                bytes.push(1);
                bytes.extend_from_slice(&action.block.0.to_le_bytes());
                bytes.extend_from_slice(&action.pressure_point.0.to_le_bytes());
                bytes.extend_from_slice(&action.victim.0.to_le_bytes());
                bytes.extend_from_slice(&action.current_view.0.to_le_bytes());
                bytes.extend_from_slice(&action.reclaimed_view.0.to_le_bytes());
                bytes.extend_from_slice(&action.original_materialize.0.to_le_bytes());
                bytes.extend_from_slice(&action.source_value.get().to_le_bytes());
                match action.value {
                    psi_core::IntegerValue::Unsigned(value) => {
                        bytes.extend_from_slice(&value.to_le_bytes());
                        bytes.push(0);
                    }
                    psi_core::IntegerValue::Signed(value) => {
                        bytes.extend_from_slice(&value.to_le_bytes());
                        bytes.push(1);
                    }
                }
                bytes.extend_from_slice(
                    &u64::try_from(action.rewrites.len())
                        .expect("rewrite count fits u64")
                        .to_le_bytes(),
                );
                for rewrite in &action.rewrites {
                    bytes.extend_from_slice(&rewrite.point.0.to_le_bytes());
                    bytes.extend_from_slice(&rewrite.instruction.0.to_le_bytes());
                    bytes.extend_from_slice(&rewrite.operand.to_le_bytes());
                }
                bytes.extend_from_slice(&action.fresh_materialize.0.to_le_bytes());
                bytes.extend_from_slice(&action.result_virtual_register.0.to_le_bytes());
                bytes.push(match action.materialize_constraint.family {
                    omega_register_model::RegisterConstraintFamily::Call => 0,
                    omega_register_model::RegisterConstraintFamily::Return => 1,
                    omega_register_model::RegisterConstraintFamily::SystemCall => 2,
                    omega_register_model::RegisterConstraintFamily::InlineAssembly => 3,
                    omega_register_model::RegisterConstraintFamily::Instruction => 4,
                });
                bytes.extend_from_slice(&action.materialize_constraint.variant.to_le_bytes());
            }
        }
    }
    bytes.extend_from_slice(&plan.transformed_selected.bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{
        OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    };
    use omega_register_model::{
        RegisterConstraintFamily, RegisterConstraintKey, RegisterViewId,
        TargetRegisterEnvironmentIdentity,
    };
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalSelectedInstructionId,
        TerminalSelectedInstructionPlanIdentity, TerminalVirtualRegisterId,
    };
    use psi_core::{FuelScheduleIdentity, IntegerValue, MachineId, ValueId};

    use crate::*;

    fn plan() -> TerminalPressureRematerializationPlan {
        TerminalPressureRematerializationPlan {
            source_selected: TerminalSelectedInstructionPlanIdentity::from_bytes([1; 32]),
            spill_choices: TerminalSpillChoiceIdentity::from_bytes([2; 32]),
            recovery_classifications: TerminalRecoveryClassificationIdentity::from_bytes([3; 32]),
            ranges: TerminalLiveRangeIdentity::from_bytes([4; 32]),
            legality: TerminalAllocationLegalityIdentity::from_bytes([5; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([6; 32]),
            allocator_availability: TerminalAllocatorAvailabilityIdentity::from_bytes([7; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([8; 32]),
            fuel_schedule: FuelScheduleIdentity::new(9).unwrap(),
            policy: TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
            budget: OptimizationWorkBudget::new(10, 10, 30, 10, 1).unwrap(),
            usage: OptimizationWorkUsage { rule_evaluations: 1, candidates: 1, validation_steps: 12, commits: 1, iterations: 1 },
            functions: vec![TerminalFunctionPressureRematerialization {
                machine: MachineId::new(10).unwrap(),
                action: Some(TerminalPressureRematerializationAction {
                    block: TerminalSelectedBlockId(0), pressure_point: TerminalLiveRangePoint(3),
                    victim: TerminalVirtualRegisterId(0), current_view: RegisterViewId(1), reclaimed_view: RegisterViewId(2),
                    original_materialize: TerminalSelectedInstructionId(0), source_value: ValueId::new(11).unwrap(),
                    value: IntegerValue::Unsigned(42),
                    rewrites: vec![TerminalPressureRematerializationRewrite {
                        point: TerminalLiveRangePoint(9),
                        instruction: TerminalSelectedInstructionId(3),
                        operand: 0,
                    }],
                    fresh_materialize: TerminalSelectedInstructionId(4), result_virtual_register: TerminalVirtualRegisterId(3),
                    materialize_constraint: RegisterConstraintKey { family: RegisterConstraintFamily::Instruction, variant: 7 },
                }),
            }],
            transformed_selected: TerminalSelectedInstructionPlanIdentity::from_bytes([12; 32]),
        }
    }

    #[test]
    fn identity_roots_recipe_and_codec_is_strict() {
        let baseline = plan();
        let identity = terminal_pressure_rematerialization_identity(&baseline);
        let mut changed = baseline.clone();
        changed.functions[0].action.as_mut().unwrap().rewrites[0].operand = 1;
        assert_ne!(
            terminal_pressure_rematerialization_identity(&changed),
            identity
        );
        let encoded = baseline.encode();
        assert_eq!(
            TerminalPressureRematerializationPlan::decode(&encoded).unwrap(),
            baseline
        );
        assert_eq!(
            TerminalPressureRematerializationPlan::decode(&encoded[..encoded.len() - 1]),
            Err(TerminalPressureRematerializationDecodeError::Truncated)
        );
        let mut corrupt = encoded.clone();
        corrupt[12] ^= 1;
        assert_eq!(
            TerminalPressureRematerializationPlan::decode(&corrupt),
            Err(TerminalPressureRematerializationDecodeError::IdentityMismatch)
        );
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            TerminalPressureRematerializationPlan::decode(&trailing),
            Err(TerminalPressureRematerializationDecodeError::TrailingBytes)
        );
    }

    #[test]
    fn codec_rejects_closed_tags() {
        let baseline = plan();
        let mut encoded = baseline.encode();
        let policy_offset = 44 + 32 * 8 + 4;
        encoded[policy_offset] = 9;
        assert_eq!(
            TerminalPressureRematerializationPlan::decode(&encoded),
            Err(TerminalPressureRematerializationDecodeError::UnknownPolicy(
                9
            ))
        );

        let mut old_version = baseline.encode();
        old_version[8..12].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            TerminalPressureRematerializationPlan::decode(&old_version),
            Err(TerminalPressureRematerializationDecodeError::UnsupportedVersion(1))
        );
    }

    #[test]
    fn codec_round_trips_the_multiple_future_use_policy_and_ordered_rewrites() {
        let mut multiple = plan();
        multiple.policy = TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1;
        multiple.functions[0]
            .action
            .as_mut()
            .unwrap()
            .rewrites
            .push(TerminalPressureRematerializationRewrite {
                point: TerminalLiveRangePoint(11),
                instruction: TerminalSelectedInstructionId(5),
                operand: 1,
            });
        let encoded = multiple.encode();
        assert_eq!(
            TerminalPressureRematerializationPlan::decode(&encoded).unwrap(),
            multiple
        );

        let mut reordered = multiple.clone();
        reordered.functions[0]
            .action
            .as_mut()
            .unwrap()
            .rewrites
            .swap(0, 1);
        assert_ne!(
            terminal_pressure_rematerialization_identity(&reordered),
            terminal_pressure_rematerialization_identity(&multiple)
        );
    }
}
