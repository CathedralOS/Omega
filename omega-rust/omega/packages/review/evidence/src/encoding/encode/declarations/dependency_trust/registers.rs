use crate::encoding::encode::encoder::Encoder;
use crate::record::{PackageReviewBoundaryCallingPolicy, PackageReviewMachineRegister};

pub(crate) fn encode_machine_register(
    encoder: &mut Encoder,
    register: PackageReviewMachineRegister,
) {
    match register {
        PackageReviewMachineRegister::X86Rax => encoder.tag("x86_rax", 0),
        PackageReviewMachineRegister::X86Rcx => encoder.tag("x86_rcx", 1),
        PackageReviewMachineRegister::X86Rdx => encoder.tag("x86_rdx", 2),
        PackageReviewMachineRegister::X86Rbx => encoder.tag("x86_rbx", 3),
        PackageReviewMachineRegister::X86Rsp => encoder.tag("x86_rsp", 4),
        PackageReviewMachineRegister::X86Rbp => encoder.tag("x86_rbp", 5),
        PackageReviewMachineRegister::X86Rsi => encoder.tag("x86_rsi", 6),
        PackageReviewMachineRegister::X86Rdi => encoder.tag("x86_rdi", 7),
        PackageReviewMachineRegister::X86R8 => encoder.tag("x86_r8", 8),
        PackageReviewMachineRegister::X86R9 => encoder.tag("x86_r9", 9),
        PackageReviewMachineRegister::X86R10 => encoder.tag("x86_r10", 10),
        PackageReviewMachineRegister::X86R11 => encoder.tag("x86_r11", 11),
        PackageReviewMachineRegister::X86R12 => encoder.tag("x86_r12", 12),
        PackageReviewMachineRegister::X86R13 => encoder.tag("x86_r13", 13),
        PackageReviewMachineRegister::X86R14 => encoder.tag("x86_r14", 14),
        PackageReviewMachineRegister::X86R15 => encoder.tag("x86_r15", 15),
        PackageReviewMachineRegister::X86Xmm(index) => {
            encoder.tag("x86_xmm", 16);
            let _ = encoder.field("index", |encoder| {
                encoder.byte(index);
                Ok(())
            });
        }
        PackageReviewMachineRegister::Aarch64X(index) => {
            encoder.tag("aarch64_x", 17);
            let _ = encoder.field("index", |encoder| {
                encoder.byte(index);
                Ok(())
            });
        }
        PackageReviewMachineRegister::Aarch64V(index) => {
            encoder.tag("aarch64_v", 18);
            let _ = encoder.field("index", |encoder| {
                encoder.byte(index);
                Ok(())
            });
        }
    }
}

pub(crate) const fn calling_policy_tag(policy: PackageReviewBoundaryCallingPolicy) -> u8 {
    match policy {
        PackageReviewBoundaryCallingPolicy::MicrosoftX64 => 0,
        PackageReviewBoundaryCallingPolicy::SystemVAMD64 => 1,
        PackageReviewBoundaryCallingPolicy::Aapcs64 => 2,
        PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64 => 3,
        PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64 => 4,
    }
}
