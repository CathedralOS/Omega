//! Independent byte-level control and stack replay for forward scalar graphs.
//! The checker decodes transfers; it never reruns selection or layout.

use std::collections::BTreeMap;

use machine_code::{
    FunctionFragmentConditionalBranchPredicate as Predicate, ScalarControlBlockEvidence as Block,
    ScalarControlTerminatorEvidence as Terminator, ScalarDirectConditionalBranchEvidence,
    ScalarStackEvidence, SemanticCodeAttribution, SemanticCodeSite,
};
use semantic_vocabulary::MachineId;
use target::Architecture;

use super::{ObjectError, ObjectScalarStack};

/// Recover the current physical graph from exact semantic edge intervals.
/// Fallthrough is a zero-width edge at the end of its conditional instruction;
/// every other edge owns one complete transfer instruction. Block boundaries
/// come from these instructions, not a producer-supplied topology description.
pub fn reconstruct_scalar_control_flow<'row>(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    attributions: impl Iterator<Item = &'row SemanticCodeAttribution>,
) -> Result<Vec<Block>, ObjectError> {
    let mut edges = attributions
        .filter(|row| matches!(row.site, SemanticCodeSite::Edge(_)))
        .collect::<Vec<_>>();
    edges.sort_by_key(|row| (row.code_offset, row.byte_count));
    let invalid = |offset| ObjectError::InvalidScalarConditionalEvidence { machine, offset };
    let mut blocks = Vec::new();
    let mut start = 0;
    let mut fallthroughs = Vec::new();
    for row in edges.iter().filter(|row| row.byte_count != 0) {
        if row.code_offset < start {
            return Err(invalid(row.code_offset));
        }
        let terminator = decode_transfer(
            architecture,
            machine,
            bytes,
            row.code_offset,
            row.byte_count,
        )?;
        let end = row
            .code_offset
            .checked_add(row.byte_count)
            .ok_or_else(|| invalid(start))?;
        if let Terminator::Conditional(branch) = &terminator {
            let matches = edges
                .iter()
                .filter(|other| {
                    other.byte_count == 0
                        && other.code_offset == branch.fallthrough_offset
                        && other.operation_ordinal == row.operation_ordinal
                })
                .count();
            if matches != 1 {
                return Err(invalid(row.code_offset));
            }
            fallthroughs.push((branch.fallthrough_offset, row.operation_ordinal));
        }
        blocks.push(Block {
            offset: start,
            byte_count: end - start,
            terminator,
        });
        start = end;
    }
    if start != bytes.len()
        || blocks.is_empty()
        || edges
            .iter()
            .filter(|row| row.byte_count == 0)
            .any(|row| !fallthroughs.contains(&(row.code_offset, row.operation_ordinal)))
    {
        return Err(invalid(start));
    }
    validate_topology(architecture, machine, bytes, &blocks)?;
    Ok(blocks)
}

