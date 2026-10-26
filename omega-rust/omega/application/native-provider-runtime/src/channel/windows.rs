//! A local named pipe with authenticated process endpoints. No additional
//! inheritable handle is enabled, so the bounded launch keeps its original
//! standard-stream setup and suspended-launch/Job Object lifecycle.

use std::{
    io::{self, Read, Write},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE,
        INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Storage::FileSystem::{
        CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, OPEN_EXISTING,
        PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
    },
    System::{
        IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId,
            GetNamedPipeServerProcessId, PIPE_REJECT_REMOTE_CLIENTS,
        },
        Threading::{CreateEventW, WaitForSingleObject},
    },
};

pub(crate) struct PreparedChannel {
    pub(crate) name: String,
    pipe: Channel,
}
pub(crate) struct Channel {
    handle: OwnedHandle,
    timeout: Duration,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}
fn milliseconds(timeout: Duration) -> u32 {
    timeout.as_millis().clamp(1, u128::from(u32::MAX - 1)) as u32
}
fn owned(raw: windows_sys::Win32::Foundation::HANDLE) -> io::Result<OwnedHandle> {
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
    }
}
fn event() -> io::Result<OwnedHandle> {
    owned(unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) })
}

pub(crate) fn prepare(timeout: Duration) -> io::Result<PreparedChannel> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let ordinal = NEXT
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| io::Error::other("native worker channel identities exhausted"))?;
    let instant = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    // The name supplies uniqueness, not authentication. Both peers verify the
    // OS-reported process identity before any adapter or request is admitted.
    let name = format!(
        r"\\.\pipe\omega-native-provider-{}-{instant}-{ordinal}",
        std::process::id()
    );
    let handle = owned(unsafe {
        CreateNamedPipeW(
            wide(&name).as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_REJECT_REMOTE_CLIENTS,
            1,
            65536,
            65536,
            milliseconds(timeout),
            std::ptr::null(),
        )
    })?;
    Ok(PreparedChannel {
        name,
        pipe: Channel { handle, timeout },
    })
}

impl PreparedChannel {
    pub(crate) fn accept(self, child_process: u32) -> io::Result<Channel> {
        let event = event()?;
        let mut operation = OVERLAPPED {
            hEvent: event.as_raw_handle(),
            ..Default::default()
        };
        let operation = &raw mut operation;
        let started = unsafe { ConnectNamedPipe(self.pipe.handle.as_raw_handle(), operation) };
        let error = io::Error::last_os_error();
        if started == 0 && error.raw_os_error() != Some(ERROR_PIPE_CONNECTED as i32) {
            unsafe {
                self.pipe
                    .complete(operation, event.as_raw_handle(), started, error)?;
            }
        }
        let mut actual = 0;
        if unsafe { GetNamedPipeClientProcessId(self.pipe.handle.as_raw_handle(), &mut actual) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }
        if actual != child_process {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "native worker pipe connected by another process",
            ));
        }
        Ok(self.pipe)
    }
}

pub(crate) fn receive(name: &str, parent_process: u32, timeout: Duration) -> io::Result<Channel> {
    let handle = owned(unsafe {
        CreateFileW(
            wide(name).as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            std::ptr::null_mut(),
        )
    })?;
    let mut actual = 0;
    if unsafe { GetNamedPipeServerProcessId(handle.as_raw_handle(), &mut actual) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if actual != parent_process {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native worker pipe served by another process",
        ));
    }
    Ok(Channel { handle, timeout })
}

impl Channel {
    /// An outstanding operation borrows stack state and the caller's buffer.
    /// On timeout, cancel and observe completion before either is released.
    /// SAFETY: operation, its event, and the I/O buffer remain alive and at
    /// fixed addresses until this function observes completion. Windows may
    /// mutate the operation meanwhile; no Rust reference to it may be formed.
    unsafe fn complete(
        &self,
        operation: *mut OVERLAPPED,
        event: windows_sys::Win32::Foundation::HANDLE,
        started: i32,
        error: io::Error,
    ) -> io::Result<u32> {
        if started == 0 && error.raw_os_error() != Some(ERROR_IO_PENDING as i32) {
            return Err(error);
        }
        if started == 0 {
            let waited = unsafe { WaitForSingleObject(event, milliseconds(self.timeout)) };
            if waited != WAIT_OBJECT_0 {
                let reason = if waited == WAIT_TIMEOUT {
                    io::Error::new(io::ErrorKind::TimedOut, "native worker channel timed out")
                } else {
                    io::Error::last_os_error()
                };
                unsafe {
                    CancelIoEx(self.handle.as_raw_handle(), operation);
                    let mut ignored = 0;
                    GetOverlappedResult(self.handle.as_raw_handle(), operation, &mut ignored, 1);
                }
                return Err(reason);
            }
        }
        let mut count = 0;
        if unsafe { GetOverlappedResult(self.handle.as_raw_handle(), operation, &mut count, 0) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(count)
    }
}
impl Read for Channel {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let event = event()?;
        let mut operation = OVERLAPPED {
            hEvent: event.as_raw_handle(),
            ..Default::default()
        };
        let operation = &raw mut operation;
        let started = unsafe {
            ReadFile(
                self.handle.as_raw_handle(),
                output.as_mut_ptr(),
                output.len().min(u32::MAX as usize) as u32,
                std::ptr::null_mut(),
                operation,
            )
        };
        let error = io::Error::last_os_error();
        match unsafe { self.complete(operation, event.as_raw_handle(), started, error) } {
            Err(error) if error.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) => Ok(0),
            result => result.map(|count| count as usize),
        }
    }
}
impl Write for Channel {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if input.is_empty() {
            return Ok(0);
        }
        let event = event()?;
        let mut operation = OVERLAPPED {
            hEvent: event.as_raw_handle(),
            ..Default::default()
        };
        let operation = &raw mut operation;
        let started = unsafe {
            WriteFile(
                self.handle.as_raw_handle(),
                input.as_ptr(),
                input.len().min(u32::MAX as usize) as u32,
                std::ptr::null_mut(),
                operation,
            )
        };
        let error = io::Error::last_os_error();
        unsafe { self.complete(operation, event.as_raw_handle(), started, error) }
            .map(|count| count as usize)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
