/*
We encode and validate all AArch64 selected instruction forms
(`encode_aarch64_selected_form`, `validate_aarch64_selected_form_encoding`),
conditional and unconditional branches, fused compare-and-branch forms, memory
and frame forms, scalar/dynamic/table/system/normalized-foreign call templates,
atomic operations, IEEE floating-point arithmetic, and hosted I/O leaves using
declarative AArch64 word constructors and canonical byte validation.
*/

mod enc {
    use super::Aarch64SelectedFormEncodingError as Error;

    pub(super) const BRK_0: u32 = 0xd420_0000;
    pub(super) const DMB_ISH: u32 = 0xd503_3fbf;
    pub(super) const CLREX: u32 = 0xd503_3f5f;
    pub(super) const BL_ZERO: u32 = 0x9400_0000;
    pub(super) const SVC_0: u32 = 0xd400_0001;
    pub(super) const SVC_80: u32 = 0xd400_1001;
    pub(super) const ZR: u8 = 31;

    pub(super) const EQ: u8 = 0x0;
    pub(super) const NE: u8 = 0x1;
    pub(super) const HS: u8 = 0x2;
    pub(super) const LO: u8 = 0x3;
    pub(super) const VS: u8 = 0x6;
    pub(super) const VC: u8 = 0x7;
    pub(super) const HI: u8 = 0x8;
    pub(super) const LS: u8 = 0x9;
    pub(super) const GE: u8 = 0xa;
    pub(super) const LT: u8 = 0xb;
    pub(super) const LE: u8 = 0xd;

    pub(super) const fn rrr(op: u32, rd: u8, rn: u8, rm: u8) -> u32 {
        op | ((rm as u32) << 16) | ((rn as u32) << 5) | rd as u32
    }

    pub(super) const fn rr(op: u32, rd: u8, rn: u8) -> u32 {
        op | ((rn as u32) << 5) | rd as u32
    }

    pub(super) const fn rri12(op: u32, rd: u8, rn: u8, imm: u16) -> u32 {
        op | ((imm as u32) << 10) | ((rn as u32) << 5) | rd as u32
    }

    pub(super) const fn mov(rd: u8, rm: u8) -> u32 {
        0xaa00_03e0 | ((rm as u32) << 16) | rd as u32
    }

    pub(super) const fn movz(rd: u8, imm: u16, shift: u8) -> u32 {
        0xd280_0000 | ((shift as u32) << 21) | ((imm as u32) << 5) | rd as u32
    }

    pub(super) const fn movk(rd: u8, imm: u16, shift: u8) -> u32 {
        0xf280_0000 | ((shift as u32) << 21) | ((imm as u32) << 5) | rd as u32
    }

    pub(super) const fn cmp_reg(rn: u8, rm: u8) -> u32 {
        0xeb00_001f | ((rm as u32) << 16) | ((rn as u32) << 5)
    }

    pub(super) const fn cmp_imm(wide: bool, rn: u8, imm: u16) -> u32 {
        (if wide { 0xf100_001f } else { 0x7100_001f }) | ((imm as u32) << 10) | ((rn as u32) << 5)
    }

    pub(super) const fn msub(rd: u8, rn: u8, rm: u8, ra: u8) -> u32 {
        0x9b00_8000 | ((rm as u32) << 16) | ((ra as u32) << 10) | ((rn as u32) << 5) | rd as u32
    }

    pub(super) const fn csel(rd: u8, rn: u8, rm: u8, cond: u8) -> u32 {
        0x9a80_0000 | ((rm as u32) << 16) | ((cond as u32) << 12) | ((rn as u32) << 5) | rd as u32
    }

    pub(super) const fn b(words: i32) -> u32 {
        0x1400_0000 | ((words as u32) & 0x03ff_ffff)
    }

    pub(super) const fn b_cond(cond: u8, words: i32) -> u32 {
        0x5400_0000 | (((words as u32) & 0x7ffff) << 5) | cond as u32
    }

    pub(super) const fn cbz(rt: u8, words: i32) -> u32 {
        0xb400_0000 | (((words as u32) & 0x7ffff) << 5) | rt as u32
    }

    pub(super) const fn cbnz(rt: u8, words: i32) -> u32 {
        0xb500_0000 | (((words as u32) & 0x7ffff) << 5) | rt as u32
    }

    pub(super) const fn ubfm(wide: bool, rd: u8, rn: u8, immr: u8, imms: u8) -> u32 {
        (if wide { 0xd340_0000 } else { 0x5300_0000 })
            | ((immr as u32) << 16)
            | ((imms as u32) << 10)
            | ((rn as u32) << 5)
            | rd as u32
    }

    pub(super) const fn sbfm(wide: bool, rd: u8, rn: u8, immr: u8, imms: u8) -> u32 {
        (if wide { 0x9340_0000 } else { 0x1300_0000 })
            | ((immr as u32) << 16)
            | ((imms as u32) << 10)
            | ((rn as u32) << 5)
            | rd as u32
    }

    pub(super) const fn ldst_imm(op: u32, rt: u8, rn: u8, scaled_imm: u32) -> u32 {
        op | (scaled_imm << 10) | ((rn as u32) << 5) | rt as u32
    }

    pub(super) const fn fmov_to_v(double: bool, vd: u8, rn: u8) -> u32 {
        (if double { 0x9e67_0000 } else { 0x1e27_0000 }) | ((rn as u32) << 5) | vd as u32
    }

    pub(super) const fn fmov_to_g(double: bool, rd: u8, vn: u8) -> u32 {
        (if double { 0x9e66_0000 } else { 0x1e26_0000 }) | ((vn as u32) << 5) | rd as u32
    }

    pub(super) fn to_bytes(words: &[u32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(words.len() * 4);
        for &word in words {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out
    }

    pub(super) fn check_bytes(actual: &[u8], expected: &[u8]) -> Result<(), Error> {
        if actual.is_empty() || !actual.len().is_multiple_of(4) || actual.len() != expected.len() {
            return Err(Error::MalformedEncoding);
        }
        if actual != expected {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(())
    }
}

mod encoding_types {
    use register_model::RegisterViewId;
    use selected_instructions::MachineEncodedEffects;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Aarch64SelectedFormFootprint {
        pub register_reads: Vec<RegisterViewId>,
        pub register_writes: Vec<RegisterViewId>,
        pub writes_nzcv: bool,
        pub encoded: MachineEncodedEffects,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedAarch64SelectedFormEncoding {
        pub(crate) bytes: Vec<u8>,
        pub(crate) footprint: Aarch64SelectedFormFootprint,
    }

    impl ValidatedAarch64SelectedFormEncoding {
        pub fn bytes(&self) -> &[u8] {
            &self.bytes
        }

        pub const fn footprint(&self) -> &Aarch64SelectedFormFootprint {
            &self.footprint
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Aarch64SelectedFormEncodingError {
        NonCanonicalPhysicalModel,
        LayoutDependentForm,
        AlternativeMismatch,
        OperandCountMismatch,
        UnknownOrNonGpr64View(RegisterViewId),
        IntegerOutsideI64Bits,
        ImmediateOutsideU12,
        BranchDisplacementMisaligned,
        BranchDisplacementOutsideImm19,
        BranchDisplacementOutsideImm26,
        MovnMaterializationDoesNotShrink,
        MalformedEncoding,
        EncodedFormMismatch,
    }

    impl std::fmt::Display for Aarch64SelectedFormEncodingError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "invalid AArch64 selected-form encoding: {self:?}"
            )
        }
    }

    impl std::error::Error for Aarch64SelectedFormEncodingError {}
}

pub(crate) mod movn_materialization {
    use super::enc;

    pub(crate) fn append_canonical_materialization(words: &mut Vec<u32>, register: u8, value: u64) {
        words.push(enc::movz(register, value as u16, 0));
        for shift in 1..4_u8 {
            let chunk = ((value >> (u32::from(shift) * 16)) & 0xffff) as u16;
            if chunk != 0 {
                words.push(enc::movk(register, chunk, shift));
            }
        }
    }
}

mod plan_words {
    use super::{Aarch64SelectedFormEncodingError as Error, enc};

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum Step<Label> {
        Zero(u8),
        One(u8),
        Cbz(u8, Label),
        Tbnz7(u8, Label),
        LdrbReg(u8, u8, u8),
        AddImm(u8, u8, u16),
        CmpReg(u8, u8),
        CmpImm(u8, u16),
        BCond(u8, Label),
        B(Label),
    }

    pub(super) fn assemble<Label: Copy + Eq>(
        plan: &[(Option<Label>, Step<Label>)],
    ) -> Result<Vec<u8>, Error> {
        let labels: Vec<(Label, usize)> = plan
            .iter()
            .enumerate()
            .filter_map(|(i, (lbl, _))| lbl.map(|l| (l, i)))
            .collect();
        let rel = |pos: usize, label: Label, bits: u32| -> Result<i32, Error> {
            let target = labels
                .iter()
                .find(|(l, _)| *l == label)
                .map(|(_, i)| *i)
                .expect("plan defines every branch label");
            let disp = target as i64 - pos as i64;
            let limit = 1_i64 << (bits - 1);
            if !(-limit..limit).contains(&disp) {
                return Err(if bits == 26 {
                    Error::BranchDisplacementOutsideImm26
                } else {
                    Error::BranchDisplacementOutsideImm19
                });
            }
            Ok(disp as i32)
        };
        let mut bytes = Vec::with_capacity(plan.len() * 4);
        for (pos, &(_, step)) in plan.iter().enumerate() {
            let word = match step {
                Step::Zero(rd) => enc::movz(rd, 0, 0),
                Step::One(rd) => enc::movz(rd, 1, 0),
                Step::Cbz(rt, lbl) => enc::cbz(rt, rel(pos, lbl, 19)?),
                Step::Tbnz7(rt, lbl) => {
                    0x3738_0000 | (((rel(pos, lbl, 14)? as u32) & 0x3fff) << 5) | u32::from(rt)
                }
                Step::LdrbReg(rd, rn, rm) => enc::rrr(0x3860_6800, rd, rn, rm),
                Step::AddImm(rd, rn, imm) => enc::rri12(0x9100_0000, rd, rn, imm),
                Step::CmpReg(rn, rm) => enc::cmp_reg(rn, rm),
                Step::CmpImm(rn, imm) => enc::cmp_imm(true, rn, imm),
                Step::BCond(cond, lbl) => enc::b_cond(cond, rel(pos, lbl, 19)?),
                Step::B(lbl) => enc::b(rel(pos, lbl, 26)?),
            };
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        Ok(bytes)
    }
}

mod byte_sequences {
    use super::plan_words::{self, Step};
    use super::selected_forms::request_registers;
    use super::{
        Aarch64SelectedFormEncodingError as Error, ValidatedAarch64SelectedFormEncoding as Encoded,
        enc, nzcv_effects_encoding,
    };
    use encoded_scalar_plan::{Step as CodecStep, plan as codec_plan};
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        ByteSequencePredicate, MachineAlternativeFamily as Fam, MachineAlternativeKey,
        MachineEncodedMemoryEffect as MemEff, SelectedInstructionKind as Kind,
    };

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum CmpLabel {
        Loop,
        Equal,
        Done,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ScanLabel {
        Loop,
        Done,
        Fail,
        Advance1,
        Cont1,
        Cont2Any,
        Lead3,
        E0,
        Ed,
        Cont3Any,
        F0,
        F4,
    }

    fn scan_layout(
        predicate: ByteSequencePredicate,
        regs: [u8; 5],
    ) -> Vec<(Option<ScanLabel>, Step<ScanLabel>)> {
        use ScanLabel as L;
        let [base, count, cursor, byte, result] = regs;
        let cmp_br = |lbl, imm, cond, target| {
            [
                (lbl, Step::CmpImm(byte, imm)),
                (None, Step::BCond(cond, target)),
            ]
        };
        let mut out = vec![
            (None, Step::Zero(cursor)),
            (None, Step::Zero(byte)),
            (None, Step::Zero(result)),
        ];
        if predicate == ByteSequencePredicate::NonEmpty {
            out.extend([
                (None, Step::Cbz(count, L::Done)),
                (None, Step::One(result)),
                (Some(L::Done), Step::Zero(cursor)),
            ]);
            return out;
        }
        out.push((None, Step::One(result)));
        if matches!(
            predicate,
            ByteSequencePredicate::NoNul | ByteSequencePredicate::AsciiOnly
        ) {
            out.extend([
                (None, Step::Cbz(count, L::Done)),
                (Some(L::Loop), Step::LdrbReg(byte, base, cursor)),
                (
                    None,
                    if predicate == ByteSequencePredicate::NoNul {
                        Step::Cbz(byte, L::Fail)
                    } else {
                        Step::Tbnz7(byte, L::Fail)
                    },
                ),
                (None, Step::AddImm(cursor, cursor, 1)),
                (None, Step::CmpReg(cursor, count)),
                (None, Step::BCond(enc::LO, L::Loop)),
                (None, Step::B(L::Done)),
                (Some(L::Fail), Step::Zero(result)),
                (Some(L::Done), Step::Zero(cursor)),
            ]);
            return out;
        }
        out.extend([
            (None, Step::Cbz(count, L::Done)),
            (Some(L::Loop), Step::CmpReg(cursor, count)),
            (None, Step::BCond(enc::HS, L::Done)),
            (None, Step::LdrbReg(byte, base, cursor)),
        ]);
        for (lbl, imm, cond, target) in [
            (None, 0x80, enc::LO, L::Advance1),
            (None, 0xC2, enc::LO, L::Fail),
            (None, 0xE0, enc::LO, L::Cont1),
            (None, 0xF0, enc::HS, L::Lead3),
            (None, 0xE0, enc::EQ, L::E0),
            (None, 0xED, enc::EQ, L::Ed),
        ] {
            out.extend(cmp_br(lbl, imm, cond, target));
        }
        out.push((None, Step::B(L::Cont2Any)));
        out.extend(cmp_br(Some(L::Lead3), 0xF5, enc::HS, L::Fail));
        out.extend(cmp_br(None, 0xF0, enc::EQ, L::F0));
        out.extend(cmp_br(None, 0xF4, enc::EQ, L::F4));
        out.push((None, Step::B(L::Cont3Any)));
        for (label, width, low, high) in [
            (L::Cont1, 1_u16, 0x80_u16, 0xBF_u16),
            (L::Cont2Any, 2, 0x80, 0xBF),
            (L::E0, 2, 0xA0, 0xBF),
            (L::Ed, 2, 0x80, 0x9F),
            (L::Cont3Any, 3, 0x80, 0xBF),
            (L::F0, 3, 0x90, 0xBF),
            (L::F4, 3, 0x80, 0x8F),
        ] {
            out.extend([
                (Some(label), Step::AddImm(byte, cursor, width)),
                (None, Step::CmpReg(byte, count)),
                (None, Step::BCond(enc::HS, L::Fail)),
            ]);
            for off in 1..=width {
                let (lo, hi) = if off == 1 { (low, high) } else { (0x80, 0xBF) };
                out.extend([
                    (None, Step::AddImm(byte, cursor, off)),
                    (None, Step::LdrbReg(byte, base, byte)),
                ]);
                out.extend(cmp_br(None, lo, enc::LO, L::Fail));
                out.extend(cmp_br(None, hi, enc::HI, L::Fail));
            }
            out.extend([
                (None, Step::AddImm(cursor, cursor, width + 1)),
                (None, Step::B(L::Loop)),
            ]);
        }
        out.extend([
            (Some(L::Advance1), Step::AddImm(cursor, cursor, 1)),
            (None, Step::B(L::Loop)),
            (Some(L::Fail), Step::Zero(result)),
            (Some(L::Done), Step::Zero(cursor)),
        ]);
        out
    }

    fn encode_codec_word(step: CodecStep, pos: usize) -> Result<u32, Error> {
        let rel = |target: usize, bits: u32| -> Result<u32, Error> {
            let d = target as i64 - pos as i64;
            let bound = 1_i64 << (bits - 1);
            if !(-bound..bound).contains(&d) {
                return Err(Error::MalformedEncoding);
            }
            Ok((d as u32) & ((1 << bits) - 1))
        };
        Ok(match step {
            CodecStep::Zero(rd) => enc::movz(rd, 0, 0),
            CodecStep::Copy(rd, rm) => enc::mov(rd, rm),
            CodecStep::ByteCount(rd, rn) => enc::ubfm(true, rd, rn, 0, 7),
            CodecStep::SubOne(r) => enc::rri12(0xd100_0000, r, r, 1),
            CodecStep::AddOne(r) => enc::rri12(0x9100_0000, r, r, 1),
            CodecStep::LoadByte(rd, rn, rm) => enc::rrr(0x3860_6800, rd, rn, rm),
            CodecStep::StoreByte(rt, rn, rm) => enc::rrr(0x3820_6800, rt, rn, rm),
            CodecStep::SignByte(r) => enc::sbfm(true, r, r, 0, 7),
            CodecStep::ShiftLeft8(r) => enc::ubfm(true, r, r, 56, 55),
            CodecStep::ShiftRight8(r) => enc::ubfm(true, r, r, 8, 63),
            CodecStep::Or(rd, rm) => enc::rrr(0xaa00_0000, rd, rd, rm),
            CodecStep::Compare(rn, rm) => enc::cmp_reg(rn, rm),
            CodecStep::TestSigned(rt, t) => 0x3640_0000 | (rel(t, 14)? << 5) | u32::from(rt),
            CodecStep::ZeroBranch(rt, t) => 0xb400_0000 | (rel(t, 19)? << 5) | u32::from(rt),
            CodecStep::LessBranch(t) => 0x5400_0003 | (rel(t, 19)? << 5),
            CodecStep::Branch(t) => 0x1400_0000 | rel(t, 26)?,
        })
    }

    pub(super) fn try_encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Option<Result<Encoded, Error>> {
        let (bytes, reads, mem) = match kind {
            Kind::CopyBytes => {
                let [src, dst, count, cur, byte] = match request_registers::<5>(
                    physical,
                    alternative,
                    Fam::CopyBytes,
                    operands,
                    3,
                ) {
                    Ok(r) => r,
                    Err(e) => return Some(Err(e)),
                };
                let words = [
                    enc::movz(cur, 0, 0),
                    enc::cbz(count, 6),
                    enc::rrr(0x3860_6800, byte, src, cur),
                    enc::rrr(0x3820_6800, byte, dst, cur),
                    enc::rri12(0x9100_0000, cur, cur, 1),
                    enc::cmp_reg(cur, count),
                    enc::b_cond(enc::NE, -4),
                ];
                (
                    enc::to_bytes(&words),
                    3,
                    MemEff::CopyBytesV1 {
                        source_pointer_operand: 0,
                        destination_pointer_operand: 1,
                        count_operand: 2,
                    },
                )
            }
            Kind::ZeroBytes => {
                let [dst, count, cur] = match request_registers::<3>(
                    physical,
                    alternative,
                    Fam::ZeroBytes,
                    operands,
                    2,
                ) {
                    Ok(r) => r,
                    Err(e) => return Some(Err(e)),
                };
                let words = [
                    enc::movz(cur, 0, 0),
                    enc::cbz(count, 5),
                    enc::rrr(0x3820_6800, enc::ZR, dst, cur),
                    enc::rri12(0x9100_0000, cur, cur, 1),
                    enc::cmp_reg(cur, count),
                    enc::b_cond(enc::NE, -3),
                ];
                (
                    enc::to_bytes(&words),
                    2,
                    MemEff::ZeroBytesV1 {
                        destination_pointer_operand: 0,
                        count_operand: 1,
                    },
                )
            }
            Kind::CompareByteSequences => {
                let [lb, lc, rb, rc, cur, lbyte, rbyte, res] = match request_registers::<8>(
                    physical,
                    alternative,
                    Fam::CompareByteSequences,
                    operands,
                    4,
                ) {
                    Ok(r) => r,
                    Err(e) => return Some(Err(e)),
                };
                let plan = [
                    (None, Step::Zero(cur)),
                    (None, Step::Zero(lbyte)),
                    (None, Step::Zero(rbyte)),
                    (None, Step::Zero(res)),
                    (None, Step::CmpReg(lc, rc)),
                    (None, Step::BCond(enc::NE, CmpLabel::Done)),
                    (None, Step::Cbz(lc, CmpLabel::Equal)),
                    (Some(CmpLabel::Loop), Step::LdrbReg(lbyte, lb, cur)),
                    (None, Step::LdrbReg(rbyte, rb, cur)),
                    (None, Step::CmpReg(lbyte, rbyte)),
                    (None, Step::BCond(enc::NE, CmpLabel::Done)),
                    (None, Step::AddImm(cur, cur, 1)),
                    (None, Step::CmpReg(cur, lc)),
                    (None, Step::BCond(enc::NE, CmpLabel::Loop)),
                    (Some(CmpLabel::Equal), Step::One(res)),
                    (Some(CmpLabel::Done), Step::Zero(cur)),
                ];
                let bytes = match plan_words::assemble(&plan) {
                    Ok(b) => b,
                    Err(e) => return Some(Err(e)),
                };
                (
                    bytes,
                    4,
                    MemEff::CompareByteSequencesV1 {
                        left_base_operand: 0,
                        left_count_operand: 1,
                        right_base_operand: 2,
                        right_count_operand: 3,
                    },
                )
            }
            Kind::ScanByteSequence { predicate } => {
                let regs = match request_registers::<5>(
                    physical,
                    alternative,
                    Fam::ScanByteSequence,
                    operands,
                    2,
                ) {
                    Ok(r) => r,
                    Err(e) => return Some(Err(e)),
                };
                let bytes = match plan_words::assemble(&scan_layout(predicate, regs)) {
                    Ok(b) => b,
                    Err(e) => return Some(Err(e)),
                };
                (
                    bytes,
                    2,
                    MemEff::ScanByteSequenceV1 {
                        base_pointer_operand: 0,
                        count_operand: 1,
                    },
                )
            }
            Kind::LoadEncodedScalar | Kind::StoreEncodedScalar => {
                let load = kind == Kind::LoadEncodedScalar;
                let (family, inputs) = if load {
                    (Fam::LoadEncodedScalar, 2)
                } else {
                    (Fam::StoreEncodedScalar, 3)
                };
                let regs =
                    match request_registers::<6>(physical, alternative, family, operands, inputs) {
                        Ok(r) => r,
                        Err(e) => return Some(Err(e)),
                    };
                let steps = codec_plan(load, regs, CodecStep::Compare(regs[2], regs[3]));
                let mut bytes = Vec::with_capacity(steps.len() * 4);
                for (pos, step) in steps.into_iter().enumerate() {
                    match encode_codec_word(step, pos) {
                        Ok(w) => bytes.extend_from_slice(&w.to_le_bytes()),
                        Err(e) => return Some(Err(e)),
                    }
                }
                let mem = if load {
                    MemEff::ReadEncodedScalarV1 {
                        pointer_operand: 0,
                        codec_operand: 1,
                    }
                } else {
                    MemEff::WriteEncodedScalarV1 {
                        pointer_operand: 0,
                        codec_operand: 1,
                    }
                };
                (bytes, inputs, mem)
            }
            _ => return None,
        };
        Some(nzcv_effects_encoding(
            physical, operands, &bytes, reads, mem,
        ))
    }
}

mod float_bits {
    use super::selected_forms::resolve_registers;
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded, enc,
    };
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
        float_bit_transfer_properties,
    };

    pub(super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        values: &[RegisterViewId],
    ) -> Result<Encoded, Error> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        let (family, to_bits, wide) =
            float_bit_transfer_properties(kind).ok_or(Error::AlternativeMismatch)?;
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Error::AlternativeMismatch);
        }
        let [source, destination] = values else {
            return Err(Error::OperandCountMismatch);
        };
        let (float_id, bits_id) = if to_bits {
            (*source, *destination)
        } else {
            (*destination, *source)
        };
        let float_view = physical
            .model()
            .view(float_id)
            .ok_or(Error::UnknownOrNonGpr64View(float_id))?;
        let float_reg = float_view
            .name
            .strip_prefix('d')
            .and_then(|s| s.parse::<u8>().ok())
            .filter(|n| *n <= 31)
            .filter(|_| float_view.bits == 64 && float_view.allocatable)
            .ok_or(Error::UnknownOrNonGpr64View(float_id))?;
        let bits_reg = resolve_registers(physical, &[bits_id])?[0];
        let word = if to_bits {
            enc::fmov_to_g(wide, bits_reg, float_reg)
        } else {
            enc::fmov_to_v(wide, float_reg, bits_reg)
        };
        Ok(Encoded {
            bytes: word.to_le_bytes().to_vec(),
            footprint: Footprint {
                register_reads: vec![values[0]],
                register_writes: vec![values[1]],
                writes_nzcv: false,
                encoded: MachineEncodedEffects::fallthrough_v1(vec![0], vec![1]),
            },
        })
    }
}