fn decode_transfer(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    offset: usize,
    byte_count: usize,
) -> Result<Terminator, ObjectError> {
    let invalid = || ObjectError::InvalidScalarConditionalEvidence { machine, offset };
    let end = offset.checked_add(byte_count).ok_or_else(invalid)?;
    let instruction = bytes.get(offset..end).ok_or_else(invalid)?;
    let (predicate, displacement, base) = match architecture {
        Architecture::X86_64 => match instruction {
            [0xc3] => return Ok(Terminator::Return { offset, byte_count }),
            [0xeb, displacement] => (None, i64::from(*displacement as i8), end),
            [0xe9, displacement @ ..] if displacement.len() == 4 => (
                None,
                i64::from(i32::from_le_bytes(
                    displacement.try_into().map_err(|_| invalid())?,
                )),
                end,
            ),
            [opcode, displacement] if [0x75, 0x72, 0x7c].contains(opcode) => {
                let predicate = match opcode {
                    0x75 => Predicate::NonZeroV1,
                    0x72 => Predicate::U64LessThanV1,
                    _ => Predicate::I64LessThanV1,
                };
                (Some(predicate), i64::from(*displacement as i8), end)
            }
            [0x0f, opcode, displacement @ ..]
                if [0x85, 0x82, 0x8c].contains(opcode) && displacement.len() == 4 =>
            {
                let predicate = match opcode {
                    0x85 => Predicate::NonZeroV1,
                    0x82 => Predicate::U64LessThanV1,
                    _ => Predicate::I64LessThanV1,
                };
                (
                    Some(predicate),
                    i64::from(i32::from_le_bytes(
                        displacement.try_into().map_err(|_| invalid())?,
                    )),
                    end,
                )
            }
            _ => return Err(invalid()),
        },
        Architecture::Aarch64 => {
            if !offset.is_multiple_of(4) || byte_count != 4 {
                return Err(invalid());
            }
            let word = u32::from_le_bytes(instruction.try_into().map_err(|_| invalid())?);
            if word == 0xd65f_03c0 {
                return Ok(Terminator::Return { offset, byte_count });
            }
            if word & 0xfc00_0000 == 0x1400_0000 {
                (
                    None,
                    i64::from(((word & 0x03ff_ffff) as i32) << 6 >> 6) * 4,
                    offset,
                )
            } else {
                let predicate = match word & 0xff00_001f {
                    0x5400_0001 => Predicate::NonZeroV1,
                    0x5400_0003 => Predicate::U64LessThanV1,
                    0x5400_000b => Predicate::I64LessThanV1,
                    _ if word & 0xff00_0000 == 0xb500_0000 => Predicate::NonZeroV1,
                    _ => return Err(invalid()),
                };
                (
                    Some(predicate),
                    i64::from((((word >> 5) & 0x7ffff) as i32) << 13 >> 13) * 4,
                    offset,
                )
            }
        }
    };
    let target_offset = i64::try_from(base)
        .ok()
        .and_then(|base| base.checked_add(displacement))
        .and_then(|target| usize::try_from(target).ok())
        .ok_or_else(invalid)?;
    if let Some(predicate) = predicate {
        Ok(Terminator::Conditional(
            ScalarDirectConditionalBranchEvidence {
                predicate,
                branch_offset: offset,
                branch_byte_count: byte_count,
                taken_offset: target_offset,
                fallthrough_offset: end,
            },
        ))
    } else {
        Ok(Terminator::Jump {
            offset,
            byte_count,
            target_offset,
        })
    }
}

fn validate_topology(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    blocks: &[Block],
) -> Result<(), ObjectError> {
    let invalid = |offset| ObjectError::InvalidScalarConditionalEvidence { machine, offset };
    let mut end = 0;
    let mut reachable = vec![false; blocks.len()];
    if let Some(entry) = reachable.first_mut() {
        *entry = true;
    } else {
        return Err(invalid(0));
    }
    for (index, block) in blocks.iter().enumerate() {
        let (offset, byte_count, targets) = match &block.terminator {
            Terminator::Return { offset, byte_count } => (*offset, *byte_count, Vec::new()),
            Terminator::Jump {
                offset,
                byte_count,
                target_offset,
            } => (*offset, *byte_count, vec![*target_offset]),
            Terminator::Conditional(branch) => (
                branch.branch_offset,
                branch.branch_byte_count,
                vec![branch.taken_offset, branch.fallthrough_offset],
            ),
        };
        if !reachable[index]
            || block.offset != end
            || offset < block.offset
            || block.offset.checked_add(block.byte_count) != offset.checked_add(byte_count)
            || decode_transfer(architecture, machine, bytes, offset, byte_count)?
                != block.terminator
        {
            return Err(invalid(block.offset));
        }
        end = block
            .offset
            .checked_add(block.byte_count)
            .ok_or_else(|| invalid(block.offset))?;
        for target in targets {
            let Some(next) = blocks
                .iter()
                .position(|candidate| candidate.offset == target)
            else {
                return Err(invalid(target));
            };
            if next <= index {
                return Err(invalid(target));
            }
            reachable[next] = true;
        }
    }
    if end != bytes.len() {
        return Err(invalid(end));
    }
    Ok(())
}

