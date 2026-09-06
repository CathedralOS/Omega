//! Independent replay of one current predicate-bearing scalar decision.
//! The branch decoder admits actual selected target encodings, not legacy
//! true-first Boolean templates. Stack depth at the decision reaches both arms.

use std::collections::BTreeMap;

use machine_code::{
    FunctionFragmentConditionalBranchPredicate as Predicate, ScalarDirectConditionalBranchEvidence,
    ScalarStackEvidence, ScalarStackMutation,
};
use semantic_vocabulary::MachineId;
use target::Architecture;

use super::scalar_stack_mutation::{
    aarch64_control_flow_instruction, aarch64_unsupported_sp_write, replay_scalar_mutation,
    validate_aarch64_scalar_mutation, validate_x86_scalar_mutation,
};
use super::unit_stack::aarch64_stack_adjustment_at;
use super::{ObjectError, ObjectScalarStack};

/// Independently check the final branch instruction and its exact physical
/// successors. Public native publication replay uses the same byte predicate;
/// it does not invoke instruction selection, layout or branch encoding.
pub fn validate_direct_scalar_conditional_branch(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    branch: &ScalarDirectConditionalBranchEvidence,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidScalarConditionalEvidence {
        machine,
        offset: branch.branch_offset,
    };
    let end = branch
        .branch_offset
        .checked_add(branch.branch_byte_count)
        .ok_or_else(invalid)?;
    if end != branch.fallthrough_offset
        || end >= branch.taken_offset
        || branch.taken_offset >= bytes.len()
    {
        return Err(invalid());
    }
    let instruction = bytes.get(branch.branch_offset..end).ok_or_else(invalid)?;
    let displacement = match architecture {
        Architecture::X86_64 => {
            let (short, near) = match branch.predicate {
                Predicate::NonZeroV1 => (0x75, 0x85),
                Predicate::U64LessThanV1 => (0x72, 0x82),
                Predicate::I64LessThanV1 => (0x7c, 0x8c),
            };
            match instruction {
                [opcode, displacement] if *opcode == short => i64::from(*displacement as i8),
                [0x0f, opcode, displacement @ ..] if *opcode == near && displacement.len() == 4 => {
                    i64::from(i32::from_le_bytes(
                        displacement.try_into().map_err(|_| invalid())?,
                    ))
                }
                _ => return Err(invalid()),
            }
        }
        Architecture::Aarch64 => {
            if !branch.branch_offset.is_multiple_of(4) || instruction.len() != 4 {
                return Err(invalid());
            }
            let encoded = u32::from_le_bytes(instruction.try_into().map_err(|_| invalid())?);
            let condition = match branch.predicate {
                Predicate::NonZeroV1 => 1,
                Predicate::U64LessThanV1 => 3,
                Predicate::I64LessThanV1 => 11,
            };
            let conditional = encoded & 0xff00_001f == 0x5400_0000 | condition;
            let fused_nonzero =
                branch.predicate == Predicate::NonZeroV1 && encoded & 0xff00_0000 == 0xb500_0000;
            if !conditional && !fused_nonzero {
                return Err(invalid());
            }
            let immediate = ((encoded >> 5) & 0x7ffff) as i32;
            i64::from((immediate << 13 >> 13) * 4)
        }
    };
    let base = match architecture {
        Architecture::X86_64 => end,
        Architecture::Aarch64 => branch.branch_offset,
    };
    let target = i64::try_from(base)
        .ok()
        .and_then(|base| base.checked_add(displacement))
        .and_then(|target| usize::try_from(target).ok());
    if target != Some(branch.taken_offset) {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn validate_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    evidence: &ScalarStackEvidence,
    branch: &ScalarDirectConditionalBranchEvidence,
) -> Result<ObjectScalarStack, ObjectError> {
    validate_direct_scalar_conditional_branch(architecture, machine, bytes, branch)?;
    if evidence.cleanup_preservation.is_some()
        || evidence.stack_alignment != 16
        || evidence
            .mutations
            .windows(2)
            .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(ObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: branch.branch_offset,
        });
    }
    let mut claimed = evidence
        .mutations
        .iter()
        .map(|row| (row.offset, *row))
        .collect::<BTreeMap<_, _>>();
    let mut peak = 0;
    let depth = replay_region(
        architecture,
        machine,
        bytes,
        0,
        branch.branch_offset,
        0,
        false,
        &mut claimed,
        &mut peak,
    )?;
    for (start, end) in [
        (branch.fallthrough_offset, branch.taken_offset),
        (branch.taken_offset, bytes.len()),
    ] {
        if replay_region(
            architecture,
            machine,
            bytes,
            start,
            end,
            depth,
            true,
            &mut claimed,
            &mut peak,
        )? != 0
        {
            return Err(ObjectError::MissingBalancedScalarReturn(machine));
        }
    }
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(ObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    Ok(ObjectScalarStack {
        local_peak_bytes: peak,
        stack_alignment: evidence.stack_alignment,
    })
}

