use super::{
    MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect as MemEff, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior as Trap, RegisterViewId, SelectedInstructionKind as Kind,
    ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
    X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint,
};
use selected_instructions::PackedShuffleFastPattern as Fast;
use x86_64_encoding::packed_integer::Instruction as Insn;

struct PackedSpec {
    instructions: Vec<Insn>,
    reads: Vec<RegisterViewId>,
    writes: Vec<RegisterViewId>,
    ext_reads: Vec<u16>,
    ext_writes: Vec<u16>,
    clobbers: &'static [&'static str],
    memory: MemEff,
    trap: Trap,
}

fn check_alt(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    family: Family,
) -> Result<(), Error> {
    super::request_validation::guard_canonical(physical)?;
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(())
}

fn resolve_gpr(
    physical: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
) -> Result<u8, Error> {
    Ok(super::request_validation::resolve_registers(physical, &[view])?[0])
}

fn resolve_xmm(
    physical: &ValidatedPhysicalRegisterModel,
    view_id: RegisterViewId,
) -> Result<u8, Error> {
    let view = physical
        .model()
        .view(view_id)
        .ok_or(Error::EncodedFormMismatch)?;
    if view.bits != 128 || !view.allocatable {
        return Err(Error::EncodedFormMismatch);
    }
    view.name
        .strip_prefix("xmm")
        .and_then(|s| s.parse::<u8>().ok())
        .filter(|&i| i < 16)
        .ok_or(Error::EncodedFormMismatch)
}

fn pure_spec(instructions: Vec<Insn>, operands: &[RegisterViewId], inputs: usize) -> PackedSpec {
    PackedSpec {
        instructions,
        reads: operands[..inputs].to_vec(),
        writes: operands[inputs..].to_vec(),
        ext_reads: (0..inputs as u16).collect(),
        ext_writes: (inputs as u16..operands.len() as u16).collect(),
        clobbers: &[],
        memory: MemEff::NoneV1,
        trap: Trap::NeverV1,
    }
}

fn mem_spec(
    instructions: Vec<Insn>,
    operands: &[RegisterViewId],
    clobbers: &'static [&'static str],
    memory: MemEff,
) -> PackedSpec {
    PackedSpec {
        instructions,
        reads: operands.to_vec(),
        writes: Vec::new(),
        ext_reads: (0..operands.len() as u16).collect(),
        ext_writes: Vec::new(),
        clobbers,
        memory,
        trap: Trap::MayArchitecturalFaultV1,
    }
}

fn fast_shuffle_mem(pattern: Fast) -> Vec<Insn> {
    let (d, s) = (0, 1);
    match pattern {
        Fast::InterleaveLowWords => vec![Insn::UnpackLowWords {
            destination_xmm: d,
            source_xmm: s,
        }],
        Fast::InterleaveHighWords => vec![Insn::UnpackHighWords {
            destination_xmm: d,
            source_xmm: s,
        }],
        Fast::InterleaveLowDoublewords => vec![Insn::UnpackLowDoublewords {
            destination_xmm: d,
            source_xmm: s,
        }],
        Fast::InterleaveHighDoublewords => vec![Insn::UnpackHighDoublewords {
            destination_xmm: d,
            source_xmm: s,
        }],
        Fast::InterleaveLowQuadwords => vec![Insn::UnpackLowQuadwords {
            destination_xmm: d,
            source_xmm: s,
        }],
        Fast::InterleaveHighQuadwords => vec![Insn::UnpackHighQuadwords {
            destination_xmm: d,
            source_xmm: s,
        }],
        Fast::LowPairDuplicateLow => vec![
            Insn::UnpackLowDoublewords {
                destination_xmm: d,
                source_xmm: s,
            },
            Insn::UnpackLowQuadwords {
                destination_xmm: d,
                source_xmm: d,
            },
        ],
        Fast::LowPairDuplicateHigh => vec![
            Insn::UnpackLowDoublewords {
                destination_xmm: d,
                source_xmm: s,
            },
            Insn::UnpackHighQuadwords {
                destination_xmm: d,
                source_xmm: d,
            },
        ],
        Fast::HighPairDuplicateLow => vec![
            Insn::UnpackHighDoublewords {
                destination_xmm: d,
                source_xmm: s,
            },
            Insn::UnpackLowQuadwords {
                destination_xmm: d,
                source_xmm: d,
            },
        ],
        Fast::HighPairDuplicateHigh => vec![
            Insn::UnpackHighDoublewords {
                destination_xmm: d,
                source_xmm: s,
            },
            Insn::UnpackHighQuadwords {
                destination_xmm: d,
                source_xmm: d,
            },
        ],
    }
}

