//! Physical register identities and the integer result banks of ordinary native calling conventions.
//! Both the native plan evaluator and embedded target admission use the same bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    Aarch64,
    X86_64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    Elf,
    MachO,
    Coff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MachineRegister {
    X86Rax,
    X86Rcx,
    X86Rdx,
    X86Rbx,
    X86Rsp,
    X86Rbp,
    X86Rsi,
    X86Rdi,
    X86R8,
    X86R9,
    X86R10,
    X86R11,
    X86R12,
    X86R13,
    X86R14,
    X86R15,
    X86Xmm(u8),
    Aarch64X(u8),
    Aarch64V(u8),
}

impl MachineRegister {
    pub const fn codec_index(self) -> u8 {
        const BANK_BASE: [u8; 5] = [0, 0, 32, 64, 128];
        const BANK_LANES: [u8; 5] = [1, 16, 16, 32, 32];
        let (class, index) = self.wire_class_index();
        let bank = class as usize;
        BANK_BASE[bank] + index % BANK_LANES[bank]
    }

    pub const fn wire_class_index(self) -> (u8, u8) {
        match self {
            Self::X86Rax => (1, 0),
            Self::X86Rcx => (1, 1),
            Self::X86Rdx => (1, 2),
            Self::X86Rbx => (1, 3),
            Self::X86Rsp => (1, 4),
            Self::X86Rbp => (1, 5),
            Self::X86Rsi => (1, 6),
            Self::X86Rdi => (1, 7),
            Self::X86R8 => (1, 8),
            Self::X86R9 => (1, 9),
            Self::X86R10 => (1, 10),
            Self::X86R11 => (1, 11),
            Self::X86R12 => (1, 12),
            Self::X86R13 => (1, 13),
            Self::X86R14 => (1, 14),
            Self::X86R15 => (1, 15),
            Self::X86Xmm(index) => (2, index),
            Self::Aarch64X(index) => (3, index),
            Self::Aarch64V(index) => (4, index),
        }
    }

    pub fn architecture(self) -> Architecture {
        match self {
            Self::X86Rax
            | Self::X86Rcx
            | Self::X86Rdx
            | Self::X86Rbx
            | Self::X86Rsp
            | Self::X86Rbp
            | Self::X86Rsi
            | Self::X86Rdi
            | Self::X86R8
            | Self::X86R9
            | Self::X86R10
            | Self::X86R11
            | Self::X86R12
            | Self::X86R13
            | Self::X86R14
            | Self::X86R15
            | Self::X86Xmm(_) => Architecture::X86_64,
            Self::Aarch64X(_) | Self::Aarch64V(_) => Architecture::Aarch64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCallingConvention {
    MicrosoftX64,
    SystemVAMD64,
    Aapcs64,
    DarwinAapcs64,
}

impl NativeCallingConvention {
    pub const fn for_target(architecture: Architecture, format: ObjectFormat) -> Self {
        match (architecture, format) {
            (Architecture::X86_64, ObjectFormat::Coff) => Self::MicrosoftX64,
            (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::MachO) => Self::SystemVAMD64,
            (Architecture::Aarch64, ObjectFormat::Elf | ObjectFormat::Coff) => Self::Aapcs64,
            (Architecture::Aarch64, ObjectFormat::MachO) => Self::DarwinAapcs64,
        }
    }

    /// Integer components in return order for a result transported directly in registers.
    pub const fn integer_result_registers(self) -> &'static [MachineRegister] {
        use MachineRegister::*;
        match self {
            Self::MicrosoftX64 => &[X86Rax],
            Self::SystemVAMD64 => &[X86Rax, X86Rdx],
            Self::Aapcs64 | Self::DarwinAapcs64 => &[Aarch64X(0), Aarch64X(1)],
        }
    }
}

/// Architecture and object format of a concrete target accepted by the checked instruction catalog.
pub fn target_platform(name: &str) -> Option<(Architecture, ObjectFormat)> {
    use Architecture::*;
    use ObjectFormat::*;
    Some(match name {
        "linux_x86_64" => (X86_64, Elf),
        "linux_arm64" => (Aarch64, Elf),
        "macos_x86_64" => (X86_64, MachO),
        "macos_arm64" => (Aarch64, MachO),
        "windows_x86_64" | "uefi_x86_64" => (X86_64, Coff),
        "windows_arm64" => (Aarch64, Coff),
        _ => return None,
    })
}
