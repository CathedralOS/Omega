use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::MachineId;

use super::*;
use crate::{
    AllocationLegalityIdentity, FunctionRegisterHomes, LiveRangeIdentity, RegisterHomeDecodeError,
    RegisterHomePlan, VirtualRegisterHome,
};

type Mutation = fn(&mut RegisterHomePlan);

fn plan() -> RegisterHomePlan {
    RegisterHomePlan {
        legality: AllocationLegalityIdentity::from_bytes([1; 32]),
        ranges: LiveRangeIdentity::from_bytes([2; 32]),
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
fn version_six_bytes_survive_representation_ownership_changes() {
    // Fixed independently from the v6 field order, not from the encoder under test.
    let identity = [
        0xe0, 0x14, 0x35, 0x59, 0x35, 0x00, 0xd5, 0x03, 0x6e, 0x3e, 0xad, 0x06, 0xff, 0xc8, 0xc9,
        0xcb, 0x96, 0xb1, 0x1b, 0x73, 0x9f, 0x35, 0x01, 0xc2, 0xd2, 0x65, 0xcd, 0x4e, 0x8b, 0x55,
        0x9a, 0xba,
    ];
    let mut expected = b"OMGRAH\0\0\x06\0\0\0".to_vec();
    expected.extend_from_slice(&identity);
    for value in [1, 2, 3, 4] {
        expected.extend_from_slice(&[value; 32]);
    }
    // Function count, machine, assignment count, then the exact home tuple.
    for value in [1_u64, 1, 1] {
        expected.extend_from_slice(&value.to_le_bytes());
    }
    expected.extend_from_slice(&[0, 0, 0, 0, 1, 0, 2, 0]);
    // Structural-unit function count, machine, and empty assignment list.
    for value in [1_u64, 2, 0] {
        expected.extend_from_slice(&value.to_le_bytes());
    }
    assert_eq!(expected.len(), 228);
    assert_eq!(register_home_identity(&plan()).bytes(), identity);
    assert_eq!(plan().encode(), expected);
    assert_eq!(RegisterHomePlan::decode(&expected), Ok(plan()));
}

#[test]
fn identity_binds_every_home_domain() {
    let baseline = register_home_identity(&plan());
    assert_eq!(baseline, register_home_identity(&plan()));
    let mutations: Vec<Mutation> = vec![
        |plan| plan.legality = AllocationLegalityIdentity::from_bytes([4; 32]),
        |plan| plan.ranges = LiveRangeIdentity::from_bytes([5; 32]),
        |plan| plan.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([6; 32]),
        |plan| {
            plan.allocator_availability = crate::AllocatorAvailabilityIdentity::from_bytes([7; 32])
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
