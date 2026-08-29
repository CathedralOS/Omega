use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    ERROR_BAD_LENGTH, ERROR_NO_MORE_FILES, FALSE, HANDLE, INVALID_HANDLE_VALUE, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::IO::{CreateIoCompletionPort, GetQueuedCompletionStatus};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_JOB_MEMORY,
    JOB_OBJECT_LIMIT_JOB_TIME, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
    JOBOBJECT_ASSOCIATE_COMPLETION_PORT, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectAssociateCompletionPortInformation, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::SystemServices::JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO;
#[cfg(test)]
use windows_sys::Win32::System::SystemServices::{
    JOB_OBJECT_MSG_ACTIVE_PROCESS_LIMIT, JOB_OBJECT_MSG_END_OF_JOB_TIME,
    JOB_OBJECT_MSG_JOB_MEMORY_LIMIT, JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT,
};
use windows_sys::Win32::System::Threading::{
    CREATE_SUSPENDED, GetProcessId, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
};

use crate::process::limits::{
    CHILD_AGGREGATE_CPU_SECONDS, CHILD_AGGREGATE_MEMORY_BYTES, CHILD_PROCESS_LIMIT,
    CHILD_PROCESS_MEMORY_BYTES,
};

const JOB_TERMINATION_EXIT_CODE: u32 = 1;
const SNAPSHOT_ATTEMPT_LIMIT: usize = 8;
const WINDOWS_TIME_TICKS_PER_SECOND: u64 = 10_000_000;

#[derive(Debug, Clone, Copy)]
struct WindowsJobLimits {
    active_processes: u64,
    process_memory_bytes: u64,
    aggregate_memory_bytes: u64,
    aggregate_cpu_ticks: u64,
}

impl WindowsJobLimits {
    fn production() -> io::Result<Self> {
        let aggregate_cpu_ticks = CHILD_AGGREGATE_CPU_SECONDS
            .checked_mul(WINDOWS_TIME_TICKS_PER_SECOND)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "the Windows aggregate CPU limit exceeds the API field",
                )
            })?;
        Ok(Self {
            active_processes: CHILD_PROCESS_LIMIT,
            process_memory_bytes: CHILD_PROCESS_MEMORY_BYTES,
            aggregate_memory_bytes: CHILD_AGGREGATE_MEMORY_BYTES,
            aggregate_cpu_ticks,
        })
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobLimitEvent {
    ActiveProcess,
    ProcessMemory,
    AggregateMemory,
    AggregateCpu,
}

/// A resolver child contained in a compiler-owned Windows Job Object.
#[derive(Debug)]
pub(crate) struct WindowsJobChild {
    child: Child,
    job: KernelHandle,
    completion_port: KernelHandle,
    primary_status: Option<ExitStatus>,
    active_processes_zero: bool,
    #[cfg(test)]
    limit_events: Vec<JobLimitEvent>,
}

impl WindowsJobChild {
    pub(crate) fn spawn(command: &mut Command) -> io::Result<Self> {
        Self::spawn_with_config(command, WindowsJobLimits::production()?)
    }

    fn spawn_with_config(command: &mut Command, limits: WindowsJobLimits) -> io::Result<Self> {
        let job = create_job()?;
        let completion_port = create_completion_port()?;

        configure_job_limits(&job, limits)?;
        associate_completion_port(&job, &completion_port)?;

        command.creation_flags(CREATE_SUSPENDED);
        let mut child = command.spawn()?;

        let setup_result = assign_and_resume(&mut child, &job);
        if let Err(setup_error) = setup_result {
            terminate_and_reap_setup_child(&mut child, &job);
            return Err(setup_error);
        }

        Ok(Self {
            child,
            job,
            completion_port,
            primary_status: None,
            active_processes_zero: false,
            #[cfg(test)]
            limit_events: Vec::new(),
        })
    }

    pub(crate) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub(crate) fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    pub(crate) fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub(crate) fn kill(&mut self) -> io::Result<()> {
        bool_result(unsafe { TerminateJobObject(self.job.raw(), JOB_TERMINATION_EXIT_CODE) })
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.drain_job_notifications()?;

        if self.primary_status.is_none() {
            self.primary_status = self.child.try_wait()?;
        }

        if self.active_processes_zero {
            Ok(self.primary_status)
        } else {
            Ok(None)
        }
    }

