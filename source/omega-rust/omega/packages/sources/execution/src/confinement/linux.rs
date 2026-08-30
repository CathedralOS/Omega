//! Linux Landlock confinement for one resolver child tree.
//!
//! Landlock ABI v5 can close the handled filesystem mutation and execution
//! rights used here. It cannot confine every metadata read or bind network
//! connections to an exact destination address, so those guarantees remain
//! unavailable.

use crate::{ResolverExecutionBackendIdentity, ResolverExecutionPolicyObservation};
use command_group::{CommandGroup, GroupChild};
use landlock::{
    ABI, Access, AccessFs, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset, RulesetAttr,
    RulesetCreatedAttr, RulesetStatus, make_bitflags,
};
use std::io;
use std::os::fd::{AsFd, AsRawFd};
use std::path::Path;
use std::process::Command;

const LINUX_LANDLOCK_ABI: ABI = ABI::V5;

/// Return true only when the host can create a ruleset handling every ABI-v5
/// filesystem right. Final enforcement is checked again inside the dedicated
/// launch thread.
pub(crate) fn backend_available() -> bool {
    Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(LINUX_LANDLOCK_ABI))
        .and_then(|ruleset| ruleset.create())
        .is_ok()
}

/// Apply Landlock to a dedicated thread, then spawn the resolver child from
/// that thread so the child inherits confinement without restricting Omega's
/// other threads.
pub(crate) fn spawn(
    mut command: Command,
    policy: &ResolverExecutionPolicyObservation,
) -> io::Result<GroupChild> {
    match policy.backend() {
        ResolverExecutionBackendIdentity::UnixResourceLimits => return command.group_spawn(),
        ResolverExecutionBackendIdentity::LinuxLandlockV5 => {}
        _ => {
            return Err(io::Error::other(
                "Linux resolver received a non-Linux execution backend",
            ));
        }
    }

    std::thread::scope(|scope| {
        scope
            .spawn(move || {
                enforce(policy)?;
                command.group_spawn()
            })
            .join()
            .map_err(|_| io::Error::other("Linux resolver confinement thread panicked"))?
    })
}

fn enforce(policy: &ResolverExecutionPolicyObservation) -> io::Result<()> {
    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(LINUX_LANDLOCK_ABI))
        .map_err(landlock_error)?
        .create()
        .map_err(landlock_error)?
        .set_compatibility(CompatLevel::HardRequirement);

    // Reads intentionally remain broad. Landlock does not mediate all metadata
    // access, so narrowing file-content reads would not justify the stronger
    // filesystem-read guarantee.
    let read_access = make_bitflags!(AccessFs::{ReadFile | ReadDir});
    ruleset = ruleset
        .add_rule(PathBeneath::new(
            PathFd::new("/").map_err(landlock_error)?,
            read_access,
        ))
        .map_err(landlock_error)?;

    let executable_access = make_bitflags!(AccessFs::{ReadFile | Execute});
    let primary_executable = PathFd::new(policy.executable()).map_err(landlock_error)?;
    require_regular_executable(&primary_executable, policy.executable())?;
    ruleset = ruleset
        .add_rule(PathBeneath::new(primary_executable, executable_access))
        .map_err(landlock_error)?;
    for executable in policy.additional_executables() {
        let executable_fd = PathFd::new(executable).map_err(landlock_error)?;
        require_regular_executable(&executable_fd, executable)?;
        ruleset = ruleset
            .add_rule(PathBeneath::new(executable_fd, executable_access))
            .map_err(landlock_error)?;
    }

    // Resolver phases use a fixed null sink. It is the only writable device,
    // and ABI v5 lets the policy close device ioctl authority everywhere else.
    let null_access = make_bitflags!(AccessFs::{ReadFile | WriteFile | IoctlDev});
    ruleset = ruleset
        .add_rule(PathBeneath::new(
            PathFd::new("/dev/null").map_err(landlock_error)?,
            null_access,
        ))
        .map_err(landlock_error)?;

    if let Some(root) = policy.mutable_root() {
        // Permit only ordinary source-cache mutation. Device, FIFO, socket, and
        // symlink creation remain denied even beneath the mutable root.
        let mutable_access = make_bitflags!(AccessFs::{
            ReadFile
                | ReadDir
                | WriteFile
                | RemoveDir
                | RemoveFile
                | MakeDir
                | MakeReg
                | Refer
                | Truncate
        });
        ruleset = ruleset
            .add_rule(PathBeneath::new(
                PathFd::new(root).map_err(landlock_error)?,
                mutable_access,
            ))
            .map_err(landlock_error)?;
    }

    let status = ruleset.restrict_self().map_err(landlock_error)?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(io::Error::other(
            "Linux resolver Landlock v5 policy was not fully enforced",
        ));
    }
    Ok(())
}

fn require_regular_executable(executable: &PathFd, path: &Path) -> io::Result<()> {
    let mut status = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(executable.as_fd().as_raw_fd(), status.as_mut_ptr()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    let status = unsafe { status.assume_init() };
    if status.st_mode & libc::S_IFMT != libc::S_IFREG {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Linux resolver executable is not a regular file: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn landlock_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("Linux resolver Landlock failure: {error}"))
}

#[cfg(test)]
mod tests;
