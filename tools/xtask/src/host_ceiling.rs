//! Doctor check for the host CPU ceiling that the pinned mbx applies.
//!
//! When the mbx scheduler budget (`scheduler.cpus` minus
//! `scheduler.reserve_cpus`) is smaller than the machine, every mbx command
//! that runs work limits itself and its descendants before starting anything,
//! and records what it applied in `MBX_HOST_CEILING`
//! ([build slot](../../../wiki/development/build_slot.md)). `mbx xtask doctor`
//! runs this check as one of those descendants, so it reads the limit off its
//! own process: the affinity mask and priority class on Windows, the cgroup or
//! affinity mask on Linux, and the Darwin background policy on macOS.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::doctor::CheckEntry;

const CEILING_ENV: &str = "MBX_HOST_CEILING";
const CEILING_CPUS_ENV: &str = "MBX_HOST_CEILING_CPUS";

/// What the operating system says about this process's CPU limit.
#[derive(Debug, PartialEq, Eq)]
pub struct Observed {
    /// CPUs the process may use, when the platform has a hard limit.
    pub cpus: Option<u64>,
    pub detail: String,
}

/// The scheduler budget in CPUs, or `None` when it covers every CPU.
pub fn expected_budget(
    cpus: Option<u64>,
    reserve: u64,
    inherited: Option<u64>,
    logical: u64,
) -> Option<u64> {
    let budget =
        inherited.unwrap_or_else(|| cpus.unwrap_or(logical).saturating_sub(reserve).max(1));
    (budget < logical).then_some(budget)
}

fn env_count(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.trim().parse().ok()
}

/// Read a scheduler setting the way mbx resolves it: environment first, then
/// the global configuration through `mbx settings get`.
fn setting(root: &Path, env: &str, key: &str) -> Option<u64> {
    env_count(env).or_else(|| {
        let output = Command::new("mbx")
            .args(["settings", "get", key])
            .current_dir(root)
            .stdin(Stdio::null())
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    })
}

pub fn check(root: &Path) -> CheckEntry {
    let logical = platform::logical_cpus();
    let expected = expected_budget(
        setting(root, "MBX_SCHEDULER_CPUS", "scheduler.cpus"),
        setting(root, "MBX_SCHEDULER_RESERVE_CPUS", "scheduler.reserve_cpus").unwrap_or(0),
        env_count(CEILING_CPUS_ENV),
        logical,
    );
    let observed = platform::observe(logical);
    let applied = std::env::var(CEILING_ENV).ok();
    entry(expected, logical, applied.as_deref(), &observed)
}

pub fn entry(
    expected: Option<u64>,
    logical: u64,
    applied: Option<&str>,
    observed: &Observed,
) -> CheckEntry {
    let Some(budget) = expected else {
        return CheckEntry {
            name: "host-ceiling",
            status: "ok",
            detail: format!(
                "none: the mbx scheduler budget covers all {logical} CPUs (set scheduler.reserve_cpus to keep CPUs free)"
            ),
            fix: None,
        };
    };
    let Some(applied) = applied else {
        return CheckEntry {
            name: "host-ceiling",
            status: "warn",
            detail: format!(
                "the mbx scheduler budget is {budget} of {logical} CPUs, but no ceiling holds this process ({})",
                observed.detail
            ),
            fix: Some(
                "run doctor through the pinned mbx (`mbx xtask doctor`), which applies the ceiling; install it with tools/xtask/bootstrap-mbx.sh".to_owned(),
            ),
        };
    };
    let within = observed.cpus.is_none_or(|cpus| cpus <= budget);
    CheckEntry {
        name: "host-ceiling",
        status: if within { "ok" } else { "warn" },
        detail: format!("{applied}; this process: {}", observed.detail),
        fix: (!within).then(|| {
            format!("mbx reported a ceiling of {budget} CPUs, but this process may use more")
        }),
    }
}