pub(crate) mod atomic {
    use super::{Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint, enc};
    use register_model::RegisterViewId;
    use selected_instructions::{AtomicForm, AtomicOperation as Op, AtomicWidth as W};
    pub(crate) use target_aarch64::form_effects::atomic::effects;

    pub(crate) fn footprint(
        form: AtomicForm,
        operands: &[RegisterViewId],
    ) -> Aarch64SelectedFormFootprint {
        Aarch64SelectedFormFootprint {
            register_reads: operands[..usize::from(form.operation.input_count())].to_vec(),
            register_writes: form
                .output_operands()
                .map(|o| operands[usize::from(o)])
                .collect(),
            writes_nzcv: matches!(
                form.operation,
                Op::CompareExchange | Op::CompareExchangeOnce
            ),
            encoded: effects(form),
        }
    }

    pub(crate) fn encode(form: AtomicForm, regs: &[u8]) -> Result<Vec<u8>, Error> {
        if regs.len() != usize::from(form.operand_count()) {
            return Err(Error::OperandCountMismatch);
        }
        let expected_fixed: &[u8] = if form.operation == Op::CompareExchange {
            &[9, 10, 11, 12]
        } else {
            &[9, 10, 11]
        };
        if (form
            .operation
            .retries_until_commit(target::Architecture::Aarch64)
            && regs != expected_fixed)
            || regs.iter().any(|r| *r > 30)
            || form
                .output_operands()
                .any(|o| regs[..usize::from(o)].contains(&regs[usize::from(o)]))
        {
            return Err(Error::EncodedFormMismatch);
        }
        let size = u32::from(form.width.ordinal()) << 30;
        let narrow_norm = match form.width {
            W::Bits8 => Some(0x5300_1c00_u32),
            W::Bits16 => Some(0x5300_3c00_u32),
            W::Bits32 | W::Bits64 => None,
        };
        let cmp_op = if form.width == W::Bits64 {
            0xeb00_001f_u32
        } else {
            0x6b00_001f_u32
        };
        let ldxr = |rt, rn| size | enc::rr(0x085f_fc00, rt, rn);
        let stxr = |rs, rt, rn| size | enc::rrr(0x0800_fc00, rt, rn, rs);
        let words: Vec<u32> = match form.operation {
            Op::Load => vec![size | enc::rr(0x08df_fc00, regs[1], regs[0])],
            Op::Store => vec![size | enc::rr(0x089f_fc00, regs[1], regs[0])],
            Op::CompareExchange => {
                let mut w = vec![enc::DMB_ISH, ldxr(12, 9)];
                let exp: u8 = if let Some(norm) = narrow_norm {
                    w.push(enc::rr(norm, 16, 10));
                    16
                } else {
                    10
                };
                w.extend([
                    cmp_op | (u32::from(exp) << 16) | (12 << 5),
                    enc::b_cond(enc::NE, 4),
                    stxr(17, 11, 9),
                    if exp == 16 { 0x35ff_ff71 } else { 0x35ff_ff91 },
                    enc::b(2),
                    enc::CLREX,
                    enc::DMB_ISH,
                ]);
                w
            }
            Op::CompareExchangeOnce => {
                let [addr, exp, repl, obs, out] = [regs[0], regs[1], regs[2], regs[3], regs[4]];
                let mut w = vec![enc::DMB_ISH, ldxr(obs, addr)];
                let cmp_exp = if let Some(norm) = narrow_norm {
                    w.push(enc::rr(norm, out, exp));
                    out
                } else {
                    exp
                };
                w.extend([
                    cmp_op | (u32::from(cmp_exp) << 16) | (u32::from(obs) << 5),
                    enc::b_cond(enc::NE, 4),
                    stxr(out, repl, addr),
                    enc::rri12(0x1100_0000, out, out, 1),
                    enc::b(3),
                    enc::CLREX,
                    0x5280_0000 | u32::from(out),
                    enc::DMB_ISH,
                ]);
                w
            }
            op => {
                let alu = match op {
                    Op::FetchAdd => 0x0b00_0000_u32,
                    Op::FetchSub => 0x4b00_0000,
                    Op::Swap | Op::FetchOr => 0x2a00_0000,
                    Op::FetchAnd => 0x0a00_0000,
                    Op::FetchXor => 0x4a00_0000,
                    _ => return Err(Error::EncodedFormMismatch),
                };
                let wide = u32::from(form.width == W::Bits64) << 31;
                let left = if op == Op::Swap { 31 } else { 11 };
                vec![
                    enc::DMB_ISH,
                    ldxr(11, 9),
                    wide | enc::rrr(alu, 16, left, 10),
                    stxr(17, 16, 9),
                    0x35ff_ffb1,
                    enc::DMB_ISH,
                ]
            }
        };
        Ok(enc::to_bytes(&words))
    }
}

pub(crate) mod float_result_policy {
    use super::{Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint, enc};
    use register_model::RegisterViewId;
    use selected_instructions::{FloatResultPolicyForm, FloatResultPolicyKind, IeeeFloatFormat};
    pub(crate) use target_aarch64::form_effects::float_result_policy::effects;

    pub(crate) fn footprint(
        form: FloatResultPolicyForm,
        operands: &[RegisterViewId],
    ) -> Aarch64SelectedFormFootprint {
        let n = form.operand_count();
        Aarch64SelectedFormFootprint {
            register_reads: operands[..n].to_vec(),
            register_writes: operands[n..n + 1].to_vec(),
            writes_nzcv: true,
            encoded: effects(form),
        }
    }

    pub(crate) fn encode(form: FloatResultPolicyForm, regs: &[u8]) -> Result<Vec<u8>, Error> {
        let inputs = form.operand_count();
        if regs.len() != inputs + 1 || regs.iter().any(|r| *r > 30) {
            return Err(Error::OperandCountMismatch);
        }
        let wide = form.format == IeeeFloatFormat::Binary64;
        let (frac, exp) = if wide { (52_u8, 11_u8) } else { (23, 8) };
        let exp_mask = (1_u16 << exp) - 1;
        let res = regs[inputs];
        let mut w = Vec::with_capacity(20);
        for (i, &g) in regs[..inputs].iter().enumerate() {
            w.push(enc::fmov_to_v(wide, 16 + i as u8, g));
        }
        if form.policy == FloatResultPolicyKind::Trapping {
            w.extend([
                enc::fmov_to_g(wide, res, 16),
                enc::ubfm(wide, res, res, frac, frac + exp - 1),
                enc::cmp_imm(wide, res, exp_mask),
                enc::b_cond(enc::NE, 2),
                enc::BRK_0,
                enc::fmov_to_g(wide, res, 16),
            ]);
        } else {
            let mut patches = Vec::with_capacity(3);
            w.extend([
                enc::fmov_to_g(wide, res, 16),
                enc::ubfm(wide, res, res, 0, 7),
                enc::cmp_imm(wide, res, 0),
            ]);
            patches.push((w.len(), enc::EQ));
            w.push(0);
            for (first, count, imm) in [(frac, exp, exp_mask), (0, frac, 0)] {
                w.extend([
                    enc::fmov_to_g(wide, res, 17),
                    enc::ubfm(wide, res, res, first, first + count - 1),
                    enc::cmp_imm(wide, res, imm),
                ]);
                patches.push((w.len(), enc::NE));
                w.push(0);
            }
            w.extend([
                enc::fmov_to_g(wide, res, 17),
                enc::rri12(if wide { 0xd100_0000 } else { 0x5100_0000 }, res, res, 1),
                enc::b(2),
            ]);
            let retain_raw = w.len();
            w.push(enc::fmov_to_g(wide, res, 17));
            for (pos, cond) in patches {
                w[pos] = enc::b_cond(cond, (retain_raw - pos) as i32);
            }
        }
        Ok(enc::to_bytes(&w))
    }
}

pub(crate) mod ieee_float_arithmetic {
    use super::{Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint, enc};
    use register_model::RegisterViewId;
    use selected_instructions::{
        IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation as Op, IeeeFloatIntegerCarrier as C,
        IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain,
    };
    use semantic_vocabulary::IeeeFloatComparisonOperation as Rel;
    pub(crate) use target_aarch64::form_effects::ieee_float_arithmetic::effects;

    const L: u8 = 16;
    const R: u8 = 17;
    const M: u8 = 18;

    pub(crate) fn footprint(
        form: IeeeFloatArithmeticForm,
        operands: &[RegisterViewId],
    ) -> Aarch64SelectedFormFootprint {
        let n = form.operand_count();
        Aarch64SelectedFormFootprint {
            register_reads: operands[..n].to_vec(),
            register_writes: operands[n..n + 1].to_vec(),
            writes_nzcv: false,
            encoded: effects(form),
        }
    }

    const fn f_two(double: bool, op: u32, vd: u8, vn: u8, vm: u8) -> u32 {
        0x1e20_0800
            | ((double as u32) << 22)
            | ((vm as u32) << 16)
            | (op << 12)
            | ((vn as u32) << 5)
            | vd as u32
    }

    const fn f_one(double: bool, op: u32, vd: u8, vn: u8) -> u32 {
        0x1e20_4000 | ((double as u32) << 22) | (op << 15) | ((vn as u32) << 5) | vd as u32
    }

    const fn fmadd(double: bool, vd: u8, vn: u8, vm: u8, va: u8) -> u32 {
        0x1f00_0000
            | ((double as u32) << 22)
            | ((vm as u32) << 16)
            | ((va as u32) << 10)
            | ((vn as u32) << 5)
            | vd as u32
    }

    const fn f_cmp(equal: bool, double: bool, vd: u8, vn: u8, vm: u8) -> u32 {
        (if equal { 0x5e20_e400 } else { 0x7ea0_e400 })
            | ((double as u32) << 22)
            | ((vm as u32) << 16)
            | ((vn as u32) << 5)
            | vd as u32
    }

    const fn i_to_f(wide: bool, double: bool, unsigned: bool, vd: u8, rn: u8) -> u32 {
        0x1e22_0000
            | ((wide as u32) << 31)
            | ((double as u32) << 22)
            | ((unsigned as u32) << 16)
            | ((rn as u32) << 5)
            | vd as u32
    }

    const fn f_to_i(wide: bool, double: bool, unsigned: bool, rd: u8, vn: u8) -> u32 {
        0x1e38_0000
            | ((wide as u32) << 31)
            | ((double as u32) << 22)
            | ((unsigned as u32) << 16)
            | ((vn as u32) << 5)
            | rd as u32
    }

    fn materialize_top_down(words: &mut Vec<u32>, bits: u64, rd: u8) {
        words.push(enc::movz(rd, (bits >> 48) as u16, 3));
        for shift in (0..3_u8).rev() {
            let imm = (bits >> (16 * u32::from(shift))) as u16;
            if imm != 0 {
                words.push(enc::movk(rd, imm, shift));
            }
        }
    }

    fn trapping_truncation(double: bool, target: C, src: u8, res: u8) -> Vec<u32> {
        let mut w = vec![enc::fmov_to_v(double, L, src)];
        if !double {
            w.push(f_one(false, 0b00_0101, L, L));
        }
        let mag = u32::from(target.bits()) - u32::from(target.is_signed());
        let upper = u64::from(1023 + mag) << 52;
        let lower = if target.is_signed() {
            (1_u64 << 63) | upper | if mag <= 52 { 1_u64 << (52 - mag) } else { 1 }
        } else {
            (-1.0_f64).to_bits()
        };
        for (bound, left, right) in [(lower, L, R), (upper, R, L)] {
            materialize_top_down(&mut w, bound, res);
            w.extend([
                enc::fmov_to_v(true, R, res),
                f_cmp(false, true, M, left, right),
                enc::fmov_to_g(true, res, M),
                enc::cbnz(res, 2),
                enc::BRK_0,
            ]);
        }
        w.push(f_to_i(
            target.bits() == 64,
            true,
            !target.is_signed(),
            res,
            L,
        ));
        if target.is_signed() && target.bits() <= 32 {
            w.push(enc::sbfm(true, res, res, 0, 31));
        }
        w
    }

    fn narrow_truncation(double: bool, target: C, src: u8, res: u8) -> Vec<u32> {
        let mut w = vec![enc::fmov_to_v(double, L, src)];
        if !double {
            w.push(f_one(false, 0b00_0101, L, L));
        }
        let (min_b, max_b) = match target {
            C::I8 => (0xc060_0000_0000_0000_u64, 0x405f_c000_0000_0000_u64),
            C::I16 => (0xc0e0_0000_0000_0000, 0x40df_ffc0_0000_0000),
            C::U8 => (0, 0x406f_e000_0000_0000),
            C::U16 => (0, 0x40ef_ffe0_0000_0000),
            _ => (0, 0),
        };
        for (bound, op) in [(min_b, 0b0100), (max_b, 0b0101)] {
            materialize_top_down(&mut w, bound, res);
            w.extend([enc::fmov_to_v(true, R, res), f_two(true, op, L, L, R)]);
        }
        w.push(f_to_i(false, true, !target.is_signed(), res, L));
        if target.is_signed() {
            w.push(enc::sbfm(true, res, res, 0, 31));
        }
        w
    }

    fn classify_words(form: IeeeFloatArithmeticForm, regs: &[u8]) -> Vec<u32> {
        let double = !form.is_binary32();
        let res = regs[1];
        let restore = enc::fmov_to_g(true, res, L);
        let output = |cls: u16| {
            let imm = match form.operation {
                Op::Classify => cls,
                Op::IsNan => u16::from(cls == 0),
                Op::IsInfinite => u16::from(cls == 1),
                Op::IsFinite => u16::from(cls >= 2),
                Op::IsNormal => u16::from(cls == 2),
                Op::IsSubnormal => u16::from(cls == 3),
                _ => unreachable!("classification operation"),
            };
            enc::movz(res, imm, 0)
        };
        let (exp_first, exp_w, exp_max, mag_w, frac_w) = if double {
            (52_u8, 11_u8, 2047_u16, 63_u8, 52_u8)
        } else {
            (23, 8, 255, 31, 23)
        };
        let mut w = vec![
            enc::fmov_to_v(double, L, regs[0]),
            enc::ubfm(true, res, regs[0], exp_first, exp_first + exp_w - 1),
            0, // cbz -> subnormal
            enc::rri12(0xd100_0000, res, res, exp_max),
            0, // cbz -> nonfinite
            output(2),
            0, // b -> end
        ];
        let sub = w.len();
        w.extend([
            restore,
            enc::ubfm(true, res, res, 0, mag_w - 1),
            0, // cbz -> zero
            output(3),
            0, // b -> end
        ]);
        let zero = w.len();
        w.extend([output(4), 0]); // b -> end
        let nonfin = w.len();
        w.extend([
            restore,
            enc::ubfm(true, res, res, 0, frac_w - 1),
            0, // cbz -> inf
            output(0),
            0, // b -> end
        ]);
        let inf = w.len();
        w.push(output(1));
        let end = w.len();
        if form.operation == Op::Classify {
            w.extend([
                0, // cbz -> done
                enc::fmov_to_v(true, R, res),
                restore,
                enc::ubfm(true, res, res, if double { 63 } else { 31 }, 63),
                enc::ubfm(true, res, res, 32, 31),
                enc::fmov_to_v(true, L, res),
                enc::rrr(0x0ea0_1c00, L, L, R),
                restore,
            ]);
        }
        let patch_cbz = |w: &mut [u32], at: usize, dst: usize| {
            w[at] = enc::cbz(res, (dst as i32) - (at as i32));
        };
        let patch_b = |w: &mut [u32], at: usize, dst: usize| {
            w[at] = enc::b((dst as i32) - (at as i32));
        };
        patch_cbz(&mut w, 2, sub);
        patch_cbz(&mut w, 4, nonfin);
        patch_b(&mut w, 6, end);
        patch_cbz(&mut w, sub + 2, zero);
        patch_b(&mut w, sub + 4, end);
        patch_b(&mut w, zero + 1, end);
        patch_cbz(&mut w, nonfin + 2, inf);
        patch_b(&mut w, nonfin + 4, end);
        if form.operation == Op::Classify {
            let done = w.len();
            patch_cbz(&mut w, end, done);
        }
        w
    }

    fn directed_wrap(
        rounding: IeeeFloatRoundingDirection,
        double: bool,
        res: u8,
        mut w: Vec<u32>,
        op_word: u32,
    ) -> Vec<u32> {
        let mode: u16 = match rounding.ordinal() {
            0 => 3,
            1 => 1,
            2 => 2,
            _ => unreachable!(),
        };
        w.extend([
            0xd53b_4400 | u32::from(res),
            enc::rr(0x9eaf_0000, M, res),
            enc::movz(res, mode << 6, 1),
            0xd51b_4400 | u32::from(res),
            op_word,
            enc::rr(0x9eae_0000, res, M),
            0xd51b_4400 | u32::from(res),
            enc::fmov_to_g(double, res, L),
        ]);
        w
    }

    pub(crate) fn encode(form: IeeeFloatArithmeticForm, regs: &[u8]) -> Result<Vec<u8>, Error> {
        let n = form.operand_count();
        if regs.len() != n + 1 {
            return Err(Error::OperandCountMismatch);
        }
        if regs.iter().any(|r| *r > 30) {
            return Err(Error::EncodedFormMismatch);
        }
        let double = !form.is_binary32();
        let res = regs[n];
        let w: Vec<u32> = match form.operation {
            Op::DirectedBinary {
                operation,
                rounding,
            } => {
                let op = [2, 3, 0, 1][usize::from(operation.ordinal())];
                directed_wrap(
                    rounding,
                    double,
                    res,
                    vec![
                        enc::fmov_to_v(double, L, regs[0]),
                        enc::fmov_to_v(double, R, regs[1]),
                    ],
                    f_two(double, op, L, L, R),
                )
            }
            Op::DirectedSquareRoot { rounding } => directed_wrap(
                rounding,
                double,
                res,
                vec![enc::fmov_to_v(double, L, regs[0])],
                f_one(double, 0b00_0011, L, L),
            ),
            Op::IsNan
            | Op::IsInfinite
            | Op::IsFinite
            | Op::IsNormal
            | Op::IsSubnormal
            | Op::Classify => classify_words(form, regs),
            Op::Compare(cmp) => {
                let mut w = vec![
                    enc::fmov_to_v(double, L, regs[0]),
                    enc::fmov_to_v(double, R, regs[1]),
                ];
                match cmp {
                    Rel::Equal | Rel::NotEqual => w.push(f_cmp(true, double, M, L, R)),
                    Rel::Less => w.push(f_cmp(false, double, M, R, L)),
                    Rel::Greater => w.push(f_cmp(false, double, M, L, R)),
                    Rel::LessOrEqual | Rel::GreaterOrEqual => {
                        let (a, b) = if cmp == Rel::LessOrEqual {
                            (R, L)
                        } else {
                            (L, R)
                        };
                        w.extend([
                            f_cmp(true, double, M, L, R),
                            f_cmp(false, double, L, a, b),
                            enc::rrr(0x0ea0_1c00, M, M, L),
                        ]);
                    }
                }
                w.extend([
                    enc::fmov_to_g(double, res, M),
                    enc::ubfm(true, res, res, 0, 0),
                ]);
                if cmp == Rel::NotEqual {
                    w.extend([
                        enc::rri12(0xd100_0000, res, res, 1),
                        enc::ubfm(true, res, res, 0, 0),
                    ]);
                }
                w
            }
            Op::FusedMultiplyAdd | Op::DirectedFusedMultiplyAdd { .. } => {
                let mut w = vec![
                    enc::fmov_to_v(double, L, regs[0]),
                    enc::fmov_to_v(double, R, regs[1]),
                    enc::fmov_to_v(double, M, regs[2]),
                ];
                let fused = fmadd(double, L, L, R, M);
                if let Some(rounding) = form.operation.rounding_direction() {
                    directed_wrap(rounding, double, res, w, fused)
                } else {
                    w.extend([fused, enc::fmov_to_g(double, res, L)]);
                    w
                }
            }
            Op::MultiplyThenAdd => vec![
                enc::fmov_to_v(double, L, regs[0]),
                enc::fmov_to_v(double, R, regs[1]),
                f_two(double, 0b0000, L, L, R),
                enc::fmov_to_v(double, R, regs[2]),
                f_two(double, 0b0010, L, L, R),
                enc::fmov_to_g(double, res, L),
            ],
            Op::Minimum | Op::Maximum => {
                let (a, b) = if form.operation == Op::Minimum {
                    (R, L)
                } else {
                    (L, R)
                };
                vec![
                    enc::fmov_to_v(double, L, regs[0]),
                    enc::fmov_to_v(double, R, regs[1]),
                    f_cmp(false, double, M, a, b),
                    enc::rrr(0x2e60_1c00, M, L, R),
                    enc::fmov_to_g(double, res, M),
                ]
            }
            Op::Negate | Op::SquareRoot => vec![
                enc::fmov_to_v(double, L, regs[0]),
                f_one(
                    double,
                    if form.operation == Op::Negate {
                        0b00_0010
                    } else {
                        0b00_0011
                    },
                    L,
                    L,
                ),
                enc::fmov_to_g(double, res, L),
            ],
            Op::ConvertFormat => vec![
                enc::fmov_to_v(!double, L, regs[0]),
                f_one(!double, if double { 0b00_0101 } else { 0b00_0100 }, L, L),
                enc::fmov_to_g(double, res, L),
            ],
            Op::TruncateToInteger {
                target,
                domain: IeeeFloatToIntegerDomain::Trapping,
            } => trapping_truncation(double, target, regs[0], res),
            Op::TruncateToInteger { target, .. } if target.bits() < 32 => {
                narrow_truncation(double, target, regs[0], res)
            }
            Op::FromInteger(src) if src.bits() < 32 => vec![
                if src.is_signed() {
                    enc::sbfm(
                        false,
                        res,
                        regs[0],
                        0,
                        if src.bits() == 16 { 15 } else { 7 },
                    )
                } else {
                    enc::ubfm(
                        false,
                        res,
                        regs[0],
                        0,
                        if src.bits() == 16 { 15 } else { 7 },
                    )
                },
                i_to_f(false, double, !src.is_signed(), L, res),
                enc::fmov_to_g(double, res, L),
            ],
            Op::TruncateToInteger { target, .. } => {
                let mut w = vec![
                    enc::fmov_to_v(double, L, regs[0]),
                    f_to_i(target.bits() == 64, double, !target.is_signed(), res, L),
                ];
                if target.is_signed() && target.bits() == 32 {
                    w.push(enc::sbfm(true, res, res, 0, 31));
                }
                w
            }
            Op::FromInteger(src) => vec![
                i_to_f(src.bits() == 64, double, !src.is_signed(), L, regs[0]),
                enc::fmov_to_g(double, res, L),
            ],
            op => {
                let code = match op {
                    Op::Multiply => 0b0000,
                    Op::Divide => 0b0001,
                    Op::Add => 0b0010,
                    Op::Subtract => 0b0011,
                    _ => unreachable!("two-source float arithmetic"),
                };
                vec![
                    enc::fmov_to_v(double, L, regs[0]),
                    enc::fmov_to_v(double, R, regs[1]),
                    f_two(double, code, L, L, R),
                    enc::fmov_to_g(double, res, L),
                ]
            }
        };
        Ok(enc::to_bytes(&w))
    }
}