fn fast_shuffle_reg(pattern: Fast, d: u8, l: u8, r: u8) -> Vec<Insn> {
    match pattern {
        Fast::InterleaveLowWords => vec![Insn::Vpunpcklwd {
            destination_xmm: d,
            left_xmm: l,
            right_xmm: r,
        }],
        Fast::InterleaveHighWords => vec![Insn::Vpunpckhwd {
            destination_xmm: d,
            left_xmm: l,
            right_xmm: r,
        }],
        Fast::InterleaveLowDoublewords => vec![Insn::Vpunpckldq {
            destination_xmm: d,
            left_xmm: l,
            right_xmm: r,
        }],
        Fast::InterleaveHighDoublewords => vec![Insn::Vpunpckhdq {
            destination_xmm: d,
            left_xmm: l,
            right_xmm: r,
        }],
        Fast::InterleaveLowQuadwords => vec![Insn::Vpunpcklqdq {
            destination_xmm: d,
            left_xmm: l,
            right_xmm: r,
        }],
        Fast::InterleaveHighQuadwords => vec![Insn::Vpunpckhqdq {
            destination_xmm: d,
            left_xmm: l,
            right_xmm: r,
        }],
        Fast::LowPairDuplicateLow => vec![
            Insn::Vpunpckldq {
                destination_xmm: d,
                left_xmm: l,
                right_xmm: r,
            },
            Insn::Vpunpcklqdq {
                destination_xmm: d,
                left_xmm: d,
                right_xmm: d,
            },
        ],
        Fast::LowPairDuplicateHigh => vec![
            Insn::Vpunpckldq {
                destination_xmm: d,
                left_xmm: l,
                right_xmm: r,
            },
            Insn::Vpunpckhqdq {
                destination_xmm: d,
                left_xmm: d,
                right_xmm: d,
            },
        ],
        Fast::HighPairDuplicateLow => vec![
            Insn::Vpunpckhdq {
                destination_xmm: d,
                left_xmm: l,
                right_xmm: r,
            },
            Insn::Vpunpcklqdq {
                destination_xmm: d,
                left_xmm: d,
                right_xmm: d,
            },
        ],
        Fast::HighPairDuplicateHigh => vec![
            Insn::Vpunpckhdq {
                destination_xmm: d,
                left_xmm: l,
                right_xmm: r,
            },
            Insn::Vpunpckhqdq {
                destination_xmm: d,
                left_xmm: d,
                right_xmm: d,
            },
        ],
    }
}

