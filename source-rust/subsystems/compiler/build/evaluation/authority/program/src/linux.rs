use super::{PRIVATE_BYTES, RunOutput, RunStatus, WORKER};
use bounded_process::{
    BoundedCaptureBudget, BoundedCaptureLimits, BoundedProcessChild, BoundedProcessInput,
    BoundedProcessLimits, BoundedProcessPrepared, BoundedProcessRunError, PrivateFilesystemFiles,
    PrivateFilesystemOutputs, run_bounded_process,
};
use platform_custody::temporary_directory::TemporaryDirectory;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    os::fd::{AsRawFd, BorrowedFd},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::Path,
    process::Command,
    sync::Arc,
    time::Duration,
};

#[derive(Deserialize, Serialize)]
struct Request {
    arguments: Vec<String>,
    inputs: Vec<Vec<u8>>,
    outputs: Vec<Vec<u8>>,
    observation_socket: i32,
    admission_probe: bool,
}

/// The first Linux backend can seal a self-contained native image. Refuse an
/// image demanding an external loader until its runtime files can be bound too.
pub(super) fn require_static_image(image: &[u8]) -> io::Result<()> {
    let bad = || {
        io::Error::other(
            "program grant requires a static native ELF image with its runtime included",
        )
    };
    let word = |at| {
        image
            .get(at..at + 2)
            .map(|bytes| u16::from_le_bytes(bytes.try_into().unwrap()))
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => 62,
        "aarch64" => 183,
        _ => return Err(bad()),
    };
    if !image.starts_with(b"\x7fELF\x02\x01\x01")
        || !matches!(word(16), Some(2 | 3))
        || word(18) != Some(arch)
        || word(54) != Some(56)
    {
        return Err(bad());
    }
    let offset = usize::try_from(u64::from_le_bytes(
        image.get(32..40).ok_or_else(bad)?.try_into().unwrap(),
    ))
    .map_err(|_| bad())?;
    let count = usize::from(word(56).ok_or_else(bad)?);
    let end = offset.checked_add(count * 56).ok_or_else(bad)?;
    let headers = image.get(offset..end).ok_or_else(bad)?;
    let mut executable = false;
    for header in headers.as_chunks::<56>().0 {
        let kind = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let flags = u32::from_le_bytes(header[4..8].try_into().unwrap());
        if kind == 3 {
            return Err(bad());
        }
        executable |= kind == 1 && flags & 1 != 0;
    }
    if !executable {
        return Err(bad());
    }
    Ok(())
}

fn limits() -> BoundedProcessLimits {
    BoundedProcessLimits::new(
        300,
        1 << 30,
        PRIVATE_BYTES as u64,
        64,
        128,
        1 << 30,
        1 << 30,
    )
}

fn io_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

