use super::request_validation::{guard_canonical, resolve_registers, validate_request};
use super::{
    MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedEffects,
    RegisterViewId, SelectedInstructionKind as Kind, ValidatedPhysicalRegisterModel,
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
    X86_64SelectedFormFootprint,
};
use selected_instructions::{
    FloatResultPolicyForm, IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation as FloatOp,
    IeeeFloatBinaryOperation, IeeeFloatIntegerCarrier as IntCarrier, IeeeFloatRoundingDirection,
    IeeeFloatToIntegerDomain, LocalStorageSlotId, MachineSemanticKind,
    float_bit_transfer_properties,
};

pub(super) mod float_bits {
    use super::*;

    fn operands(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        values: &[RegisterViewId],
    ) -> Result<(u8, u8, bool, bool), Error> {
        guard_canonical(physical)?;
        let (family, to_bits, wide) =
            float_bit_transfer_properties(kind).ok_or(Error::AlternativeMismatch)?;
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Error::AlternativeMismatch);
        }
        let &[source, destination] = values else {
            return Err(Error::OperandCountMismatch);
        };
        let (float, bits) = if to_bits {
            (source, destination)
        } else {
            (destination, source)
        };
        let float_view = physical
            .model()
            .view(float)
            .ok_or(Error::UnknownOrNonGpr64View(float))?;
        let float_reg = float_view
            .name
            .strip_prefix("xmm")
            .and_then(|s| s.parse::<u8>().ok())
            .filter(|&n| n <= 15)
            .ok_or(Error::UnknownOrNonGpr64View(float))?;
        if float_view.bits != 128 || !float_view.allocatable {
            return Err(Error::UnknownOrNonGpr64View(float));
        }
        Ok((
            float_reg,
            resolve_registers(physical, &[bits])?[0],
            to_bits,
            wide,
        ))
    }

    fn encode_bytes(float: u8, bits: u8, to_bits: bool, wide: bool) -> Vec<u8> {
        let mut bytes = vec![0x66];
        let rex = 0x40 | (u8::from(wide) << 3) | ((float >> 3) << 2) | (bits >> 3);
        if rex != 0x40 {
            bytes.push(rex);
        }
        bytes.extend([
            0x0f,
            if to_bits { 0x7e } else { 0x6e },
            0xc0 | ((float & 7) << 3) | (bits & 7),
        ]);
        bytes
    }

    fn finish(values: &[RegisterViewId], bytes: Vec<u8>) -> ValidatedX86_64SelectedFormEncoding {
        ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: X86_64SelectedFormFootprint {
                register_reads: vec![values[0]],
                register_writes: vec![values[1]],
                writes_rflags: false,
                encoded: MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]),
            },
        }
    }

    pub(in super::super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        values: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (float, bits, to_bits, wide) = operands(physical, kind, alternative, values)?;
        Ok(finish(values, encode_bytes(float, bits, to_bits, wide)))
    }

    pub(in super::super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        values: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (float, bits, to_bits, wide) = operands(physical, kind, alternative, values)?;
        let expected = encode_bytes(float, bits, to_bits, wide);
        if bytes.is_empty() || bytes[0] != 0x66 {
            return Err(Error::MalformedEncoding);
        }
        if bytes != expected {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(finish(values, bytes.to_vec()))
    }
}

pub(crate) mod floating_control {
    use super::*;
    pub(crate) use target_x86_64::form_effects::floating_control::effects;

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<(bool, u32), Error> {
        let semantic = match kind {
            Kind::SaveFloatingControl {
                slot: LocalStorageSlotId::Boundary { .. },
            } => MachineSemanticKind::SaveFloatingControl,
            Kind::RestoreFloatingControl {
                slot: LocalStorageSlotId::Boundary { .. },
            } => MachineSemanticKind::RestoreFloatingControl,
            _ => return Err(Error::EncodedFormMismatch),
        };
        if physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
            || !operands.is_empty()
            || alternative
                != (MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                })
            || !(0..=i64::from(i32::MAX)).contains(&displacement)
            || displacement % 8 != 0
        {
            return Err(Error::EncodedFormMismatch);
        }
        let offset = u32::try_from(displacement).map_err(|_| Error::EncodedFormMismatch)?;
        Ok((
            semantic == MachineSemanticKind::RestoreFloatingControl,
            offset,
        ))
    }

    fn encode_bytes(restore: bool, offset: u32) -> Result<Vec<u8>, Error> {
        if restore {
            crate::encode_ldmxcsr_rsp_displacement(offset)
        } else {
            crate::encode_stmxcsr_rsp_displacement(offset)
        }
        .map_err(|_| Error::EncodedFormMismatch)
    }

    fn finish(restore: bool, bytes: Vec<u8>) -> ValidatedX86_64SelectedFormEncoding {
        ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: X86_64SelectedFormFootprint {
                register_reads: Vec::new(),
                register_writes: Vec::new(),
                writes_rflags: false,
                encoded: effects(restore),
            },
        }
    }

    pub(crate) fn encode_x86_64_selected_floating_control_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (restore, offset) = request(physical, kind, alternative, operands, displacement)?;
        Ok(finish(restore, encode_bytes(restore, offset)?))
    }

    pub(crate) fn validate_x86_64_selected_floating_control_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (restore, offset) = request(physical, kind, alternative, operands, displacement)?;
        if bytes != encode_bytes(restore, offset)? {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(finish(restore, bytes.to_vec()))
    }
}

