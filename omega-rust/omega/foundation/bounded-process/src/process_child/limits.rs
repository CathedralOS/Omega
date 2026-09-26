use std::io;
use std::process::Command;

use crate::BoundedProcessLimits;

#[cfg(unix)]
pub(crate) fn configure_child_resource_limits(
    command: &mut Command,
    limits: BoundedProcessLimits,
) -> io::Result<()> {
    use std::os::unix::process::CommandExt;

    // SAFETY: the pre-exec closure performs only fixed `setrlimit` syscalls and
    // captures no references. The limits are inherited by the complete helper
    // process tree after exec.
    unsafe {
        command.pre_exec(move || {
            set_limit(rustix::process::Resource::Core, 0)?;
            set_limit(rustix::process::Resource::Cpu, limits.cpu_seconds)?;
            #[cfg(any(target_os = "linux", target_os = "android"))]
            set_limit(rustix::process::Resource::As, limits.address_space_bytes)?;
            set_limit(rustix::process::Resource::Fsize, limits.file_size_bytes)?;
            set_limit(rustix::process::Resource::Nofile, limits.open_files)
        });
    }
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn configure_child_resource_limits(
    _command: &mut Command,
    _limits: BoundedProcessLimits,
) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_limit(resource: rustix::process::Resource, value: u64) -> io::Result<()> {
    let limit = intersect_limit(rustix::process::getrlimit(resource), value);
    rustix::process::setrlimit(resource, limit).map_err(io::Error::from)
}

#[cfg(unix)]
pub(crate) fn intersect_limit(
    inherited: rustix::process::Rlimit,
    ceiling: u64,
) -> rustix::process::Rlimit {
    let maximum = inherited
        .maximum
        .map_or(ceiling, |limit| limit.min(ceiling));
    let current = inherited
        .current
        .map_or(maximum, |limit| limit.min(maximum));
    rustix::process::Rlimit {
        current: Some(current),
        maximum: Some(maximum),
    }
}
