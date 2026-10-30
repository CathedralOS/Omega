//! A private Windows observation pipe authenticated to the actual child process.
use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    process::{Command, Output},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING,
        INVALID_HANDLE_VALUE,
    },
    Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND, ReadFile},
    System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_NOWAIT,
        PIPE_REJECT_REMOTE_CLIENTS,
    },
};

pub struct PrivateObservationCapture {
    completed: Arc<AtomicBool>,
    child: Arc<AtomicU32>,
    captured: Option<std::thread::JoinHandle<io::Result<Vec<u8>>>>,
}
impl PrivateObservationCapture {
    pub fn bind_child(&self, process_id: u32) {
        self.child.store(process_id, Ordering::Release);
    }
    pub fn finish(mut self) -> io::Result<Vec<u8>> {
        self.completed.store(true, Ordering::Release);
        self.captured
            .take()
            .ok_or_else(|| io::Error::other("private observation already retired"))?
            .join()
            .map_err(|_| io::Error::other("private observation reader did not complete"))?
    }
}
impl Drop for PrivateObservationCapture {
    fn drop(&mut self) {
        self.completed.store(true, Ordering::Release);
    }
}

pub fn prepare_private_observation(
    command: &mut Command,
    expected_bytes: usize,
) -> io::Result<PrivateObservationCapture> {
    if expected_bytes == 0 || expected_bytes > (65536 + 84) * 5 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private observation roster exceeds its bounded extent",
        ));
    }
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let ordinal = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| io::Error::other("private observation identities exhausted"))?;
    let instant = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let name = format!(
        r"\\.\pipe\omega-observation-{}-{instant}-{ordinal}",
        std::process::id()
    );
    let wide = name.encode_utf16().chain([0]).collect::<Vec<_>>();
    // The first-instance flag prevents a pre-existing endpoint from answering this lane.
    let raw = unsafe {
        CreateNamedPipeW(
            wide.as_ptr(),
            PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            0,
            65536,
            0,
            std::ptr::null(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let pipe = unsafe { OwnedHandle::from_raw_handle(raw) };
    command.env("OMEGA_PRIVATE_OBSERVATION_PIPE", name);
    let completed = Arc::new(AtomicBool::new(false));
    let child = Arc::new(AtomicU32::new(0));
    let reader_completed = Arc::clone(&completed);
    let expected_child = Arc::clone(&child);
    let captured = std::thread::spawn(move || {
        let mut connected = false;
        let mut authenticated = false;
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            if !connected {
                let result =
                    unsafe { ConnectNamedPipe(pipe.as_raw_handle(), std::ptr::null_mut()) };
                let error = io::Error::last_os_error();
                // In NOWAIT mode a nonzero result only makes an instance available.
                connected =
                    result == 0 && error.raw_os_error() == Some(ERROR_PIPE_CONNECTED as i32);
                if result == 0
                    && !connected
                    && !matches!(error.raw_os_error(), Some(code) if code == ERROR_PIPE_LISTENING as i32 || code == ERROR_NO_DATA as i32)
                {
                    return Err(error);
                }
            }
            if connected && !authenticated && expected_child.load(Ordering::Acquire) != 0 {
                let mut actual = 0;
                if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle(), &mut actual) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                if actual != expected_child.load(Ordering::Acquire) {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "private observation connected by another process",
                    ));
                }
                authenticated = true;
            }
            if authenticated {
                let mut count = 0;
                let result = unsafe {
                    ReadFile(
                        pipe.as_raw_handle(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        &mut count,
                        std::ptr::null_mut(),
                    )
                };
                if result != 0 && count != 0 {
                    if bytes
                        .len()
                        .checked_add(count as usize)
                        .is_none_or(|length| length > expected_bytes)
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "private runtime observation contains extra bytes",
                        ));
                    }
                    bytes.extend_from_slice(&buffer[..count as usize]);
                    continue;
                }
                if result == 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) {
                        break;
                    }
                    if error.raw_os_error() != Some(ERROR_NO_DATA as i32) {
                        return Err(error);
                    }
                }
            }
            if reader_completed.load(Ordering::Acquire) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !authenticated || bytes.len() != expected_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "private runtime observation differs from its exact byte roster",
            ));
        }
        Ok(bytes)
    });
    Ok(PrivateObservationCapture {
        completed,
        child,
        captured: Some(captured),
    })
}

