use sha2::{Digest, Sha256};

use crate::{TerminalRegisterHomeIdentity, TerminalRegisterHomePlan};

pub fn terminal_register_home_identity(
    plan: &TerminalRegisterHomePlan,
) -> TerminalRegisterHomeIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-register-homes.v1\0");
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.assignments.len());
        for assignment in &function.assignments {
            bytes.extend_from_slice(&assignment.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.class.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.view.0.to_le_bytes());
        }
    }
    TerminalRegisterHomeIdentity(Sha256::digest(bytes).into())
}

fn encode_len(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("register-home identity length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use omega_register_model::{
        RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity,
    };
    use omega_terminal_selected_instructions::TerminalVirtualRegisterId;
    use psi_core::MachineId;

    use super::*;
    use crate::{
        TerminalAllocationLegalityIdentity, TerminalFunctionRegisterHomes,
        TerminalLiveRangeIdentity, TerminalRegisterHomePlan, TerminalVirtualRegisterHome,
    };

    type Mutation = fn(&mut TerminalRegisterHomePlan);

    fn plan() -> TerminalRegisterHomePlan {
        TerminalRegisterHomePlan {
            legality: TerminalAllocationLegalityIdentity([1; 32]),
            ranges: TerminalLiveRangeIdentity([2; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            functions: vec![TerminalFunctionRegisterHomes {
                machine: MachineId::new(1).unwrap(),
                assignments: vec![TerminalVirtualRegisterHome {
                    virtual_register: TerminalVirtualRegisterId(0),
                    class: RegisterClassId(1),
                    view: RegisterViewId(2),
                }],
            }],
        }
    }

    #[test]
    fn identity_binds_every_home_domain() {
        let baseline = terminal_register_home_identity(&plan());
        assert_eq!(baseline, terminal_register_home_identity(&plan()));
        let mutations: Vec<Mutation> = vec![
            |plan| plan.legality = TerminalAllocationLegalityIdentity([4; 32]),
            |plan| plan.ranges = TerminalLiveRangeIdentity([5; 32]),
            |plan| {
                plan.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([6; 32])
            },
            |plan| plan.functions[0].machine = MachineId::new(2).unwrap(),
            |plan| plan.functions[0].assignments[0].virtual_register = TerminalVirtualRegisterId(1),
            |plan| plan.functions[0].assignments[0].class = RegisterClassId(2),
            |plan| plan.functions[0].assignments[0].view = RegisterViewId(3),
            |plan| plan.functions[0].assignments.clear(),
            |plan| plan.functions.clear(),
        ];
        for mutate in mutations {
            let mut changed = plan();
            mutate(&mut changed);
            assert_ne!(baseline, terminal_register_home_identity(&changed));
        }
    }
}
