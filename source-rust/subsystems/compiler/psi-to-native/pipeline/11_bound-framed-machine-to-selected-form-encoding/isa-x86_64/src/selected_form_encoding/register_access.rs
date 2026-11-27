pub(crate) use target_x86_64::form_effects::register_access::effects;

use super::emit::{assemble_labeled_stream, modrm, rex};
use super::{
    MachineAlternativeFamily as Family, MachineAlternativeKey, RegisterViewId,
    SelectedInstructionKind, ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
    X86_64SelectedFormEncodingError as Error, X86_64SelectedFormFootprint,
};
use selected_instructions::{
    ControlRegister, GlobalDescriptorTableOperation as DescriptorOp, InterruptControlKind,
    RegisterOperation as Operation, SegmentRegister, SegmentReload, SystemRegister,
};

pub(crate) fn family(operation: Operation) -> Result<Family, Error> {
    target_x86_64::form_effects::register_access::family(operation)
        .ok_or(Error::AlternativeMismatch)
}

mod descriptor {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum Label {
        Refused,
        Resumed,
        ActRefused,
        ActDone,
        Done,
    }

    #[derive(Clone, Copy)]
    pub(super) enum Step {
        Cs,
        Flags,
        Sgdt,
        WriteLimit,
        WriteBase,
        Lidt,
        ClearBusy,
        Stop,
        Selector(u8),
        Load {
            bits: u8,
            base: u8,
            off: u8,
            dst: u8,
        },
        Mov(u8, u8),
        Imm(u8, u32),
        Bin(u8, u8, u8),
        ImmOp(u8, u8, u32),
        Shift(u8, u8, u8),
        ReadCr4,
        Rdmsr,
        Wrmsr,
        LoadSeg(SegmentRegister, u8),
        ReadSeg(SegmentRegister, u8),
        Push(u8),
        Cont(u8, Label),
        Retf,
        Nop,
        Ltr,
        Str(u8),
        Br(u8, Label),
        End,
    }

    fn seg_code(reg: SegmentRegister) -> u8 {
        match reg {
            SegmentRegister::Extra => 0,
            SegmentRegister::Code => 1,
            SegmentRegister::Stack => 2,
            SegmentRegister::Data => 3,
            SegmentRegister::Fs => 4,
            SegmentRegister::Gs => 5,
        }
    }

    fn push_tss_base_check(
        s: &mut Vec<(Option<Label>, Step)>,
        access_mask: Option<u32>,
        access_val: u32,
        flags_mask: Option<u32>,
    ) {
        use Step::*;
        let fail = Br(0x85, Label::Refused);
        s.push((
            None,
            Load {
                bits: 8,
                base: 9,
                off: 5,
                dst: 0,
            },
        ));
        if let Some(m) = access_mask {
            s.push((None, ImmOp(4, 0, m)));
        }
        s.extend([(None, ImmOp(7, 0, access_val)), (None, fail)]);
        s.push((
            None,
            Load {
                bits: 8,
                base: 9,
                off: 6,
                dst: 0,
            },
        ));
        if let Some(m) = flags_mask {
            s.push((None, ImmOp(4, 0, m)));
        }
        s.extend([
            (None, ImmOp(7, 0, 0)),
            (None, fail),
            (
                None,
                Load {
                    bits: 16,
                    base: 9,
                    off: 0,
                    dst: 0,
                },
            ),
            (None, ImmOp(7, 0, 103)),
            (None, fail),
            (
                None,
                Load {
                    bits: 32,
                    base: 9,
                    off: 12,
                    dst: 0,
                },
            ),
            (None, ImmOp(7, 0, 0)),
            (None, fail),
            (
                None,
                Load {
                    bits: 32,
                    base: 9,
                    off: 2,
                    dst: 10,
                },
            ),
            (None, ImmOp(4, 10, 0x00ff_ffff)),
            (
                None,
                Load {
                    bits: 8,
                    base: 9,
                    off: 7,
                    dst: 11,
                },
            ),
            (None, Shift(4, 11, 24)),
            (None, Bin(0x09, 11, 10)),
            (
                None,
                Load {
                    bits: 32,
                    base: 9,
                    off: 8,
                    dst: 11,
                },
            ),
            (None, Shift(4, 11, 32)),
            (None, Bin(0x09, 11, 10)),
            (None, Bin(0x39, 6, 10)),
            (None, fail),
        ]);
    }

