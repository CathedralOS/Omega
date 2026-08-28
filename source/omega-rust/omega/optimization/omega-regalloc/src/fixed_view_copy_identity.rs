use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{RegisterConstraintFamily, RegisterOperandAccess};
use omega_terminal_target_operations_to_selected_instructions::terminal_selected_instruction_plan_identity;
use sha2::{Digest, Sha256};

use crate::{
    TerminalFixedViewCopyIdentity, TerminalFixedViewCopyPlan, TerminalFixedViewCopyPolicy,
    TerminalVirtualFixedConstraintSite,
};

pub fn terminal_fixed_view_copy_identity(
    plan: &TerminalFixedViewCopyPlan,
) -> TerminalFixedViewCopyIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-fixed-view-copies.v3\0");
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.source_ranges.bytes());
    bytes.extend_from_slice(&plan.source_legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.push(match plan.policy {
        TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => 0,
        TerminalFixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => 1,
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
    bytes
        .extend_from_slice(&terminal_selected_instruction_plan_identity(&plan.transformed).bytes());
    TerminalFixedViewCopyIdentity(Sha256::digest(bytes).into())
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

fn encode_fixed_site(bytes: &mut Vec<u8>, site: TerminalVirtualFixedConstraintSite) {
    match site {
        TerminalVirtualFixedConstraintSite::Entry => bytes.push(0),
        TerminalVirtualFixedConstraintSite::Operand {
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
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalSelectedInstructionId, TerminalSelectedInstructionPlan,
        TerminalSelectedInstructionPlanIdentity, TerminalVirtualRegisterId,
    };
    use psi_core::{FuelScheduleIdentity, MachineId, ValueId};
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;
    use crate::{
        TerminalAllocationLegalityIdentity, TerminalFixedViewCopy,
        TerminalFixedViewCopyDestination, TerminalFixedViewCopyPlan, TerminalFixedViewCopyPolicy,
        TerminalLiveRangeIdentity, TerminalLiveRangePoint, TerminalLivenessPosition,
        TerminalVirtualFixedConstraintSite,
    };

    type Mutation = fn(&mut TerminalFixedViewCopyPlan);

    fn plan() -> TerminalFixedViewCopyPlan {
        TerminalFixedViewCopyPlan {
            source_selected: TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(b"s"),
            source_ranges: TerminalLiveRangeIdentity([2; 32]),
            source_legality: TerminalAllocationLegalityIdentity([3; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            allocator_availability: crate::TerminalAllocatorAvailabilityIdentity::from_bytes(
                [5; 32],
            ),
            policy: TerminalFixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
            budget: OptimizationWorkBudget::new(10, 10, 10, 10, 10).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 2,
                validation_steps: 3,
                commits: 4,
                iterations: 1,
            },
            copies: vec![TerminalFixedViewCopy {
                function: 0,
                machine: MachineId::new(1).unwrap(),
                source_virtual_register: TerminalVirtualRegisterId(1),
                source_value: ValueId::new(2).unwrap(),
                source_definition_site: ValueDefinitionSite::FunctionParameter(1),
                from_view: RegisterViewId(3),
                to_view: RegisterViewId(7),
                insertion_block: TerminalSelectedBlockId(8),
                before_instruction: TerminalSelectedInstructionId(6),
                destinations: vec![
                    TerminalFixedViewCopyDestination {
                        site: TerminalVirtualFixedConstraintSite::Operand {
                            position: TerminalLivenessPosition(4),
                            point: TerminalLiveRangePoint(5),
                            instruction: TerminalSelectedInstructionId(6),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        },
                        block: TerminalSelectedBlockId(8),
                        view: RegisterViewId(7),
                    },
                    TerminalFixedViewCopyDestination {
                        site: TerminalVirtualFixedConstraintSite::Operand {
                            position: TerminalLivenessPosition(9),
                            point: TerminalLiveRangePoint(10),
                            instruction: TerminalSelectedInstructionId(11),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        },
                        block: TerminalSelectedBlockId(12),
                        view: RegisterViewId(7),
                    },
                ],
                copy_instruction: TerminalSelectedInstructionId(9),
                result_virtual_register: TerminalVirtualRegisterId(10),
                copy_constraint: RegisterConstraintKey {
                    family: RegisterConstraintFamily::Instruction,
                    variant: 11,
                },
            }],
            transformed: TerminalSelectedInstructionPlan {
                terminal_psi: TerminalPsiIdentity {
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
        let baseline = terminal_fixed_view_copy_identity(&plan());
        assert_eq!(baseline, terminal_fixed_view_copy_identity(&plan()));
        let mutations: Vec<Mutation> = vec![
            |plan| {
                plan.source_selected =
                    TerminalSelectedInstructionPlanIdentity::from_canonical_bytes(b"changed")
            },
            |plan| plan.source_ranges = TerminalLiveRangeIdentity([13; 32]),
            |plan| plan.source_legality = TerminalAllocationLegalityIdentity([14; 32]),
            |plan| {
                plan.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([15; 32])
            },
            |plan| {
                plan.allocator_availability =
                    crate::TerminalAllocatorAvailabilityIdentity::from_bytes([16; 32])
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
            |plan| plan.policy = TerminalFixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            |plan| plan.copies[0].destinations[0].site = TerminalVirtualFixedConstraintSite::Entry,
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
            assert_ne!(baseline, terminal_fixed_view_copy_identity(&changed));
        }
    }
}
