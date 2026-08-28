use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus};
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
use windows_sys::Win32::System::Threading::{
    CREATE_SUSPENDED, GetProcessId, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
};

use super::{
    CHILD_AGGREGATE_CPU_SECONDS, CHILD_AGGREGATE_MEMORY_BYTES, CHILD_PROCESS_LIMIT,
    CHILD_PROCESS_MEMORY_BYTES,
};

const JOB_TERMINATION_EXIT_CODE: u32 = 1;
const SNAPSHOT_ATTEMPT_LIMIT: usize = 8;
const WINDOWS_TIME_TICKS_PER_SECOND: u64 = 10_000_000;

/// A resolver child contained in a compiler-owned Windows Job Object.
#[derive(Debug)]
pub(crate) struct WindowsJobChild {
    child: Child,
    job: KernelHandle,
    completion_port: KernelHandle,
    primary_status: Option<ExitStatus>,
    active_processes_zero: bool,
}

impl WindowsJobChild {
    pub(crate) fn spawn(command: &mut Command) -> io::Result<Self> {
        let job = create_job()?;
        let completion_port = create_completion_port()?;

        configure_job_limits(&job)?;
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
        })
    }

    pub(crate) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
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
        }
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

fn configure_job_limits(job: &KernelHandle) -> io::Result<()> {
    let process_memory_limit = usize::try_from(CHILD_PROCESS_MEMORY_BYTES).map_err(|_| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "the Windows target cannot represent the process memory limit",
        )
    })?;
    let job_memory_limit = usize::try_from(CHILD_AGGREGATE_MEMORY_BYTES).map_err(|_| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "the Windows target cannot represent the job memory limit",
        )
    })?;

    let active_process_limit = u32::try_from(CHILD_PROCESS_LIMIT).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "the Windows active-process limit exceeds the API field",
        )
    })?;
    let job_time_limit = CHILD_AGGREGATE_CPU_SECONDS
        .checked_mul(WINDOWS_TIME_TICKS_PER_SECOND)
        .and_then(|ticks| i64::try_from(ticks).ok())
        .ok_or_else(|| {
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
    use super::WindowsJobChild;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    fn wait_bounded(child: &mut WindowsJobChild) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
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

    #[test]
    fn job_reaches_active_process_zero_after_normal_exit() {
        let mut command = Command::new("cmd.exe");
        command
            .args(["/D", "/S", "/C", "exit /b 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = WindowsJobChild::spawn(&mut command).expect("spawn contained child");
        assert!(wait_bounded(&mut child).success());
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
        assert!(!wait_bounded(&mut child).success());
    }
}