    fn preflight(
        desc_bytes: u32,
        canonical_sel: bool,
        reject_cet: bool,
    ) -> Vec<(Option<Label>, Step)> {
        use Step::*;
        let fail = Br(0x85, Label::Refused);
        let mut s = vec![
            (None, Cs),
            (None, ImmOp(4, 0, 3)),
            (None, ImmOp(7, 0, 0)),
            (None, fail),
            (None, Flags),
            (None, ImmOp(4, 0, 0x200)),
            (None, ImmOp(7, 0, 0)),
            (None, fail),
        ];
        if reject_cet {
            s.extend([
                (None, ReadCr4),
                (None, ImmOp(4, 0, 1 << 23)),
                (None, ImmOp(7, 0, 0)),
                (None, fail),
            ]);
        }
        s.extend([
            (None, Sgdt),
            (
                None,
                Load {
                    bits: 16,
                    base: 1,
                    off: 0,
                    dst: 0,
                },
            ),
            (None, ImmOp(0, 0, 1)),
            (None, Bin(0x39, 8, 0)),
            (None, fail),
            (
                None,
                Load {
                    bits: 64,
                    base: 1,
                    off: 2,
                    dst: 0,
                },
            ),
            (None, Bin(0x39, 7, 0)),
            (None, fail),
            (None, Selector(9)),
            (None, Mov(9, 0)),
            (None, ImmOp(4, 0, if canonical_sel { 7 } else { 4 })),
            (None, ImmOp(7, 0, 0)),
            (None, fail),
            (None, ImmOp(4, 9, 0xffff_fff8)),
            (None, ImmOp(7, 9, 0)),
            (None, Br(0x84, Label::Refused)),
            (None, Mov(9, 0)),
            (None, ImmOp(0, 0, desc_bytes)),
            (None, Bin(0x39, 8, 0)),
            (None, Br(0x87, Label::Refused)),
            (None, Bin(0x01, 7, 9)),
        ]);
        s
    }

    pub(super) fn activation() -> Vec<(Option<Label>, Step)> {
        use Step::*;
        let mut s = preflight(16, false, false);
        push_tss_base_check(&mut s, Some(0x9f), 0x89, Some(0xef));
        s.extend([
            (None, Ltr),
            (None, Imm(0, 1)),
            (None, Br(0xe9, Label::Done)),
            (Some(Label::Refused), Imm(0, 0)),
            (Some(Label::Done), End),
        ]);
        s
    }

    fn reload(kind: SegmentReload) -> Vec<(Option<Label>, Step)> {
        use Step::*;
        let mut s = preflight(8, true, kind == SegmentReload::Code);
        let (m5, v5, m6, v6) = match kind {
            SegmentReload::Code => (0xf8, 0x98, 0x60, 0x20),
            SegmentReload::Data => (0xfa, 0x92, 0x20, 0x00),
        };
        s.extend([
            (
                None,
                Load {
                    bits: 8,
                    base: 9,
                    off: 5,
                    dst: 0,
                },
            ),
            (None, ImmOp(4, 0, m5)),
            (None, ImmOp(7, 0, v5)),
            (None, Br(0x85, Label::Refused)),
            (
                None,
                Load {
                    bits: 8,
                    base: 9,
                    off: 6,
                    dst: 0,
                },
            ),
            (None, ImmOp(4, 0, m6)),
            (None, ImmOp(7, 0, v6)),
            (None, Br(0x85, Label::Refused)),
        ]);
        match kind {
            SegmentReload::Code => s.extend([
                (None, Selector(10)),
                (None, Push(10)),
                (None, Cont(10, Label::Resumed)),
                (None, Push(10)),
                (None, Retf),
            ]),
            SegmentReload::Data => {
                s.push((None, Mov(2, 8)));
                for (msr, saved) in [(0xc000_0100, 10), (0xc000_0101, 11)] {
                    s.extend([
                        (None, Imm(1, msr)),
                        (None, Rdmsr),
                        (None, Mov(0, saved)),
                        (None, Shift(4, 2, 32)),
                        (None, Bin(0x09, 2, saved)),
                    ]);
                }
                for reg in [
                    SegmentRegister::Data,
                    SegmentRegister::Extra,
                    SegmentRegister::Stack,
                ] {
                    s.push((None, LoadSeg(reg, 8)));
                }
                s.extend([(None, Nop), (None, Imm(0, 0))]);
                for reg in [SegmentRegister::Fs, SegmentRegister::Gs] {
                    s.push((None, LoadSeg(reg, 0)));
                }
                for (msr, saved) in [(0xc000_0100, 10), (0xc000_0101, 11)] {
                    s.extend([
                        (None, Imm(1, msr)),
                        (None, Mov(saved, 0)),
                        (None, Mov(saved, 2)),
                        (None, Shift(5, 2, 32)),
                        (None, Wrmsr),
                    ]);
                }
            }
        }
        s.extend([
            (Some(Label::Resumed), Imm(0, 1)),
            (None, Br(0xe9, Label::Done)),
            (Some(Label::Refused), Imm(0, 0)),
            (Some(Label::Done), End),
        ]);
        s
    }

