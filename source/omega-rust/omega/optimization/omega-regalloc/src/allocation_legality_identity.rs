use sha2::{Digest, Sha256};

use crate::{
    TerminalAllocationLegalityIdentity, TerminalAllocationLegalityPlan,
    TerminalVirtualFixedConstraintSite,
};

pub fn terminal_allocation_legality_identity(
    plan: &TerminalAllocationLegalityPlan,
) -> TerminalAllocationLegalityIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-allocation-legality.v2\0");
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
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
            length(&mut bytes, register.entry_transitions.len());
            for transition in &register.entry_transitions {
                bytes.extend_from_slice(&transition.from_view.0.to_le_bytes());
                encode_fixed_site(&mut bytes, transition.to_site);
                bytes.extend_from_slice(&transition.to_view.0.to_le_bytes());
            }
        }
    }
    TerminalAllocationLegalityIdentity(Sha256::digest(bytes).into())
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
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalSelectedInstructionId, TerminalVirtualRegisterId,
    };
    use psi_core::MachineId;

    use super::*;
    use crate::{
        TerminalAllocationLegalityPlan, TerminalAllocatorAvailabilityIdentity,
        TerminalEntryFixedViewTransition, TerminalFunctionAllocationLegality,
        TerminalLiveRangeIdentity, TerminalLiveRangePoint, TerminalLivenessPosition,
        TerminalVirtualFixedConstraintSite, TerminalVirtualPointLegality,
        TerminalVirtualRegisterAllocationLegality,
    };

    type Mutation = fn(&mut TerminalAllocationLegalityPlan);

    fn plan() -> TerminalAllocationLegalityPlan {
        TerminalAllocationLegalityPlan {
            ranges: TerminalLiveRangeIdentity([1; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([2; 32]),
            allocator_availability: TerminalAllocatorAvailabilityIdentity::from_bytes([5; 32]),
            functions: vec![TerminalFunctionAllocationLegality {
                machine: MachineId::new(1).unwrap(),
                virtual_registers: vec![TerminalVirtualRegisterAllocationLegality {
                    virtual_register: TerminalVirtualRegisterId(0),
                    class: RegisterClassId(0),
                    points: vec![TerminalVirtualPointLegality {
                        block: TerminalSelectedBlockId(0),
                        point: TerminalLiveRangePoint(1),
                        candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                    }],
                    entry_transitions: vec![TerminalEntryFixedViewTransition {
                        from_view: RegisterViewId(0),
                        to_site: TerminalVirtualFixedConstraintSite::Operand {
                            position: TerminalLivenessPosition(0),
                            point: TerminalLiveRangePoint(1),
                            instruction: TerminalSelectedInstructionId(2),
                            operand: 0,
                            access: RegisterOperandAccess::Use,
                        },
                        to_view: RegisterViewId(1),
                    }],
                }],
            }],
        }
    }

    #[test]
    fn identity_binds_every_legality_domain() {
        let baseline = terminal_allocation_legality_identity(&plan());
        assert_eq!(baseline, terminal_allocation_legality_identity(&plan()));
        let mutations: Vec<Mutation> = vec![
            |plan| plan.ranges = TerminalLiveRangeIdentity([3; 32]),
            |plan| {
                plan.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([4; 32])
            },
            |plan| {
                plan.allocator_availability =
                    TerminalAllocatorAvailabilityIdentity::from_bytes([6; 32])
            },
            |plan| plan.functions[0].machine = MachineId::new(2).unwrap(),
            |plan| {
                plan.functions[0].virtual_registers[0].virtual_register =
                    TerminalVirtualRegisterId(1)
            },
            |plan| plan.functions[0].virtual_registers[0].class = RegisterClassId(1),
            |plan| {
                plan.functions[0].virtual_registers[0].points[0].block = TerminalSelectedBlockId(1)
            },
            |plan| {
                plan.functions[0].virtual_registers[0].points[0].point = TerminalLiveRangePoint(2)
            },
            |plan| {
                plan.functions[0].virtual_registers[0].points[0]
                    .candidates
                    .pop();
            },
            |plan| {
                plan.functions[0].virtual_registers[0].entry_transitions[0].from_view =
                    RegisterViewId(2)
            },
            |plan| {
                let TerminalVirtualFixedConstraintSite::Operand { position, .. } =
                    &mut plan.functions[0].virtual_registers[0].entry_transitions[0].to_site
                else {
                    unreachable!()
                };
                *position = TerminalLivenessPosition(1);
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
        ];
        for mutate in mutations {
            let mut changed = plan();
            mutate(&mut changed);
            assert_ne!(baseline, terminal_allocation_legality_identity(&changed));
        }
    }
}
