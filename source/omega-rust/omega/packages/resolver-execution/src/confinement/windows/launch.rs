use std::io;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::process::Child;

use windows_sys::Win32::Foundation::{ERROR_BAD_LENGTH, ERROR_NO_MORE_FILES, FALSE, HANDLE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
use windows_sys::Win32::System::Threading::{
    GetProcessId, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
};

use super::native::{KernelHandle, bool_result, raw_os_error_is};

const SNAPSHOT_ATTEMPT_LIMIT: usize = 8;

pub(super) fn assign_and_resume(child: &mut Child, job: &KernelHandle) -> io::Result<()> {
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