    pub(super) fn cpu_state_steps(
        gdt_bytes: u32,
        preserve_sel: bool,
    ) -> Vec<(Option<Label>, Step)> {
        use Step::*;
        let fail = Br(0x85, Label::Refused);
        let mut s = vec![
            (None, Cs),
            (None, ImmOp(4, 0, 3)),
            (None, ImmOp(7, 0, 0)),
            (None, fail),
            (None, Flags),
            (None, ImmOp(4, 0, 0x200)),
            (None, ImmOp(7, 0, 0)),
            (None, fail),
            (None, Sgdt),
            (
                None,
                Load {
                    bits: 16,
                    base: 1,
                    off: 0,
                    dst: 0,
                },
            ),
            (None, ImmOp(7, 0, gdt_bytes - 1)),
            (None, fail),
            (
                None,
                Load {
                    bits: 64,
                    base: 1,
                    off: 2,
                    dst: 0,
                },
            ),
            (None, Bin(0x39, 7, 0)),
            (None, fail),
            (None, Str(9)),
            (None, Mov(9, 0)),
            (None, ImmOp(4, 0, 7)),
            (None, ImmOp(7, 0, 0)),
            (None, fail),
            (None, ImmOp(7, 9, 0)),
            (None, Br(0x84, Label::Refused)),
            (None, Mov(9, 0)),
            (None, ImmOp(0, 0, 16)),
            (None, ImmOp(7, 0, gdt_bytes)),
            (None, Br(0x87, Label::Refused)),
        ];
        if preserve_sel {
            s.push((None, Mov(9, 2)));
        }
        s.push((None, Bin(0x01, 7, 9)));
        push_tss_base_check(&mut s, None, 0x8b, None);
        s.extend([
            (None, Imm(0, 1)),
            (None, Br(0xe9, Label::Done)),
            (Some(Label::Refused), Imm(0, 0)),
            (Some(Label::Done), End),
        ]);
        s
    }

