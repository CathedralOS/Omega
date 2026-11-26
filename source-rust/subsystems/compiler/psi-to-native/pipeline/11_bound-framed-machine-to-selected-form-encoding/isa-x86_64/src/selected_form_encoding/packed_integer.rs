use super::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, RegisterViewId, SelectedInstructionKind,
    ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
    X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
};
use selected_instructions::PackedShuffleFastPattern;
use x86_64_encoding::packed_integer::Instruction;

struct PackedSpec {
    instructions: Vec<Instruction>,
    register_reads: Vec<RegisterViewId>,
    register_writes: Vec<RegisterViewId>,
    external_operand_reads: Vec<u16>,
    external_operand_writes: Vec<u16>,
    clobbers: Vec<&'static str>,
    memory: MachineEncodedMemoryEffect,
    trap: MachineEncodedTrapBehavior,
}

fn check_canonical_alternative(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: MachineAlternativeFamily,
) -> Result<(), X86_64SelectedFormEncodingError> {
    super::request_validation::guard_canonical(physical)?;
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

fn resolve_gpr(
    physical: &ValidatedPhysicalRegisterModel,
    view_id: RegisterViewId,
) -> Result<u8, X86_64SelectedFormEncodingError> {
    let registers = super::request_validation::resolve_registers(physical, &[view_id])?;
    Ok(registers[0])
}

fn resolve_xmm(
    physical: &ValidatedPhysicalRegisterModel,
    view_id: RegisterViewId,
) -> Result<u8, X86_64SelectedFormEncodingError> {
    let view = physical
        .model()
        .view(view_id)
        .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    if view.bits != 128 || !view.allocatable {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let Some(suffix) = view.name.strip_prefix("xmm") else {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    };
    let index = suffix
        .parse::<u8>()
        .ok()
        .filter(|&i| i < 16)
        .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    Ok(index)
}

fn validate_disp(byte_offset: u32) -> Result<i32, X86_64SelectedFormEncodingError> {
    i32::try_from(byte_offset).map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)
}

fn pure_spec(
    instructions: Vec<Instruction>,
    operands: &[RegisterViewId],
    input_count: usize,
) -> PackedSpec {
    PackedSpec {
        instructions,
        register_reads: operands[..input_count].to_vec(),
        register_writes: operands[input_count..].to_vec(),
        external_operand_reads: (0..input_count as u16).collect(),
        external_operand_writes: (input_count as u16..operands.len() as u16).collect(),
        clobbers: Vec::new(),
        memory: MachineEncodedMemoryEffect::NoneV1,
        trap: MachineEncodedTrapBehavior::NeverV1,
    }
}

fn spec(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<PackedSpec, X86_64SelectedFormEncodingError> {
    match kind {
        SelectedInstructionKind::PackedAddI16x8 | SelectedInstructionKind::PackedSubI16x8 => {
            let family = if kind == SelectedInstructionKind::PackedAddI16x8 {
                MachineAlternativeFamily::PackedAddI16x8
            } else {
                MachineAlternativeFamily::PackedSubI16x8
            };
            let [left, right, destination] = super::request_validation::byte_loop_registers(
                physical,
                alternative,
                family,
                operands,
                3,
            )?;
            let op = if kind == SelectedInstructionKind::PackedAddI16x8 {
                Instruction::AddWords {
                    destination_xmm: 0,
                    source_xmm: 1,
                }
            } else {
                Instruction::SubWords {
                    destination_xmm: 0,
                    source_xmm: 1,
                }
            };
            Ok(PackedSpec {
                instructions: vec![
                    Instruction::LoadUnaligned {
                        destination_xmm: 0,
                        base: left,
                    },
                    Instruction::LoadUnaligned {
                        destination_xmm: 1,
                        base: right,
                    },
                    op,
                    Instruction::StoreUnaligned {
                        base: destination,
                        source_xmm: 0,
                    },
                ],
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                external_operand_reads: vec![0, 1, 2],
                external_operand_writes: Vec::new(),
                clobbers: vec!["xmm0", "xmm1"],
                memory: MachineEncodedMemoryEffect::PackedBinary16BytesV1 {
                    left_pointer_operand: 0,
                    right_pointer_operand: 1,
                    destination_pointer_operand: 2,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedShuffleI16x8 { lanes } => {
            let pattern = PackedShuffleFastPattern::from_lanes(lanes)
                .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
            let [left, right, destination] = super::request_validation::byte_loop_registers(
                physical,
                alternative,
                MachineAlternativeFamily::PackedShuffleI16x8,
                operands,
                3,
            )?;
            let mut instructions = vec![
                Instruction::LoadUnaligned {
                    destination_xmm: 0,
                    base: left,
                },
                Instruction::LoadUnaligned {
                    destination_xmm: 1,
                    base: right,
                },
            ];
            match pattern {
                PackedShuffleFastPattern::InterleaveLowWords => {
                    instructions.push(Instruction::UnpackLowWords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    });
                }
                PackedShuffleFastPattern::InterleaveHighWords => {
                    instructions.push(Instruction::UnpackHighWords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    });
                }
                PackedShuffleFastPattern::InterleaveLowDoublewords => {
                    instructions.push(Instruction::UnpackLowDoublewords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    });
                }
                PackedShuffleFastPattern::InterleaveHighDoublewords => {
                    instructions.push(Instruction::UnpackHighDoublewords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    });
                }
                PackedShuffleFastPattern::InterleaveLowQuadwords => {
                    instructions.push(Instruction::UnpackLowQuadwords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    });
                }
                PackedShuffleFastPattern::InterleaveHighQuadwords => {
                    instructions.push(Instruction::UnpackHighQuadwords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    });
                }
                PackedShuffleFastPattern::LowPairDuplicateLow => {
                    instructions.push(Instruction::UnpackLowDoublewords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    });
                    instructions.push(Instruction::UnpackLowQuadwords {
                        destination_xmm: 0,
                        source_xmm: 0,
                    });
                }
                PackedShuffleFastPattern::LowPairDuplicateHigh => {
                    instructions.push(Instruction::UnpackLowDoublewords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    });
                    instructions.push(Instruction::UnpackHighQuadwords {
                        destination_xmm: 0,
                        source_xmm: 0,
                    });
                }
                PackedShuffleFastPattern::HighPairDuplicateLow => {
                    instructions.push(Instruction::UnpackHighDoublewords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    });
                    instructions.push(Instruction::UnpackLowQuadwords {
                        destination_xmm: 0,
                        source_xmm: 0,
                    });
                }
                PackedShuffleFastPattern::HighPairDuplicateHigh => {
                    instructions.push(Instruction::UnpackHighDoublewords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    });
                    instructions.push(Instruction::UnpackHighQuadwords {
                        destination_xmm: 0,
                        source_xmm: 0,
                    });
                }
            }
            instructions.push(Instruction::StoreUnaligned {
                base: destination,
                source_xmm: 0,
            });
            Ok(PackedSpec {
                instructions,
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                external_operand_reads: vec![0, 1, 2],
                external_operand_writes: Vec::new(),
                clobbers: vec!["xmm0", "xmm1"],
                memory: MachineEncodedMemoryEffect::PackedBinary16BytesV1 {
                    left_pointer_operand: 0,
                    right_pointer_operand: 1,
                    destination_pointer_operand: 2,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedShuffleGeneralI16x8 { lanes } => {
            if lanes.iter().any(|&lane| lane >= 16) {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            let [left, right, destination, scratch] =
                super::request_validation::byte_loop_registers(
                    physical,
                    alternative,
                    MachineAlternativeFamily::PackedShuffleGeneralI16x8,
                    operands,
                    3,
                )?;
            let mut instructions = Vec::with_capacity(19);
            instructions.push(Instruction::LoadUnaligned {
                destination_xmm: 0,
                base: left,
            });
            instructions.push(Instruction::LoadUnaligned {
                destination_xmm: 1,
                base: right,
            });
            for (dst_lane, &src_lane) in lanes.iter().enumerate() {
                instructions.push(Instruction::ExtractWord {
                    destination_gpr: scratch,
                    source_xmm: u8::from(src_lane >= 8),
                    lane: src_lane & 7,
                });
                instructions.push(Instruction::InsertWord {
                    destination_xmm: 2,
                    source_gpr: scratch,
                    lane: dst_lane as u8,
                });
            }
            instructions.push(Instruction::StoreUnaligned {
                base: destination,
                source_xmm: 2,
            });
            Ok(PackedSpec {
                instructions,
                register_reads: operands[..3].to_vec(),
                register_writes: vec![operands[3]],
                external_operand_reads: vec![0, 1, 2],
                external_operand_writes: vec![3],
                clobbers: vec!["xmm0", "xmm1", "xmm2"],
                memory: MachineEncodedMemoryEffect::PackedBinary16BytesV1 {
                    left_pointer_operand: 0,
                    right_pointer_operand: 1,
                    destination_pointer_operand: 2,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { count } => {
            if count >= 16 {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            let [source, destination] = super::request_validation::byte_loop_registers(
                physical,
                alternative,
                MachineAlternativeFamily::PackedShiftRightArithmeticI16x8,
                operands,
                2,
            )?;
            Ok(PackedSpec {
                instructions: vec![
                    Instruction::LoadUnaligned {
                        destination_xmm: 0,
                        base: source,
                    },
                    Instruction::ShiftRightArithmeticWords { xmm: 0, count },
                    Instruction::StoreUnaligned {
                        base: destination,
                        source_xmm: 0,
                    },
                ],
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                external_operand_reads: vec![0, 1],
                external_operand_writes: Vec::new(),
                clobbers: vec!["xmm0"],
                memory: MachineEncodedMemoryEffect::PackedUnary16To16BytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedWidenU8x8ToI16x8 => {
            let [source, destination] = super::request_validation::byte_loop_registers(
                physical,
                alternative,
                MachineAlternativeFamily::PackedWidenU8x8ToI16x8,
                operands,
                2,
            )?;
            Ok(PackedSpec {
                instructions: vec![
                    Instruction::LoadQuad {
                        destination_xmm: 0,
                        base: source,
                    },
                    Instruction::XorPacked {
                        destination_xmm: 1,
                        source_xmm: 1,
                    },
                    Instruction::UnpackLowBytes {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                    Instruction::StoreUnaligned {
                        base: destination,
                        source_xmm: 0,
                    },
                ],
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                external_operand_reads: vec![0, 1],
                external_operand_writes: Vec::new(),
                clobbers: vec!["xmm0", "xmm1"],
                memory: MachineEncodedMemoryEffect::PackedUnary8To16BytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedNarrowI16x8ToU8x8Saturating => {
            let [source, destination] = super::request_validation::byte_loop_registers(
                physical,
                alternative,
                MachineAlternativeFamily::PackedNarrowI16x8ToU8x8Saturating,
                operands,
                2,
            )?;
            Ok(PackedSpec {
                instructions: vec![
                    Instruction::LoadUnaligned {
                        destination_xmm: 0,
                        base: source,
                    },
                    Instruction::PackUnsignedSaturatingWordsToBytes {
                        destination_xmm: 0,
                        source_xmm: 0,
                    },
                    Instruction::StoreQuad {
                        base: destination,
                        source_xmm: 0,
                    },
                ],
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                external_operand_reads: vec![0, 1],
                external_operand_writes: Vec::new(),
                clobbers: vec!["xmm0"],
                memory: MachineEncodedMemoryEffect::PackedUnary16To8BytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedLoadI16x4ZeroExtend => {
            let [source, destination] = super::request_validation::byte_loop_registers(
                physical,
                alternative,
                MachineAlternativeFamily::PackedLoadI16x4ZeroExtend,
                operands,
                2,
            )?;
            Ok(PackedSpec {
                instructions: vec![
                    Instruction::LoadQuad {
                        destination_xmm: 0,
                        base: source,
                    },
                    Instruction::StoreUnaligned {
                        base: destination,
                        source_xmm: 0,
                    },
                ],
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                external_operand_reads: vec![0, 1],
                external_operand_writes: Vec::new(),
                clobbers: vec!["xmm0"],
                memory: MachineEncodedMemoryEffect::PackedUnary8To16BytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedWidenU8x4ToI16x8 => {
            let [source, destination] = super::request_validation::byte_loop_registers(
                physical,
                alternative,
                MachineAlternativeFamily::PackedWidenU8x4ToI16x8,
                operands,
                2,
            )?;
            Ok(PackedSpec {
                instructions: vec![
                    Instruction::LoadDword {
                        destination_xmm: 0,
                        base: source,
                    },
                    Instruction::XorPacked {
                        destination_xmm: 1,
                        source_xmm: 1,
                    },
                    Instruction::UnpackLowBytes {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                    Instruction::StoreUnaligned {
                        base: destination,
                        source_xmm: 0,
                    },
                ],
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                external_operand_reads: vec![0, 1],
                external_operand_writes: Vec::new(),
                clobbers: vec!["xmm0", "xmm1"],
                memory: MachineEncodedMemoryEffect::PackedUnary4To16BytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating => {
            let [source, destination] = super::request_validation::byte_loop_registers(
                physical,
                alternative,
                MachineAlternativeFamily::PackedNarrowI16x8ToU8x4Saturating,
                operands,
                2,
            )?;
            Ok(PackedSpec {
                instructions: vec![
                    Instruction::LoadUnaligned {
                        destination_xmm: 0,
                        base: source,
                    },
                    Instruction::PackUnsignedSaturatingWordsToBytes {
                        destination_xmm: 0,
                        source_xmm: 0,
                    },
                    Instruction::StoreDword {
                        base: destination,
                        source_xmm: 0,
                    },
                ],
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                external_operand_reads: vec![0, 1],
                external_operand_writes: Vec::new(),
                clobbers: vec!["xmm0"],
                memory: MachineEncodedMemoryEffect::PackedUnary16To4BytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedLoad128 { byte_offset }
        | SelectedInstructionKind::PackedLoad64ZeroExtend { byte_offset }
        | SelectedInstructionKind::PackedLoad32ZeroExtend { byte_offset } => {
            let (family, byte_count) = match kind {
                SelectedInstructionKind::PackedLoad128 { .. } => {
                    (MachineAlternativeFamily::PackedLoad128, 16_u16)
                }
                SelectedInstructionKind::PackedLoad64ZeroExtend { .. } => {
                    (MachineAlternativeFamily::PackedLoad64ZeroExtend, 8_u16)
                }
                _ => (MachineAlternativeFamily::PackedLoad32ZeroExtend, 4_u16),
            };
            check_canonical_alternative(physical, alternative, family)?;
            let &[base_id, dst_id] = operands else {
                return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
            };
            let base = resolve_gpr(physical, base_id)?;
            let destination_xmm = resolve_xmm(physical, dst_id)?;
            let disp = validate_disp(byte_offset)?;
            let instruction = match byte_count {
                16 => Instruction::VmovdquLoad {
                    destination_xmm,
                    base,
                    disp,
                },
                8 => Instruction::VmovqLoad {
                    destination_xmm,
                    base,
                    disp,
                },
                4 => Instruction::VmovdLoad {
                    destination_xmm,
                    base,
                    disp,
                },
                _ => unreachable!(),
            };
            Ok(PackedSpec {
                instructions: vec![instruction],
                register_reads: vec![base_id],
                register_writes: vec![dst_id],
                external_operand_reads: vec![0],
                external_operand_writes: vec![1],
                clobbers: Vec::new(),
                memory: MachineEncodedMemoryEffect::PackedVectorLoadBytesV1 {
                    pointer_operand: 0,
                    byte_count,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedStore128 { byte_offset }
        | SelectedInstructionKind::PackedStore64 { byte_offset }
        | SelectedInstructionKind::PackedStore32 { byte_offset } => {
            let (family, byte_count) = match kind {
                SelectedInstructionKind::PackedStore128 { .. } => {
                    (MachineAlternativeFamily::PackedStore128, 16_u16)
                }
                SelectedInstructionKind::PackedStore64 { .. } => {
                    (MachineAlternativeFamily::PackedStore64, 8_u16)
                }
                _ => (MachineAlternativeFamily::PackedStore32, 4_u16),
            };
            check_canonical_alternative(physical, alternative, family)?;
            let &[base_id, src_id] = operands else {
                return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
            };
            let base = resolve_gpr(physical, base_id)?;
            let source_xmm = resolve_xmm(physical, src_id)?;
            let disp = validate_disp(byte_offset)?;
            let instruction = match byte_count {
                16 => Instruction::VmovdquStore {
                    base,
                    disp,
                    source_xmm,
                },
                8 => Instruction::VmovqStore {
                    base,
                    disp,
                    source_xmm,
                },
                4 => Instruction::VmovdStore {
                    base,
                    disp,
                    source_xmm,
                },
                _ => unreachable!(),
            };
            Ok(PackedSpec {
                instructions: vec![instruction],
                register_reads: vec![base_id, src_id],
                register_writes: Vec::new(),
                external_operand_reads: vec![0, 1],
                external_operand_writes: Vec::new(),
                clobbers: Vec::new(),
                memory: MachineEncodedMemoryEffect::PackedVectorStoreBytesV1 {
                    pointer_operand: 0,
                    byte_count,
                },
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            })
        }
        SelectedInstructionKind::PackedMaterializeZero => {
            check_canonical_alternative(
                physical,
                alternative,
                MachineAlternativeFamily::PackedMaterializeZero,
            )?;
            let &[dst_id] = operands else {
                return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
            };
            let dst = resolve_xmm(physical, dst_id)?;
            Ok(pure_spec(
                vec![Instruction::Vpxor {
                    destination_xmm: dst,
                    left_xmm: dst,
                    right_xmm: dst,
                }],
                operands,
                0,
            ))
        }
        SelectedInstructionKind::PackedFromGpr64ZeroExtend => {
            check_canonical_alternative(
                physical,
                alternative,
                MachineAlternativeFamily::PackedFromGpr64ZeroExtend,
            )?;
            let &[src_id, dst_id] = operands else {
                return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
            };
            let source_gpr = resolve_gpr(physical, src_id)?;
            let destination_xmm = resolve_xmm(physical, dst_id)?;
            Ok(pure_spec(
                vec![Instruction::VmovqFromGpr {
                    destination_xmm,
                    source_gpr,
                }],
                operands,
                1,
            ))
        }
        SelectedInstructionKind::PackedFromGpr64Pair => {
            check_canonical_alternative(
                physical,
                alternative,
                MachineAlternativeFamily::PackedFromGpr64Pair,
            )?;
            let &[lo_id, hi_id, dst_id] = operands else {
                return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
            };
            let lo = resolve_gpr(physical, lo_id)?;
            let hi = resolve_gpr(physical, hi_id)?;
            let dst = resolve_xmm(physical, dst_id)?;
            Ok(pure_spec(
                vec![
                    Instruction::VmovqFromGpr {
                        destination_xmm: dst,
                        source_gpr: lo,
                    },
                    Instruction::Vpinsrq {
                        destination_xmm: dst,
                        source_xmm: dst,
                        source_gpr: hi,
                        lane: 1,
                    },
                ],
                operands,
                2,
            ))
        }
        SelectedInstructionKind::PackedAddI16x8Reg | SelectedInstructionKind::PackedSubI16x8Reg => {
            let family = if kind == SelectedInstructionKind::PackedAddI16x8Reg {
                MachineAlternativeFamily::PackedAddI16x8Reg
            } else {
                MachineAlternativeFamily::PackedSubI16x8Reg
            };
            check_canonical_alternative(physical, alternative, family)?;
            let &[lhs_id, rhs_id, dst_id] = operands else {
                return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
            };
            let left_xmm = resolve_xmm(physical, lhs_id)?;
            let right_xmm = resolve_xmm(physical, rhs_id)?;
            let destination_xmm = resolve_xmm(physical, dst_id)?;
            let instruction = if kind == SelectedInstructionKind::PackedAddI16x8Reg {
                Instruction::Vpaddw {
                    destination_xmm,
                    left_xmm,
                    right_xmm,
                }
            } else {
                Instruction::Vpsubw {
                    destination_xmm,
                    left_xmm,
                    right_xmm,
                }
            };
            Ok(pure_spec(vec![instruction], operands, 2))
        }
        SelectedInstructionKind::PackedShiftRightArithmeticI16x8Reg { count } => {
            if count >= 16 {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            check_canonical_alternative(
                physical,
                alternative,
                MachineAlternativeFamily::PackedShiftRightArithmeticI16x8Reg,
            )?;
            let &[src_id, dst_id] = operands else {
                return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
            };
            let source_xmm = resolve_xmm(physical, src_id)?;
            let destination_xmm = resolve_xmm(physical, dst_id)?;
            Ok(pure_spec(
                vec![Instruction::VpsrawImm {
                    destination_xmm,
                    source_xmm,
                    count,
                }],
                operands,
                1,
            ))
        }
        SelectedInstructionKind::PackedShuffleI16x8Reg { lanes } => {
            let pattern = PackedShuffleFastPattern::from_lanes(lanes)
                .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
            check_canonical_alternative(
                physical,
                alternative,
                MachineAlternativeFamily::PackedShuffleI16x8Reg,
            )?;
            let &[lhs_id, rhs_id, dst_id] = operands else {
                return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
            };
            let left_xmm = resolve_xmm(physical, lhs_id)?;
            let right_xmm = resolve_xmm(physical, rhs_id)?;
            let destination_xmm = resolve_xmm(physical, dst_id)?;
            let instructions = match pattern {
                PackedShuffleFastPattern::InterleaveLowWords => vec![Instruction::Vpunpcklwd {
                    destination_xmm,
                    left_xmm,
                    right_xmm,
                }],
                PackedShuffleFastPattern::InterleaveHighWords => vec![Instruction::Vpunpckhwd {
                    destination_xmm,
                    left_xmm,
                    right_xmm,
                }],
                PackedShuffleFastPattern::InterleaveLowDoublewords => {
                    vec![Instruction::Vpunpckldq {
                        destination_xmm,
                        left_xmm,
                        right_xmm,
                    }]
                }
                PackedShuffleFastPattern::InterleaveHighDoublewords => {
                    vec![Instruction::Vpunpckhdq {
                        destination_xmm,
                        left_xmm,
                        right_xmm,
                    }]
                }
                PackedShuffleFastPattern::InterleaveLowQuadwords => {
                    vec![Instruction::Vpunpcklqdq {
                        destination_xmm,
                        left_xmm,
                        right_xmm,
                    }]
                }
                PackedShuffleFastPattern::InterleaveHighQuadwords => {
                    vec![Instruction::Vpunpckhqdq {
                        destination_xmm,
                        left_xmm,
                        right_xmm,
                    }]
                }
                PackedShuffleFastPattern::LowPairDuplicateLow => vec![
                    Instruction::Vpunpckldq {
                        destination_xmm,
                        left_xmm,
                        right_xmm,
                    },
                    Instruction::Vpunpcklqdq {
                        destination_xmm,
                        left_xmm: destination_xmm,
                        right_xmm: destination_xmm,
                    },
                ],
                PackedShuffleFastPattern::LowPairDuplicateHigh => vec![
                    Instruction::Vpunpckldq {
                        destination_xmm,
                        left_xmm,
                        right_xmm,
                    },
                    Instruction::Vpunpckhqdq {
                        destination_xmm,
                        left_xmm: destination_xmm,
                        right_xmm: destination_xmm,
                    },
                ],
                PackedShuffleFastPattern::HighPairDuplicateLow => vec![
                    Instruction::Vpunpckhdq {
                        destination_xmm,
                        left_xmm,
                        right_xmm,
                    },
                    Instruction::Vpunpcklqdq {
                        destination_xmm,
                        left_xmm: destination_xmm,
                        right_xmm: destination_xmm,
                    },
                ],
                PackedShuffleFastPattern::HighPairDuplicateHigh => vec![
                    Instruction::Vpunpckhdq {
                        destination_xmm,
                        left_xmm,
                        right_xmm,
                    },
                    Instruction::Vpunpckhqdq {
                        destination_xmm,
                        left_xmm: destination_xmm,
                        right_xmm: destination_xmm,
                    },
                ],
            };
            Ok(pure_spec(instructions, operands, 2))
        }
        SelectedInstructionKind::PackedWidenU8x8ToI16x8Reg => {
            check_canonical_alternative(
                physical,
                alternative,
                MachineAlternativeFamily::PackedWidenU8x8ToI16x8Reg,
            )?;
            let &[src_id, dst_id] = operands else {
                return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
            };
            let source_xmm = resolve_xmm(physical, src_id)?;
            let destination_xmm = resolve_xmm(physical, dst_id)?;
            Ok(pure_spec(
                vec![Instruction::Vpmovzxbw {
                    destination_xmm,
                    source_xmm,
                }],
                operands,
                1,
            ))
        }
        SelectedInstructionKind::PackedNarrowI16x8ToU8x8SaturatingReg => {
            check_canonical_alternative(
                physical,
                alternative,
                MachineAlternativeFamily::PackedNarrowI16x8ToU8x8SaturatingReg,
            )?;
            let &[src_id, dst_id] = operands else {
                return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
            };
            let src = resolve_xmm(physical, src_id)?;
            let dst = resolve_xmm(physical, dst_id)?;
            Ok(pure_spec(
                vec![Instruction::Vpackuswb {
                    destination_xmm: dst,
                    left_xmm: src,
                    right_xmm: src,
                }],
                operands,
                1,
            ))
        }
        _ => Err(X86_64SelectedFormEncodingError::EncodedFormMismatch),
    }
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let spec = spec(physical, kind, alternative, operands)?;
    let mut bytes = Vec::with_capacity(spec.instructions.len() * 6);
    for instruction in &spec.instructions {
        instruction
            .encode(&mut bytes)
            .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    }
    validate(physical, kind, alternative, operands, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let spec = spec(physical, kind, alternative, operands)?;
    let mismatch = X86_64SelectedFormEncodingError::EncodedFormMismatch;
    let mut remaining = bytes;
    for expected in spec.instructions {
        let (actual, tail) = Instruction::decode(remaining).ok_or(mismatch.clone())?;
        if actual != expected {
            return Err(mismatch);
        }
        remaining = tail;
    }
    if !remaining.is_empty() {
        return Err(mismatch);
    }
    let mut clobbers = Vec::new();
    for name in spec.clobbers {
        let view = physical
            .model()
            .view_named(name)
            .ok_or(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)?;
        clobbers.extend_from_slice(&view.units);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: spec.register_reads,
            register_writes: spec.register_writes,
            writes_rflags: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: spec.external_operand_reads,
                external_operand_writes: spec.external_operand_writes,
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: register_model::sorted_units(clobbers),
                memory: spec.memory,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: spec.trap,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selected_form_encoding::{
        encode_x86_64_selected_form, validate_x86_64_selected_form_encoding,
    };
    use selected_instructions::{MachineSemanticKind, SelectedInstructionKind};

    #[test]
    fn packed_add_validates_memory_addresses_vector_clobbers_and_all_bytes() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let constraints = crate::validate_x86_64_register_constraint_catalog(
            crate::x86_64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap();
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::PackedAddI16x8,
            variant: 0,
        };
        let operands =
            ["rcx", "r12", "r13"].map(|name| physical.model().view_named(name).unwrap().id);
        let kind = SelectedInstructionKind::PackedAddI16x8;
        let encoding =
            encode_x86_64_selected_form(&physical, kind, alternative, &operands).unwrap();
        assert_eq!(
            encoding.bytes(),
            [
                0xf3, 0x0f, 0x6f, 0x01, 0xf3, 0x41, 0x0f, 0x6f, 0x0c, 0x24, 0x66, 0x0f, 0xfd, 0xc1,
                0xf3, 0x41, 0x0f, 0x7f, 0x45, 0x00,
            ]
        );
        assert!(!encoding.footprint().writes_rflags);
        for target in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::windows_x64(),
        ] {
            let catalog = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
            crate::validate_x86_64_machine_effect_catalog(target, &constraints, catalog.clone())
                .unwrap();
            let row = catalog
                .declaration_by_semantic(MachineSemanticKind::PackedAddI16x8)
                .unwrap();
            assert_eq!(encoding.footprint().encoded, row.alternatives[0].encoded);
        }
        for bit in 0..encoding.bytes().len() * 8 {
            let mut corrupted = encoding.bytes().to_vec();
            corrupted[bit / 8] ^= 1 << (bit % 8);
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &corrupted
                )
                .is_err()
            );
        }
    }

    #[test]
    fn all_six_packed_forms_encode_validate_and_match_catalog_effects() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let constraints = crate::validate_x86_64_register_constraint_catalog(
            crate::x86_64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap();
        let ops3 = ["rcx", "rdx", "r8"].map(|n| physical.model().view_named(n).unwrap().id);
        let ops4 = ["rcx", "rdx", "r8", "r9"].map(|n| physical.model().view_named(n).unwrap().id);
        let ops2 = ["rcx", "rdx"].map(|n| physical.model().view_named(n).unwrap().id);
        let cases: Vec<(
            SelectedInstructionKind,
            MachineSemanticKind,
            &[RegisterViewId],
        )> = vec![
            (
                SelectedInstructionKind::PackedSubI16x8,
                MachineSemanticKind::PackedSubI16x8,
                &ops3,
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8 {
                    lanes: [0, 8, 1, 9, 2, 10, 3, 11],
                },
                MachineSemanticKind::PackedShuffleI16x8,
                &ops3,
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8 {
                    lanes: [0, 1, 8, 9, 0, 1, 8, 9],
                },
                MachineSemanticKind::PackedShuffleI16x8,
                &ops3,
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8 {
                    lanes: [2, 3, 10, 11, 2, 3, 10, 11],
                },
                MachineSemanticKind::PackedShuffleI16x8,
                &ops3,
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8 {
                    lanes: [4, 5, 12, 13, 4, 5, 12, 13],
                },
                MachineSemanticKind::PackedShuffleI16x8,
                &ops3,
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8 {
                    lanes: [6, 7, 14, 15, 6, 7, 14, 15],
                },
                MachineSemanticKind::PackedShuffleI16x8,
                &ops3,
            ),
            (
                SelectedInstructionKind::PackedShuffleGeneralI16x8 {
                    lanes: [15, 0, 14, 1, 13, 2, 12, 3],
                },
                MachineSemanticKind::PackedShuffleGeneralI16x8,
                &ops4,
            ),
            (
                SelectedInstructionKind::PackedShiftRightArithmeticI16x8 { count: 6 },
                MachineSemanticKind::PackedShiftRightArithmeticI16x8,
                &ops2,
            ),
            (
                SelectedInstructionKind::PackedWidenU8x8ToI16x8,
                MachineSemanticKind::PackedWidenU8x8ToI16x8,
                &ops2,
            ),
            (
                SelectedInstructionKind::PackedNarrowI16x8ToU8x8Saturating,
                MachineSemanticKind::PackedNarrowI16x8ToU8x8Saturating,
                &ops2,
            ),
            (
                SelectedInstructionKind::PackedLoadI16x4ZeroExtend,
                MachineSemanticKind::PackedLoadI16x4ZeroExtend,
                &ops2,
            ),
            (
                SelectedInstructionKind::PackedWidenU8x4ToI16x8,
                MachineSemanticKind::PackedWidenU8x4ToI16x8,
                &ops2,
            ),
            (
                SelectedInstructionKind::PackedNarrowI16x8ToU8x4Saturating,
                MachineSemanticKind::PackedNarrowI16x8ToU8x4Saturating,
                &ops2,
            ),
        ];
        let gpr_xmm = ["rcx", "xmm1"].map(|n| physical.model().view_named(n).unwrap().id);
        let xmm1 = ["xmm1"].map(|n| physical.model().view_named(n).unwrap().id);
        let gpr2_xmm = ["rcx", "r8", "xmm9"].map(|n| physical.model().view_named(n).unwrap().id);
        let xmm2 = ["xmm3", "xmm11"].map(|n| physical.model().view_named(n).unwrap().id);
        let xmm3 = ["xmm1", "xmm10", "xmm12"].map(|n| physical.model().view_named(n).unwrap().id);
        let mut all_cases = cases;
        all_cases.extend([
            (
                SelectedInstructionKind::PackedLoad128 { byte_offset: 32 },
                MachineSemanticKind::PackedLoad128,
                &gpr_xmm[..],
            ),
            (
                SelectedInstructionKind::PackedStore128 { byte_offset: 16 },
                MachineSemanticKind::PackedStore128,
                &gpr_xmm[..],
            ),
            (
                SelectedInstructionKind::PackedLoad64ZeroExtend { byte_offset: 8 },
                MachineSemanticKind::PackedLoad64ZeroExtend,
                &gpr_xmm[..],
            ),
            (
                SelectedInstructionKind::PackedStore64 { byte_offset: 8 },
                MachineSemanticKind::PackedStore64,
                &gpr_xmm[..],
            ),
            (
                SelectedInstructionKind::PackedLoad32ZeroExtend { byte_offset: 0 },
                MachineSemanticKind::PackedLoad32ZeroExtend,
                &gpr_xmm[..],
            ),
            (
                SelectedInstructionKind::PackedStore32 { byte_offset: 4 },
                MachineSemanticKind::PackedStore32,
                &gpr_xmm[..],
            ),
            (
                SelectedInstructionKind::PackedMaterializeZero,
                MachineSemanticKind::PackedMaterializeZero,
                &xmm1[..],
            ),
            (
                SelectedInstructionKind::PackedFromGpr64ZeroExtend,
                MachineSemanticKind::PackedFromGpr64ZeroExtend,
                &gpr_xmm[..],
            ),
            (
                SelectedInstructionKind::PackedFromGpr64Pair,
                MachineSemanticKind::PackedFromGpr64Pair,
                &gpr2_xmm[..],
            ),
            (
                SelectedInstructionKind::PackedAddI16x8Reg,
                MachineSemanticKind::PackedAddI16x8Reg,
                &xmm3[..],
            ),
            (
                SelectedInstructionKind::PackedSubI16x8Reg,
                MachineSemanticKind::PackedSubI16x8Reg,
                &xmm3[..],
            ),
            (
                SelectedInstructionKind::PackedShiftRightArithmeticI16x8Reg { count: 6 },
                MachineSemanticKind::PackedShiftRightArithmeticI16x8Reg,
                &xmm2[..],
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8Reg {
                    lanes: [0, 8, 1, 9, 2, 10, 3, 11],
                },
                MachineSemanticKind::PackedShuffleI16x8Reg,
                &xmm3[..],
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8Reg {
                    lanes: [4, 12, 5, 13, 6, 14, 7, 15],
                },
                MachineSemanticKind::PackedShuffleI16x8Reg,
                &xmm3[..],
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8Reg {
                    lanes: [0, 1, 8, 9, 2, 3, 10, 11],
                },
                MachineSemanticKind::PackedShuffleI16x8Reg,
                &xmm3[..],
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8Reg {
                    lanes: [4, 5, 12, 13, 6, 7, 14, 15],
                },
                MachineSemanticKind::PackedShuffleI16x8Reg,
                &xmm3[..],
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8Reg {
                    lanes: [0, 1, 2, 3, 8, 9, 10, 11],
                },
                MachineSemanticKind::PackedShuffleI16x8Reg,
                &xmm3[..],
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8Reg {
                    lanes: [4, 5, 6, 7, 12, 13, 14, 15],
                },
                MachineSemanticKind::PackedShuffleI16x8Reg,
                &xmm3[..],
            ),
            (
                SelectedInstructionKind::PackedShuffleI16x8Reg {
                    lanes: [0, 1, 8, 9, 0, 1, 8, 9],
                },
                MachineSemanticKind::PackedShuffleI16x8Reg,
                &xmm3[..],
            ),
            (
                SelectedInstructionKind::PackedWidenU8x8ToI16x8Reg,
                MachineSemanticKind::PackedWidenU8x8ToI16x8Reg,
                &xmm2[..],
            ),
            (
                SelectedInstructionKind::PackedNarrowI16x8ToU8x8SaturatingReg,
                MachineSemanticKind::PackedNarrowI16x8ToU8x8SaturatingReg,
                &xmm2[..],
            ),
        ]);
        for (kind, semantic, operands) in all_cases {
            let alternative = MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            };
            let encoding =
                encode_x86_64_selected_form(&physical, kind, alternative, operands).unwrap();
            assert!(!encoding.footprint().writes_rflags);
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                operands,
                encoding.bytes(),
            )
            .unwrap();
            for bit in 0..encoding.bytes().len() * 8 {
                let mut corrupted = encoding.bytes().to_vec();
                corrupted[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative,
                        operands,
                        &corrupted
                    )
                    .is_err()
                );
            }
            for target in [
                target::NativeTarget::linux_x64(),
                target::NativeTarget::windows_x64(),
            ] {
                let catalog = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
                let row = catalog.declaration_by_semantic(semantic).unwrap();
                assert_eq!(encoding.footprint().encoded, row.alternatives[0].encoded);
            }
        }
    }
}