    fn drain_job_notifications(&mut self) -> io::Result<()> {
        loop {
            let mut message = 0_u32;
            let mut completion_key = 0_usize;
            let mut overlapped = null_mut();
            let result = unsafe {
                GetQueuedCompletionStatus(
                    self.completion_port.raw(),
                    &mut message,
                    &mut completion_key,
                    &mut overlapped,
                    0,
                )
            };

            if result == FALSE {
                let error = io::Error::last_os_error();
                if raw_os_error_is(&error, WAIT_TIMEOUT) && overlapped.is_null() {
                    return Ok(());
                }
                return Err(error);
            }

            if completion_key != self.job.raw() as usize {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows Job Object completion key did not match its owner",
                ));
            }

            if message == JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO {
                self.active_processes_zero = true;
            }
            #[cfg(test)]
            if let Some(event) = job_limit_event(message)
                && !self.limit_events.contains(&event)
            {
                self.limit_events.push(event);
            }
        }
    }
}

#[cfg(test)]
fn job_limit_event(message: u32) -> Option<JobLimitEvent> {
    match message {
        JOB_OBJECT_MSG_ACTIVE_PROCESS_LIMIT => Some(JobLimitEvent::ActiveProcess),
        JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT => Some(JobLimitEvent::ProcessMemory),
        JOB_OBJECT_MSG_JOB_MEMORY_LIMIT => Some(JobLimitEvent::AggregateMemory),
        JOB_OBJECT_MSG_END_OF_JOB_TIME => Some(JobLimitEvent::AggregateCpu),
        _ => None,
    }
}

impl Drop for WindowsJobChild {
    fn drop(&mut self) {
        let _ = unsafe { TerminateJobObject(self.job.raw(), JOB_TERMINATION_EXIT_CODE) };
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Debug)]
struct KernelHandle(OwnedHandle);

impl KernelHandle {
    fn from_nullable(raw: HANDLE) -> io::Result<Self> {
        if raw.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(unsafe { OwnedHandle::from_raw_handle(raw) }))
        }
    }

    fn from_snapshot(raw: HANDLE) -> io::Result<Self> {
        if raw == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Self::from_nullable(raw)
        }
    }

    fn raw(&self) -> HANDLE {
        self.0.as_raw_handle()
    }
}

fn create_job() -> io::Result<KernelHandle> {
    KernelHandle::from_nullable(unsafe { CreateJobObjectW(null(), null()) })
}

fn create_completion_port() -> io::Result<KernelHandle> {
    KernelHandle::from_nullable(unsafe {
        CreateIoCompletionPort(INVALID_HANDLE_VALUE, null_mut(), 0, 1)
    })
}

fn configure_job_limits(job: &KernelHandle, configured: WindowsJobLimits) -> io::Result<()> {
    let process_memory_limit = usize::try_from(configured.process_memory_bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "the Windows target cannot represent the process memory limit",
        )
    })?;
    let job_memory_limit = usize::try_from(configured.aggregate_memory_bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "the Windows target cannot represent the job memory limit",
        )
    })?;

    let active_process_limit = u32::try_from(configured.active_processes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "the Windows active-process limit exceeds the API field",
        )
    })?;
    let job_time_limit = i64::try_from(configured.aggregate_cpu_ticks).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "the Windows aggregate CPU limit exceeds the API field",
        )
    })?;

    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_JOB_MEMORY
        | JOB_OBJECT_LIMIT_JOB_TIME;
    limits.BasicLimitInformation.ActiveProcessLimit = active_process_limit;
    limits.BasicLimitInformation.PerJobUserTimeLimit = job_time_limit;
    limits.ProcessMemoryLimit = process_memory_limit;
    limits.JobMemoryLimit = job_memory_limit;

    set_job_information(
        job,
        JobObjectExtendedLimitInformation,
        &limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    )
}

fn associate_completion_port(job: &KernelHandle, completion_port: &KernelHandle) -> io::Result<()> {
    let association = JOBOBJECT_ASSOCIATE_COMPLETION_PORT {
        CompletionKey: job.raw(),
        CompletionPort: completion_port.raw(),
    };
    set_job_information(
        job,
        JobObjectAssociateCompletionPortInformation,
        &association as *const JOBOBJECT_ASSOCIATE_COMPLETION_PORT,
    )
}