pub(crate) mod trapping_forms {
    use super::enc;
    use selected_instructions::{SaturatingCarrier as C, TrappingForm, TrappingOperation as Op};
    use semantic_vocabulary::IntegerSign;
    pub(crate) use target_aarch64::form_effects::trapping_forms::{
        conversion_high_bit, effects, operand_count,
    };

    const fn ext_code(carrier: C) -> u32 {
        match carrier {
            C::U8 => 0,
            C::U16 => 1,
            C::U32 => 2,
            C::I8 => 4,
            C::I16 => 5,
            C::I32 => 6,
            C::I64 | C::U64 => panic!("64-bit carriers have no narrow extension check"),
        }
    }

    pub(crate) fn append_words(w: &mut Vec<u32>, form: TrappingForm, regs: &[u8]) {
        let c = form.carrier;
        let skip_trap = |w: &mut Vec<u32>, cond: u8| w.extend([enc::b_cond(cond, 2), enc::BRK_0]);
        let narrow_check = |w: &mut Vec<u32>, v: u8| {
            w.push(0xeb20_001f | (u32::from(v) << 16) | (ext_code(c) << 13) | (u32::from(v) << 5));
            skip_trap(w, enc::EQ);
        };
        if let Op::Convert { source } = form.operation {
            let (src, dst) = (regs[0], regs[1]);
            if let Some(bit) = conversion_high_bit(source, c) {
                w.push(
                    0xf240_001f
                        | ((64 - u32::from(bit)) << 16)
                        | ((63 - u32::from(bit)) << 10)
                        | (u32::from(src) << 5),
                );
                skip_trap(w, enc::EQ);
            } else if source == IntegerSign::Signed && c != C::I64 {
                narrow_check(w, src);
            }
            w.push(enc::mov(dst, src));
            return;
        }
        let (left, right, val, scratch) = (regs[0], regs[1], regs[2], regs[3]);
        match form.operation {
            Op::Add | Op::Subtract if !c.is_narrow() => {
                let op = if form.operation == Op::Add {
                    0xab00_0000
                } else {
                    0xeb00_0000
                };
                w.push(enc::rrr(op, val, left, right));
                let cond = match (form.operation, c.is_signed()) {
                    (_, true) => enc::VC,
                    (Op::Add, false) => enc::LO,
                    _ => enc::HS,
                };
                skip_trap(w, cond);
            }
            Op::Add | Op::Subtract | Op::Multiply if c.is_narrow() => {
                let op = match form.operation {
                    Op::Add => 0x8b00_0000,
                    Op::Subtract => 0xcb00_0000,
                    _ => 0x9b00_7c00,
                };
                w.push(enc::rrr(op, val, left, right));
                narrow_check(w, val);
            }
            Op::Multiply => {
                let hi_op = if c.is_signed() {
                    0x9b40_7c00
                } else {
                    0x9bc0_7c00
                };
                w.extend([
                    enc::rrr(hi_op, scratch, left, right),
                    enc::rrr(0x9b00_7c00, val, left, right),
                    if c.is_signed() {
                        0xeb80_fc1f | (u32::from(val) << 16) | (u32::from(scratch) << 5)
                    } else {
                        enc::cmp_imm(true, scratch, 0)
                    },
                ]);
                skip_trap(w, enc::EQ);
            }
            Op::Divide | Op::Remainder => {
                w.extend([enc::cbnz(right, 2), enc::BRK_0]);
                if c == C::I64 {
                    w.extend([
                        0xb100_041f | (u32::from(right) << 5),
                        0xfa41_0800 | (u32::from(left) << 5),
                    ]);
                    skip_trap(w, enc::VC);
                }
                let q = if form.operation == Op::Divide {
                    val
                } else {
                    scratch
                };
                w.push(enc::rrr(
                    if c.is_signed() {
                        0x9ac0_0c00
                    } else {
                        0x9ac0_0800
                    },
                    q,
                    left,
                    right,
                ));
                if c.is_signed() && c.is_narrow() {
                    narrow_check(w, q);
                }
                if form.operation == Op::Remainder {
                    w.push(enc::msub(val, scratch, right, left));
                }
            }
            Op::ShiftLeft | Op::ShiftRight => {
                w.push(enc::cmp_imm(true, right, c.bits()));
                skip_trap(w, enc::LO);
                if form.operation == Op::ShiftRight {
                    w.push(enc::rrr(
                        if c.is_signed() {
                            0x9ac0_2800
                        } else {
                            0x9ac0_2400
                        },
                        val,
                        left,
                        right,
                    ));
                    return;
                }
                w.push(enc::rrr(0x9ac0_2000, val, left, right));
                if c.is_narrow() {
                    narrow_check(w, val);
                } else {
                    w.extend([
                        enc::rrr(
                            if c.is_signed() {
                                0x9ac0_2800
                            } else {
                                0x9ac0_2400
                            },
                            scratch,
                            val,
                            right,
                        ),
                        enc::cmp_reg(scratch, left),
                    ]);
                    skip_trap(w, enc::EQ);
                }
            }
            Op::Add | Op::Subtract | Op::Convert { .. } => unreachable!(),
        }
    }
}

pub(crate) mod register_access {
    use super::Aarch64SelectedFormEncodingError as Error;
    use selected_instructions::{MemoryFenceKind, RegisterOperation, SystemRegister};
    pub(crate) use target_aarch64::form_effects::register_access::effects;

    const SYSREG_TABLE: [(SystemRegister, u16); 9] = [
        (SystemRegister::SctlrEl1, 0xc080),
        (SystemRegister::TcrEl1, 0xc102),
        (SystemRegister::Ttbr0El1, 0xc100),
        (SystemRegister::Ttbr1El1, 0xc101),
        (SystemRegister::MairEl1, 0xc510),
        (SystemRegister::VbarEl1, 0xc600),
        (SystemRegister::TpidrEl1, 0xc684),
        (SystemRegister::EsrEl1, 0xc290),
        (SystemRegister::FarEl1, 0xc300),
    ];

    pub(crate) fn encode_fence(kind: MemoryFenceKind) -> Option<Vec<u8>> {
        let option: u32 = match kind {
            MemoryFenceKind::Aarch64Load => 0xd,
            MemoryFenceKind::Aarch64Store => 0xe,
            MemoryFenceKind::Aarch64Full => 0xf,
            _ => return None,
        };
        Some((0xd503_30bf_u32 | (option << 8)).to_le_bytes().to_vec())
    }

    pub(crate) fn encode(operation: RegisterOperation, regs: &[u8]) -> Result<Vec<u8>, Error> {
        let [operand] = regs else {
            return Err(Error::OperandCountMismatch);
        };
        if *operand > 30 || !operation.is_admitted() {
            return Err(Error::AlternativeMismatch);
        }
        let (base, reg) = match operation {
            RegisterOperation::SystemRead(r) => (0xd520_0000_u32, r),
            RegisterOperation::SystemWrite(r) => (0xd500_0000_u32, r),
            _ => return Err(Error::AlternativeMismatch),
        };
        let sel = SYSREG_TABLE
            .iter()
            .find(|(r, _)| *r == reg)
            .map(|(_, s)| *s)
            .ok_or(Error::AlternativeMismatch)?;
        Ok((base | (u32::from(sel) << 5) | u32::from(*operand))
            .to_le_bytes()
            .to_vec())
    }
}

mod selected_forms {
    use super::movn_materialization::append_canonical_materialization;
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded, byte_sequences, enc, float_bits,
    };
    use crate::aarch64_physical_register_model_ref;
    use crate::saturating_forms::SaturatingRealization;
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily as Fam, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, RegisterOperation, SelectedInstructionKind as Kind,
        is_float_bit_transfer,
    };
    use semantic_vocabulary::IntegerValue;

    pub const AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET: u16 = 0;
    pub const AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET: u16 = 4;

    const BOUND_WORDS: [(u64, u32); 11] = [
        (i8::MAX as u64, 0xb240_1be0),
        (i8::MIN as i64 as u64, 0xb279_e3e0),
        (i16::MAX as u64, 0xb240_3be0),
        (i16::MIN as i64 as u64, 0xb271_c3e0),
        (i32::MAX as u64, 0xb240_7be0),
        (i32::MIN as i64 as u64, 0xb261_83e0),
        (i64::MAX as u64, 0xb240_fbe0),
        (i64::MIN as u64, 0xb241_03e0),
        (u8::MAX as u64, 0xb240_1fe0),
        (u16::MAX as u64, 0xb240_3fe0),
        (u32::MAX as u64, 0xb240_7fe0),
    ];

    pub fn encode_aarch64_selected_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<Encoded, Error> {
        if let Some(res) = byte_sequences::try_encode(physical, kind, alternative, operands) {
            return res;
        }
        if is_float_bit_transfer(kind) {
            return float_bits::encode(physical, kind, alternative, operands);
        }
        validate_request(physical, kind, alternative, operands)?;
        let regs = resolve_scalar_registers(physical, kind, operands)?;
        validate_return_home(kind, &regs)?;
        let bytes = encode_words(kind, &regs)?;
        Ok(Encoded {
            bytes,
            footprint: footprint(kind, operands),
        })
    }

    pub fn validate_aarch64_selected_form_encoding(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let canonical = encode_aarch64_selected_form(physical, kind, alternative, operands)?;
        enc::check_bytes(bytes, &canonical.bytes)?;
        Ok(canonical)
    }

    fn validate_request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<(), Error> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        let (family, count) = family_and_operand_count(kind)?;
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Error::AlternativeMismatch);
        }
        if operands.len() != count {
            return Err(Error::OperandCountMismatch);
        }
        Ok(())
    }

    #[derive(Clone, Copy)]
    enum SimpleForm {
        Fixed(u32),
        Cset(u8),
        Rr(u32),
        RmRd(u32),
        Cmp0,
        CmpImm(IntegerValue),
        CmpReg,
        Rri12(u32, IntegerValue),
        Rrr(u32),
        Rem(u32),
    }

    impl SimpleForm {
        const fn operands(self) -> usize {
            match self {
                Self::Fixed(_) => 0,
                Self::Cset(_) | Self::Cmp0 | Self::CmpImm(_) => 1,
                Self::Rr(_) | Self::RmRd(_) | Self::CmpReg | Self::Rri12(_, _) => 2,
                Self::Rrr(_) | Self::Rem(_) => 3,
            }
        }

        fn io(self) -> (Vec<u16>, Vec<u16>, bool, bool) {
            match self {
                Self::Fixed(_) => (vec![], vec![], false, false),
                Self::Cset(_) => (vec![], vec![0], false, true),
                Self::Rr(_) | Self::RmRd(_) | Self::Rri12(_, _) => (vec![0], vec![1], false, false),
                Self::Cmp0 | Self::CmpImm(_) => (vec![0], vec![], true, false),
                Self::CmpReg => (vec![0, 1], vec![], true, false),
                Self::Rrr(_) | Self::Rem(_) => (vec![0, 1], vec![2], false, false),
            }
        }

        fn emit(self, w: &mut Vec<u32>, r: &[u8]) -> Result<(), Error> {
            match self {
                Self::Fixed(word) => w.push(word),
                Self::Cset(cond) => {
                    w.push(0x9a9f_07e0 | ((u32::from(cond) ^ 1) << 12) | u32::from(r[0]));
                }
                Self::Rr(op) => w.push(enc::rr(op, r[1], r[0])),
                Self::RmRd(op) => w.push(op | (u32::from(r[0]) << 16) | u32::from(r[1])),
                Self::Cmp0 => w.push(enc::cmp_imm(true, r[0], 0)),
                Self::CmpImm(imm) => w.push(enc::cmp_imm(true, r[0], u12(imm)?)),
                Self::CmpReg => w.push(enc::cmp_reg(r[0], r[1])),
                Self::Rri12(op, imm) => w.push(enc::rri12(op, r[1], r[0], u12(imm)?)),
                Self::Rrr(op) => w.push(enc::rrr(op, r[2], r[0], r[1])),
                Self::Rem(div_op) => {
                    if r[2] == r[0] || r[2] == r[1] {
                        return Err(Error::EncodedFormMismatch);
                    }
                    w.extend([
                        enc::rrr(div_op, r[2], r[0], r[1]),
                        enc::msub(r[2], r[2], r[1], r[0]),
                    ]);
                }
            }
            Ok(())
        }
    }

    #[rustfmt::skip]
    fn simple_form(kind: Kind) -> Option<(Fam, SimpleForm)> {
        use SimpleForm as S;
        Some(match kind {
            Kind::AtomicFence => (Fam::AtomicFence, S::Fixed(enc::DMB_ISH)),
            Kind::Crash => (Fam::Crash, S::Fixed(enc::BRK_0)),
            Kind::MaterializeBooleanEqual => (Fam::MaterializeBooleanEqual, S::Cset(enc::EQ)),
            Kind::MaterializeBooleanU64LessThan => (Fam::MaterializeBooleanU64LessThan, S::Cset(enc::LO)),
            Kind::MaterializeBooleanI64LessThan => (Fam::MaterializeBooleanI64LessThan, S::Cset(enc::LT)),
            Kind::MaterializeBooleanU64LessOrEqual => (Fam::MaterializeBooleanU64LessOrEqual, S::Cset(enc::LS)),
            Kind::MaterializeBooleanI64LessOrEqual => (Fam::MaterializeBooleanI64LessOrEqual, S::Cset(enc::LE)),
            Kind::ZeroExtendU8 => (Fam::ZeroExtendU8, S::Rr(0xd340_1c00)),
            Kind::ZeroExtendU16 => (Fam::ZeroExtendU16, S::Rr(0xd340_3c00)),
            Kind::ZeroExtendU32 => (Fam::ZeroExtendU32, S::Rr(0xd340_7c00)),
            Kind::SignExtendI8 => (Fam::SignExtendI8, S::Rr(0x9340_1c00)),
            Kind::SignExtendI16 => (Fam::SignExtendI16, S::Rr(0x9340_3c00)),
            Kind::SignExtendI32 => (Fam::SignExtendI32, S::Rr(0x9340_7c00)),
            Kind::CopyI64 => (Fam::CopyI64, S::RmRd(0xaa00_03e0)),
            Kind::BitwiseNotI64 => (Fam::BitwiseNotI64, S::RmRd(0xaa20_03e0)),
            Kind::CompareI64Zero => (Fam::CompareI64Zero, S::Cmp0),
            Kind::CompareI64Immediate { immediate } => (Fam::CompareI64Immediate, S::CmpImm(immediate)),
            Kind::CompareI64 => (Fam::CompareI64, S::CmpReg),
            Kind::ExactAddI64Immediate { immediate, .. } => (Fam::ExactAddI64Immediate, S::Rri12(0x9100_0000, immediate)),
            Kind::ExactSubtractI64Immediate { immediate, .. } => (Fam::ExactSubtractI64Immediate, S::Rri12(0xd100_0000, immediate)),
            Kind::ByteViewAddress => (Fam::ByteViewAddress, S::Rrr(0x8b00_0000)),
            Kind::WrappingAddI64 => (Fam::WrappingAddI64, S::Rrr(0x8b00_0000)),
            Kind::ExactAddI64 { .. } => (Fam::ExactAddI64, S::Rrr(0x8b00_0000)),
            Kind::WrappingSubtractI64 => (Fam::WrappingSubtractI64, S::Rrr(0xcb00_0000)),
            Kind::ExactSubtractI64 { .. } => (Fam::ExactSubtractI64, S::Rrr(0xcb00_0000)),
            Kind::WrappingMultiplyI64 => (Fam::WrappingMultiplyI64, S::Rrr(0x9b00_7c00)),
            Kind::ExactMultiplyI64 { .. } => (Fam::ExactMultiplyI64, S::Rrr(0x9b00_7c00)),
            Kind::BitwiseAndI64 => (Fam::BitwiseAndI64, S::Rrr(0x8a00_0000)),
            Kind::BitwiseOrI64 => (Fam::BitwiseOrI64, S::Rrr(0xaa00_0000)),
            Kind::BitwiseXorI64 => (Fam::BitwiseXorI64, S::Rrr(0xca00_0000)),
            Kind::ExactDivideU64 { .. } => (Fam::ExactDivideU64, S::Rrr(0x9ac0_0800)),
            Kind::WrappingDivideI64 { .. } => (Fam::WrappingDivideI64, S::Rrr(0x9ac0_0c00)),
            Kind::ExactDivideI64 { .. } => (Fam::ExactDivideI64, S::Rrr(0x9ac0_0c00)),
            Kind::ExactRemainderU64 { .. } => (Fam::ExactRemainderU64, S::Rem(0x9ac0_0800)),
            Kind::WrappingRemainderI64 { .. } => (Fam::WrappingRemainderI64, S::Rem(0x9ac0_0c00)),
            Kind::ExactRemainderI64 { .. } => (Fam::ExactRemainderI64, S::Rem(0x9ac0_0c00)),
            Kind::WrappingShiftLeftI64 => (Fam::WrappingShiftLeftI64, S::Rrr(0x9ac0_2000)),
            Kind::ExactShiftLeftI64 { .. } => (Fam::ExactShiftLeftI64, S::Rrr(0x9ac0_2000)),
            Kind::WrappingShiftRightU64 => (Fam::WrappingShiftRightU64, S::Rrr(0x9ac0_2400)),
            Kind::ExactShiftRightU64 { .. } => (Fam::ExactShiftRightU64, S::Rrr(0x9ac0_2400)),
            Kind::WrappingShiftRightI64 => (Fam::WrappingShiftRightI64, S::Rrr(0x9ac0_2800)),
            Kind::ExactShiftRightI64 { .. } => (Fam::ExactShiftRightI64, S::Rrr(0x9ac0_2800)),
            _ => return None,
        })
    }

    fn family_and_operand_count(kind: Kind) -> Result<(Fam, usize), Error> {
        if let Some((family, form)) = simple_form(kind) {
            return Ok((family, form.operands()));
        }
        Ok(match kind {
            Kind::PackedAddI16x8
            | Kind::PackedSubI16x8
            | Kind::PackedShuffleI16x8 { .. }
            | Kind::PackedShuffleGeneralI16x8 { .. }
            | Kind::PackedShiftRightArithmeticI16x8 { .. }
            | Kind::PackedWidenU8x8ToI16x8
            | Kind::PackedNarrowI16x8ToU8x8Saturating
            | Kind::PackedLoadI16x4ZeroExtend
            | Kind::PackedWidenU8x4ToI16x8
            | Kind::PackedNarrowI16x8ToU8x4Saturating
            | Kind::PackedLoad128 { .. }
            | Kind::PackedStore128 { .. }
            | Kind::PackedLoad64ZeroExtend { .. }
            | Kind::PackedStore64 { .. }
            | Kind::PackedLoad32ZeroExtend { .. }
            | Kind::PackedStore32 { .. }
            | Kind::PackedMaterializeZero
            | Kind::PackedFromGpr64ZeroExtend
            | Kind::PackedFromGpr64Pair
            | Kind::PackedAddI16x8Reg
            | Kind::PackedSubI16x8Reg
            | Kind::PackedShiftRightArithmeticI16x8Reg { .. }
            | Kind::PackedShuffleI16x8Reg { .. }
            | Kind::PackedWidenU8x8ToI16x8Reg
            | Kind::PackedNarrowI16x8ToU8x8SaturatingReg
            | Kind::PackedAddU8x16
            | Kind::PackedMinI16x8
            | Kind::PackedCmpGtI16x8
            | Kind::PackedBitAndI16x8
            | Kind::PackedBitAndNotI16x8
            | Kind::PackedAbsI16x8
            | Kind::PackedAddU8x16Reg
            | Kind::PackedMinI16x8Reg
            | Kind::PackedCmpGtI16x8Reg
            | Kind::PackedBitAndI16x8Reg
            | Kind::PackedBitAndNotI16x8Reg
            | Kind::PackedAbsI16x8Reg
            | Kind::PrivilegedCrash
            | Kind::InstalledInterruptCpuCheck { .. }
            | Kind::InstalledInterruptTaskCommit { .. }
            | Kind::InstalledInterruptTablePublication { .. }
            | Kind::PortWrite { .. } => return Err(Error::AlternativeMismatch),
            Kind::AccessorScalarAccess { form, .. } => (Fam::AccessorScalarAccess(form), 2),
            Kind::AtomicAccess { form } => {
                (Fam::AtomicAccess(form), usize::from(form.operand_count()))
            }
            Kind::NearestIeeeFloatArithmetic { form } => (
                Fam::NearestIeeeFloatArithmetic(form),
                form.operand_count() + 1,
            ),
            Kind::FloatResultPolicy { form } => {
                (Fam::FloatResultPolicy(form), form.operand_count() + 1)
            }
            Kind::RegisterAccess {
                operation: RegisterOperation::MemoryFence(fence),
            } => {
                if fence.target() != selected_instructions::RegisterTargetApplicability::Aarch64 {
                    return Err(Error::AlternativeMismatch);
                }
                (Fam::MemoryFence, 0)
            }
            Kind::RegisterAccess { operation } => (
                if operation.produces_value() {
                    Fam::SystemRegisterRead
                } else {
                    Fam::SystemRegisterWrite
                },
                1,
            ),
            Kind::TrappingInteger { form } => (
                Fam::TrappingInteger(form),
                super::trapping_forms::operand_count(form),
            ),
            Kind::MaterializeI64 { .. } => (Fam::MaterializeI64, 1),
            Kind::ImageAddress { .. } => (Fam::ImageAddress, 1),
            Kind::SaturatingAdd { carrier }
            | Kind::SaturatingSubtract { carrier }
            | Kind::SaturatingDivide { carrier, .. }
            | Kind::SaturatingRemainder { carrier, .. }
            | Kind::SaturatingMultiply { carrier } => {
                let family = match kind {
                    Kind::SaturatingAdd { .. } => Fam::SaturatingAdd(carrier),
                    Kind::SaturatingSubtract { .. } => Fam::SaturatingSubtract(carrier),
                    Kind::SaturatingDivide { .. } => Fam::SaturatingDivide(carrier),
                    Kind::SaturatingRemainder { .. } => Fam::SaturatingRemainder(carrier),
                    _ => Fam::SaturatingMultiply(carrier),
                };
                let count = SaturatingRealization::of_kind(kind)
                    .ok_or(Error::EncodedFormMismatch)?
                    .operand_count();
                (family, count)
            }
            Kind::ReturnScalar => (Fam::ReturnScalar, 1),
            Kind::ReturnAggregate { fragment_count } => {
                if !(1..=2).contains(&fragment_count) {
                    return Err(Error::EncodedFormMismatch);
                }
                (Fam::ReturnAggregate, usize::from(fragment_count))
            }
            Kind::ReturnUnit => (Fam::ReturnUnit, 0),
            _ => return Err(Error::LayoutDependentForm),
        })
    }

    fn validate_return_home(kind: Kind, regs: &[u8]) -> Result<(), Error> {
        if matches!(kind, Kind::ReturnScalar) && regs != [0] {
            return Err(Error::EncodedFormMismatch);
        }
        if let Kind::ReturnAggregate { fragment_count } = kind
            && (!(1..=2).contains(&fragment_count)
                || regs != &([0, 1][..usize::from(fragment_count)]))
        {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(())
    }

    fn resolve_scalar_registers(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, Error> {
        if kind == Kind::ReturnScalar
            && physical
                .model()
                .view_named("d0")
                .is_some_and(|v| operands == [v.id])
        {
            return Ok(vec![0]);
        }
        resolve_registers(physical, operands)
    }

    pub(super) fn request_registers<const COUNT: usize>(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        family: Fam,
        operands: &[RegisterViewId],
        first_early: usize,
    ) -> Result<[u8; COUNT], Error> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Error::AlternativeMismatch);
        }
        let regs: [u8; COUNT] = resolve_registers(physical, operands)?
            .try_into()
            .map_err(|_| Error::OperandCountMismatch)?;
        if (first_early..COUNT).any(|slot| regs[..slot].contains(&regs[slot])) {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(regs)
    }

    pub(crate) fn resolve_registers(
        physical: &ValidatedPhysicalRegisterModel,
        operands: &[RegisterViewId],
    ) -> Result<Vec<u8>, Error> {
        operands
            .iter()
            .map(|&id| {
                let view = physical
                    .model()
                    .view(id)
                    .ok_or(Error::UnknownOrNonGpr64View(id))?;
                view.name
                    .strip_prefix('x')
                    .and_then(|s| s.parse::<u8>().ok())
                    .filter(|&i| i <= 30 && view.bits == 64 && view.allocatable)
                    .ok_or(Error::UnknownOrNonGpr64View(id))
            })
            .collect()
    }

    fn integer_bits(value: IntegerValue) -> Result<u64, Error> {
        match value {
            IntegerValue::Signed(v) => i64::try_from(v)
                .map(|v| v as u64)
                .map_err(|_| Error::IntegerOutsideI64Bits),
            IntegerValue::Unsigned(v) => u64::try_from(v).map_err(|_| Error::IntegerOutsideI64Bits),
        }
    }

    fn u12(value: IntegerValue) -> Result<u16, Error> {
        match value {
            IntegerValue::Unsigned(v) if v <= 4095 => Ok(v as u16),
            _ => Err(Error::ImmediateOutsideU12),
        }
    }

    fn encode_words(kind: Kind, r: &[u8]) -> Result<Vec<u8>, Error> {
        let mut w = Vec::with_capacity(8);
        if let Some((_, form)) = simple_form(kind) {
            form.emit(&mut w, r)?;
            return Ok(enc::to_bytes(&w));
        }
        match kind {
            Kind::AccessorScalarAccess { .. } => return Err(Error::EncodedFormMismatch),
            Kind::AtomicAccess { form } => return super::atomic::encode(form, r),
            Kind::NearestIeeeFloatArithmetic { form } => {
                return super::ieee_float_arithmetic::encode(form, r);
            }
            Kind::FloatResultPolicy { form } => return super::float_result_policy::encode(form, r),
            Kind::RegisterAccess {
                operation: RegisterOperation::MemoryFence(fence),
            } => {
                if !r.is_empty() {
                    return Err(Error::OperandCountMismatch);
                }
                return super::register_access::encode_fence(fence)
                    .ok_or(Error::AlternativeMismatch);
            }
            Kind::RegisterAccess { operation } => {
                return super::register_access::encode(operation, r);
            }
            Kind::TrappingInteger { form } => {
                let s = form.source_count();
                if r[s..]
                    .iter()
                    .enumerate()
                    .any(|(i, late)| r[..s].contains(late) || r[s + i + 1..].contains(late))
                {
                    return Err(Error::EncodedFormMismatch);
                }
                super::trapping_forms::append_words(&mut w, form, r);
            }
            Kind::MaterializeI64 { value } => {
                append_canonical_materialization(&mut w, r[0], integer_bits(value)?);
            }
            Kind::ImageAddress { .. } => {
                w.extend([
                    0x9000_0000 | u32::from(r[0]),
                    enc::rri12(0x9100_0000, r[0], r[0], 0),
                ]);
            }
            selected_instructions::saturating_add_or_subtract_pattern!()
            | Kind::SaturatingDivide { .. }
            | Kind::SaturatingRemainder { .. }
            | Kind::SaturatingMultiply { .. } => append_saturating(&mut w, kind, r)?,
            Kind::ReturnScalar | Kind::ReturnAggregate { .. } | Kind::ReturnUnit => {
                w.push(u32::from_le_bytes(crate::ORDINARY_RETURN_BYTES));
            }
            _ => return Err(Error::LayoutDependentForm),
        }
        Ok(enc::to_bytes(&w))
    }

    fn bound_word(bound: u64, rd: u8) -> Result<u32, Error> {
        BOUND_WORDS
            .iter()
            .find(|(v, _)| *v == bound)
            .map(|(_, word)| word | u32::from(rd))
            .ok_or(Error::EncodedFormMismatch)
    }

    fn append_saturating(w: &mut Vec<u32>, kind: Kind, r: &[u8]) -> Result<(), Error> {
        let real = SaturatingRealization::of_kind(kind).ok_or(Error::EncodedFormMismatch)?;
        let (left, right, val) = (r[0], r[1], r[2]);
        if real.operand_count() == 4
            && (r[2..].iter().any(|late| r[..2].contains(late)) || r[2] == r[3])
        {
            return Err(Error::EncodedFormMismatch);
        }
        match real {
            SaturatingRealization::AddU64 => w.extend([
                enc::rrr(0xab00_0000, val, left, right),
                enc::rr(0xda9f_3000, val, val),
            ]),
            SaturatingRealization::SubtractUnsigned => w.extend([
                enc::rrr(0xeb00_0000, val, left, right),
                enc::rr(0x9a9f_2000, val, val),
            ]),
            SaturatingRealization::DivideUnsigned => {
                w.push(enc::rrr(0x9ac0_0800, val, left, right));
            }
            SaturatingRealization::Remainder { signed } => {
                if val == left || val == right {
                    return Err(Error::EncodedFormMismatch);
                }
                w.extend([
                    enc::rrr(
                        if signed { 0x9ac0_0c00 } else { 0x9ac0_0800 },
                        val,
                        left,
                        right,
                    ),
                    enc::msub(val, val, right, left),
                ]);
            }
            SaturatingRealization::ClampNarrow { operation, .. } => {
                let s = r[3];
                let op = match operation {
                    selected_instructions::SaturatingOperation::Add => 0x8b00_0000,
                    selected_instructions::SaturatingOperation::Subtract => 0xcb00_0000,
                    selected_instructions::SaturatingOperation::Divide => 0x9ac0_0c00,
                    selected_instructions::SaturatingOperation::Multiply => 0x9b00_7c00,
                    selected_instructions::SaturatingOperation::Remainder => unreachable!(),
                };
                w.push(enc::rrr(op, val, left, right));
                for (i, bound) in real.clamp_bounds().into_iter().enumerate() {
                    let cond = if i == 0 {
                        real.upper_condition() as u8
                    } else {
                        enc::LT
                    };
                    w.extend([
                        bound_word(bound, s)?,
                        enc::cmp_reg(val, s),
                        enc::csel(val, s, val, cond),
                    ]);
                }
            }
            SaturatingRealization::MultiplyI64 => {
                let s = r[3];
                w.extend([
                    enc::rrr(0x9b40_7c00, s, left, right),
                    enc::rrr(0x9b00_7c00, val, left, right),
                    0xeb80_fc1f | (u32::from(val) << 16) | (u32::from(s) << 5),
                    enc::rr(0x937f_fc00, s, s),
                    enc::rr(0xd240_f800, s, s),
                    enc::csel(val, s, val, enc::NE),
                ]);
            }
            SaturatingRealization::MultiplyU64 => {
                let s = r[3];
                w.extend([
                    enc::rrr(0x9bc0_7c00, s, left, right),
                    enc::rrr(0x9b00_7c00, val, left, right),
                    enc::cmp_imm(true, s, 0),
                    enc::rr(0xda9f_0000, val, val),
                ]);
            }
            SaturatingRealization::OverflowI64 { subtract } => {
                let s = r[3];
                w.extend([
                    enc::rr(0x937f_fc00, s, left),
                    enc::rr(0xd240_f800, s, s),
                    enc::rrr(
                        if subtract { 0xeb00_0000 } else { 0xab00_0000 },
                        val,
                        left,
                        right,
                    ),
                    enc::csel(val, s, val, enc::VS),
                ]);
            }
            SaturatingRealization::DivideI64 => {
                let s = r[3];
                w.extend([
                    enc::rrr(0x9ac0_0c00, val, left, right),
                    bound_word(i64::MIN as u64, s)?,
                    enc::cmp_reg(val, s),
                    0xba41_0800 | (u32::from(right) << 5),
                    bound_word(i64::MAX as u64, s)?,
                    enc::csel(val, s, val, enc::EQ),
                ]);
            }
        }
        Ok(())
    }

    pub(crate) fn footprint(kind: Kind, operands: &[RegisterViewId]) -> Footprint {
        match kind {
            Kind::AtomicAccess { form } => return super::atomic::footprint(form, operands),
            Kind::NearestIeeeFloatArithmetic { form } => {
                return super::ieee_float_arithmetic::footprint(form, operands);
            }
            Kind::FloatResultPolicy { form } => {
                return super::float_result_policy::footprint(form, operands);
            }
            _ => {}
        }
        let (read_idx, write_idx, writes_nzcv, reads_nzcv) = if let Some((_, form)) =
            simple_form(kind)
        {
            form.io()
        } else {
            let (r, w, z) = match kind {
                Kind::AccessorScalarAccess { form, .. } => {
                    if form.write {
                        (vec![0, 1], vec![], false)
                    } else {
                        (vec![0], vec![1], false)
                    }
                }
                Kind::RegisterAccess {
                    operation: RegisterOperation::MemoryFence(_),
                }
                | Kind::ReturnScalar
                | Kind::ReturnAggregate { .. }
                | Kind::ReturnUnit
                | Kind::ConditionalBranchNonZero
                | Kind::ConditionalBranchU64LessThan
                | Kind::ConditionalBranchI64LessThan => (vec![], vec![], false),
                Kind::RegisterAccess { operation } => {
                    if operation.produces_value() {
                        (vec![], vec![0], false)
                    } else {
                        (vec![0], vec![], false)
                    }
                }
                Kind::TrappingInteger { form } => (
                    (0..form.source_count() as u16).collect(),
                    (form.source_count() as u16..super::trapping_forms::operand_count(form) as u16)
                        .collect(),
                    true,
                ),
                Kind::MaterializeI64 { .. } | Kind::ImageAddress { .. } => (vec![], vec![0], false),
                selected_instructions::saturating_add_or_subtract_pattern!()
                | Kind::SaturatingDivide { .. }
                | Kind::SaturatingRemainder { .. }
                | Kind::SaturatingMultiply { .. } => {
                    let r = SaturatingRealization::of_kind(kind).expect("saturating realization");
                    (
                        vec![0, 1],
                        (2..r.operand_count() as u16).collect(),
                        r.defines_nzcv(),
                    )
                }
                _ => unreachable!("non-scalar forms have dedicated footprints"),
            };
            (r, w, z, false)
        };
        let register_reads = read_idx.iter().map(|&i| operands[usize::from(i)]).collect();
        let register_writes = write_idx
            .iter()
            .map(|&i| operands[usize::from(i)])
            .collect();
        let physical = aarch64_physical_register_model_ref();
        let units = |name: &str| physical.view_named(name).unwrap().units.clone();
        let encoded = match kind {
            Kind::RegisterAccess {
                operation: RegisterOperation::MemoryFence(_),
            } => MachineEncodedEffects::catalog_fence_v1(),
            Kind::RegisterAccess { operation } => {
                super::register_access::effects(operation.produces_value())
            }
            Kind::AtomicFence => MachineEncodedEffects::atomic_fence_v1(),
            Kind::Crash => MachineEncodedEffects::crash_v1(&units("pc")),
            Kind::TrappingInteger { .. } => super::trapping_forms::effects(read_idx, write_idx),
            Kind::ReturnScalar | Kind::ReturnAggregate { .. } | Kind::ReturnUnit => {
                MachineEncodedEffects {
                    external_operand_reads: vec![],
                    external_operand_writes: vec![],
                    implicit_unit_uses: units("x30"),
                    implicit_unit_defs: units("pc"),
                    implicit_unit_clobbers: vec![],
                    memory: MachineEncodedMemoryEffect::NoneV1,
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                        target: physical.view_named("x30").unwrap().id,
                    },
                }
            }
            Kind::ConditionalBranchNonZero
            | Kind::ConditionalBranchU64LessThan
            | Kind::ConditionalBranchI64LessThan => {
                let mut uses = units("nzcv");
                uses.extend(units("pc"));
                uses.sort_unstable();
                uses.dedup();
                MachineEncodedEffects {
                    external_operand_reads: vec![],
                    external_operand_writes: vec![],
                    implicit_unit_uses: uses,
                    implicit_unit_defs: units("pc"),
                    implicit_unit_clobbers: vec![],
                    memory: MachineEncodedMemoryEffect::NoneV1,
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
                }
            }
            _ => {
                let mut eff = MachineEncodedEffects::fallthrough_v1(read_idx, write_idx);
                if writes_nzcv {
                    eff.implicit_unit_defs = units("nzcv");
                }
                if reads_nzcv {
                    eff.implicit_unit_uses = units("nzcv");
                }
                eff
            }
        };
        Footprint {
            register_reads,
            register_writes,
            writes_nzcv,
            encoded,
        }
    }
}

