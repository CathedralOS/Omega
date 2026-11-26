//! SSE2 operations for 8-lane 16-bit and 8-bit packed integer arrays.
//! Omega arrays only guarantee their element alignment, so 16-byte memory forms use MOVDQU
//! and 8-byte memory forms use MOVQ.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Instruction {
    LoadUnaligned {
        destination_xmm: u8,
        base: u8,
    },
    StoreUnaligned {
        base: u8,
        source_xmm: u8,
    },
    LoadQuad {
        destination_xmm: u8,
        base: u8,
    },
    StoreQuad {
        base: u8,
        source_xmm: u8,
    },
    LoadDword {
        destination_xmm: u8,
        base: u8,
    },
    StoreDword {
        base: u8,
        source_xmm: u8,
    },
    AddWords {
        destination_xmm: u8,
        source_xmm: u8,
    },
    SubWords {
        destination_xmm: u8,
        source_xmm: u8,
    },
    UnpackLowBytes {
        destination_xmm: u8,
        source_xmm: u8,
    },
    UnpackLowWords {
        destination_xmm: u8,
        source_xmm: u8,
    },
    UnpackHighWords {
        destination_xmm: u8,
        source_xmm: u8,
    },
    UnpackLowDoublewords {
        destination_xmm: u8,
        source_xmm: u8,
    },
    UnpackHighDoublewords {
        destination_xmm: u8,
        source_xmm: u8,
    },
    UnpackLowQuadwords {
        destination_xmm: u8,
        source_xmm: u8,
    },
    UnpackHighQuadwords {
        destination_xmm: u8,
        source_xmm: u8,
    },
    XorPacked {
        destination_xmm: u8,
        source_xmm: u8,
    },
    PackUnsignedSaturatingWordsToBytes {
        destination_xmm: u8,
        source_xmm: u8,
    },
    ShiftRightArithmeticWords {
        xmm: u8,
        count: u8,
    },
    ExtractWord {
        destination_gpr: u8,
        source_xmm: u8,
        lane: u8,
    },
    InsertWord {
        destination_xmm: u8,
        source_gpr: u8,
        lane: u8,
    },
    VmovdquLoad {
        destination_xmm: u8,
        base: u8,
        disp: i32,
    },
    VmovdquStore {
        base: u8,
        disp: i32,
        source_xmm: u8,
    },
    VmovqLoad {
        destination_xmm: u8,
        base: u8,
        disp: i32,
    },
    VmovqStore {
        base: u8,
        disp: i32,
        source_xmm: u8,
    },
    VmovdLoad {
        destination_xmm: u8,
        base: u8,
        disp: i32,
    },
    VmovdStore {
        base: u8,
        disp: i32,
        source_xmm: u8,
    },
    VmovqFromGpr {
        destination_xmm: u8,
        source_gpr: u8,
    },
    Vpinsrq {
        destination_xmm: u8,
        source_xmm: u8,
        source_gpr: u8,
        lane: u8,
    },
    Vpxor {
        destination_xmm: u8,
        left_xmm: u8,
        right_xmm: u8,
    },
    Vpaddw {
        destination_xmm: u8,
        left_xmm: u8,
        right_xmm: u8,
    },
    Vpsubw {
        destination_xmm: u8,
        left_xmm: u8,
        right_xmm: u8,
    },
    VpsrawImm {
        destination_xmm: u8,
        source_xmm: u8,
        count: u8,
    },
    Vpunpcklwd {
        destination_xmm: u8,
        left_xmm: u8,
        right_xmm: u8,
    },
    Vpunpckhwd {
        destination_xmm: u8,
        left_xmm: u8,
        right_xmm: u8,
    },
    Vpunpckldq {
        destination_xmm: u8,
        left_xmm: u8,
        right_xmm: u8,
    },
    Vpunpckhdq {
        destination_xmm: u8,
        left_xmm: u8,
        right_xmm: u8,
    },
    Vpunpcklqdq {
        destination_xmm: u8,
        left_xmm: u8,
        right_xmm: u8,
    },
    Vpunpckhqdq {
        destination_xmm: u8,
        left_xmm: u8,
        right_xmm: u8,
    },
    Vpmovzxbw {
        destination_xmm: u8,
        source_xmm: u8,
    },
    Vpackuswb {
        destination_xmm: u8,
        left_xmm: u8,
        right_xmm: u8,
    },
}

#[derive(Clone, Copy)]
struct VexSpec {
    map: u8, // 1 = 0F, 2 = 0F38, 3 = 0F3A
    pp: u8,  // 0 = none, 1 = 66, 2 = F3, 3 = F2
    w: bool,
    vvvv: u8,
    opcode: u8,
    reg: u8,
    rm: u8,
    mem_disp: Option<i32>,
    imm8: Option<u8>,
}

