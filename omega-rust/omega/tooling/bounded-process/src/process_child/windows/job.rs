use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::IO::CreateIoCompletionPort;
use windows_sys::Win32::System::JobObjects::{
    CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION,
    JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_JOB_TIME, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOBOBJECT_ASSOCIATE_COMPLETION_PORT,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectAssociateCompletionPortInformation,
    JobObjectExtendedLimitInformation, SetInformationJobObject,
};

use super::limits::WindowsJobLimits;
use super::native::{KernelHandle, bool_result};

pub(super) fn create_job() -> io::Result<KernelHandle> {
    KernelHandle::from_nullable(unsafe { CreateJobObjectW(null(), null()) })
}

pub(super) fn create_completion_port() -> io::Result<KernelHandle> {
    KernelHandle::from_nullable(unsafe {
        CreateIoCompletionPort(INVALID_HANDLE_VALUE, null_mut(), 0, 1)
    })
}

pub(super) fn configure_job_limits(
    job: &KernelHandle,
    configured: WindowsJobLimits,
) -> io::Result<()> {
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

pub(super) fn associate_completion_port(
    job: &KernelHandle,
    completion_port: &KernelHandle,
) -> io::Result<()> {
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