mod branch_forms {
    use super::selected_forms::{footprint, resolve_registers};
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded, enc,
    };
    use crate::aarch64_physical_register_model_ref;
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily as Fam, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, SelectedInstructionKind as Kind,
    };

    pub const AARCH64_CONDITIONAL_BRANCH_BYTE_REACH: std::ops::Range<i64> = -(1 << 20)..(1 << 20);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64ConditionalBranchForm {
        CanonicalWordV1,
        WidenedPairV1,
    }

    fn arm(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        family: Fam,
    ) -> Result<(u8, u8, Kind), Error> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        if alternative != (MachineAlternativeKey { family, variant: 0 }) {
            return Err(Error::AlternativeMismatch);
        }
        match family {
            Fam::ConditionalBranchNonZero => Ok((enc::NE, enc::EQ, Kind::ConditionalBranchNonZero)),
            Fam::ConditionalBranchU64LessThan => {
                Ok((enc::LO, enc::HS, Kind::ConditionalBranchU64LessThan))
            }
            Fam::ConditionalBranchI64LessThan => {
                Ok((enc::LT, enc::GE, Kind::ConditionalBranchI64LessThan))
            }
            _ => Err(Error::AlternativeMismatch),
        }
    }

    fn imm19_words(disp: i64) -> Result<i32, Error> {
        if disp % 4 != 0 {
            return Err(Error::BranchDisplacementMisaligned);
        }
        if !AARCH64_CONDITIONAL_BRANCH_BYTE_REACH.contains(&disp) {
            return Err(Error::BranchDisplacementOutsideImm19);
        }
        Ok((disp / 4) as i32)
    }

    fn widened_imm26_words(disp: i64) -> Result<i32, Error> {
        if disp % 4 != 0 {
            return Err(Error::BranchDisplacementMisaligned);
        }
        let words = disp / 4 - 1;
        if !(-(1_i64 << 25)..(1_i64 << 25)).contains(&words) {
            return Err(Error::BranchDisplacementOutsideImm26);
        }
        Ok(words as i32)
    }

    pub fn encode_aarch64_selected_conditional_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        family: Fam,
        form: Aarch64ConditionalBranchForm,
        byte_displacement_from_instruction: i64,
    ) -> Result<Encoded, Error> {
        let (cond, inv, kind) = arm(physical, alternative, family)?;
        let bytes = match form {
            Aarch64ConditionalBranchForm::CanonicalWordV1 => {
                enc::b_cond(cond, imm19_words(byte_displacement_from_instruction)?)
                    .to_le_bytes()
                    .to_vec()
            }
            Aarch64ConditionalBranchForm::WidenedPairV1 => {
                let words = [
                    enc::b_cond(inv, 2),
                    enc::b(widened_imm26_words(byte_displacement_from_instruction)?),
                ];
                enc::to_bytes(&words)
            }
        };
        Ok(Encoded {
            bytes,
            footprint: footprint(kind, &[]),
        })
    }

    pub fn validate_aarch64_selected_conditional_branch_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        family: Fam,
        form: Aarch64ConditionalBranchForm,
        byte_displacement_from_instruction: i64,
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let expected = encode_aarch64_selected_conditional_branch_form(
            physical,
            alternative,
            family,
            form,
            byte_displacement_from_instruction,
        )?;
        enc::check_bytes(bytes, &expected.bytes)?;
        Ok(expected)
    }

    pub fn encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
        physical: &ValidatedPhysicalRegisterModel,
        source: RegisterViewId,
        byte_displacement_from_instruction: i64,
    ) -> Result<Encoded, Error> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        let reg = resolve_registers(physical, &[source])?[0];
        let words = imm19_words(byte_displacement_from_instruction)?;
        let pc = aarch64_physical_register_model_ref()
            .view_named("pc")
            .unwrap()
            .units
            .clone();
        Ok(Encoded {
            bytes: enc::cbnz(reg, words).to_le_bytes().to_vec(),
            footprint: Footprint {
                register_reads: vec![source],
                register_writes: vec![],
                writes_nzcv: false,
                encoded: MachineEncodedEffects {
                    external_operand_reads: vec![],
                    external_operand_writes: vec![],
                    implicit_unit_uses: pc.clone(),
                    implicit_unit_defs: pc,
                    implicit_unit_clobbers: vec![],
                    memory: MachineEncodedMemoryEffect::NoneV1,
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
                },
            },
        })
    }

    pub fn validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
        physical: &ValidatedPhysicalRegisterModel,
        source: RegisterViewId,
        byte_displacement_from_instruction: i64,
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let expected = encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
            physical,
            source,
            byte_displacement_from_instruction,
        )?;
        enc::check_bytes(bytes, &expected.bytes)?;
        Ok(expected)
    }
}

mod jump {
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded, enc,
    };
    use register_model::ValidatedPhysicalRegisterModel;
    use selected_instructions::{
        MachineAlternativeFamily as Fam, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior,
    };

    fn jump_footprint(physical: &ValidatedPhysicalRegisterModel) -> Footprint {
        let pc = physical
            .model()
            .view_named("pc")
            .expect("canonical model has PC")
            .units
            .clone();
        Footprint {
            register_reads: vec![],
            register_writes: vec![],
            writes_nzcv: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: vec![],
                external_operand_writes: vec![],
                implicit_unit_uses: pc.clone(),
                implicit_unit_defs: pc,
                implicit_unit_clobbers: vec![],
                memory: MachineEncodedMemoryEffect::NoneV1,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::UnconditionalRelativeBranchV1,
            },
        }
    }

    pub fn encode_aarch64_selected_jump_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        displacement: i64,
    ) -> Result<Encoded, Error> {
        if displacement % 4 != 0 {
            return Err(Error::BranchDisplacementMisaligned);
        }
        let words = displacement / 4;
        if !(-(1_i64 << 25)..(1_i64 << 25)).contains(&words) {
            return Err(Error::BranchDisplacementOutsideImm26);
        }
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        if alternative
            != (MachineAlternativeKey {
                family: Fam::Jump,
                variant: 0,
            })
        {
            return Err(Error::AlternativeMismatch);
        }
        Ok(Encoded {
            bytes: enc::b(words as i32).to_le_bytes().to_vec(),
            footprint: jump_footprint(physical),
        })
    }

    pub fn validate_aarch64_selected_jump_form(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        displacement: i64,
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Error::NonCanonicalPhysicalModel);
        }
        if alternative
            != (MachineAlternativeKey {
                family: Fam::Jump,
                variant: 0,
            })
        {
            return Err(Error::AlternativeMismatch);
        }
        if bytes.len() != 4 {
            return Err(Error::MalformedEncoding);
        }
        let expected = encode_aarch64_selected_jump_form(physical, alternative, displacement)
            .map_err(|_| Error::EncodedFormMismatch)?;
        if bytes != expected.bytes.as_slice() {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(expected)
    }
}

pub(crate) mod floating_control {
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded,
    };
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeKey, MachineSemanticKind,
        SelectedInstructionKind as Kind,
    };
    pub(crate) use target_aarch64::form_effects::floating_control::effects;

    pub fn encode_aarch64_selected_floating_control_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<Encoded, Error> {
        let semantic = match kind {
            Kind::SaveFloatingControl {
                slot: LocalStorageSlotId::Boundary { .. },
            } => MachineSemanticKind::SaveFloatingControl,
            Kind::RestoreFloatingControl {
                slot: LocalStorageSlotId::Boundary { .. },
            } => MachineSemanticKind::RestoreFloatingControl,
            _ => return Err(Error::EncodedFormMismatch),
        };
        let Ok(offset) = u32::try_from(displacement) else {
            return Err(Error::EncodedFormMismatch);
        };
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || !operands.is_empty()
            || alternative
                != (MachineAlternativeKey {
                    family: semantic.into(),
                    variant: 0,
                })
            || !offset.is_multiple_of(8)
        {
            return Err(Error::EncodedFormMismatch);
        }
        let restore = semantic == MachineSemanticKind::RestoreFloatingControl;
        let bytes = if restore {
            crate::encode_restore_fpcr_from_sp_displacement(offset)
        } else {
            crate::encode_save_fpcr_to_sp_displacement(offset)
        }
        .map_err(|_| Error::EncodedFormMismatch)?;
        Ok(Encoded {
            bytes,
            footprint: Footprint {
                register_reads: Vec::new(),
                register_writes: Vec::new(),
                writes_nzcv: false,
                encoded: effects(restore),
            },
        })
    }

    pub(crate) fn validate_aarch64_selected_floating_control_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let expected = encode_aarch64_selected_floating_control_form(
            physical,
            kind,
            alternative,
            operands,
            displacement,
        )?;
        if bytes != expected.bytes.as_slice() {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(expected)
    }
}

