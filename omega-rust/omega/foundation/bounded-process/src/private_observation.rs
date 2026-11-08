// Unix private child runtime observation lane (`FD 3`), separate from guest
// standard streams.
//
// `prepare_private_observation` creates an anonymous `UnixStream::pair()`,
// marks ambient parent descriptors close-on-exec, and installs an
// async-signal-safe `pre_exec` hook that duplicates the writer endpoint onto
// descriptor `3` and clears `FD_CLOEXEC` on `3` in the child. A background
// reader thread drains the parent endpoint under an exact `expected_bytes`
// roster, polling with a 100ms wake interval while `WouldBlock` persists so
// that once `completed` is signaled after child exit (or on `Drop`), the reader
// drains any buffered bytes and retires even if an unproved descendant leaked
// descriptor `3`.

use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Output};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct PrivateObservationCapture {
    completed: Arc<AtomicBool>, // Signals child completion to wake the reader loop.
    captured: Option<std::thread::JoinHandle<io::Result<Vec<u8>>>>, // Background reader thread verifying the exact byte roster.
}

impl PrivateObservationCapture {
    pub fn bind_child(&self, _process_id: u32) {}

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
    let (mut reader, writer) = UnixStream::pair()?;
    reader.set_nonblocking(true)?;
    let completed = Arc::new(AtomicBool::new(false));
    let reader_completed = Arc::clone(&completed);
    let writer_descriptor = writer.as_raw_fd();
    super::process_child::descriptors::mark_ambient_close_on_exec(command)?;
    unsafe {
        command.pre_exec(move || {
            let _keep_alive = &writer;
            if libc::dup2(writer_descriptor, 3) == -1 || libc::fcntl(3, libc::F_SETFD, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let captured = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    if bytes
                        .len()
                        .checked_add(count)
                        .is_none_or(|size| size > expected_bytes)
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "private runtime observation contains extra bytes",
                        ));
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if reader_completed.load(Ordering::Acquire) {
                        break;
                    }
                    let mut descriptor = libc::pollfd {
                        fd: reader.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    let result = unsafe { libc::poll(&mut descriptor, 1, 100) };
                    if result < 0 {
                        let error = io::Error::last_os_error();
                        if error.kind() != io::ErrorKind::Interrupted {
                            return Err(error);
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        if bytes.len() != expected_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "private runtime observation differs from its exact byte roster",
            ));
        }
        Ok(bytes)
    });
    Ok(PrivateObservationCapture {
        completed,
        captured: Some(captured),
    })
}

pub fn capture_private_observation(
    mut command: Command,
    expected_bytes: usize,
) -> io::Result<(Output, Vec<u8>)> {
    let capture = prepare_private_observation(&mut command, expected_bytes)?;
    let output = command.output();
    drop(command);
    let observation = capture.finish();
    Ok((output?, observation?))
}

#[cfg(test)]
mod tests {
    use super::{capture_private_observation, prepare_private_observation};
    use std::process::Command;

    #[test]
    fn prepared_lane_uses_real_child_bytes_and_retires_on_every_completion() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf abc >&3"]);
        let capture = prepare_private_observation(&mut command, 3).expect("prepare private lane");
        assert!(command.status().expect("run finite child").success());
        assert_eq!(capture.finish().expect("capture exact bytes"), b"abc");
        for script in ["printf ab >&3", "printf abcd >&3"] {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            assert!(capture_private_observation(command, 3).is_err());
        }
        let error =
            capture_private_observation(Command::new("/nonexistent/omega-observation-child"), 3)
                .expect_err("spawn failure must retire its private lane");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
}