#[cfg(windows)]
mod platform {
    use super::Observed;
    use windows_sys::Win32::System::JobObjects::{
        IsProcessInJob, JOB_OBJECT_LIMIT_AFFINITY, JOBOBJECT_BASIC_LIMIT_INFORMATION,
        JobObjectBasicLimitInformation, QueryInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::{
        ALL_PROCESSOR_GROUPS, GetActiveProcessorCount, GetCurrentProcess, GetPriorityClass,
        GetProcessAffinityMask, IDLE_PRIORITY_CLASS,
    };

    pub(super) fn logical_cpus() -> u64 {
        u64::from(unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) }).max(1)
    }

    pub(super) fn observe(logical: u64) -> Observed {
        let process = unsafe { GetCurrentProcess() };
        let (mut affinity, mut system) = (0usize, 0usize);
        let cpus = (unsafe { GetProcessAffinityMask(process, &mut affinity, &mut system) } != 0)
            .then(|| u64::from(affinity.count_ones()));
        let idle = unsafe { GetPriorityClass(process) } == IDLE_PRIORITY_CLASS;
        let mut member = 0;
        let in_job = unsafe { IsProcessInJob(process, std::ptr::null_mut(), &mut member) } != 0
            && member != 0;
        // A null job handle queries the innermost job holding this process.
        let mut limits = JOBOBJECT_BASIC_LIMIT_INFORMATION::default();
        let job_affinity = (in_job
            && unsafe {
                QueryInformationJobObject(
                    std::ptr::null_mut(),
                    JobObjectBasicLimitInformation,
                    (&mut limits as *mut JOBOBJECT_BASIC_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_BASIC_LIMIT_INFORMATION>() as u32,
                    std::ptr::null_mut(),
                )
            } != 0
            && limits.LimitFlags & JOB_OBJECT_LIMIT_AFFINITY != 0)
            .then_some(limits.Affinity);
        let mut detail = match cpus {
            Some(cpus) => format!("{cpus} of {logical} CPUs (affinity {affinity:#x})"),
            None => "affinity unreadable".to_owned(),
        };
        detail.push_str(if idle {
            ", Idle class"
        } else {
            ", not Idle class"
        });
        if let Some(mask) = job_affinity {
            detail.push_str(&format!(", innermost job enforces affinity {mask:#x}"));
        }
        Observed { cpus, detail }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::Observed;

    pub(super) fn logical_cpus() -> u64 {
        u64::try_from(unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) })
            .unwrap_or(1)
            .max(1)
    }

    /// The CPU quota of the `omega-host.slice` cgroup this process sits in.
    pub fn slice_quota(
        groups: &str,
        read: impl Fn(&str) -> Option<String>,
    ) -> Option<(u64, String)> {
        let path = groups.lines().find_map(|line| line.strip_prefix("0::"))?;
        let end = path.find("/omega-host.slice")? + "/omega-host.slice".len();
        let slice = &path[..end];
        let max = read(&format!("/sys/fs/cgroup{slice}/cpu.max"))?;
        let mut fields = max.split_whitespace();
        let quota: u64 = fields.next()?.parse().ok()?;
        let period: u64 = fields.next()?.parse().ok()?;
        Some((
            quota / period.max(1),
            format!("cgroup {slice} cpu.max {}", max.trim()),
        ))
    }

    pub(super) fn observe(logical: u64) -> Observed {
        let read = |path: &str| std::fs::read_to_string(path).ok();
        if let Some((cpus, detail)) =
            read("/proc/self/cgroup").and_then(|groups| slice_quota(&groups, read))
        {
            return Observed {
                cpus: Some(cpus),
                detail,
            };
        }
        let mut allowed: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<libc::cpu_set_t>();
        let cpus = (unsafe { libc::sched_getaffinity(0, size, &mut allowed) } == 0)
            .then(|| unsafe { libc::CPU_COUNT(&allowed) } as u64);
        let nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, 0) };
        Observed {
            cpus,
            detail: format!(
                "{} of {logical} CPUs by affinity, nice {nice}",
                cpus.map_or("unknown".to_owned(), |cpus| cpus.to_string())
            ),
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::Observed;

    pub(super) fn logical_cpus() -> u64 {
        std::thread::available_parallelism().map_or(1, |cpus| cpus.get() as u64)
    }

    pub(super) fn observe(_logical: u64) -> Observed {
        let background = (unsafe { libc::getpriority(libc::PRIO_DARWIN_PROCESS, 0) }) == 1;
        Observed {
            cpus: None,
            detail: if background {
                "Darwin background policy (no hard ceiling on macOS)".to_owned()
            } else {
                "not in the Darwin background policy".to_owned()
            },
        }
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod platform {
    use super::Observed;

    pub(super) fn logical_cpus() -> u64 {
        std::thread::available_parallelism().map_or(1, |cpus| cpus.get() as u64)
    }

    pub(super) fn observe(_logical: u64) -> Observed {
        Observed {
            cpus: None,
            detail: "no host CPU ceiling on this platform".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_budget_matches_mbx_resolution() {
        assert_eq!(expected_budget(None, 0, None, 32), None);
        assert_eq!(expected_budget(None, 4, None, 32), Some(28));
        assert_eq!(expected_budget(Some(28), 0, None, 32), Some(28));
        assert_eq!(expected_budget(Some(40), 0, None, 32), None);
        // Inside a Linux quota, mbx's default CPU count shrinks; the budget an
        // ancestor applied wins.
        assert_eq!(expected_budget(Some(28), 4, Some(28), 32), Some(28));
    }

    #[test]
    fn a_budget_with_no_applied_ceiling_warns() {
        let observed = Observed {
            cpus: Some(32),
            detail: "32 of 32 CPUs".to_owned(),
        };
        assert_eq!(entry(None, 32, None, &observed).status, "ok");
        let missing = entry(Some(28), 32, None, &observed);
        assert_eq!(missing.status, "warn");
        assert!(
            missing
                .fix
                .is_some_and(|fix| fix.contains("mbx xtask doctor"))
        );
        let widened = entry(Some(28), 32, Some("windows-job cpus=28/32"), &observed);
        assert_eq!(widened.status, "warn");
        let held = Observed {
            cpus: Some(28),
            detail: "28 of 32 CPUs".to_owned(),
        };
        let ok = entry(Some(28), 32, Some("windows-job cpus=28/32"), &held);
        assert_eq!(ok.status, "ok");
        assert!(
            ok.detail
                .starts_with("windows-job cpus=28/32; this process: 28 of 32")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reads_the_slice_quota() {
        let groups = "0::/user.slice/user-1001.slice/user@1001.service/omega.slice/omega-host.slice/omega-host-mbx-7-1.scope\n";
        let quota = platform::slice_quota(groups, |path| {
            (path
                == "/sys/fs/cgroup/user.slice/user-1001.slice/user@1001.service/omega.slice/omega-host.slice/cpu.max")
                .then(|| "1200000 100000\n".to_owned())
        });
        assert_eq!(quota.map(|(cpus, _)| cpus), Some(12));
    }
}
