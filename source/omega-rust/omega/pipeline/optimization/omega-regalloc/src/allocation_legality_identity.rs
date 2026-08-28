use sha2::{Digest, Sha256};

use crate::{AllocationLegalityIdentity, AllocationLegalityPlan, VirtualFixedConstraintSite};

pub fn allocation_legality_identity(plan: &AllocationLegalityPlan) -> AllocationLegalityIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-allocation-legality.v5\0");
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    for functions in [&plan.functions, &plan.structural_unit_functions] {
        length(&mut bytes, functions.len());
        for function in functions {
            bytes.extend_from_slice(&function.machine.get().to_le_bytes());
            length(&mut bytes, function.virtual_registers.len());
            for register in &function.virtual_registers {
                bytes.extend_from_slice(&register.virtual_register.0.to_le_bytes());
                bytes.extend_from_slice(&register.class.0.to_le_bytes());
                length(&mut bytes, register.points.len());
                for point in &register.points {
                    bytes.extend_from_slice(&point.block.0.to_le_bytes());
                    bytes.extend_from_slice(&point.point.0.to_le_bytes());
                    length(&mut bytes, point.candidates.len());
                    for candidate in &point.candidates {
                        bytes.extend_from_slice(&candidate.0.to_le_bytes());
                    }
                }
                length(&mut bytes, register.early_clobber_points.len());
                for point in &register.early_clobber_points {
                    bytes.extend_from_slice(&point.block.0.to_le_bytes());
                    bytes.extend_from_slice(&point.position.0.to_le_bytes());
                    bytes.extend_from_slice(&point.instruction.0.to_le_bytes());
                    bytes.extend_from_slice(&point.operand.to_le_bytes());
                    bytes.extend_from_slice(&point.point.0.to_le_bytes());
                    length(&mut bytes, point.candidates.len());
                    for candidate in &point.candidates {
                        bytes.extend_from_slice(&candidate.0.to_le_bytes());
                    }
                }
                length(&mut bytes, register.entry_transitions.len());
                for transition in &register.entry_transitions {
                    bytes.extend_from_slice(&transition.from_view.0.to_le_bytes());
                    encode_fixed_site(&mut bytes, transition.to_site);
                    bytes.extend_from_slice(&transition.to_view.0.to_le_bytes());
                }
            }
        }
    }
    AllocationLegalityIdentity(Sha256::digest(bytes).into())
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
                omega_register_model::RegisterOperandAccess::Use => 0,
                omega_register_model::RegisterOperandAccess::Def => 1,
                omega_register_model::RegisterOperandAccess::UseDef => 2,
            });
        }
    }
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("allocation-legality identity length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use omega_register_model::{
        RegisterClassId, RegisterOperandAccess, RegisterViewId, TargetRegisterEnvironmentIdentity,
    };
    use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
    use psi_core::MachineId;

    use super::*;
    use crate::{
        AllocationLegalityPlan, AllocatorAvailabilityIdentity, EntryFixedViewTransition,
        FunctionAllocationLegality, LiveRangeIdentity, LiveRangePoint, LivenessPosition,
        VirtualEarlyClobberPointLegality, VirtualFixedConstraintSite, VirtualPointLegality,
        VirtualRegisterAllocationLegality,
    };

    type Mutation = fn(&mut AllocationLegalityPlan);

    fn plan() -> AllocationLegalityPlan {
        AllocationLegalityPlan {
            ranges: LiveRangeIdentity([1; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([2; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([5; 32]),
            functions: vec![FunctionAllocationLegality {
                machine: MachineId::new(1).unwrap(),
                virtual_registers: vec![VirtualRegisterAllocationLegality {
                    virtual_register: VirtualRegisterId(0),
                    class: RegisterClassId(0),
                    points: vec![VirtualPointLegality {
                        block: SelectedBlockId(0),
                        point: LiveRangePoint(1),
                        candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                    }],
                    early_clobber_points: vec![VirtualEarlyClobberPointLegality {
                        block: SelectedBlockId(0),
                        position: LivenessPosition(0),
                        instruction: SelectedInstructionId(2),
                        operand: 1,
                        point: LiveRangePoint(0),
                        candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                    }],
                    entry_transitions: vec![EntryFixedViewTransition {
                        from_view: RegisterViewId(0),
                        to_site: VirtualFixedConstraintSite::Operand {
                            position: LivenessPosition(0),
                            point: LiveRangePoint(1),
                            instruction: SelectedInstructionId(2),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        },
                        to_view: RegisterViewId(1),
                    }],
                }],
            }],
            structural_unit_functions: vec![FunctionAllocationLegality {
                machine: MachineId::new(2).unwrap(),
                virtual_registers: Vec::new(),
            }],
        }
    }

    #[test]
    fn identity_binds_every_legality_domain() {
        let baseline = allocation_legality_identity(&plan());
        assert_eq!(baseline, allocation_legality_identity(&plan()));
        let mutations: Vec<Mutation> = vec![
            |plan| plan.ranges = LiveRangeIdentity([3; 32]),
            |plan| {
                plan.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([4; 32])
            },
            |plan| plan.allocator_availability = AllocatorAvailabilityIdentity::from_bytes([6; 32]),
            |plan| plan.functions[0].machine = MachineId::new(2).unwrap(),
            |plan| plan.functions[0].virtual_registers[0].virtual_register = VirtualRegisterId(1),
            |plan| plan.functions[0].virtual_registers[0].class = RegisterClassId(1),
            |plan| plan.functions[0].virtual_registers[0].points[0].block = SelectedBlockId(1),
            |plan| plan.functions[0].virtual_registers[0].points[0].point = LiveRangePoint(2),
            |plan| {
                plan.functions[0].virtual_registers[0].points[0]
                    .candidates
                    .pop();
            },
            |plan| {
                plan.functions[0].virtual_registers[0].early_clobber_points[0].block =
                    SelectedBlockId(1)
            },
            |plan| {
                plan.functions[0].virtual_registers[0].early_clobber_points[0].position =
                    LivenessPosition(1)
            },
            |plan| {
                plan.functions[0].virtual_registers[0].early_clobber_points[0].instruction =
                    SelectedInstructionId(3)
            },
            |plan| plan.functions[0].virtual_registers[0].early_clobber_points[0].operand = 2,
            |plan| {
                plan.functions[0].virtual_registers[0].early_clobber_points[0].point =
                    LiveRangePoint(2)
            },
            |plan| {
                plan.functions[0].virtual_registers[0].early_clobber_points[0]
                    .candidates
                    .pop();
            },
            |plan| {
                plan.functions[0].virtual_registers[0].entry_transitions[0].from_view =
                    RegisterViewId(2)
            },
            |plan| {
                let VirtualFixedConstraintSite::Operand { position, .. } =
                    &mut plan.functions[0].virtual_registers[0].entry_transitions[0].to_site
                else {
                    unreachable!()
                };
                *position = LivenessPosition(1);
            },
            |plan| {
                plan.functions[0].virtual_registers[0].entry_transitions[0].to_view =
                    RegisterViewId(2)
            },
            |plan| {
                plan.functions[0].virtual_registers[0]
                    .entry_transitions
                    .clear()
            },
            |plan| plan.functions.clear(),
            |plan| plan.structural_unit_functions.clear(),
            |plan| plan.structural_unit_functions[0].machine = MachineId::new(3).unwrap(),
        ];
        for mutate in mutations {
            let mut changed = plan();
            mutate(&mut changed);
            assert_ne!(baseline, allocation_legality_identity(&changed));
        }
    }
}