pub(crate) mod float_result_policy {
    use super::*;
    pub(crate) use target_x86_64::form_effects::float_result_policy::effects;

    pub(crate) fn footprint(
        form: FloatResultPolicyForm,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        let inputs = form.operand_count();
        X86_64SelectedFormFootprint {
            register_reads: operands[..inputs].to_vec(),
            register_writes: operands[inputs..inputs + 1].to_vec(),
            writes_rflags: true,
            encoded: effects(form),
        }
    }

    #[derive(Clone, Copy)]
    enum Op {
        Xfer(bool, bool, u8, u8),
        Shift(bool, bool, u8, u8),
        Cmp(bool, u8, u32),
        Jcc(bool, i32),
        Jmp(i32),
        Sub1(bool, u8),
        Crash,
    }

    impl Op {
        const fn size(self) -> usize {
            match self {
                Self::Xfer(..) | Self::Jmp(_) => 5,
                Self::Shift(..) | Self::Sub1(..) => 4,
                Self::Cmp(..) => 7,
                Self::Jcc(..) => 6,
                Self::Crash => 2,
            }
        }
    }

    fn encode_bytes(form: FloatResultPolicyForm, regs: &[u8]) -> Vec<u8> {
        let wide = !form.is_binary32();
        let (frac, exp_mask) = if wide { (52u8, 2047u32) } else { (23, 255) };
        let inputs = form.operand_count();
        let g = regs[inputs];
        let mut ops: Vec<Op> = regs[..inputs]
            .iter()
            .enumerate()
            .map(|(i, &src)| Op::Xfer(true, wide, 3 + i as u8, src))
            .collect();
        if form.can_trap() {
            ops.extend([
                Op::Xfer(false, wide, 3, g),
                Op::Shift(true, wide, g, 1),
                Op::Shift(false, wide, g, frac + 1),
                Op::Cmp(wide, g, exp_mask),
                Op::Jcc(false, 2),
                Op::Crash,
                Op::Xfer(false, wide, 3, g),
            ]);
        } else {
            let mut branches = Vec::with_capacity(3);
            let s1 = if wide { 56 } else { 24 };
            ops.extend([
                Op::Xfer(false, wide, 3, g),
                Op::Shift(true, wide, g, s1),
                Op::Shift(false, wide, g, s1),
                Op::Cmp(wide, g, 0),
            ]);
            branches.push(ops.len());
            ops.push(Op::Jcc(true, 0));
            ops.extend([
                Op::Xfer(false, wide, 4, g),
                Op::Shift(true, wide, g, 1),
                Op::Shift(false, wide, g, frac + 1),
                Op::Cmp(wide, g, exp_mask),
            ]);
            branches.push(ops.len());
            ops.push(Op::Jcc(false, 0));
            ops.extend([
                Op::Xfer(false, wide, 4, g),
                Op::Shift(true, wide, g, if wide { 12 } else { 9 }),
                Op::Cmp(wide, g, 0),
            ]);
            branches.push(ops.len());
            ops.push(Op::Jcc(false, 0));
            ops.extend([Op::Xfer(false, wide, 4, g), Op::Sub1(wide, g), Op::Jmp(5)]);
            let target: usize = ops.iter().map(|o| o.size()).sum();
            ops.push(Op::Xfer(false, wide, 4, g));
            for pos in branches {
                let next: usize = ops[..=pos].iter().map(|o| o.size()).sum();
                if let Op::Jcc(_, d) = &mut ops[pos] {
                    *d = (target - next) as i32;
                }
            }
        }
        let mut bytes = Vec::with_capacity(ops.iter().map(|o| o.size()).sum());
        let rex = |w: bool, r: u8| 0x40 | (u8::from(w) << 3) | (r >> 3);
        for op in ops {
            match op {
                Op::Xfer(save, w, vec, r) => bytes.extend([
                    0x66,
                    rex(w, r),
                    0x0f,
                    if save { 0x6e } else { 0x7e },
                    0xc0 | (vec << 3) | (r & 7),
                ]),
                Op::Shift(left, w, r, c) => bytes.extend([
                    rex(w, r),
                    0xc1,
                    0xc0 | ((if left { 4 } else { 5 }) << 3) | (r & 7),
                    c,
                ]),
                Op::Cmp(w, r, imm) => {
                    bytes.extend([rex(w, r), 0x81, 0xf8 | (r & 7)]);
                    bytes.extend(imm.to_le_bytes());
                }
                Op::Jcc(eq, disp) => {
                    bytes.extend([0x0f, if eq { 0x84 } else { 0x85 }]);
                    bytes.extend(disp.to_le_bytes());
                }
                Op::Jmp(disp) => {
                    bytes.push(0xe9);
                    bytes.extend(disp.to_le_bytes());
                }
                Op::Sub1(w, r) => bytes.extend([rex(w, r), 0x83, 0xe8 | (r & 7), 1]),
                Op::Crash => bytes.extend([0x0f, 0x0b]),
            }
        }
        bytes
    }

