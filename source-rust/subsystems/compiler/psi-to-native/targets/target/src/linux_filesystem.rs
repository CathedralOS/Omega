//! Linux assigns filesystem system calls different numbers on x86-64 and arm64.
//! Native review and the Psi host resolve them against the same target table.

use crate::TargetProfile;

/// One Linux filesystem system call, independent of its per-target number.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinuxFilesystemOperation {
    Openat,
    Close,
    Dup,
    Lseek,
    Mkdirat,
    Unlinkat,
    Newfstatat,
    Fstat,
    Getdents64,
    Pread64,
    Pwrite64,
    Readlinkat,
    Fchmodat,
    Fchmod,
    Renameat,
    Linkat,
    Symlinkat,
    Ftruncate,
    Utimensat,
    Fsync,
    Fdatasync,
    Flock,
    Fchownat,
    Fchown,
}

impl LinuxFilesystemOperation {
    /// Resolve the kernel operation using the target of the selected binding.
    pub fn from_syscall(target: TargetProfile, number: i64) -> Option<Self> {
        use TargetProfile::{LinuxArm64, LinuxX64};
        Some(match (target, number) {
            (LinuxX64, 257) | (LinuxArm64, 56) => Self::Openat,
            (LinuxX64, 3) | (LinuxArm64, 57) => Self::Close,
            (LinuxX64, 32) | (LinuxArm64, 23) => Self::Dup,
            (LinuxX64, 8) | (LinuxArm64, 62) => Self::Lseek,
            (LinuxX64, 258) | (LinuxArm64, 34) => Self::Mkdirat,
            (LinuxX64, 263) | (LinuxArm64, 35) => Self::Unlinkat,
            (LinuxX64, 262) | (LinuxArm64, 79) => Self::Newfstatat,
            (LinuxX64, 5) | (LinuxArm64, 80) => Self::Fstat,
            (LinuxX64, 217) | (LinuxArm64, 61) => Self::Getdents64,
            (LinuxX64, 17) | (LinuxArm64, 67) => Self::Pread64,
            (LinuxX64, 18) | (LinuxArm64, 68) => Self::Pwrite64,
            (LinuxX64, 267) | (LinuxArm64, 78) => Self::Readlinkat,
            (LinuxX64, 268) | (LinuxArm64, 53) => Self::Fchmodat,
            (LinuxX64, 91) | (LinuxArm64, 52) => Self::Fchmod,
            (LinuxX64, 264) | (LinuxArm64, 38) => Self::Renameat,
            (LinuxX64, 265) | (LinuxArm64, 37) => Self::Linkat,
            (LinuxX64, 266) | (LinuxArm64, 36) => Self::Symlinkat,
            (LinuxX64, 77) | (LinuxArm64, 46) => Self::Ftruncate,
            (LinuxX64, 280) | (LinuxArm64, 88) => Self::Utimensat,
            (LinuxX64, 74) | (LinuxArm64, 82) => Self::Fsync,
            (LinuxX64, 75) | (LinuxArm64, 83) => Self::Fdatasync,
            (LinuxX64, 73) | (LinuxArm64, 32) => Self::Flock,
            (LinuxX64, 260) | (LinuxArm64, 54) => Self::Fchownat,
            (LinuxX64, 93) | (LinuxArm64, 55) => Self::Fchown,
            _ => return None,
        })
    }
}
