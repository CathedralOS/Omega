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
    // Closed standard streams can make socketpair allocate FD0/1/2. Keep
    // both owned endpoints above that namespace before Command sets stdio.
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

/// Duplicate the explicitly inherited endpoint into a unique Rust owner.
/// Even malformed launcher input cannot make a second owner of a live Rust FD.
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
    // Native providers must not leak the control endpoint into an exec child.
    if unsafe { libc::fcntl(raw, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
        return Err(io::Error::last_os_error());
    }
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    Ok(stream)
}
