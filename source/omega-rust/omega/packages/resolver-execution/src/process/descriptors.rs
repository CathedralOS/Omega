//! Ambient Unix descriptor closure for resolver children.

use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

pub(crate) fn mark_ambient_close_on_exec(command: &mut Command) -> io::Result<()> {
    #[cfg(target_os = "linux")]
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