fn set_job_information<T>(job: &KernelHandle, class: i32, information: *const T) -> io::Result<()> {
    let information_size = u32::try_from(size_of::<T>()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows Job Object information exceeds the API size limit",
        )
    })?;
    bool_result(unsafe {
        SetInformationJobObject(
            job.raw(),
            class,
            information.cast::<c_void>(),
            information_size,
        )
    })
}

fn assign_and_resume(child: &mut Child, job: &KernelHandle) -> io::Result<()> {
    let process_handle = child.as_raw_handle();
    bool_result(unsafe { AssignProcessToJobObject(job.raw(), process_handle) })?;
    resume_process_threads(process_handle)
}

fn resume_process_threads(process: HANDLE) -> io::Result<()> {
    let process_id = unsafe { GetProcessId(process) };
    if process_id == 0 {
        return Err(io::Error::last_os_error());
    }

    let snapshot = create_thread_snapshot()?;
    let entry_size = u32::try_from(size_of::<THREADENTRY32>()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows thread entry exceeds the API size limit",
        )
    })?;
    let mut entry = THREADENTRY32 {
        dwSize: entry_size,
        ..THREADENTRY32::default()
    };
    bool_result(unsafe { Thread32First(snapshot.raw(), &mut entry) })?;

    let mut thread_ids = Vec::new();
    loop {
        if entry.th32OwnerProcessID == process_id {
            thread_ids.push(entry.th32ThreadID);
        }

        let next = unsafe { Thread32Next(snapshot.raw(), &mut entry) };
        if next == FALSE {
            let error = io::Error::last_os_error();
            if raw_os_error_is(&error, ERROR_NO_MORE_FILES) {
                break;
            }
            return Err(error);
        }
    }

    if thread_ids.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the suspended Windows child had no discoverable thread",
        ));
    }

    for thread_id in thread_ids {
        let thread = KernelHandle::from_nullable(unsafe {
            OpenThread(THREAD_SUSPEND_RESUME, FALSE, thread_id)
        })?;
        resume_thread_fully(&thread)?;
    }

    Ok(())
}

fn create_thread_snapshot() -> io::Result<KernelHandle> {
    let mut last_error = None;
    for _ in 0..SNAPSHOT_ATTEMPT_LIMIT {
        match KernelHandle::from_snapshot(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })
        {
            Ok(snapshot) => return Ok(snapshot),
            Err(error) if raw_os_error_is(&error, ERROR_BAD_LENGTH) => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or_else(io::Error::last_os_error))
}

fn resume_thread_fully(thread: &KernelHandle) -> io::Result<()> {
    loop {
        let previous_suspend_count = unsafe { ResumeThread(thread.raw()) };
        if previous_suspend_count == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        if previous_suspend_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a Windows child thread was not suspended before release",
            ));
        }
        if previous_suspend_count == 1 {
            return Ok(());
        }
    }
}

fn terminate_and_reap_setup_child(child: &mut Child, job: &KernelHandle) {
    let _ = unsafe { TerminateJobObject(job.raw(), JOB_TERMINATION_EXIT_CODE) };
    let _ = child.kill();
    let _ = child.wait();
}