    pub(super) fn emit(bytes: &mut Vec<u8>, step: Step, patches: &mut Vec<(usize, Label)>) {
        let r = |w, reg, base| rex(w, reg, 0, base);
        let rr = |reg, base| modrm(3, reg, base);
        match step {
            Step::ReadCr4 => bytes.extend([0x0f, 0x20, 0xe0]),
            Step::Rdmsr => bytes.extend([0x0f, 0x32]),
            Step::Wrmsr => bytes.extend([0x0f, 0x30]),
            Step::Nop => bytes.push(0x90),
            Step::Retf => bytes.extend([0x48, 0xcb]),
            Step::Push(reg) => {
                if reg >= 8 {
                    bytes.push(0x41);
                }
                bytes.push(0x50 | (reg & 7));
            }
            Step::Cont(reg, lbl) => {
                bytes.extend([r(true, reg, 0), 0x8d, ((reg & 7) << 3) | 5]);
                patches.push((bytes.len(), lbl));
                bytes.extend([0; 4]);
            }
            Step::LoadSeg(seg, src) => {
                bytes.extend([r(false, 0, src), 0x8e, rr(seg_code(seg), src)])
            }
            Step::ReadSeg(seg, dst) => {
                bytes.extend([r(false, 0, dst), 0x8c, rr(seg_code(seg), dst)]);
                bytes.extend([r(false, dst, dst), 0x0f, 0xb7, rr(dst, dst)]);
            }
            Step::Cs => bytes.extend([0x8c, 0xc8]),
            Step::Flags => bytes.extend([0x9c, 0x58]),
            Step::Sgdt => bytes.extend([0x0f, 0x01, 0x01]),
            Step::WriteLimit => bytes.extend([0x66, 0x89, 0x01]),
            Step::WriteBase => bytes.extend([0x48, 0x89, 0x79, 0x02]),
            Step::Lidt => bytes.extend([0x0f, 0x01, 0x19]),
            Step::ClearBusy => bytes.extend([0x41, 0x80, 0x61, 0x05, 0xfd]),
            Step::Stop => bytes.extend(crate::PRIVILEGED_STOP_BYTES),
            Step::Selector(dst) => bytes.extend([r(false, dst, 2), 0x0f, 0xb7, rr(dst, 2)]),
            Step::Load {
                bits,
                base,
                off,
                dst,
            } => {
                bytes.push(r(bits == 64, dst, base));
                match bits {
                    8 => bytes.extend([0x0f, 0xb6]),
                    16 => bytes.extend([0x0f, 0xb7]),
                    _ => bytes.push(0x8b),
                }
                bytes.extend([modrm(1, dst, base), off]);
            }
            Step::Mov(src, dst) => bytes.extend([r(true, src, dst), 0x89, rr(src, dst)]),
            Step::Imm(reg, val) => {
                bytes.extend([r(false, 0, reg), 0xb8 | (reg & 7)]);
                bytes.extend(val.to_le_bytes());
            }
            Step::Bin(op, src, dst) => bytes.extend([r(true, src, dst), op, rr(src, dst)]),
            Step::ImmOp(ext, reg, val) => {
                bytes.extend([r(true, 0, reg), 0x81, rr(ext, reg)]);
                bytes.extend(val.to_le_bytes());
            }
            Step::Shift(ext, reg, cnt) => bytes.extend([r(true, 0, reg), 0xc1, rr(ext, reg), cnt]),
            Step::Ltr => bytes.extend([0x0f, 0x00, 0xda]),
            Step::Str(dst) => {
                bytes.extend([r(false, 0, dst), 0x0f, 0x00, rr(1, dst)]);
                bytes.extend([r(false, dst, dst), 0x0f, 0xb7, rr(dst, dst)]);
            }
            Step::Br(cc, lbl) => {
                if cc == 0xe9 {
                    bytes.push(0xe9);
                } else {
                    bytes.extend([0x0f, cc]);
                }
                patches.push((bytes.len(), lbl));
                bytes.extend([0; 4]);
            }
            Step::End => {}
        }
    }

    pub(super) fn encode(op: Operation, regs: &[u8]) -> Result<Vec<u8>, Error> {
        match (op, regs) {
            (Operation::SegmentReload(kind), [7, 2, 1, 8, 0]) => {
                assemble_labeled_stream(&reload(kind), emit)
            }
            (Operation::SegmentRead(seg), [dst]) if *dst < 16 && *dst != 4 => {
                let mut bytes = Vec::new();
                emit(&mut bytes, Step::ReadSeg(seg, *dst), &mut Vec::new());
                Ok(bytes)
            }
            (Operation::TaskRegisterActivate, [7, 6, 2, 1, 8, 0]) => {
                assemble_labeled_stream(&activation(), emit)
            }
            (Operation::TaskRegisterRead, [dst]) if *dst < 16 && *dst != 4 => {
                let mut bytes = Vec::new();
                emit(&mut bytes, Step::Str(*dst), &mut Vec::new());
                Ok(bytes)
            }
            _ => Err(Error::EncodedFormMismatch),
        }
    }
}

pub(crate) mod installed_table {
    use super::super::emit::assemble_labeled_stream;
    use super::Error as FormError;
    use super::descriptor::{Label, Step, activation, cpu_state_steps, emit};
    use calling_conventions::MachineRegister;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct X86InstalledCpuStateRegisters {
        pub gdt: MachineRegister,
        pub task: MachineRegister,
        pub descriptor_scratch: MachineRegister,
        pub result: MachineRegister,
        pub scratch: [MachineRegister; 3],
    }