mod memory {
    use super::selected_forms::{request_registers, resolve_registers};
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded, enc,
    };
    use crate::immediate_offset::{
        ADD_IMMEDIATE, address_words, aligned_frame_address_words, append_page_steps,
    };
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        FrameStorageSlotId, MachineAlternativeFamily as Fam, MachineAlternativeKey,
        MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect as MemEff,
        MachineEncodedStackEffect, MachineEncodedTrapBehavior as Trap, PackedByteWidth,
        SelectedInstructionKind as Kind,
    };

    fn mem_footprint(
        operands: &[RegisterViewId],
        read_idx: Vec<u16>,
        write_idx: Vec<u16>,
        implicit_unit_uses: Vec<register_model::RegisterUnitId>,
        memory: MemEff,
        trap: Trap,
    ) -> Footprint {
        Footprint {
            register_reads: read_idx.iter().map(|&i| operands[usize::from(i)]).collect(),
            register_writes: write_idx
                .iter()
                .map(|&i| operands[usize::from(i)])
                .collect(),
            writes_nzcv: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: read_idx,
                external_operand_writes: write_idx,
                implicit_unit_uses,
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: Vec::new(),
                memory,
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        }
    }

    pub fn encode_aarch64_selected_frame_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
        alignment_bytes: u16,
    ) -> Result<Encoded, Error> {
        let store = matches!(kind, Kind::Store64 { .. });
        let family = if store {
            Fam::Store64
        } else {
            Fam::FrameAddress
        };
        if matches!(
            kind,
            Kind::Store64 {
                slot: FrameStorageSlotId::ActivationBase | FrameStorageSlotId::Incoming { .. },
                ..
            }
        ) || matches!(
            kind,
            Kind::FrameAddress {
                slot: FrameStorageSlotId::ActivationBase,
                byte_offset
            } if byte_offset != 0 || displacement != 0 || alignment_bytes != 1
        ) || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || !matches!(kind, Kind::Store64 { .. } | Kind::FrameAddress { .. })
            || alternative != (MachineAlternativeKey { family, variant: 0 })
            || (store && !displacement.is_multiple_of(8))
        {
            return Err(Error::EncodedFormMismatch);
        }
        let regs = resolve_registers(physical, operands)?;
        let (reg, scratch) = match regs.as_slice() {
            [v, s] if store && v != s => (*v, Some(*s)),
            [r] if !store => (*r, None),
            _ => return Err(Error::EncodedFormMismatch),
        };
        let words = if alignment_bytes != 1 {
            let (Kind::FrameAddress { byte_offset, .. } | Kind::Store64 { byte_offset, .. }) = kind
            else {
                return Err(Error::EncodedFormMismatch);
            };
            if scratch.is_some() && !byte_offset.is_multiple_of(8) {
                return Err(Error::EncodedFormMismatch);
            }
            let addr = scratch.unwrap_or(reg);
            let mut w =
                aligned_frame_address_words(addr, displacement, alignment_bytes, byte_offset)
                    .ok_or(Error::EncodedFormMismatch)?;
            if scratch.is_some() {
                w.push(enc::ldst_imm(0xf900_0000, reg, addr, 0));
            }
            w
        } else if let Some(scratch) = scratch {
            let pages = u64::from(displacement >> 12);
            let mut w = if pages == 0 {
                vec![ADD_IMMEDIATE | (1 << 22) | (31 << 5) | u32::from(scratch)]
            } else {
                let mut w = Vec::new();
                append_page_steps(&mut w, ADD_IMMEDIATE, scratch, 31, pages);
                w
            };
            w.push(enc::ldst_imm(
                0xf900_0000,
                reg,
                scratch,
                (displacement & 4095) / 8,
            ));
            w
        } else {
            address_words(reg, 31, u64::from(displacement))
        };
        let sp = physical.model().view_named("sp").expect("canonical sp");
        let (reads, writes, mem, trap) = if store {
            (
                vec![0],
                vec![1],
                MemEff::WriteFrameStorageV1 {
                    stack_pointer: sp.id,
                    byte_count: 8,
                },
                Trap::MayArchitecturalFaultV1,
            )
        } else {
            (vec![], vec![0], MemEff::NoneV1, Trap::NeverV1)
        };
        Ok(Encoded {
            bytes: enc::to_bytes(&words),
            footprint: mem_footprint(operands, reads, writes, sp.units.clone(), mem, trap),
        })
    }

    pub fn validate_aarch64_selected_frame_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
        alignment_bytes: u16,
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let expected = encode_aarch64_selected_frame_form(
            physical,
            kind,
            alternative,
            operands,
            displacement,
            alignment_bytes,
        )?;
        enc::check_bytes(bytes, &expected.bytes)?;
        Ok(expected)
    }

    fn accessor_scalar_ordinary(
        kind: Kind,
        alternative: MachineAlternativeKey,
    ) -> Result<(Kind, MachineAlternativeKey), Error> {
        let Kind::AccessorScalarAccess {
            form, byte_offset, ..
        } = kind
        else {
            return Ok((kind, alternative));
        };
        if alternative.family != Fam::AccessorScalarAccess(form) || alternative.variant != 0 {
            return Err(Error::EncodedFormMismatch);
        }
        let ordinary = form
            .ordinary_kind(byte_offset)
            .ok_or(Error::EncodedFormMismatch)?;
        let semantic = form.ordinary_semantic().ok_or(Error::EncodedFormMismatch)?;
        Ok((
            ordinary,
            MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            },
        ))
    }

    pub fn encode_aarch64_selected_memory_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<Encoded, Error> {
        let (kind, alternative) = accessor_scalar_ordinary(kind, alternative)?;
        if matches!(
            kind,
            Kind::SaveFloatingControl { .. } | Kind::RestoreFloatingControl { .. }
        ) {
            return super::floating_control::encode_aarch64_selected_floating_control_form(
                physical,
                kind,
                alternative,
                operands,
                displacement,
            );
        }
        let disp = u32::try_from(displacement).map_err(|_| Error::EncodedFormMismatch)?;
        match kind {
            Kind::Store64 { .. } | Kind::FrameAddress { .. } => {
                encode_aarch64_selected_frame_form(physical, kind, alternative, operands, disp, 1)
            }
            Kind::Load8Indexed => {
                if disp != 0 {
                    return Err(Error::EncodedFormMismatch);
                }
                let [base, idx, dst] =
                    request_registers::<3>(physical, alternative, Fam::Load8Indexed, operands, 3)?;
                Ok(Encoded {
                    bytes: enc::rrr(0x3860_6800, dst, base, idx).to_le_bytes().to_vec(),
                    footprint: mem_footprint(
                        operands,
                        vec![0, 1],
                        vec![2],
                        vec![],
                        MemEff::ReadIndexedPointerV1 {
                            pointer_operand: 0,
                            index_operand: 1,
                            byte_count: 1,
                        },
                        Trap::MayArchitecturalFaultV1,
                    ),
                })
            }
            Kind::LoadPacked { byte_offset, width } | Kind::StorePacked { byte_offset, width } => {
                let load = matches!(kind, Kind::LoadPacked { .. });
                let family = if load {
                    match width {
                        PackedByteWidth::Three => Fam::LoadPacked3,
                        PackedByteWidth::Five => Fam::LoadPacked5,
                        PackedByteWidth::Six => Fam::LoadPacked6,
                        PackedByteWidth::Seven => Fam::LoadPacked7,
                    }
                } else {
                    Fam::StorePacked
                };
                let w = width.byte_size();
                let [base, val, scratch] =
                    request_registers::<3>(physical, alternative, family, operands, 2)?;
                if (load && base == val)
                    || byte_offset != disp
                    || disp
                        .checked_add(u32::from(w) - 1)
                        .is_none_or(|last| last > 4095)
                {
                    return Err(Error::EncodedFormMismatch);
                }
                let mut words = Vec::with_capacity(usize::from(w) * 2);
                if !load {
                    words.push(enc::mov(scratch, val));
                }
                for byte in 0..u32::from(w) {
                    let off = disp + byte;
                    if load {
                        let dst = if byte == 0 { val } else { scratch };
                        words.push(enc::ldst_imm(0x3940_0000, dst, base, off));
                        if byte != 0 {
                            words.push(
                                0xaa00_0000
                                    | (u32::from(scratch) << 16)
                                    | ((byte * 8) << 10)
                                    | (u32::from(val) << 5)
                                    | u32::from(val),
                            );
                        }
                    } else {
                        words.push(enc::ldst_imm(0x3900_0000, scratch, base, off));
                        if byte + 1 != u32::from(w) {
                            words.push(enc::ubfm(true, scratch, scratch, 8, 63));
                        }
                    }
                }
                let (reads, writes, mem) = if load {
                    (
                        vec![0],
                        vec![1, 2],
                        MemEff::ReadPointerV1 {
                            pointer_operand: 0,
                            byte_count: u16::from(w),
                        },
                    )
                } else {
                    (
                        vec![0, 1],
                        vec![2],
                        MemEff::WritePointerV1 { pointer_operand: 0 },
                    )
                };
                Ok(Encoded {
                    bytes: enc::to_bytes(&words),
                    footprint: mem_footprint(
                        operands,
                        reads,
                        writes,
                        vec![],
                        mem,
                        Trap::MayArchitecturalFaultV1,
                    ),
                })
            }
            Kind::Store {
                byte_offset,
                byte_size,
            } => {
                let opcode = match byte_size {
                    1 => 0x3900_0000_u32,
                    2 => 0x7900_0000,
                    4 => 0xb900_0000,
                    8 => 0xf900_0000,
                    _ => return Err(Error::EncodedFormMismatch),
                };
                let scale = u32::from(byte_size);
                let [base, val] =
                    request_registers::<2>(physical, alternative, Fam::Store, operands, 2)?;
                if byte_offset != disp || !disp.is_multiple_of(scale) || disp / scale > 4095 {
                    return Err(Error::EncodedFormMismatch);
                }
                Ok(Encoded {
                    bytes: enc::ldst_imm(opcode, val, base, disp / scale)
                        .to_le_bytes()
                        .to_vec(),
                    footprint: mem_footprint(
                        operands,
                        vec![0, 1],
                        vec![],
                        vec![],
                        MemEff::WritePointerV1 { pointer_operand: 0 },
                        Trap::MayArchitecturalFaultV1,
                    ),
                })
            }
            Kind::AddressOffset { byte_offset } => {
                let [base, dst] =
                    request_registers::<2>(physical, alternative, Fam::AddressOffset, operands, 2)?;
                if byte_offset != disp {
                    return Err(Error::EncodedFormMismatch);
                }
                Ok(Encoded {
                    bytes: enc::to_bytes(&address_words(dst, base, u64::from(disp))),
                    footprint: mem_footprint(
                        operands,
                        vec![0],
                        vec![1],
                        vec![],
                        MemEff::NoneV1,
                        Trap::NeverV1,
                    ),
                })
            }
            Kind::Load8 { byte_offset }
            | Kind::Load16 { byte_offset }
            | Kind::Load32 { byte_offset }
            | Kind::Load64 { byte_offset } => {
                let (family, width, opcode) = match kind {
                    Kind::Load8 { .. } => (Fam::Load8, 1_u32, 0x3940_0000_u32),
                    Kind::Load16 { .. } => (Fam::Load16, 2, 0x7940_0000),
                    Kind::Load32 { .. } => (Fam::Load32, 4, 0xb940_0000),
                    Kind::Load64 { .. } => (Fam::Load64, 8, 0xf940_0000),
                    _ => unreachable!(),
                };
                if physical.identity()
                    != crate::canonical_aarch64_physical_register_model_identity()
                    || byte_offset != disp
                    || alternative != (MachineAlternativeKey { family, variant: 0 })
                    || !disp.is_multiple_of(width)
                    || disp / width > 4095
                {
                    return Err(Error::EncodedFormMismatch);
                }
                let [base, dst]: [u8; 2] = resolve_registers(physical, operands)?
                    .try_into()
                    .map_err(|_| Error::EncodedFormMismatch)?;
                Ok(Encoded {
                    bytes: enc::ldst_imm(opcode, dst, base, disp / width)
                        .to_le_bytes()
                        .to_vec(),
                    footprint: mem_footprint(
                        operands,
                        vec![0],
                        vec![1],
                        vec![],
                        MemEff::ReadPointerV1 {
                            pointer_operand: 0,
                            byte_count: width as u16,
                        },
                        Trap::MayArchitecturalFaultV1,
                    ),
                })
            }
            _ => Err(Error::AlternativeMismatch),
        }
    }

    pub fn validate_aarch64_selected_memory_form(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let (norm_kind, norm_alt) = accessor_scalar_ordinary(kind, alternative)?;
        if matches!(
            norm_kind,
            Kind::SaveFloatingControl { .. } | Kind::RestoreFloatingControl { .. }
        ) {
            return super::floating_control::validate_aarch64_selected_floating_control_form(
                physical,
                norm_kind,
                norm_alt,
                operands,
                displacement,
                bytes,
            );
        }
        let expected = encode_aarch64_selected_memory_form(
            physical,
            kind,
            alternative,
            operands,
            displacement,
        )?;
        enc::check_bytes(bytes, &expected.bytes)?;
        Ok(expected)
    }
}

pub(crate) mod hosted_exit_process {
    use super::selected_forms::resolve_registers;
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded, enc,
    };
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily as Fam, MachineAlternativeKey, SelectedInstructionKind as Kind,
    };
    use target::NativeTarget;
    pub use target_aarch64::inspection::decode_aarch64_selected_hosted_exit_process_i32;

    pub fn encode_aarch64_selected_hosted_exit_process_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<Encoded, Error> {
        if ![NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target)
            || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || kind != Kind::HostedExitProcessI32
            || alternative
                != (MachineAlternativeKey {
                    family: Fam::HostedExitProcessI32,
                    variant: 0,
                })
        {
            return Err(Error::EncodedFormMismatch);
        }
        let regs = resolve_registers(physical, operands)?;
        let [reg] = regs.as_slice() else {
            return Err(Error::EncodedFormMismatch);
        };
        let darwin = target == NativeTarget::macos_arm64();
        let words = [
            0x2a00_03e0 | (u32::from(*reg) << 16),
            if darwin { 0xd280_0030 } else { 0xd280_0bc8 },
            if darwin { enc::SVC_80 } else { enc::SVC_0 },
            enc::BRK_0,
        ];
        Ok(Encoded {
            bytes: enc::to_bytes(&words),
            footprint: Footprint {
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                writes_nzcv: true,
                encoded: target_aarch64::form_effects::hosted_exit_process::effects(target)
                    .map_err(|_| Error::EncodedFormMismatch)?,
            },
        })
    }

    pub fn validate_aarch64_selected_hosted_exit_process_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let expected = encode_aarch64_selected_hosted_exit_process_form(
            target,
            physical,
            kind,
            alternative,
            operands,
        )?;
        if bytes != expected.bytes.as_slice() {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(expected)
    }
}

pub(crate) mod hosted_read_byte {
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded,
    };
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeFamily as Fam, MachineAlternativeKey,
        SelectedInstructionKind as Kind,
    };
    use target::NativeTarget;
    pub(crate) use target_aarch64::form_effects::hosted_read_byte::effects;
    pub use target_aarch64::inspection::decode_aarch64_selected_hosted_read_byte;

    pub fn encode_aarch64_selected_hosted_read_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<Encoded, Error> {
        if ![NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target)
            || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || !matches!(
                kind,
                Kind::HostedReadByte {
                    slot: LocalStorageSlotId::Structural { .. }
                }
            )
            || alternative
                != (MachineAlternativeKey {
                    family: Fam::HostedReadByte,
                    variant: 0,
                })
            || !operands.is_empty()
            || displacement < 0
            || displacement % 4 != 0
            || displacement
                .checked_add(4)
                .is_none_or(|p| u32::try_from(p).is_err())
        {
            return Err(Error::EncodedFormMismatch);
        }
        let disp = u32::try_from(displacement).map_err(|_| Error::EncodedFormMismatch)?;
        let bytes = if target == NativeTarget::macos_arm64() {
            crate::encode_macos_read_byte_to_stack(disp, disp + 4)
        } else {
            crate::encode_linux_read_byte_to_stack(disp, disp + 4)
        }
        .map_err(|_| Error::EncodedFormMismatch)?;
        Ok(Encoded {
            bytes,
            footprint: Footprint {
                register_reads: Vec::new(),
                register_writes: Vec::new(),
                writes_nzcv: true,
                encoded: effects(target),
            },
        })
    }

    pub fn validate_aarch64_selected_hosted_read_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let expected = encode_aarch64_selected_hosted_read_byte_form(
            target,
            physical,
            kind,
            alternative,
            operands,
            displacement,
        )?;
        if bytes != expected.bytes.as_slice() {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(expected)
    }
}

pub(crate) mod hosted_write_byte {
    use super::selected_forms::resolve_registers;
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded, enc,
    };
    use crate::immediate_offset::address_words;
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeFamily as Fam, MachineAlternativeKey,
        SelectedInstructionKind as Kind,
    };
    use target::NativeTarget;
    pub(crate) use target_aarch64::form_effects::hosted_write_byte::effects;
    pub use target_aarch64::inspection::{
        aarch64_hosted_write_byte_i32_byte_count, decode_aarch64_selected_hosted_write_byte_i32,
    };

    pub fn encode_aarch64_selected_hosted_write_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
    ) -> Result<Encoded, Error> {
        let Ok(disp) = u32::try_from(displacement) else {
            return Err(Error::EncodedFormMismatch);
        };
        if ![NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target)
            || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
            || !matches!(
                kind,
                Kind::HostedWriteByteI32 {
                    slot: LocalStorageSlotId::Boundary { .. }
                }
            )
            || alternative
                != (MachineAlternativeKey {
                    family: Fam::HostedWriteByteI32,
                    variant: 0,
                })
        {
            return Err(Error::EncodedFormMismatch);
        }
        let regs = resolve_registers(physical, operands)?;
        let [reg] = regs.as_slice() else {
            return Err(Error::EncodedFormMismatch);
        };
        let mut words = if disp <= 4095 {
            vec![
                enc::ldst_imm(0x3900_0000, *reg, 31, disp),
                enc::movz(0, 1, 0),
                enc::rri12(0x9100_0000, 1, 31, disp as u16),
                enc::movz(2, 1, 0),
            ]
        } else {
            let mut w = vec![0x2a00_03e2 | (u32::from(*reg) << 16)];
            w.extend(address_words(1, 31, u64::from(disp)));
            w.extend([0x3900_0022, enc::movz(0, 1, 0), enc::movz(2, 1, 0)]);
            w
        };
        if target == NativeTarget::macos_arm64() {
            words.extend([
                0xd280_0090,
                enc::SVC_80,
                0x5400_0062,
                0xf100_041f,
                0x5400_0040,
                enc::BRK_0,
            ]);
        } else {
            words.extend([
                0xd280_0808,
                enc::SVC_0,
                0xf100_001f,
                0x5400_004c,
                enc::BRK_0,
            ]);
        }
        Ok(Encoded {
            bytes: enc::to_bytes(&words),
            footprint: Footprint {
                register_reads: operands.to_vec(),
                register_writes: Vec::new(),
                writes_nzcv: true,
                encoded: effects(target),
            },
        })
    }

    pub fn validate_aarch64_selected_hosted_write_byte_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: i64,
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let expected = encode_aarch64_selected_hosted_write_byte_form(
            target,
            physical,
            kind,
            alternative,
            operands,
            displacement,
        )?;
        if bytes != expected.bytes.as_slice() {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(expected)
    }
}

pub(crate) mod system_call {
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded, enc,
    };
    use register_model::{
        RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel, views_with_access,
    };
    use selected_instructions::{MachineAlternativeKey, SelectedInstructionKind};
    use target::NativeTarget;

    pub fn encode_aarch64_selected_system_call_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        number: i64,
    ) -> Result<Encoded, Error> {
        let n = number as u64;
        let words = [
            enc::movz(8, n as u16, 0),
            enc::movk(8, (n >> 16) as u16, 1),
            enc::movk(8, (n >> 32) as u16, 2),
            enc::movk(8, (n >> 48) as u16, 3),
            enc::SVC_0,
        ];
        validate_aarch64_selected_system_call_form(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            operand_accesses,
            number,
            &enc::to_bytes(&words),
        )
    }

    pub fn validate_aarch64_selected_system_call_form(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        number: i64,
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let effects = target_aarch64::inspection::selected_system_call_effects(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            operand_accesses,
            number,
            bytes,
        )
        .ok_or(Error::EncodedFormMismatch)?;
        Ok(Encoded {
            bytes: bytes.to_vec(),
            footprint: Footprint {
                register_reads: views_with_access(
                    operand_views,
                    operand_accesses,
                    RegisterOperandAccess::Use,
                ),
                register_writes: views_with_access(
                    operand_views,
                    operand_accesses,
                    RegisterOperandAccess::Def,
                ),
                writes_nzcv: true,
                encoded: effects,
            },
        })
    }
}

pub(crate) mod table_call {
    use super::{
        Aarch64SelectedFormEncodingError as Error, Aarch64SelectedFormFootprint as Footprint,
        ValidatedAarch64SelectedFormEncoding as Encoded, enc,
    };
    use register_model::{RegisterOperandAccess, RegisterViewId, views_with_access};
    use selected_instructions::{MachineAlternativeKey, SelectedNormalizedForeignCall};
    use target_aarch64::form_effects::table_call::table_address;

    pub fn encode_aarch64_selected_table_call_form(
        record: &SelectedNormalizedForeignCall,
        alternative: MachineAlternativeKey,
        views: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
    ) -> Result<Encoded, Error> {
        let (base, offset) = table_address(record).ok_or(Error::EncodedFormMismatch)?;
        let words = [
            enc::movz(17, offset as u16, 0),
            enc::movk(17, (offset >> 16) as u16, 1),
            enc::rrr(0x8b00_0000, 17, base, 17),
            0xf940_0231,
            0xd63f_0220,
        ];
        validate_aarch64_selected_table_call_form(
            record,
            alternative,
            views,
            accesses,
            &enc::to_bytes(&words),
        )
    }

    pub fn validate_aarch64_selected_table_call_form(
        record: &SelectedNormalizedForeignCall,
        alternative: MachineAlternativeKey,
        views: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
        bytes: &[u8],
    ) -> Result<Encoded, Error> {
        let effects = target_aarch64::inspection::selected_table_call_effects(
            record,
            alternative,
            views,
            accesses,
            bytes,
        )
        .ok_or(Error::EncodedFormMismatch)?;
        Ok(Encoded {
            bytes: bytes.to_vec(),
            footprint: Footprint {
                register_reads: views_with_access(views, accesses, RegisterOperandAccess::Use),
                register_writes: views_with_access(views, accesses, RegisterOperandAccess::Def),
                writes_nzcv: true,
                encoded: effects,
            },
        })
    }
}

mod normalized_foreign_call {
    mod callback_field;

