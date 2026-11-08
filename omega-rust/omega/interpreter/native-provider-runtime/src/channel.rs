/*
We establish a private duplex control channel between the host interpreter and
the isolated native provider worker, independent of C standard streams.

On Unix, closed standard streams can cause `socketpair` to allocate file
descriptors `0`, `1`, or `2`, so `prepare` and `receive` duplicate both owned
endpoints above descriptor `3` with `F_DUPFD_CLOEXEC` and verify `SOCK_STREAM`
before `Command` configures standard streams.

On Windows, we create a local overlapped named pipe without enabling additional
inheritable handles so the bounded child keeps its standard-stream setup and
suspended-launch / Job Object lifecycle. The pipe name supplies uniqueness
rather than authentication; both peers verify the OS-reported process identity
(`GetNamedPipeClientProcessId` / `GetNamedPipeServerProcessId`) before admitting
any adapter or request.
*/

#[cfg(unix)]
mod unix {
    use std::io;
    use std::os::fd::{FromRawFd, OwnedFd, RawFd};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    pub(crate) struct PreparedChannel {
        pub(crate) parent: UnixStream,
        pub(crate) child: OwnedFd,
    }

    pub(crate) fn prepare(timeout: Duration) -> io::Result<PreparedChannel> {
        let (original_parent, original_child) = UnixStream::pair()?;
        fn private_copy(stream: &UnixStream) -> io::Result<UnixStream> {
            use std::os::fd::AsRawFd;
            let raw = unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
            if raw == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(unsafe { UnixStream::from_raw_fd(raw) })
        }
        let parent = private_copy(&original_parent)?;
        let child = private_copy(&original_child)?;
        drop((original_parent, original_child));
        parent.set_read_timeout(Some(timeout))?;
        parent.set_write_timeout(Some(timeout))?;
        Ok(PreparedChannel {
            parent,
            child: child.into(),
        })
    }

    pub(crate) fn receive(raw: RawFd, timeout: Duration) -> io::Result<UnixStream> {
        if raw < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "worker channel is not a private descriptor",
            ));
        }
        let duplicate = unsafe { libc::fcntl(raw, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicate == -1 {
            return Err(io::Error::last_os_error());
        }
        let stream = unsafe { UnixStream::from_raw_fd(duplicate) };
        let mut kind = 0;
        let mut size = std::mem::size_of_val(&kind) as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                duplicate,
                libc::SOL_SOCKET,
                libc::SO_TYPE,
                (&mut kind as *mut i32).cast(),
                &mut size,
            )
        } == -1
        {
            return Err(io::Error::last_os_error());
        }
        if kind != libc::SOCK_STREAM {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "worker endpoint is not a stream socket",
            ));
        }
        if unsafe { libc::fcntl(raw, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            return Err(io::Error::last_os_error());
        }
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(stream)
    }
}

#[cfg(windows)]
mod windows {
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

    pub(crate) fn receive(
        name: &str,
        parent_process: u32,
        timeout: Duration,
    ) -> io::Result<Channel> {
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
        // SAFETY: `operation`, its `event`, and the caller's I/O buffer remain
        // alive and at fixed addresses until this function observes completion.
        // Windows may mutate `operation` while pending; no Rust reference to it
        // may be formed until `GetOverlappedResult` completes after `CancelIoEx`.
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
                        GetOverlappedResult(
                            self.handle.as_raw_handle(),
                            operation,
                            &mut ignored,
                            1,
                        );
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
}

#[cfg(unix)]
pub(crate) use unix::*;
#[cfg(windows)]
pub(crate) use windows::*;
