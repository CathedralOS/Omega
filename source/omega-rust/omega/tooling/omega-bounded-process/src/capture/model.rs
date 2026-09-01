use crate::BoundedProcessExitStatus;
use std::fs::File;
use std::time::Duration;

#[derive(Debug)]
pub enum BoundedProcessInput {
    Null,
    Bytes(Vec<u8>),
    File(File),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedCaptureLimits {
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub timeout: Duration,
    pub cleanup_timeout: Duration,
    pub poll_interval: Duration,
}

impl BoundedCaptureLimits {
    pub const fn new(
        stdout_bytes: usize,
        stderr_bytes: usize,
        timeout: Duration,
        cleanup_timeout: Duration,
        poll_interval: Duration,
    ) -> Self {
        Self {
            stdout_bytes,
            stderr_bytes,
            timeout,
            cleanup_timeout,
            poll_interval,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedProcessOutput {
    pub status: BoundedProcessExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedProcessStream {
    Stdout,
    Stderr,
}

impl BoundedProcessStream {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedProcessRunError {
    InvalidLimits,
    Spawn(String),
    WorkerSpawn {
        worker: &'static str,
        message: String,
    },
    StreamCapture {
        stream: BoundedProcessStream,
        message: String,
    },
    InputTransfer(String),
    OutputOverflow {
        stream: BoundedProcessStream,
        limit: usize,
    },
    AggregateOutputOverflow {
        ceiling: u64,
        attempted: u64,
    },
    TimedOut {
        timeout: Duration,
    },
    Wait(String),
    Cleanup(String),
    Finalize(String),
    WorkersEndedEarly,
}

impl std::fmt::Display for BoundedProcessRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "bounded process execution failed: {self:?}")
    }
}

impl std::error::Error for BoundedProcessRunError {}