fn bool_result(result: i32) -> io::Result<()> {
    if result == FALSE {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn raw_os_error_is(error: &io::Error, expected: u32) -> bool {
    error
        .raw_os_error()
        .and_then(|code| u32::try_from(code).ok())
        == Some(expected)
}

#[cfg(test)]
mod tests {
    use super::{JobLimitEvent, WindowsJobChild, WindowsJobLimits};
    use std::hint::black_box;
    use std::process::{Command, Stdio};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    const WORKER_MODE: &str = "OMEGA_RESOLVER_WINDOWS_JOB_WORKER";
    const WORKER_VALUE: &str = "OMEGA_RESOLVER_WINDOWS_JOB_WORKER_VALUE";
    const WORKER_TEST: &str = "windows::tests::job_limit_worker";
    const MIB: u64 = 1024 * 1024;
    static JOB_LIMIT_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn wait_bounded(child: &mut WindowsJobChild, timeout: Duration) -> std::process::ExitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait().expect("query Windows Job completion") {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "Windows Job did not reach active-process zero"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn test_limits(
        active_processes: u64,
        process_memory_mib: u64,
        aggregate_memory_mib: u64,
        aggregate_cpu: Duration,
    ) -> WindowsJobLimits {
        let aggregate_cpu_ticks = u64::try_from(aggregate_cpu.as_nanos() / u128::from(100_u64))
            .expect("test CPU limit fits Windows ticks");
        WindowsJobLimits {
            active_processes,
            process_memory_bytes: process_memory_mib * MIB,
            aggregate_memory_bytes: aggregate_memory_mib * MIB,
            aggregate_cpu_ticks,
        }
    }

    fn worker_command(mode: &str, value: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().expect("resolve test executable"));
        command
            .args(["--exact", WORKER_TEST, "--test-threads=1"])
            .env(WORKER_MODE, mode)
            .env(WORKER_VALUE, value)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    fn run_worker(
        mode: &str,
        value: &str,
        limits: WindowsJobLimits,
        timeout: Duration,
    ) -> (std::process::ExitStatus, Vec<JobLimitEvent>) {
        let mut command = worker_command(mode, value);
        let mut child = WindowsJobChild::spawn_with_config(&mut command, limits)
            .expect("spawn limited Windows Job worker");
        let status = wait_bounded(&mut child, timeout);
        (status, child.limit_events.clone())
    }

    fn parse_worker_values(value: &str, expected: usize) -> Vec<usize> {
        let values = value
            .split(',')
            .map(|field| field.parse::<usize>().expect("parse worker value"))
            .collect::<Vec<_>>();
        assert_eq!(values.len(), expected, "unexpected worker value shape");
        values
    }

    fn touch_memory(bytes: usize, hold_millis: usize) {
        let mut memory = Vec::new();
        memory
            .try_reserve_exact(bytes)
            .expect("worker memory reservation should remain below its expected limit");
        memory.resize(bytes, 0x5a_u8);
        black_box(&memory);
        std::thread::sleep(Duration::from_millis(
            u64::try_from(hold_millis).expect("hold duration fits u64"),
        ));
    }

    fn wait_for_all(children: &mut [std::process::Child]) -> bool {
        let mut every_child_succeeded = true;
        for child in children {
            every_child_succeeded &= child.wait().is_ok_and(|status| status.success());
        }
        every_child_succeeded
    }

    #[test]
    fn job_limit_worker() {
        let Ok(mode) = std::env::var(WORKER_MODE) else {
            return;
        };
        let value = std::env::var(WORKER_VALUE).expect("limited worker value");
        match mode.as_str() {
            "hold" => {
                let millis = value.parse::<u64>().expect("parse hold duration");
                std::thread::sleep(Duration::from_millis(millis));
            }
            "fanout" => {
                let values = parse_worker_values(&value, 2);
                let expected_success = values[1] != 0;
                let mut children = Vec::new();
                let mut every_spawn_succeeded = true;
                for _ in 0..values[0] {
                    match worker_command("hold", "750").spawn() {
                        Ok(child) => children.push(child),
                        Err(_) => every_spawn_succeeded = false,
                    }
                }
                let every_child_succeeded = wait_for_all(&mut children);
                assert_eq!(
                    every_spawn_succeeded && every_child_succeeded,
                    expected_success,
                    "active-process worker observed an unexpected fanout result"
                );
            }
            "touch" => {
                let values = parse_worker_values(&value, 2);
                touch_memory(values[0], values[1]);
            }
            "aggregate-memory" => {
                let values = parse_worker_values(&value, 3);
                let expected_success = values[2] != 0;
                let mut children = (0..values[0])
                    .filter_map(|_| {
                        worker_command("touch", &format!("{},750", values[1]))
                            .spawn()
                            .ok()
                    })
                    .collect::<Vec<_>>();
                let every_child_spawned = children.len() == values[0];
                let child_statuses_succeeded = wait_for_all(&mut children);
                let every_child_succeeded = every_child_spawned && child_statuses_succeeded;
                assert_eq!(
                    every_child_succeeded, expected_success,
                    "aggregate-memory worker observed an unexpected child result"
                );
            }
            "spin" => {
                let deadline = Instant::now()
                    + Duration::from_millis(value.parse::<u64>().expect("parse spin duration"));
                let mut value = 0_u64;
                while Instant::now() < deadline {
                    value = black_box(value.wrapping_mul(6364136223846793005).wrapping_add(1));
                }
                black_box(value);
            }
            _ => panic!("unknown Windows Job test worker mode `{mode}`"),
        }
    }

    #[test]
    fn job_reaches_active_process_zero_after_normal_exit() {
        let mut command = Command::new("cmd.exe");
        command
            .args(["/D", "/S", "/C", "exit /b 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = WindowsJobChild::spawn(&mut command).expect("spawn contained child");
        assert!(wait_bounded(&mut child, Duration::from_secs(5)).success());
    }

    #[test]
    fn job_termination_reaches_active_process_zero() {
        let mut command = Command::new("cmd.exe");
        command
            .args(["/D", "/S", "/C", "ping -n 30 127.0.0.1 >nul"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = WindowsJobChild::spawn(&mut command).expect("spawn contained child");
        child.kill().expect("terminate complete Windows Job");
        assert!(!wait_bounded(&mut child, Duration::from_secs(5)).success());
    }

    #[test]
    fn job_active_process_limit_rejects_excess_descendant() {
        let _serial = JOB_LIMIT_TEST_LOCK.lock().expect("lock Job limit tests");
        let control = test_limits(4, 512, 1024, Duration::from_secs(30));
        let (status, events) = run_worker("fanout", "2,1", control, Duration::from_secs(10));
        assert!(status.success(), "below-limit fanout should succeed");
        assert!(!events.contains(&JobLimitEvent::ActiveProcess));

        let limited = test_limits(2, 512, 1024, Duration::from_secs(30));
        let (status, events) = run_worker("fanout", "2,0", limited, Duration::from_secs(10));
        assert!(
            status.success(),
            "worker should observe the rejected excess child"
        );
        assert!(events.contains(&JobLimitEvent::ActiveProcess));
    }

    #[test]
    fn job_process_memory_limit_blocks_excess_commit() {
        let _serial = JOB_LIMIT_TEST_LOCK.lock().expect("lock Job limit tests");
        let control = test_limits(4, 256, 512, Duration::from_secs(30));
        let (status, events) = run_worker(
            "touch",
            &format!("{},0", 32 * MIB),
            control,
            Duration::from_secs(10),
        );
        assert!(
            status.success(),
            "below-limit process memory should succeed"
        );
        assert!(!events.contains(&JobLimitEvent::ProcessMemory));

        let limited = test_limits(4, 128, 512, Duration::from_secs(30));
        let (status, events) = run_worker(
            "touch",
            &format!("{},0", 256 * MIB),
            limited,
            Duration::from_secs(10),
        );
        assert!(!status.success(), "over-limit process memory must fail");
        assert!(events.contains(&JobLimitEvent::ProcessMemory));
    }

    #[test]
    fn job_aggregate_memory_limit_spans_descendants() {
        let _serial = JOB_LIMIT_TEST_LOCK.lock().expect("lock Job limit tests");
        let control = test_limits(4, 256, 512, Duration::from_secs(30));
        let (status, events) = run_worker(
            "aggregate-memory",
            &format!("1,{},1", 32 * MIB),
            control,
            Duration::from_secs(10),
        );
        assert!(
            status.success(),
            "below-limit aggregate memory should succeed"
        );
        assert!(!events.contains(&JobLimitEvent::AggregateMemory));

        let limited = test_limits(4, 256, 256, Duration::from_secs(30));
        let (status, events) = run_worker(
            "aggregate-memory",
            &format!("2,{},0", 160 * MIB),
            limited,
            Duration::from_secs(15),
        );
        assert!(
            status.success(),
            "worker should observe an aggregate-memory child failure"
        );
        assert!(events.contains(&JobLimitEvent::AggregateMemory));
    }

    #[test]
    fn job_aggregate_cpu_limit_terminates_the_job() {
        let _serial = JOB_LIMIT_TEST_LOCK.lock().expect("lock Job limit tests");
        let control = test_limits(2, 256, 512, Duration::from_secs(5));
        let (status, events) = run_worker("spin", "100", control, Duration::from_secs(10));
        assert!(status.success(), "below-limit aggregate CPU should succeed");
        assert!(!events.contains(&JobLimitEvent::AggregateCpu));

        let limited = test_limits(2, 256, 512, Duration::from_secs(1));
        let (status, events) = run_worker("spin", "5000", limited, Duration::from_secs(10));
        assert!(!status.success(), "over-limit aggregate CPU must terminate");
        assert!(events.contains(&JobLimitEvent::AggregateCpu));
    }
}