    use super::enc;
    use crate::codec_bytes::push_encoded;
    use crate::machine_effects::aarch64_selected_abi;
    use normalized_foreign_call_plan::{Fixup, Template};
    use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
        SelectedNormalizedForeignCall,
    };
    use semantic_vocabulary::BoundaryMachineId;
    use target::NativeTarget;

    const PATCH_WIDTH: u8 = 4;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64NormalizedForeignCallFixupKind {
        BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64NormalizedForeignCallFixupState {
        UnresolvedImportFieldV1,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Aarch64CallbackMaterializationDestination {
        Register(calling_conventions::MachineRegister),
        RecordField {
            root_register: calling_conventions::MachineRegister,
            placements: calling_conventions::PrivateCallbackPlacements,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Aarch64CallbackAddressMaterialization {
        pub destination: Aarch64CallbackMaterializationDestination,
        pub materialization_byte_offset: u16,
        pub materialization_byte_count: u16,
        pub page_relocation_byte_offset: u16,
        pub page_offset_relocation_byte_offset: u16,
    }

    pub type Aarch64NormalizedForeignCallFixup = Fixup<
        Aarch64NormalizedForeignCallFixupKind,
        Aarch64NormalizedForeignCallFixupState,
        Aarch64CallbackAddressMaterialization,
    >;
    pub type ValidatedAarch64SelectedNormalizedForeignCallTemplate =
        Template<Aarch64NormalizedForeignCallFixup>;
    pub type Aarch64NormalizedForeignCallTemplateError = normalized_foreign_call_plan::Error;

    pub fn encode_aarch64_selected_normalized_foreign_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        effects: &MachineEncodedEffects,
        callback_destinations: &[Aarch64CallbackMaterializationDestination],
        record: &SelectedNormalizedForeignCall,
    ) -> Result<
        ValidatedAarch64SelectedNormalizedForeignCallTemplate,
        Aarch64NormalizedForeignCallTemplateError,
    > {
        let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = kind else {
            return Err(Aarch64NormalizedForeignCallTemplateError::InstructionKindMismatch);
        };
        let bytes = canonical_call_bytes(callback_destinations)
            .ok_or(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)?;
        let fixup = callback_fixup(boundary, ordinal, callback_destinations)
            .ok_or(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)?;
        validate_aarch64_selected_normalized_foreign_call_template(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            operand_accesses,
            effects,
            &bytes,
            fixup,
            callback_destinations,
            record,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_aarch64_selected_normalized_foreign_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        operand_accesses: &[RegisterOperandAccess],
        effects: &MachineEncodedEffects,
        bytes: &[u8],
        fixup: Aarch64NormalizedForeignCallFixup,
        callback_destinations: &[Aarch64CallbackMaterializationDestination],
        record: &SelectedNormalizedForeignCall,
    ) -> Result<
        ValidatedAarch64SelectedNormalizedForeignCallTemplate,
        Aarch64NormalizedForeignCallTemplateError,
    > {
        let (boundary, ordinal) = normalized_foreign_call_plan::admit(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            operand_accesses,
            effects,
            record,
            |native| aarch64_selected_abi(native).is_ok(),
            crate::canonical_aarch64_physical_register_model_identity,
            crate::machine_effects::normalized_foreign_constraint,
            crate::aarch64_normalized_foreign_call_declaration,
        )?;
        let expected_dests = record
            .call
            .callbacks
            .iter()
            .map(|cb| {
                let [calling_conventions::ValueLocation::Register { register, .. }] =
                    cb.application.placement.locations.as_slice()
                else {
                    return None;
                };
                match cb.destination {
                    calling_conventions::NativePlace::Parameter(_) => Some(
                        Aarch64CallbackMaterializationDestination::Register(*register),
                    ),
                    calling_conventions::NativePlace::Field { .. } => {
                        Some(Aarch64CallbackMaterializationDestination::RecordField {
                            root_register: *register,
                            placements: cb.field_placements.clone()?,
                        })
                    }
                }
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
        if expected_dests != callback_destinations {
            return Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch);
        }
        let expected_bytes = canonical_call_bytes(callback_destinations)
            .ok_or(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate)?;
        if bytes != expected_bytes.as_slice() {
            return Err(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate);
        }
        let expected_fixup = callback_fixup(boundary, ordinal, callback_destinations)
            .ok_or(Aarch64NormalizedForeignCallTemplateError::FixupMismatch)?;
        if fixup != expected_fixup {
            return Err(Aarch64NormalizedForeignCallTemplateError::FixupMismatch);
        }
        Ok(Template::new(
            kind,
            alternative,
            operand_views,
            effects,
            bytes,
            fixup,
        ))
    }

    fn canonical_call_bytes(
        destinations: &[Aarch64CallbackMaterializationDestination],
    ) -> Option<Vec<u8>> {
        let mut bytes = Vec::new();
        for d in destinations {
            bytes.extend(callback_group_bytes(d)?);
        }
        push_encoded(&mut bytes, enc::BL_ZERO.to_le_bytes());
        Some(bytes)
    }

    pub(super) fn callback_group_bytes(
        destination: &Aarch64CallbackMaterializationDestination,
    ) -> Option<Vec<u8>> {
        let reg = match destination {
            Aarch64CallbackMaterializationDestination::Register(r) => *r,
            Aarch64CallbackMaterializationDestination::RecordField { .. } => {
                calling_conventions::MachineRegister::Aarch64X(16)
            }
        };
        let calling_conventions::MachineRegister::Aarch64X(idx) = reg else {
            return None;
        };
        if idx > 30 {
            return None;
        }
        let mut bytes = Vec::new();
        push_encoded(&mut bytes, (0x9000_0000_u32 | u32::from(idx)).to_le_bytes());
        push_encoded(
            &mut bytes,
            enc::rri12(0x9100_0000, idx, idx, 0).to_le_bytes(),
        );
        if let Aarch64CallbackMaterializationDestination::RecordField {
            root_register,
            placements,
        } = destination
        {
            let calling_conventions::MachineRegister::Aarch64X(root) = *root_register else {
                return None;
            };
            if placements.as_slice().is_empty() {
                return None;
            }
            for &p in placements.as_slice() {
                callback_field::append_fragment(&mut bytes, 16, root, 15, 17, p)?;
            }
        }
        Some(bytes)
    }

    fn callback_fixup(
        boundary: BoundaryMachineId,
        ordinal: u32,
        destinations: &[Aarch64CallbackMaterializationDestination],
    ) -> Option<Aarch64NormalizedForeignCallFixup> {
        let mut mats = Vec::with_capacity(destinations.len());
        let mut offset = 0_u16;
        for d in destinations {
            let count = u16::try_from(callback_group_bytes(d)?.len()).ok()?;
            mats.push(Aarch64CallbackAddressMaterialization {
                destination: d.clone(),
                materialization_byte_offset: offset,
                materialization_byte_count: count,
                page_relocation_byte_offset: offset,
                page_offset_relocation_byte_offset: offset + 4,
            });
            offset = offset.checked_add(count)?;
        }
        Some(Aarch64NormalizedForeignCallFixup {
            kind: Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
            state: Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
            boundary,
            ordinal,
            opcode_byte_offset: offset,
            patch_byte_offset: offset,
            reference_byte_offset: offset,
            patch_byte_width: PATCH_WIDTH,
            callbacks: mats,
        })
    }
}

mod scalar_call {
    use super::enc;
    use crate::machine_effects::{Aarch64SelectedAbi, aarch64_selected_abi};
    use crate::{aarch64_aapcs64_register_call_keys, aarch64_register_constraint_catalog_for};
    use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
    use selected_instructions::{
        MachineAlternativeFamily as Fam, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, SelectedInstructionKind as Kind,
    };
    use semantic_vocabulary::MachineId;
    use target::NativeTarget;

    const TEMPLATE_LEN: usize = 4;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64ScalarCallFixupKind {
        SignedImmediate26WordsFromInstructionToInternalMachineV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64ScalarCallFixupState {
        UnresolvedZeroImmediateV1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Aarch64ScalarCallFixup {
        pub kind: Aarch64ScalarCallFixupKind,
        pub state: Aarch64ScalarCallFixupState,
        pub callee: MachineId,
        pub opcode_byte_offset: u16,
        pub patch_byte_offset: u16,
        pub reference_byte_offset: u16,
        pub patch_byte_width: u8,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedAarch64SelectedScalarCallTemplate {
        kind: Kind,
        alternative: MachineAlternativeKey,
        operand_views: Vec<RegisterViewId>,
        effects: MachineEncodedEffects,
        bytes: [u8; TEMPLATE_LEN],
        fixup: Aarch64ScalarCallFixup,
    }

    impl ValidatedAarch64SelectedScalarCallTemplate {
        pub const fn kind(&self) -> Kind {
            self.kind
        }
        pub const fn alternative(&self) -> MachineAlternativeKey {
            self.alternative
        }
        pub fn operand_views(&self) -> &[RegisterViewId] {
            &self.operand_views
        }
        pub const fn effects(&self) -> &MachineEncodedEffects {
            &self.effects
        }
        pub const fn bytes(&self) -> &[u8; TEMPLATE_LEN] {
            &self.bytes
        }
        pub const fn fixup(&self) -> Aarch64ScalarCallFixup {
            self.fixup
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64ScalarCallTemplateError {
        UnsupportedTarget,
        NonCanonicalPhysicalModel,
        InstructionKindMismatch,
        AlternativeMismatch,
        OperandViewMismatch,
        EffectMismatch,
        MalformedTemplate,
        FixupMismatch,
    }

    impl std::fmt::Display for Aarch64ScalarCallTemplateError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "invalid AArch64 scalar-call template: {self:?}")
        }
    }

    impl std::error::Error for Aarch64ScalarCallTemplateError {}

    pub fn encode_aarch64_selected_scalar_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
    ) -> Result<ValidatedAarch64SelectedScalarCallTemplate, Aarch64ScalarCallTemplateError> {
        let callee = kind
            .machine_callee()
            .ok_or(Aarch64ScalarCallTemplateError::InstructionKindMismatch)?;
        validate_aarch64_selected_scalar_call_template(
            target,
            physical,
            kind,
            alternative,
            operand_views,
            effects,
            &crate::ORDINARY_DIRECT_CALL_BYTES,
            canonical_fixup(callee),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_aarch64_selected_scalar_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
        bytes: &[u8],
        fixup: Aarch64ScalarCallFixup,
    ) -> Result<ValidatedAarch64SelectedScalarCallTemplate, Aarch64ScalarCallTemplateError> {
        let abi = aarch64_selected_abi(target)
            .map_err(|_| Aarch64ScalarCallTemplateError::UnsupportedTarget)?;
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Aarch64ScalarCallTemplateError::NonCanonicalPhysicalModel);
        }
        let callee = kind
            .machine_callee()
            .ok_or(Aarch64ScalarCallTemplateError::InstructionKindMismatch)?;
        if alternative != kind.call_alternative_key() {
            return Err(Aarch64ScalarCallTemplateError::AlternativeMismatch);
        }
        let (expected_views, expected) = if matches!(kind, Kind::InvokeNoNormal { .. }) {
            expected_no_normal_mechanics(abi, physical, operand_views, effects)?
        } else {
            expected_call_mechanics(abi, physical, kind, operand_views)?
        };
        if effects != &expected {
            return Err(Aarch64ScalarCallTemplateError::EffectMismatch);
        }
        let bytes: [u8; TEMPLATE_LEN] = bytes
            .try_into()
            .map_err(|_| Aarch64ScalarCallTemplateError::MalformedTemplate)?;
        if u32::from_le_bytes(bytes) != enc::BL_ZERO {
            return Err(Aarch64ScalarCallTemplateError::MalformedTemplate);
        }
        if fixup != canonical_fixup(callee) {
            return Err(Aarch64ScalarCallTemplateError::FixupMismatch);
        }
        Ok(ValidatedAarch64SelectedScalarCallTemplate {
            kind,
            alternative,
            operand_views: expected_views,
            effects: effects.clone(),
            bytes,
            fixup,
        })
    }

    pub(super) fn canonical_fixup(callee: MachineId) -> Aarch64ScalarCallFixup {
        Aarch64ScalarCallFixup {
            kind:
                Aarch64ScalarCallFixupKind::SignedImmediate26WordsFromInstructionToInternalMachineV1,
            state: Aarch64ScalarCallFixupState::UnresolvedZeroImmediateV1,
            callee,
            opcode_byte_offset: 0,
            patch_byte_offset: 0,
            reference_byte_offset: 0,
            patch_byte_width: 4,
        }
    }

    fn effects_from_row(
        row: &register_model::RegisterInstructionConstraint,
    ) -> MachineEncodedEffects {
        MachineEncodedEffects {
            external_operand_reads: row.operand_reads(),
            external_operand_writes: row.operand_writes(),
            implicit_unit_uses: row.implicit_uses.clone(),
            implicit_unit_defs: row.implicit_defs.clone(),
            implicit_unit_clobbers: row.clobbers.clone(),
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::DirectRelativeCallV1,
        }
    }

    fn row_matches_views(
        row: &register_model::RegisterInstructionConstraint,
        views: &[RegisterViewId],
    ) -> bool {
        row.operands.len() == views.len()
            && row
                .operands
                .iter()
                .zip(views)
                .all(|(op, view)| op.fixed_view == Some(*view))
    }

    fn expected_no_normal_mechanics(
        abi: Aarch64SelectedAbi,
        physical: &ValidatedPhysicalRegisterModel,
        views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
    ) -> Result<(Vec<RegisterViewId>, MachineEncodedEffects), Aarch64ScalarCallTemplateError> {
        let darwin = abi == Aarch64SelectedAbi::Darwin;
        let mut keys: Vec<_> = if darwin {
            crate::aarch64_darwin_register_call_keys()
                .into_iter()
                .chain(crate::aarch64_darwin_register_unit_call_keys())
                .chain(crate::aarch64_darwin_mixed_unit_call_keys())
                .collect()
        } else {
            aarch64_aapcs64_register_call_keys()
                .into_iter()
                .chain(crate::aarch64_aapcs64_register_unit_call_keys())
                .chain(crate::aarch64_aapcs64_mixed_unit_call_keys())
                .collect()
        };
        keys.extend(crate::aarch64_float_scalar_call_keys(darwin));
        keys.extend(crate::aarch64_register_aggregate_call_keys(darwin));
        keys.extend(crate::aarch64_mixed_aggregate_call_keys(darwin));
        keys.extend(crate::aarch64_indirect_aggregate_call_keys(darwin));
        let catalog = aarch64_register_constraint_catalog_for(physical);
        for row in &catalog.constraints {
            if keys.contains(&row.key) && row_matches_views(row, views) {
                let expected = effects_from_row(row);
                if &expected == effects {
                    return Ok((views.to_vec(), expected));
                }
            }
        }
        Err(Aarch64ScalarCallTemplateError::EffectMismatch)
    }

    fn expected_call_mechanics(
        abi: Aarch64SelectedAbi,
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        views: &[RegisterViewId],
    ) -> Result<(Vec<RegisterViewId>, MachineEncodedEffects), Aarch64ScalarCallTemplateError> {
        let darwin = abi == Aarch64SelectedAbi::Darwin;
        let unit = matches!(kind, Kind::CallUnit { .. });
        let aggregate = matches!(kind, Kind::CallAggregate { .. });
        let float_scalar = !unit
            && !aggregate
            && views.iter().any(|id| {
                physical
                    .model()
                    .views
                    .iter()
                    .any(|v| v.id == *id && v.name.starts_with('d'))
            });
        let catalog = aarch64_register_constraint_catalog_for(physical);
        if unit || aggregate || float_scalar {
            let keys: Vec<_> = if aggregate {
                crate::aarch64_register_aggregate_call_keys(darwin)
                    .into_iter()
                    .chain(crate::aarch64_mixed_aggregate_call_keys(darwin))
                    .chain(crate::aarch64_indirect_aggregate_call_keys(darwin))
                    .collect()
            } else if float_scalar {
                crate::aarch64_float_scalar_call_keys(darwin)
            } else if darwin {
                crate::aarch64_darwin_register_unit_call_keys()
                    .into_iter()
                    .chain(crate::aarch64_darwin_mixed_unit_call_keys())
                    .collect()
            } else {
                crate::aarch64_aapcs64_register_unit_call_keys()
                    .into_iter()
                    .chain(crate::aarch64_aapcs64_mixed_unit_call_keys())
                    .collect()
            };
            let row = catalog
                .constraints
                .iter()
                .find(|row| keys.contains(&row.key) && row_matches_views(row, views))
                .ok_or(Aarch64ScalarCallTemplateError::OperandViewMismatch)?;
            return Ok((views.to_vec(), effects_from_row(row)));
        }
        let arity = views
            .len()
            .checked_sub(1)
            .filter(|&a| a <= 8)
            .ok_or(Aarch64ScalarCallTemplateError::OperandViewMismatch)?;
        let expected_views: Vec<_> = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
            .into_iter()
            .take(arity)
            .chain(["x0"])
            .map(|name| physical.model().view_named(name).expect("ABI reg").id)
            .collect();
        if views != expected_views {
            return Err(Aarch64ScalarCallTemplateError::OperandViewMismatch);
        }
        let key = if darwin {
            crate::aarch64_darwin_register_call_keys()[arity]
        } else {
            aarch64_aapcs64_register_call_keys()[arity]
        };
        let row = catalog
            .constraints
            .iter()
            .find(|r| r.key == key)
            .expect("canonical scalar-call constraint");
        Ok((expected_views, effects_from_row(row)))
    }

    pub fn encode_aarch64_selected_dynamic_call_template(
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
    ) -> Result<(Vec<u8>, MachineEncodedEffects), Aarch64ScalarCallTemplateError> {
        let abi = aarch64_selected_abi(target)
            .map_err(|_| Aarch64ScalarCallTemplateError::UnsupportedTarget)?;
        if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
            return Err(Aarch64ScalarCallTemplateError::NonCanonicalPhysicalModel);
        }
        let Some(offset) = kind.call_target().and_then(|t| t.indirect_code_offset()) else {
            return Err(Aarch64ScalarCallTemplateError::InstructionKindMismatch);
        };
        let family = match kind {
            Kind::CallUnit { .. } => Fam::CallUnit,
            Kind::CallScalar { .. } => Fam::CallScalar,
            Kind::CallAggregate { .. } => Fam::CallAggregate,
            _ => return Err(Aarch64ScalarCallTemplateError::InstructionKindMismatch),
        };
        if alternative != (MachineAlternativeKey { family, variant: 1 }) {
            return Err(Aarch64ScalarCallTemplateError::AlternativeMismatch);
        }
        let selector = physical
            .model()
            .view_named("x16")
            .ok_or(Aarch64ScalarCallTemplateError::OperandViewMismatch)?
            .id;
        let mut ordinary = operand_views.to_vec();
        let pos = ordinary
            .iter()
            .position(|v| *v == selector)
            .ok_or(Aarch64ScalarCallTemplateError::OperandViewMismatch)?;
        ordinary.remove(pos);
        let (_, baseline) = expected_call_mechanics(abi, physical, kind, &ordinary)?;
        if pos != baseline.external_operand_reads.len() {
            return Err(Aarch64ScalarCallTemplateError::OperandViewMismatch);
        }
        let expected = selected_instructions::calls::dynamic::encoded_effects(
            &baseline,
            physical.model().architecture,
        )
        .ok_or(Aarch64ScalarCallTemplateError::EffectMismatch)?;
        if *effects != expected {
            return Err(Aarch64ScalarCallTemplateError::EffectMismatch);
        }
        let words = [
            enc::movz(17, offset as u16, 0),
            enc::movk(17, (offset >> 16) as u16, 1),
            enc::movk(17, (offset >> 32) as u16, 2),
            enc::movk(17, (offset >> 48) as u16, 3),
            0x8b11_0211,
            0xf940_0231,
            0xb500_0051,
            enc::BRK_0,
            0xd63f_0220,
        ];
        Ok((enc::to_bytes(&words), expected))
    }
}

pub use branch_forms::{
    AARCH64_CONDITIONAL_BRANCH_BYTE_REACH, Aarch64ConditionalBranchForm,
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_conditional_branch_form,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_conditional_branch_form,
};
pub(crate) use encoding_types::Aarch64SelectedFormFootprint;
pub use encoding_types::{Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding};
pub use floating_control::encode_aarch64_selected_floating_control_form;
pub use hosted_write_byte::{
    encode_aarch64_selected_hosted_write_byte_form,
    validate_aarch64_selected_hosted_write_byte_form,
};
pub use jump::{encode_aarch64_selected_jump_form, validate_aarch64_selected_jump_form};
pub use memory::{
    encode_aarch64_selected_frame_form, encode_aarch64_selected_memory_form,
    validate_aarch64_selected_frame_form, validate_aarch64_selected_memory_form,
};
pub use normalized_foreign_call::{
    Aarch64CallbackAddressMaterialization, Aarch64CallbackMaterializationDestination,
    Aarch64NormalizedForeignCallFixup, Aarch64NormalizedForeignCallFixupKind,
    Aarch64NormalizedForeignCallFixupState, Aarch64NormalizedForeignCallTemplateError,
    encode_aarch64_selected_normalized_foreign_call_template,
    validate_aarch64_selected_normalized_foreign_call_template,
};
pub use scalar_call::{
    Aarch64ScalarCallFixup, Aarch64ScalarCallFixupKind, Aarch64ScalarCallFixupState,
    Aarch64ScalarCallTemplateError, encode_aarch64_selected_dynamic_call_template,
    encode_aarch64_selected_scalar_call_template, validate_aarch64_selected_scalar_call_template,
};
pub use selected_forms::{
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET,
    AARCH64_BYTE_LITERAL_ADDRESS_PAGE_OFFSET_FIELD_OFFSET, encode_aarch64_selected_form,
    validate_aarch64_selected_form_encoding,
};
pub use table_call::{
    encode_aarch64_selected_table_call_form, validate_aarch64_selected_table_call_form,
};

fn nzcv_effects_encoding(
    physical: &register_model::ValidatedPhysicalRegisterModel,
    operands: &[register_model::RegisterViewId],
    bytes: &[u8],
    read_count: usize,
    memory: selected_instructions::MachineEncodedMemoryEffect,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let nzcv = physical
        .model()
        .view_named("nzcv")
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
        .units
        .clone();
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: operands[..read_count].to_vec(),
            register_writes: operands[read_count..].to_vec(),
            writes_nzcv: true,
            encoded: selected_instructions::MachineEncodedEffects {
                external_operand_reads: (0..read_count as u16).collect(),
                external_operand_writes: (read_count as u16..operands.len() as u16).collect(),
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: nzcv,
                memory,
                stack: selected_instructions::MachineEncodedStackEffect::UnchangedV1,
                trap: selected_instructions::MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: selected_instructions::MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aarch64_physical_register_model;
    use crate::saturating_forms::SaturatingRealization;
    use crate::test_support::{reject_every_bit_flip, rejects_every_bit_flip};
    use optimization_core::AcceptedObligationFactIdentity;
    use register_model::{
        RegisterViewId, ValidatedPhysicalRegisterModel, validate_physical_register_model,
    };
    use selected_instructions::{
        AtomicForm, AtomicOperation, AtomicWidth, ByteSequencePredicate, FloatResultPolicyForm,
        FloatResultPolicyKind, FrameStorageSlotId, IeeeFloatArithmeticForm,
        IeeeFloatArithmeticOperation, IeeeFloatFormat, IeeeFloatIntegerCarrier,
        IeeeFloatToIntegerDomain, LocalStorageSlotId, MachineAlternativeFamily as Fam,
        MachineAlternativeKey, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
        MachineSizeKnowledge, MemoryFenceKind, PackedByteWidth, RegisterOperation,
        SaturatingCarrier as C, SaturatingOperation as SatOp, SelectedCallTarget,
        SelectedImageAddressReference, SelectedInstructionKind as Kind, SystemRegister,
        TrappingForm, TrappingOperation as TrapOp,
    };
    use semantic_vocabulary::{
        IntegerSign, IntegerValue, MachineId, ObligationId, OperationId, PlaceId,
    };
    use target::NativeTarget;

    fn phys() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(aarch64_physical_register_model()).unwrap()
    }

    fn v(p: &ValidatedPhysicalRegisterModel, name: &str) -> RegisterViewId {
        p.model().view_named(name).unwrap().id
    }

    fn vs(p: &ValidatedPhysicalRegisterModel, names: &[&str]) -> Vec<RegisterViewId> {
        names.iter().map(|n| v(p, n)).collect()
    }

    fn key(family: Fam) -> MachineAlternativeKey {
        MachineAlternativeKey { family, variant: 0 }
    }

    fn le(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    fn ob() -> ObligationId {
        ObligationId::new(1).unwrap()
    }

    fn fact() -> AcceptedObligationFactIdentity {
        AcceptedObligationFactIdentity::from_bytes([3; 32])
    }

    #[test]
    fn scalar_forms_match_golden_words_and_reject_bit_flips() {
        let p = phys();
        #[rustfmt::skip]
        let cases: Vec<(Kind, Fam, &[&str], &[u32], bool)> = vec![
            (Kind::AtomicFence, Fam::AtomicFence, &[], &[0xd503_3fbf], false),
            (Kind::Crash, Fam::Crash, &[], &[0xd420_0000], false),
            (Kind::RegisterAccess { operation: RegisterOperation::MemoryFence(MemoryFenceKind::Aarch64Load) }, Fam::MemoryFence, &[], &[0xd503_3dbf], false),
            (Kind::RegisterAccess { operation: RegisterOperation::MemoryFence(MemoryFenceKind::Aarch64Store) }, Fam::MemoryFence, &[], &[0xd503_3ebf], false),
            (Kind::RegisterAccess { operation: RegisterOperation::MemoryFence(MemoryFenceKind::Aarch64Full) }, Fam::MemoryFence, &[], &[0xd503_3fbf], false),
            (Kind::RegisterAccess { operation: RegisterOperation::SystemRead(SystemRegister::SctlrEl1) }, Fam::SystemRegisterRead, &["x3"], &[0xd538_1003], false),
            (Kind::RegisterAccess { operation: RegisterOperation::SystemWrite(SystemRegister::TpidrEl1) }, Fam::SystemRegisterWrite, &["x4"], &[0xd518_d084], false),
            (Kind::ReturnUnit, Fam::ReturnUnit, &[], &[0xd65f_03c0], false),
            (Kind::ReturnScalar, Fam::ReturnScalar, &["x0"], &[0xd65f_03c0], false),
            (Kind::ReturnScalar, Fam::ReturnScalar, &["d0"], &[0xd65f_03c0], false),
            (Kind::ReturnAggregate { fragment_count: 2 }, Fam::ReturnAggregate, &["x0", "x1"], &[0xd65f_03c0], false),
            (Kind::MaterializeBooleanEqual, Fam::MaterializeBooleanEqual, &["x2"], &[0x9a9f_17e2], false),
            (Kind::MaterializeBooleanU64LessThan, Fam::MaterializeBooleanU64LessThan, &["x2"], &[0x9a9f_27e2], false),
            (Kind::MaterializeBooleanI64LessThan, Fam::MaterializeBooleanI64LessThan, &["x2"], &[0x9a9f_a7e2], false),
            (Kind::MaterializeBooleanU64LessOrEqual, Fam::MaterializeBooleanU64LessOrEqual, &["x2"], &[0x9a9f_87e2], false),
            (Kind::MaterializeBooleanI64LessOrEqual, Fam::MaterializeBooleanI64LessOrEqual, &["x2"], &[0x9a9f_c7e2], false),
            (Kind::MaterializeI64 { value: IntegerValue::Unsigned(0x1234_0000_5678_9abc) }, Fam::MaterializeI64, &["x5"], &[0xd293_5785, 0xf2aa_cf05, 0xf2e2_4685], false),
            (Kind::ImageAddress { reference: SelectedImageAddressReference::RetainedEntryStorageStart }, Fam::ImageAddress, &["x3"], &[0x9000_0003, 0x9100_0063], false),
            (Kind::CopyI64, Fam::CopyI64, &["x1", "x2"], &[0xaa01_03e2], false),
            (Kind::BitwiseNotI64, Fam::BitwiseNotI64, &["x1", "x2"], &[0xaa21_03e2], false),
            (Kind::ZeroExtendU8, Fam::ZeroExtendU8, &["x1", "x2"], &[0xd340_1c22], false),
            (Kind::ZeroExtendU16, Fam::ZeroExtendU16, &["x1", "x2"], &[0xd340_3c22], false),
            (Kind::ZeroExtendU32, Fam::ZeroExtendU32, &["x1", "x2"], &[0xd340_7c22], false),
            (Kind::SignExtendI8, Fam::SignExtendI8, &["x1", "x2"], &[0x9340_1c22], false),
            (Kind::SignExtendI16, Fam::SignExtendI16, &["x1", "x2"], &[0x9340_3c22], false),
            (Kind::SignExtendI32, Fam::SignExtendI32, &["x1", "x2"], &[0x9340_7c22], false),
            (Kind::CompareI64Zero, Fam::CompareI64Zero, &["x3"], &[0xf100_007f], true),
            (Kind::CompareI64Immediate { immediate: IntegerValue::Unsigned(42) }, Fam::CompareI64Immediate, &["x3"], &[0xf100_a87f], true),
            (Kind::CompareI64, Fam::CompareI64, &["x1", "x2"], &[0xeb02_003f], true),
            (Kind::ByteViewAddress, Fam::ByteViewAddress, &["x1", "x2", "x3"], &[0x8b02_0023], false),
            (Kind::WrappingAddI64, Fam::WrappingAddI64, &["x1", "x2", "x3"], &[0x8b02_0023], false),
            (Kind::WrappingSubtractI64, Fam::WrappingSubtractI64, &["x1", "x2", "x3"], &[0xcb02_0023], false),
            (Kind::WrappingMultiplyI64, Fam::WrappingMultiplyI64, &["x1", "x2", "x3"], &[0x9b02_7c23], false),
            (Kind::BitwiseAndI64, Fam::BitwiseAndI64, &["x1", "x2", "x3"], &[0x8a02_0023], false),
            (Kind::BitwiseOrI64, Fam::BitwiseOrI64, &["x1", "x2", "x3"], &[0xaa02_0023], false),
            (Kind::BitwiseXorI64, Fam::BitwiseXorI64, &["x1", "x2", "x3"], &[0xca02_0023], false),
            (Kind::ExactAddI64Immediate { immediate: IntegerValue::Unsigned(16), obligation: ob(), accepted_fact: fact() }, Fam::ExactAddI64Immediate, &["x1", "x2"], &[0x9100_4022], false),
            (Kind::ExactSubtractI64Immediate { immediate: IntegerValue::Unsigned(16), obligation: ob(), accepted_fact: fact() }, Fam::ExactSubtractI64Immediate, &["x1", "x2"], &[0xd100_4022], false),
            (Kind::ExactDivideU64 { obligation: ob(), accepted_fact: fact() }, Fam::ExactDivideU64, &["x9", "x10", "x11"], &[0x9aca_092b], false),
            (Kind::ExactDivideI64 { obligation: ob(), accepted_fact: fact() }, Fam::ExactDivideI64, &["x9", "x10", "x11"], &[0x9aca_0d2b], false),
            (Kind::ExactRemainderU64 { obligation: ob(), accepted_fact: fact() }, Fam::ExactRemainderU64, &["x9", "x10", "x11"], &[0x9aca_092b, 0x9b0a_a56b], false),
            (Kind::WrappingRemainderI64 { obligation: ob(), accepted_fact: fact() }, Fam::WrappingRemainderI64, &["x9", "x10", "x11"], &[0x9aca_0d2b, 0x9b0a_a56b], false),
            (Kind::WrappingShiftLeftI64, Fam::WrappingShiftLeftI64, &["x1", "x2", "x3"], &[0x9ac2_2023], false),
            (Kind::WrappingShiftRightU64, Fam::WrappingShiftRightU64, &["x1", "x2", "x3"], &[0x9ac2_2423], false),
            (Kind::WrappingShiftRightI64, Fam::WrappingShiftRightI64, &["x1", "x2", "x3"], &[0x9ac2_2823], false),
        ];
        for (kind, family, names, expected, nzcv) in cases {
            let ops = vs(&p, names);
            let k = key(family);
            let encoded = encode_aarch64_selected_form(&p, kind, k, &ops).unwrap();
            assert_eq!(encoded.bytes(), le(expected), "{kind:?}");
            assert_eq!(encoded.footprint().writes_nzcv, nzcv, "{kind:?}");
            validate_aarch64_selected_form_encoding(&p, kind, k, &ops, encoded.bytes()).unwrap();
            rejects_every_bit_flip(&p, kind, k, &ops, encoded.bytes());
        }
        assert!(
            encode_aarch64_selected_form(
                &p,
                Kind::ReturnScalar,
                key(Fam::ReturnScalar),
                &vs(&p, &["x1"])
            )
            .is_err()
        );
        assert!(
            encode_aarch64_selected_form(
                &p,
                Kind::ExactRemainderU64 {
                    obligation: ob(),
                    accepted_fact: fact()
                },
                key(Fam::ExactRemainderU64),
                &vs(&p, &["x1", "x2", "x1"])
            )
            .is_err()
        );
        assert_eq!(
            encode_aarch64_selected_form(
                &p,
                Kind::CompareI64Immediate {
                    immediate: IntegerValue::Unsigned(4096)
                },
                key(Fam::CompareI64Immediate),
                &vs(&p, &["x0"])
            ),
            Err(Aarch64SelectedFormEncodingError::ImmediateOutsideU12)
        );
        assert_eq!(
            encode_aarch64_selected_form(&p, Kind::PackedAddI16x8, key(Fam::CopyI64), &[]),
            Err(Aarch64SelectedFormEncodingError::AlternativeMismatch)
        );
        assert_eq!(
            encode_aarch64_selected_form(&p, Kind::Jump, key(Fam::Jump), &[]),
            Err(Aarch64SelectedFormEncodingError::LayoutDependentForm)
        );
    }

    #[test]
    fn saturating_forms_match_all_forty_clang_golden_vectors() {
        let p = phys();
        #[rustfmt::skip]
        const ROWS: [(SatOp, C, &[u32]); 40] = [
            (SatOp::Add, C::I8, &[0x8b02_0023, 0xb240_1be4, 0xeb04_007f, 0x9a83_c083, 0xb279_e3e4, 0xeb04_007f, 0x9a83_b083]),
            (SatOp::Add, C::I16, &[0x8b02_0023, 0xb240_3be4, 0xeb04_007f, 0x9a83_c083, 0xb271_c3e4, 0xeb04_007f, 0x9a83_b083]),
            (SatOp::Add, C::I32, &[0x8b02_0023, 0xb240_7be4, 0xeb04_007f, 0x9a83_c083, 0xb261_83e4, 0xeb04_007f, 0x9a83_b083]),
            (SatOp::Add, C::I64, &[0x937f_fc24, 0xd240_f884, 0xab02_0023, 0x9a83_6083]),
            (SatOp::Add, C::U8, &[0x8b02_0023, 0xb240_1fe4, 0xeb04_007f, 0x9a83_c083]),
            (SatOp::Add, C::U16, &[0x8b02_0023, 0xb240_3fe4, 0xeb04_007f, 0x9a83_c083]),
            (SatOp::Add, C::U32, &[0x8b02_0023, 0xb240_7fe4, 0xeb04_007f, 0x9a83_c083]),
            (SatOp::Add, C::U64, &[0xab02_0023, 0xda9f_3063]),
            (SatOp::Subtract, C::I8, &[0xcb02_0023, 0xb240_1be4, 0xeb04_007f, 0x9a83_c083, 0xb279_e3e4, 0xeb04_007f, 0x9a83_b083]),
            (SatOp::Subtract, C::I16, &[0xcb02_0023, 0xb240_3be4, 0xeb04_007f, 0x9a83_c083, 0xb271_c3e4, 0xeb04_007f, 0x9a83_b083]),
            (SatOp::Subtract, C::I32, &[0xcb02_0023, 0xb240_7be4, 0xeb04_007f, 0x9a83_c083, 0xb261_83e4, 0xeb04_007f, 0x9a83_b083]),
            (SatOp::Subtract, C::I64, &[0x937f_fc24, 0xd240_f884, 0xeb02_0023, 0x9a83_6083]),
            (SatOp::Subtract, C::U8, &[0xeb02_0023, 0x9a9f_2063]),
            (SatOp::Subtract, C::U16, &[0xeb02_0023, 0x9a9f_2063]),
            (SatOp::Subtract, C::U32, &[0xeb02_0023, 0x9a9f_2063]),
            (SatOp::Subtract, C::U64, &[0xeb02_0023, 0x9a9f_2063]),
            (SatOp::Divide, C::I8, &[0x9ac2_0c23, 0xb240_1be4, 0xeb04_007f, 0x9a83_c083]),
            (SatOp::Divide, C::I16, &[0x9ac2_0c23, 0xb240_3be4, 0xeb04_007f, 0x9a83_c083]),
            (SatOp::Divide, C::I32, &[0x9ac2_0c23, 0xb240_7be4, 0xeb04_007f, 0x9a83_c083]),
            (SatOp::Divide, C::I64, &[0x9ac2_0c23, 0xb241_03e4, 0xeb04_007f, 0xba41_0840, 0xb240_fbe4, 0x9a83_0083]),
            (SatOp::Divide, C::U8, &[0x9ac2_0823]),
            (SatOp::Divide, C::U16, &[0x9ac2_0823]),
            (SatOp::Divide, C::U32, &[0x9ac2_0823]),
            (SatOp::Divide, C::U64, &[0x9ac2_0823]),
            (SatOp::Remainder, C::I8, &[0x9ac2_0c23, 0x9b02_8463]),
            (SatOp::Remainder, C::I16, &[0x9ac2_0c23, 0x9b02_8463]),
            (SatOp::Remainder, C::I32, &[0x9ac2_0c23, 0x9b02_8463]),
            (SatOp::Remainder, C::I64, &[0x9ac2_0c23, 0x9b02_8463]),
            (SatOp::Remainder, C::U8, &[0x9ac2_0823, 0x9b02_8463]),
            (SatOp::Remainder, C::U16, &[0x9ac2_0823, 0x9b02_8463]),
            (SatOp::Remainder, C::U32, &[0x9ac2_0823, 0x9b02_8463]),
            (SatOp::Remainder, C::U64, &[0x9ac2_0823, 0x9b02_8463]),
            (SatOp::Multiply, C::I8, &[0x9b02_7c23, 0xb240_1be4, 0xeb04_007f, 0x9a83_c083, 0xb279_e3e4, 0xeb04_007f, 0x9a83_b083]),
            (SatOp::Multiply, C::I16, &[0x9b02_7c23, 0xb240_3be4, 0xeb04_007f, 0x9a83_c083, 0xb271_c3e4, 0xeb04_007f, 0x9a83_b083]),
            (SatOp::Multiply, C::I32, &[0x9b02_7c23, 0xb240_7be4, 0xeb04_007f, 0x9a83_c083, 0xb261_83e4, 0xeb04_007f, 0x9a83_b083]),
            (SatOp::Multiply, C::I64, &[0x9b42_7c24, 0x9b02_7c23, 0xeb83_fc9f, 0x937f_fc84, 0xd240_f884, 0x9a83_1083]),
            (SatOp::Multiply, C::U8, &[0x9b02_7c23, 0xb240_1fe4, 0xeb04_007f, 0x9a83_8083]),
            (SatOp::Multiply, C::U16, &[0x9b02_7c23, 0xb240_3fe4, 0xeb04_007f, 0x9a83_8083]),
            (SatOp::Multiply, C::U32, &[0x9b02_7c23, 0xb240_7fe4, 0xeb04_007f, 0x9a83_8083]),
            (SatOp::Multiply, C::U64, &[0x9bc2_7c24, 0x9b02_7c23, 0xf100_009f, 0xda9f_0063]),
        ];
        for (op, carrier, expected) in ROWS {
            let (kind, family) = match op {
                SatOp::Add => (Kind::SaturatingAdd { carrier }, Fam::SaturatingAdd(carrier)),
                SatOp::Subtract => (
                    Kind::SaturatingSubtract { carrier },
                    Fam::SaturatingSubtract(carrier),
                ),
                SatOp::Divide => (
                    Kind::SaturatingDivide {
                        carrier,
                        obligation: ob(),
                        accepted_fact: fact(),
                    },
                    Fam::SaturatingDivide(carrier),
                ),
                SatOp::Remainder => (
                    Kind::SaturatingRemainder {
                        carrier,
                        obligation: ob(),
                        accepted_fact: fact(),
                    },
                    Fam::SaturatingRemainder(carrier),
                ),
                SatOp::Multiply => (
                    Kind::SaturatingMultiply { carrier },
                    Fam::SaturatingMultiply(carrier),
                ),
            };
            let n = SaturatingRealization::of(op, carrier).operand_count();
            let ops = vs(&p, &["x1", "x2", "x3", "x4"][..n]);
            let encoded = encode_aarch64_selected_form(&p, kind, key(family), &ops).unwrap();
            assert_eq!(encoded.bytes(), le(expected), "{op:?} {carrier:?}");
            rejects_every_bit_flip(&p, kind, key(family), &ops, encoded.bytes());
            if n == 4 {
                assert!(
                    encode_aarch64_selected_form(
                        &p,
                        kind,
                        key(family),
                        &vs(&p, &["x1", "x2", "x3", "x3"])
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn trapping_forms_match_all_seventy_two_clang_golden_vectors() {
        let p = phys();
        const fn conv(source: IntegerSign, c: C, w: &'static [u32]) -> (TrapOp, C, &'static [u32]) {
            (TrapOp::Convert { source }, c, w)
        }
        #[rustfmt::skip]
        const ROWS: [(TrapOp, C, &[u32]); 72] = [
            (TrapOp::Add, C::I8, &[0x8b02_0023, 0xeb23_807f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Add, C::I16, &[0x8b02_0023, 0xeb23_a07f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Add, C::I32, &[0x8b02_0023, 0xeb23_c07f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Add, C::I64, &[0xab02_0023, 0x5400_0047, 0xd420_0000]),
            (TrapOp::Add, C::U8, &[0x8b02_0023, 0xeb23_007f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Add, C::U16, &[0x8b02_0023, 0xeb23_207f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Add, C::U32, &[0x8b02_0023, 0xeb23_407f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Add, C::U64, &[0xab02_0023, 0x5400_0043, 0xd420_0000]),
            (TrapOp::Subtract, C::I8, &[0xcb02_0023, 0xeb23_807f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Subtract, C::I16, &[0xcb02_0023, 0xeb23_a07f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Subtract, C::I32, &[0xcb02_0023, 0xeb23_c07f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Subtract, C::I64, &[0xeb02_0023, 0x5400_0047, 0xd420_0000]),
            (TrapOp::Subtract, C::U8, &[0xcb02_0023, 0xeb23_007f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Subtract, C::U16, &[0xcb02_0023, 0xeb23_207f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Subtract, C::U32, &[0xcb02_0023, 0xeb23_407f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Subtract, C::U64, &[0xeb02_0023, 0x5400_0042, 0xd420_0000]),
            (TrapOp::Multiply, C::I8, &[0x9b02_7c23, 0xeb23_807f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Multiply, C::I16, &[0x9b02_7c23, 0xeb23_a07f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Multiply, C::I32, &[0x9b02_7c23, 0xeb23_c07f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Multiply, C::I64, &[0x9b42_7c24, 0x9b02_7c23, 0xeb83_fc9f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Multiply, C::U8, &[0x9b02_7c23, 0xeb23_007f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Multiply, C::U16, &[0x9b02_7c23, 0xeb23_207f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Multiply, C::U32, &[0x9b02_7c23, 0xeb23_407f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Multiply, C::U64, &[0x9bc2_7c24, 0x9b02_7c23, 0xf100_009f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Divide, C::I8, &[0xb500_0042, 0xd420_0000, 0x9ac2_0c23, 0xeb23_807f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Divide, C::I16, &[0xb500_0042, 0xd420_0000, 0x9ac2_0c23, 0xeb23_a07f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Divide, C::I32, &[0xb500_0042, 0xd420_0000, 0x9ac2_0c23, 0xeb23_c07f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::Divide, C::I64, &[0xb500_0042, 0xd420_0000, 0xb100_045f, 0xfa41_0820, 0x5400_0047, 0xd420_0000, 0x9ac2_0c23]),
            (TrapOp::Divide, C::U8, &[0xb500_0042, 0xd420_0000, 0x9ac2_0823]),
            (TrapOp::Divide, C::U16, &[0xb500_0042, 0xd420_0000, 0x9ac2_0823]),
            (TrapOp::Divide, C::U32, &[0xb500_0042, 0xd420_0000, 0x9ac2_0823]),
            (TrapOp::Divide, C::U64, &[0xb500_0042, 0xd420_0000, 0x9ac2_0823]),
            (TrapOp::Remainder, C::I8, &[0xb500_0042, 0xd420_0000, 0x9ac2_0c24, 0xeb24_809f, 0x5400_0040, 0xd420_0000, 0x9b02_8483]),
            (TrapOp::Remainder, C::I16, &[0xb500_0042, 0xd420_0000, 0x9ac2_0c24, 0xeb24_a09f, 0x5400_0040, 0xd420_0000, 0x9b02_8483]),
            (TrapOp::Remainder, C::I32, &[0xb500_0042, 0xd420_0000, 0x9ac2_0c24, 0xeb24_c09f, 0x5400_0040, 0xd420_0000, 0x9b02_8483]),
            (TrapOp::Remainder, C::I64, &[0xb500_0042, 0xd420_0000, 0xb100_045f, 0xfa41_0820, 0x5400_0047, 0xd420_0000, 0x9ac2_0c24, 0x9b02_8483]),
            (TrapOp::Remainder, C::U8, &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483]),
            (TrapOp::Remainder, C::U16, &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483]),
            (TrapOp::Remainder, C::U32, &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483]),
            (TrapOp::Remainder, C::U64, &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483]),
            (TrapOp::ShiftLeft, C::I8, &[0xf100_205f, 0x5400_0043, 0xd420_0000, 0x9ac2_2023, 0xeb23_807f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::ShiftLeft, C::I16, &[0xf100_405f, 0x5400_0043, 0xd420_0000, 0x9ac2_2023, 0xeb23_a07f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::ShiftLeft, C::I32, &[0xf100_805f, 0x5400_0043, 0xd420_0000, 0x9ac2_2023, 0xeb23_c07f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::ShiftLeft, C::I64, &[0xf101_005f, 0x5400_0043, 0xd420_0000, 0x9ac2_2023, 0x9ac2_2864, 0xeb01_009f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::ShiftLeft, C::U8, &[0xf100_205f, 0x5400_0043, 0xd420_0000, 0x9ac2_2023, 0xeb23_007f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::ShiftLeft, C::U16, &[0xf100_405f, 0x5400_0043, 0xd420_0000, 0x9ac2_2023, 0xeb23_207f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::ShiftLeft, C::U32, &[0xf100_805f, 0x5400_0043, 0xd420_0000, 0x9ac2_2023, 0xeb23_407f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::ShiftLeft, C::U64, &[0xf101_005f, 0x5400_0043, 0xd420_0000, 0x9ac2_2023, 0x9ac2_2464, 0xeb01_009f, 0x5400_0040, 0xd420_0000]),
            (TrapOp::ShiftRight, C::I8, &[0xf100_205f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823]),
            (TrapOp::ShiftRight, C::I16, &[0xf100_405f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823]),
            (TrapOp::ShiftRight, C::I32, &[0xf100_805f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823]),
            (TrapOp::ShiftRight, C::I64, &[0xf101_005f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823]),
            (TrapOp::ShiftRight, C::U8, &[0xf100_205f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423]),
            (TrapOp::ShiftRight, C::U16, &[0xf100_405f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423]),
            (TrapOp::ShiftRight, C::U32, &[0xf100_805f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423]),
            (TrapOp::ShiftRight, C::U64, &[0xf101_005f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423]),
            conv(IntegerSign::Signed, C::I8, &[0xeb21_803f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Signed, C::I16, &[0xeb21_a03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Signed, C::I32, &[0xeb21_c03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Signed, C::I64, &[0xaa01_03e3]),
            conv(IntegerSign::Signed, C::U8, &[0xf278_dc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Signed, C::U16, &[0xf270_bc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Signed, C::U32, &[0xf260_7c3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Signed, C::U64, &[0xf241_003f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Unsigned, C::I8, &[0xf279_e03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Unsigned, C::I16, &[0xf271_c03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Unsigned, C::I32, &[0xf261_803f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Unsigned, C::I64, &[0xf241_003f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Unsigned, C::U8, &[0xf278_dc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Unsigned, C::U16, &[0xf270_bc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Unsigned, C::U32, &[0xf260_7c3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3]),
            conv(IntegerSign::Unsigned, C::U64, &[0xaa01_03e3]),
        ];
        assert_eq!(TrappingForm::ALL.len(), ROWS.len());
        for (operation, carrier, expected) in ROWS {
            let form = TrappingForm { operation, carrier };
            let kind = Kind::TrappingInteger { form };
            let ops = vs(
                &p,
                if form.source_count() == 1 {
                    &["x1", "x3", "x4"]
                } else {
                    &["x1", "x2", "x3", "x4"]
                },
            );
            let encoded =
                encode_aarch64_selected_form(&p, kind, key(Fam::TrappingInteger(form)), &ops)
                    .unwrap();
            assert_eq!(encoded.bytes(), le(expected), "{form:?}");
            assert_eq!(
                usize::from(target_aarch64::form_effects::trapping_forms::byte_size(
                    form
                )),
                encoded.bytes().len()
            );
            rejects_every_bit_flip(
                &p,
                kind,
                key(Fam::TrappingInteger(form)),
                &ops,
                encoded.bytes(),
            );
        }
    }

    #[test]
    fn atomic_forms_match_golden_vectors_and_reject_bit_flips() {
        let p = phys();
        for (width, load, store) in [
            (AtomicWidth::Bits8, 0x085f_fd2b_u32, 0x0811_fd30_u32),
            (AtomicWidth::Bits16, 0x485f_fd2b, 0x4811_fd30),
            (AtomicWidth::Bits32, 0x885f_fd2b, 0x8811_fd30),
            (AtomicWidth::Bits64, 0xc85f_fd2b, 0xc811_fd30),
        ] {
            for (op, narrow, wide) in [
                (AtomicOperation::FetchAdd, 0x0b0a_0170_u32, 0x8b0a_0170_u32),
                (AtomicOperation::FetchSub, 0x4b0a_0170, 0xcb0a_0170),
                (AtomicOperation::Swap, 0x2a0a_03f0, 0xaa0a_03f0),
                (AtomicOperation::FetchAnd, 0x0a0a_0170, 0x8a0a_0170),
                (AtomicOperation::FetchOr, 0x2a0a_0170, 0xaa0a_0170),
                (AtomicOperation::FetchXor, 0x4a0a_0170, 0xca0a_0170),
            ] {
                let form = AtomicForm {
                    operation: op,
                    width,
                };
                let kind = Kind::AtomicAccess { form };
                let ops = vs(&p, &["x9", "x10", "x11"]);
                let encoded =
                    encode_aarch64_selected_form(&p, kind, key(Fam::AtomicAccess(form)), &ops)
                        .unwrap();
                let alu = if width == AtomicWidth::Bits64 {
                    wide
                } else {
                    narrow
                };
                assert_eq!(
                    encoded.bytes(),
                    le(&[0xd503_3fbf, load, alu, store, 0x35ff_ffb1, 0xd503_3fbf])
                );
                rejects_every_bit_flip(
                    &p,
                    kind,
                    key(Fam::AtomicAccess(form)),
                    &ops,
                    encoded.bytes(),
                );
            }
        }
        for form in AtomicForm::ALL {
            let kind = Kind::AtomicAccess { form };
            let ops = match form.operation {
                AtomicOperation::Load | AtomicOperation::Store => vs(&p, &["x1", "x2"]),
                AtomicOperation::CompareExchange => vs(&p, &["x9", "x10", "x11", "x12"]),
                AtomicOperation::CompareExchangeOnce => vs(&p, &["x1", "x2", "x3", "x4", "x5"]),
                _ => vs(&p, &["x9", "x10", "x11"]),
            };
            let encoded =
                encode_aarch64_selected_form(&p, kind, key(Fam::AtomicAccess(form)), &ops).unwrap();
            validate_aarch64_selected_form_encoding(
                &p,
                kind,
                key(Fam::AtomicAccess(form)),
                &ops,
                encoded.bytes(),
            )
            .unwrap();
            rejects_every_bit_flip(
                &p,
                kind,
                key(Fam::AtomicAccess(form)),
                &ops,
                encoded.bytes(),
            );
        }
    }

    #[test]
    fn float_arithmetic_policy_and_bit_transfers_match_golden_words_and_declared_lengths() {
        let p = phys();
        use IeeeFloatArithmeticOperation as FOp;
        use IeeeFloatFormat::{Binary32, Binary64};
        #[rustfmt::skip]
        const GOLDEN: &[(IeeeFloatArithmeticForm, &[&str], &[u32])] = &[
            (IeeeFloatArithmeticForm { operation: FOp::Add, format: Binary64 }, &["x1", "x2", "x0"], &[0x9e67_0030, 0x9e67_0051, 0x1e71_2a10, 0x9e66_0200]),
            (IeeeFloatArithmeticForm { operation: FOp::Divide, format: Binary32 }, &["x3", "x4", "x5"], &[0x1e27_0070, 0x1e27_0091, 0x1e31_1a10, 0x1e26_0205]),
            (IeeeFloatArithmeticForm { operation: FOp::Minimum, format: Binary64 }, &["x1", "x2", "x0"], &[0x9e67_0030, 0x9e67_0051, 0x7ef0_e632, 0x2e71_1e12, 0x9e66_0240]),
            (IeeeFloatArithmeticForm { operation: FOp::MultiplyThenAdd, format: Binary64 }, &["x1", "x2", "x3", "x0"], &[0x9e67_0030, 0x9e67_0051, 0x1e71_0a10, 0x9e67_0071, 0x1e71_2a10, 0x9e66_0200]),
            (IeeeFloatArithmeticForm { operation: FOp::ConvertFormat, format: Binary64 }, &["x3", "x4"], &[0x1e27_0070, 0x1e22_c210, 0x9e66_0204]),
            (IeeeFloatArithmeticForm { operation: FOp::FromInteger(IeeeFloatIntegerCarrier::I8), format: Binary64 }, &["x3", "x4"], &[0x1300_1c64, 0x1e62_0090, 0x9e66_0204]),
            (IeeeFloatArithmeticForm { operation: FOp::TruncateToInteger { target: IeeeFloatIntegerCarrier::I32, domain: IeeeFloatToIntegerDomain::Trapping }, format: Binary64 }, &["x3", "x4"], &[0x9e67_0070, 0xd2f8_3c04, 0xf2a0_0404, 0x9e67_0091, 0x7ef1_e612, 0x9e66_0244, 0xb500_0044, 0xd420_0000, 0xd2e8_3c04, 0x9e67_0091, 0x7ef0_e632, 0x9e66_0244, 0xb500_0044, 0xd420_0000, 0x1e78_0204, 0x9340_7c84]),
            (IeeeFloatArithmeticForm { operation: FOp::TruncateToInteger { target: IeeeFloatIntegerCarrier::I8, domain: IeeeFloatToIntegerDomain::Saturating }, format: Binary64 }, &["x3", "x4"], &[0x9e67_0070, 0xd2f8_0c04, 0x9e67_0091, 0x1e71_4a10, 0xd2e8_0be4, 0xf2d8_0004, 0x9e67_0091, 0x1e71_5a10, 0x1e78_0204, 0x9340_7c84]),
            (IeeeFloatArithmeticForm { operation: FOp::Classify, format: Binary64 }, &["x0", "x0"], &[0x9e67_0010, 0xd374_f800, 0xb400_00a0, 0xd11f_fc00, 0xb400_0140, 0xd280_0040, 0x1400_000e, 0x9e66_0200, 0xd340_f800, 0xb400_0060, 0xd280_0060, 0x1400_0009, 0xd280_0080, 0x1400_0007, 0x9e66_0200, 0xd340_cc00, 0xb400_0060, 0xd280_0000, 0x1400_0002, 0xd280_0020, 0xb400_0100, 0x9e67_0011, 0x9e66_0200, 0xd37f_fc00, 0xd360_7c00, 0x9e67_0010, 0x0eb1_1e10, 0x9e66_0200]),
        ];
        for &(form, names, expected) in GOLDEN {
            let ops = vs(&p, names);
            let encoded = encode_aarch64_selected_form(
                &p,
                Kind::NearestIeeeFloatArithmetic { form },
                key(Fam::NearestIeeeFloatArithmetic(form)),
                &ops,
            )
            .unwrap();
            assert_eq!(encoded.bytes(), le(expected), "{form:?}");
        }
        for form in IeeeFloatArithmeticForm::ALL {
            let ops = vs(
                &p,
                &["x6", "x7", "x30", "x12"][4 - form.operand_count() - 1..],
            );
            let kind = Kind::NearestIeeeFloatArithmetic { form };
            let encoded = encode_aarch64_selected_form(
                &p,
                kind,
                key(Fam::NearestIeeeFloatArithmetic(form)),
                &ops,
            )
            .unwrap();
            if let MachineSizeKnowledge::ExactBytes(n) =
                target_aarch64::float_sizes::ieee_float_arithmetic_size(form)
            {
                assert_eq!(encoded.bytes().len(), usize::from(n), "{form:?}");
            }
            rejects_every_bit_flip(
                &p,
                kind,
                key(Fam::NearestIeeeFloatArithmetic(form)),
                &ops,
                encoded.bytes(),
            );
        }
        for form in FloatResultPolicyForm::ALL {
            let ops = vs(
                &p,
                if form.can_trap() {
                    &["x3", "x4"]
                } else {
                    &["x1", "x3", "x4"]
                },
            );
            let kind = Kind::FloatResultPolicy { form };
            let encoded =
                encode_aarch64_selected_form(&p, kind, key(Fam::FloatResultPolicy(form)), &ops)
                    .unwrap();
            if form
                == (FloatResultPolicyForm {
                    format: Binary64,
                    policy: FloatResultPolicyKind::Trapping,
                })
            {
                assert_eq!(
                    encoded.bytes(),
                    le(&[
                        0x9e67_0070,
                        0x9e66_0204,
                        0xd374_f884,
                        0xf11f_fc9f,
                        0x5400_0041,
                        0xd420_0000,
                        0x9e66_0204
                    ])
                );
            }
            if let MachineSizeKnowledge::ExactBytes(n) =
                target_aarch64::float_sizes::float_result_policy_size(form)
            {
                assert_eq!(encoded.bytes().len(), usize::from(n), "{form:?}");
            }
            rejects_every_bit_flip(
                &p,
                kind,
                key(Fam::FloatResultPolicy(form)),
                &ops,
                encoded.bytes(),
            );
        }
        for (kind, family, names, expected) in [
            (
                Kind::Float32ToBits,
                Fam::Float32ToBits,
                &["d3", "x4"],
                0x1e26_0064_u32,
            ),
            (
                Kind::Float64ToBits,
                Fam::Float64ToBits,
                &["d3", "x4"],
                0x9e66_0064,
            ),
            (
                Kind::BitsToFloat32,
                Fam::BitsToFloat32,
                &["x3", "d4"],
                0x1e27_0064,
            ),
            (
                Kind::BitsToFloat64,
                Fam::BitsToFloat64,
                &["x3", "d4"],
                0x9e67_0064,
            ),
        ] {
            let ops = vs(&p, names);
            let encoded = encode_aarch64_selected_form(&p, kind, key(family), &ops).unwrap();
            assert_eq!(encoded.bytes(), expected.to_le_bytes());
            rejects_every_bit_flip(&p, kind, key(family), &ops, encoded.bytes());
        }
    }

    #[test]
    fn byte_sequences_and_encoded_scalar_match_golden_bytes_and_reject_mutations() {
        let p = phys();
        let zero_ops = vs(&p, &["x0", "x1", "x2"]);
        let zero =
            encode_aarch64_selected_form(&p, Kind::ZeroBytes, key(Fam::ZeroBytes), &zero_ops)
                .unwrap();
        assert_eq!(
            zero.bytes(),
            le(&[
                0xd280_0002,
                0xb400_00a1,
                0x3822_681f,
                0x9100_0442,
                0xeb01_005f,
                0x54ff_ffa1
            ])
        );
        rejects_every_bit_flip(
            &p,
            Kind::ZeroBytes,
            key(Fam::ZeroBytes),
            &zero_ops,
            zero.bytes(),
        );

        let copy_ops = vs(&p, &["x0", "x1", "x2", "x3", "x4"]);
        let copy =
            encode_aarch64_selected_form(&p, Kind::CopyBytes, key(Fam::CopyBytes), &copy_ops)
                .unwrap();
        rejects_every_bit_flip(
            &p,
            Kind::CopyBytes,
            key(Fam::CopyBytes),
            &copy_ops,
            copy.bytes(),
        );
        assert!(
            encode_aarch64_selected_form(
                &p,
                Kind::CopyBytes,
                key(Fam::CopyBytes),
                &vs(&p, &["x0", "x1", "x2", "x0", "x4"])
            )
            .is_err()
        );

        let cmp_ops = vs(&p, &["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]);
        let cmp = encode_aarch64_selected_form(
            &p,
            Kind::CompareByteSequences,
            key(Fam::CompareByteSequences),
            &cmp_ops,
        )
        .unwrap();
        rejects_every_bit_flip(
            &p,
            Kind::CompareByteSequences,
            key(Fam::CompareByteSequences),
            &cmp_ops,
            cmp.bytes(),
        );

        for pred in ByteSequencePredicate::ALL {
            let kind = Kind::ScanByteSequence { predicate: pred };
            let scan =
                encode_aarch64_selected_form(&p, kind, key(Fam::ScanByteSequence), &copy_ops)
                    .unwrap();
            rejects_every_bit_flip(
                &p,
                kind,
                key(Fam::ScanByteSequence),
                &copy_ops,
                scan.bytes(),
            );
        }

        let codec_ops: [RegisterViewId; 6] = vs(&p, &["x0", "x1", "x2", "x3", "x4", "x5"])
            .try_into()
            .unwrap();
        encoded_scalar_plan::harness::rejects_every_single_bit_mutation(
            codec_ops,
            |k, alt, ops| encode_aarch64_selected_form(&p, k, alt, ops).map(|e| e.bytes),
            |k, alt, ops, bytes| {
                validate_aarch64_selected_form_encoding(&p, k, alt, ops, bytes).is_err()
            },
        );
    }

    #[test]
    fn control_memory_frame_and_hosted_forms_match_golden_bytes_and_reject_mutations() {
        let p = phys();
        let jmp = encode_aarch64_selected_jump_form(&p, key(Fam::Jump), -16).unwrap();
        assert_eq!(jmp.bytes(), 0x17ff_fffc_u32.to_le_bytes());
        reject_every_bit_flip(jmp.bytes(), "jump", |b| {
            validate_aarch64_selected_jump_form(&p, key(Fam::Jump), -16, b).is_err()
        });
        assert_eq!(
            encode_aarch64_selected_jump_form(&p, key(Fam::Jump), 2),
            Err(Aarch64SelectedFormEncodingError::BranchDisplacementMisaligned)
        );

        for (fam, disp, form, expected) in [
            (
                Fam::ConditionalBranchNonZero,
                16,
                Aarch64ConditionalBranchForm::CanonicalWordV1,
                &[0x5400_0081_u32][..],
            ),
            (
                Fam::ConditionalBranchU64LessThan,
                -8,
                Aarch64ConditionalBranchForm::CanonicalWordV1,
                &[0x54ff_ffc3][..],
            ),
            (
                Fam::ConditionalBranchI64LessThan,
                12,
                Aarch64ConditionalBranchForm::CanonicalWordV1,
                &[0x5400_006b][..],
            ),
            (
                Fam::ConditionalBranchNonZero,
                1 << 20,
                Aarch64ConditionalBranchForm::WidenedPairV1,
                &[0x5400_0040, 0x1403_ffff][..],
            ),
        ] {
            let br = encode_aarch64_selected_conditional_branch_form(&p, key(fam), fam, form, disp)
                .unwrap();
            assert_eq!(br.bytes(), le(expected));
            reject_every_bit_flip(br.bytes(), "branch", |b| {
                validate_aarch64_selected_conditional_branch_form(&p, key(fam), fam, form, disp, b)
                    .is_err()
            });
        }
        let cbnz =
            encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(&p, v(&p, "x5"), 20)
                .unwrap();
        assert_eq!(cbnz.bytes(), 0xb500_00a5_u32.to_le_bytes());
        reject_every_bit_flip(cbnz.bytes(), "cbnz", |b| {
            validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                &p,
                v(&p, "x5"),
                20,
                b,
            )
            .is_err()
        });

        let to_fp =
            |e: ValidatedAarch64SelectedFormEncoding| isa_conformance::narrow_load::Footprint {
                bytes: e.bytes().to_vec(),
                register_reads: e.footprint().register_reads.clone(),
                register_writes: e.footprint().register_writes.clone(),
                memory: e.footprint().encoded.memory,
                stack: e.footprint().encoded.stack,
                trap: e.footprint().encoded.trap,
                flags_untouched: !e.footprint().writes_nzcv,
            };
        isa_conformance::narrow_load::conformance(
            &p,
            &isa_conformance::narrow_load::Roster {
                bases: &["x0", "x1", "x15", "x28"],
                destination: "x2",
                offsets: |w| vec![0, u32::from(w), 4095 * u32::from(w)],
            },
            &isa_conformance::narrow_load::Exact {
                operands: ["x1", "x2"],
                expected: [(1, &[0x22, 0, 0x40, 0x39]), (2, &[0x22, 0, 0x40, 0x79])],
                signed_opcode: (2, [0x80, 0x80]),
                invalid_views: &["sp", "w1", "d0"],
                invalid_offsets: &[(2, 1), (2, 8192), (1, 4096)],
            },
            |k, alt, ops, d| encode_aarch64_selected_memory_form(&p, k, alt, ops, d).map(to_fp),
            |k, alt, ops, d, b| {
                validate_aarch64_selected_memory_form(&p, k, alt, ops, d, b).map(to_fp)
            },
        );
        let l32 = encode_aarch64_selected_memory_form(
            &p,
            Kind::Load32 { byte_offset: 4 },
            key(Fam::Load32),
            &vs(&p, &["x1", "x2"]),
            4,
        )
        .unwrap();
        assert_eq!(l32.bytes(), &[0x22, 0x04, 0x40, 0xb9]);
        let idx = encode_aarch64_selected_memory_form(
            &p,
            Kind::Load8Indexed,
            key(Fam::Load8Indexed),
            &vs(&p, &["x1", "x2", "x3"]),
            0,
        )
        .unwrap();
        assert_eq!(idx.bytes(), 0x3862_6823_u32.to_le_bytes());
        let pk_kind = Kind::LoadPacked {
            byte_offset: 2,
            width: PackedByteWidth::Three,
        };
        let pk_ops = vs(&p, &["x1", "x2", "x3"]);
        let pk =
            encode_aarch64_selected_memory_form(&p, pk_kind, key(Fam::LoadPacked3), &pk_ops, 2)
                .unwrap();
        reject_every_bit_flip(pk.bytes(), "packed", |b| {
            validate_aarch64_selected_memory_form(&p, pk_kind, key(Fam::LoadPacked3), &pk_ops, 2, b)
                .is_err()
        });
        let st = encode_aarch64_selected_memory_form(
            &p,
            Kind::Store {
                byte_offset: 16,
                byte_size: 8,
            },
            key(Fam::Store),
            &vs(&p, &["x1", "x2"]),
            16,
        )
        .unwrap();
        assert_eq!(st.bytes(), 0xf900_0822_u32.to_le_bytes());
        let slot = FrameStorageSlotId::Local(LocalStorageSlotId::Structural {
            operation: OperationId::new(104).unwrap(),
            place: PlaceId::new(102).unwrap(),
        });
        let fr = encode_aarch64_selected_frame_form(
            &p,
            Kind::Store64 {
                slot,
                byte_offset: 8,
            },
            key(Fam::Store64),
            &vs(&p, &["x9", "x10"]),
            64,
            32,
        )
        .unwrap();
        reject_every_bit_flip(fr.bytes(), "aligned frame", |b| {
            validate_aarch64_selected_frame_form(
                &p,
                Kind::Store64 {
                    slot,
                    byte_offset: 8,
                },
                key(Fam::Store64),
                &vs(&p, &["x9", "x10"]),
                64,
                32,
                b,
            )
            .is_err()
        });

        let bslot = LocalStorageSlotId::Boundary {
            operation: OperationId::new(1).unwrap(),
        };
        let fp = encode_aarch64_selected_floating_control_form(
            &p,
            Kind::SaveFloatingControl { slot: bslot },
            key(Fam::SaveFloatingControl),
            &[],
            16,
        )
        .unwrap();
        reject_every_bit_flip(fp.bytes(), "fpcr", |b| {
            validate_aarch64_selected_memory_form(
                &p,
                Kind::SaveFloatingControl { slot: bslot },
                key(Fam::SaveFloatingControl),
                &[],
                16,
                b,
            )
            .is_err()
        });

        let sslot = LocalStorageSlotId::Structural {
            operation: OperationId::new(1).unwrap(),
            place: PlaceId::new(1).unwrap(),
        };
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            let ex = hosted_exit_process::encode_aarch64_selected_hosted_exit_process_form(
                target,
                &p,
                Kind::HostedExitProcessI32,
                key(Fam::HostedExitProcessI32),
                &vs(&p, &["x1"]),
            )
            .unwrap();
            reject_every_bit_flip(ex.bytes(), "hosted_exit", |b| {
                hosted_exit_process::validate_aarch64_selected_hosted_exit_process_form(
                    target,
                    &p,
                    Kind::HostedExitProcessI32,
                    key(Fam::HostedExitProcessI32),
                    &vs(&p, &["x1"]),
                    b,
                )
                .is_err()
            });
            let rd = hosted_read_byte::encode_aarch64_selected_hosted_read_byte_form(
                target,
                &p,
                Kind::HostedReadByte { slot: sslot },
                key(Fam::HostedReadByte),
                &[],
                16,
            )
            .unwrap();
            reject_every_bit_flip(rd.bytes(), "hosted_read", |b| {
                hosted_read_byte::validate_aarch64_selected_hosted_read_byte_form(
                    target,
                    &p,
                    Kind::HostedReadByte { slot: sslot },
                    key(Fam::HostedReadByte),
                    &[],
                    16,
                    b,
                )
                .is_err()
            });
            let wr = encode_aarch64_selected_hosted_write_byte_form(
                target,
                &p,
                Kind::HostedWriteByteI32 { slot: bslot },
                key(Fam::HostedWriteByteI32),
                &vs(&p, &["x1"]),
                16,
            )
            .unwrap();
            reject_every_bit_flip(wr.bytes(), "hosted_write", |b| {
                validate_aarch64_selected_hosted_write_byte_form(
                    target,
                    &p,
                    Kind::HostedWriteByteI32 { slot: bslot },
                    key(Fam::HostedWriteByteI32),
                    &vs(&p, &["x1"]),
                    16,
                    b,
                )
                .is_err()
            });
        }
    }

    #[test]
    fn call_templates_and_callback_fields_cover_linux_macos_and_windows_arm64() {
        let p = phys();
        let constraints = crate::validate_aarch64_register_constraint_catalog(
            crate::aarch64_register_constraint_catalog(&p),
            &p,
        )
        .unwrap();
        let kind = Kind::CallScalar {
            target: SelectedCallTarget::Machine(MachineId::new(7).unwrap()),
        };
        assert!(
            crate::AARCH64_REQUIRED_REGISTER_CONSTRAINTS
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        for target in [
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_arm64(),
        ] {
            let catalog = crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
            let ops = vs(&p, &["x0", "x1", "x0"]);
            let eff = &catalog
                .declarations
                .iter()
                .find(|d| {
                    d.semantic == selected_instructions::MachineSemanticKind::CallScalar
                        && d.alternatives[0].encoded.external_operand_reads.len() == 2
                })
                .unwrap()
                .alternatives[0]
                .encoded;
            let tmpl = encode_aarch64_selected_scalar_call_template(
                target,
                &p,
                kind,
                key(Fam::CallScalar),
                &ops,
                eff,
            )
            .unwrap();
            assert_eq!(tmpl.bytes(), &crate::ORDINARY_DIRECT_CALL_BYTES);
            assert_eq!(
                tmpl.fixup(),
                scalar_call::canonical_fixup(MachineId::new(7).unwrap())
            );
            assert!(
                validate_aarch64_selected_scalar_call_template(
                    target,
                    &p,
                    kind,
                    key(Fam::CallScalar),
                    &ops,
                    eff,
                    &[0, 0, 0, 0],
                    tmpl.fixup()
                )
                .is_err()
            );
            assert_eq!(
                tmpl.effects().control,
                MachineEncodedControlEffect::DirectRelativeCallV1
            );
            assert_eq!(tmpl.effects().memory, MachineEncodedMemoryEffect::NoneV1);
        }

        use calling_conventions::{MachineRegister as R, PrivateCallbackPlacements};
        use layout_plans::LayoutPlacementReport::{At, Bits};
        let field = Aarch64CallbackMaterializationDestination::RecordField {
            root_register: R::Aarch64X(0),
            placements: PrivateCallbackPlacements::new(&[Bits {
                container: 4,
                container_width: 32,
                destination_lsb: 5,
                source_lsb: 17,
                width: 10,
            }]),
        };
        assert_eq!(
            normalized_foreign_call::callback_group_bytes(&field).unwrap(),
            [
                0x10, 0x00, 0x00, 0x90, 0x10, 0x02, 0x00, 0x91, 0x11, 0x04, 0x40, 0xb9, 0x0f, 0x6a,
                0x51, 0xd3, 0xf1, 0x25, 0x7b, 0xb3, 0x11, 0x04, 0x00, 0xb9
            ]
        );
        for root in [R::Aarch64X(0), R::Aarch64X(15), R::Aarch64X(30), R::X86Rdi] {
            for placements in [
                vec![],
                vec![At { offset: 0 }],
                vec![At { offset: 4096 }],
                vec![Bits {
                    container: 8,
                    container_width: 64,
                    destination_lsb: 1,
                    source_lsb: 3,
                    width: 4,
                }],
            ] {
                let dest = Aarch64CallbackMaterializationDestination::RecordField {
                    root_register: root,
                    placements: PrivateCallbackPlacements::new(&placements),
                };
                let actual = normalized_foreign_call::callback_group_bytes(&dest)
                    .map(|b| u16::try_from(b.len()).unwrap());
                assert_eq!(
                    target_aarch64::callback_geometry::callback_field_bytes(root, &placements),
                    actual
                );
            }
        }
    }
}
