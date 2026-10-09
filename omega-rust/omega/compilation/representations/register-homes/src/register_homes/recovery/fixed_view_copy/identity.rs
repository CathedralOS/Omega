use super::codec::evidence::encode_source_evidence;
use crate::identity::encode_fixed_constraint_site_identity;
use optimization_unit::encode_value_definition_site_identity;
use selected_instructions::{FixedViewCopyIdentity, selected_instruction_plan_identity};

use crate::{FixedViewCopyPlan, FixedViewCopyPolicy};

pub fn fixed_view_copy_identity(plan: &FixedViewCopyPlan) -> FixedViewCopyIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.source_ranges.bytes());
    bytes.extend_from_slice(&plan.source_legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    encode_source_evidence(&mut bytes, plan.source_evidence);
    bytes.push(match plan.policy {
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => 0,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => 1,
        FixedViewCopyPolicy::ImmediateBeforeFixedUseV1 => 2,
        FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1 => 3,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    crate::codec_bytes::length(&mut bytes, plan.copies.len(), "fixed-view copy identity");
    for copy in &plan.copies {
        bytes.extend_from_slice(&copy.function.to_le_bytes());
        bytes.extend_from_slice(&copy.machine.get().to_le_bytes());
        bytes.extend_from_slice(&copy.source_virtual_register.0.to_le_bytes());
        bytes.extend_from_slice(&copy.source_value.get().to_le_bytes());
        encode_value_definition_site_identity(&mut bytes, copy.source_definition_site);
        bytes.extend_from_slice(&copy.from_view.0.to_le_bytes());
        bytes.extend_from_slice(&copy.to_view.0.to_le_bytes());
        bytes.extend_from_slice(&copy.insertion_block.0.to_le_bytes());
        bytes.extend_from_slice(&copy.before_instruction.0.to_le_bytes());
        crate::codec_bytes::length(
            &mut bytes,
            copy.destinations.len(),
            "fixed-view copy identity",
        );
        for destination in &copy.destinations {
            encode_fixed_constraint_site_identity(&mut bytes, destination.site);
            bytes.extend_from_slice(&destination.block.0.to_le_bytes());
            bytes.extend_from_slice(&destination.view.0.to_le_bytes());
        }
        bytes.extend_from_slice(&copy.copy_instruction.0.to_le_bytes());
        bytes.extend_from_slice(&copy.result_virtual_register.0.to_le_bytes());
        bytes.push(copy.copy_constraint.family.tag());
        bytes.extend_from_slice(&copy.copy_constraint.variant.to_le_bytes());
    }
    bytes.extend_from_slice(&selected_instruction_plan_identity(&plan.transformed).bytes());
    FixedViewCopyIdentity::from_bytes(crate::codec_bytes::domain_identity(
        b"omega.terminal-fixed-view-copies.v7\0",
        &bytes,
    ))
}

#[cfg(test)]
mod tests {
    use crate::FixedViewCopySourceEvidence;
    use crate::fixed_view_copy_identity;
    use crate::{
        AllocationLegalityIdentity, FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan,
        FixedViewCopyPolicy,
    };
    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use optimization_unit::ValueDefinitionSite;
    use register_model::{
        RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess, RegisterViewId,
        TargetRegisterEnvironmentIdentity,
    };
    use selected_instructions::{
        LiveRangeIdentity, LiveRangePoint, LivenessPosition, SelectedBlockId,
        SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
        VirtualFixedConstraintSite, VirtualRegisterId,
    };
    use semantic_vocabulary::{FuelScheduleIdentity, MachineId, ValueId};
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    type Mutation = fn(&mut FixedViewCopyPlan);

    fn plan() -> FixedViewCopyPlan {
        FixedViewCopyPlan {
            source_selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"s"),
            source_ranges: LiveRangeIdentity::from_bytes([2; 32]),
            source_legality: AllocationLegalityIdentity::from_bytes([3; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            allocator_availability: crate::AllocatorAvailabilityIdentity::from_bytes([5; 32]),
            source_evidence: FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
                fixed_intervals: crate::FixedPrecoloredIntervalPlanIdentity::from_bytes([6; 32]),
                split_requirements: crate::FixedPrecoloredSplitRequirementPlanIdentity::from_bytes(
                    [7; 32],
                ),
                segment_homes: crate::FixedPrecoloredSegmentHomePlanIdentity::from_bytes([8; 32]),
            },
            policy: FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
            budget: OptimizationWorkBudget::new(10, 10, 10, 10, 10).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 2,
                validation_steps: 3,
                commits: 4,
                iterations: 1,
            },
            copies: vec![FixedViewCopy {
                function: 0,
                machine: MachineId::new(1).unwrap(),
                source_virtual_register: VirtualRegisterId(1),
                source_value: ValueId::new(2).unwrap(),
                source_definition_site: ValueDefinitionSite::FunctionParameter(1),
                from_view: RegisterViewId(3),
                to_view: RegisterViewId(7),
                insertion_block: SelectedBlockId(8),
                before_instruction: SelectedInstructionId(6),
                destinations: vec![
                    FixedViewCopyDestination {
                        site: VirtualFixedConstraintSite::Operand {
                            position: LivenessPosition(4),
                            point: LiveRangePoint(5),
                            instruction: SelectedInstructionId(6),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        },
                        block: SelectedBlockId(8),
                        view: RegisterViewId(7),
                    },
                    FixedViewCopyDestination {
                        site: VirtualFixedConstraintSite::Operand {
                            position: LivenessPosition(9),
                            point: LiveRangePoint(10),
                            instruction: SelectedInstructionId(11),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        },
                        block: SelectedBlockId(12),
                        view: RegisterViewId(7),
                    },
                ],
                copy_instruction: SelectedInstructionId(9),
                result_virtual_register: VirtualRegisterId(10),
                copy_constraint: RegisterConstraintKey {
                    family: RegisterConstraintFamily::Instruction,
                    variant: 11,
                },
            }],
            transformed: SelectedInstructionPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([12; 32]),
                },
                fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
                target: target::NativeTarget::linux_x64(),
                entry: MachineId::new(1).unwrap(),
                functions: Default::default(),
            }
            .into(),
        }
    }

    #[test]
    fn identity_binds_roots_work_copy_rows_and_transformed_plan() {
        let baseline = fixed_view_copy_identity(&plan());
        assert_eq!(baseline, fixed_view_copy_identity(&plan()));
        let mutations: Vec<Mutation> = vec![
            |plan| {
                plan.source_selected =
                    SelectedInstructionPlanIdentity::from_canonical_bytes(b"changed")
            },
            |plan| plan.source_ranges = LiveRangeIdentity::from_bytes([13; 32]),
            |plan| plan.source_legality = AllocationLegalityIdentity::from_bytes([14; 32]),
            |plan| {
                plan.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([15; 32])
            },
            |plan| {
                plan.allocator_availability =
                    crate::AllocatorAvailabilityIdentity::from_bytes([16; 32])
            },
            |plan| plan.source_evidence = FixedViewCopySourceEvidence::LegacyLegalityTransitionsV1,
            |plan| plan.budget = OptimizationWorkBudget::new(11, 10, 10, 10, 10).unwrap(),
            |plan| plan.usage.commits += 1,
            |plan| plan.copies[0].function += 1,
            |plan| plan.copies[0].machine = MachineId::new(2).unwrap(),
            |plan| plan.copies[0].source_virtual_register.0 += 1,
            |plan| plan.copies[0].source_value = ValueId::new(3).unwrap(),
            |plan| {
                plan.copies[0].source_definition_site = ValueDefinitionSite::FunctionParameter(2)
            },
            |plan| plan.copies[0].from_view.0 += 1,
            |plan| plan.copies[0].to_view.0 += 1,
            |plan| plan.policy = FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            |plan| plan.copies[0].destinations[0].site = VirtualFixedConstraintSite::Entry,
            |plan| plan.copies[0].destinations[0].view.0 += 1,
            |plan| plan.copies[0].destinations[0].block.0 += 1,
            |plan| plan.copies[0].destinations.swap(0, 1),
            |plan| plan.copies[0].insertion_block.0 += 1,
            |plan| plan.copies[0].before_instruction.0 += 1,
            |plan| plan.copies[0].copy_instruction.0 += 1,
            |plan| plan.copies[0].result_virtual_register.0 += 1,
            |plan| plan.copies[0].copy_constraint.variant += 1,
            |plan| plan.copies.clear(),
            |plan| plan.copies[0].destinations.clear(),
            |plan| {
                std::sync::Arc::make_mut(&mut plan.transformed).entry = MachineId::new(2).unwrap()
            },
        ];
        for mutate in mutations {
            let mut changed = plan();
            mutate(&mut changed);
            assert_ne!(baseline, fixed_view_copy_identity(&changed));
        }
    }
}
