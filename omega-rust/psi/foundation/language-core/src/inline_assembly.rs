/*
We define compiler-owned source-assembly catalog metadata, scratch-register
views, and register/processor-control/memory-ordering event operations. Source
parsing is target-agnostic because target selection follows syntax construction,
so this catalog makes the accepted instruction shape, target applicability,
authority requirement, operand constraints, machine-state changes, availability,
and register clobbers explicit while keeping recognized instructions without a
complete source contract as refusal entries.
*/

pub use register_operations::{AsmRegisterCarrier, AsmRegisterOperation};
pub use scratch_registers::{
    ASM_SCRATCH_CLOBBERS, ASM_SCRATCH_NAMES, ASM_SCRATCH_SLOT_COUNT, ASM_ZERO_FLAG_SLOT,
    is_unmodeled_aarch64_scratch_view, is_unmodeled_x86_scratch_view, scratch_register,
};

mod register_operations {
    use crate::descriptor_tables::{SegmentRegister, SegmentReload};

    use super::{
        AsmFenceKind, AsmGlobalDescriptorTableOperation, AsmInterruptControlKind,
        AsmSystemRegister, AsmTargetApplicability,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AsmRegisterCarrier {
        U8,
        U16,
        U32,
        U64,
    }

    impl AsmRegisterCarrier {
        pub const fn bits(self) -> u16 {
            match self {
                Self::U8 => 8,
                Self::U16 => 16,
                Self::U32 => 32,
                Self::U64 => 64,
            }
        }
        pub const fn name(self) -> &'static str {
            match self {
                Self::U8 => "u8",
                Self::U16 => "u16",
                Self::U32 => "u32",
                Self::U64 => "u64",
            }
        }
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum AsmRegisterOperation {
        ProcessorHalt, // Stop instruction execution until the architecture's admitted wake event.
        InterruptControl(AsmInterruptControlKind),
        MemoryFence(AsmFenceKind), // Ordering event with exact architecture and access mask retained.
        SystemRead(AsmSystemRegister),
        SystemWrite(AsmSystemRegister),
        PortRead,
        PortWrite,
        ModelSpecificRead,
        ModelSpecificWrite,
        GlobalDescriptorTable(AsmGlobalDescriptorTableOperation), // Native descriptor access; byte loan retained separately.
        TaskRegisterActivate,
        TaskRegisterRead,
        SegmentReload(SegmentReload),
        SegmentRead(SegmentRegister),
    }

    impl AsmRegisterOperation {
        pub const fn catalog_tag(self) -> u16 {
            match self {
                Self::TaskRegisterActivate => 0x602,
                Self::TaskRegisterRead => 0x603,
                Self::SegmentReload(kind) => 0x610 | kind.code() as u16,
                Self::SegmentRead(register) => 0x620 | register.code() as u16,
                Self::GlobalDescriptorTable(AsmGlobalDescriptorTableOperation::Store) => 0x600,
                Self::GlobalDescriptorTable(AsmGlobalDescriptorTableOperation::Load) => 0x601,
                Self::ProcessorHalt => 0x400,
                Self::InterruptControl(AsmInterruptControlKind::Disable) => 0x401,
                Self::InterruptControl(AsmInterruptControlKind::Enable) => 0x402,
                Self::MemoryFence(AsmFenceKind::Load) => 0x500,
                Self::MemoryFence(AsmFenceKind::Store) => 0x501,
                Self::MemoryFence(AsmFenceKind::Full) => 0x502,
                Self::MemoryFence(AsmFenceKind::Aarch64Load) => 0x503,
                Self::MemoryFence(AsmFenceKind::Aarch64Store) => 0x504,
                Self::MemoryFence(AsmFenceKind::Aarch64Full) => 0x505,
                Self::SystemRead(register) => register.catalog_id() as u16,
                Self::SystemWrite(register) => 0x100 | register.catalog_id() as u16,
                Self::PortRead => 0x200,
                Self::PortWrite => 0x201,
                Self::ModelSpecificRead => 0x300,
                Self::ModelSpecificWrite => 0x301,
            }
        }

        pub const fn from_catalog_tag(tag: u16) -> Option<Self> {
            match tag {
                0x602 => return Some(Self::TaskRegisterActivate),
                0x603 => return Some(Self::TaskRegisterRead),
                0x610..=0x611 => {
                    return match SegmentReload::from_code((tag & 0xf) as u8) {
                        Some(kind) => Some(Self::SegmentReload(kind)),
                        None => None,
                    };
                }
                0x620..=0x625 => {
                    return match SegmentRegister::from_code((tag & 0xf) as u8) {
                        Some(register) => Some(Self::SegmentRead(register)),
                        None => None,
                    };
                }
                0x600 => {
                    return Some(Self::GlobalDescriptorTable(
                        AsmGlobalDescriptorTableOperation::Store,
                    ));
                }
                0x601 => {
                    return Some(Self::GlobalDescriptorTable(
                        AsmGlobalDescriptorTableOperation::Load,
                    ));
                }
                0x400 => return Some(Self::ProcessorHalt),
                0x401 => return Some(Self::InterruptControl(AsmInterruptControlKind::Disable)),
                0x402 => return Some(Self::InterruptControl(AsmInterruptControlKind::Enable)),
                0x500 => return Some(Self::MemoryFence(AsmFenceKind::Load)),
                0x501 => return Some(Self::MemoryFence(AsmFenceKind::Store)),
                0x502 => return Some(Self::MemoryFence(AsmFenceKind::Full)),
                0x503 => return Some(Self::MemoryFence(AsmFenceKind::Aarch64Load)),
                0x504 => return Some(Self::MemoryFence(AsmFenceKind::Aarch64Store)),
                0x505 => return Some(Self::MemoryFence(AsmFenceKind::Aarch64Full)),
                0x200 => return Some(Self::PortRead),
                0x201 => return Some(Self::PortWrite),
                0x300 => return Some(Self::ModelSpecificRead),
                0x301 => return Some(Self::ModelSpecificWrite),
                _ => {}
            }
            let Some(register) = AsmSystemRegister::from_catalog_id((tag & 0xff) as u8) else {
                return None;
            };
            match tag >> 8 {
                0 => Some(Self::SystemRead(register)),
                1 if register.write_intrinsic_name().is_some() => Some(Self::SystemWrite(register)),
                _ => None,
            }
        }

        pub fn from_intrinsic_name(name: &str) -> Option<Self> {
            match name {
                "asm#hlt" => return Some(Self::ProcessorHalt),
                "asm#port_in" => return Some(Self::PortRead),
                "asm#port_out" => return Some(Self::PortWrite),
                "asm#rdmsr" => return Some(Self::ModelSpecificRead),
                "asm#wrmsr" => return Some(Self::ModelSpecificWrite),
                _ => {}
            }
            if let Some(kind) = AsmInterruptControlKind::from_intrinsic_name(name) {
                return Some(Self::InterruptControl(kind));
            }
            if let Some(kind) = AsmFenceKind::from_intrinsic_name(name) {
                return Some(Self::MemoryFence(kind));
            }
            AsmSystemRegister::from_read_intrinsic_name(name)
                .map(Self::SystemRead)
                .or_else(|| {
                    AsmSystemRegister::from_write_intrinsic_name(name).map(Self::SystemWrite)
                })
        }

        pub const fn requires_service_authority(self) -> bool {
            !matches!(
                self,
                Self::SystemRead(AsmSystemRegister::Rflags) | Self::MemoryFence(_)
            )
        }

        pub const fn produces_value(self) -> bool {
            self.result_carrier().is_some()
        }

        pub const fn input_carriers(self) -> &'static [AsmRegisterCarrier] {
            use AsmRegisterCarrier::*;
            match self {
                Self::TaskRegisterActivate => &[U64, U64, U16, U64, U64],
                Self::TaskRegisterRead | Self::SegmentRead(_) => &[],
                Self::SegmentReload(_) => &[U64, U16, U64, U64],
                Self::SystemRead(_)
                | Self::ProcessorHalt
                | Self::InterruptControl(_)
                | Self::MemoryFence(_) => &[],
                Self::SystemWrite(_) | Self::GlobalDescriptorTable(_) => &[U64],
                Self::PortRead => &[U16],
                Self::PortWrite => &[U16, U8],
                Self::ModelSpecificRead => &[U32],
                Self::ModelSpecificWrite => &[U32, U64],
            }
        }

        pub const fn result_carrier(self) -> Option<AsmRegisterCarrier> {
            match self {
                Self::SystemRead(_) | Self::ModelSpecificRead => Some(AsmRegisterCarrier::U64),
                Self::PortRead | Self::TaskRegisterActivate | Self::SegmentReload(_) => {
                    Some(AsmRegisterCarrier::U8)
                }
                Self::TaskRegisterRead | Self::SegmentRead(_) => Some(AsmRegisterCarrier::U16),
                _ => None,
            }
        }

        pub const fn target_applicability(self) -> AsmTargetApplicability {
            match self {
                Self::SystemRead(register) | Self::SystemWrite(register) => {
                    register.target_applicability()
                }
                Self::MemoryFence(kind) => kind.target(),
                _ => AsmTargetApplicability::X86_64,
            }
        }

        pub const fn is_admitted(self) -> bool {
            match self {
                Self::SystemRead(_)
                | Self::PortRead
                | Self::PortWrite
                | Self::ModelSpecificRead
                | Self::ProcessorHalt
                | Self::InterruptControl(_)
                | Self::MemoryFence(_)
                | Self::ModelSpecificWrite
                | Self::GlobalDescriptorTable(_)
                | Self::TaskRegisterActivate
                | Self::TaskRegisterRead
                | Self::SegmentReload(_)
                | Self::SegmentRead(_) => true,
                Self::SystemWrite(register) => register.write_intrinsic_name().is_some(),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn control_register_events_preserve_closed_catalog_identity_and_authority() {
            use crate::inline_assembly::AsmControlRegister;
            for control in [
                AsmControlRegister::Cr0,
                AsmControlRegister::Cr2,
                AsmControlRegister::Cr3,
                AsmControlRegister::Cr4,
            ] {
                let register = AsmSystemRegister::Control(control);
                let read = AsmRegisterOperation::SystemRead(register);
                assert_eq!(
                    AsmRegisterOperation::from_intrinsic_name(control.read_intrinsic_name()),
                    Some(read)
                );
                assert_eq!(
                    AsmRegisterOperation::from_catalog_tag(read.catalog_tag()),
                    Some(read)
                );
                assert!(read.requires_service_authority());
                assert_eq!(read.result_carrier(), Some(AsmRegisterCarrier::U64));
                assert!(read.input_carriers().is_empty());
                let write = AsmRegisterOperation::SystemWrite(register);
                assert_eq!(
                    write.is_admitted(),
                    control.write_intrinsic_name().is_some()
                );
                assert_eq!(
                    AsmRegisterOperation::from_catalog_tag(write.catalog_tag()),
                    write.is_admitted().then_some(write)
                );
                if let Some(name) = control.write_intrinsic_name() {
                    assert_eq!(AsmRegisterOperation::from_intrinsic_name(name), Some(write));
                    assert_eq!(write.input_carriers(), &[AsmRegisterCarrier::U64]);
                    assert_eq!(write.result_carrier(), None);
                    assert!(write.requires_service_authority());
                }
            }
        }

        #[test]
        fn x86_register_signatures_retain_exact_carriers_and_catalog_identity() {
            use AsmRegisterCarrier::*;
            for (name, operation, inputs, result) in [
                (
                    "asm#hlt",
                    AsmRegisterOperation::ProcessorHalt,
                    &[][..],
                    None,
                ),
                (
                    "asm#cli",
                    AsmRegisterOperation::InterruptControl(AsmInterruptControlKind::Disable),
                    &[][..],
                    None,
                ),
                (
                    "asm#sti",
                    AsmRegisterOperation::InterruptControl(AsmInterruptControlKind::Enable),
                    &[][..],
                    None,
                ),
                (
                    "asm#port_in",
                    AsmRegisterOperation::PortRead,
                    &[U16][..],
                    Some(U8),
                ),
                (
                    "asm#port_out",
                    AsmRegisterOperation::PortWrite,
                    &[U16, U8][..],
                    None,
                ),
                (
                    "asm#rdmsr",
                    AsmRegisterOperation::ModelSpecificRead,
                    &[U32][..],
                    Some(U64),
                ),
                (
                    "asm#wrmsr",
                    AsmRegisterOperation::ModelSpecificWrite,
                    &[U32, U64][..],
                    None,
                ),
            ] {
                assert_eq!(
                    AsmRegisterOperation::from_intrinsic_name(name),
                    Some(operation)
                );
                assert_eq!(
                    AsmRegisterOperation::from_catalog_tag(operation.catalog_tag()),
                    Some(operation)
                );
                assert_eq!(operation.input_carriers(), inputs);
                assert_eq!(operation.result_carrier(), result);
                assert_eq!(
                    operation.target_applicability(),
                    AsmTargetApplicability::X86_64
                );
            }
            for tag in [0x202, 0x302, 0x403, u16::MAX] {
                assert_eq!(AsmRegisterOperation::from_catalog_tag(tag), None);
            }
        }
    }
}

mod scratch_registers {
    pub const ASM_SCRATCH_SLOT_COUNT: usize = 51;
    pub const ASM_ZERO_FLAG_SLOT: usize = 14;
    pub const ASM_SCRATCH_NAMES: [&str; ASM_SCRATCH_SLOT_COUNT] = [
        "eax",
        "ebx",
        "ecx",
        "edx",
        "esi",
        "edi",
        "r8d",
        "r9d",
        "r10d",
        "r11d",
        "r12d",
        "r13d",
        "r14d",
        "r15d",
        "zf",
        "asm#flags",
        "asm#flags_environment",
        "asm#sub_left",
        "asm#sub_right",
        "asm#flags_mask",
        "asm#flags_snapshot",
        "asm#comparison_result",
        "w0",
        "w1",
        "w2",
        "w3",
        "w4",
        "w5",
        "w6",
        "w7",
        "w8",
        "w9",
        "w10",
        "w11",
        "w12",
        "w13",
        "w14",
        "w15",
        "w16",
        "w17",
        "w18",
        "w19",
        "w20",
        "w21",
        "w22",
        "w23",
        "w24",
        "w25",
        "w26",
        "w27",
        "w28",
    ];
    pub const ASM_SCRATCH_CLOBBERS: [&str; ASM_ZERO_FLAG_SLOT] = [
        "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13", "r14",
        "r15",
    ];

    pub fn is_unmodeled_x86_scratch_view(name: &str) -> bool {
        matches!(
            name,
            "rax"
                | "rbx"
                | "rcx"
                | "rdx"
                | "rsi"
                | "rdi"
                | "rsp"
                | "rbp"
                | "esp"
                | "ebp"
                | "ax"
                | "bx"
                | "cx"
                | "dx"
                | "si"
                | "di"
                | "sp"
                | "bp"
                | "al"
                | "ah"
                | "bl"
                | "bh"
                | "cl"
                | "ch"
                | "dl"
                | "dh"
                | "sil"
                | "dil"
                | "spl"
                | "bpl"
                | "r8"
                | "r9"
                | "r10"
                | "r11"
                | "r12"
                | "r13"
                | "r14"
                | "r15"
                | "r8w"
                | "r9w"
                | "r10w"
                | "r11w"
                | "r12w"
                | "r13w"
                | "r14w"
                | "r15w"
                | "r8b"
                | "r9b"
                | "r10b"
                | "r11b"
                | "r12b"
                | "r13b"
                | "r14b"
                | "r15b"
                | "rflags"
                | "eflags"
                | "flags"
                | "cf"
                | "of"
                | "sf"
                | "af"
                | "pf"
        )
    }

    pub const ASM_ARM_REGISTER_START: usize = 22;
    pub const ASM_ARM_CLOBBERS: [&str; 29] = [
        "x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8", "x9", "x10", "x11", "x12", "x13",
        "x14", "x15", "x16", "x17", "x18", "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26",
        "x27", "x28",
    ];

    pub fn scratch_register(
        name: &str,
    ) -> Option<(usize, super::AsmTargetApplicability, &'static str)> {
        ASM_SCRATCH_NAMES
            .iter()
            .position(|candidate| *candidate == name)
            .and_then(|index| {
                if index < ASM_ZERO_FLAG_SLOT {
                    Some((
                        index,
                        super::AsmTargetApplicability::X86_64,
                        ASM_SCRATCH_CLOBBERS[index],
                    ))
                } else if index >= ASM_ARM_REGISTER_START {
                    Some((
                        index,
                        super::AsmTargetApplicability::Aarch64,
                        ASM_ARM_CLOBBERS[index - ASM_ARM_REGISTER_START],
                    ))
                } else {
                    None
                }
            })
    }

    pub fn is_unmodeled_aarch64_scratch_view(name: &str) -> bool {
        matches!(name, "wsp" | "xzr" | "wzr" | "x31" | "w31" | "w29" | "w30")
            || name
                .strip_prefix('x')
                .is_some_and(|number| number.parse::<u8>().is_ok_and(|register| register <= 30))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInstructionAvailability {
    UserChecked,
    DeriverOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInstructionShape {
    JumpState,
    Subtract,             // Modular 32-bit x86 subtraction; establishes arithmetic flags.
    SubtractThreeOperand, // AArch64 SUBS over W-registers; establishes NZCV.
    Add,                  // Modular 32-bit x86 addition; establishes arithmetic flags.
    Compare,              // Compare modeled u32 registers or fitting immediate.
    JumpZeroFlag { when_set: bool },
    Halt,
    PortOut,
    PortIn,
    RegisterMove, // Data move between writable Omega place and readable value.
    MemoryTransfer(AsmMemoryTransferKind),
    MemoryFence(AsmFenceKind),
    InterruptControl(AsmInterruptControlKind),
    FlagsSnapshot,
    FlagsRestore,
    MsrRead,
    MsrWrite,
    ControlRegisterRead(AsmControlRegister),
    ControlRegisterWrite(AsmControlRegister),
    SystemRegisterRead(AsmSystemRegister),
    SystemRegisterWrite(AsmSystemRegister),
    InstructionSerialization(AsmInstructionSerializationKind),
    SchedulingHint(AsmSchedulingHintKind),
    CacheOperation(AsmCacheOperationKind),
    DescriptorTableLoad,
    GlobalDescriptorTable(AsmGlobalDescriptorTableOperation),
    DerivedExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AsmGlobalDescriptorTableOperation {
    Store,
    Load,
}

impl AsmGlobalDescriptorTableOperation {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Store => "sgdt",
            Self::Load => "lgdt",
        }
    }
    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::Store => "asm#sgdt",
            Self::Load => "asm#lgdt",
        }
    }
    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        match name {
            "asm#sgdt" => Some(Self::Store),
            "asm#lgdt" => Some(Self::Load),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmMemoryTransferKind {
    Load,  // `ldr destination, place`
    Store, // `str value, place`
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInstructionSerializationKind {
    Serialize,                         // x86_64 `serialize`
    InstructionSynchronizationBarrier, // AArch64 `isb`
}

impl AsmInstructionSerializationKind {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Serialize => "serialize",
            Self::InstructionSynchronizationBarrier => "isb",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::Serialize => "asm#serialize",
            Self::InstructionSynchronizationBarrier => "asm#isb",
        }
    }

    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        [Self::Serialize, Self::InstructionSynchronizationBarrier]
            .into_iter()
            .find(|kind| kind.intrinsic_name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmCacheOperationKind {
    WriteBackInvalidate,   // x86_64 `wbinvd`
    Invalidate,            // x86_64 `invd`
    WriteBackNoInvalidate, // x86_64 `wbnoinvd`
}

impl AsmCacheOperationKind {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::WriteBackInvalidate => "wbinvd",
            Self::Invalidate => "invd",
            Self::WriteBackNoInvalidate => "wbnoinvd",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::WriteBackInvalidate => "asm#wbinvd",
            Self::Invalidate => "asm#invd",
            Self::WriteBackNoInvalidate => "asm#wbnoinvd",
        }
    }

    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        [
            Self::WriteBackInvalidate,
            Self::Invalidate,
            Self::WriteBackNoInvalidate,
        ]
        .into_iter()
        .find(|kind| kind.intrinsic_name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmSchedulingHintKind {
    SpinPause,        // x86_64 `pause`
    Yield,            // AArch64 `yield`
    Nop,              // `nop` on both supported ISAs
    WaitForEvent,     // AArch64 `wfe`
    WaitForInterrupt, // AArch64 `wfi`
    SendEvent,        // AArch64 `sev`
    SendEventLocal,   // AArch64 `sevl`
}

impl AsmSchedulingHintKind {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::SpinPause => "pause",
            Self::Yield => "yield",
            Self::Nop => "nop",
            Self::WaitForEvent => "wfe",
            Self::WaitForInterrupt => "wfi",
            Self::SendEvent => "sev",
            Self::SendEventLocal => "sevl",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::SpinPause => "asm#pause",
            Self::Yield => "asm#yield",
            Self::Nop => "asm#nop",
            Self::WaitForEvent => "asm#wfe",
            Self::WaitForInterrupt => "asm#wfi",
            Self::SendEvent => "asm#sev",
            Self::SendEventLocal => "asm#sevl",
        }
    }

    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        [
            Self::SpinPause,
            Self::Yield,
            Self::Nop,
            Self::WaitForEvent,
            Self::WaitForInterrupt,
            Self::SendEvent,
            Self::SendEventLocal,
        ]
        .into_iter()
        .find(|kind| kind.intrinsic_name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AsmControlRegister {
    Cr0,
    Cr2,
    Cr3,
    Cr4,
}

impl AsmControlRegister {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cr0 => "cr0",
            Self::Cr2 => "cr2",
            Self::Cr3 => "cr3",
            Self::Cr4 => "cr4",
        }
    }

    pub const fn read_mnemonic(self) -> &'static str {
        match self {
            Self::Cr0 => "read_cr0",
            Self::Cr2 => "read_cr2",
            Self::Cr3 => "read_cr3",
            Self::Cr4 => "read_cr4",
        }
    }

    pub const fn write_mnemonic(self) -> Option<&'static str> {
        match self {
            Self::Cr0 => Some("write_cr0"),
            Self::Cr2 => None,
            Self::Cr3 => Some("write_cr3"),
            Self::Cr4 => Some("write_cr4"),
        }
    }

    pub const fn read_intrinsic_name(self) -> &'static str {
        match self {
            Self::Cr0 => "asm#read_cr0",
            Self::Cr2 => "asm#read_cr2",
            Self::Cr3 => "asm#read_cr3",
            Self::Cr4 => "asm#read_cr4",
        }
    }

