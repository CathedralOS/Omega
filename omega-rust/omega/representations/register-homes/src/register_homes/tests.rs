use super::register_home_identity;
use crate::CalleeSavedModificationWitness;
use crate::{
    AllocationLegalityIdentity, FunctionRegisterHomes, RegisterHomeDecodeError, RegisterHomePlan,
    VirtualRegisterHome,
};
use register_model::{
    RegisterClassId, RegisterViewId, RegisterWriteSemantics, TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{
    LiveRangeIdentity, SelectedBlockId, SelectedInstructionId, VirtualRegisterId,
};
use semantic_vocabulary::MachineId;
use sha2::Digest;

type Mutation = fn(&mut RegisterHomePlan);

fn plan() -> RegisterHomePlan {
    RegisterHomePlan {
        legality: AllocationLegalityIdentity::from_bytes([1; 32]),
        ranges: LiveRangeIdentity::from_bytes([2; 32]),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
        allocator_availability: crate::AllocatorAvailabilityIdentity::from_bytes([4; 32]),
        functions: vec![
            FunctionRegisterHomes {
                machine: MachineId::new(1).unwrap(),
                assignments: vec![VirtualRegisterHome {
                    virtual_register: VirtualRegisterId(0),
                    class: RegisterClassId(1),
                    view: RegisterViewId(2),
                }],
            },
            FunctionRegisterHomes {
                machine: MachineId::new(2).unwrap(),
                assignments: Vec::new(),
            },
        ],
    }
}

#[test]
fn retired_split_roster_wire_rejects_before_identity_comparison() {
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
    assert_eq!(
        RegisterHomePlan::decode(&expected),
        Err(RegisterHomeDecodeError::UnsupportedVersion(6))
    );
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
        |plan| {
            plan.functions.pop();
        },
        |plan| plan.functions[1].machine = MachineId::new(3).unwrap(),
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

/// The callee-saved witness encoder feeds a hasher, so its bytes are pinned by
/// hashing the expected byte table and comparing digests: a single changed
/// byte in the encoder changes the digest.
fn witness_digest(witness: CalleeSavedModificationWitness) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    crate::encode_callee_saved_modification_witness_identity(&mut hasher, witness);
    hasher.finalize().into()
}

fn digest_of(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

#[test]
fn callee_saved_modification_witness_identity_bytes_are_pinned() {
    // Expected bytes ported from the copies of this encoder that
    // `selected-instructions-to-register-homes` and `machine-emission`
    // carried before de-duplication: a variant tag, then `u32` block,
    // `u32` instruction, `u16` operand, `u32` virtual register, `u16` home
    // view and the write-semantics tag, all little-endian.
    let operand_definition = |write_semantics| CalleeSavedModificationWitness::OperandDefinition {
        block: SelectedBlockId(0x0403_0201),
        instruction: SelectedInstructionId(0x0807_0605),
        operand: 0x0a09,
        virtual_register: VirtualRegisterId(0x0e0d_0c0b),
        home_view: RegisterViewId(0x100f),
        write_semantics,
    };
    let write_semantics_tags: [(RegisterWriteSemantics, u8); 6] = [
        (RegisterWriteSemantics::ExactView, 0),
        (RegisterWriteSemantics::PreservesUnwritten, 1),
        (RegisterWriteSemantics::ZeroExtendsParent, 2),
        (RegisterWriteSemantics::ZeroExtendsWithinUnit, 3),
        (RegisterWriteSemantics::Discards, 4),
        (RegisterWriteSemantics::InstructionDefined, 5),
    ];
    for (write_semantics, tag) in write_semantics_tags {
        assert_eq!(
            witness_digest(operand_definition(write_semantics)),
            digest_of(&[
                0, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
                0x0e, 0x0f, 0x10, tag,
            ]),
            "{write_semantics:?}",
        );
    }
    assert_eq!(
        witness_digest(CalleeSavedModificationWitness::ImplicitDefinition {
            block: SelectedBlockId(0x0403_0201),
            instruction: SelectedInstructionId(0x0807_0605),
        }),
        digest_of(&[1, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]),
    );
    assert_eq!(
        witness_digest(CalleeSavedModificationWitness::ImplicitClobber {
            block: SelectedBlockId(0x0403_0201),
            instruction: SelectedInstructionId(0x0807_0605),
        }),
        digest_of(&[2, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]),
    );
}
