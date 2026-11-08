//! Opt-in kernel observations retained before the child's native handle closes.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessResourceUsage {
    pub provider: &'static str,
    pub scope: &'static str,
    pub user_cpu_ns: Option<u64>,
    pub system_cpu_ns: Option<u64>,
    pub peak_resident_bytes: Option<u64>,
    pub minor_page_faults: Option<u64>,
    pub major_page_faults: Option<u64>,
    pub total_page_faults: Option<u64>,
    pub cpu_cycles: Option<u64>,
    pub error: Option<String>,
}

impl ProcessResourceUsage {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            provider: "unavailable",
            scope: "unavailable",
            user_cpu_ns: None,
            system_cpu_ns: None,
            peak_resident_bytes: None,
            minor_page_faults: None,
            major_page_faults: None,
            total_page_faults: None,
            cpu_cycles: None,
            error: Some(reason.into()),
        }
    }
}

#[cfg(unix)]
pub(super) fn try_wait_with_usage(
    pid: u32,
) -> std::io::Result<Option<(std::process::ExitStatus, ProcessResourceUsage)>> {
    use std::os::unix::process::ExitStatusExt;
    let pid = i32::try_from(pid).map_err(std::io::Error::other)?;
    let mut status = 0;
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: this owner alone reaps this exact child. Both output pointers
    // are valid; rusage is read only when wait4 reports that child as reaped.
    let waited = unsafe { libc::wait4(pid, &mut status, libc::WNOHANG, usage.as_mut_ptr()) };
    if waited == 0 {
        return Ok(None);
    }
    if waited < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            return Ok(None);
        }
        return Err(error);
    }
    // SAFETY: successful wait4 initialized the complete resource structure.
    let usage = unsafe { usage.assume_init() };
    let nanoseconds = |time: libc::timeval| {
        u64::try_from(time.tv_sec)
            .ok()?
            .checked_mul(1_000_000_000)?
            .checked_add(u64::try_from(time.tv_usec).ok()?.checked_mul(1000)?)
    };
    #[cfg(target_os = "macos")]
    let rss_multiplier = 1;
    #[cfg(not(target_os = "macos"))]
    let rss_multiplier = 1024;
    Ok(Some((
        std::process::ExitStatus::from_raw(status),
        ProcessResourceUsage {
            provider: "wait4_rusage",
            scope: "exact child; includes reaped descendant usage as defined by the OS",
            user_cpu_ns: nanoseconds(usage.ru_utime),
            system_cpu_ns: nanoseconds(usage.ru_stime),
            peak_resident_bytes: u64::try_from(usage.ru_maxrss)
                .ok()
                .and_then(|rss| rss.checked_mul(rss_multiplier)),
            minor_page_faults: u64::try_from(usage.ru_minflt).ok(),
            major_page_faults: u64::try_from(usage.ru_majflt).ok(),
            total_page_faults: None,
            cpu_cycles: None,
            error: None,
        },
    )))
}

#[cfg(windows)]
pub(super) fn windows_usage(
    process: windows_sys::Win32::Foundation::HANDLE,
) -> ProcessResourceUsage {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetProcessTimes;
    use windows_sys::Win32::System::WindowsProgramming::QueryProcessCycleTime;
    let mut result = ProcessResourceUsage::unavailable("kernel observations were not collected");
    result.provider = "Win32_GetProcessTimes_GetProcessMemoryInfo_QueryProcessCycleTime";
    result.scope = "primary process only; excludes descendants";
    result.error = None;
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: the child owns a live process handle through completion. Each
    // API receives writable, correctly sized output storage and no aliases.
    if unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) } != 0 {
        let ns = |time: FILETIME| {
            ((u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime))
                .checked_mul(100)
        };
        result.user_cpu_ns = ns(user);
        result.system_cpu_ns = ns(kernel);
    } else {
        result.error = Some(format!(
            "GetProcessTimes: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut memory = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    // SAFETY: memory.cb matches the pointed-to initialized structure.
    if unsafe { GetProcessMemoryInfo(process, &mut memory, memory.cb) } != 0 {
        result.peak_resident_bytes = u64::try_from(memory.PeakWorkingSetSize).ok();
        result.total_page_faults = Some(u64::from(memory.PageFaultCount));
    } else {
        result.error = Some(format!(
            "GetProcessMemoryInfo: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut cycles = 0;
    // SAFETY: cycles is a writable u64 and process is still owned.
    if unsafe { QueryProcessCycleTime(process, &mut cycles) } != 0 {
        result.cpu_cycles = Some(cycles);
    } else {
        result.error = Some(format!(
            "QueryProcessCycleTime: {}",
            std::io::Error::last_os_error()
        ));
    }
    result
}