    impl X86InstalledCpuStateRegisters {
        pub const FIXED: Self = Self {
            gdt: MachineRegister::X86Rdi,
            task: MachineRegister::X86Rsi,
            descriptor_scratch: MachineRegister::X86Rcx,
            result: MachineRegister::X86Rax,
            scratch: [
                MachineRegister::X86R9,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
            ],
        };
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct X86InstalledCpuStateGeometry {
        pub gdt_bytes: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct X86InstalledTaskCommitRegisters {
        pub cpu: X86InstalledCpuStateRegisters,
        pub selector_scratch: MachineRegister,
        pub extent_scratch: MachineRegister,
    }

    impl X86InstalledTaskCommitRegisters {
        pub const FIXED: Self = Self {
            cpu: X86InstalledCpuStateRegisters::FIXED,
            selector_scratch: MachineRegister::X86Rdx,
            extent_scratch: MachineRegister::X86R8,
        };
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct X86InstalledTablePublicationRegisters {
        pub table: MachineRegister,
        pub descriptor_scratch: MachineRegister,
        pub scratch: MachineRegister,
    }

    impl X86InstalledTablePublicationRegisters {
        pub const FIXED: Self = Self {
            table: MachineRegister::X86Rdi,
            descriptor_scratch: MachineRegister::X86Rcx,
            scratch: MachineRegister::X86Rax,
        };
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum X86InstalledTableEncodingError {
        UnsupportedRegisters,
        InvalidGdtExtent,
        InvalidIdtExtent,
        Encoding(FormError),
        ReplayMismatch,
    }

    impl std::fmt::Display for X86InstalledTableEncodingError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::UnsupportedRegisters => f.write_str("installed table operand registers differ from the explicit fixed-register contract"),
                Self::InvalidGdtExtent => f.write_str("GDT extent must contain whole eight-byte entries and fit its 16-bit limit"),
                Self::InvalidIdtExtent => f.write_str("IDT extent must contain 1 through 256 complete 16-byte gates"),
                Self::Encoding(e) => write!(f, "installed table instruction encoding failed: {e}"),
                Self::ReplayMismatch => f.write_str("installed table bytes differ from the retained operand and geometry checks"),
            }
        }
    }
    impl std::error::Error for X86InstalledTableEncodingError {}
    type Err = X86InstalledTableEncodingError;

    fn check_gdt(g: X86InstalledCpuStateGeometry) -> Result<(), Err> {
        if !(8..=65_536).contains(&g.gdt_bytes) || !g.gdt_bytes.is_multiple_of(8) {
            return Err(Err::InvalidGdtExtent);
        }
        Ok(())
    }

    fn cpu_stream(
        r: X86InstalledCpuStateRegisters,
        g: X86InstalledCpuStateGeometry,
        keep_sel: bool,
    ) -> Result<Vec<(Option<Label>, Step)>, Err> {
        if r != X86InstalledCpuStateRegisters::FIXED {
            return Err(Err::UnsupportedRegisters);
        }
        check_gdt(g)?;
        Ok(cpu_state_steps(g.gdt_bytes, keep_sel))
    }

    fn commit_stream(
        r: X86InstalledTaskCommitRegisters,
        g: X86InstalledCpuStateGeometry,
    ) -> Result<Vec<(Option<Label>, Step)>, Err> {
        if r != X86InstalledTaskCommitRegisters::FIXED {
            return Err(Err::UnsupportedRegisters);
        }
        let mut s = cpu_stream(r.cpu, g, true)?;
        s.truncate(s.len() - 4);
        s.extend([(None, Step::ClearBusy), (None, Step::Imm(8, g.gdt_bytes))]);
        let remap = |l| match l {
            Label::Refused => Label::ActRefused,
            Label::Done => Label::ActDone,
            other => other,
        };
        for (lbl, step) in activation() {
            let step = match step {
                Step::Br(c, l) => Step::Br(c, remap(l)),
                other => other,
            };
            s.push((lbl.map(remap), step));
        }
        s.extend([
            (None, Step::ImmOp(7, 0, 0)),
            (None, Step::Br(0x85, Label::Done)),
            (None, Step::Stop),
            (Some(Label::Refused), Step::Stop),
            (Some(Label::Done), Step::End),
        ]);
        Ok(s)
    }

    fn pub_stream(
        r: X86InstalledTablePublicationRegisters,
        table_bytes: u32,
    ) -> Result<Vec<(Option<Label>, Step)>, Err> {
        if r != X86InstalledTablePublicationRegisters::FIXED {
            return Err(Err::UnsupportedRegisters);
        }
        if !(16..=4096).contains(&table_bytes) || !table_bytes.is_multiple_of(16) {
            return Err(Err::InvalidIdtExtent);
        }
        Ok(vec![
            (None, Step::Imm(0, table_bytes - 1)),
            (None, Step::WriteLimit),
            (None, Step::WriteBase),
            (None, Step::Lidt),
        ])
    }