    fn regs(
        physical: &ValidatedPhysicalRegisterModel,
        form: FloatResultPolicyForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, Error> {
        validate_request(
            physical,
            Kind::FloatResultPolicy { form },
            alternative,
            operands,
        )?;
        resolve_registers(physical, operands)
    }

    pub(crate) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        form: FloatResultPolicyForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let r = regs(physical, form, alternative, operands)?;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: encode_bytes(form, &r),
            footprint: footprint(form, operands),
        })
    }

    pub(crate) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        form: FloatResultPolicyForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let r = regs(physical, form, alternative, operands)?;
        if bytes != encode_bytes(form, &r) {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: footprint(form, operands),
        })
    }
}

pub(crate) mod ieee_float_arithmetic {
    use super::*;
    pub(crate) use target_x86_64::form_effects::ieee_float_arithmetic::{
        conversion_control, effects,
    };

    const LEFT: u8 = 4;
    const RIGHT: u8 = 5;
    const ADDEND: u8 = 3;
    const TWO_TO_31: u64 = 0x41e0_0000_0000_0000;
    const TWO_TO_63: u64 = 0x43e0_0000_0000_0000;
    const TWO_TO_64: u64 = 0x43f0_0000_0000_0000;
    const U32_MAX_F64: u64 = 0x41ef_ffff_ffe0_0000;
    const ZERO_CC: u8 = 4;
    const NZ_CC: u8 = 5;
    const NS_CC: u8 = 9;
    const CMP_EQ: u8 = 0;
    const CMP_LT: u8 = 1;
    const CMP_LE: u8 = 2;
    const CMP_NE: u8 = 4;
    const CMP_ORD: u8 = 7;

    pub(crate) fn footprint(
        form: IeeeFloatArithmeticForm,
        operands: &[RegisterViewId],
    ) -> X86_64SelectedFormFootprint {
        let inputs = form.operand_count();
        X86_64SelectedFormFootprint {
            register_reads: operands[..inputs].to_vec(),
            register_writes: operands[inputs..inputs + 1].to_vec(),
            writes_rflags: conversion_control(form),
            encoded: effects(form),
        }
    }

    #[derive(Clone, Copy)]
    enum Op {
        AdjustRsp(i8),
        Mxcsr {
            load: bool,
            disp: u8,
        },
        StoreMxcsr(u32),
        Xfer {
            to_xmm: bool,
            wide: bool,
            xmm: u8,
            gpr: u8,
        },
        Scalar {
            double: bool,
            opcode: u8,
            dst: u8,
            src: u8,
        },
        AllOnes(u8),
        ShlXmm {
            quad: bool,
            xmm: u8,
            count: u8,
        },
        ShrXmm {
            quad: bool,
            arith: bool,
            xmm: u8,
            count: u8,
        },
        XorXmm {
            double: bool,
            dst: u8,
            src: u8,
        },
        IntToFloat {
            double: bool,
            wide: bool,
            xmm: u8,
            gpr: u8,
        },
        Mov32 {
            dst: u8,
            src: u8,
        },
        MovA {
            dst: u8,
            src: u8,
        },
        CmpScalar {
            double: bool,
            pred: u8,
            dst: u8,
            src: u8,
        },
        AndXmm {
            dst: u8,
            src: u8,
        },
        MovImm64 {
            gpr: u8,
            val: u64,
        },
        FloatToInt {
            wide: bool,
            gpr: u8,
            xmm: u8,
        },
        Sext32 {
            dst: u8,
            src: u8,
        },
        OrXmm {
            dst: u8,
            src: u8,
        },
        ShiftGpr64 {
            left: bool,
            gpr: u8,
            count: u8,
        },
        CmpImm64 {
            gpr: u8,
            val: u32,
        },
        Test64(u8),
        Jcc(u8, i32),
        Jmp(i32),
        Crash,
        Fma {
            double: bool,
            dst: u8,
            mul: u8,
            add: u8,
        },
    }

