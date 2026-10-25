//! A private child's runtime observation lane, separate from guest streams.
//!
//! FD 3 names this fresh socket only in the child. The parent retains the other
//! endpoint, drains concurrently under a bounded byte count, and closes its
//! write endpoint after child completion. Unexpected bytes, a missing observer,
//! or an escaped writer cannot produce an accepted exact observation roster.

use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Output};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Prepared private lane, retired after the caller's own bounded child loop.
/// Dropping it also wakes the reader; it never extends a child's deadline.
pub struct PrivateObservationCapture {
    completed: Arc<AtomicBool>,
    captured: Option<std::thread::JoinHandle<io::Result<Vec<u8>>>>,
}

impl PrivateObservationCapture {
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
    // The closure performs only async-signal-safe descriptor operations after
    // fork. Captured ownership keeps the source descriptor valid through exec.
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
                    // Once the actual child has completed, all of its successful
                    // writes are already in this socket. Drain them, then retire
                    // the reader even if an unproved descendant leaked a writer.
                    if reader_completed.load(Ordering::Acquire) {
                        break;
                    }
                    let mut descriptor = libc::pollfd {
                        fd: reader.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    // Poll only borrows this owned descriptor and stack record.
                    // The short wake interval observes actual process completion;
                    // it imposes no independent body execution timeout.
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
    // Retire the lane even after a failed spawn, preserving the actual error.
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
