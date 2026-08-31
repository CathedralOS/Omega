use std::io;
use std::process::Command;

#[cfg(unix)]
const CHILD_CPU_SECONDS: u64 = 120;
#[cfg(any(target_os = "linux", target_os = "android"))]
const CHILD_ADDRESS_SPACE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
#[cfg(unix)]
const CHILD_FILE_SIZE_BYTES: u64 = 1024 * 1024 * 1024;
#[cfg(unix)]
pub(crate) const CHILD_OPEN_FILE_LIMIT: u64 = 256;
#[cfg(windows)]
pub(crate) const CHILD_PROCESS_LIMIT: u64 = 16;
#[cfg(windows)]
pub(crate) const CHILD_PROCESS_MEMORY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
#[cfg(windows)]
pub(crate) const CHILD_AGGREGATE_MEMORY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
#[cfg(windows)]
pub(crate) const CHILD_AGGREGATE_CPU_SECONDS: u64 = 120;

#[cfg(unix)]
pub(crate) fn configure_child_resource_limits(command: &mut Command) -> io::Result<()> {
    use std::os::unix::process::CommandExt;

    // SAFETY: the pre-exec closure performs only fixed `setrlimit` syscalls and
    // captures no references. The limits are inherited by the complete helper
    // process tree after exec.
    unsafe {
        command.pre_exec(|| {
            set_limit(rustix::process::Resource::Core, 0)?;
            set_limit(rustix::process::Resource::Cpu, CHILD_CPU_SECONDS)?;
            #[cfg(any(target_os = "linux", target_os = "android"))]
            set_limit(rustix::process::Resource::As, CHILD_ADDRESS_SPACE_BYTES)?;
            set_limit(rustix::process::Resource::Fsize, CHILD_FILE_SIZE_BYTES)?;
            set_limit(rustix::process::Resource::Nofile, CHILD_OPEN_FILE_LIMIT)
        });
    }
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn configure_child_resource_limits(_command: &mut Command) -> io::Result<()> {
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