#[allow(clippy::too_many_arguments)]
fn replay_region(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    mut depth: u32,
    require_return: bool,
    claimed: &mut BTreeMap<usize, ScalarStackMutation>,
    peak: &mut u32,
) -> Result<u32, ObjectError> {
    let mut saw_return = false;
    match architecture {
        Architecture::X86_64 => {
            let mut decoder = iced_x86::Decoder::with_ip(
                64,
                &bytes[start..end],
                start as u64,
                iced_x86::DecoderOptions::NONE,
            );
            let mut information = iced_x86::InstructionInfoFactory::new();
            while decoder.can_decode() {
                let instruction = decoder.decode();
                let offset =
                    usize::try_from(instruction.ip()).expect("function-relative instruction");
                if instruction.is_invalid() {
                    return Err(ObjectError::InvalidScalarInstructionEncoding { machine, offset });
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Ret {
                    if !require_return
                        || saw_return
                        || offset.checked_add(instruction.len()) != Some(end)
                        || instruction.code() != iced_x86::Code::Retnq
                    {
                        return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
                    }
                    saw_return = true;
                    continue;
                }
                if instruction.flow_control() != iced_x86::FlowControl::Next {
                    return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
                }
                let mutation = matches!(
                    instruction.mnemonic(),
                    iced_x86::Mnemonic::Push | iced_x86::Mnemonic::Pop
                ) || matches!(
                    instruction.mnemonic(),
                    iced_x86::Mnemonic::Add | iced_x86::Mnemonic::Sub | iced_x86::Mnemonic::Lea
                ) && instruction.op0_register() == iced_x86::Register::RSP;
                if mutation {
                    // The retained push/pop kinds mean exactly eight bytes.
                    // POP RSP instead replaces the stack pointer with a loaded
                    // value and is not a numeric release of the active frame.
                    let exact_push_pop = match instruction.mnemonic() {
                        iced_x86::Mnemonic::Push => {
                            matches!(
                                instruction.code(),
                                iced_x86::Code::Push_r64 | iced_x86::Code::Push_rm64
                            ) && instruction.op0_kind() == iced_x86::OpKind::Register
                        }
                        iced_x86::Mnemonic::Pop => {
                            matches!(
                                instruction.code(),
                                iced_x86::Code::Pop_r64 | iced_x86::Code::Pop_rm64
                            ) && instruction.op0_kind() == iced_x86::OpKind::Register
                                && instruction.op0_register() != iced_x86::Register::RSP
                        }
                        _ => true,
                    };
                    if !exact_push_pop {
                        return Err(ObjectError::UnsupportedScalarStackMutation {
                            machine,
                            offset,
                        });
                    }
                    let row = claimed
                        .remove(&offset)
                        .ok_or(ObjectError::UnclaimedScalarStackMutation { machine, offset })?;
                    if matches!(row.kind,
                        machine_code::ScalarStackMutationKind::Allocate { byte_size }
                        | machine_code::ScalarStackMutationKind::Release { byte_size }
                        | machine_code::ScalarStackMutationKind::X86ReleasePreservingFlags { byte_size }
                        if byte_size > i32::MAX as u32)
                    {
                        // Immediate/displacement32 is sign-extended in these
                        // 64-bit encodings; it cannot claim a positive larger
                        // numeric allocation or release.
                        return Err(ObjectError::InvalidScalarStackEvidence { machine, offset });
                    }
                    validate_x86_scalar_mutation(machine, bytes, &instruction, row)?;
                    replay_scalar_mutation(machine, offset, row.kind, &mut depth, peak)?;
                } else if information
                    .info(&instruction)
                    .used_registers()
                    .iter()
                    .any(|register| {
                        matches!(
                            register.register(),
                            iced_x86::Register::RSP
                                | iced_x86::Register::ESP
                                | iced_x86::Register::SP
                                | iced_x86::Register::SPL
                        ) && matches!(
                            register.access(),
                            iced_x86::OpAccess::Write
                                | iced_x86::OpAccess::CondWrite
                                | iced_x86::OpAccess::ReadWrite
                                | iced_x86::OpAccess::ReadCondWrite
                        )
                    })
                {
                    return Err(ObjectError::UnsupportedScalarStackMutation { machine, offset });
                }
            }
        }
        Architecture::Aarch64 => {
            if !start.is_multiple_of(4) || !end.is_multiple_of(4) {
                return Err(ObjectError::InvalidScalarConditionalEvidence {
                    machine,
                    offset: start,
                });
            }
            for offset in (start..end).step_by(4) {
                let encoded = u32::from_le_bytes(
                    bytes[offset..offset + 4]
                        .try_into()
                        .expect("instruction word"),
                );
                if encoded == 0xd65f_03c0 {
                    if !require_return || saw_return || offset + 4 != end {
                        return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
                    }
                    saw_return = true;
                    continue;
                }
                if aarch64_control_flow_instruction(encoded) {
                    return Err(ObjectError::NonLinearScalarControlFlow { machine, offset });
                }
                if aarch64_stack_adjustment_at(bytes, offset) {
                    let row = claimed
                        .remove(&offset)
                        .ok_or(ObjectError::UnclaimedScalarStackMutation { machine, offset })?;
                    validate_aarch64_scalar_mutation(machine, encoded, row)?;
                    replay_scalar_mutation(machine, offset, row.kind, &mut depth, peak)?;
                } else if aarch64_unsupported_sp_write(encoded)
                    // Non-flag-setting ADD/SUB immediate or extended-register
                    // forms can replace SP/WSP from any source register. Only
                    // the exact full-width SP adjustments above are admitted.
                    || encoded & 0x3f00_001f == 0x1100_001f
                    || encoded & 0x3f20_001f == 0x0b20_001f
                {
                    return Err(ObjectError::UnsupportedScalarStackMutation { machine, offset });
                }
            }
        }
    }
    if saw_return != require_return {
        return Err(ObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok(depth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::{ScalarControlFlowEvidence, ScalarStackMutationKind};

    fn framed(
        architecture: Architecture,
    ) -> (
        Vec<u8>,
        ScalarStackEvidence,
        ScalarDirectConditionalBranchEvidence,
    ) {
        let (bytes, branch, size, releases) = match architecture {
            Architecture::X86_64 => (
                vec![
                    0x48, 0x83, 0xec, 8, 0x48, 0x85, 0xc0, 0x0f, 0x85, 10, 0, 0, 0, 0xb8, 1, 0, 0,
                    0, 0x48, 0x83, 0xc4, 8, 0xc3, 0xb8, 2, 0, 0, 0, 0x48, 0x83, 0xc4, 8, 0xc3,
                ],
                ScalarDirectConditionalBranchEvidence {
                    predicate: Predicate::NonZeroV1,
                    branch_offset: 7,
                    branch_byte_count: 6,
                    taken_offset: 23,
                    fallthrough_offset: 13,
                },
                8,
                [18, 28],
            ),
            Architecture::Aarch64 => (
                vec![
                    0xff, 0x43, 0x00, 0xd1, 0x1f, 0x00, 0x00, 0xf1, 0x81, 0x00, 0x00, 0x54, 0x20,
                    0x00, 0x80, 0xd2, 0xff, 0x43, 0x00, 0x91, 0xc0, 0x03, 0x5f, 0xd6, 0x40, 0x00,
                    0x80, 0xd2, 0xff, 0x43, 0x00, 0x91, 0xc0, 0x03, 0x5f, 0xd6,
                ],
                ScalarDirectConditionalBranchEvidence {
                    predicate: Predicate::NonZeroV1,
                    branch_offset: 8,
                    branch_byte_count: 4,
                    taken_offset: 24,
                    fallthrough_offset: 12,
                },
                16,
                [16, 28],
            ),
        };
        let evidence = ScalarStackEvidence {
            mutations: vec![
                ScalarStackMutation {
                    offset: 0,
                    byte_count: 4,
                    kind: ScalarStackMutationKind::Allocate { byte_size: size },
                },
                ScalarStackMutation {
                    offset: releases[0],
                    byte_count: 4,
                    kind: ScalarStackMutationKind::Release { byte_size: size },
                },
                ScalarStackMutation {
                    offset: releases[1],
                    byte_count: 4,
                    kind: ScalarStackMutationKind::Release { byte_size: size },
                },
            ],
            control_flow: ScalarControlFlowEvidence::DirectConditional { branch },
            stack_alignment: 16,
            cleanup_preservation: None,
        };
        (bytes, evidence, branch)
    }

    #[test]
    fn selected_conditional_replays_one_prefix_frame_on_both_returns() {
        let machine = MachineId::new(1).unwrap();
        for architecture in [Architecture::X86_64, Architecture::Aarch64] {
            let (bytes, evidence, branch) = framed(architecture);
            let stack = validate_stack(architecture, machine, &bytes, &evidence, &branch).unwrap();
            assert_eq!(
                stack.local_peak_bytes,
                if architecture == Architecture::X86_64 {
                    8
                } else {
                    16
                }
            );
            assert_eq!(stack.stack_alignment, 16);
        }
    }

    #[test]
    fn selected_conditional_rejects_direction_targets_and_stale_stack_rows() {
        let machine = MachineId::new(1).unwrap();
        for architecture in [Architecture::X86_64, Architecture::Aarch64] {
            let (bytes, evidence, branch) = framed(architecture);
            for corruption in 0..8 {
                let mut bytes = bytes.clone();
                let mut evidence = evidence.clone();
                let mut branch = branch;
                match corruption {
                    0 => branch.predicate = Predicate::U64LessThanV1,
                    1 => branch.taken_offset += 1,
                    2 => branch.fallthrough_offset += 1,
                    3 => {
                        evidence.mutations.pop();
                    }
                    4 => {
                        evidence.mutations[0].kind =
                            ScalarStackMutationKind::Allocate { byte_size: 32 }
                    }
                    5 => match architecture {
                        Architecture::X86_64 => bytes[branch.branch_offset + 1] = 0x84,
                        Architecture::Aarch64 => bytes[branch.branch_offset] ^= 1,
                    },
                    6 => {
                        let release = evidence.mutations.pop().unwrap();
                        let replacement: &[u8] = match architecture {
                            Architecture::X86_64 => &[0x90, 0x90, 0x90, 0x90],
                            Architecture::Aarch64 => &[0x1f, 0x20, 0x03, 0xd5],
                        };
                        bytes[release.offset..release.offset + 4].copy_from_slice(replacement);
                    }
                    7 => match architecture {
                        Architecture::X86_64 => bytes[4..7].copy_from_slice(&[0xff, 0xd0, 0x90]),
                        Architecture::Aarch64 => {
                            bytes[4..8].copy_from_slice(&[0x00, 0x00, 0x3f, 0xd6])
                        }
                    },
                    _ => unreachable!(),
                }
                assert!(
                    validate_stack(architecture, machine, &bytes, &evidence, &branch).is_err(),
                    "{architecture:?} corruption {corruption}"
                );
            }
        }
    }

    #[test]
    fn selected_predicates_accept_short_near_and_fused_encodings() {
        let machine = MachineId::new(1).unwrap();
        for (predicate, short, near, condition) in [
            (Predicate::NonZeroV1, 0x75, 0x85, 1),
            (Predicate::U64LessThanV1, 0x72, 0x82, 3),
            (Predicate::I64LessThanV1, 0x7c, 0x8c, 11),
        ] {
            for bytes in [
                vec![short, 1, 0xc3, 0xc3],
                vec![0x0f, near, 1, 0, 0, 0, 0xc3, 0xc3],
            ] {
                let branch = ScalarDirectConditionalBranchEvidence {
                    predicate,
                    branch_offset: 0,
                    branch_byte_count: bytes.len() - 2,
                    fallthrough_offset: bytes.len() - 2,
                    taken_offset: bytes.len() - 1,
                };
                validate_direct_scalar_conditional_branch(
                    Architecture::X86_64,
                    machine,
                    &bytes,
                    &branch,
                )
                .unwrap();
            }
            let mut bytes = (0x5400_0040_u32 | condition).to_le_bytes().to_vec();
            bytes.extend_from_slice(&[0xc0, 0x03, 0x5f, 0xd6, 0xc0, 0x03, 0x5f, 0xd6]);
            let branch = ScalarDirectConditionalBranchEvidence {
                predicate,
                branch_offset: 0,
                branch_byte_count: 4,
                fallthrough_offset: 4,
                taken_offset: 8,
            };
            validate_direct_scalar_conditional_branch(
                Architecture::Aarch64,
                machine,
                &bytes,
                &branch,
            )
            .unwrap();
        }
        let bytes = [
            0x40, 0x00, 0x00, 0xb5, 0xc0, 0x03, 0x5f, 0xd6, 0xc0, 0x03, 0x5f, 0xd6,
        ];
        let branch = ScalarDirectConditionalBranchEvidence {
            predicate: Predicate::NonZeroV1,
            branch_offset: 0,
            branch_byte_count: 4,
            taken_offset: 8,
            fallthrough_offset: 4,
        };
        validate_direct_scalar_conditional_branch(Architecture::Aarch64, machine, &bytes, &branch)
            .unwrap();
    }

    fn push_pop_tree(
        push: &[u8],
        fallthrough_pop: &[u8],
        taken_pop: &[u8],
    ) -> (
        Vec<u8>,
        ScalarStackEvidence,
        ScalarDirectConditionalBranchEvidence,
    ) {
        let branch_offset = push.len() + 2;
        let fallthrough_offset = branch_offset + 2;
        let taken_offset = fallthrough_offset + fallthrough_pop.len() + 1;
        let branch = ScalarDirectConditionalBranchEvidence {
            predicate: Predicate::NonZeroV1,
            branch_offset,
            branch_byte_count: 2,
            taken_offset,
            fallthrough_offset,
        };
        let mut bytes = push.to_vec();
        bytes.extend_from_slice(&[0x85, 0xc0, 0x75, (fallthrough_pop.len() + 1) as u8]);
        bytes.extend_from_slice(fallthrough_pop);
        bytes.push(0xc3);
        bytes.extend_from_slice(taken_pop);
        bytes.push(0xc3);
        let evidence = ScalarStackEvidence {
            mutations: vec![
                ScalarStackMutation {
                    offset: 0,
                    byte_count: push.len(),
                    kind: ScalarStackMutationKind::X86Push,
                },
                ScalarStackMutation {
                    offset: fallthrough_offset,
                    byte_count: fallthrough_pop.len(),
                    kind: ScalarStackMutationKind::X86Pop,
                },
                ScalarStackMutation {
                    offset: taken_offset,
                    byte_count: taken_pop.len(),
                    kind: ScalarStackMutationKind::X86Pop,
                },
            ],
            control_flow: ScalarControlFlowEvidence::DirectConditional { branch },
            stack_alignment: 16,
            cleanup_preservation: None,
        };
        (bytes, evidence, branch)
    }

    #[test]
    fn selected_conditional_stack_rejects_pop_rsp_and_sixteen_bit_stack_operations() {
        let machine = MachineId::new(1).unwrap();
        let push64 = &[0x50][..];
        let pop64 = &[0x58][..];
        let pop_stack = &[0x5c][..];
        let push16 = &[0x66, 0x50][..];
        let pop16 = &[0x66, 0x58][..];
        let (bytes, evidence, branch) = push_pop_tree(push64, pop64, pop64);
        assert_eq!(
            validate_stack(Architecture::X86_64, machine, &bytes, &evidence, &branch)
                .unwrap()
                .local_peak_bytes,
            8
        );
        for (push, fallthrough, taken) in [
            (push64, pop_stack, pop64),
            (push64, pop64, pop_stack),
            (push64, pop_stack, pop_stack),
            (push16, pop16, pop16),
            (push16, pop64, pop64),
            (push64, pop16, pop64),
            (push64, pop64, pop16),
        ] {
            let (bytes, evidence, branch) = push_pop_tree(push, fallthrough, taken);
            assert!(
                matches!(
                    validate_stack(Architecture::X86_64, machine, &bytes, &evidence, &branch),
                    Err(ObjectError::UnsupportedScalarStackMutation { .. })
                ),
                "push {push:x?}, fallthrough {fallthrough:x?}, taken {taken:x?}"
            );
        }
    }

    #[test]
    fn selected_conditional_stack_rejects_unrecorded_sp_and_wsp_replacement() {
        let machine = MachineId::new(1).unwrap();
        let (bytes, evidence, branch) = framed(Architecture::Aarch64);
        // CMP X0,#0 in the original prefix writes ZR/flags, not SP, and remains
        // valid. These replacements all have S=0 and Rd=31, hence write SP/WSP.
        for encoded in [
            0x1100_001f_u32,
            0x5100_001f,
            0x9100_001f,
            0xd100_001f,
            0x0b20_401f,
            0x4b20_401f,
            0x8b20_401f,
            0xcb20_401f,
        ] {
            let mut changed = bytes.clone();
            changed[4..8].copy_from_slice(&encoded.to_le_bytes());
            assert!(
                matches!(validate_stack(Architecture::Aarch64, machine, &changed, &evidence, &branch), Err(ObjectError::UnsupportedScalarStackMutation { machine: owner, offset: 4 } | ObjectError::UnclaimedScalarStackMutation { machine: owner, offset: 4 }) if owner == machine),
                "SP replacement {encoded:08x}"
            );
        }
    }

    #[test]
    fn selected_conditional_stack_rejects_sign_extended_negative_frame_sizes() {
        let machine = MachineId::new(1).unwrap();
        let size = 0xffff_fff0;
        let mut bytes = vec![
            0x48, 0x81, 0xec, 0xf0, 0xff, 0xff, 0xff, 0x85, 0xc0, 0x75, 8,
        ];
        for _ in 0..2 {
            bytes.extend_from_slice(&[0x48, 0x81, 0xc4, 0xf0, 0xff, 0xff, 0xff, 0xc3]);
        }
        let branch = ScalarDirectConditionalBranchEvidence {
            predicate: Predicate::NonZeroV1,
            branch_offset: 9,
            branch_byte_count: 2,
            fallthrough_offset: 11,
            taken_offset: 19,
        };
        let evidence = ScalarStackEvidence {
            mutations: vec![
                ScalarStackMutation {
                    offset: 0,
                    byte_count: 7,
                    kind: ScalarStackMutationKind::Allocate { byte_size: size },
                },
                ScalarStackMutation {
                    offset: 11,
                    byte_count: 7,
                    kind: ScalarStackMutationKind::Release { byte_size: size },
                },
                ScalarStackMutation {
                    offset: 19,
                    byte_count: 7,
                    kind: ScalarStackMutationKind::Release { byte_size: size },
                },
            ],
            control_flow: ScalarControlFlowEvidence::DirectConditional { branch },
            stack_alignment: 16,
            cleanup_preservation: None,
        };
        assert!(matches!(
            validate_stack(Architecture::X86_64, machine, &bytes, &evidence, &branch),
            Err(ObjectError::InvalidScalarStackEvidence { offset: 0, .. })
        ));
    }
}
