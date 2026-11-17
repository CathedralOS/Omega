use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostedKernel {
    pub write: u32,
    pub exit: u32,
    pub carry_reports_failure: bool,
}

pub const DARWIN_UNIX_CLASS: u32 = 0x0200_0000;

const LINUX: HostedKernel = HostedKernel {
    write: 1,
    exit: 231,
    carry_reports_failure: false,
};

const DARWIN: HostedKernel = HostedKernel {
    write: DARWIN_UNIX_CLASS | 4,
    exit: DARWIN_UNIX_CLASS | 1,
    carry_reports_failure: true,
};

pub const DARWIN_READ: u32 = DARWIN_UNIX_CLASS | 3;

pub fn hosted_kernel(target: NativeTarget) -> Option<HostedKernel> {
    if target == NativeTarget::linux_x64() {
        Some(LINUX)
    } else if target == NativeTarget::macos_x64() {
        Some(DARWIN)
    } else {
        None
    }
}

pub fn hosted_syscall_target(target: NativeTarget) -> bool {
    hosted_kernel(target).is_some()
}

pub fn hosted_exit_syscall_number(target: NativeTarget) -> Option<u32> {
    hosted_kernel(target).map(|kernel| kernel.exit)
}