    fn emit_op(bytes: &mut Vec<u8>, op: Op) {
        let mr = |reg: u8, rm: u8| 0xc0 | ((reg & 7) << 3) | (rm & 7);
        let opt_rex = |bytes: &mut Vec<u8>, r: u8| {
            if r != 0x40 {
                bytes.push(r);
            }
        };
        let sp = |double: bool| if double { 0xf2 } else { 0xf3 };
        match op {
            Op::AdjustRsp(d) => bytes.extend([0x48, 0x8d, 0x64, 0x24, d as u8]),
            Op::Mxcsr { load, disp } => {
                let ext = if load { 2 } else { 3 };
                bytes.extend([
                    0x0f,
                    0xae,
                    (ext << 3) | 4 | if disp == 0 { 0 } else { 0x40 },
                    0x24,
                ]);
                if disp != 0 {
                    bytes.push(disp);
                }
            }
            Op::StoreMxcsr(val) => {
                bytes.extend([0xc7, 0x44, 0x24, 4]);
                bytes.extend(val.to_le_bytes());
            }
            Op::Xfer {
                to_xmm,
                wide,
                xmm,
                gpr,
            } => {
                bytes.push(0x66);
                opt_rex(bytes, 0x40 | (u8::from(wide) << 3) | (gpr >> 3));
                bytes.extend([0x0f, if to_xmm { 0x6e } else { 0x7e }, mr(xmm, gpr)]);
            }
            Op::Scalar {
                double,
                opcode,
                dst,
                src,
            } => bytes.extend([sp(double), 0x0f, opcode, mr(dst, src)]),
            Op::AllOnes(xmm) => bytes.extend([0x66, 0x0f, 0x76, mr(xmm, xmm)]),
            Op::ShlXmm { quad, xmm, count } => bytes.extend([
                0x66,
                0x0f,
                if quad { 0x73 } else { 0x72 },
                mr(6, xmm),
                count,
            ]),
            Op::ShrXmm {
                quad,
                arith,
                xmm,
                count,
            } => bytes.extend([
                0x66,
                0x0f,
                if quad { 0x73 } else { 0x72 },
                mr(if arith { 4 } else { 2 }, xmm),
                count,
            ]),
            Op::XorXmm { double, dst, src } => {
                if double {
                    bytes.push(0x66);
                }
                bytes.extend([0x0f, 0x57, mr(dst, src)]);
            }
            Op::IntToFloat {
                double,
                wide,
                xmm,
                gpr,
            } => {
                bytes.push(sp(double));
                opt_rex(bytes, 0x40 | (u8::from(wide) << 3) | (gpr >> 3));
                bytes.extend([0x0f, 0x2a, mr(xmm, gpr)]);
            }
            Op::Mov32 { dst, src } => {
                opt_rex(bytes, 0x40 | ((src >> 3) << 2) | (dst >> 3));
                bytes.extend([0x89, mr(src, dst)]);
            }
            Op::MovA { dst, src } => bytes.extend([0x66, 0x0f, 0x28, mr(dst, src)]),
            Op::CmpScalar {
                double,
                pred,
                dst,
                src,
            } => bytes.extend([sp(double), 0x0f, 0xc2, mr(dst, src), pred]),
            Op::AndXmm { dst, src } => bytes.extend([0x66, 0x0f, 0x54, mr(dst, src)]),
            Op::MovImm64 { gpr, val } => {
                bytes.extend([0x48 | (gpr >> 3), 0xb8 | (gpr & 7)]);
                bytes.extend(val.to_le_bytes());
            }
            Op::FloatToInt { wide, gpr, xmm } => {
                bytes.push(0xf2);
                opt_rex(bytes, 0x40 | (u8::from(wide) << 3) | ((gpr >> 3) << 2));
                bytes.extend([0x0f, 0x2c, mr(gpr, xmm)]);
            }
            Op::Sext32 { dst, src } => {
                bytes.extend([0x48 | ((dst >> 3) << 2) | (src >> 3), 0x63, mr(dst, src)])
            }
            Op::OrXmm { dst, src } => bytes.extend([0x66, 0x0f, 0xeb, mr(dst, src)]),
            Op::ShiftGpr64 { left, gpr, count } => bytes.extend([
                0x48 | (gpr >> 3),
                0xc1,
                mr(if left { 4 } else { 5 }, gpr),
                count,
            ]),
            Op::CmpImm64 { gpr, val } => {
                bytes.extend([0x48 | (gpr >> 3), 0x81, mr(7, gpr)]);
                bytes.extend(val.to_le_bytes());
            }
            Op::Test64(gpr) => {
                bytes.extend([0x48 | ((gpr >> 3) << 2) | (gpr >> 3), 0x85, mr(gpr, gpr)])
            }
            Op::Jcc(cc, disp) => {
                bytes.extend([0x0f, 0x80 | cc]);
                bytes.extend(disp.to_le_bytes());
            }
            Op::Jmp(disp) => {
                bytes.push(0xe9);
                bytes.extend(disp.to_le_bytes());
            }
            Op::Crash => bytes.extend([0x0f, 0x0b]),
            Op::Fma {
                double,
                dst,
                mul,
                add,
            } => bytes.extend([
                0xc4,
                0xe2,
                (u8::from(double) << 7) | ((!mul & 0xf) << 3) | 1,
                0xa9,
                mr(dst, add),
            ]),
        }
    }

    fn encoded_len(ops: &[Op]) -> i32 {
        let mut buf = Vec::new();
        for &op in ops {
            emit_op(&mut buf, op);
        }
        buf.len() as i32
    }

    fn choose(out: &mut Vec<Op>, cc: u8, taken: Vec<Op>, not_taken: Vec<Op>) {
        out.push(Op::Jcc(cc, encoded_len(&not_taken) + 5));
        out.extend(not_taken);
        out.push(Op::Jmp(encoded_len(&taken)));
        out.extend(taken);
    }

    fn load_bound(gpr: u8, val: u64) -> [Op; 2] {
        [
            Op::MovImm64 { gpr, val },
            Op::Xfer {
                to_xmm: true,
                wide: true,
                xmm: RIGHT,
                gpr,
            },
        ]
    }

    fn observe_mask(gpr: u8) -> [Op; 2] {
        [
            Op::Xfer {
                to_xmm: false,
                wide: true,
                xmm: RIGHT,
                gpr,
            },
            Op::Test64(gpr),
        ]
    }

    fn cmp_bound(pred: u8) -> Op {
        Op::CmpScalar {
            double: true,
            pred,
            dst: RIGHT,
            src: LEFT,
        }
    }

