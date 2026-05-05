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
