//! The transport/provider boundary: one private pipe pair per binding.
//!
//! A `PipeAdapter` is the target-owned transport realization the installer
//! selects for a binding's `transport` identity. Its contract is narrow:
//! every pair is private — exactly four ends exist, all starting in
//! installer custody — and no listener, broker, global lookup, discovery,
//! connection pooling, or general handle inheritance can mint additional
//! ends. The installer decides which instance receives each end; the
//! adapter never sees the graph.
//!
//! `StdPipeAdapter` is the host realization over `std::io::pipe`: anonymous
//! pipe handles on Windows and private pipe descriptors on macOS — the same
//! primitives the Omega package's target-owned adapters wrap. The kernel,
//! not this crate, attests which process produced a delivered frame; that
//! and exact endpoint delivery remain disclosed provider assumptions (see
//! `PipeAdapter::assumptions`), not properties proved here.

use crate::deployment_plan::Identity;
use std::fmt;

/// The four ends of one binding's dedicated request/response pair.
/// `request_*` carries invocations from the importer to the exporter;
/// `response_*` carries the bounded reply back. There is no third channel:
/// synchronous returns stay on the binding's response end.
pub struct PipePair<Endpoint> {
    /// Writes request frames; assigned to the importing instance.
    pub request_write: Endpoint,
    /// Reads request frames; assigned to the exporting instance.
    pub request_read: Endpoint,
    /// Writes the response frame; assigned to the exporting instance.
    pub response_write: Endpoint,
    /// Reads the response frame; assigned to the importing instance.
    pub response_read: Endpoint,
}

/// The transport provider boundary the installer drives.
pub trait PipeAdapter {
    /// One physical pipe end (an anonymous pipe handle or descriptor).
    type Endpoint;
    type Error: fmt::Debug + fmt::Display;

    /// Create one dedicated request/response pipe pair. Every end begins in
    /// installer custody; the adapter must not retain, duplicate, or hand
    /// out another reference to any of them.
    fn create_binding_pair(&mut self) -> Result<PipePair<Self::Endpoint>, Self::Error>;

    /// The stable physical identity of one end, for the route table and the
    /// installed-token echo the activation gate checks. Two distinct live
    /// ends must never report the same token, and the token must identify
    /// the kernel object — not the caller-local descriptor number — so an
    /// end installed in another process still reports the same token.
    fn endpoint_token(&self, endpoint: &Self::Endpoint) -> Result<u64, Self::Error>;

    /// Permanently close one end still in installer custody. `Ok` confirms
    /// the OS released it; `Err` means custody could not be confirmed and
    /// the caller must report the end as leaked rather than claim clean
    /// teardown.
    fn close_endpoint(&mut self, endpoint: Self::Endpoint) -> Result<(), Self::Error>;

    /// Exact provider identity recorded in the installation receipt.
    fn provider(&self) -> Identity;

    /// The OS/loader assumptions this realization relies on. They are
    /// disclosures the receipt names, not guarantees this crate established:
    /// private-pipe creation, exact delivery of each end to its assigned
    /// process only, disabled general handle inheritance, and
    /// kernel-attested sender identity.
    fn assumptions(&self) -> &'static [&'static str];
}

/// One end of a `std::io` anonymous-pipe pair.
pub enum StdPipeEnd {
    Read(std::io::PipeReader),
    Write(std::io::PipeWriter),
}

impl fmt::Debug for StdPipeEnd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(_) => formatter.write_str("StdPipeEnd::Read(..)"),
            Self::Write(_) => formatter.write_str("StdPipeEnd::Write(..)"),
        }
    }
}

/// The host pipe adapter: `std::io::pipe` anonymous pipes. One safe std
/// entrypoint covers both supported hosts — anonymous pipe handles on
/// Windows, private pipe descriptors on macOS — so the reference installer
/// exercises the real OS primitive rather than a stand-in. Passing the ends
/// into spawned components remains the supervisor's provider contract; this
/// adapter only mints and closes private ends and reports their identity.
pub struct StdPipeAdapter;

impl PipeAdapter for StdPipeAdapter {
    type Endpoint = StdPipeEnd;
    type Error = std::io::Error;

    fn create_binding_pair(&mut self) -> Result<PipePair<StdPipeEnd>, std::io::Error> {
        let (request_read, request_write) = std::io::pipe()?;
        let (response_read, response_write) = std::io::pipe()?;
        Ok(PipePair {
            request_write: StdPipeEnd::Write(request_write),
            request_read: StdPipeEnd::Read(request_read),
            response_write: StdPipeEnd::Write(response_write),
            response_read: StdPipeEnd::Read(response_read),
        })
    }

    /// The end's token is its kernel pipe identity: the anonymous pipe's
    /// inode shifted one bit, with the low bit recording the write end.
    /// Both ends of one pipe share the inode, so the direction bit keeps
    /// the pair's ends distinct; the inode is stable across the fork/exec
    /// handoff that installs the end in the member, which is what lets the
    /// activation gate compare a child's own attestation to the recorded
    /// assignment instead of trusting a descriptor number.
    #[cfg(unix)]
    fn endpoint_token(&self, endpoint: &StdPipeEnd) -> Result<u64, std::io::Error> {
        use std::os::fd::AsFd;
        use std::os::unix::fs::MetadataExt;
        let (end, is_write) = match endpoint {
            StdPipeEnd::Read(end) => (end.as_fd(), false),
            StdPipeEnd::Write(end) => (end.as_fd(), true),
        };
        let inode = std::fs::File::from(end.try_clone_to_owned()?)
            .metadata()?
            .ino();
        Ok(inode.wrapping_shl(1) | u64::from(is_write))
    }

    #[cfg(windows)]
    fn endpoint_token(&self, endpoint: &StdPipeEnd) -> Result<u64, std::io::Error> {
        use std::os::windows::io::AsRawHandle;
        Ok(match endpoint {
            StdPipeEnd::Read(end) => end.as_raw_handle() as usize as u64,
            StdPipeEnd::Write(end) => end.as_raw_handle() as usize as u64,
        })
    }

    fn close_endpoint(&mut self, endpoint: StdPipeEnd) -> Result<(), std::io::Error> {
        // Dropping the last reference releases the descriptor/handle; there
        // is no fallible close path for std pipe ends.
        drop(endpoint);
        Ok(())
    }

    fn provider(&self) -> Identity {
        crate::deployment_plan::identity_of(b"topology::std-pipe-adapter")
    }

    fn assumptions(&self) -> &'static [&'static str] {
        &[
            "anonymous pipe creation supplies exactly four private ends",
            "endpoint delivery installs each end only into its assigned process",
            "general handle inheritance is disabled for spawned members",
            "the kernel attests which process produced each delivered frame",
            "an endpoint token is its kernel pipe inode and direction, stable across exec",
        ]
    }
}

/// Unix custody handoff: the physical descriptor a supervisor retains
/// through the member's exec.
#[cfg(unix)]
impl std::os::fd::AsFd for StdPipeEnd {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        match self {
            StdPipeEnd::Read(end) => end.as_fd(),
            StdPipeEnd::Write(end) => end.as_fd(),
        }
    }
}