pub(super) fn run(
    image: &[u8],
    worker: &Path,
    arguments: &[&str],
    files: PrivateFilesystemFiles,
    timeout: Duration,
    admission_probe: bool,
) -> io::Result<RunOutput> {
    if timeout.is_zero() {
        return Err(io::Error::other("program timeout must be positive"));
    }
    let scratch = TemporaryDirectory::create("build-program", false)?;
    let dir = scratch.path();
    fs::write(dir.join("image"), image)?;
    fs::set_permissions(dir.join("image"), fs::Permissions::from_mode(0o500))?;
    let (observation, sender) = PrivateFilesystemOutputs::channel(files.outputs.clone())?;
    let request = Request {
        observation_socket: sender.as_raw_fd(),
        admission_probe,
        arguments: arguments.iter().map(|text| (*text).to_owned()).collect(),
        inputs: files.initial.keys().cloned().collect(),
        outputs: files.outputs,
    };
    let mut bytes_left = PRIVATE_BYTES;
    for (index, bytes) in files.initial.values().enumerate() {
        bytes_left = bytes_left
            .checked_sub(bytes.len())
            .ok_or_else(|| io::Error::other("program inputs exceed their capture budget"))?;
        fs::write(dir.join(format!("in-{index}")), bytes)?;
    }
    let descriptor = fs::File::create(dir.join("request"))?;
    serde_json::to_writer(descriptor, &request)?;
    // The host trust base supplies util-linux. After this succeeds the
    // trusted worker is init of the private PID namespace and owns its tree.
    let mut command = Command::new("/usr/bin/unshare");
    command
        .env_clear()
        .args([
            "--user",
            "--map-root-user",
            "--pid",
            "--fork",
            "--kill-child=KILL",
            "--net",
            "--ipc",
            "--uts",
            "--mount",
            "--",
        ])
        .arg(worker)
        .arg(WORKER)
        .arg(dir);
    let sponsor = std::process::id();
    // SAFETY: these process-local syscalls use no allocation between fork
    // and exec. Check for a parent that exited before PDEATHSIG was armed.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() as u32 != sponsor {
                return Err(io::ErrorKind::Interrupted.into());
            }
            Ok(())
        });
    }
    let mut prepared = BoundedProcessPrepared::new(command, limits(), "Build program supervisor")?;
    prepared.retain_descriptor(sender)?;
    let captured = run_bounded_process(
        prepared,
        BoundedProcessInput::Null,
        BoundedCaptureLimits::new(
            0,
            4096,
            timeout,
            Duration::from_millis(100),
            Duration::from_millis(2),
        ),
        BoundedCaptureBudget::new(4096),
    );
    let result = match captured {
        Err(BoundedProcessRunError::TimedOut { .. }) => {
            return Ok(RunOutput {
                status: RunStatus::TimedOut,
                outputs: Vec::new(),
            });
        }
        other => other.map_err(io_error)?,
    };
    if !result.status.success() {
        return Err(io::Error::other(format!(
            "host cannot confine the Build program: {}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    let status = serde_json::from_reader(fs::File::open(dir.join("status"))?)?;
    // Linux destroys every process in the namespace when its init exits.
    // The only remaining reference to the private mount is the observation.
    let outputs = if status == RunStatus::StartFailed {
        Vec::new()
    } else {
        observation.collect(PRIVATE_BYTES)?
    };
    Ok(RunOutput { status, outputs })
}

pub(super) fn supervise(dir: &Path) -> io::Result<()> {
    // Only private init can reap the namespace's orphaned descendants.
    // Refuse direct invocation before touching the caller's files.
    if std::process::id() != 1 {
        return Err(io::Error::other(
            "program worker requires its own PID namespace",
        ));
    }
    // A guest must never gain access to the trusted supervisor's handles.
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let request: Request = serde_json::from_reader(fs::File::open(dir.join("request"))?)?;
    let mut initial = BTreeMap::new();
    for (i, name) in request.inputs.into_iter().enumerate() {
        initial.insert(name, Arc::from(fs::read(dir.join(format!("in-{i}")))?));
    }
    let files = PrivateFilesystemFiles {
        initial,
        outputs: request.outputs,
    };
    let mut command = Command::new(dir.join("image"));
    command.args(&request.arguments).env_clear();
    let mut child = BoundedProcessPrepared::new(command, limits(), "bound Build program")?;
    let purpose = if request.admission_probe {
        bounded_process::PrivateFilesystemPurpose::ProgramAdmissionProbe
    } else {
        bounded_process::PrivateFilesystemPurpose::ReviewedProgram
    };
    let observations = child.use_private_filesystem(dir, files, purpose)?;
    child
        .stdin_null()
        .stdout_null()
        .stderr_null()
        .observe_resources();
    let outcome = match BoundedProcessChild::spawn(child) {
        Ok(mut running) => {
            while running.try_wait()?.is_none() {
                std::thread::sleep(Duration::from_millis(2));
            }
            running.terminate()?;
            let completed = running.finish()?;
            completed
                .status()
                .code()
                .map_or(RunStatus::AbnormalExit, |code| {
                    RunStatus::Exited(code as u32)
                })
        }
        Err(error) if matches!(error.raw_os_error(), Some(libc::ENOEXEC | libc::ENOENT)) => {
            RunStatus::StartFailed
        }
        Err(error) => return Err(error),
    };
    if outcome != RunStatus::StartFailed {
        // This descriptor entered through the sponsor's retained descriptors.
        // No guest can obtain it. Do not let a manual private-worker invocation
        // synthesize a BorrowedFd from a closed descriptor.
        if request.observation_socket < 3
            || unsafe { libc::fcntl(request.observation_socket, libc::F_GETFD) } < 0
        {
            return Err(io::Error::other("missing program observation socket"));
        }
        let socket = unsafe { BorrowedFd::borrow_raw(request.observation_socket) };
        observations.forward_root(&socket)?;
    }
    serde_json::to_writer(fs::File::create(dir.join("status"))?, &outcome)?;
    Ok(())
}
