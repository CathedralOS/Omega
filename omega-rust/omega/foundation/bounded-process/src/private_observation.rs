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

pub fn capture_private_observation(
    mut command: Command,
    expected_bytes: usize,
) -> io::Result<(Output, Vec<u8>)> {
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
    super::process_child::descriptors::mark_ambient_close_on_exec(&mut command)?;
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
    let output = command.output();
    drop(command);
    completed.store(true, Ordering::Release);
    let observation = captured
        .join()
        .map_err(|_| io::Error::other("private observation reader did not complete"));
    // Keep the actual spawn/execution error ahead of a consequent absent frame.
    // The reader still retires and joins on every path.
    let output = output?;
    let bytes = observation??;
    Ok((output, bytes))
}