impl Instruction {
    fn vex_spec(self) -> Option<VexSpec> {
        let reg_bin = |pp: u8, opcode: u8, dst: u8, lhs: u8, rhs: u8| {
            Some(VexSpec {
                map: 1,
                pp,
                w: false,
                vvvv: lhs,
                opcode,
                reg: dst,
                rm: rhs,
                mem_disp: None,
                imm8: None,
            })
        };
        let mem = |pp: u8, opcode: u8, reg: u8, base: u8, disp: i32| {
            Some(VexSpec {
                map: 1,
                pp,
                w: false,
                vvvv: 0,
                opcode,
                reg,
                rm: base,
                mem_disp: Some(disp),
                imm8: None,
            })
        };
        match self {
            Self::VmovdquLoad {
                destination_xmm,
                base,
                disp,
            } => mem(2, 0x6f, destination_xmm, base, disp),
            Self::VmovdquStore {
                base,
                disp,
                source_xmm,
            } => mem(2, 0x7f, source_xmm, base, disp),
            Self::VmovqLoad {
                destination_xmm,
                base,
                disp,
            } => mem(2, 0x7e, destination_xmm, base, disp),
            Self::VmovqStore {
                base,
                disp,
                source_xmm,
            } => mem(1, 0xd6, source_xmm, base, disp),
            Self::VmovdLoad {
                destination_xmm,
                base,
                disp,
            } => mem(1, 0x6e, destination_xmm, base, disp),
            Self::VmovdStore {
                base,
                disp,
                source_xmm,
            } => mem(1, 0x7e, source_xmm, base, disp),
            Self::VmovqFromGpr {
                destination_xmm,
                source_gpr,
            } => Some(VexSpec {
                map: 1,
                pp: 1,
                w: true,
                vvvv: 0,
                opcode: 0x6e,
                reg: destination_xmm,
                rm: source_gpr,
                mem_disp: None,
                imm8: None,
            }),
            Self::Vpinsrq {
                destination_xmm,
                source_xmm,
                source_gpr,
                lane,
            } => Some(VexSpec {
                map: 3,
                pp: 1,
                w: true,
                vvvv: source_xmm,
                opcode: 0x22,
                reg: destination_xmm,
                rm: source_gpr,
                mem_disp: None,
                imm8: Some(lane),
            }),
            Self::Vpxor {
                destination_xmm,
                left_xmm,
                right_xmm,
            } => reg_bin(1, 0xef, destination_xmm, left_xmm, right_xmm),
            Self::Vpaddw {
                destination_xmm,
                left_xmm,
                right_xmm,
            } => reg_bin(1, 0xfd, destination_xmm, left_xmm, right_xmm),
            Self::Vpsubw {
                destination_xmm,
                left_xmm,
                right_xmm,
            } => reg_bin(1, 0xf9, destination_xmm, left_xmm, right_xmm),
            Self::VpsrawImm {
                destination_xmm,
                source_xmm,
                count,
            } => Some(VexSpec {
                map: 1,
                pp: 1,
                w: false,
                vvvv: destination_xmm,
                opcode: 0x71,
                reg: 4,
                rm: source_xmm,
                mem_disp: None,
                imm8: Some(count),
            }),
            Self::Vpunpcklwd {
                destination_xmm,
                left_xmm,
                right_xmm,
            } => reg_bin(1, 0x61, destination_xmm, left_xmm, right_xmm),
            Self::Vpunpckhwd {
                destination_xmm,
                left_xmm,
                right_xmm,
            } => reg_bin(1, 0x69, destination_xmm, left_xmm, right_xmm),
            Self::Vpunpckldq {
                destination_xmm,
                left_xmm,
                right_xmm,
            } => reg_bin(1, 0x62, destination_xmm, left_xmm, right_xmm),
            Self::Vpunpckhdq {
                destination_xmm,
                left_xmm,
                right_xmm,
            } => reg_bin(1, 0x6a, destination_xmm, left_xmm, right_xmm),
            Self::Vpunpcklqdq {
                destination_xmm,
                left_xmm,
                right_xmm,
            } => reg_bin(1, 0x6c, destination_xmm, left_xmm, right_xmm),
            Self::Vpunpckhqdq {
                destination_xmm,
                left_xmm,
                right_xmm,
            } => reg_bin(1, 0x6d, destination_xmm, left_xmm, right_xmm),
            Self::Vpmovzxbw {
                destination_xmm,
                source_xmm,
            } => Some(VexSpec {
                map: 2,
                pp: 1,
                w: false,
                vvvv: 0,
                opcode: 0x30,
                reg: destination_xmm,
                rm: source_xmm,
                mem_disp: None,
                imm8: None,
            }),
            Self::Vpackuswb {
                destination_xmm,
                left_xmm,
                right_xmm,
            } => reg_bin(1, 0x67, destination_xmm, left_xmm, right_xmm),
            _ => None,
        }
    }