    fn from_u64(double: bool, src: u8, res: u8) -> Vec<Op> {
        let mut p = vec![
            Op::Xfer {
                to_xmm: true,
                wide: true,
                xmm: LEFT,
                gpr: src,
            },
            Op::Test64(src),
        ];
        let low = vec![Op::IntToFloat {
            double,
            wide: true,
            xmm: LEFT,
            gpr: src,
        }];
        let high = vec![
            Op::MovA {
                dst: RIGHT,
                src: LEFT,
            },
            Op::ShlXmm {
                quad: true,
                xmm: RIGHT,
                count: 63,
            },
            Op::ShrXmm {
                quad: true,
                arith: false,
                xmm: RIGHT,
                count: 63,
            },
            Op::ShrXmm {
                quad: true,
                arith: false,
                xmm: LEFT,
                count: 1,
            },
            Op::OrXmm {
                dst: LEFT,
                src: RIGHT,
            },
            Op::Xfer {
                to_xmm: false,
                wide: true,
                xmm: LEFT,
                gpr: res,
            },
            Op::IntToFloat {
                double,
                wide: true,
                xmm: LEFT,
                gpr: res,
            },
            Op::Scalar {
                double,
                opcode: 0x58,
                dst: LEFT,
                src: LEFT,
            },
        ];
        choose(&mut p, NS_CC, low, high);
        p.push(Op::Xfer {
            to_xmm: false,
            wide: double,
            xmm: LEFT,
            gpr: res,
        });
        p
    }

    fn to_u64(res: u8) -> Vec<Op> {
        let mut p = vec![
            Op::MovA {
                dst: RIGHT,
                src: LEFT,
            },
            Op::CmpScalar {
                double: true,
                pred: CMP_ORD,
                dst: RIGHT,
                src: LEFT,
            },
            Op::AndXmm {
                dst: LEFT,
                src: RIGHT,
            },
            Op::XorXmm {
                double: true,
                dst: RIGHT,
                src: RIGHT,
            },
            Op::Scalar {
                double: true,
                opcode: 0x5f,
                dst: LEFT,
                src: RIGHT,
            },
        ];
        p.extend(load_bound(res, TWO_TO_64));
        p.push(cmp_bound(CMP_LE));
        p.extend(observe_mask(res));
        let mut in_range = Vec::from(load_bound(res, TWO_TO_63));
        in_range.push(cmp_bound(CMP_LE));
        in_range.extend(observe_mask(res));
        let low = vec![Op::FloatToInt {
            wide: true,
            gpr: res,
            xmm: LEFT,
        }];
        let mut high = Vec::from(load_bound(res, TWO_TO_63));
        high.extend([
            Op::Scalar {
                double: true,
                opcode: 0x5c,
                dst: LEFT,
                src: RIGHT,
            },
            Op::FloatToInt {
                wide: true,
                gpr: res,
                xmm: LEFT,
            },
            Op::Xfer {
                to_xmm: true,
                wide: true,
                xmm: LEFT,
                gpr: res,
            },
            Op::AllOnes(RIGHT),
            Op::ShlXmm {
                quad: true,
                xmm: RIGHT,
                count: 63,
            },
            Op::OrXmm {
                dst: LEFT,
                src: RIGHT,
            },
            Op::Xfer {
                to_xmm: false,
                wide: true,
                xmm: LEFT,
                gpr: res,
            },
        ]);
        choose(&mut in_range, NZ_CC, high, low);
        choose(
            &mut p,
            NZ_CC,
            vec![Op::MovImm64 {
                gpr: res,
                val: u64::MAX,
            }],
            in_range,
        );
        p
    }

    fn trapping_trunc(double: bool, target: IntCarrier, src: u8, res: u8) -> Vec<Op> {
        let (min, pred, max) = match target {
            IntCarrier::I8 => (0xc060_2000_0000_0000, CMP_LT, 0x4060_0000_0000_0000),
            IntCarrier::I16 => (0xc0e0_0020_0000_0000, CMP_LT, 0x40e0_0000_0000_0000),
            IntCarrier::I32 => (0xc1e0_0000_0020_0000, CMP_LT, TWO_TO_31),
            IntCarrier::I64 => (0xc3e0_0000_0000_0000, CMP_LE, TWO_TO_63),
            IntCarrier::U8 => (0xbff0_0000_0000_0000, CMP_LT, 0x4070_0000_0000_0000),
            IntCarrier::U16 => (0xbff0_0000_0000_0000, CMP_LT, 0x40f0_0000_0000_0000),
            IntCarrier::U32 => (0xbff0_0000_0000_0000, CMP_LT, 0x41f0_0000_0000_0000),
            IntCarrier::U64 => (0xbff0_0000_0000_0000, CMP_LT, TWO_TO_64),
        };
        let mut p = vec![Op::Xfer {
            to_xmm: true,
            wide: double,
            xmm: LEFT,
            gpr: src,
        }];
        if !double {
            p.push(Op::Scalar {
                double: false,
                opcode: 0x5a,
                dst: LEFT,
                src: LEFT,
            });
        }
        p.extend(load_bound(res, min));
        p.push(cmp_bound(pred));
        p.extend(observe_mask(res));
        p.extend([Op::Jcc(NZ_CC, 2), Op::Crash]);
        p.extend(load_bound(res, max));
        p.push(cmp_bound(CMP_LE));
        p.extend(observe_mask(res));
        p.extend([Op::Jcc(ZERO_CC, 2), Op::Crash]);
        p.extend(trunc_landed(target, res));
        p
    }

