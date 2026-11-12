/*
Run a native test executable over a private filesystem the runner creates empty
(wiki/spec/build/testing.md#group-configuration-and-authority).

On Linux the kernel supplies it: the child enters a new user and mount
namespace, mounts an empty tmpfs (`size=64m`, root `0755`) and makes it its
root, then executes the test from a descriptor opened beforehand. The test's
ordinary filesystem calls therefore reach only that tmpfs, which disappears
with the namespace, and no host path, absolute or relative, is reachable.

The child is root only inside its namespace, and gives up every capability
before it executes the test, so owner permission bits bind it as they bind an
ordinary user; umask is 022. The interpreter's private namespace
(`native_provider_runtime::PrivateLinuxFilesystem`) follows the same rules, so
both engines observe one filesystem behavior.

Everything the child does between fork and exec is a raw system call on memory
prepared before the fork. A host that cannot create the namespace (no
unprivileged user namespaces, or not Linux) reports the native engine
unavailable; it never falls back to the host's filesystem.
*/

/// Why this host cannot run an executable over a private root.
#[derive(Debug)]
pub struct PrivateRootUnavailable(pub String);

#[cfg(not(target_os = "linux"))]
pub fn run_in_private_root(
    _: &std::path::Path,
    _: &std::path::Path,
) -> Result<std::io::Result<std::process::Output>, PrivateRootUnavailable> {
    Err(PrivateRootUnavailable(
        "this host cannot supply the private test filesystem to a native executable".to_owned(),
    ))
}

#[cfg(target_os = "linux")]
pub fn run_in_private_root(
    executable: &std::path::Path,
    scratch: &std::path::Path,
) -> Result<std::io::Result<std::process::Output>, PrivateRootUnavailable> {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::process::CommandExt;

    let unavailable = PrivateRootUnavailable;
    let image = std::fs::File::open(executable)
        .map_err(|error| unavailable(format!("cannot open the test executable: {error}")))?;
    // An empty mount point inside the runner's own scratch directory; the
    // tmpfs hides it from the child, which sees only the tmpfs.
    let mount_point = scratch.join("private-root");
    std::fs::create_dir(&mount_point)
        .map_err(|error| unavailable(format!("cannot create the private root: {error}")))?;
    let c = |text: &[u8]| CString::new(text).map_err(|_| unavailable("path holds NUL".to_owned()));
    let mount_point = c(mount_point.as_os_str().as_bytes())?;
    // SAFETY: identity queries take no arguments and cannot fail.
    let uid_map = format!("0 {} 1", unsafe { libc::geteuid() });
    let gid_map = format!("0 {} 1", unsafe { libc::getegid() });
    let program = c(executable.as_os_str().as_bytes())?;
    let prepared = Prepared {
        image: image.as_raw_fd(),
        mount_point,
        setgroups: c(b"/proc/self/setgroups")?,
        uid_map_path: c(b"/proc/self/uid_map")?,
        gid_map_path: c(b"/proc/self/gid_map")?,
        uid_map,
        gid_map,
        root: c(b"/")?,
        tmpfs: c(b"tmpfs")?,
        options: c(b"size=64m,mode=0755")?,
        empty: c(b"")?,
        argv: [program.as_ptr(), std::ptr::null()],
        envp: [std::ptr::null()],
        program,
    };
    let mut command = std::process::Command::new(executable);
    command.stdin(std::process::Stdio::null());
    // SAFETY: the hook makes only async-signal-safe system calls on memory
    // `prepared` owns, which outlives the spawn below.
    unsafe {
        command.pre_exec(move || prepared.enter());
    }
    let output = command.output();
    drop(image);
    Ok(output)
}

#[cfg(target_os = "linux")]
struct Prepared {
    image: i32,
    mount_point: std::ffi::CString,
    setgroups: std::ffi::CString,
    uid_map_path: std::ffi::CString,
    gid_map_path: std::ffi::CString,
    uid_map: String,
    gid_map: String,
    root: std::ffi::CString,
    tmpfs: std::ffi::CString,
    options: std::ffi::CString,
    empty: std::ffi::CString,
    argv: [*const libc::c_char; 2],
    envp: [*const libc::c_char; 1],
    // Owns the bytes `argv[0]` points at.
    #[allow(dead_code)]
    program: std::ffi::CString,
}

// SAFETY: the raw pointers name bytes this value owns; the value moves into
// the spawn hook whole and is used only by the forked child.
#[cfg(target_os = "linux")]
unsafe impl Send for Prepared {}
#[cfg(target_os = "linux")]
unsafe impl Sync for Prepared {}

#[cfg(target_os = "linux")]
impl Prepared {
    // Runs in the forked child; returns only on failure.
    fn enter(&self) -> std::io::Result<()> {
        let check = |status: libc::c_int| {
            if status < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        };
        unsafe {
            check(libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS))?;
            self.write(&self.setgroups, b"deny")?;
            self.write(&self.uid_map_path, self.uid_map.as_bytes())?;
            self.write(&self.gid_map_path, self.gid_map.as_bytes())?;
            check(libc::mount(
                std::ptr::null(),
                self.root.as_ptr(),
                std::ptr::null(),
                libc::MS_REC | libc::MS_PRIVATE,
                std::ptr::null(),
            ))?;
            check(libc::mount(
                self.tmpfs.as_ptr(),
                self.mount_point.as_ptr(),
                self.tmpfs.as_ptr(),
                libc::MS_NOSUID | libc::MS_NODEV,
                self.options.as_ptr().cast(),
            ))?;
            check(libc::chdir(self.mount_point.as_ptr()))?;
            check(libc::chroot(c".".as_ptr()))?;
            check(libc::chdir(self.root.as_ptr()))?;
            libc::umask(0o022);
            // Root of the namespace only: executing as uid 0 must not regain
            // capabilities, and every capability is dropped before it does.
            let securebits = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 5);
            check(libc::prctl(libc::PR_SET_SECUREBITS, securebits, 0, 0, 0))?;
            check(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0))?;
            for capability in 0..64 {
                libc::prctl(libc::PR_CAPBSET_DROP, capability, 0, 0, 0);
            }
            #[repr(C)]
            struct Header {
                version: u32,
                pid: i32,
            }
            #[repr(C)]
            struct Data {
                effective: u32,
                permitted: u32,
                inheritable: u32,
            }
            let header = Header {
                version: 0x2008_0522, // _LINUX_CAPABILITY_VERSION_3
                pid: 0,
            };
            let data = [
                Data {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
                Data {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
            ];
            check(libc::syscall(libc::SYS_capset, &header, data.as_ptr()) as libc::c_int)?;
            libc::syscall(
                libc::SYS_execveat,
                self.image,
                self.empty.as_ptr(),
                self.argv.as_ptr(),
                self.envp.as_ptr(),
                libc::AT_EMPTY_PATH,
            );
        }
        Err(std::io::Error::last_os_error())
    }

    unsafe fn write(&self, path: &std::ffi::CStr, bytes: &[u8]) -> std::io::Result<()> {
        unsafe {
            let descriptor = libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
            if descriptor < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let written = libc::write(descriptor, bytes.as_ptr().cast(), bytes.len());
            libc::close(descriptor);
            if written != bytes.len() as isize {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }
}