    pub fn encode_x86_64_installed_cpu_state_check(
        r: X86InstalledCpuStateRegisters,
        g: X86InstalledCpuStateGeometry,
    ) -> Result<Vec<u8>, Err> {
        assemble_labeled_stream(&cpu_stream(r, g, false)?, emit).map_err(Err::Encoding)
    }
    pub fn encode_x86_64_installed_task_commit(
        r: X86InstalledTaskCommitRegisters,
        g: X86InstalledCpuStateGeometry,
    ) -> Result<Vec<u8>, Err> {
        assemble_labeled_stream(&commit_stream(r, g)?, emit).map_err(Err::Encoding)
    }
    pub fn encode_x86_64_installed_table_publication(
        r: X86InstalledTablePublicationRegisters,
        table_bytes: u32,
    ) -> Result<Vec<u8>, Err> {
        assemble_labeled_stream(&pub_stream(r, table_bytes)?, emit).map_err(Err::Encoding)
    }
}

fn mov_reg(bytes: &mut Vec<u8>, src: u8, dst: u8, wide: bool) {
    bytes.extend([rex(wide, src, 0, dst), 0x89, modrm(3, src, dst)]);
}

fn bswap(bytes: &mut Vec<u8>, reg: u8, wide: bool) {
    let prefix = rex(wide, 0, 0, reg);
    if prefix != 0x40 {
        bytes.push(prefix);
    }
    bytes.extend([0x0f, 0xc8 | (reg & 7)]);
}

pub(crate) fn encode(op: Operation, regs: &[u8]) -> Result<Vec<u8>, Error> {
    if matches!(
        op,
        Operation::TaskRegisterActivate
            | Operation::TaskRegisterRead
            | Operation::SegmentReload(_)
            | Operation::SegmentRead(_)
    ) {
        return descriptor::encode(op, regs);
    }
    if let Operation::MemoryFence(kind) = op {
        if !regs.is_empty() {
            return Err(Error::OperandCountMismatch);
        }
        return super::memory_fence::encode(kind).ok_or(Error::AlternativeMismatch);
    }
    family(op)?;
    match op {
        Operation::GlobalDescriptorTable(dir) => {
            let &[ptr] = regs else {
                return Err(Error::OperandCountMismatch);
            };
            if ptr > 15 {
                return Err(Error::EncodedFormMismatch);
            }
            let mut bytes = Vec::new();
            mov_reg(&mut bytes, ptr, 10, true);
            bytes.extend([
                0x41,
                0x0f,
                0x01,
                if dir == DescriptorOp::Store {
                    0x02
                } else {
                    0x12
                },
            ]);
            Ok(bytes)
        }
        Operation::SystemRead(SystemRegister::Control(c))
        | Operation::SystemWrite(SystemRegister::Control(c)) => {
            let &[operand] = regs else {
                return Err(Error::OperandCountMismatch);
            };
            if operand > 15 || (op.produces_value() && operand == 4) || !op.is_admitted() {
                return Err(Error::EncodedFormMismatch);
            }
            let sel = match c {
                ControlRegister::Cr0 => 0,
                ControlRegister::Cr2 => 2,
                ControlRegister::Cr3 => 3,
                ControlRegister::Cr4 => 4,
            };
            let mut bytes = Vec::new();
            if op.produces_value() {
                bytes.extend([0x41, 0x0f, 0x20, 0xc2 | (sel << 3)]);
                mov_reg(&mut bytes, 10, 15, true);
                mov_reg(&mut bytes, 15, operand, true);
            } else {
                mov_reg(&mut bytes, operand, 10, true);
                bytes.extend([0x41, 0x0f, 0x22, 0xc2 | (sel << 3)]);
            }
            Ok(bytes)
        }
        Operation::InterruptControl(k) => {
            if !regs.is_empty() {
                return Err(Error::OperandCountMismatch);
            }
            Ok(vec![if k == InterruptControlKind::Disable {
                0xfa
            } else {
                0xfb
            }])
        }
        Operation::ProcessorHalt => {
            if !regs.is_empty() {
                return Err(Error::OperandCountMismatch);
            }
            Ok(vec![0xf4])
        }
        Operation::SystemRead(SystemRegister::Rflags) => {
            let &[dst] = regs else {
                return Err(Error::OperandCountMismatch);
            };
            if dst > 15 || dst == 4 {
                return Err(Error::EncodedFormMismatch);
            }
            let mut bytes = vec![0x9c, 0x41, 0x5a];
            mov_reg(&mut bytes, 10, 15, true);
            mov_reg(&mut bytes, 15, dst, true);
            Ok(bytes)
        }
        Operation::SystemWrite(SystemRegister::Rflags) => {
            let &[src] = regs else {
                return Err(Error::OperandCountMismatch);
            };
            if src > 15 || src == 4 {
                return Err(Error::EncodedFormMismatch);
            }
            let mut bytes = Vec::new();
            mov_reg(&mut bytes, src, 10, true);
            mov_reg(&mut bytes, 10, 15, true);
            bytes.extend([0x41, 0x57, 0x9d]);
            Ok(bytes)
        }
        Operation::PortRead
        | Operation::PortWrite
        | Operation::ModelSpecificRead
        | Operation::ModelSpecificWrite => {
            let &[first, second] = regs else {
                return Err(Error::OperandCountMismatch);
            };
            if first > 15 || second > 15 || (op.produces_value() && second == 4) {
                return Err(Error::EncodedFormMismatch);
            }
            let mut bytes = Vec::new();
            if !op.produces_value() && second == 10 && first != 10 {
                if first == 11 {
                    mov_reg(&mut bytes, 11, 0, true);
                    mov_reg(&mut bytes, 10, 11, true);
                    mov_reg(&mut bytes, 0, 10, true);
                } else {
                    mov_reg(&mut bytes, 10, 11, true);
                    mov_reg(&mut bytes, first, 10, true);
                }
            } else {
                mov_reg(&mut bytes, first, 10, true);
                if !op.produces_value() {
                    mov_reg(&mut bytes, second, 11, true);
                }
            }
            match op {
                Operation::PortRead => {
                    mov_reg(&mut bytes, 10, 2, false);
                    bytes.extend([
                        0xec,
                        0x48 | ((second >> 3) << 2),
                        0x0f,
                        0xb6,
                        0xc0 | ((second & 7) << 3),
                    ]);
                }
                Operation::PortWrite => {
                    mov_reg(&mut bytes, 10, 2, false);
                    mov_reg(&mut bytes, 11, 0, false);
                    bytes.push(0xee);
                }
                Operation::ModelSpecificRead => {
                    mov_reg(&mut bytes, 10, 1, false);
                    bytes.extend([0x0f, 0x32]);
                    bswap(&mut bytes, 2, false);
                    bswap(&mut bytes, 2, true);
                    bytes.extend([
                        0x48 | ((second >> 3) << 2),
                        0x8d,
                        0x04 | ((second & 7) << 3),
                        0x10,
                    ]);
                }
                Operation::ModelSpecificWrite => {
                    mov_reg(&mut bytes, 10, 1, false);
                    mov_reg(&mut bytes, 11, 2, true);
                    bswap(&mut bytes, 2, true);
                    bswap(&mut bytes, 2, false);
                    mov_reg(&mut bytes, 11, 0, false);
                    bytes.extend([0x0f, 0x30]);
                }
                _ => unreachable!(),
            }
            Ok(bytes)
        }
        _ => Err(Error::AlternativeMismatch),
    }
}

pub(crate) fn footprint(op: Operation, operands: &[RegisterViewId]) -> X86_64SelectedFormFootprint {
    let inputs = op.input_carriers().len();
    X86_64SelectedFormFootprint {
        register_reads: operands[..inputs].to_vec(),
        register_writes: if op.produces_value() {
            vec![operands[inputs]]
        } else {
            Vec::new()
        },
        writes_rflags: matches!(
            op,
            Operation::SystemWrite(SystemRegister::Rflags)
                | Operation::InterruptControl(_)
                | Operation::TaskRegisterActivate
                | Operation::SegmentReload(_)
        ),
        encoded: effects(op),
    }
}

pub(crate) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    op: Operation,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let kind = SelectedInstructionKind::RegisterAccess { operation: op };
    super::request_validation::validate_request(physical, kind, alternative, operands)?;
    let regs = super::request_validation::resolve_scalar_registers(physical, kind, operands)?;
    if bytes != encode(op, &regs)? {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(op, operands),
    })
}