    pub const fn write_intrinsic_name(self) -> Option<&'static str> {
        match self {
            Self::Cr0 => Some("asm#write_cr0"),
            Self::Cr2 => None,
            Self::Cr3 => Some("asm#write_cr3"),
            Self::Cr4 => Some("asm#write_cr4"),
        }
    }

    pub fn from_read_mnemonic(name: &str) -> Option<Self> {
        [Self::Cr0, Self::Cr2, Self::Cr3, Self::Cr4]
            .into_iter()
            .find(|register| register.read_mnemonic() == name)
    }

    pub fn from_write_mnemonic(name: &str) -> Option<Self> {
        [Self::Cr0, Self::Cr3, Self::Cr4]
            .into_iter()
            .find(|register| register.write_mnemonic() == Some(name))
    }

    pub fn from_read_intrinsic_name(name: &str) -> Option<Self> {
        [Self::Cr0, Self::Cr2, Self::Cr3, Self::Cr4]
            .into_iter()
            .find(|register| register.read_intrinsic_name() == name)
    }

    pub fn from_write_intrinsic_name(name: &str) -> Option<Self> {
        [Self::Cr0, Self::Cr3, Self::Cr4]
            .into_iter()
            .find(|register| register.write_intrinsic_name() == Some(name))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AsmSystemRegister {
    Control(AsmControlRegister),
    Rflags,
    SctlrEl1,
    TcrEl1,
    Ttbr0El1,
    Ttbr1El1,
    MairEl1,
    VbarEl1,
    TpidrEl1,
    EsrEl1,
    FarEl1,
}

impl AsmSystemRegister {
    pub const fn target_applicability(self) -> AsmTargetApplicability {
        match self {
            Self::Rflags | Self::Control(_) => AsmTargetApplicability::X86_64,
            _ => AsmTargetApplicability::Aarch64,
        }
    }

    pub const fn catalog_id(self) -> u8 {
        match self {
            Self::Control(AsmControlRegister::Cr0) => 10,
            Self::Control(AsmControlRegister::Cr2) => 11,
            Self::Control(AsmControlRegister::Cr3) => 12,
            Self::Control(AsmControlRegister::Cr4) => 13,
            Self::SctlrEl1 => 0,
            Self::TcrEl1 => 1,
            Self::Ttbr0El1 => 2,
            Self::Ttbr1El1 => 3,
            Self::MairEl1 => 4,
            Self::VbarEl1 => 5,
            Self::TpidrEl1 => 6,
            Self::EsrEl1 => 7,
            Self::FarEl1 => 8,
            Self::Rflags => 9,
        }
    }

    pub const fn from_catalog_id(identity: u8) -> Option<Self> {
        match identity {
            10 => Some(Self::Control(AsmControlRegister::Cr0)),
            11 => Some(Self::Control(AsmControlRegister::Cr2)),
            12 => Some(Self::Control(AsmControlRegister::Cr3)),
            13 => Some(Self::Control(AsmControlRegister::Cr4)),
            0 => Some(Self::SctlrEl1),
            1 => Some(Self::TcrEl1),
            2 => Some(Self::Ttbr0El1),
            3 => Some(Self::Ttbr1El1),
            4 => Some(Self::MairEl1),
            5 => Some(Self::VbarEl1),
            6 => Some(Self::TpidrEl1),
            7 => Some(Self::EsrEl1),
            8 => Some(Self::FarEl1),
            9 => Some(Self::Rflags),
            _ => None,
        }
    }

    const ALL: [Self; 14] = [
        Self::Control(AsmControlRegister::Cr0),
        Self::Control(AsmControlRegister::Cr2),
        Self::Control(AsmControlRegister::Cr3),
        Self::Control(AsmControlRegister::Cr4),
        Self::Rflags,
        Self::SctlrEl1,
        Self::TcrEl1,
        Self::Ttbr0El1,
        Self::Ttbr1El1,
        Self::MairEl1,
        Self::VbarEl1,
        Self::TpidrEl1,
        Self::EsrEl1,
        Self::FarEl1,
    ];

    const WRITABLE: [Self; 11] = [
        Self::Control(AsmControlRegister::Cr0),
        Self::Control(AsmControlRegister::Cr3),
        Self::Control(AsmControlRegister::Cr4),
        Self::Rflags,
        Self::SctlrEl1,
        Self::TcrEl1,
        Self::Ttbr0El1,
        Self::Ttbr1El1,
        Self::MairEl1,
        Self::VbarEl1,
        Self::TpidrEl1,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Control(register) => register.name(),
            Self::SctlrEl1 => "sctlr_el1",
            Self::TcrEl1 => "tcr_el1",
            Self::Ttbr0El1 => "ttbr0_el1",
            Self::Ttbr1El1 => "ttbr1_el1",
            Self::MairEl1 => "mair_el1",
            Self::VbarEl1 => "vbar_el1",
            Self::TpidrEl1 => "tpidr_el1",
            Self::EsrEl1 => "esr_el1",
            Self::FarEl1 => "far_el1",
            Self::Rflags => "rflags",
        }
    }

    pub const fn read_mnemonic(self) -> &'static str {
        match self {
            Self::Control(register) => register.read_mnemonic(),
            Self::SctlrEl1 => "read_sctlr_el1",
            Self::TcrEl1 => "read_tcr_el1",
            Self::Ttbr0El1 => "read_ttbr0_el1",
            Self::Ttbr1El1 => "read_ttbr1_el1",
            Self::MairEl1 => "read_mair_el1",
            Self::VbarEl1 => "read_vbar_el1",
            Self::TpidrEl1 => "read_tpidr_el1",
            Self::EsrEl1 => "read_esr_el1",
            Self::FarEl1 => "read_far_el1",
            Self::Rflags => "pushfq",
        }
    }

    pub const fn write_mnemonic(self) -> Option<&'static str> {
        match self {
            Self::Control(register) => register.write_mnemonic(),
            Self::SctlrEl1 => Some("write_sctlr_el1"),
            Self::TcrEl1 => Some("write_tcr_el1"),
            Self::Ttbr0El1 => Some("write_ttbr0_el1"),
            Self::Ttbr1El1 => Some("write_ttbr1_el1"),
            Self::MairEl1 => Some("write_mair_el1"),
            Self::VbarEl1 => Some("write_vbar_el1"),
            Self::TpidrEl1 => Some("write_tpidr_el1"),
            Self::Rflags => Some("popfq"),
            Self::EsrEl1 | Self::FarEl1 => None,
        }
    }

    pub const fn read_intrinsic_name(self) -> &'static str {
        match self {
            Self::Control(register) => register.read_intrinsic_name(),
            Self::SctlrEl1 => "asm#read_sctlr_el1",
            Self::TcrEl1 => "asm#read_tcr_el1",
            Self::Ttbr0El1 => "asm#read_ttbr0_el1",
            Self::Ttbr1El1 => "asm#read_ttbr1_el1",
            Self::MairEl1 => "asm#read_mair_el1",
            Self::VbarEl1 => "asm#read_vbar_el1",
            Self::TpidrEl1 => "asm#read_tpidr_el1",
            Self::EsrEl1 => "asm#read_esr_el1",
            Self::FarEl1 => "asm#read_far_el1",
            Self::Rflags => "asm#pushfq",
        }
    }

    pub const fn write_intrinsic_name(self) -> Option<&'static str> {
        match self {
            Self::Control(register) => register.write_intrinsic_name(),
            Self::SctlrEl1 => Some("asm#write_sctlr_el1"),
            Self::TcrEl1 => Some("asm#write_tcr_el1"),
            Self::Ttbr0El1 => Some("asm#write_ttbr0_el1"),
            Self::Ttbr1El1 => Some("asm#write_ttbr1_el1"),
            Self::MairEl1 => Some("asm#write_mair_el1"),
            Self::VbarEl1 => Some("asm#write_vbar_el1"),
            Self::TpidrEl1 => Some("asm#write_tpidr_el1"),
            Self::Rflags => Some("asm#popfq"),
            Self::EsrEl1 | Self::FarEl1 => None,
        }
    }

    pub fn from_read_mnemonic(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|register| register.read_mnemonic() == name)
    }

    pub fn from_write_mnemonic(name: &str) -> Option<Self> {
        Self::WRITABLE
            .into_iter()
            .find(|register| register.write_mnemonic() == Some(name))
    }

    pub fn from_read_intrinsic_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|register| register.read_intrinsic_name() == name)
    }

    pub fn from_write_intrinsic_name(name: &str) -> Option<Self> {
        Self::WRITABLE
            .into_iter()
            .find(|register| register.write_intrinsic_name() == Some(name))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum AsmTargetApplicability {
    #[default]
    Any,
    X86_64,
    Aarch64,
}

impl AsmTargetApplicability {
    pub fn intersect(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Any, target) | (target, Self::Any) => Some(target),
            (left, right) if left == right => Some(left),
            _ => None,
        }
    }

    pub fn accepts_target_name(self, target: &str) -> bool {
        match self {
            Self::Any => true,
            Self::X86_64 => matches!(
                target,
                "linux_x86_64" | "windows_x86_64" | "macos_x86_64" | "uefi_x86_64"
            ),
            Self::Aarch64 => matches!(target, "linux_arm64" | "macos_arm64"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmAuthorityRequirement {
    None,
    MachineOwner,
    PortIo,
    IdtControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct AsmAuthorityAdmission {
    machine_owner: bool,
    port_io: bool,
    idt_control: bool,
}

impl AsmAuthorityAdmission {
    pub const HOSTED: Self = Self {
        machine_owner: false,
        port_io: false,
        idt_control: false,
    };

    pub const MACHINE_OWNER: Self = Self {
        machine_owner: true,
        port_io: true,
        idt_control: true,
    };

    pub const fn from_freestanding(freestanding: bool) -> Self {
        if freestanding {
            Self::MACHINE_OWNER
        } else {
            Self::HOSTED
        }
    }

    pub const fn with_grants(self, port_io: bool, interrupt_table: bool) -> Self {
        Self {
            machine_owner: self.machine_owner,
            port_io: self.port_io || port_io,
            idt_control: self.idt_control || interrupt_table,
        }
    }

    pub const fn admits(&self, requirement: AsmAuthorityRequirement) -> bool {
        match requirement {
            AsmAuthorityRequirement::None => true,
            AsmAuthorityRequirement::MachineOwner => self.machine_owner,
            AsmAuthorityRequirement::PortIo => self.port_io,
            AsmAuthorityRequirement::IdtControl => self.idt_control,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AsmFenceKind {
    Load,
    Store,
    Full,
    Aarch64Load,
    Aarch64Store,
    Aarch64Full,
}

impl AsmFenceKind {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Load => "lfence",
            Self::Store => "sfence",
            Self::Full => "mfence",
            Self::Aarch64Load | Self::Aarch64Store | Self::Aarch64Full => "dmb",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::Load => "asm#lfence",
            Self::Store => "asm#sfence",
            Self::Full => "asm#mfence",
            Self::Aarch64Load => "asm#dmb_ld",
            Self::Aarch64Store => "asm#dmb_st",
            Self::Aarch64Full => "asm#dmb_sy",
        }
    }

    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.intrinsic_name() == name)
    }

    pub const ALL: [Self; 6] = [
        Self::Load,
        Self::Store,
        Self::Full,
        Self::Aarch64Load,
        Self::Aarch64Store,
        Self::Aarch64Full,
    ];

    pub const fn target(self) -> AsmTargetApplicability {
        match self {
            Self::Load | Self::Store | Self::Full => AsmTargetApplicability::X86_64,
            _ => AsmTargetApplicability::Aarch64,
        }
    }

    pub fn from_aarch64_option(option: &str) -> Option<Self> {
        match option {
            "ld" => Some(Self::Aarch64Load),
            "st" => Some(Self::Aarch64Store),
            "sy" => Some(Self::Aarch64Full),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmMemoryOrdering {
    None,
    Fence(AsmFenceKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AsmInterruptControlKind {
    Disable,
    Enable,
}

impl AsmInterruptControlKind {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Disable => "cli",
            Self::Enable => "sti",
        }
    }

    pub const fn intrinsic_name(self) -> &'static str {
        match self {
            Self::Disable => "asm#cli",
            Self::Enable => "asm#sti",
        }
    }

    pub fn from_intrinsic_name(name: &str) -> Option<Self> {
        [Self::Disable, Self::Enable]
            .into_iter()
            .find(|kind| kind.intrinsic_name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInterruptFlagEffect {
    None,
    Disable,
    EnableAfterNextInstruction,
    RestoreFromOperand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmFlagsDataFlow {
    None,
    ArithmeticResult,
    ZeroFlagBranch,
    SnapshotToOperand,
    RestoreFromOperand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmOperandAccess {
    Read,
    ReadPlace,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsmOperandConstraint {
    pub role: &'static str, // Source-facing role used in diagnostics (`port`, `value`, ...).
    pub target_register: &'static str, // Exact architectural register the realized sequence presents.
    pub access: AsmOperandAccess,
    pub expected_type_name: &'static str,
    pub maximum_literal: Option<u64>, // Literals are admitted only when this bound is present.
}

impl AsmOperandConstraint {
    pub const fn read(
        role: &'static str,
        target_register: &'static str,
        expected_type_name: &'static str,
        maximum_literal: u64,
    ) -> Self {
        Self {
            role,
            target_register,
            access: AsmOperandAccess::Read,
            expected_type_name,
            maximum_literal: Some(maximum_literal),
        }
    }

    pub const fn write_place(
        role: &'static str,
        target_register: &'static str,
        expected_type_name: &'static str,
    ) -> Self {
        Self {
            role,
            target_register,
            access: AsmOperandAccess::Write,
            expected_type_name,
            maximum_literal: None,
        }
    }

    pub const fn read_place(
        role: &'static str,
        target_register: &'static str,
        expected_type_name: &'static str,
    ) -> Self {
        Self {
            role,
            target_register,
            access: AsmOperandAccess::ReadPlace,
            expected_type_name,
            maximum_literal: None,
        }
    }

    pub const fn expected_type_name(self) -> &'static str {
        self.expected_type_name
    }

    pub const fn maximum_literal(self) -> Option<u64> {
        self.maximum_literal
    }

    pub const fn requires_writable_place(self) -> bool {
        matches!(self.access, AsmOperandAccess::Write)
    }

    pub const fn requires_place(self) -> bool {
        matches!(
            self.access,
            AsmOperandAccess::ReadPlace | AsmOperandAccess::Write
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsmInstructionContract {
    pub availability: AsmInstructionAvailability,
    pub shape: AsmInstructionShape,
    pub target: AsmTargetApplicability,
    pub required_authority: AsmAuthorityRequirement,
    pub operands: &'static [AsmOperandConstraint], // Fixed source-order target-register constraints.
    pub memory_ordering: AsmMemoryOrdering,        // Ordering established by the instruction.
    pub interrupt_flag_effect: AsmInterruptFlagEffect, // Architectural interrupt-flag transition.
    pub flags_data_flow: AsmFlagsDataFlow,         // Explicit RFLAGS value flow.
    pub clobbers: &'static [&'static str], // Registers changed by the realized instruction sequence.
}

impl AsmInstructionContract {
    fn without_side_effects(shape: AsmInstructionShape, target: AsmTargetApplicability) -> Self {
        Self {
            availability: AsmInstructionAvailability::UserChecked,
            shape,
            target,
            required_authority: AsmAuthorityRequirement::None,
            operands: NO_OPERANDS,
            memory_ordering: AsmMemoryOrdering::None,
            interrupt_flag_effect: AsmInterruptFlagEffect::None,
            flags_data_flow: AsmFlagsDataFlow::None,
            clobbers: NO_CLOBBERS,
        }
    }

    fn machine_authority(
        shape: AsmInstructionShape,
        target: AsmTargetApplicability,
        operands: &'static [AsmOperandConstraint],
        clobbers: &'static [&'static str],
    ) -> Self {
        Self {
            availability: AsmInstructionAvailability::UserChecked,
            shape,
            target,
            required_authority: AsmAuthorityRequirement::MachineOwner,
            operands,
            memory_ordering: AsmMemoryOrdering::None,
            interrupt_flag_effect: AsmInterruptFlagEffect::None,
            flags_data_flow: AsmFlagsDataFlow::None,
            clobbers,
        }
    }

    pub fn memory_fence(kind: AsmFenceKind) -> Self {
        Self {
            availability: AsmInstructionAvailability::UserChecked,
            shape: AsmInstructionShape::MemoryFence(kind),
            target: kind.target(),
            required_authority: AsmAuthorityRequirement::None,
            operands: NO_OPERANDS,
            memory_ordering: AsmMemoryOrdering::Fence(kind),
            interrupt_flag_effect: AsmInterruptFlagEffect::None,
            flags_data_flow: AsmFlagsDataFlow::None,
            clobbers: NO_CLOBBERS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmInstructionRefusal {
    HiddenControlExit, // A return, call, or indirect branch would bypass Omega state edges.
    UnmodeledMemoryAccess, // No provenance/permission-bearing operand contract exists yet.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsmCatalogEntry {
    Contract(AsmInstructionContract),
    Refused(AsmInstructionRefusal),
}

const NO_OPERANDS: &[AsmOperandConstraint] = &[];
const PORT_OUT_OPERANDS: &[AsmOperandConstraint] = &[
    AsmOperandConstraint::read("port", "dx", "u16", u16::MAX as u64),
    AsmOperandConstraint::read("value", "al", "u8", u8::MAX as u64),
];
const PORT_IN_OPERANDS: &[AsmOperandConstraint] = &[
    AsmOperandConstraint::write_place("destination", "al", "u8"),
    AsmOperandConstraint::read("port", "dx", "u16", u16::MAX as u64),
];
const FLAGS_SNAPSHOT_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::write_place(
    "destination",
    "rflags",
    "u64",
)];
const FLAGS_RESTORE_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read_place(
    "saved flags",
    "rflags",
    "u64",
)];
const MSR_READ_OPERANDS: &[AsmOperandConstraint] = &[
    AsmOperandConstraint::write_place("destination", "edx:eax", "u64"),
    AsmOperandConstraint::read("MSR index", "ecx", "u32", u32::MAX as u64),
];
const MSR_WRITE_OPERANDS: &[AsmOperandConstraint] = &[
    AsmOperandConstraint::read("MSR index", "ecx", "u32", u32::MAX as u64),
    AsmOperandConstraint::read("value", "edx:eax", "u64", u64::MAX),
];

const fn register_destination_operand(target_register: &'static str) -> AsmOperandConstraint {
    AsmOperandConstraint::write_place("destination", target_register, "u64")
}

const fn register_value_operand(target_register: &'static str) -> AsmOperandConstraint {
    AsmOperandConstraint::read("value", target_register, "u64", u64::MAX)
}

const CR0_READ_OPERANDS: &[AsmOperandConstraint] = &[register_destination_operand("cr0")];
const CR2_READ_OPERANDS: &[AsmOperandConstraint] = &[register_destination_operand("cr2")];
const CR3_READ_OPERANDS: &[AsmOperandConstraint] = &[register_destination_operand("cr3")];
const CR4_READ_OPERANDS: &[AsmOperandConstraint] = &[register_destination_operand("cr4")];
const CR0_WRITE_OPERANDS: &[AsmOperandConstraint] = &[register_value_operand("cr0")];
const CR3_WRITE_OPERANDS: &[AsmOperandConstraint] = &[register_value_operand("cr3")];
const CR4_WRITE_OPERANDS: &[AsmOperandConstraint] = &[register_value_operand("cr4")];
const SCTLR_EL1_READ_OPERANDS: &[AsmOperandConstraint] =
    &[register_destination_operand("sctlr_el1")];
const TCR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[register_destination_operand("tcr_el1")];
const TTBR0_EL1_READ_OPERANDS: &[AsmOperandConstraint] =
    &[register_destination_operand("ttbr0_el1")];
const TTBR1_EL1_READ_OPERANDS: &[AsmOperandConstraint] =
    &[register_destination_operand("ttbr1_el1")];
const MAIR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[register_destination_operand("mair_el1")];
const VBAR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[register_destination_operand("vbar_el1")];
const TPIDR_EL1_READ_OPERANDS: &[AsmOperandConstraint] =
    &[register_destination_operand("tpidr_el1")];
const ESR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[register_destination_operand("esr_el1")];
const FAR_EL1_READ_OPERANDS: &[AsmOperandConstraint] = &[register_destination_operand("far_el1")];
const SCTLR_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[register_value_operand("sctlr_el1")];
const TCR_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[register_value_operand("tcr_el1")];
const TTBR0_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[register_value_operand("ttbr0_el1")];
const TTBR1_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[register_value_operand("ttbr1_el1")];
const MAIR_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[register_value_operand("mair_el1")];
const VBAR_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[register_value_operand("vbar_el1")];
const TPIDR_EL1_WRITE_OPERANDS: &[AsmOperandConstraint] = &[register_value_operand("tpidr_el1")];
const NO_CLOBBERS: &[&str] = &[];
const PORT_OUT_CLOBBERS: &[&str] = &["rax", "rdx", "r10", "r11", "r15"];
const PORT_IN_CLOBBERS: &[&str] = &["rax", "rdx", "r10", "r15"];
const FLAGS_OPERAND_CLOBBERS: &[&str] = &["r10", "r15"];
const MSR_READ_CLOBBERS: &[&str] = &["rax", "rcx", "rdx", "r10", "r11", "r15"];
const MSR_WRITE_CLOBBERS: &[&str] = &["rax", "rcx", "rdx", "r10", "r11", "r15"];
const CONTROL_REGISTER_READ_CLOBBERS: &[&str] = &["r10", "r15"];
const CONTROL_REGISTER_WRITE_CLOBBERS: &[&str] = &["rax", "r10", "r11", "r15"];
const SYSTEM_REGISTER_READ_CLOBBERS: &[&str] = &["x9", "x15"];
const SYSTEM_REGISTER_WRITE_CLOBBERS: &[&str] = &["x9", "x10", "x11", "x15"];
const GDTR_STORE_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read_place(
    "destination",
    "r10",
    "&write [u8; 10]",
)];
const GDTR_LOAD_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read_place(
    "descriptor",
    "r10",
    "&[u8; 10]",
)];
const IDT_DESCRIPTOR_OPERANDS: &[AsmOperandConstraint] = &[AsmOperandConstraint::read_place(
    "IDT descriptor",
    "r10",
    "IdtDescriptor",
)];
const IDT_DESCRIPTOR_CLOBBERS: &[&str] = &["r10"];

pub fn asm_intrinsic_mnemonic(intrinsic_name: &str) -> Option<&'static str> {
    if let Some(register) = AsmControlRegister::from_read_intrinsic_name(intrinsic_name) {
        return Some(register.read_mnemonic());
    }
    if let Some(register) = AsmControlRegister::from_write_intrinsic_name(intrinsic_name) {
        return register.write_mnemonic();
    }
    if let Some(register) = AsmSystemRegister::from_read_intrinsic_name(intrinsic_name) {
        return Some(register.read_mnemonic());
    }
    if let Some(register) = AsmSystemRegister::from_write_intrinsic_name(intrinsic_name) {
        return register.write_mnemonic();
    }
    if let Some(kind) = AsmFenceKind::from_intrinsic_name(intrinsic_name) {
        return Some(kind.mnemonic());
    }
    if let Some(kind) = AsmInterruptControlKind::from_intrinsic_name(intrinsic_name) {
        return Some(kind.mnemonic());
    }
    if let Some(kind) = AsmInstructionSerializationKind::from_intrinsic_name(intrinsic_name) {
        return Some(kind.mnemonic());
    }
    if let Some(kind) = AsmSchedulingHintKind::from_intrinsic_name(intrinsic_name) {
        return Some(kind.mnemonic());
    }
    if let Some(kind) = AsmCacheOperationKind::from_intrinsic_name(intrinsic_name) {
        return Some(kind.mnemonic());
    }
    match intrinsic_name {
        "asm#hlt" => Some("hlt"),
        "asm#sgdt" => Some("sgdt"),
        "asm#lgdt" => Some("lgdt"),
        "asm#port_out" => Some("out"),
        "asm#port_in" => Some("in"),
        "asm#pushfq" => Some("pushfq"),
        "asm#popfq" => Some("popfq"),
        "asm#rdmsr" => Some("rdmsr"),
        "asm#wrmsr" => Some("wrmsr"),
        _ => None,
    }
}

pub fn asm_intrinsic_retains_flag_place(intrinsic_name: &str) -> bool {
    let Some(mnemonic) = asm_intrinsic_mnemonic(intrinsic_name) else {
        return false;
    };
    matches!(
        asm_catalog_entry(mnemonic),
        Some(AsmCatalogEntry::Contract(contract))
            if matches!(
                contract.flags_data_flow,
                AsmFlagsDataFlow::SnapshotToOperand | AsmFlagsDataFlow::RestoreFromOperand
            )
    )
}

pub fn asm_catalog_entry(mnemonic: &str) -> Option<AsmCatalogEntry> {
    use AsmAuthorityRequirement::{
        IdtControl as IdtControlAuthority, MachineOwner, None as NoAuthority,
        PortIo as PortIoAuthority,
    };
    use AsmCatalogEntry::{Contract, Refused};
    use AsmFenceKind::{Full, Load, Store};
    use AsmFlagsDataFlow::{
        None as NoFlagsDataFlow, RestoreFromOperand as RestoreFlags,
        SnapshotToOperand as SnapshotFlags,
    };
    use AsmInstructionAvailability::{DeriverOnly, UserChecked};
    use AsmInstructionRefusal::{HiddenControlExit, UnmodeledMemoryAccess};
    use AsmInstructionSerializationKind::{InstructionSynchronizationBarrier, Serialize};
    use AsmInstructionShape::{
        CacheOperation, DerivedExit, DescriptorTableLoad, FlagsRestore, FlagsSnapshot, Halt,
        InstructionSerialization, InterruptControl, JumpState, MemoryTransfer, MsrRead, MsrWrite,
        PortIn, PortOut, RegisterMove, SchedulingHint,
    };
    use AsmInterruptControlKind::{Disable, Enable};
    use AsmInterruptFlagEffect::{
        Disable as DisablesInterrupts, EnableAfterNextInstruction, None as NoInterruptChange,
        RestoreFromOperand as RestoreInterruptFlag,
    };
    use AsmMemoryOrdering::None as NoOrdering;
    use AsmSchedulingHintKind::{
        Nop, SendEvent, SendEventLocal, SpinPause, WaitForEvent, WaitForInterrupt, Yield,
    };
    use AsmTargetApplicability::{Aarch64, Any, X86_64};

    if let Some(register) = AsmControlRegister::from_read_mnemonic(mnemonic) {
        let operands = match register {
            AsmControlRegister::Cr0 => CR0_READ_OPERANDS,
            AsmControlRegister::Cr2 => CR2_READ_OPERANDS,
            AsmControlRegister::Cr3 => CR3_READ_OPERANDS,
            AsmControlRegister::Cr4 => CR4_READ_OPERANDS,
        };
        return Some(Contract(AsmInstructionContract::machine_authority(
            AsmInstructionShape::ControlRegisterRead(register),
            X86_64,
            operands,
            CONTROL_REGISTER_READ_CLOBBERS,
        )));
    }
    if let Some(register) = AsmControlRegister::from_write_mnemonic(mnemonic) {
        let operands = match register {
            AsmControlRegister::Cr0 => CR0_WRITE_OPERANDS,
            AsmControlRegister::Cr2 => unreachable!("CR2 has no source write form"),
            AsmControlRegister::Cr3 => CR3_WRITE_OPERANDS,
            AsmControlRegister::Cr4 => CR4_WRITE_OPERANDS,
        };
        return Some(Contract(AsmInstructionContract::machine_authority(
            AsmInstructionShape::ControlRegisterWrite(register),
            X86_64,
            operands,
            CONTROL_REGISTER_WRITE_CLOBBERS,
        )));
    }
    if let Some(register) = AsmSystemRegister::from_read_mnemonic(mnemonic) {
        if register == AsmSystemRegister::Rflags {
            return Some(Contract(AsmInstructionContract {
                availability: UserChecked,
                shape: FlagsSnapshot,
                target: X86_64,
                required_authority: NoAuthority,
                operands: FLAGS_SNAPSHOT_OPERANDS,
                memory_ordering: NoOrdering,
                interrupt_flag_effect: NoInterruptChange,
                flags_data_flow: SnapshotFlags,
                clobbers: FLAGS_OPERAND_CLOBBERS,
            }));
        }

        let operands = match register {
            AsmSystemRegister::SctlrEl1 => SCTLR_EL1_READ_OPERANDS,
            AsmSystemRegister::TcrEl1 => TCR_EL1_READ_OPERANDS,
            AsmSystemRegister::Ttbr0El1 => TTBR0_EL1_READ_OPERANDS,
            AsmSystemRegister::Ttbr1El1 => TTBR1_EL1_READ_OPERANDS,
            AsmSystemRegister::MairEl1 => MAIR_EL1_READ_OPERANDS,
            AsmSystemRegister::VbarEl1 => VBAR_EL1_READ_OPERANDS,
            AsmSystemRegister::TpidrEl1 => TPIDR_EL1_READ_OPERANDS,
            AsmSystemRegister::EsrEl1 => ESR_EL1_READ_OPERANDS,
            AsmSystemRegister::FarEl1 => FAR_EL1_READ_OPERANDS,
            AsmSystemRegister::Rflags | AsmSystemRegister::Control(_) => return None,
        };
        return Some(Contract(AsmInstructionContract::machine_authority(
            AsmInstructionShape::SystemRegisterRead(register),
            Aarch64,
            operands,
            SYSTEM_REGISTER_READ_CLOBBERS,
        )));
    }
    if let Some(register) = AsmSystemRegister::from_write_mnemonic(mnemonic)
        && register != AsmSystemRegister::Rflags
    {
        let operands = match register {
            AsmSystemRegister::SctlrEl1 => SCTLR_EL1_WRITE_OPERANDS,
            AsmSystemRegister::TcrEl1 => TCR_EL1_WRITE_OPERANDS,
            AsmSystemRegister::Ttbr0El1 => TTBR0_EL1_WRITE_OPERANDS,
            AsmSystemRegister::Ttbr1El1 => TTBR1_EL1_WRITE_OPERANDS,
            AsmSystemRegister::MairEl1 => MAIR_EL1_WRITE_OPERANDS,
            AsmSystemRegister::VbarEl1 => VBAR_EL1_WRITE_OPERANDS,
            AsmSystemRegister::TpidrEl1 => TPIDR_EL1_WRITE_OPERANDS,
            AsmSystemRegister::Rflags => {
                unreachable!("rflags restore keeps the popfq contract")
            }
            AsmSystemRegister::EsrEl1
            | AsmSystemRegister::FarEl1
            | AsmSystemRegister::Control(_) => {
                return None;
            }
        };
        return Some(Contract(AsmInstructionContract::machine_authority(
            AsmInstructionShape::SystemRegisterWrite(register),
            Aarch64,
            operands,
            SYSTEM_REGISTER_WRITE_CLOBBERS,
        )));
    }

    let entry = match mnemonic {
        "sub" => Contract(AsmInstructionContract {
            flags_data_flow: AsmFlagsDataFlow::ArithmeticResult,
            clobbers: &["rflags"],
            ..AsmInstructionContract::without_side_effects(AsmInstructionShape::Subtract, X86_64)
        }),
        "subs" => Contract(AsmInstructionContract {
            flags_data_flow: AsmFlagsDataFlow::ArithmeticResult,
            clobbers: &["nzcv"],
            ..AsmInstructionContract::without_side_effects(
                AsmInstructionShape::SubtractThreeOperand,
                Aarch64,
            )
        }),
        "cmp" => Contract(AsmInstructionContract {
            flags_data_flow: AsmFlagsDataFlow::ArithmeticResult,
            ..AsmInstructionContract::without_side_effects(AsmInstructionShape::Compare, Any)
        }),
        "b" => Contract(AsmInstructionContract::without_side_effects(
            AsmInstructionShape::JumpState,
            Aarch64,
        )),
        "b.eq" | "b.ne" => Contract(AsmInstructionContract {
            flags_data_flow: AsmFlagsDataFlow::ZeroFlagBranch,
            ..AsmInstructionContract::without_side_effects(
                AsmInstructionShape::JumpZeroFlag {
                    when_set: mnemonic == "b.eq",
                },
                Aarch64,
            )
        }),
        "add" => Contract(AsmInstructionContract {
            flags_data_flow: AsmFlagsDataFlow::ArithmeticResult,
            clobbers: &["rflags"],
            ..AsmInstructionContract::without_side_effects(AsmInstructionShape::Add, X86_64)
        }),
        "jnz" | "jz" => Contract(AsmInstructionContract {
            flags_data_flow: AsmFlagsDataFlow::ZeroFlagBranch,
            ..AsmInstructionContract::without_side_effects(
                AsmInstructionShape::JumpZeroFlag {
                    when_set: mnemonic == "jz",
                },
                X86_64,
            )
        }),
        "jmp" => Contract(AsmInstructionContract::without_side_effects(JumpState, Any)),
        "hlt" => Contract(AsmInstructionContract::machine_authority(
            Halt,
            Any,
            NO_OPERANDS,
            NO_CLOBBERS,
        )),
        "out" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: PortOut,
            target: X86_64,
            required_authority: PortIoAuthority,
            operands: PORT_OUT_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: PORT_OUT_CLOBBERS,
        }),
        "in" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: PortIn,
            target: X86_64,
            required_authority: PortIoAuthority,
            operands: PORT_IN_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: PORT_IN_CLOBBERS,
        }),
        "lfence" => Contract(AsmInstructionContract::memory_fence(Load)),
        "sfence" => Contract(AsmInstructionContract::memory_fence(Store)),
        "mfence" => Contract(AsmInstructionContract::memory_fence(Full)),
        "dmb" => Contract(AsmInstructionContract::memory_fence(
            AsmFenceKind::Aarch64Full,
        )),
        "cli" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: InterruptControl(Disable),
            target: X86_64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: DisablesInterrupts,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "sti" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: InterruptControl(Enable),
            target: X86_64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: EnableAfterNextInstruction,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "popfq" => Contract(AsmInstructionContract {
            availability: UserChecked,
            shape: FlagsRestore,
            target: X86_64,
            required_authority: MachineOwner,
            operands: FLAGS_RESTORE_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: RestoreInterruptFlag,
            flags_data_flow: RestoreFlags,
            clobbers: FLAGS_OPERAND_CLOBBERS,
        }),
        "rdmsr" => Contract(AsmInstructionContract::machine_authority(
            MsrRead,
            X86_64,
            MSR_READ_OPERANDS,
            MSR_READ_CLOBBERS,
        )),
        "wrmsr" => Contract(AsmInstructionContract::machine_authority(
            MsrWrite,
            X86_64,
            MSR_WRITE_OPERANDS,
            MSR_WRITE_CLOBBERS,
        )),
        "serialize" => Contract(AsmInstructionContract::without_side_effects(
            InstructionSerialization(Serialize),
            X86_64,
        )),
        "isb" => Contract(AsmInstructionContract::without_side_effects(
            InstructionSerialization(InstructionSynchronizationBarrier),
            Aarch64,
        )),
        "pause" => Contract(AsmInstructionContract::without_side_effects(
            SchedulingHint(SpinPause),
            X86_64,
        )),
        "yield" => Contract(AsmInstructionContract::without_side_effects(
            SchedulingHint(Yield),
            Aarch64,
        )),
        "nop" => Contract(AsmInstructionContract::without_side_effects(
            SchedulingHint(Nop),
            Any,
        )),
        "wfe" => Contract(AsmInstructionContract::without_side_effects(
            SchedulingHint(WaitForEvent),
            Aarch64,
        )),
        "wfi" => Contract(AsmInstructionContract::without_side_effects(
            SchedulingHint(WaitForInterrupt),
            Aarch64,
        )),
        "sev" => Contract(AsmInstructionContract::without_side_effects(
            SchedulingHint(SendEvent),
            Aarch64,
        )),
        "sevl" => Contract(AsmInstructionContract::without_side_effects(
            SchedulingHint(SendEventLocal),
            Aarch64,
        )),
        "wbinvd" => Contract(AsmInstructionContract::machine_authority(
            CacheOperation(AsmCacheOperationKind::WriteBackInvalidate),
            X86_64,
            NO_OPERANDS,
            NO_CLOBBERS,
        )),
        "invd" => Contract(AsmInstructionContract::machine_authority(
            CacheOperation(AsmCacheOperationKind::Invalidate),
            X86_64,
            NO_OPERANDS,
            NO_CLOBBERS,
        )),
        "wbnoinvd" => Contract(AsmInstructionContract::machine_authority(
            CacheOperation(AsmCacheOperationKind::WriteBackNoInvalidate),
            X86_64,
            NO_OPERANDS,
            NO_CLOBBERS,
        )),
        "sgdt" | "lgdt" => Contract(AsmInstructionContract::machine_authority(
            AsmInstructionShape::GlobalDescriptorTable(if mnemonic == "sgdt" {
                AsmGlobalDescriptorTableOperation::Store
            } else {
                AsmGlobalDescriptorTableOperation::Load
            }),
            X86_64,
            if mnemonic == "sgdt" {
                GDTR_STORE_OPERANDS
            } else {
                GDTR_LOAD_OPERANDS
            },
            IDT_DESCRIPTOR_CLOBBERS,
        )),
        "lidt" => Contract(AsmInstructionContract {
            availability: DeriverOnly,
            shape: DescriptorTableLoad,
            target: X86_64,
            required_authority: IdtControlAuthority,
            operands: IDT_DESCRIPTOR_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: IDT_DESCRIPTOR_CLOBBERS,
        }),
        "iretq" | "sysret" | "sysretq" => Contract(AsmInstructionContract {
            availability: DeriverOnly,
            shape: DerivedExit,
            target: X86_64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "eret" => Contract(AsmInstructionContract {
            availability: DeriverOnly,
            shape: DerivedExit,
            target: Aarch64,
            required_authority: MachineOwner,
            operands: NO_OPERANDS,
            memory_ordering: NoOrdering,
            interrupt_flag_effect: NoInterruptChange,
            flags_data_flow: NoFlagsDataFlow,
            clobbers: NO_CLOBBERS,
        }),
        "ret" | "retq" | "retn" | "retw" | "retaa" | "retab" | "retf" | "lret" | "iret"
        | "iretd" | "iretw" | "call" | "callq" | "callf" | "lcall" | "jmpq" | "jmpf" | "jmpl"
        | "ljmp" | "ljmpl" | "br" | "blr" | "bl" | "bx" | "blx" | "bc" | "braa" | "brab"
        | "braaz" | "brabz" | "blraa" | "blrab" | "blraaz" | "blrabz" | "eretaa" | "eretab"
        | "drps" | "cbz" | "cbnz" | "tbz" | "tbnz" | "loop" | "loope" | "loopne" | "loopz"
        | "loopnz" | "jcxz" | "jecxz" | "jrcxz" | "int" | "int1" | "int3" | "into" | "je"
        | "jne" | "ja" | "jae" | "jb" | "jbe" | "jna" | "jnae" | "jnb" | "jnbe" | "jg" | "jge"
        | "jl" | "jle" | "jng" | "jnge" | "jnl" | "jnle" | "jo" | "jno" | "js" | "jns" | "jp"
        | "jpe" | "jnp" | "jpo" | "jc" | "jnc" => Refused(HiddenControlExit),
        "mov" | "movq" => Contract(AsmInstructionContract::without_side_effects(
            RegisterMove,
            Any,
        )),
        "ldr" => Contract(AsmInstructionContract::without_side_effects(
            MemoryTransfer(AsmMemoryTransferKind::Load),
            Aarch64,
        )),
        "str" => Contract(AsmInstructionContract::without_side_effects(
            MemoryTransfer(AsmMemoryTransferKind::Store),
            Aarch64,
        )),
        "ldp" | "stp" | "ldpsw" | "ldnp" | "stnp" | "push" | "pop" | "pushq" | "popq" | "pushw"
        | "pushl" | "pushf" | "pushfd" | "pusha" | "pushal" | "pushad" | "popa" | "popal"
        | "popad" | "popw" | "popl" | "popf" | "popfd" | "enter" | "leave" | "ldrb" | "ldrh"
        | "ldrsb" | "ldrsh" | "ldrsw" | "strb" | "strh" | "ldur" | "stur" | "ldurb" | "ldurh"
        | "ldursb" | "ldursh" | "ldursw" | "sturb" | "sturh" | "ldtr" | "ldtrb" | "ldtrh"
        | "ldtrsb" | "ldtrsh" | "ldtrsw" | "sttr" | "sttrb" | "sttrh" | "ldapr" | "ldaprb"
        | "ldaprh" | "ldaprsb" | "ldaprsh" | "ldaprsw" | "ldapur" | "ldapurb" | "ldapurh"
        | "ldapursb" | "ldapursh" | "ldapursw" | "stlur" | "stlurb" | "stlurh" | "ldlar"
        | "ldlarb" | "ldlarh" | "stllr" | "stllrb" | "stllrh" | "ldxr" | "ldxrb" | "ldxrh"
        | "stxr" | "stxrb" | "stxrh" | "ldax" | "ldaxr" | "ldaxrb" | "ldaxrh" | "stlxr"
        | "stlxrb" | "stlxrh" | "ldxp" | "stxp" | "ldaxp" | "stlxp" | "ldar" | "ldarb"
        | "ldarh" | "stlr" | "stlrb" | "stlrh" | "ld64b" | "st64b" | "st64bv" | "st64bv0"
        | "ld1" | "st1" | "ld2" | "st2" | "ld3" | "st3" | "ld4" | "st4" | "ld1r" | "ld2r"
        | "ld3r" | "ld4r" | "swp" | "swpb" | "swph" | "swpa" | "swpal" | "swpl" | "swpab"
        | "swpah" | "swpalb" | "swpalh" | "swplb" | "swplh" | "cas" | "casb" | "cash" | "casa"
        | "casal" | "casl" | "casab" | "casah" | "caslb" | "caslh" | "casalb" | "casalh"
        | "casp" | "caspa" | "caspal" | "caspl" | "ldadd" | "ldaddb" | "ldaddh" | "ldadda"
        | "ldaddab" | "ldaddah" | "ldaddl" | "ldaddlb" | "ldaddlh" | "ldaddal" | "ldaddalb"
        | "ldaddalh" | "ldclr" | "ldclrb" | "ldclrh" | "ldclra" | "ldclrab" | "ldclrah"
        | "ldclrl" | "ldclrlb" | "ldclrlh" | "ldclral" | "ldclralb" | "ldclralh" | "ldeor"
        | "ldeorb" | "ldeorh" | "ldeora" | "ldeorab" | "ldeorah" | "ldeorl" | "ldeorlb"
        | "ldeorlh" | "ldeoral" | "ldeoralb" | "ldeoralh" | "ldset" | "ldsetb" | "ldseth"
        | "ldseta" | "ldsetab" | "ldsetah" | "ldsetl" | "ldsetlb" | "ldsetlh" | "ldsetal"
        | "ldsetalb" | "ldsetalh" | "ldsmax" | "ldsmaxb" | "ldsmaxh" | "ldsmaxa" | "ldsmaxab"
        | "ldsmaxah" | "ldsmaxl" | "ldsmaxlb" | "ldsmaxlh" | "ldsmaxal" | "ldsmaxalb"
        | "ldsmaxalh" | "ldsmin" | "ldsminb" | "ldsminh" | "ldsmina" | "ldsminab" | "ldsminah"
        | "ldsminl" | "ldsminlb" | "ldsminlh" | "ldsminal" | "ldsminalb" | "ldsminalh"
        | "ldumax" | "ldumaxb" | "ldumaxh" | "ldumaxa" | "ldumaxab" | "ldumaxah" | "ldumaxl"
        | "ldumaxlb" | "ldumaxlh" | "ldumaxal" | "ldumaxalb" | "ldumaxalh" | "ldumin"
        | "lduminb" | "lduminh" | "ldumina" | "lduminab" | "lduminah" | "lduminl" | "lduminlb"
        | "lduminlh" | "lduminal" | "lduminalb" | "lduminalh" | "stadd" | "staddb" | "staddh"
        | "staddl" | "staddlb" | "staddlh" | "stclr" | "stclrb" | "stclrh" | "stclrl"
        | "stclrlb" | "stclrlh" | "steor" | "steorb" | "steorh" | "steorl" | "steorlb"
        | "steorlh" | "stset" | "stsetb" | "stseth" | "stsetl" | "stsetlb" | "stsetlh"
        | "stsmax" | "stsmaxb" | "stsmaxh" | "stsmaxl" | "stsmaxlb" | "stsmaxlh" | "stsmin"
        | "stsminb" | "stsminh" | "stsminl" | "stsminlb" | "stsminlh" | "stumax" | "stumaxb"
        | "stumaxh" | "stumaxl" | "stumaxlb" | "stumaxlh" | "stumin" | "stuminb" | "stuminh"
        | "stuminl" | "stuminlb" | "stuminlh" | "xchg" | "xadd" | "cmpxchg" | "cmpxchg8b"
        | "cmpxchg16b" | "xlat" | "xlatb" | "lds" | "les" | "lss" | "lfs" | "lgs" | "sidt"
        | "movnti" | "movntq" | "movntdq" | "movntdqa" | "bound" | "fxsave" | "fxrstor"
        | "xsave" | "xsavec" | "xsaves" | "xsaveopt" | "xrstor" | "xrstors" | "movs" | "movsb"
        | "movsw" | "movsq" | "lods" | "lodsb" | "lodsw" | "lodsq" | "lodsd" | "stos" | "stosb"
        | "stosw" | "stosq" | "stosd" | "scas" | "scasb" | "scasw" | "scasq" | "scasd" | "cmps"
        | "cmpsb" | "cmpsw" | "cmpsq" | "ins" | "outs" | "insb" | "insw" | "insd" | "outsb"
        | "outsw" | "outsd" => Refused(UnmodeledMemoryAccess),
        _ => return None,
    };
    Some(entry)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86FlagsEnvironment {
    rflags: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86FlagsEnvironmentError;

impl core::fmt::Display for X86FlagsEnvironmentError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("invalid x86 flags environment: reserved RFLAGS bits")
    }
}

impl X86FlagsEnvironment {
    pub fn new(rflags: u64) -> Result<Self, X86FlagsEnvironmentError> {
        if rflags & 2 == 0 || rflags & !0x3f7fd7 != 0 {
            return Err(X86FlagsEnvironmentError);
        }
        Ok(Self { rflags })
    }

    pub fn snapshot(self) -> u64 {
        self.rflags & !((1 << 16) | (1 << 17))
    }

    pub fn restore_arithmetic_bits(&mut self, image: u64) {
        const ARITHMETIC: u64 = 0x8d5;
        self.rflags = (self.rflags & !ARITHMETIC) | (image & ARITHMETIC);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AsmAuthorityRequirement, AsmCacheOperationKind, AsmCatalogEntry, AsmControlRegister,
        AsmFenceKind, AsmFlagsDataFlow, AsmGlobalDescriptorTableOperation,
        AsmInstructionAvailability, AsmInstructionRefusal, AsmInstructionSerializationKind,
        AsmInstructionShape, AsmInterruptControlKind, AsmInterruptFlagEffect, AsmMemoryOrdering,
        AsmMemoryTransferKind, AsmOperandAccess, AsmSchedulingHintKind, AsmSystemRegister,
        AsmTargetApplicability, X86FlagsEnvironment, asm_catalog_entry, asm_intrinsic_mnemonic,
        scratch_register,
    };

    #[test]
    fn catalog_separates_user_and_deriver_availability() {
        let AsmCatalogEntry::Contract(hlt) = asm_catalog_entry("hlt").expect("hlt contract") else {
            panic!("hlt must be a contracted instruction");
        };
        assert_eq!(hlt.availability, AsmInstructionAvailability::UserChecked);
        assert_eq!(
            hlt.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );

        let AsmCatalogEntry::Contract(iretq) = asm_catalog_entry("iretq").expect("iretq contract")
        else {
            panic!("iretq must be a contracted instruction");
        };
        assert_eq!(iretq.availability, AsmInstructionAvailability::DeriverOnly);

        let AsmCatalogEntry::Contract(lidt) = asm_catalog_entry("lidt").expect("lidt contract")
        else {
            panic!("lidt must be a contracted instruction");
        };
        assert_eq!(lidt.availability, AsmInstructionAvailability::DeriverOnly);
        assert_eq!(lidt.shape, AsmInstructionShape::DescriptorTableLoad);
        assert_eq!(lidt.required_authority, AsmAuthorityRequirement::IdtControl);
        assert_eq!(lidt.operands.len(), 1);
        assert!(lidt.operands[0].requires_place());
        assert_eq!(lidt.operands[0].target_register, "r10");
        assert_eq!(lidt.clobbers, &["r10"]);
    }

    #[test]
    fn port_contracts_pin_operands_and_realized_clobbers() {
        let AsmCatalogEntry::Contract(out) = asm_catalog_entry("out").expect("out contract") else {
            panic!("out must be a contracted instruction");
        };
        assert_eq!(out.target, AsmTargetApplicability::X86_64);
        assert_eq!(out.required_authority, AsmAuthorityRequirement::PortIo);
        assert_eq!(
            out.operands
                .iter()
                .map(|operand| (
                    operand.role,
                    operand.target_register,
                    operand.access,
                    operand.expected_type_name,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("port", "dx", AsmOperandAccess::Read, "u16"),
                ("value", "al", AsmOperandAccess::Read, "u8"),
            ]
        );
        assert_eq!(out.clobbers, &["rax", "rdx", "r10", "r11", "r15"]);

        let AsmCatalogEntry::Contract(input) = asm_catalog_entry("in").expect("in contract") else {
            panic!("in must be a contracted instruction");
        };
        assert_eq!(
            input
                .operands
                .iter()
                .map(|operand| (
                    operand.role,
                    operand.target_register,
                    operand.access,
                    operand.expected_type_name,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("destination", "al", AsmOperandAccess::Write, "u8"),
                ("port", "dx", AsmOperandAccess::Read, "u16"),
            ]
        );
        assert_eq!(input.clobbers, &["rax", "rdx", "r10", "r15"]);
    }

    #[test]
    fn flags_contracts_are_explicit_and_stack_balanced_by_lowering() {
        let AsmCatalogEntry::Contract(snapshot) =
            asm_catalog_entry("pushfq").expect("pushfq contract")
        else {
            panic!("pushfq must be contracted");
        };
        assert_eq!(snapshot.required_authority, AsmAuthorityRequirement::None);
        assert_eq!(
            snapshot.flags_data_flow,
            AsmFlagsDataFlow::SnapshotToOperand
        );
        assert_eq!(snapshot.operands[0].access, AsmOperandAccess::Write);
        assert_eq!(snapshot.operands[0].expected_type_name, "u64");
        assert_eq!(snapshot.clobbers, &["r10", "r15"]);

        let AsmCatalogEntry::Contract(restore) =
            asm_catalog_entry("popfq").expect("popfq contract")
        else {
            panic!("popfq must be contracted");
        };
        assert_eq!(
            restore.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );
        assert_eq!(
            restore.flags_data_flow,
            AsmFlagsDataFlow::RestoreFromOperand
        );
        assert_eq!(
            restore.interrupt_flag_effect,
            AsmInterruptFlagEffect::RestoreFromOperand
        );
        assert_eq!(restore.operands[0].access, AsmOperandAccess::ReadPlace);
        assert_eq!(restore.clobbers, &["r10", "r15"]);
    }

    #[test]
    fn x86_flags_environment_validates_reserved_bits_and_restores_arithmetic_only() {
        let image: u64 = 0x246;
        let mut environment = X86FlagsEnvironment::new(image).expect("valid host image");
        assert_eq!(environment.snapshot(), image);
        assert!(X86FlagsEnvironment::new(image & !2).is_err());
        assert!(X86FlagsEnvironment::new(image | 8).is_err());
        assert!(X86FlagsEnvironment::new(image | (1 << 22)).is_err());
        environment.restore_arithmetic_bits(!0u64);
        assert_eq!(environment.snapshot(), (image & !0x8d5) | 0x8d5);
        environment.restore_arithmetic_bits(0);
        assert_eq!(environment.snapshot(), image & !0x8d5);
    }

    #[test]
    fn msr_contracts_pin_structured_value_flow_and_machine_authority() {
        let AsmCatalogEntry::Contract(read) = asm_catalog_entry("rdmsr").expect("rdmsr contract")
        else {
            panic!("rdmsr must be contracted");
        };
        assert_eq!(read.target, AsmTargetApplicability::X86_64);
        assert_eq!(
            read.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );
        assert_eq!(read.operands[0].access, AsmOperandAccess::Write);
        assert_eq!(read.operands[0].target_register, "edx:eax");
        assert_eq!(read.operands[1].expected_type_name, "u32");
        assert_eq!(read.clobbers, &["rax", "rcx", "rdx", "r10", "r11", "r15"]);

        let AsmCatalogEntry::Contract(write) = asm_catalog_entry("wrmsr").expect("wrmsr contract")
        else {
            panic!("wrmsr must be contracted");
        };
        assert_eq!(
            write.required_authority,
            AsmAuthorityRequirement::MachineOwner
        );
        assert_eq!(write.operands[0].target_register, "ecx");
        assert_eq!(write.operands[1].expected_type_name, "u64");
        assert_eq!(write.clobbers, &["rax", "rcx", "rdx", "r10", "r11", "r15"]);
    }

    #[test]
    fn control_register_contracts_pin_exact_u64_flow_and_machine_authority() {
        for register in [
            AsmControlRegister::Cr0,
            AsmControlRegister::Cr2,
            AsmControlRegister::Cr3,
            AsmControlRegister::Cr4,
        ] {
            let AsmCatalogEntry::Contract(read) = asm_catalog_entry(register.read_mnemonic())
                .expect("control-register read contract")
            else {
                panic!("control-register read must be contracted");
            };
            assert_eq!(
                read.shape,
                AsmInstructionShape::ControlRegisterRead(register)
            );
            assert_eq!(read.target, AsmTargetApplicability::X86_64);
            assert_eq!(
                read.required_authority,
                AsmAuthorityRequirement::MachineOwner
            );
            assert_eq!(read.operands[0].access, AsmOperandAccess::Write);
            assert_eq!(read.operands[0].target_register, register.name());
            assert_eq!(read.operands[0].expected_type_name, "u64");
            assert_eq!(read.clobbers, &["r10", "r15"]);
        }

        for register in [
            AsmControlRegister::Cr0,
            AsmControlRegister::Cr3,
            AsmControlRegister::Cr4,
        ] {
            let mnemonic = register
                .write_mnemonic()
                .expect("writable control register");
            let AsmCatalogEntry::Contract(write) =
                asm_catalog_entry(mnemonic).expect("control-register write contract")
            else {
                panic!("control-register write must be contracted");
            };
            assert_eq!(
                write.shape,
                AsmInstructionShape::ControlRegisterWrite(register)
            );
            assert_eq!(
                write.required_authority,
                AsmAuthorityRequirement::MachineOwner
            );
            assert_eq!(write.operands[0].access, AsmOperandAccess::Read);
            assert_eq!(write.operands[0].target_register, register.name());
            assert_eq!(write.operands[0].expected_type_name, "u64");
            assert_eq!(write.clobbers, &["rax", "r10", "r11", "r15"]);
        }

        assert_eq!(asm_catalog_entry("write_cr2"), None);
    }

    #[test]
    fn system_register_contracts_pin_exact_u64_flow_and_machine_authority() {
        for register in AsmSystemRegister::ALL
            .into_iter()
            .filter(|register| register.target_applicability() == AsmTargetApplicability::Aarch64)
        {
            let AsmCatalogEntry::Contract(read) =
                asm_catalog_entry(register.read_mnemonic()).expect("system-register read contract")
            else {
                panic!("system-register read must be contracted");
            };
            assert_eq!(
                read.shape,
                AsmInstructionShape::SystemRegisterRead(register)
            );
            assert_eq!(read.target, AsmTargetApplicability::Aarch64);
            assert_eq!(
                read.required_authority,
                AsmAuthorityRequirement::MachineOwner
            );
            assert_eq!(read.operands[0].access, AsmOperandAccess::Write);
            assert_eq!(read.operands[0].target_register, register.name());
            assert_eq!(read.operands[0].expected_type_name, "u64");
            assert_eq!(read.clobbers, &["x9", "x15"]);
        }

        for register in AsmSystemRegister::WRITABLE
            .into_iter()
            .filter(|register| register.target_applicability() == AsmTargetApplicability::Aarch64)
        {
            let mnemonic = register.write_mnemonic().expect("writable system register");
            let AsmCatalogEntry::Contract(write) =
                asm_catalog_entry(mnemonic).expect("system-register write contract")
            else {
                panic!("system-register write must be contracted");
            };
            assert_eq!(
                write.shape,
                AsmInstructionShape::SystemRegisterWrite(register)
            );
            assert_eq!(write.target, AsmTargetApplicability::Aarch64);
            assert_eq!(
                write.required_authority,
                AsmAuthorityRequirement::MachineOwner
            );
            assert_eq!(write.operands[0].access, AsmOperandAccess::Read);
            assert_eq!(write.operands[0].target_register, register.name());
            assert_eq!(write.operands[0].expected_type_name, "u64");
            assert_eq!(write.clobbers, &["x9", "x10", "x11", "x15"]);
        }

        assert_eq!(asm_catalog_entry("write_esr_el1"), None);
        assert_eq!(asm_catalog_entry("write_far_el1"), None);
    }

    #[test]
    fn register_move_contracts_delegate_operand_checking_to_the_assignment() {
        for mnemonic in ["mov", "movq"] {
            let AsmCatalogEntry::Contract(contract) =
                asm_catalog_entry(mnemonic).expect("register-move contract")
            else {
                panic!("{mnemonic} must be contracted");
            };
            assert_eq!(contract.shape, AsmInstructionShape::RegisterMove);
            assert_eq!(
                contract.availability,
                AsmInstructionAvailability::UserChecked
            );
            assert_eq!(contract.target, AsmTargetApplicability::Any);
            assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
            assert!(contract.operands.is_empty());
            assert!(contract.clobbers.is_empty());
        }
    }

    #[test]
    fn addition_establishes_the_same_arithmetic_flag_flow_as_subtraction() {
        let Some(AsmCatalogEntry::Contract(addition)) = asm_catalog_entry("add") else {
            panic!("add retains a checked arithmetic contract");
        };
        let Some(AsmCatalogEntry::Contract(subtraction)) = asm_catalog_entry("sub") else {
            panic!("sub retains a checked arithmetic contract");
        };
        assert_eq!(addition.shape, AsmInstructionShape::Add);
        assert_eq!(subtraction.shape, AsmInstructionShape::Subtract);
        assert_eq!(addition.flags_data_flow, AsmFlagsDataFlow::ArithmeticResult);
        assert_eq!(addition.flags_data_flow, subtraction.flags_data_flow);
        assert_eq!(addition.target, AsmTargetApplicability::X86_64);
        assert_eq!(addition.clobbers, &["rflags"]);
        assert_eq!(
            addition.availability,
            AsmInstructionAvailability::UserChecked
        );
        assert_eq!(addition.required_authority, AsmAuthorityRequirement::None);
    }

    #[test]
    fn catalog_names_semantic_refusal_classes() {
        let Some(AsmCatalogEntry::Contract(branch)) = asm_catalog_entry("jnz") else {
            panic!("jnz retains its checked zero-flag edge");
        };
        assert_eq!(
            branch.shape,
            AsmInstructionShape::JumpZeroFlag { when_set: false }
        );
        assert_eq!(branch.flags_data_flow, AsmFlagsDataFlow::ZeroFlagBranch);
        for mnemonic in [
            "ret", "retq", "retn", "retw", "retaa", "retab", "retf", "lret", "iret", "iretd",
            "iretw", "call", "callq", "callf", "lcall", "jmpq", "jmpf", "jmpl", "ljmp", "ljmpl",
            "br", "blr", "bl", "bx", "blx", "bc", "braa", "brab", "braaz", "brabz", "blraa",
            "blrab", "blraaz", "blrabz", "eretaa", "eretab", "drps", "cbz", "cbnz", "tbz", "tbnz",
            "loop", "loope", "loopne", "loopz", "loopnz", "jcxz", "jecxz", "jrcxz", "int", "int1",
            "int3", "into", "je", "jne", "ja", "jae", "jb", "jbe", "jna", "jnae", "jnb", "jnbe",
            "jg", "jge", "jl", "jle", "jng", "jnge", "jnl", "jnle", "jo", "jno", "js", "jns", "jp",
            "jpe", "jnp", "jpo", "jc", "jnc",
        ] {
            assert_eq!(
                asm_catalog_entry(mnemonic),
                Some(AsmCatalogEntry::Refused(
                    AsmInstructionRefusal::HiddenControlExit
                )),
                "{mnemonic} stays a hidden-exit refusal"
            );
        }
        for mnemonic in [
            "ldp",
            "stp",
            "ldpsw",
            "ldnp",
            "stnp",
            "push",
            "pop",
            "pushq",
            "popq",
            "pushw",
            "pushl",
            "pushf",
            "pushfd",
            "pusha",
            "pushal",
            "pushad",
            "popa",
            "popal",
            "popad",
            "popw",
            "popl",
            "popf",
            "popfd",
            "enter",
            "leave",
            "ldrb",
            "ldrh",
            "ldrsb",
            "ldrsh",
            "ldrsw",
            "strb",
            "strh",
            "ldur",
            "stur",
            "ldurb",
            "ldurh",
            "ldursb",
            "ldursh",
            "ldursw",
            "sturb",
            "sturh",
            "ldtr",
            "ldtrb",
            "ldtrh",
            "ldtrsb",
            "ldtrsh",
            "ldtrsw",
            "sttr",
            "sttrb",
            "sttrh",
            "ldapr",
            "ldaprb",
            "ldaprh",
            "ldaprsb",
            "ldaprsh",
            "ldaprsw",
            "ldapur",
            "ldapurb",
            "ldapurh",
            "ldapursb",
            "ldapursh",
            "ldapursw",
            "stlur",
            "stlurb",
            "stlurh",
            "ldlar",
            "ldlarb",
            "ldlarh",
            "stllr",
            "stllrb",
            "stllrh",
            "ldxr",
            "ldxrb",
            "ldxrh",
            "stxr",
            "stxrb",
            "stxrh",
            "ldaxr",
            "ldaxrb",
            "ldaxrh",
            "stlxr",
            "stlxrb",
            "stlxrh",
            "ldxp",
            "stxp",
            "ldaxp",
            "stlxp",
            "ldar",
            "ldarb",
            "ldarh",
            "stlr",
            "stlrb",
            "stlrh",
            "ld64b",
            "st64b",
            "st64bv",
            "st64bv0",
            "ld1",
            "st1",
            "ld2",
            "st2",
            "ld3",
            "st3",
            "ld4",
            "st4",
            "ld1r",
            "ld2r",
            "ld3r",
            "ld4r",
            "swp",
            "swpb",
            "swph",
            "swpa",
            "swpal",
            "swpl",
            "swpab",
            "swpah",
            "swpalb",
            "swpalh",
            "swplb",
            "swplh",
            "cas",
            "casb",
            "cash",
            "casa",
            "casal",
            "casl",
            "casab",
            "casah",
            "caslb",
            "caslh",
            "casalb",
            "casalh",
            "casp",
            "caspa",
            "caspal",
            "caspl",
            "ldadd",
            "ldaddb",
            "ldaddh",
            "ldadda",
            "ldaddab",
            "ldaddah",
            "ldaddl",
            "ldaddlb",
            "ldaddlh",
            "ldaddal",
            "ldaddalb",
            "ldaddalh",
            "ldclr",
            "ldclrb",
            "ldclrh",
            "ldclra",
            "ldclrab",
            "ldclrah",
            "ldclrl",
            "ldclrlb",
            "ldclrlh",
            "ldclral",
            "ldclralb",
            "ldclralh",
            "ldeor",
            "ldeorb",
            "ldeorh",
            "ldeora",
            "ldeorab",
            "ldeorah",
            "ldeorl",
            "ldeorlb",
            "ldeorlh",
            "ldeoral",
            "ldeoralb",
            "ldeoralh",
            "ldset",
            "ldsetb",
            "ldseth",
            "ldseta",
            "ldsetab",
            "ldsetah",
            "ldsetl",
            "ldsetlb",
            "ldsetlh",
            "ldsetal",
            "ldsetalb",
            "ldsetalh",
            "ldsmax",
            "ldsmaxb",
            "ldsmaxh",
            "ldsmaxa",
            "ldsmaxab",
            "ldsmaxah",
            "ldsmaxl",
            "ldsmaxlb",
            "ldsmaxlh",
            "ldsmaxal",
            "ldsmaxalb",
            "ldsmaxalh",
            "ldsmin",
            "ldsminb",
            "ldsminh",
            "ldsmina",
            "ldsminab",
            "ldsminah",
            "ldsminl",
            "ldsminlb",
            "ldsminlh",
            "ldsminal",
            "ldsminalb",
            "ldsminalh",
            "ldumax",
            "ldumaxb",
            "ldumaxh",
            "ldumaxa",
            "ldumaxab",
            "ldumaxah",
            "ldumaxl",
            "ldumaxlb",
            "ldumaxlh",
            "ldumaxal",
            "ldumaxalb",
            "ldumaxalh",
            "ldumin",
            "lduminb",
            "lduminh",
            "ldumina",
            "lduminab",
            "lduminah",
            "lduminl",
            "lduminlb",
            "lduminlh",
            "lduminal",
            "lduminalb",
            "lduminalh",
            "stadd",
            "staddb",
            "staddh",
            "staddl",
            "staddlb",
            "staddlh",
            "stclr",
            "stclrb",
            "stclrh",
            "stclrl",
            "stclrlb",
            "stclrlh",
            "steor",
            "steorb",
            "steorh",
            "steorl",
            "steorlb",
            "steorlh",
            "stset",
            "stsetb",
            "stseth",
            "stsetl",
            "stsetlb",
            "stsetlh",
            "stsmax",
            "stsmaxb",
            "stsmaxh",
            "stsmaxl",
            "stsmaxlb",
            "stsmaxlh",
            "stsmin",
            "stsminb",
            "stsminh",
            "stsminl",
            "stsminlb",
            "stsminlh",
            "stumax",
            "stumaxb",
            "stumaxh",
            "stumaxl",
            "stumaxlb",
            "stumaxlh",
            "stumin",
            "stuminb",
            "stuminh",
            "stuminl",
            "stuminlb",
            "stuminlh",
            "xchg",
            "xadd",
            "cmpxchg",
            "cmpxchg8b",
            "cmpxchg16b",
            "xlat",
            "xlatb",
            "lds",
            "les",
            "lss",
            "lfs",
            "lgs",
            "sidt",
            "movnti",
            "movntq",
            "movntdq",
            "movntdqa",
            "bound",
            "fxsave",
            "fxrstor",
            "xsave",
            "xsavec",
            "xsaves",
            "xsaveopt",
            "xrstor",
            "xrstors",
            "movs",
            "movsb",
            "movsw",
            "movsq",
            "lods",
            "lodsb",
            "lodsw",
            "lodsq",
            "lodsd",
            "stos",
            "stosb",
            "stosw",
            "stosq",
            "stosd",
            "scas",
            "scasb",
            "scasw",
            "scasq",
            "scasd",
            "cmps",
            "cmpsb",
            "cmpsw",
            "cmpsq",
            "ins",
            "outs",
            "insb",
            "insw",
            "insd",
            "outsb",
            "outsw",
            "outsd",
        ] {
            assert_eq!(
                asm_catalog_entry(mnemonic),
                Some(AsmCatalogEntry::Refused(
                    AsmInstructionRefusal::UnmodeledMemoryAccess
                )),
                "{mnemonic} stays an unmodeled-memory refusal"
            );
        }
        for (mnemonic, operation) in [
            ("sgdt", AsmGlobalDescriptorTableOperation::Store),
            ("lgdt", AsmGlobalDescriptorTableOperation::Load),
        ] {
            let AsmCatalogEntry::Contract(contract) =
                asm_catalog_entry(mnemonic).expect("descriptor-table contract")
            else {
                panic!("{mnemonic} must be contracted");
            };
            assert_eq!(
                contract.shape,
                AsmInstructionShape::GlobalDescriptorTable(operation),
                "{mnemonic}"
            );
            assert_eq!(
                contract.availability,
                AsmInstructionAvailability::UserChecked
            );
            assert_eq!(contract.target, AsmTargetApplicability::X86_64);
        }
        for mnemonic in ["db", "svc", "hvc", "smc", "brk", "lea", "dsb"] {
            assert_eq!(
                asm_catalog_entry(mnemonic),
                None,
                "{mnemonic} stays an unknown mnemonic"
            );
        }
    }

    #[test]
    fn memory_transfer_contracts_pin_place_operands_and_operand_order() {
        for (mnemonic, kind) in [
            ("ldr", AsmMemoryTransferKind::Load),
            ("str", AsmMemoryTransferKind::Store),
        ] {
            let AsmCatalogEntry::Contract(contract) =
                asm_catalog_entry(mnemonic).expect("memory-transfer contract")
            else {
                panic!("{mnemonic} must be contracted");
            };
            assert_eq!(contract.shape, AsmInstructionShape::MemoryTransfer(kind));
            assert_eq!(contract.target, AsmTargetApplicability::Aarch64);
            assert_eq!(
                contract.availability,
                AsmInstructionAvailability::UserChecked
            );
            assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
            assert_eq!(contract.memory_ordering, AsmMemoryOrdering::None);
            assert!(contract.operands.is_empty());
            assert!(contract.clobbers.is_empty());
        }
        for mnemonic in [
            "ldrb", "ldrsw", "strh", "ldur", "sturh", "ldxr", "stxrh", "ldar",
        ] {
            assert_eq!(
                asm_catalog_entry(mnemonic),
                Some(AsmCatalogEntry::Refused(
                    AsmInstructionRefusal::UnmodeledMemoryAccess
                )),
                "{mnemonic} stays an unmodeled-memory refusal"
            );
        }
    }

    #[test]
    fn fence_contracts_pin_ordering_without_invented_clobbers() {
        for (mnemonic, kind) in [
            ("lfence", AsmFenceKind::Load),
            ("sfence", AsmFenceKind::Store),
            ("mfence", AsmFenceKind::Full),
        ] {
            let AsmCatalogEntry::Contract(contract) =
                asm_catalog_entry(mnemonic).expect("fence contract")
            else {
                panic!("{mnemonic} must be contracted");
            };
            assert_eq!(contract.memory_ordering, AsmMemoryOrdering::Fence(kind));
            assert_eq!(contract.target, AsmTargetApplicability::X86_64);
            assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
            assert!(contract.operands.is_empty());
            assert!(contract.clobbers.is_empty());
        }
    }

    #[test]
    fn pipeline_directive_contracts_pin_no_authority_and_no_clobbers() {
        macro_rules! row {
            ($name:literal => $kind:path => $target:ident) => {
                ($name, $kind, AsmTargetApplicability::$target)
            };
        }
        for (mnemonic, kind, target) in [
            row!("serialize" => AsmInstructionSerializationKind::Serialize => X86_64),
            (
                "isb",
                AsmInstructionSerializationKind::InstructionSynchronizationBarrier,
                AsmTargetApplicability::Aarch64,
            ),
        ] {
            let AsmCatalogEntry::Contract(contract) =
                asm_catalog_entry(mnemonic).expect("serialization contract")
            else {
                panic!("{mnemonic} must be contracted");
            };
            assert_eq!(
                contract.shape,
                AsmInstructionShape::InstructionSerialization(kind)
            );
            assert_eq!(contract.target, target);
            assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
            assert_eq!(contract.memory_ordering, AsmMemoryOrdering::None);
            assert!(contract.operands.is_empty());
            assert!(contract.clobbers.is_empty());
        }

        for (mnemonic, kind, target) in [
            row!("pause" => AsmSchedulingHintKind::SpinPause => X86_64),
            row!("yield" => AsmSchedulingHintKind::Yield => Aarch64),
            row!("nop" => AsmSchedulingHintKind::Nop => Any),
            row!("wfe" => AsmSchedulingHintKind::WaitForEvent => Aarch64),
            row!("wfi" => AsmSchedulingHintKind::WaitForInterrupt => Aarch64),
            row!("sev" => AsmSchedulingHintKind::SendEvent => Aarch64),
            row!("sevl" => AsmSchedulingHintKind::SendEventLocal => Aarch64),
        ] {
            let AsmCatalogEntry::Contract(contract) =
                asm_catalog_entry(mnemonic).expect("scheduling-hint contract")
            else {
                panic!("{mnemonic} must be contracted");
            };
            assert_eq!(contract.shape, AsmInstructionShape::SchedulingHint(kind));
            assert_eq!(contract.target, target);
            assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
            assert!(contract.operands.is_empty());
            assert!(contract.clobbers.is_empty());
        }
    }

    #[test]
    fn cache_operation_contracts_pin_machine_owner_and_no_operands() {
        for (mnemonic, kind) in [
            ("wbinvd", AsmCacheOperationKind::WriteBackInvalidate),
            ("invd", AsmCacheOperationKind::Invalidate),
            ("wbnoinvd", AsmCacheOperationKind::WriteBackNoInvalidate),
        ] {
            let AsmCatalogEntry::Contract(contract) =
                asm_catalog_entry(mnemonic).expect("cache-operation contract")
            else {
                panic!("{mnemonic} must be contracted");
            };
            assert_eq!(contract.shape, AsmInstructionShape::CacheOperation(kind));
            assert_eq!(contract.target, AsmTargetApplicability::X86_64);
            assert_eq!(
                contract.required_authority,
                AsmAuthorityRequirement::MachineOwner
            );
            assert_eq!(
                contract.availability,
                AsmInstructionAvailability::UserChecked
            );
            assert!(contract.operands.is_empty());
            assert!(contract.clobbers.is_empty());
        }
    }

    #[test]
    fn interrupt_control_contracts_pin_authority_and_delayed_sti_semantics() {
        for (mnemonic, flag_effect) in [
            ("cli", AsmInterruptFlagEffect::Disable),
            ("sti", AsmInterruptFlagEffect::EnableAfterNextInstruction),
        ] {
            let AsmCatalogEntry::Contract(contract) =
                asm_catalog_entry(mnemonic).expect("interrupt-control contract")
            else {
                panic!("{mnemonic} must be contracted");
            };
            assert_eq!(contract.target, AsmTargetApplicability::X86_64);
            assert_eq!(
                contract.required_authority,
                AsmAuthorityRequirement::MachineOwner
            );
            assert_eq!(contract.interrupt_flag_effect, flag_effect);
            assert!(contract.operands.is_empty());
            assert!(contract.clobbers.is_empty());
        }
    }

    #[test]
    fn every_intrinsic_name_answers_with_its_contracted_mnemonic() {
        let mut pairs: Vec<(&str, &str)> = vec![
            ("asm#hlt", "hlt"),
            ("asm#port_out", "out"),
            ("asm#port_in", "in"),
            ("asm#pushfq", "pushfq"),
            ("asm#popfq", "popfq"),
            ("asm#rdmsr", "rdmsr"),
            ("asm#wrmsr", "wrmsr"),
        ];
        for kind in [AsmFenceKind::Load, AsmFenceKind::Store, AsmFenceKind::Full] {
            pairs.push((kind.intrinsic_name(), kind.mnemonic()));
        }
        for kind in [
            AsmInterruptControlKind::Disable,
            AsmInterruptControlKind::Enable,
        ] {
            pairs.push((kind.intrinsic_name(), kind.mnemonic()));
        }
        for kind in [
            AsmInstructionSerializationKind::Serialize,
            AsmInstructionSerializationKind::InstructionSynchronizationBarrier,
        ] {
            pairs.push((kind.intrinsic_name(), kind.mnemonic()));
        }
        for kind in [
            AsmSchedulingHintKind::SpinPause,
            AsmSchedulingHintKind::Yield,
            AsmSchedulingHintKind::Nop,
            AsmSchedulingHintKind::WaitForEvent,
            AsmSchedulingHintKind::WaitForInterrupt,
            AsmSchedulingHintKind::SendEvent,
            AsmSchedulingHintKind::SendEventLocal,
        ] {
            pairs.push((kind.intrinsic_name(), kind.mnemonic()));
        }
        for kind in [
            AsmCacheOperationKind::WriteBackInvalidate,
            AsmCacheOperationKind::Invalidate,
            AsmCacheOperationKind::WriteBackNoInvalidate,
        ] {
            pairs.push((kind.intrinsic_name(), kind.mnemonic()));
        }
        for register in [
            AsmControlRegister::Cr0,
            AsmControlRegister::Cr2,
            AsmControlRegister::Cr3,
            AsmControlRegister::Cr4,
        ] {
            pairs.push((register.read_intrinsic_name(), register.read_mnemonic()));
            if let Some(name) = register.write_intrinsic_name() {
                pairs.push((name, register.write_mnemonic().expect("writable")));
            }
        }
        for register in [
            AsmSystemRegister::SctlrEl1,
            AsmSystemRegister::TcrEl1,
            AsmSystemRegister::Ttbr0El1,
            AsmSystemRegister::Ttbr1El1,
            AsmSystemRegister::MairEl1,
            AsmSystemRegister::VbarEl1,
            AsmSystemRegister::TpidrEl1,
            AsmSystemRegister::EsrEl1,
            AsmSystemRegister::FarEl1,
        ] {
            pairs.push((register.read_intrinsic_name(), register.read_mnemonic()));
            if let Some(name) = register.write_intrinsic_name() {
                pairs.push((name, register.write_mnemonic().expect("writable")));
            }
        }
        assert_eq!(pairs.len(), 47, "one pair per asm intrinsic builtin");

        for (intrinsic_name, mnemonic) in pairs {
            assert_eq!(
                asm_intrinsic_mnemonic(intrinsic_name),
                Some(mnemonic),
                "{intrinsic_name} must answer with its source mnemonic"
            );
            let Some(AsmCatalogEntry::Contract(contract)) = asm_catalog_entry(mnemonic) else {
                panic!("{mnemonic} must be a contracted catalog entry");
            };
            assert_eq!(
                contract.availability,
                AsmInstructionAvailability::UserChecked,
                "{mnemonic} must be user-checked"
            );
        }

        for spelling in ["hlt", "in", "read_cr0", "max", "sqrt", "old", "asm#", ""] {
            assert_eq!(asm_intrinsic_mnemonic(spelling), None, "{spelling:?}");
        }
    }

    #[test]
    fn zero_flag_branches_retain_polarity_without_effects_or_authority() {
        for (mnemonic, when_set) in [("jz", true), ("jnz", false)] {
            let Some(AsmCatalogEntry::Contract(contract)) = asm_catalog_entry(mnemonic) else {
                panic!("zero-flag branch needs a catalog contract");
            };
            assert_eq!(
                contract.shape,
                AsmInstructionShape::JumpZeroFlag { when_set }
            );
            assert_eq!(contract.flags_data_flow, AsmFlagsDataFlow::ZeroFlagBranch);
            assert_eq!(
                contract.availability,
                AsmInstructionAvailability::UserChecked
            );
            assert_eq!(contract.target, AsmTargetApplicability::X86_64);
            assert_eq!(contract.required_authority, AsmAuthorityRequirement::None);
            assert!(contract.clobbers.is_empty());
        }
    }

    #[test]
    fn comparison_and_arm_local_edges_retain_catalog_target_demands() {
        let Some(AsmCatalogEntry::Contract(compare)) = asm_catalog_entry("cmp") else {
            panic!("comparison has a structured register contract");
        };
        assert_eq!(compare.shape, AsmInstructionShape::Compare);
        assert_eq!(compare.flags_data_flow, AsmFlagsDataFlow::ArithmeticResult);
        assert_eq!(compare.required_authority, AsmAuthorityRequirement::None);
        for (mnemonic, shape) in [
            ("b", AsmInstructionShape::JumpState),
            ("b.eq", AsmInstructionShape::JumpZeroFlag { when_set: true }),
            (
                "b.ne",
                AsmInstructionShape::JumpZeroFlag { when_set: false },
            ),
        ] {
            let Some(AsmCatalogEntry::Contract(branch)) = asm_catalog_entry(mnemonic) else {
                panic!("local branch retains its checked edge");
            };
            assert_eq!(branch.target, AsmTargetApplicability::Aarch64);
            assert_eq!(branch.shape, shape);
            assert!(branch.clobbers.is_empty());
        }
        assert_eq!(
            AsmTargetApplicability::X86_64.intersect(AsmTargetApplicability::Aarch64),
            None
        );
        assert_eq!(
            scratch_register("w28").map(|(_, target, clobber)| (target, clobber)),
            Some((AsmTargetApplicability::Aarch64, "x28"))
        );
        assert_eq!(scratch_register("w29"), None);
        assert_eq!(scratch_register("w30"), None);
    }

    #[test]
    fn fence_event_identity_keeps_target_mask_and_authority_separate() {
        use super::AsmRegisterOperation;
        let mut tags = std::collections::BTreeSet::new();
        for kind in AsmFenceKind::ALL {
            let operation = AsmRegisterOperation::MemoryFence(kind);
            assert!(tags.insert(operation.catalog_tag()));
            assert_eq!(
                AsmRegisterOperation::from_catalog_tag(operation.catalog_tag()),
                Some(operation)
            );
            assert_eq!(
                AsmRegisterOperation::from_intrinsic_name(kind.intrinsic_name()),
                Some(operation)
            );
            assert!(!operation.requires_service_authority());
            assert!(operation.input_carriers().is_empty() && operation.result_carrier().is_none());
            assert_eq!(operation.target_applicability(), kind.target());
            let contract = super::AsmInstructionContract::memory_fence(kind);
            assert_eq!(contract.memory_ordering, AsmMemoryOrdering::Fence(kind));
            assert!(contract.clobbers.is_empty() && contract.operands.is_empty());
            assert_eq!(contract.target, kind.target());
        }
        assert_eq!(AsmRegisterOperation::from_catalog_tag(0x506), None);
        for option in ["ish", "osh", "nsh", "0", "", "SY"] {
            assert_eq!(AsmFenceKind::from_aarch64_option(option), None);
        }
    }
}
