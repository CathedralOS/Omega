//! Bind the reviewed program image before a Build runs. A launch receives
//! only copied inputs and returns observations for its requested output paths.
use bounded_process::PrivateFilesystemFiles;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
    sync::Arc,
    time::Duration,
};

#[cfg(target_os = "linux")]
mod linux;

const PRIVATE_BYTES: usize = 64 << 20;
const WORKER: &str = "--omega-private-program-worker";

#[derive(Clone, Debug)]
pub struct ReviewedProgram(Arc<[u8]>);

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    Exited(u32),
    StartFailed,
    AbnormalExit,
    TimedOut,
}

pub struct RunOutput {
    pub status: RunStatus,
    pub outputs: Vec<Option<Vec<u8>>>,
}

impl ReviewedProgram {
    /// Read and own the executable bytes now. Replacing the original pathname
    /// after review cannot change the image used by this activation.
    pub fn capture(path: &Path) -> io::Result<Self> {
        let mut bytes = Vec::new();
        let mut open = File::options();
        open.read(true);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            open.custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY);
        }
        let file = open.open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::other(
                "program grant requires a regular executable",
            ));
        }
        file.take(PRIVATE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > PRIVATE_BYTES {
            return Err(io::Error::other(
                "program image exceeds its private launch budget",
            ));
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            // Only a file already installed as an executable may be reviewed.
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(io::Error::other(
                    "program grant requires an executable file",
                ));
            }
            linux::require_static_image(&bytes)?;
            Ok(Self(bytes.into()))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(io::Error::other("this host cannot confine a Build program"))
        }
    }

    pub fn run(
        &self,
        worker: &Path,
        arguments: &[Vec<u8>],
        files: PrivateFilesystemFiles,
        timeout: Duration,
    ) -> io::Result<RunOutput> {
        let mut strings = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let Ok(text) = std::str::from_utf8(argument) else {
                return Ok(no_start());
            };
            if text.contains('\0') {
                return Ok(no_start());
            }
            strings.push(text);
        }
        #[cfg(target_os = "linux")]
        {
            linux::run(&self.0, worker, &strings, files, timeout, false)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (worker, files, timeout, strings, &self.0);
            Err(io::Error::other("this host cannot confine a Build program"))
        }
    }
}

/// Exercise the actual filesystem, timeout supervisor, private PID namespace,
/// syscall filter and observation path before starting a restricted Build.
/// The probe exits inside trusted setup and executes no approved program.
pub fn require_host(worker: &Path) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let files = PrivateFilesystemFiles {
            initial: [(b"in".to_vec(), Arc::from([]))].into(),
            outputs: vec![b"out".to_vec()],
        };
        let result = linux::run(&[], worker, &[], files, Duration::from_secs(3), true)?;
        if result.status != RunStatus::Exited(0) || result.outputs != [None] {
            return Err(io::Error::other("host program confinement probe failed"));
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = worker;
        Err(io::Error::other("this host cannot confine a Build program"))
    }
}

fn no_start() -> RunOutput {
    RunOutput {
        status: RunStatus::StartFailed,
        outputs: Vec::new(),
    }
}

/// The stock CLI calls this before creating compiler worker threads. An
/// embedding host can designate a worker executable with the same entry.
pub fn run_worker_if_requested() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(WORKER)) {
        return;
    }
    #[cfg(target_os = "linux")]
    {
        let dir = args.next();
        if let Some(dir) = dir
            && args.next().is_none()
        {
            match linux::supervise(Path::new(&dir)) {
                Ok(()) => std::process::exit(0),
                Err(reason) => eprintln!("Build program confinement: {reason}"),
            }
        }
    }
    std::process::exit(1);
}