    fn trunc_landed(target: IntCarrier, res: u8) -> Vec<Op> {
        if target == IntCarrier::U64 {
            return to_u64(res);
        }
        let mut p = vec![
            Op::MovA {
                dst: RIGHT,
                src: LEFT,
            },
            Op::CmpScalar {
                double: true,
                pred: CMP_ORD,
                dst: RIGHT,
                src: LEFT,
            },
            Op::AndXmm {
                dst: LEFT,
                src: RIGHT,
            },
        ];
        if !target.is_signed() || target.bits() < 32 {
            let (min, max) = match target {
                IntCarrier::I8 => (Some(0xc060_0000_0000_0000), 0x405f_c000_0000_0000),
                IntCarrier::I16 => (Some(0xc0e0_0000_0000_0000), 0x40df_ffc0_0000_0000),
                IntCarrier::U8 => (None, 0x406f_e000_0000_0000),
                IntCarrier::U16 => (None, 0x40ef_ffe0_0000_0000),
                _ => (None, U32_MAX_F64),
            };
            match min {
                None => p.push(Op::XorXmm {
                    double: true,
                    dst: RIGHT,
                    src: RIGHT,
                }),
                Some(v) => p.extend(load_bound(res, v)),
            }
            p.push(Op::Scalar {
                double: true,
                opcode: 0x5f,
                dst: LEFT,
                src: RIGHT,
            });
            p.extend(load_bound(res, max));
            p.extend([
                Op::Scalar {
                    double: true,
                    opcode: 0x5d,
                    dst: LEFT,
                    src: RIGHT,
                },
                Op::FloatToInt {
                    wide: target.bits() == 32,
                    gpr: res,
                    xmm: LEFT,
                },
            ]);
            if target.is_signed() {
                p.push(Op::Sext32 { dst: res, src: res });
            }
            return p;
        }
        let wide = target.bits() == 64;
        p.extend(load_bound(res, if wide { TWO_TO_63 } else { TWO_TO_31 }));
        p.extend([
            cmp_bound(CMP_LE),
            Op::FloatToInt {
                wide,
                gpr: res,
                xmm: LEFT,
            },
            Op::Xfer {
                to_xmm: true,
                wide,
                xmm: LEFT,
                gpr: res,
            },
            Op::XorXmm {
                double: true,
                dst: LEFT,
                src: RIGHT,
            },
            Op::Xfer {
                to_xmm: false,
                wide,
                xmm: LEFT,
                gpr: res,
            },
        ]);
        if !wide {
            p.push(Op::Sext32 { dst: res, src: res });
        }
        p
    }

    fn classify_program(form: IeeeFloatArithmeticForm, regs: &[u8]) -> Vec<Op> {
        let double = !form.is_binary32();
        let res = regs[1];
        let restore = Op::Xfer {
            to_xmm: false,
            wide: true,
            xmm: LEFT,
            gpr: res,
        };
        let out = |class: u64| Op::MovImm64 {
            gpr: res,
            val: match form.operation {
                FloatOp::Classify => class,
                FloatOp::IsNan => u64::from(class == 0),
                FloatOp::IsInfinite => u64::from(class == 1),
                FloatOp::IsFinite => u64::from(class >= 2),
                FloatOp::IsNormal => u64::from(class == 2),
                FloatOp::IsSubnormal => u64::from(class == 3),
                _ => unreachable!(),
            },
        };
        let mut p = vec![
            Op::Xfer {
                to_xmm: true,
                wide: double,
                xmm: LEFT,
                gpr: regs[0],
            },
            restore,
            Op::ShiftGpr64 {
                left: true,
                gpr: res,
                count: if double { 1 } else { 33 },
            },
            Op::ShiftGpr64 {
                left: false,
                gpr: res,
                count: if double { 53 } else { 56 },
            },
            Op::CmpImm64 { gpr: res, val: 0 },
            Op::Jcc(ZERO_CC, 0),
            Op::CmpImm64 {
                gpr: res,
                val: if double { 2047 } else { 255 },
            },
            Op::Jcc(ZERO_CC, 0),
            out(2),
            Op::Jmp(0),
        ];
        let subnormal = p.len();
        p.extend([
            restore,
            Op::ShiftGpr64 {
                left: true,
                gpr: res,
                count: if double { 1 } else { 33 },
            },
            Op::Test64(res),
            Op::Jcc(ZERO_CC, 0),
            out(3),
            Op::Jmp(0),
        ]);
        let zero = p.len();
        p.extend([out(4), Op::Jmp(0)]);
        let nonfinite = p.len();
        p.extend([
            restore,
            Op::ShiftGpr64 {
                left: true,
                gpr: res,
                count: if double { 12 } else { 41 },
            },
            Op::Test64(res),
            Op::Jcc(ZERO_CC, 0),
            out(0),
            Op::Jmp(0),
        ]);
        let inf = p.len();
        p.push(out(1));
        let end = p.len();
        if form.operation == FloatOp::Classify {
            p.extend([
                Op::CmpImm64 { gpr: res, val: 0 },
                Op::Jcc(ZERO_CC, 0),
                Op::Xfer {
                    to_xmm: true,
                    wide: true,
                    xmm: RIGHT,
                    gpr: res,
                },
                Op::ShrXmm {
                    quad: double,
                    arith: false,
                    xmm: LEFT,
                    count: if double { 63 } else { 31 },
                },
                Op::ShlXmm {
                    quad: true,
                    xmm: LEFT,
                    count: 32,
                },
                Op::OrXmm {
                    dst: LEFT,
                    src: RIGHT,
                },
                restore,
            ]);
        }
        let mut branches = vec![
            (5, subnormal),
            (7, nonfinite),
            (9, end),
            (subnormal + 3, zero),
            (subnormal + 5, end),
            (zero + 1, end),
            (nonfinite + 3, inf),
            (nonfinite + 5, end),
        ];
        if form.operation == FloatOp::Classify {
            branches.push((end + 1, p.len()));
        }
        let mut offsets = vec![0usize];
        let mut scratch = Vec::new();
        for &op in &p {
            scratch.clear();
            emit_op(&mut scratch, op);
            offsets.push(offsets.last().unwrap() + scratch.len());
        }
        for (br, dst) in branches {
            let disp = (offsets[dst] as i64 - offsets[br + 1] as i64) as i32;
            match &mut p[br] {
                Op::Jmp(d) | Op::Jcc(_, d) => *d = disp,
                _ => unreachable!(),
            }
        }
        p
    }

