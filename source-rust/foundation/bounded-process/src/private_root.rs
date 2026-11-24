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

The bounded launcher installs this final exec hook after resource and descriptor
setup. Everything between fork and exec uses raw system calls on memory
prepared before the fork. A host that cannot create the namespace (no
unprivileged user namespaces, or not Linux) reports the native engine
unavailable; it never falls back to the host's filesystem.
*/

use std::{io, path::Path, process::Command};

#[cfg(not(target_os = "linux"))]
pub(crate) fn install(_: &mut Command, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "this host cannot supply the private test filesystem",
    ))
}

#[cfg(target_os = "linux")]
pub(crate) fn install(command: &mut Command, scratch: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::process::CommandExt;

    let image = std::fs::File::open(command.get_program())?;
    let mount_point = std::path::absolute(scratch)?.join("private-root");
    std::fs::create_dir(&mount_point)?;

    let c = |bytes: &[u8]| CString::new(bytes).map_err(io::Error::from);
    let arguments = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|argument| c(argument.as_bytes()))
        .collect::<io::Result<Vec<_>>>()?;
    // The test gets the environment its sponsor explicitly supplied.
    let environment = command
        .get_envs()
        .filter_map(|(key, value)| {
            value.map(|value| {
                let mut entry = key.as_bytes().to_vec();
                entry.push(b'=');
                entry.extend_from_slice(value.as_bytes());
                c(&entry)
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    // SAFETY: identity queries take no arguments and cannot fail.
    let uid_map = format!("0 {} 1", unsafe { libc::geteuid() });
    let gid_map = format!("0 {} 1", unsafe { libc::getegid() });
    let prepared = Prepared {
        image,
        mount_point: c(mount_point.as_os_str().as_bytes())?,
        uid_map,
        gid_map,
        arguments: ExecStrings::new(arguments),
        environment: ExecStrings::new(environment),
    };
    // The image starts inside this hook. Install it after the bounded child's
    // resource and descriptor setup, with all allocations complete before fork.
    unsafe { command.pre_exec(move || prepared.enter()) };
    Ok(())
}

/// A null-terminated execveat vector and the strings its pointers address.
#[cfg(target_os = "linux")]
struct ExecStrings {
    pointers: Vec<*const libc::c_char>,
    _strings: Vec<std::ffi::CString>,
}

#[cfg(target_os = "linux")]
impl ExecStrings {
    fn new(strings: Vec<std::ffi::CString>) -> Self {
        let pointers = strings
            .iter()
            .map(|string| string.as_ptr())
            .chain(std::iter::once(std::ptr::null()))
            .collect();
        Self {
            pointers,
            _strings: strings,
        }
    }
}

// SAFETY: the immutable vector points only to this object's owned CStrings,
// whose allocations stay in place when the object moves into the spawn hook.
#[cfg(target_os = "linux")]
unsafe impl Send for ExecStrings {}
#[cfg(target_os = "linux")]
unsafe impl Sync for ExecStrings {}

#[cfg(target_os = "linux")]
struct Prepared {
    image: std::fs::File,
    mount_point: std::ffi::CString,
    uid_map: String,
    gid_map: String,
    arguments: ExecStrings,
    environment: ExecStrings,
}

#[cfg(target_os = "linux")]
impl Prepared {
    // Runs in the forked child; returns only on failure.
    fn enter(&self) -> std::io::Result<()> {
        use std::os::fd::AsRawFd;
        let check = |status: libc::c_int| {
            if status < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        };
        unsafe {
            check(libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS))?;
            self.write(c"/proc/self/setgroups", b"deny")?;
            self.write(c"/proc/self/uid_map", self.uid_map.as_bytes())?;
            self.write(c"/proc/self/gid_map", self.gid_map.as_bytes())?;
            check(libc::mount(
                std::ptr::null(),
                c"/".as_ptr(),
                std::ptr::null(),
                libc::MS_REC | libc::MS_PRIVATE,
                std::ptr::null(),
            ))?;
            check(libc::mount(
                c"tmpfs".as_ptr(),
                self.mount_point.as_ptr(),
                c"tmpfs".as_ptr(),
                libc::MS_NOSUID | libc::MS_NODEV,
                c"size=64m,mode=0755".as_ptr().cast(),
            ))?;
            check(libc::chdir(self.mount_point.as_ptr()))?;
            check(libc::chroot(c".".as_ptr()))?;
            check(libc::chdir(c"/".as_ptr()))?;
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
                self.image.as_raw_fd(),
                c"".as_ptr(),
                self.arguments.pointers.as_ptr(),
                self.environment.pointers.as_ptr(),
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
