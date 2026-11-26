//! Kernel entry points available to a reviewed, self-contained static image.
//! The filesystem namespace mediates path operations. The PID namespace
//! bounds process and signal targets. Devices, socket creation, keyrings,
//! namespace management, privileged kernel interfaces and async I/O have no
//! grant. Unknown ABIs and operations receive no access.
use std::io;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub(super) fn install() -> io::Result<()> {
    const LOAD_WORD: u16 = 0x20;
    const EQUAL: u16 = 0x15;
    const RETURN: u16 = 0x06;
    #[cfg(target_arch = "x86_64")]
    const AUDIT_ARCH: u32 = 0xc000_003e;
    #[cfg(target_arch = "aarch64")]
    const AUDIT_ARCH: u32 = 0xc000_00b7;

    const fn op(code: u16, jt: u8, jf: u8, k: u32) -> libc::sock_filter {
        libc::sock_filter { code, jt, jf, k }
    }
    macro_rules! profile {
        ($($number:ident),+ $(,)?) => {
            [
                op(LOAD_WORD, 0, 0, 4), // seccomp_data.arch
                op(EQUAL, 1, 0, AUDIT_ARCH),
                op(RETURN, 0, 0, libc::SECCOMP_RET_KILL_PROCESS),
                op(LOAD_WORD, 0, 0, 0), // seccomp_data.nr
                $(
                    op(EQUAL, 0, 1, libc::$number as u32),
                    op(RETURN, 0, 0, libc::SECCOMP_RET_ALLOW),
                )+
                op(RETURN, 0, 0, libc::SECCOMP_RET_ERRNO | libc::ENOSYS as u32),
            ]
        };
    }
    // Both native ABIs supply these calls. Classic, pre-*at file operations
    // additionally used by x86-64 libc appear in the target-specific tail.
    macro_rules! common {
        ($($platform:ident),* $(,)?) => {
            profile!(
                SYS_read, SYS_write, SYS_readv, SYS_writev, SYS_pread64, SYS_pwrite64,
                SYS_openat, SYS_close, SYS_newfstatat, SYS_fstat, SYS_statx, SYS_lseek,
                SYS_ftruncate, SYS_fsync, SYS_fdatasync, SYS_fchmod, SYS_fchmodat,
                SYS_getdents64, SYS_unlinkat, SYS_renameat, SYS_renameat2, SYS_mkdirat,
                SYS_readlinkat, SYS_linkat, SYS_symlinkat, SYS_faccessat, SYS_utimensat,
                SYS_getcwd, SYS_chdir, SYS_fchdir, SYS_umask, SYS_fcntl, SYS_dup,
                SYS_dup3, SYS_pipe2, SYS_ppoll, SYS_pselect6, SYS_sendfile,
                SYS_brk, SYS_mmap, SYS_munmap, SYS_mremap, SYS_mprotect, SYS_madvise,
                SYS_clock_gettime, SYS_clock_getres, SYS_clock_nanosleep, SYS_nanosleep,
                SYS_gettimeofday, SYS_getrandom, SYS_sched_yield, SYS_sched_getaffinity,
                SYS_futex, SYS_set_robust_list, SYS_rseq, SYS_set_tid_address,
                SYS_getpid, SYS_getppid, SYS_gettid, SYS_getuid, SYS_geteuid, SYS_getgid,
                SYS_getegid, SYS_getgroups, SYS_uname, SYS_prlimit64, SYS_getrusage,
                SYS_rt_sigprocmask, SYS_rt_sigaction, SYS_rt_sigreturn, SYS_sigaltstack,
                SYS_tgkill, SYS_kill, SYS_clone, SYS_wait4, SYS_waitid, SYS_setpgid,
                SYS_getpgid, SYS_setsid, SYS_getsid, SYS_execveat, SYS_execve,
                SYS_exit, SYS_exit_group, $($platform),*
            )
        }
    }
    #[cfg(target_arch = "x86_64")]
    let filter = common!(
        SYS_arch_prctl,
        SYS_access,
        SYS_open,
        SYS_stat,
        SYS_lstat,
        SYS_chmod,
        SYS_unlink,
        SYS_rename,
        SYS_mkdir,
        SYS_readlink,
        SYS_rmdir,
        SYS_pipe,
        SYS_dup2,
        SYS_poll,
        SYS_fork,
        SYS_vfork,
        SYS_time,
        SYS_getrlimit
    );
    #[cfg(target_arch = "aarch64")]
    let filter = common!();
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_ptr().cast_mut(),
    };
    // SAFETY: the kernel copies this bounded classic BPF program synchronously.
    // The private-root setup already installed no_new_privs.
    if unsafe { libc::prctl(libc::PR_SET_SECCOMP, libc::SECCOMP_MODE_FILTER, &program) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub(super) fn install() -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
