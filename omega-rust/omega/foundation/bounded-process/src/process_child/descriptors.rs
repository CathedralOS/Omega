// Ambient Unix descriptor closure (`mark_ambient_close_on_exec`) and explicit
// descriptor retention (`retain_descriptors`) for bounded children.
//
// On Linux we mark descriptors `3..=u32::MAX` `CLOEXEC` in one
// `SYS_close_range` syscall inside `pre_exec`. On macOS/BSD we enumerate
// `/dev/fd` in the parent before fork and set `FD_CLOEXEC` on each descriptor
// `>= 3` inside `pre_exec`. `retain_descriptors` is registered after
// `mark_ambient_close_on_exec` so its `pre_exec` closure runs second and clears
// `FD_CLOEXEC` on the explicitly retained descriptor numbers without
// renumbering them.

use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

pub(crate) fn mark_ambient_close_on_exec(command: &mut Command) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    // SAFETY: `SYS_close_range` with `CLOSE_RANGE_CLOEXEC` is async-signal-safe
    // and only mutates descriptor flags >= 3 in the forked child before exec.
    unsafe {
        command.pre_exec(|| {
            let result = libc::syscall(
                libc::SYS_close_range,
                3_u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            );
            if result == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
        let ambient_descriptors = ambient_descriptors()?;
        // SAFETY: the pre_exec closure performs only async-signal-safe `fcntl`
        // calls over the pre-allocated `ambient_descriptors` slice.
        unsafe {
            command.pre_exec(move || {
                for descriptor in &ambient_descriptors {
                    let flags = libc::fcntl(*descriptor, libc::F_GETFD);
                    if flags == -1 {
                        let error = io::Error::last_os_error();
                        if error.raw_os_error() == Some(libc::EBADF) {
                            continue;
                        }
                        return Err(error);
                    }
                    if libc::fcntl(*descriptor, libc::F_SETFD, flags | libc::FD_CLOEXEC) == -1 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
    }
    Ok(())
}

pub(crate) fn retain_descriptors(
    command: &mut Command,
    retained: Vec<std::os::fd::RawFd>,
) -> io::Result<()> {
    if retained.is_empty() {
        return Ok(());
    }
    // SAFETY: the pre_exec closure performs only async-signal-safe `fcntl`
    // calls over the pre-allocated `retained` slice.
    unsafe {
        command.pre_exec(move || {
            for descriptor in &retained {
                if libc::fcntl(*descriptor, libc::F_SETFD, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn ambient_descriptors() -> io::Result<Vec<i32>> {
    let mut descriptors = Vec::new();
    for entry in std::fs::read_dir("/dev/fd")? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(descriptor) = name.parse::<i32>() else {
            continue;
        };
        if descriptor >= 3 {
            descriptors.push(descriptor);
        }
    }
    descriptors.sort_unstable();
    descriptors.dedup();
    Ok(descriptors)
}
