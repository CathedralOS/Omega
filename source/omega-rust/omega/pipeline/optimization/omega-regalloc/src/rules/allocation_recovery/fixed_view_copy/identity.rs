use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{RegisterConstraintFamily, RegisterOperandAccess};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;
use sha2::{Digest, Sha256};

use crate::{
    FixedViewCopyIdentity, FixedViewCopyPlan, FixedViewCopyPolicy, VirtualFixedConstraintSite,
};

pub fn fixed_view_copy_identity(plan: &FixedViewCopyPlan) -> FixedViewCopyIdentity {
    fixed_view_copy_identity_with_schema(
        plan,
        b"omega.terminal-fixed-view-copies.v4\0",
        selected_instruction_plan_identity(&plan.transformed),
    )
}

pub(crate) fn fixed_view_copy_identity_v3_legacy(
    plan: &FixedViewCopyPlan,
) -> FixedViewCopyIdentity {
    fixed_view_copy_identity_with_schema(
        plan,
        b"omega.terminal-fixed-view-copies.v3\0",
        omega_target_operations_to_selected_instructions::selected_instruction_plan_identity_v11_legacy(
            &plan.transformed,
        ),
    )
}

fn fixed_view_copy_identity_with_schema(
    plan: &FixedViewCopyPlan,
    domain: &[u8],
    transformed: omega_selected_instructions::SelectedInstructionPlanIdentity,
) -> FixedViewCopyIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.source_ranges.bytes());
    bytes.extend_from_slice(&plan.source_legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.push(match plan.policy {
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => 0,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => 1,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_len(&mut bytes, plan.copies.len());
    for copy in &plan.copies {
        bytes.extend_from_slice(&copy.function.to_le_bytes());
        bytes.extend_from_slice(&copy.machine.get().to_le_bytes());
        bytes.extend_from_slice(&copy.source_virtual_register.0.to_le_bytes());
        bytes.extend_from_slice(&copy.source_value.get().to_le_bytes());
        encode_definition_site(&mut bytes, copy.source_definition_site);
        bytes.extend_from_slice(&copy.from_view.0.to_le_bytes());
        bytes.extend_from_slice(&copy.to_view.0.to_le_bytes());
        bytes.extend_from_slice(&copy.insertion_block.0.to_le_bytes());
        bytes.extend_from_slice(&copy.before_instruction.0.to_le_bytes());
        encode_len(&mut bytes, copy.destinations.len());
        for destination in &copy.destinations {
            encode_fixed_site(&mut bytes, destination.site);
            bytes.extend_from_slice(&destination.block.0.to_le_bytes());
            bytes.extend_from_slice(&destination.view.0.to_le_bytes());
        }
        bytes.extend_from_slice(&copy.copy_instruction.0.to_le_bytes());
        bytes.extend_from_slice(&copy.result_virtual_register.0.to_le_bytes());
        bytes.push(constraint_family(copy.copy_constraint.family));
        bytes.extend_from_slice(&copy.copy_constraint.variant.to_le_bytes());
    }
    bytes.extend_from_slice(&transformed.bytes());
    FixedViewCopyIdentity(Sha256::digest(bytes).into())
}

fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

fn encode_fixed_site(bytes: &mut Vec<u8>, site: VirtualFixedConstraintSite) {
    match site {
        VirtualFixedConstraintSite::Entry => bytes.push(0),
        VirtualFixedConstraintSite::Operand {
            position,
            point,
            instruction,
            operand,
            access,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&position.0.to_le_bytes());
            bytes.extend_from_slice(&point.0.to_le_bytes());
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&operand.to_le_bytes());
            bytes.push(match access {
                RegisterOperandAccess::Use => 0,
                RegisterOperandAccess::Def => 1,
                RegisterOperandAccess::UseDef => 2,
            });
        }
    }
}

const fn constraint_family(family: RegisterConstraintFamily) -> u8 {
    match family {
        RegisterConstraintFamily::Call => 0,
        RegisterConstraintFamily::Return => 1,
        RegisterConstraintFamily::SystemCall => 2,
        RegisterConstraintFamily::InlineAssembly => 3,
        RegisterConstraintFamily::Instruction => 4,
    }
}

fn encode_len(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("fixed-view copy identity length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use omega_optimization_unit::ValueDefinitionSite;
    use omega_register_model::{
        RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess, RegisterViewId,
        TargetRegisterEnvironmentIdentity,
    };
    use omega_selected_instructions::{
        SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
        SelectedInstructionPlanIdentity, VirtualRegisterId,
    };
    use psi_core::{FuelScheduleIdentity, MachineId, ValueId};
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;
    use crate::{
        AllocationLegalityIdentity, FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan,
        FixedViewCopyPolicy, LiveRangeIdentity, LiveRangePoint, LivenessPosition,
        VirtualFixedConstraintSite,
    };

    type Mutation = fn(&mut FixedViewCopyPlan);

    fn plan() -> FixedViewCopyPlan {
        FixedViewCopyPlan {
            source_selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"s"),
            source_ranges: LiveRangeIdentity([2; 32]),
            source_legality: AllocationLegalityIdentity([3; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            allocator_availability: crate::AllocatorAvailabilityIdentity::from_bytes([5; 32]),
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
                target: omega_target::NativeTarget::linux_x64(),
                entry: MachineId::new(1).unwrap(),
                functions: Vec::new(),
                structural_unit_functions: Vec::new(),
            },
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
            |plan| plan.source_ranges = LiveRangeIdentity([13; 32]),
            |plan| plan.source_legality = AllocationLegalityIdentity([14; 32]),
            |plan| {
                plan.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([15; 32])
            },
            |plan| {
                plan.allocator_availability =
                    crate::AllocatorAvailabilityIdentity::from_bytes([16; 32])
            },
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
            |plan| plan.transformed.entry = MachineId::new(2).unwrap(),
        ];
        for mutate in mutations {
            let mut changed = plan();
            mutate(&mut changed);
            assert_ne!(baseline, fixed_view_copy_identity(&changed));
        }
    }
}
