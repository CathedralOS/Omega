use super::emit::{
    append_inc, append_indexed_byte_load, append_jcc_rel32, append_jmp_rel32, append_self_clear,
    append_self_test, assemble_labeled_stream, modrm, rex, rex64, rr64,
};
use super::request_validation::{byte_loop_registers, guard_canonical, resolve_registers};
use super::{
    MachineAlternativeFamily as Family, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect as MemEff, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, RegisterViewId, SelectedInstructionKind as Kind,
    ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
    X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint,
    canonical_effects_encoding,
};
use encoded_scalar_plan::{Step, plan as encoded_scalar_plan};
use selected_instructions::ByteSequencePredicate;

const CC_B: u8 = 0x82;
const CC_AE: u8 = 0x83;
const CC_E: u8 = 0x84;
const CC_NE: u8 = 0x85;
const CC_A: u8 = 0x87;

fn check_canonical(expected: &[u8], actual: &[u8]) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(())
}

pub(super) mod copy_bytes {
    use super::*;

    fn encode_bytes([src, dst, count, cursor, byte]: [u8; 5]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(36);
        append_self_clear(&mut bytes, cursor);
        append_self_clear(&mut bytes, byte);
        append_self_test(&mut bytes, count);
        bytes.extend([0x74, 25]);
        append_indexed_byte_load(&mut bytes, byte, src, cursor, 0);
        bytes.extend([
            rex(false, byte, cursor, dst),
            0x88,
            modrm(2, byte, 4),
            ((cursor & 7) << 3) | (dst & 7),
            0,
            0,
            0,
            0,
        ]);
        append_inc(&mut bytes, cursor);
        rr64(&mut bytes, 0x39, count, cursor);
        bytes.extend([0x72, 0xe7]);
        bytes
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<[u8; 5], Error> {
        let regs = byte_loop_registers::<5>(physical, alternative, Family::CopyBytes, operands, 3)?;
        if displacement != 0 {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(regs)
    }

    fn finish(
        physical: &ValidatedPhysicalRegisterModel,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        canonical_effects_encoding(
            physical,
            operands,
            bytes,
            3,
            MemEff::CopyBytesV1 {
                source_pointer_operand: 0,
                destination_pointer_operand: 1,
                count_operand: 2,
            },
        )
    }

    pub(in super::super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let regs = request(physical, alternative, operands, displacement)?;
        finish(physical, operands, &encode_bytes(regs))
    }

    pub(in super::super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let regs = request(physical, alternative, operands, displacement)?;
        check_canonical(&encode_bytes(regs), bytes)?;
        finish(physical, operands, bytes)
    }
}

pub(super) mod zero_bytes {
    use super::*;

    fn encode_bytes([dst, count, cursor]: [u8; 3]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(25);
        append_self_clear(&mut bytes, cursor);
        append_self_test(&mut bytes, count);
        bytes.extend([0x74, 17]);
        bytes.extend([
            rex(false, 0, cursor, dst),
            0xc6,
            modrm(2, 0, 4),
            ((cursor & 7) << 3) | (dst & 7),
            0,
            0,
            0,
            0,
            0,
        ]);
        append_inc(&mut bytes, cursor);
        rr64(&mut bytes, 0x39, count, cursor);
        bytes.extend([0x72, (-17i8) as u8]);
        bytes
    }

    fn finish(
        physical: &ValidatedPhysicalRegisterModel,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        canonical_effects_encoding(
            physical,
            operands,
            bytes,
            2,
            MemEff::ZeroBytesV1 {
                destination_pointer_operand: 0,
                count_operand: 1,
            },
        )
    }

    pub(in super::super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let regs = byte_loop_registers::<3>(physical, alternative, Family::ZeroBytes, operands, 2)?;
        finish(physical, operands, &encode_bytes(regs))
    }

    pub(in super::super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let regs = byte_loop_registers::<3>(physical, alternative, Family::ZeroBytes, operands, 2)?;
        check_canonical(&encode_bytes(regs), bytes)?;
        finish(physical, operands, bytes)
    }
}

pub(super) mod compare_byte_sequences {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Label {
        Loop,
        Equal,
        Done,
    }

    #[derive(Clone, Copy)]
    enum Op {
        Zero(u8),
        Inc(u8),
        Test64(u8),
        Cmp64(u8, u8),
        LoadByte(u8, u8, u8),
        Jcc(u8, Label),
    }

    fn encode_bytes(regs: [u8; 8]) -> Result<Vec<u8>, Error> {
        let [lb, lc, rb, rc, cur, lbyte, rbyte, res] = regs;
        let stream = [
            (None, Op::Zero(cur)),
            (None, Op::Zero(lbyte)),
            (None, Op::Zero(rbyte)),
            (None, Op::Zero(res)),
            (None, Op::Cmp64(rc, lc)),
            (None, Op::Jcc(CC_NE, Label::Done)),
            (None, Op::Test64(lc)),
            (None, Op::Jcc(CC_E, Label::Equal)),
            (Some(Label::Loop), Op::LoadByte(lbyte, lb, cur)),
            (None, Op::LoadByte(rbyte, rb, cur)),
            (None, Op::Cmp64(rbyte, lbyte)),
            (None, Op::Jcc(CC_NE, Label::Done)),
            (None, Op::Inc(cur)),
            (None, Op::Cmp64(lc, cur)),
            (None, Op::Jcc(CC_NE, Label::Loop)),
            (Some(Label::Equal), Op::Inc(res)),
            (Some(Label::Done), Op::Zero(cur)),
        ];
        assemble_labeled_stream(&stream, |bytes, op, patches| match op {
            Op::Zero(r) => append_self_clear(bytes, r),
            Op::Inc(r) => append_inc(bytes, r),
            Op::Test64(r) => append_self_test(bytes, r),
            Op::Cmp64(src, dst) => rr64(bytes, 0x39, src, dst),
            Op::LoadByte(val, base, idx) => append_indexed_byte_load(bytes, val, base, idx, 0),
            Op::Jcc(cc, target) => append_jcc_rel32(bytes, cc, target, patches),
        })
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<[u8; 8], Error> {
        let regs = byte_loop_registers::<8>(
            physical,
            alternative,
            Family::CompareByteSequences,
            operands,
            4,
        )?;
        if displacement != 0 || regs[4] == 4 {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(regs)
    }

    fn finish(
        physical: &ValidatedPhysicalRegisterModel,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        canonical_effects_encoding(
            physical,
            operands,
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

    pub(in super::super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let regs = request(physical, alternative, operands, displacement)?;
        finish(physical, operands, &encode_bytes(regs)?)
    }

    pub(in super::super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let regs = request(physical, alternative, operands, displacement)?;
        check_canonical(&encode_bytes(regs)?, bytes)?;
        finish(physical, operands, bytes)
    }
}

pub(super) mod scan_byte_sequence {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Label {
        Loop,
        Done,
        Fail,
        Advance1,
        Cont1,
        Cont2Any,
        E0Path,
        EdPath,
        Lead3,
        Cont3Any,
        F0Path,
        F4Path,
    }

    #[derive(Clone, Copy)]
    enum Op {
        Zero(u8),
        Inc(u8),
        AddImm8(u8, u8),
        Test64(u8),
        Test8(u8),
        TestImm8(u8, u8),
        CmpImm8(u8, u8),
        Cmp64(u8, u8),
        LoadByte(u8, u8, u8, u32),
        SetNe8(u8),
        Lea8(u8, u8, u8),
        Jcc(u8, Label),
        Jmp(Label),
    }

    fn encode_bytes(predicate: ByteSequencePredicate, regs: [u8; 5]) -> Result<Vec<u8>, Error> {
        let [base, count, cur, byte, res] = regs;
        let mut s: Vec<(Option<Label>, Op)> = vec![
            (None, Op::Zero(cur)),
            (None, Op::Zero(byte)),
            (None, Op::Zero(res)),
        ];
        let cmp_br = |s: &mut Vec<(Option<Label>, Op)>, lbl, r, imm, cc, tgt| {
            s.push((lbl, Op::CmpImm8(r, imm)));
            s.push((None, Op::Jcc(cc, tgt)));
        };
        if predicate == ByteSequencePredicate::NonEmpty {
            s.push((None, Op::Test64(count)));
            s.push((None, Op::SetNe8(res)));
        } else {
            s.extend([
                (None, Op::Inc(res)),
                (None, Op::Test64(count)),
                (None, Op::Jcc(CC_E, Label::Done)),
            ]);
            if predicate == ByteSequencePredicate::ValidUtf8 {
                s.extend([
                    (Some(Label::Loop), Op::Cmp64(count, cur)),
                    (None, Op::Jcc(CC_AE, Label::Done)),
                    (None, Op::LoadByte(byte, base, cur, 0)),
                ]);
                for &(lbl, imm, cc, tgt) in &[
                    (None, 0x80, CC_B, Label::Advance1),
                    (None, 0xc2, CC_B, Label::Fail),
                    (None, 0xe0, CC_B, Label::Cont1),
                    (None, 0xf0, CC_AE, Label::Lead3),
                    (None, 0xe0, CC_E, Label::E0Path),
                    (None, 0xed, CC_E, Label::EdPath),
                ] {
                    cmp_br(&mut s, lbl, byte, imm, cc, tgt);
                }
                s.push((None, Op::Jmp(Label::Cont2Any)));
                for &(lbl, imm, cc, tgt) in &[
                    (Some(Label::Lead3), 0xf5, CC_AE, Label::Fail),
                    (None, 0xf0, CC_E, Label::F0Path),
                    (None, 0xf4, CC_E, Label::F4Path),
                ] {
                    cmp_br(&mut s, lbl, byte, imm, cc, tgt);
                }
                s.push((None, Op::Jmp(Label::Cont3Any)));
                for &(lbl, conts, lo, hi) in &[
                    (Label::Cont1, 1u8, 0x80u8, 0xbfu8),
                    (Label::Cont2Any, 2, 0x80, 0xbf),
                    (Label::E0Path, 2, 0xa0, 0xbf),
                    (Label::EdPath, 2, 0x80, 0x9f),
                    (Label::Cont3Any, 3, 0x80, 0xbf),
                    (Label::F0Path, 3, 0x90, 0xbf),
                    (Label::F4Path, 3, 0x80, 0x8f),
                ] {
                    s.extend([
                        (Some(lbl), Op::Lea8(byte, cur, conts)),
                        (None, Op::Cmp64(count, byte)),
                        (None, Op::Jcc(CC_AE, Label::Fail)),
                    ]);
                    for off in 1..=u32::from(conts) {
                        s.push((None, Op::LoadByte(byte, base, cur, off)));
                        let (l, h) = if off == 1 { (lo, hi) } else { (0x80, 0xbf) };
                        cmp_br(&mut s, None, byte, l, CC_B, Label::Fail);
                        cmp_br(&mut s, None, byte, h, CC_A, Label::Fail);
                    }
                    s.push((None, Op::AddImm8(cur, conts + 1)));
                    s.push((None, Op::Jmp(Label::Loop)));
                }
                s.push((Some(Label::Advance1), Op::Inc(cur)));
                s.push((None, Op::Jmp(Label::Loop)));
            } else {
                s.push((Some(Label::Loop), Op::LoadByte(byte, base, cur, 0)));
                match predicate {
                    ByteSequencePredicate::NoNul => {
                        s.push((None, Op::Test8(byte)));
                        s.push((None, Op::Jcc(CC_E, Label::Fail)));
                    }
                    ByteSequencePredicate::AsciiOnly => {
                        s.push((None, Op::TestImm8(byte, 0x80)));
                        s.push((None, Op::Jcc(CC_NE, Label::Fail)));
                    }
                    _ => unreachable!(),
                }
                s.extend([
                    (None, Op::Inc(cur)),
                    (None, Op::Cmp64(count, cur)),
                    (None, Op::Jcc(CC_B, Label::Loop)),
                    (None, Op::Jmp(Label::Done)),
                ]);
            }
            s.push((Some(Label::Fail), Op::Zero(res)));
            s.push((Some(Label::Done), Op::Zero(cur)));
        }
        assemble_labeled_stream(&s, |bytes, op, patches| match op {
            Op::Zero(r) => append_self_clear(bytes, r),
            Op::Inc(r) => append_inc(bytes, r),
            Op::AddImm8(r, imm) => bytes.extend([rex64(0, 0, r), 0x83, modrm(3, 0, r), imm]),
            Op::Test64(r) => append_self_test(bytes, r),
            Op::Test8(r) => bytes.extend([rex(false, r, 0, r), 0x84, modrm(3, r, r)]),
            Op::TestImm8(r, imm) => bytes.extend([rex(false, 0, 0, r), 0xf6, modrm(3, 0, r), imm]),
            Op::CmpImm8(r, imm) => bytes.extend([rex(false, 0, 0, r), 0x80, modrm(3, 7, r), imm]),
            Op::Cmp64(src, dst) => rr64(bytes, 0x39, src, dst),
            Op::LoadByte(v, b, i, off) => append_indexed_byte_load(bytes, v, b, i, off),
            Op::SetNe8(r) => bytes.extend([rex(false, 0, 0, r), 0x0f, 0x95, modrm(3, 0, r)]),
            Op::Lea8(v, b, off) => {
                bytes.extend([rex64(v, 0, b), 0x8d]);
                if b & 7 == 4 {
                    bytes.extend([modrm(1, v, 4), 0x24 | (b & 7), off]);
                } else {
                    bytes.extend([modrm(1, v, b), off]);
                }
            }
            Op::Jcc(cc, target) => append_jcc_rel32(bytes, cc, target, patches),
            Op::Jmp(target) => append_jmp_rel32(bytes, target, patches),
        })
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<[u8; 5], Error> {
        let regs =
            byte_loop_registers::<5>(physical, alternative, Family::ScanByteSequence, operands, 2)?;
        if displacement != 0 || regs[2] == 4 || regs[3] == 4 {
            return Err(Error::EncodedFormMismatch);
        }
        Ok(regs)
    }

    fn finish(
        physical: &ValidatedPhysicalRegisterModel,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        canonical_effects_encoding(
            physical,
            operands,
            bytes,
            2,
            MemEff::ScanByteSequenceV1 {
                base_pointer_operand: 0,
                count_operand: 1,
            },
        )
    }

    pub(in super::super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        predicate: ByteSequencePredicate,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let regs = request(physical, alternative, operands, displacement)?;
        finish(physical, operands, &encode_bytes(predicate, regs)?)
    }

    pub(in super::super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        predicate: ByteSequencePredicate,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        displacement: u32,
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let regs = request(physical, alternative, operands, displacement)?;
        check_canonical(&encode_bytes(predicate, regs)?, bytes)?;
        finish(physical, operands, bytes)
    }
}

pub(super) mod encoded_scalar {
    use super::*;

    fn step_size(step: Step) -> usize {
        match step {
            Step::Zero(_) | Step::Copy(..) | Step::Or(..) | Step::Compare(..) => 3,
            Step::ByteCount(..)
            | Step::SubOne(_)
            | Step::AddOne(_)
            | Step::SignByte(_)
            | Step::ShiftLeft8(_)
            | Step::ShiftRight8(_) => 4,
            Step::LoadByte(..) | Step::ZeroBranch(..) => 9,
            Step::StoreByte(..) => 8,
            Step::TestSigned(..) => 13,
            Step::LessBranch(_) => 6,
            Step::Branch(_) => 5,
        }
    }

    fn assemble(plan: &[Step]) -> Result<Vec<u8>, Error> {
        let mut offset = 0;
        let offsets: Vec<usize> = plan
            .iter()
            .map(|&s| {
                let p = offset;
                offset += step_size(s);
                p
            })
            .collect();
        let mut bytes = Vec::with_capacity(offset);
        let push_jmp = |bytes: &mut Vec<u8>, prefix: &[u8], target: usize| -> Result<(), Error> {
            let pos = *offsets.get(target).ok_or(Error::MalformedEncoding)?;
            bytes.extend_from_slice(prefix);
            let disp = i32::try_from(pos as i64 - (bytes.len() as i64 + 4))
                .map_err(|_| Error::MalformedEncoding)?;
            bytes.extend(disp.to_le_bytes());
            Ok(())
        };
        for &step in plan {
            match step {
                Step::Zero(r) => append_self_clear(&mut bytes, r),
                Step::Copy(dst, src) => rr64(&mut bytes, 0x89, src, dst),
                Step::Or(dst, src) => rr64(&mut bytes, 0x09, src, dst),
                Step::Compare(l, r) => rr64(&mut bytes, 0x39, r, l),
                Step::ByteCount(dst, src) => {
                    bytes.extend([rex64(dst, 0, src), 0x0f, 0xb6, modrm(3, dst, src)])
                }
                Step::SignByte(r) => bytes.extend([rex64(r, 0, r), 0x0f, 0xbe, modrm(3, r, r)]),
                Step::SubOne(r) => bytes.extend([rex64(0, 0, r), 0x83, modrm(3, 5, r), 1]),
                Step::AddOne(r) => bytes.extend([rex64(0, 0, r), 0x83, modrm(3, 0, r), 1]),
                Step::ShiftLeft8(r) => bytes.extend([rex64(0, 0, r), 0xc1, modrm(3, 4, r), 8]),
                Step::ShiftRight8(r) => bytes.extend([rex64(0, 0, r), 0xc1, modrm(3, 5, r), 8]),
                Step::LoadByte(dst, b, i) => append_indexed_byte_load(&mut bytes, dst, b, i, 0),
                Step::StoreByte(src, b, i) => bytes.extend([
                    rex(false, src, i, b),
                    0x88,
                    modrm(2, src, 4),
                    ((i & 7) << 3) | (b & 7),
                    0,
                    0,
                    0,
                    0,
                ]),
                Step::TestSigned(r, target) => {
                    bytes.extend([rex64(0, 0, r), 0xf7, modrm(3, 0, r)]);
                    bytes.extend(0x100u32.to_le_bytes());
                    push_jmp(&mut bytes, &[0x0f, CC_E], target)?;
                }
                Step::ZeroBranch(r, target) => {
                    append_self_test(&mut bytes, r);
                    push_jmp(&mut bytes, &[0x0f, CC_E], target)?;
                }
                Step::LessBranch(target) => push_jmp(&mut bytes, &[0x0f, CC_B], target)?,
                Step::Branch(target) => push_jmp(&mut bytes, &[0xe9], target)?,
            }
        }
        Ok(bytes)
    }

    fn request(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<(bool, [u8; 6]), Error> {
        guard_canonical(physical)?;
        let load = kind == Kind::LoadEncodedScalar;
        let family = if load {
            Family::LoadEncodedScalar
        } else {
            Family::StoreEncodedScalar
        };
        if !matches!(kind, Kind::LoadEncodedScalar | Kind::StoreEncodedScalar)
            || alternative != (MachineAlternativeKey { family, variant: 0 })
        {
            return Err(Error::EncodedFormMismatch);
        }
        let regs: [u8; 6] = resolve_registers(physical, operands)?
            .try_into()
            .map_err(|_| Error::OperandCountMismatch)?;
        let inputs = if load { 2 } else { 3 };
        for scratch in inputs..6 {
            if regs[..scratch].contains(&regs[scratch]) {
                return Err(Error::EncodedFormMismatch);
            }
        }
        Ok((load, regs))
    }

    fn finish(
        physical: &ValidatedPhysicalRegisterModel,
        load: bool,
        operands: &[RegisterViewId],
        bytes: Vec<u8>,
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let inputs = if load { 2 } else { 3 };
        let flags = physical
            .model()
            .view_named("condition_flags")
            .ok_or(Error::NonCanonicalPhysicalModel)?
            .units
            .clone();
        Ok(ValidatedX86_64SelectedFormEncoding {
            bytes,
            footprint: X86_64SelectedFormFootprint {
                register_reads: operands[..inputs].to_vec(),
                register_writes: operands[inputs..].to_vec(),
                writes_rflags: true,
                encoded: MachineEncodedEffects {
                    external_operand_reads: (0..inputs as u16).collect(),
                    external_operand_writes: (inputs as u16..6).collect(),
                    implicit_unit_uses: vec![],
                    implicit_unit_defs: vec![],
                    implicit_unit_clobbers: flags,
                    memory: if load {
                        MemEff::ReadEncodedScalarV1 {
                            pointer_operand: 0,
                            codec_operand: 1,
                        }
                    } else {
                        MemEff::WriteEncodedScalarV1 {
                            pointer_operand: 0,
                            codec_operand: 1,
                        }
                    },
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    control: MachineEncodedControlEffect::FallThroughV1,
                },
            },
        })
    }

    pub(in super::super) fn encode(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (load, regs) = request(physical, kind, alternative, operands)?;
        let bytes = assemble(&encoded_scalar_plan(load, regs, Step::Zero(regs[2])))?;
        finish(physical, load, operands, bytes)
    }

    pub(in super::super) fn validate(
        physical: &ValidatedPhysicalRegisterModel,
        kind: Kind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        bytes: &[u8],
    ) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
        let (load, regs) = request(physical, kind, alternative, operands)?;
        let expected = assemble(&encoded_scalar_plan(load, regs, Step::Zero(regs[2])))?;
        check_canonical(&expected, bytes)?;
        finish(physical, load, operands, bytes.to_vec())
    }
}