pub fn capture_private_observation(
    mut command: Command,
    expected_bytes: usize,
) -> io::Result<(Output, Vec<u8>)> {
    let capture = prepare_private_observation(&mut command, expected_bytes)?;
    let started = std::time::Instant::now();
    let limits = crate::BoundedCaptureLimits::new(
        4 * 1024 * 1024,
        4 * 1024 * 1024,
        Duration::from_secs(60),
        Duration::from_secs(5),
        Duration::from_millis(10),
    );
    let mut prepared = crate::BoundedProcessPrepared::new(
        command,
        crate::BoundedProcessLimits::new(
            60,
            1024 * 1024 * 1024,
            8 * 1024 * 1024,
            64,
            1,
            1024 * 1024 * 1024,
            1024 * 1024 * 1024,
        ),
        "installed entry observation",
    )?;
    prepared.preserve_configured_stdin();
    prepared.stdout_piped().stderr_piped();
    let output = crate::BoundedProcessChild::spawn(prepared).and_then(|mut child| {
        capture.bind_child(child.id());
        crate::bounded_process::capture_child(
            child,
            crate::BoundedProcessInput::Null,
            limits,
            crate::BoundedCaptureBudget::new(8 * 1024 * 1024),
            started,
        )
        .map_err(io::Error::other)
        .and_then(|output| {
            use std::os::windows::process::ExitStatusExt;
            let code = output
                .status
                .code()
                .ok_or_else(|| io::Error::other("Windows child has no exit status"))?;
            Ok(Output {
                status: std::process::ExitStatus::from_raw(code as u32),
                stdout: output.stdout,
                stderr: output.stderr,
            })
        })
    });
    let observation = capture.finish();
    Ok((output?, observation?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::{
        Foundation::GENERIC_WRITE,
        Storage::FileSystem::{CreateFileW, FlushFileBuffers, OPEN_EXISTING, WriteFile},
    };

    #[test]
    fn writer() {
        let Ok(contents) = std::env::var("OMEGA_OBSERVATION_TEST_CHILD") else {
            return;
        };
        if contents == "without-observer" {
            return;
        }
        if contents == "flood" {
            use std::io::Write;
            let _ = std::io::stdout()
                .lock()
                .write_all(&vec![b'x'; 5 * 1024 * 1024]);
            return;
        }
        let name = std::env::var("OMEGA_PRIVATE_OBSERVATION_PIPE")
            .unwrap()
            .encode_utf16()
            .chain([0])
            .collect::<Vec<_>>();
        let raw = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_WRITE,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(raw, INVALID_HANDLE_VALUE);
        let pipe = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut count = 0;
        assert_ne!(
            unsafe {
                WriteFile(
                    pipe.as_raw_handle(),
                    contents.as_ptr(),
                    contents.len() as u32,
                    &mut count,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        assert_eq!(count as usize, contents.len());
        assert_ne!(unsafe { FlushFileBuffers(pipe.as_raw_handle()) }, 0);
    }
    #[test]
    fn private_lane_checks_real_child_bytes_and_retires_after_failed_spawn() {
        for contents in ["abc", "ab", "abcd"] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "private_observation::tests::writer",
                    "--nocapture",
                ])
                .env("OMEGA_OBSERVATION_TEST_CHILD", contents);
            let result = capture_private_observation(command, 3);
            if contents == "abc" {
                let (output, bytes) = result.unwrap();
                assert!(output.status.success());
                assert_eq!(bytes, b"abc");
            } else {
                assert!(result.is_err());
            }
        }
        let mut absent = Command::new(std::env::current_exe().unwrap());
        absent
            .args([
                "--exact",
                "private_observation::tests::writer",
                "--nocapture",
            ])
            .env("OMEGA_OBSERVATION_TEST_CHILD", "without-observer");
        let error = capture_private_observation(absent, 3).unwrap_err();
        assert!(error.to_string().contains("exact byte roster"), "{error}");
        let mut flood = Command::new(std::env::current_exe().unwrap());
        flood
            .args([
                "--exact",
                "private_observation::tests::writer",
                "--nocapture",
            ])
            .env("OMEGA_OBSERVATION_TEST_CHILD", "flood");
        let error = capture_private_observation(flood, 3).unwrap_err();
        assert!(error.to_string().contains("OutputOverflow"), "{error}");
        assert_eq!(
            capture_private_observation(
                Command::new("Z:/nonexistent/omega-observation-child.exe"),
                3
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::NotFound
        );
    }
}
