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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeTarget {
    pub architecture: Architecture,
    pub object_format: ObjectFormat,
    pub pointer_size: usize,
    pub pointer_alignment: usize,
}

impl NativeTarget {
    pub fn host() -> Self {
        Self {
            architecture: host_architecture(),
            object_format: host_object_format(),
            pointer_size: std::mem::size_of::<usize>(),
            pointer_alignment: std::mem::align_of::<usize>(),
        }
    }

    pub fn from_omega_target_name(target_name: Option<&str>) -> Self {
        match target_name {
            Some("linux_x64") => Self::linux_x64(),
            Some("macos_arm64") => Self::macos_arm64(),
            Some("windows_x64") => Self::windows_x64(),
            Some("cross_platform_cli") | Some("local_unchecked") | None => Self::host(),
            Some(_) => Self::host(),
        }
    }

    pub fn linux_x64() -> Self {
        Self {
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Elf,
            pointer_size: 8,
            pointer_alignment: 8,
        }
    }

    pub fn macos_arm64() -> Self {
        Self {
            architecture: Architecture::Aarch64,
            object_format: ObjectFormat::MachO,
            pointer_size: 8,
            pointer_alignment: 8,
        }
    }

    pub fn windows_x64() -> Self {
        Self {
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Coff,
            pointer_size: 8,
            pointer_alignment: 8,
        }
    }
}

fn host_architecture() -> Architecture {
    if cfg!(target_arch = "aarch64") {
        Architecture::Aarch64
    } else if cfg!(target_arch = "x86_64") {
        Architecture::X86_64
    } else {
        panic!("unsupported host architecture for Omega native planning")
    }
}

fn host_object_format() -> ObjectFormat {
    if cfg!(target_os = "macos") {
        ObjectFormat::MachO
    } else if cfg!(target_os = "windows") {
        ObjectFormat::Coff
    } else {
        ObjectFormat::Elf
    }
}