    fn encode_vex(spec: VexSpec, output: &mut Vec<u8>) -> Option<()> {
        if spec.reg >= 16 || spec.rm >= 16 || spec.vvvv >= 16 {
            return None;
        }
        match spec.opcode {
            0x71 if spec.imm8.is_some_and(|c| c >= 16) => return None,
            0x22 if spec.imm8.is_some_and(|l| l >= 2) => return None,
            _ => {}
        }
        let inv_r = ((spec.reg >> 3) ^ 1) << 7;
        let inv_vvvv = ((!spec.vvvv) & 0x0f) << 3;
        if spec.map == 1 && !spec.w && spec.rm < 8 {
            output.extend([0xc5, inv_r | inv_vvvv | spec.pp]);
        } else {
            let inv_b = ((spec.rm >> 3) ^ 1) << 5;
            let b1 = inv_r | 0x40 | inv_b | spec.map;
            let b2 = (u8::from(spec.w) << 7) | inv_vvvv | spec.pp;
            output.extend([0xc4, b1, b2]);
        }
        output.push(spec.opcode);
        let low_rm = spec.rm & 7;
        match spec.mem_disp {
            None => {
                output.push(0xc0 | ((spec.reg & 7) << 3) | low_rm);
            }
            Some(disp) => {
                let (mode, disp_len) = if disp == 0 && low_rm != 5 {
                    (0_u8, 0_usize)
                } else if i8::try_from(disp).is_ok() {
                    (1, 1)
                } else {
                    (2, 4)
                };
                output.push((mode << 6) | ((spec.reg & 7) << 3) | low_rm);
                if low_rm == 4 {
                    output.push(0x24);
                }
                match disp_len {
                    0 => {}
                    1 => output.push((disp as i8) as u8),
                    4 => output.extend(disp.to_le_bytes()),
                    _ => unreachable!(),
                }
            }
        }
        if let Some(imm) = spec.imm8 {
            output.push(imm);
        }
        Some(())
    }