pub(super) fn validate_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    evidence: &ScalarStackEvidence,
    blocks: &[Block],
) -> Result<ObjectScalarStack, ObjectError> {
    validate_topology(architecture, machine, bytes, blocks)?;
    if evidence.cleanup_preservation.is_some()
        || evidence.stack_alignment != 16
        || evidence
            .mutations
            .windows(2)
            .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(ObjectError::InvalidScalarStackEvidence { machine, offset: 0 });
    }
    let mut claimed = evidence
        .mutations
        .iter()
        .map(|row| (row.offset, *row))
        .collect::<BTreeMap<_, _>>();
    let mut incoming = vec![None; blocks.len()];
    incoming[0] = Some(0);
    let mut peak = 0;
    for (index, block) in blocks.iter().enumerate() {
        let depth = incoming[index].ok_or(ObjectError::InvalidScalarStackEvidence {
            machine,
            offset: block.offset,
        })?;
        let (end, returned, targets) = match &block.terminator {
            Terminator::Return { .. } => (block.offset + block.byte_count, true, Vec::new()),
            Terminator::Jump {
                offset,
                target_offset,
                ..
            } => (*offset, false, vec![*target_offset]),
            Terminator::Conditional(branch) => (
                branch.branch_offset,
                false,
                vec![branch.taken_offset, branch.fallthrough_offset],
            ),
        };
        let outgoing = super::scalar_stack_regions::replay_region(
            architecture,
            machine,
            bytes,
            block.offset,
            end,
            depth,
            returned,
            &mut claimed,
            &mut peak,
        )?;
        if returned && outgoing != 0 {
            return Err(ObjectError::MissingBalancedScalarReturn(machine));
        }
        for target in targets {
            let next = blocks
                .iter()
                .position(|candidate| candidate.offset == target)
                .expect("topology validated");
            if incoming[next].is_some_and(|depth| depth != outgoing) {
                return Err(ObjectError::InvalidScalarStackEvidence {
                    machine,
                    offset: target,
                });
            }
            incoming[next] = Some(outgoing);
        }
    }
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(ObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    Ok(ObjectScalarStack {
        local_peak_bytes: peak,
        stack_alignment: 16,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::{ScalarControlFlowEvidence, ScalarStackMutation, ScalarStackMutationKind};
    use semantic_vocabulary::EdgeId;

    fn fixture(architecture: Architecture) -> (Vec<u8>, Vec<SemanticCodeAttribution>) {
        let (bytes, coordinates) = match architecture {
            Architecture::X86_64 => (
                vec![
                    0x48, 0x39, 0xf7, 0x75, 10, 0xb8, 0, 0, 0, 0, 0xe9, 10, 0, 0, 0, 0xb8, 7, 0, 0,
                    0, 0xe9, 0, 0, 0, 0, 0xc3,
                ],
                [(1, 3, 2), (1, 5, 0), (4, 10, 5), (6, 20, 5), (2, 25, 1)],
            ),
            Architecture::Aarch64 => (
                [
                    0xeb01_001f_u32,
                    0x5400_0061,
                    0xd280_0000,
                    0x1400_0003,
                    0xd280_00e0,
                    0x1400_0001,
                    0xd65f_03c0,
                ]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
                [(1, 4, 4), (1, 8, 0), (4, 12, 4), (6, 20, 4), (2, 24, 4)],
            ),
        };
        let rows = coordinates
            .into_iter()
            .enumerate()
            .map(
                |(index, (operation_ordinal, code_offset, byte_count))| SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(EdgeId::new(index as u64 + 1).unwrap()),
                    operation_ordinal,
                    code_offset,
                    byte_count,
                },
            )
            .collect();
        (bytes, rows)
    }

    fn stack(blocks: Vec<Block>) -> ScalarStackEvidence {
        ScalarStackEvidence {
            stack_alignment: 16,
            mutations: Vec::new(),
            control_flow: machine_code::ScalarControlFlowEvidence::Acyclic { blocks },
            cleanup_preservation: None,
        }
    }

    #[test]
    fn common_return_graph_is_recovered_from_real_transfers() {
        for architecture in [Architecture::X86_64, Architecture::Aarch64] {
            let machine = MachineId::new(1).unwrap();
            let (bytes, rows) = fixture(architecture);
            let blocks =
                reconstruct_scalar_control_flow(architecture, machine, &bytes, rows.iter())
                    .unwrap();
            assert_eq!(blocks.len(), 4);
            assert!(matches!(blocks[0].terminator, Terminator::Conditional(_)));
            assert!(matches!(blocks[1].terminator, Terminator::Jump { .. }));
            assert!(matches!(blocks[2].terminator, Terminator::Jump { .. }));
            assert!(matches!(blocks[3].terminator, Terminator::Return { .. }));
            validate_stack(
                architecture,
                machine,
                &bytes,
                &stack(blocks.clone()),
                &blocks,
            )
            .unwrap();
            let mut padded = rows.clone();
            let mut extra = rows[1];
            extra.operation_ordinal += 1;
            extra.site = SemanticCodeSite::Edge(EdgeId::new(99).unwrap());
            padded.push(extra);
            assert!(
                reconstruct_scalar_control_flow(architecture, machine, &bytes, padded.iter())
                    .is_err()
            );
            for index in 0..rows.len() {
                let mut missing = rows.clone();
                missing.remove(index);
                if let Ok(incomplete) =
                    reconstruct_scalar_control_flow(architecture, machine, &bytes, missing.iter())
                {
                    assert!(
                        validate_stack(
                            architecture,
                            machine,
                            &bytes,
                            &stack(incomplete.clone()),
                            &incomplete
                        )
                        .is_err()
                    );
                }
            }
            let mut stale = blocks.clone();
            let Terminator::Jump { target_offset, .. } = &mut stale[1].terminator else {
                unreachable!()
            };
            *target_offset += 1;
            assert!(
                validate_stack(architecture, machine, &bytes, &stack(stale.clone()), &stale)
                    .is_err()
            );
            let mut backward = bytes.clone();
            match architecture {
                Architecture::X86_64 => backward[11..15].copy_from_slice(&(-15_i32).to_le_bytes()),
                Architecture::Aarch64 => {
                    backward[12..16].copy_from_slice(&0x17ff_fffd_u32.to_le_bytes())
                }
            }
            assert!(
                reconstruct_scalar_control_flow(architecture, machine, &backward, rows.iter())
                    .is_err()
            );
        }
    }

    #[test]
    fn convergence_requires_equal_incoming_stack_depths() {
        // One arm pushes, the other does not; a common pop cannot make this
        // function valid. Numeric global balance is not a path proof.
        let machine = MachineId::new(1).unwrap();
        let bytes = [0x75, 3, 0x50, 0xeb, 2, 0xeb, 0, 0x58, 0xc3];
        let blocks = vec![
            Block {
                offset: 0,
                byte_count: 2,
                terminator: Terminator::Conditional(ScalarDirectConditionalBranchEvidence {
                    predicate: Predicate::NonZeroV1,
                    branch_offset: 0,
                    branch_byte_count: 2,
                    taken_offset: 5,
                    fallthrough_offset: 2,
                }),
            },
            Block {
                offset: 2,
                byte_count: 3,
                terminator: Terminator::Jump {
                    offset: 3,
                    byte_count: 2,
                    target_offset: 7,
                },
            },
            Block {
                offset: 5,
                byte_count: 2,
                terminator: Terminator::Jump {
                    offset: 5,
                    byte_count: 2,
                    target_offset: 7,
                },
            },
            Block {
                offset: 7,
                byte_count: 2,
                terminator: Terminator::Return {
                    offset: 8,
                    byte_count: 1,
                },
            },
        ];
        let mut evidence = stack(blocks.clone());
        evidence.mutations = vec![
            ScalarStackMutation {
                offset: 2,
                byte_count: 1,
                kind: ScalarStackMutationKind::X86Push,
            },
            ScalarStackMutation {
                offset: 7,
                byte_count: 1,
                kind: ScalarStackMutationKind::X86Pop,
            },
        ];
        assert!(validate_stack(Architecture::X86_64, machine, &bytes, &evidence, &blocks).is_err());
        let ScalarControlFlowEvidence::Acyclic { blocks } = &evidence.control_flow else {
            unreachable!()
        };
        assert_eq!(blocks.len(), 4);
    }
}
