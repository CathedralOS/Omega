use sha2::{Digest, Sha256};

use crate::{RegisterHomeIdentity, RegisterHomePlan};

pub fn register_home_identity(plan: &RegisterHomePlan) -> RegisterHomeIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-register-homes.v6\0");
    bytes.extend_from_slice(&encode_terminal_register_home_content(plan));
    RegisterHomeIdentity(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_register_home_content(plan: &RegisterHomePlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
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
    encode_len(&mut bytes, plan.structural_unit_functions.len());
    for function in &plan.structural_unit_functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_len(&mut bytes, function.assignments.len());
        for assignment in &function.assignments {
            bytes.extend_from_slice(&assignment.virtual_register.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.class.0.to_le_bytes());
            bytes.extend_from_slice(&assignment.view.0.to_le_bytes());
        }
    }
    bytes
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
    use omega_selected_instructions::VirtualRegisterId;
    use psi_core::MachineId;

    use super::*;
    use crate::{
        AllocationLegalityIdentity, FunctionRegisterHomes, LiveRangeIdentity,
        RegisterHomeDecodeError, RegisterHomePlan, VirtualRegisterHome,
    };

    type Mutation = fn(&mut RegisterHomePlan);

    fn plan() -> RegisterHomePlan {
        RegisterHomePlan {
            legality: AllocationLegalityIdentity([1; 32]),
            ranges: LiveRangeIdentity([2; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            allocator_availability: crate::AllocatorAvailabilityIdentity::from_bytes([4; 32]),
            functions: vec![FunctionRegisterHomes {
                machine: MachineId::new(1).unwrap(),
                assignments: vec![VirtualRegisterHome {
                    virtual_register: VirtualRegisterId(0),
                    class: RegisterClassId(1),
                    view: RegisterViewId(2),
                }],
            }],
            structural_unit_functions: vec![FunctionRegisterHomes {
                machine: MachineId::new(2).unwrap(),
                assignments: Vec::new(),
            }],
        }
    }

    #[test]
    fn identity_binds_every_home_domain() {
        let baseline = register_home_identity(&plan());
        assert_eq!(baseline, register_home_identity(&plan()));
        let mutations: Vec<Mutation> = vec![
            |plan| plan.legality = AllocationLegalityIdentity([4; 32]),
            |plan| plan.ranges = LiveRangeIdentity([5; 32]),
            |plan| {
                plan.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([6; 32])
            },
            |plan| {
                plan.allocator_availability =
                    crate::AllocatorAvailabilityIdentity::from_bytes([7; 32])
            },
            |plan| plan.functions[0].machine = MachineId::new(2).unwrap(),
            |plan| plan.functions[0].assignments[0].virtual_register = VirtualRegisterId(1),
            |plan| plan.functions[0].assignments[0].class = RegisterClassId(2),
            |plan| plan.functions[0].assignments[0].view = RegisterViewId(3),
            |plan| plan.functions[0].assignments.clear(),
            |plan| plan.functions.clear(),
            |plan| plan.structural_unit_functions.clear(),
            |plan| plan.structural_unit_functions[0].machine = MachineId::new(3).unwrap(),
        ];
        for mutate in mutations {
            let mut changed = plan();
            mutate(&mut changed);
            assert_ne!(baseline, register_home_identity(&changed));
        }
    }

    #[test]
    fn canonical_home_codec_rejects_framing_and_identity_corruption() {
        let plan = plan();
        let encoded = plan.encode();
        assert_eq!(RegisterHomePlan::decode(&encoded), Ok(plan));

        let mut identity_tamper = encoded.clone();
        identity_tamper[12] ^= 1;
        assert_eq!(
            RegisterHomePlan::decode(&identity_tamper),
            Err(RegisterHomeDecodeError::IdentityMismatch)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            RegisterHomePlan::decode(&trailing),
            Err(RegisterHomeDecodeError::TrailingBytes)
        );
        assert_eq!(
            RegisterHomePlan::decode(&encoded[..encoded.len() - 1]),
            Err(RegisterHomeDecodeError::Truncated)
        );
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            RegisterHomePlan::decode(&wrong_magic),
            Err(RegisterHomeDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        wrong_version[8..12].copy_from_slice(&4_u32.to_le_bytes());
        assert_eq!(
            RegisterHomePlan::decode(&wrong_version),
            Err(RegisterHomeDecodeError::UnsupportedVersion(4))
        );
        let mut invalid_machine = encoded;
        let machine_offset = 8 + 4 + 32 + (4 * 32) + 8;
        invalid_machine[machine_offset..machine_offset + 8].copy_from_slice(&0_u64.to_le_bytes());
        assert_eq!(
            RegisterHomePlan::decode(&invalid_machine),
            Err(RegisterHomeDecodeError::InvalidMachineId(0))
        );
    }
}