    fn decode_vex(bytes: &[u8]) -> Option<(Self, &[u8])> {
        let (&lead, rest) = bytes.split_first()?;
        let (map, r_high, b_high, w, vvvv, pp, after_vex) = match lead {
            0xc5 => {
                let (&b1, tail) = rest.split_first()?;
                if (b1 & 0x04) != 0 {
                    return None; // L must be 0
                }
                let r_high = ((b1 >> 7) ^ 1) << 3;
                let vvvv = (!(b1 >> 3)) & 0x0f;
                (1_u8, r_high, 0_u8, false, vvvv, b1 & 3, tail)
            }
            0xc4 => {
                let (&b1, tail) = rest.split_first()?;
                let (&b2, tail) = tail.split_first()?;
                if (b1 & 0x40) == 0 || (b2 & 0x04) != 0 {
                    return None; // X=0 (~X=1) and L=0 required
                }
                let map = b1 & 0x1f;
                let r_high = ((b1 >> 7) ^ 1) << 3;
                let b_high = (((b1 >> 5) & 1) ^ 1) << 3;
                let w = (b2 >> 7) != 0;
                let vvvv = (!(b2 >> 3)) & 0x0f;
                let pp = b2 & 3;
                if map == 1 && !w && b_high == 0 {
                    return None; // Non-canonical 3-byte VEX when 2-byte applies
                }
                (map, r_high, b_high, w, vvvv, pp, tail)
            }
            _ => return None,
        };
        let (&opcode, after_op) = after_vex.split_first()?;
        let (&modrm, after_modrm) = after_op.split_first()?;
        let mode = modrm >> 6;
        let reg = r_high | ((modrm >> 3) & 7);
        let rm = b_high | (modrm & 7);
        let low_rm = rm & 7;
        if mode == 3 {
            let (instruction, tail) = match (map, pp, w, opcode) {
                (1, 1, false, 0xef) => (
                    Self::Vpxor {
                        destination_xmm: reg,
                        left_xmm: vvvv,
                        right_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, false, 0xfd) => (
                    Self::Vpaddw {
                        destination_xmm: reg,
                        left_xmm: vvvv,
                        right_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, false, 0xf9) => (
                    Self::Vpsubw {
                        destination_xmm: reg,
                        left_xmm: vvvv,
                        right_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, false, 0x61) => (
                    Self::Vpunpcklwd {
                        destination_xmm: reg,
                        left_xmm: vvvv,
                        right_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, false, 0x69) => (
                    Self::Vpunpckhwd {
                        destination_xmm: reg,
                        left_xmm: vvvv,
                        right_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, false, 0x62) => (
                    Self::Vpunpckldq {
                        destination_xmm: reg,
                        left_xmm: vvvv,
                        right_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, false, 0x6a) => (
                    Self::Vpunpckhdq {
                        destination_xmm: reg,
                        left_xmm: vvvv,
                        right_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, false, 0x6c) => (
                    Self::Vpunpcklqdq {
                        destination_xmm: reg,
                        left_xmm: vvvv,
                        right_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, false, 0x6d) => (
                    Self::Vpunpckhqdq {
                        destination_xmm: reg,
                        left_xmm: vvvv,
                        right_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, false, 0x67) => (
                    Self::Vpackuswb {
                        destination_xmm: reg,
                        left_xmm: vvvv,
                        right_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, false, 0x71) if reg == 4 => {
                    let (&count, tail) = after_modrm.split_first()?;
                    if count >= 16 {
                        return None;
                    }
                    (
                        Self::VpsrawImm {
                            destination_xmm: vvvv,
                            source_xmm: rm,
                            count,
                        },
                        tail,
                    )
                }
                (2, 1, false, 0x30) if vvvv == 0 => (
                    Self::Vpmovzxbw {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    after_modrm,
                ),
                (1, 1, true, 0x6e) if vvvv == 0 => (
                    Self::VmovqFromGpr {
                        destination_xmm: reg,
                        source_gpr: rm,
                    },
                    after_modrm,
                ),
                (3, 1, true, 0x22) => {
                    let (&lane, tail) = after_modrm.split_first()?;
                    if lane >= 2 {
                        return None;
                    }
                    (
                        Self::Vpinsrq {
                            destination_xmm: reg,
                            source_xmm: vvvv,
                            source_gpr: rm,
                            lane,
                        },
                        tail,
                    )
                }
                _ => return None,
            };
            return Some((instruction, tail));
        }
        if vvvv != 0 || w {
            return None;
        }
        let after_sib = if low_rm == 4 {
            let (&sib, tail) = after_modrm.split_first()?;
            if sib != 0x24 {
                return None;
            }
            tail
        } else {
            after_modrm
        };
        let (disp, tail) = match (mode, low_rm) {
            (0, 5) => return None,
            (0, _) => (0_i32, after_sib),
            (1, _) => {
                let (&d8, tail) = after_sib.split_first()?;
                let disp = i32::from(d8 as i8);
                if disp == 0 && low_rm != 5 {
                    return None; // Must use mod=0 when disp==0 and base!=rbp/r13
                }
                (disp, tail)
            }
            (2, _) => {
                let (d_bytes, tail) = after_sib.split_at_checked(4)?;
                let disp = i32::from_le_bytes([d_bytes[0], d_bytes[1], d_bytes[2], d_bytes[3]]);
                if i8::try_from(disp).is_ok() {
                    return None; // Must use mod=0 or mod=1
                }
                (disp, tail)
            }
            _ => return None,
        };
        let instruction = match (map, pp, opcode) {
            (1, 2, 0x6f) => Self::VmovdquLoad {
                destination_xmm: reg,
                base: rm,
                disp,
            },
            (1, 2, 0x7f) => Self::VmovdquStore {
                base: rm,
                disp,
                source_xmm: reg,
            },
            (1, 2, 0x7e) => Self::VmovqLoad {
                destination_xmm: reg,
                base: rm,
                disp,
            },
            (1, 1, 0xd6) => Self::VmovqStore {
                base: rm,
                disp,
                source_xmm: reg,
            },
            (1, 1, 0x6e) => Self::VmovdLoad {
                destination_xmm: reg,
                base: rm,
                disp,
            },
            (1, 1, 0x7e) => Self::VmovdStore {
                base: rm,
                disp,
                source_xmm: reg,
            },
            _ => return None,
        };
        Some((instruction, tail))
    }
}

impl Instruction {
    /// Append one instruction. Invalid register numbers or immediates leave the output intact.
    pub fn encode(self, output: &mut Vec<u8>) -> Option<()> {
        if let Some(vex) = self.vex_spec() {
            return Self::encode_vex(vex, output);
        }
        let (prefix, opcode, reg, rm, register_operand, immediate) = match self {
            Self::LoadUnaligned {
                destination_xmm,
                base,
            } => (0xf3, 0x6f, destination_xmm, base, false, None),
            Self::StoreUnaligned { base, source_xmm } => {
                (0xf3, 0x7f, source_xmm, base, false, None)
            }
            Self::LoadQuad {
                destination_xmm,
                base,
            } => (0xf3, 0x7e, destination_xmm, base, false, None),
            Self::StoreQuad { base, source_xmm } => (0x66, 0xd6, source_xmm, base, false, None),
            Self::LoadDword {
                destination_xmm,
                base,
            } => (0x66, 0x6e, destination_xmm, base, false, None),
            Self::StoreDword { base, source_xmm } => (0x66, 0x7e, source_xmm, base, false, None),
            Self::AddWords {
                destination_xmm,
                source_xmm,
            } => (0x66, 0xfd, destination_xmm, source_xmm, true, None),
            Self::SubWords {
                destination_xmm,
                source_xmm,
            } => (0x66, 0xf9, destination_xmm, source_xmm, true, None),
            Self::UnpackLowBytes {
                destination_xmm,
                source_xmm,
            } => (0x66, 0x60, destination_xmm, source_xmm, true, None),
            Self::UnpackLowWords {
                destination_xmm,
                source_xmm,
            } => (0x66, 0x61, destination_xmm, source_xmm, true, None),
            Self::UnpackHighWords {
                destination_xmm,
                source_xmm,
            } => (0x66, 0x69, destination_xmm, source_xmm, true, None),
            Self::UnpackLowDoublewords {
                destination_xmm,
                source_xmm,
            } => (0x66, 0x62, destination_xmm, source_xmm, true, None),
            Self::UnpackHighDoublewords {
                destination_xmm,
                source_xmm,
            } => (0x66, 0x6a, destination_xmm, source_xmm, true, None),
            Self::UnpackLowQuadwords {
                destination_xmm,
                source_xmm,
            } => (0x66, 0x6c, destination_xmm, source_xmm, true, None),
            Self::UnpackHighQuadwords {
                destination_xmm,
                source_xmm,
            } => (0x66, 0x6d, destination_xmm, source_xmm, true, None),
            Self::XorPacked {
                destination_xmm,
                source_xmm,
            } => (0x66, 0xef, destination_xmm, source_xmm, true, None),
            Self::PackUnsignedSaturatingWordsToBytes {
                destination_xmm,
                source_xmm,
            } => (0x66, 0x67, destination_xmm, source_xmm, true, None),
            Self::ShiftRightArithmeticWords { xmm, count } => {
                if count >= 16 {
                    return None;
                }
                (0x66, 0x71, 4, xmm, true, Some(count))
            }
            Self::ExtractWord {
                destination_gpr,
                source_xmm,
                lane,
            } => {
                if lane >= 8 {
                    return None;
                }
                (0x66, 0xc5, destination_gpr, source_xmm, true, Some(lane))
            }
            Self::InsertWord {
                destination_xmm,
                source_gpr,
                lane,
            } => {
                if lane >= 8 {
                    return None;
                }
                (0x66, 0xc4, destination_xmm, source_gpr, true, Some(lane))
            }
            _ => return None,
        };
        if reg >= 16 || rm >= 16 {
            return None;
        }
        output.push(prefix);
        let rex = 0x40 | ((reg >> 3) << 2) | (rm >> 3);
        if rex != 0x40 {
            output.push(rex);
        }
        let low_rm = rm & 7;
        // In memory operands RSP/R12 need a SIB and RBP/R13 need a displacement.
        let mode = if register_operand {
            3
        } else {
            u8::from(low_rm == 5)
        };
        output.extend([0x0f, opcode, mode << 6 | ((reg & 7) << 3) | low_rm]);
        if !register_operand {
            match low_rm {
                4 => output.push(0x24),
                5 => output.push(0),
                _ => {}
            }
        }
        if let Some(imm) = immediate {
            output.push(imm);
        }
        Some(())
    }

    /// Decode the canonical selected form and return the bytes left after it.
    pub fn decode(bytes: &[u8]) -> Option<(Self, &[u8])> {
        let (&prefix, rest_after_prefix) = bytes.split_first()?;
        if matches!(prefix, 0xc4 | 0xc5) {
            return Self::decode_vex(bytes);
        }
        let bytes = rest_after_prefix;
        let (reg_high, rm_high, bytes) = match bytes {
            [rex @ (0x41 | 0x44 | 0x45), rest @ ..] => ((rex & 4) << 1, (rex & 1) << 3, rest),
            _ => (0, 0, bytes),
        };
        let [0x0f, opcode, modrm, rest @ ..] = bytes else {
            return None;
        };
        let reg = reg_high | ((modrm >> 3) & 7);
        let rm = rm_high | (modrm & 7);
        if (prefix, modrm >> 6) == (0x66, 3) {
            return match opcode {
                0xfd => Some((
                    Self::AddWords {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0xf9 => Some((
                    Self::SubWords {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0x60 => Some((
                    Self::UnpackLowBytes {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0x61 => Some((
                    Self::UnpackLowWords {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0x69 => Some((
                    Self::UnpackHighWords {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0x62 => Some((
                    Self::UnpackLowDoublewords {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0x6a => Some((
                    Self::UnpackHighDoublewords {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0x6c => Some((
                    Self::UnpackLowQuadwords {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0x6d => Some((
                    Self::UnpackHighQuadwords {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0xef => Some((
                    Self::XorPacked {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0x67 => Some((
                    Self::PackUnsignedSaturatingWordsToBytes {
                        destination_xmm: reg,
                        source_xmm: rm,
                    },
                    rest,
                )),
                0x71 if reg == 4 => {
                    let (&count, tail) = rest.split_first()?;
                    (count < 16)
                        .then_some((Self::ShiftRightArithmeticWords { xmm: rm, count }, tail))
                }
                0xc5 => {
                    let (&lane, tail) = rest.split_first()?;
                    (lane < 8).then_some((
                        Self::ExtractWord {
                            destination_gpr: reg,
                            source_xmm: rm,
                            lane,
                        },
                        tail,
                    ))
                }
                0xc4 => {
                    let (&lane, tail) = rest.split_first()?;
                    (lane < 8).then_some((
                        Self::InsertWord {
                            destination_xmm: reg,
                            source_gpr: rm,
                            lane,
                        },
                        tail,
                    ))
                }
                _ => None,
            };
        }
        let rest = match (modrm >> 6, rm & 7, rest) {
            (0, 4, [0x24, tail @ ..]) | (1, 5, [0, tail @ ..]) => tail,
            (0, 0..=3 | 6..=7, tail) => tail,
            _ => return None,
        };
        let instruction = match (prefix, opcode) {
            (0xf3, 0x6f) => Self::LoadUnaligned {
                destination_xmm: reg,
                base: rm,
            },
            (0xf3, 0x7f) => Self::StoreUnaligned {
                base: rm,
                source_xmm: reg,
            },
            (0xf3, 0x7e) => Self::LoadQuad {
                destination_xmm: reg,
                base: rm,
            },
            (0x66, 0xd6) => Self::StoreQuad {
                base: rm,
                source_xmm: reg,
            },
            (0x66, 0x6e) => Self::LoadDword {
                destination_xmm: reg,
                base: rm,
            },
            (0x66, 0x7e) => Self::StoreDword {
                base: rm,
                source_xmm: reg,
            },
            _ => return None,
        };
        Some((instruction, rest))
    }
}

#[cfg(test)]
mod tests {
    use super::Instruction::*;
    use super::*;

    #[test]
    fn sse2_encodes_unaligned_addresses_and_register_extensions() {
        for (instruction, expected) in [
            (
                LoadUnaligned {
                    destination_xmm: 0,
                    base: 0,
                },
                &[0xf3, 0x0f, 0x6f, 0x00][..],
            ),
            (
                LoadUnaligned {
                    destination_xmm: 15,
                    base: 12,
                },
                &[0xf3, 0x45, 0x0f, 0x6f, 0x3c, 0x24],
            ),
            (
                StoreUnaligned {
                    base: 13,
                    source_xmm: 0,
                },
                &[0xf3, 0x41, 0x0f, 0x7f, 0x45, 0],
            ),
            (
                AddWords {
                    destination_xmm: 3,
                    source_xmm: 11,
                },
                &[0x66, 0x41, 0x0f, 0xfd, 0xdb],
            ),
            (
                LoadDword {
                    destination_xmm: 0,
                    base: 12,
                },
                &[0x66, 0x41, 0x0f, 0x6e, 0x04, 0x24],
            ),
            (
                StoreDword {
                    base: 13,
                    source_xmm: 9,
                },
                &[0x66, 0x45, 0x0f, 0x7e, 0x4d, 0x00],
            ),
        ] {
            let mut bytes = Vec::new();
            instruction.encode(&mut bytes).unwrap();
            assert_eq!(bytes, expected);
            assert_eq!(Instruction::decode(&bytes), Some((instruction, &[][..])));
            for length in 0..bytes.len() {
                assert!(Instruction::decode(&bytes[..length]).is_none());
            }
        }
    }

    #[test]
    fn each_register_and_base_round_trips_without_aliasing() {
        for first in 0..16 {
            for second in 0..16 {
                for instruction in [
                    LoadUnaligned {
                        destination_xmm: first,
                        base: second,
                    },
                    StoreUnaligned {
                        base: second,
                        source_xmm: first,
                    },
                    AddWords {
                        destination_xmm: first,
                        source_xmm: second,
                    },
                    SubWords {
                        destination_xmm: first,
                        source_xmm: second,
                    },
                    UnpackLowBytes {
                        destination_xmm: first,
                        source_xmm: second,
                    },
                    UnpackLowWords {
                        destination_xmm: first,
                        source_xmm: second,
                    },
                    UnpackLowDoublewords {
                        destination_xmm: first,
                        source_xmm: second,
                    },
                    UnpackHighDoublewords {
                        destination_xmm: first,
                        source_xmm: second,
                    },
                    UnpackLowQuadwords {
                        destination_xmm: first,
                        source_xmm: second,
                    },
                    UnpackHighQuadwords {
                        destination_xmm: first,
                        source_xmm: second,
                    },
                    XorPacked {
                        destination_xmm: first,
                        source_xmm: second,
                    },
                    PackUnsignedSaturatingWordsToBytes {
                        destination_xmm: first,
                        source_xmm: second,
                    },
                    LoadQuad {
                        destination_xmm: first,
                        base: second,
                    },
                    StoreQuad {
                        base: second,
                        source_xmm: first,
                    },
                    LoadDword {
                        destination_xmm: first,
                        base: second,
                    },
                    StoreDword {
                        base: second,
                        source_xmm: first,
                    },
                    ShiftRightArithmeticWords {
                        xmm: first,
                        count: second & 15,
                    },
                    ExtractWord {
                        destination_gpr: first,
                        source_xmm: second,
                        lane: first & 7,
                    },
                    InsertWord {
                        destination_xmm: first,
                        source_gpr: second,
                        lane: second & 7,
                    },
                ] {
                    let mut bytes = Vec::new();
                    instruction.encode(&mut bytes).unwrap();
                    bytes.push(0xcc);
                    assert_eq!(
                        Instruction::decode(&bytes),
                        Some((instruction, &[0xcc][..]))
                    );
                }
            }
        }
        for invalid in [
            ShiftRightArithmeticWords { xmm: 0, count: 16 },
            ExtractWord {
                destination_gpr: 0,
                source_xmm: 0,
                lane: 8,
            },
            InsertWord {
                destination_xmm: 0,
                source_gpr: 0,
                lane: 8,
            },
        ] {
            let mut bytes = vec![0xcc];
            assert_eq!(invalid.encode(&mut bytes), None);
            assert_eq!(bytes, [0xcc]);
        }
        let mut bytes = vec![0xcc];
        assert_eq!(
            AddWords {
                destination_xmm: 16,
                source_xmm: 0
            }
            .encode(&mut bytes),
            None
        );
        assert_eq!(bytes, [0xcc]);
    }

    #[test]
    fn vex_encodes_ffmpeg_matching_bytes_and_round_trips_all_registers_and_displacements() {
        for (instruction, expected) in [
            (
                VmovqLoad {
                    destination_xmm: 0,
                    base: 2,
                    disp: 0,
                },
                &[0xc5, 0xfa, 0x7e, 0x02][..],
            ),
            (
                VpsrawImm {
                    destination_xmm: 4,
                    source_xmm: 1,
                    count: 1,
                },
                &[0xc5, 0xd9, 0x71, 0xe1, 0x01],
            ),
            (
                Vpaddw {
                    destination_xmm: 3,
                    left_xmm: 2,
                    right_xmm: 0,
                },
                &[0xc5, 0xe9, 0xfd, 0xd8],
            ),
            (
                Vpxor {
                    destination_xmm: 6,
                    left_xmm: 6,
                    right_xmm: 6,
                },
                &[0xc5, 0xc9, 0xef, 0xf6],
            ),
            (
                VmovdLoad {
                    destination_xmm: 5,
                    base: 1,
                    disp: 0,
                },
                &[0xc5, 0xf9, 0x6e, 0x29],
            ),
            (
                VmovdStore {
                    base: 1,
                    disp: 0,
                    source_xmm: 5,
                },
                &[0xc5, 0xf9, 0x7e, 0x29],
            ),
        ] {
            let mut bytes = Vec::new();
            instruction.encode(&mut bytes).unwrap();
            assert_eq!(bytes, expected, "{instruction:?}");
            assert_eq!(Instruction::decode(&bytes), Some((instruction, &[][..])));
        }

        for r0 in 0..16 {
            for r1 in 0..16 {
                for disp in [0, 8, -128, 127, 128, 0x1000] {
                    for instruction in [
                        VmovdquLoad {
                            destination_xmm: r0,
                            base: r1,
                            disp,
                        },
                        VmovdquStore {
                            base: r1,
                            disp,
                            source_xmm: r0,
                        },
                        VmovqLoad {
                            destination_xmm: r0,
                            base: r1,
                            disp,
                        },
                        VmovqStore {
                            base: r1,
                            disp,
                            source_xmm: r0,
                        },
                        VmovdLoad {
                            destination_xmm: r0,
                            base: r1,
                            disp,
                        },
                        VmovdStore {
                            base: r1,
                            disp,
                            source_xmm: r0,
                        },
                    ] {
                        let mut bytes = Vec::new();
                        instruction.encode(&mut bytes).unwrap();
                        bytes.push(0xcc);
                        assert_eq!(
                            Instruction::decode(&bytes),
                            Some((instruction, &[0xcc][..])),
                            "{instruction:?}"
                        );
                    }
                }
                for instruction in [
                    VmovqFromGpr {
                        destination_xmm: r0,
                        source_gpr: r1,
                    },
                    Vpinsrq {
                        destination_xmm: r0,
                        source_xmm: (r0 + 1) & 15,
                        source_gpr: r1,
                        lane: 1,
                    },
                    Vpxor {
                        destination_xmm: r0,
                        left_xmm: r1,
                        right_xmm: (r0 ^ r1) & 15,
                    },
                    Vpaddw {
                        destination_xmm: r0,
                        left_xmm: r1,
                        right_xmm: (r0 + 3) & 15,
                    },
                    Vpsubw {
                        destination_xmm: r0,
                        left_xmm: r1,
                        right_xmm: (r0 + 5) & 15,
                    },
                    VpsrawImm {
                        destination_xmm: r0,
                        source_xmm: r1,
                        count: (r0 + r1) & 15,
                    },
                    Vpunpcklwd {
                        destination_xmm: r0,
                        left_xmm: r1,
                        right_xmm: (r0 + 1) & 15,
                    },
                    Vpunpckhwd {
                        destination_xmm: r0,
                        left_xmm: r1,
                        right_xmm: (r0 + 1) & 15,
                    },
                    Vpunpckldq {
                        destination_xmm: r0,
                        left_xmm: r1,
                        right_xmm: (r0 + 2) & 15,
                    },
                    Vpunpckhdq {
                        destination_xmm: r0,
                        left_xmm: r1,
                        right_xmm: (r0 + 3) & 15,
                    },
                    Vpunpcklqdq {
                        destination_xmm: r0,
                        left_xmm: r1,
                        right_xmm: (r0 + 4) & 15,
                    },
                    Vpunpckhqdq {
                        destination_xmm: r0,
                        left_xmm: r1,
                        right_xmm: (r0 + 5) & 15,
                    },
                    Vpmovzxbw {
                        destination_xmm: r0,
                        source_xmm: r1,
                    },
                    Vpackuswb {
                        destination_xmm: r0,
                        left_xmm: r1,
                        right_xmm: r1,
                    },
                ] {
                    let mut bytes = Vec::new();
                    instruction.encode(&mut bytes).unwrap();
                    bytes.push(0xcc);
                    assert_eq!(
                        Instruction::decode(&bytes),
                        Some((instruction, &[0xcc][..])),
                        "{instruction:?}"
                    );
                }
            }
        }
        for (sse2, vex) in [
            (
                UnpackHighWords {
                    destination_xmm: 2,
                    source_xmm: 9,
                },
                Vpunpckhwd {
                    destination_xmm: 2,
                    left_xmm: 5,
                    right_xmm: 9,
                },
            ),
            (
                UnpackHighWords {
                    destination_xmm: 9,
                    source_xmm: 2,
                },
                Vpunpckhwd {
                    destination_xmm: 9,
                    left_xmm: 2,
                    right_xmm: 4,
                },
            ),
        ] {
            for bytes_target in [encode_check(sse2), encode_check(vex)] {
                assert!(!bytes_target.is_empty());
            }
        }
    }

    fn encode_check(instruction: Instruction) -> Vec<u8> {
        let mut bytes = Vec::new();
        instruction.encode(&mut bytes).unwrap();
        assert_eq!(Instruction::decode(&bytes), Some((instruction, &[][..])));
        bytes
    }
}