    fn mxcsr_envelope(rounding: IeeeFloatRoundingDirection, arith: Op) -> [Op; 7] {
        let rc = match rounding {
            IeeeFloatRoundingDirection::TowardZero => 3u32,
            IeeeFloatRoundingDirection::TowardPositive => 2,
            IeeeFloatRoundingDirection::TowardNegative => 1,
        };
        [
            Op::AdjustRsp(-16),
            Op::Mxcsr {
                load: false,
                disp: 0,
            },
            Op::StoreMxcsr(crate::OMEGA_CANONICAL_MXCSR | (rc << 13)),
            Op::Mxcsr {
                load: true,
                disp: 4,
            },
            arith,
            Op::Mxcsr {
                load: true,
                disp: 0,
            },
            Op::AdjustRsp(16),
        ]
    }

    fn program(form: IeeeFloatArithmeticForm, regs: &[u8]) -> Result<Vec<Op>, Error> {
        let double = !form.is_binary32();
        let res = regs[form.operand_count()];
        let mv_in = |gpr, xmm, wide| Op::Xfer {
            to_xmm: true,
            wide,
            xmm,
            gpr,
        };
        let mv_out = Op::Xfer {
            to_xmm: false,
            wide: double,
            xmm: LEFT,
            gpr: res,
        };
        let in_place = |d, opcode| Op::Scalar {
            double: d,
            opcode,
            dst: LEFT,
            src: LEFT,
        };
        Ok(match form.operation {
            FloatOp::DirectedBinary {
                operation,
                rounding,
            } => {
                let opcode = match operation {
                    IeeeFloatBinaryOperation::Add => 0x58,
                    IeeeFloatBinaryOperation::Subtract => 0x5c,
                    IeeeFloatBinaryOperation::Multiply => 0x59,
                    IeeeFloatBinaryOperation::Divide => 0x5e,
                };
                let mut p = vec![mv_in(regs[0], LEFT, double), mv_in(regs[1], RIGHT, double)];
                p.extend(mxcsr_envelope(
                    rounding,
                    Op::Scalar {
                        double,
                        opcode,
                        dst: LEFT,
                        src: RIGHT,
                    },
                ));
                p.push(mv_out);
                p
            }
            FloatOp::DirectedSquareRoot { rounding } => {
                let mut p = vec![mv_in(regs[0], LEFT, double)];
                p.extend(mxcsr_envelope(rounding, in_place(double, 0x51)));
                p.push(mv_out);
                p
            }
            FloatOp::FusedMultiplyAdd | FloatOp::DirectedFusedMultiplyAdd { .. } => {
                let mut p = vec![
                    mv_in(regs[0], LEFT, double),
                    mv_in(regs[1], RIGHT, double),
                    mv_in(regs[2], ADDEND, double),
                ];
                let fma = Op::Fma {
                    double,
                    dst: LEFT,
                    mul: RIGHT,
                    add: ADDEND,
                };
                if let Some(r) = form.operation.rounding_direction() {
                    p.extend(mxcsr_envelope(r, fma));
                } else {
                    p.push(fma);
                }
                p.push(mv_out);
                p
            }
            FloatOp::IsNan
            | FloatOp::IsInfinite
            | FloatOp::IsFinite
            | FloatOp::IsNormal
            | FloatOp::IsSubnormal
            | FloatOp::Classify => classify_program(form, regs),
            FloatOp::Compare(cmp) => {
                use semantic_vocabulary::IeeeFloatComparisonOperation as Rel;
                let (l, r, pred) = match cmp {
                    Rel::Equal => (regs[0], regs[1], CMP_EQ),
                    Rel::NotEqual => (regs[0], regs[1], CMP_NE),
                    Rel::Less => (regs[0], regs[1], CMP_LT),
                    Rel::LessOrEqual => (regs[0], regs[1], CMP_LE),
                    Rel::Greater => (regs[1], regs[0], CMP_LT),
                    Rel::GreaterOrEqual => (regs[1], regs[0], CMP_LE),
                };
                vec![
                    mv_in(l, LEFT, double),
                    mv_in(r, RIGHT, double),
                    Op::CmpScalar {
                        double,
                        pred,
                        dst: LEFT,
                        src: RIGHT,
                    },
                    Op::ShrXmm {
                        quad: double,
                        arith: false,
                        xmm: LEFT,
                        count: if double { 63 } else { 31 },
                    },
                    mv_out,
                ]
            }
            FloatOp::MultiplyThenAdd => vec![
                mv_in(regs[0], LEFT, double),
                mv_in(regs[1], RIGHT, double),
                Op::Scalar {
                    double,
                    opcode: 0x59,
                    dst: LEFT,
                    src: RIGHT,
                },
                mv_in(regs[2], ADDEND, double),
                Op::Scalar {
                    double,
                    opcode: 0x58,
                    dst: LEFT,
                    src: ADDEND,
                },
                mv_out,
            ],
            FloatOp::Negate => vec![
                mv_in(regs[0], LEFT, double),
                Op::AllOnes(RIGHT),
                Op::ShlXmm {
                    quad: double,
                    xmm: RIGHT,
                    count: if double { 63 } else { 31 },
                },
                Op::XorXmm {
                    double,
                    dst: LEFT,
                    src: RIGHT,
                },
                mv_out,
            ],
            FloatOp::SquareRoot => {
                vec![mv_in(regs[0], LEFT, double), in_place(double, 0x51), mv_out]
            }
            FloatOp::ConvertFormat => vec![
                mv_in(regs[0], LEFT, !double),
                in_place(!double, 0x5a),
                mv_out,
            ],
            FloatOp::TruncateToInteger {
                target,
                domain: IeeeFloatToIntegerDomain::Trapping,
            } => trapping_trunc(double, target, regs[0], res),
            FloatOp::TruncateToInteger { target, .. } => {
                let mut p = vec![mv_in(regs[0], LEFT, double)];
                if !double {
                    p.push(in_place(false, 0x5a));
                }
                p.extend(trunc_landed(target, res));
                p
            }
            FloatOp::FromInteger(IntCarrier::U64) => from_u64(double, regs[0], res),
            FloatOp::FromInteger(src) if src.bits() < 32 => {
                let shift = (32 - src.bits()) as u8;
                let mut p = vec![
                    mv_in(regs[0], LEFT, false),
                    Op::ShlXmm {
                        quad: false,
                        xmm: LEFT,
                        count: shift,
                    },
                    Op::ShrXmm {
                        quad: false,
                        arith: src.is_signed(),
                        xmm: LEFT,
                        count: shift,
                    },
                    in_place(false, 0xe6),
                ];
                if !double {
                    p.push(in_place(true, 0x5a));
                }
                p.push(mv_out);
                p
            }
            FloatOp::FromInteger(src) if !src.is_signed() => vec![
                Op::Mov32 {
                    dst: res,
                    src: regs[0],
                },
                Op::IntToFloat {
                    double,
                    wide: true,
                    xmm: LEFT,
                    gpr: res,
                },
                mv_out,
            ],
            FloatOp::FromInteger(src) => vec![
                Op::IntToFloat {
                    double,
                    wide: src.bits() == 64,
                    xmm: LEFT,
                    gpr: regs[0],
                },
                mv_out,
            ],
            bin => {
                let opcode = match bin {
                    FloatOp::Add => 0x58,
                    FloatOp::Multiply => 0x59,
                    FloatOp::Subtract => 0x5c,
                    FloatOp::Minimum => 0x5d,
                    FloatOp::Divide => 0x5e,
                    FloatOp::Maximum => 0x5f,
                    _ => return Err(Error::EncodedFormMismatch),
                };
                vec![
                    mv_in(regs[0], LEFT, double),
                    mv_in(regs[1], RIGHT, double),
                    Op::Scalar {
                        double,
                        opcode,
                        dst: LEFT,
                        src: RIGHT,
                    },
                    mv_out,
                ]
            }
        })
    }

    fn canonical(form: IeeeFloatArithmeticForm, regs: &[u8]) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::with_capacity(24);
        for op in program(form, regs)? {
            emit_op(&mut bytes, op);
        }
        Ok(bytes)
    }

    fn regs(
        physical: &ValidatedPhysicalRegisterModel,
        form: IeeeFloatArithmeticForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, Error> {
        guard_canonical(physical)?;
        if alternative
            != (MachineAlternativeKey {
                family: Family::NearestIeeeFloatArithmetic(form),
                variant: 0,
            })
        {
            return Err(Error::AlternativeMismatch);
        }
        if operands.len() != form.operand_count() + 1 {
            return Err(Error::OperandCountMismatch);
        }
        resolve_registers(physical, operands)
    }

    pub(crate) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        form: IeeeFloatArithmeticForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let r = regs(physical, form, alternative, operands)?;
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: canonical(form, &r)?,
            footprint: footprint(form, operands),
        })
    }

    pub(crate) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        form: IeeeFloatArithmeticForm,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let r = regs(physical, form, alternative, operands)?;
        if bytes != canonical(form, &r)? {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes: bytes.to_vec(),
            footprint: footprint(form, operands),
        })
    }
}
