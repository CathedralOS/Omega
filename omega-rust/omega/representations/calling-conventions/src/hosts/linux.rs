use target::Architecture;

pub fn linux_clock_gettime_syscall_number(architecture: Architecture) -> u32 {
    match architecture {
        Architecture::Aarch64 => 113,
        Architecture::X86_64 => 228,
    }
}

pub fn linux_nanosleep_syscall_number(architecture: Architecture) -> u32 {
    match architecture {
        Architecture::Aarch64 => 101,
        Architecture::X86_64 => 35,
    }
}