fn spec(
    physical: &ValidatedPhysicalRegisterModel,
    kind: Kind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<PackedSpec, Error> {
    const BINARY_MEM: MemEff = MemEff::PackedBinary16BytesV1 {
        left_pointer_operand: 0,
        right_pointer_operand: 1,
        destination_pointer_operand: 2,
    };
    const UNARY_16_16: MemEff = MemEff::PackedUnary16To16BytesV1 {
        source_pointer_operand: 0,
        destination_pointer_operand: 1,
    };
    const XMM0: &[&str] = &["xmm0"];
    const XMM01: &[&str] = &["xmm0", "xmm1"];
    use super::request_validation::byte_loop_registers as loop_regs;

    match kind {
        Kind::PackedAddI16x8
        | Kind::PackedSubI16x8
        | Kind::PackedAddU8x16
        | Kind::PackedMinI16x8
        | Kind::PackedCmpGtI16x8
        | Kind::PackedBitAndI16x8
        | Kind::PackedBitAndNotI16x8 => {
            let (family, op) = match kind {
                Kind::PackedAddI16x8 => (
                    Family::PackedAddI16x8,
                    Insn::AddWords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                ),
                Kind::PackedSubI16x8 => (
                    Family::PackedSubI16x8,
                    Insn::SubWords {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                ),
                Kind::PackedAddU8x16 => (
                    Family::PackedAddU8x16,
                    Insn::AddBytes {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                ),
                Kind::PackedMinI16x8 => (
                    Family::PackedMinI16x8,
                    Insn::MinWordsSigned {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                ),
                Kind::PackedCmpGtI16x8 => (
                    Family::PackedCmpGtI16x8,
                    Insn::CompareGreaterWordsSigned {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                ),
                Kind::PackedBitAndI16x8 => (
                    Family::PackedBitAndI16x8,
                    Insn::BitwiseAndPacked {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                ),
                _ => (
                    Family::PackedBitAndNotI16x8,
                    Insn::BitwiseAndNotPacked {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                ),
            };
            let [left, right, dst] = loop_regs::<3>(physical, alternative, family, operands, 3)?;
            Ok(mem_spec(
                vec![
                    Insn::LoadUnaligned {
                        destination_xmm: 0,
                        base: left,
                    },
                    Insn::LoadUnaligned {
                        destination_xmm: 1,
                        base: right,
                    },
                    op,
                    Insn::StoreUnaligned {
                        base: dst,
                        source_xmm: 0,
                    },
                ],
                operands,
                XMM01,
                BINARY_MEM,
            ))
        }
        Kind::PackedShuffleI16x8 { lanes } => {
            let pattern = Fast::from_lanes(lanes).ok_or(Error::EncodedFormMismatch)?;
            let [left, right, dst] = loop_regs::<3>(
                physical,
                alternative,
                Family::PackedShuffleI16x8,
                operands,
                3,
            )?;
            let mut insns = vec![
                Insn::LoadUnaligned {
                    destination_xmm: 0,
                    base: left,
                },
                Insn::LoadUnaligned {
                    destination_xmm: 1,
                    base: right,
                },
            ];
            insns.extend(fast_shuffle_mem(pattern));
            insns.push(Insn::StoreUnaligned {
                base: dst,
                source_xmm: 0,
            });
            Ok(mem_spec(insns, operands, XMM01, BINARY_MEM))
        }
        Kind::PackedShuffleGeneralI16x8 { lanes } => {
            if lanes.iter().any(|&lane| lane >= 16) {
                return Err(Error::EncodedFormMismatch);
            }
            let [left, right, dst, scratch] = loop_regs::<4>(
                physical,
                alternative,
                Family::PackedShuffleGeneralI16x8,
                operands,
                3,
            )?;
            let mut insns = Vec::with_capacity(19);
            insns.push(Insn::LoadUnaligned {
                destination_xmm: 0,
                base: left,
            });
            insns.push(Insn::LoadUnaligned {
                destination_xmm: 1,
                base: right,
            });
            for (dst_lane, &src_lane) in lanes.iter().enumerate() {
                insns.push(Insn::ExtractWord {
                    destination_gpr: scratch,
                    source_xmm: u8::from(src_lane >= 8),
                    lane: src_lane & 7,
                });
                insns.push(Insn::InsertWord {
                    destination_xmm: 2,
                    source_gpr: scratch,
                    lane: dst_lane as u8,
                });
            }
            insns.push(Insn::StoreUnaligned {
                base: dst,
                source_xmm: 2,
            });
            Ok(PackedSpec {
                instructions: insns,
                reads: operands[..3].to_vec(),
                writes: vec![operands[3]],
                ext_reads: vec![0, 1, 2],
                ext_writes: vec![3],
                clobbers: &["xmm0", "xmm1", "xmm2"],
                memory: BINARY_MEM,
                trap: Trap::MayArchitecturalFaultV1,
            })
        }
        Kind::PackedShiftRightArithmeticI16x8 { count } => {
            if count >= 16 {
                return Err(Error::EncodedFormMismatch);
            }
            let [src, dst] = loop_regs::<2>(
                physical,
                alternative,
                Family::PackedShiftRightArithmeticI16x8,
                operands,
                2,
            )?;
            Ok(mem_spec(
                vec![
                    Insn::LoadUnaligned {
                        destination_xmm: 0,
                        base: src,
                    },
                    Insn::ShiftRightArithmeticWords { xmm: 0, count },
                    Insn::StoreUnaligned {
                        base: dst,
                        source_xmm: 0,
                    },
                ],
                operands,
                XMM0,
                UNARY_16_16,
            ))
        }
        Kind::PackedAbsI16x8
        | Kind::PackedWidenU8x8ToI16x8
        | Kind::PackedNarrowI16x8ToU8x8Saturating
        | Kind::PackedLoadI16x4ZeroExtend
        | Kind::PackedWidenU8x4ToI16x8
        | Kind::PackedNarrowI16x8ToU8x4Saturating => {
            let (family, clobbers, memory) = match kind {
                Kind::PackedAbsI16x8 => (Family::PackedAbsI16x8, XMM0, UNARY_16_16),
                Kind::PackedWidenU8x8ToI16x8 => (
                    Family::PackedWidenU8x8ToI16x8,
                    XMM01,
                    MemEff::PackedUnary8To16BytesV1 {
                        source_pointer_operand: 0,
                        destination_pointer_operand: 1,
                    },
                ),
                Kind::PackedNarrowI16x8ToU8x8Saturating => (
                    Family::PackedNarrowI16x8ToU8x8Saturating,
                    XMM0,
                    MemEff::PackedUnary16To8BytesV1 {
                        source_pointer_operand: 0,
                        destination_pointer_operand: 1,
                    },
                ),
                Kind::PackedLoadI16x4ZeroExtend => (
                    Family::PackedLoadI16x4ZeroExtend,
                    XMM0,
                    MemEff::PackedUnary8To16BytesV1 {
                        source_pointer_operand: 0,
                        destination_pointer_operand: 1,
                    },
                ),
                Kind::PackedWidenU8x4ToI16x8 => (
                    Family::PackedWidenU8x4ToI16x8,
                    XMM01,
                    MemEff::PackedUnary4To16BytesV1 {
                        source_pointer_operand: 0,
                        destination_pointer_operand: 1,
                    },
                ),
                _ => (
                    Family::PackedNarrowI16x8ToU8x4Saturating,
                    XMM0,
                    MemEff::PackedUnary16To4BytesV1 {
                        source_pointer_operand: 0,
                        destination_pointer_operand: 1,
                    },
                ),
            };
            let [src, dst] = loop_regs::<2>(physical, alternative, family, operands, 2)?;
            let insns = match kind {
                Kind::PackedAbsI16x8 => vec![
                    Insn::LoadUnaligned {
                        destination_xmm: 0,
                        base: src,
                    },
                    Insn::AbsWordsSigned {
                        destination_xmm: 0,
                        source_xmm: 0,
                    },
                    Insn::StoreUnaligned {
                        base: dst,
                        source_xmm: 0,
                    },
                ],
                Kind::PackedWidenU8x8ToI16x8 => vec![
                    Insn::LoadQuad {
                        destination_xmm: 0,
                        base: src,
                    },
                    Insn::XorPacked {
                        destination_xmm: 1,
                        source_xmm: 1,
                    },
                    Insn::UnpackLowBytes {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                    Insn::StoreUnaligned {
                        base: dst,
                        source_xmm: 0,
                    },
                ],
                Kind::PackedWidenU8x4ToI16x8 => vec![
                    Insn::LoadDword {
                        destination_xmm: 0,
                        base: src,
                    },
                    Insn::XorPacked {
                        destination_xmm: 1,
                        source_xmm: 1,
                    },
                    Insn::UnpackLowBytes {
                        destination_xmm: 0,
                        source_xmm: 1,
                    },
                    Insn::StoreUnaligned {
                        base: dst,
                        source_xmm: 0,
                    },
                ],
                Kind::PackedNarrowI16x8ToU8x8Saturating => vec![
                    Insn::LoadUnaligned {
                        destination_xmm: 0,
                        base: src,
                    },
                    Insn::PackUnsignedSaturatingWordsToBytes {
                        destination_xmm: 0,
                        source_xmm: 0,
                    },
                    Insn::StoreQuad {
                        base: dst,
                        source_xmm: 0,
                    },
                ],
                Kind::PackedNarrowI16x8ToU8x4Saturating => vec![
                    Insn::LoadUnaligned {
                        destination_xmm: 0,
                        base: src,
                    },
                    Insn::PackUnsignedSaturatingWordsToBytes {
                        destination_xmm: 0,
                        source_xmm: 0,
                    },
                    Insn::StoreDword {
                        base: dst,
                        source_xmm: 0,
                    },
                ],
                _ => vec![
                    Insn::LoadQuad {
                        destination_xmm: 0,
                        base: src,
                    },
                    Insn::StoreUnaligned {
                        base: dst,
                        source_xmm: 0,
                    },
                ],
            };
            Ok(mem_spec(insns, operands, clobbers, memory))
        }
        Kind::PackedLoad128 { byte_offset }
        | Kind::PackedLoad64ZeroExtend { byte_offset }
        | Kind::PackedLoad32ZeroExtend { byte_offset }
        | Kind::PackedStore128 { byte_offset }
        | Kind::PackedStore64 { byte_offset }
        | Kind::PackedStore32 { byte_offset } => {
            let (family, byte_count, is_load) = match kind {
                Kind::PackedLoad128 { .. } => (Family::PackedLoad128, 16_u16, true),
                Kind::PackedLoad64ZeroExtend { .. } => {
                    (Family::PackedLoad64ZeroExtend, 8_u16, true)
                }
                Kind::PackedLoad32ZeroExtend { .. } => {
                    (Family::PackedLoad32ZeroExtend, 4_u16, true)
                }
                Kind::PackedStore128 { .. } => (Family::PackedStore128, 16_u16, false),
                Kind::PackedStore64 { .. } => (Family::PackedStore64, 8_u16, false),
                _ => (Family::PackedStore32, 4_u16, false),
            };
            check_alt(physical, alternative, family)?;
            let &[base_id, vec_id] = operands else {
                return Err(Error::OperandCountMismatch);
            };
            let base = resolve_gpr(physical, base_id)?;
            let xmm = resolve_xmm(physical, vec_id)?;
            let disp = i32::try_from(byte_offset).map_err(|_| Error::EncodedFormMismatch)?;
            let insn = match (is_load, byte_count) {
                (true, 16) => Insn::VmovdquLoad {
                    destination_xmm: xmm,
                    base,
                    disp,
                },
                (true, 8) => Insn::VmovqLoad {
                    destination_xmm: xmm,
                    base,
                    disp,
                },
                (true, 4) => Insn::VmovdLoad {
                    destination_xmm: xmm,
                    base,
                    disp,
                },
                (false, 16) => Insn::VmovdquStore {
                    base,
                    disp,
                    source_xmm: xmm,
                },
                (false, 8) => Insn::VmovqStore {
                    base,
                    disp,
                    source_xmm: xmm,
                },
                _ => Insn::VmovdStore {
                    base,
                    disp,
                    source_xmm: xmm,
                },
            };
            Ok(PackedSpec {
                instructions: vec![insn],
                reads: if is_load {
                    vec![base_id]
                } else {
                    vec![base_id, vec_id]
                },
                writes: if is_load { vec![vec_id] } else { Vec::new() },
                ext_reads: if is_load { vec![0] } else { vec![0, 1] },
                ext_writes: if is_load { vec![1] } else { Vec::new() },
                clobbers: &[],
                memory: if is_load {
                    MemEff::PackedVectorLoadBytesV1 {
                        pointer_operand: 0,
                        byte_count,
                    }
                } else {
                    MemEff::PackedVectorStoreBytesV1 {
                        pointer_operand: 0,
                        byte_count,
                    }
                },
                trap: Trap::MayArchitecturalFaultV1,
            })
        }
        Kind::PackedMaterializeZero => {
            check_alt(physical, alternative, Family::PackedMaterializeZero)?;
            let &[dst_id] = operands else {
                return Err(Error::OperandCountMismatch);
            };
            let d = resolve_xmm(physical, dst_id)?;
            Ok(pure_spec(
                vec![Insn::Vpxor {
                    destination_xmm: d,
                    left_xmm: d,
                    right_xmm: d,
                }],
                operands,
                0,
            ))
        }
        Kind::PackedFromGpr64ZeroExtend => {
            check_alt(physical, alternative, Family::PackedFromGpr64ZeroExtend)?;
            let &[src_id, dst_id] = operands else {
                return Err(Error::OperandCountMismatch);
            };
            Ok(pure_spec(
                vec![Insn::VmovqFromGpr {
                    destination_xmm: resolve_xmm(physical, dst_id)?,
                    source_gpr: resolve_gpr(physical, src_id)?,
                }],
                operands,
                1,
            ))
        }
        Kind::PackedFromGpr64Pair => {
            check_alt(physical, alternative, Family::PackedFromGpr64Pair)?;
            let &[lo_id, hi_id, dst_id] = operands else {
                return Err(Error::OperandCountMismatch);
            };
            let (lo, hi, dst) = (
                resolve_gpr(physical, lo_id)?,
                resolve_gpr(physical, hi_id)?,
                resolve_xmm(physical, dst_id)?,
            );
            Ok(pure_spec(
                vec![
                    Insn::VmovqFromGpr {
                        destination_xmm: dst,
                        source_gpr: lo,
                    },
                    Insn::Vpinsrq {
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
        Kind::PackedAddI16x8Reg
        | Kind::PackedSubI16x8Reg
        | Kind::PackedAddU8x16Reg
        | Kind::PackedMinI16x8Reg
        | Kind::PackedCmpGtI16x8Reg
        | Kind::PackedBitAndI16x8Reg
        | Kind::PackedBitAndNotI16x8Reg => {
            let family = match kind {
                Kind::PackedAddI16x8Reg => Family::PackedAddI16x8Reg,
                Kind::PackedSubI16x8Reg => Family::PackedSubI16x8Reg,
                Kind::PackedAddU8x16Reg => Family::PackedAddU8x16Reg,
                Kind::PackedMinI16x8Reg => Family::PackedMinI16x8Reg,
                Kind::PackedCmpGtI16x8Reg => Family::PackedCmpGtI16x8Reg,
                Kind::PackedBitAndI16x8Reg => Family::PackedBitAndI16x8Reg,
                _ => Family::PackedBitAndNotI16x8Reg,
            };
            check_alt(physical, alternative, family)?;
            let &[lhs_id, rhs_id, dst_id] = operands else {
                return Err(Error::OperandCountMismatch);
            };
            let (l, r, d) = (
                resolve_xmm(physical, lhs_id)?,
                resolve_xmm(physical, rhs_id)?,
                resolve_xmm(physical, dst_id)?,
            );
            let insn = match kind {
                Kind::PackedAddI16x8Reg => Insn::Vpaddw {
                    destination_xmm: d,
                    left_xmm: l,
                    right_xmm: r,
                },
                Kind::PackedSubI16x8Reg => Insn::Vpsubw {
                    destination_xmm: d,
                    left_xmm: l,
                    right_xmm: r,
                },
                Kind::PackedAddU8x16Reg => Insn::Vpaddb {
                    destination_xmm: d,
                    left_xmm: l,
                    right_xmm: r,
                },
                Kind::PackedMinI16x8Reg => Insn::Vpminsw {
                    destination_xmm: d,
                    left_xmm: l,
                    right_xmm: r,
                },
                Kind::PackedCmpGtI16x8Reg => Insn::Vpcmpgtw {
                    destination_xmm: d,
                    left_xmm: l,
                    right_xmm: r,
                },
                Kind::PackedBitAndI16x8Reg => Insn::Vpand {
                    destination_xmm: d,
                    left_xmm: l,
                    right_xmm: r,
                },
                _ => Insn::Vpandn {
                    destination_xmm: d,
                    left_xmm: l,
                    right_xmm: r,
                },
            };
            Ok(pure_spec(vec![insn], operands, 2))
        }
        Kind::PackedShuffleI16x8Reg { lanes } => {
            let pattern = Fast::from_lanes(lanes).ok_or(Error::EncodedFormMismatch)?;
            check_alt(physical, alternative, Family::PackedShuffleI16x8Reg)?;
            let &[lhs_id, rhs_id, dst_id] = operands else {
                return Err(Error::OperandCountMismatch);
            };
            let insns = fast_shuffle_reg(
                pattern,
                resolve_xmm(physical, dst_id)?,
                resolve_xmm(physical, lhs_id)?,
                resolve_xmm(physical, rhs_id)?,
            );
            Ok(pure_spec(insns, operands, 2))
        }
        Kind::PackedAbsI16x8Reg
        | Kind::PackedShiftRightArithmeticI16x8Reg { .. }
        | Kind::PackedWidenU8x8ToI16x8Reg
        | Kind::PackedNarrowI16x8ToU8x8SaturatingReg => {
            if let Kind::PackedShiftRightArithmeticI16x8Reg { count } = kind
                && count >= 16
            {
                return Err(Error::EncodedFormMismatch);
            }
            let family = match kind {
                Kind::PackedAbsI16x8Reg => Family::PackedAbsI16x8Reg,
                Kind::PackedShiftRightArithmeticI16x8Reg { .. } => {
                    Family::PackedShiftRightArithmeticI16x8Reg
                }
                Kind::PackedWidenU8x8ToI16x8Reg => Family::PackedWidenU8x8ToI16x8Reg,
                _ => Family::PackedNarrowI16x8ToU8x8SaturatingReg,
            };
            check_alt(physical, alternative, family)?;
            let &[src_id, dst_id] = operands else {
                return Err(Error::OperandCountMismatch);
            };
            let (s, d) = (
                resolve_xmm(physical, src_id)?,
                resolve_xmm(physical, dst_id)?,
            );
            let insn = match kind {
                Kind::PackedAbsI16x8Reg => Insn::Vpabsw {
                    destination_xmm: d,
                    source_xmm: s,
                },
                Kind::PackedShiftRightArithmeticI16x8Reg { count } => Insn::VpsrawImm {
                    destination_xmm: d,
                    source_xmm: s,
                    count,
                },
                Kind::PackedWidenU8x8ToI16x8Reg => Insn::Vpmovzxbw {
                    destination_xmm: d,
                    source_xmm: s,
                },
                _ => Insn::Vpackuswb {
                    destination_xmm: d,
                    left_xmm: s,
                    right_xmm: s,
                },
            };
            Ok(pure_spec(vec![insn], operands, 1))
        }
        _ => Err(Error::EncodedFormMismatch),
    }
}

fn encode_spec(spec: &PackedSpec) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::with_capacity(spec.instructions.len() * 6);
    for instruction in &spec.instructions {
        instruction
            .encode(&mut bytes)
            .ok_or(Error::EncodedFormMismatch)?;
    }
    Ok(bytes)
}

fn build_validated(
    physical: &ValidatedPhysicalRegisterModel,
    spec: PackedSpec,
    bytes: Vec<u8>,
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let mut clobbers = Vec::new();
    for name in spec.clobbers {
        let view = physical
            .model()
            .view_named(name)
            .ok_or(Error::NonCanonicalPhysicalModel)?;
        clobbers.extend_from_slice(&view.units);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes,
        footprint: X86_64SelectedFormFootprint {
            register_reads: spec.reads,
            register_writes: spec.writes,
            writes_rflags: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: spec.ext_reads,
                external_operand_writes: spec.ext_writes,
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

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: Kind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let spec = spec(physical, kind, alternative, operands)?;
    let bytes = encode_spec(&spec)?;
    build_validated(physical, spec, bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: Kind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let spec = spec(physical, kind, alternative, operands)?;
    let expected = encode_spec(&spec)?;
    if bytes != expected {
        return Err(Error::EncodedFormMismatch);
    }
    build_validated(physical, spec, bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selected_form_encoding::{
        assert_byte_fragility, assert_every_bit_flip_rejected, encode_x86_64_selected_form,
        validate_x86_64_selected_form_encoding,
    };

    #[test]
    fn packed_forms_match_golden_bytes_and_reject_corruptions() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let v = |name| physical.model().view_named(name).unwrap().id;
        let cases: &[(Kind, Family, &[&str], &[u8])] = &[
            (
                Kind::PackedAddI16x8,
                Family::PackedAddI16x8,
                &["rdi", "rsi", "rdx"],
                &[
                    0xf3, 0x0f, 0x6f, 0x07, 0xf3, 0x0f, 0x6f, 0x0e, 0x66, 0x0f, 0xfd, 0xc1, 0xf3,
                    0x0f, 0x7f, 0x02,
                ],
            ),
            (
                Kind::PackedSubI16x8,
                Family::PackedSubI16x8,
                &["rdi", "rsi", "rdx"],
                &[
                    0xf3, 0x0f, 0x6f, 0x07, 0xf3, 0x0f, 0x6f, 0x0e, 0x66, 0x0f, 0xf9, 0xc1, 0xf3,
                    0x0f, 0x7f, 0x02,
                ],
            ),
            (
                Kind::PackedShiftRightArithmeticI16x8 { count: 4 },
                Family::PackedShiftRightArithmeticI16x8,
                &["rdi", "rsi"],
                &[
                    0xf3, 0x0f, 0x6f, 0x07, 0x66, 0x0f, 0x71, 0xe0, 0x04, 0xf3, 0x0f, 0x7f, 0x06,
                ],
            ),
            (
                Kind::PackedWidenU8x8ToI16x8,
                Family::PackedWidenU8x8ToI16x8,
                &["rdi", "rsi"],
                &[
                    0xf3, 0x0f, 0x7e, 0x07, 0x66, 0x0f, 0xef, 0xc9, 0x66, 0x0f, 0x60, 0xc1, 0xf3,
                    0x0f, 0x7f, 0x06,
                ],
            ),
            (
                Kind::PackedNarrowI16x8ToU8x8Saturating,
                Family::PackedNarrowI16x8ToU8x8Saturating,
                &["rdi", "rsi"],
                &[
                    0xf3, 0x0f, 0x6f, 0x07, 0x66, 0x0f, 0x67, 0xc0, 0x66, 0x0f, 0xd6, 0x06,
                ],
            ),
            (
                Kind::PackedMaterializeZero,
                Family::PackedMaterializeZero,
                &["xmm0"],
                &[0xc5, 0xf9, 0xef, 0xc0],
            ),
            (
                Kind::PackedAddI16x8Reg,
                Family::PackedAddI16x8Reg,
                &["xmm1", "xmm2", "xmm0"],
                &[0xc5, 0xf1, 0xfd, 0xc2],
            ),
        ];
        for &(kind, family, names, expected) in cases {
            let ops: Vec<_> = names.iter().map(|n| v(n)).collect();
            let key = MachineAlternativeKey { family, variant: 0 };
            let encoded = encode_x86_64_selected_form(&physical, kind, key, &ops).unwrap();
            assert_eq!(encoded.bytes(), expected, "{kind:?}");
            assert_eq!(
                validate_x86_64_selected_form_encoding(&physical, kind, key, &ops, expected)
                    .unwrap(),
                encoded
            );
            assert_every_bit_flip_rejected(expected, |b| {
                validate_x86_64_selected_form_encoding(&physical, kind, key, &ops, b).is_err()
            });
            assert_byte_fragility(expected, |b| {
                validate_x86_64_selected_form_encoding(&physical, kind, key, &ops, b).is_err()
            });
        }
        assert!(
            encode_x86_64_selected_form(
                &physical,
                Kind::PackedShiftRightArithmeticI16x8 { count: 16 },
                MachineAlternativeKey {
                    family: Family::PackedShiftRightArithmeticI16x8,
                    variant: 0
                },
                &[v("rdi"), v("rsi")],
            )
            .is_err()
        );
        assert!(
            encode_x86_64_selected_form(
                &physical,
                Kind::PackedShuffleGeneralI16x8 {
                    lanes: [16, 0, 0, 0, 0, 0, 0, 0]
                },
                MachineAlternativeKey {
                    family: Family::PackedShuffleGeneralI16x8,
                    variant: 0
                },
                &[v("rdi"), v("rsi"), v("rdx"), v("r8")],
            )
            .is_err()
        );
    }
}
