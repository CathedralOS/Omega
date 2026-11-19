// Structured command preparation (`BoundedProcessPrepared`,
// `BoundedProcessLimits`).
//
// Callers cannot extract or replace the underlying `std::process::Command`.
// Before spawning, `into_command` verifies that:
// - all three standard streams (`stdin`, `stdout`, `stderr`) were explicitly
//   set to `Null` or `Piped` (or preserved by `preserve_configured_stdin` for
//   Windows private observation);
// - argument count (`<= 4,096`), environment entry count (`<= 1,024`), and
//   aggregate configuration bytes (`<= 4 MiB`) stay within fixed ceilings;
// - any Unix private descriptors passed to `retain_descriptor` are `>= 3` and
//   kept alive inside a `pre_exec` closure across fork/exec while all other
//   ambient descriptors above stderr are marked `CLOEXEC`.

use crate::process_child::limits;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

const MAXIMUM_COMMAND_ARGUMENTS: usize = 4 * 1024;
const MAXIMUM_COMMAND_ENVIRONMENT_ENTRIES: usize = 1024;
const MAXIMUM_COMMAND_CONFIGURATION_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub struct BoundedProcessPrepared {
    command: Command,
    limits: BoundedProcessLimits,
    boundary: &'static str,
    observe_resources: bool,
    stdin: Option<BoundedStandardStreamDisposition>,
    stdout: Option<BoundedStandardStreamDisposition>,
    stderr: Option<BoundedStandardStreamDisposition>,
    #[cfg(unix)]
    retained_descriptors: Vec<std::os::fd::RawFd>, // Parent descriptors (>= 3) kept open across exec.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundedStandardStreamDisposition {
    Null,
    Piped,
    #[cfg(windows)]
    CallerConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedProcessLimits {
    pub cpu_seconds: u64,
    pub address_space_bytes: u64,
    pub file_size_bytes: u64,
    pub open_files: u64,
    pub active_processes: u64,
    pub process_memory_bytes: u64,
    pub aggregate_memory_bytes: u64,
}

impl BoundedProcessLimits {
    pub const fn new(
        cpu_seconds: u64,
        address_space_bytes: u64,
        file_size_bytes: u64,
        open_files: u64,
        active_processes: u64,
        process_memory_bytes: u64,
        aggregate_memory_bytes: u64,
    ) -> Self {
        Self {
            cpu_seconds,
            address_space_bytes,
            file_size_bytes,
            open_files,
            active_processes,
            process_memory_bytes,
            aggregate_memory_bytes,
        }
    }
}

macro_rules! standard_stream_disposition {
    ($stream:ident, $null:ident, $piped:ident) => {
        pub fn $null(&mut self) -> &mut Self {
            self.command.$stream(Stdio::null());
            self.$stream = Some(BoundedStandardStreamDisposition::Null);
            self
        }

        pub fn $piped(&mut self) -> &mut Self {
            self.command.$stream(Stdio::piped());
            self.$stream = Some(BoundedStandardStreamDisposition::Piped);
            self
        }
    };
}

impl BoundedProcessPrepared {
    pub fn new(
        mut command: Command,
        limits: BoundedProcessLimits,
        boundary: &'static str,
    ) -> io::Result<Self> {
        validate_limits(limits)?;
        limits::configure_child_resource_limits(&mut command, limits)?;
        Ok(Self {
            command,
            limits,
            boundary,
            observe_resources: false,
            stdin: None,
            stdout: None,
            stderr: None,
            #[cfg(unix)]
            retained_descriptors: Vec::new(),
        })
    }

    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.command.env(key, value);
        self
    }

    pub fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self {
        self.command.arg(argument);
        self
    }

    pub fn args<I, S>(&mut self, arguments: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command.args(arguments);
        self
    }

    standard_stream_disposition!(stdin, stdin_null, stdin_piped);
    standard_stream_disposition!(stdout, stdout_null, stdout_piped);
    standard_stream_disposition!(stderr, stderr_null, stderr_piped);
    #[cfg(unix)]
    pub(crate) fn retained_descriptors(&self) -> &[std::os::fd::RawFd] {
        &self.retained_descriptors
    }

    #[cfg(unix)]
    pub fn retain_descriptor(
        &mut self,
        descriptor: std::os::fd::OwnedFd,
    ) -> io::Result<std::os::fd::RawFd> {
        use std::os::fd::AsRawFd;
        use std::os::unix::process::CommandExt;
        let raw = descriptor.as_raw_fd();
        if raw < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained private descriptors must be separate from standard streams",
            ));
        }
        self.retained_descriptors.push(raw);
        // SAFETY: the pre_exec closure only borrows `descriptor` to keep its
        // OwnedFd alive inside Command until fork/exec completes.
        unsafe {
            self.command.pre_exec(move || {
                let _keep_alive = &descriptor;
                Ok(())
            });
        }
        Ok(raw)
    }

    #[cfg(windows)]
    pub(crate) fn preserve_configured_stdin(&mut self) {
        self.stdin = Some(BoundedStandardStreamDisposition::CallerConfigured);
    }

    pub fn get_program(&self) -> &OsStr {
        self.command.get_program()
    }

    pub fn get_args(&self) -> impl Iterator<Item = &OsStr> {
        self.command.get_args()
    }

    pub fn get_envs(&self) -> impl Iterator<Item = (&OsStr, Option<&OsStr>)> {
        self.command.get_envs()
    }

    pub fn get_current_dir(&self) -> Option<&Path> {
        self.command.get_current_dir()
    }

    pub const fn limits(&self) -> BoundedProcessLimits {
        self.limits
    }

    pub fn observe_resources(&mut self) -> &mut Self {
        self.observe_resources = true;
        self
    }

    pub(crate) const fn should_observe_resources(&self) -> bool {
        self.observe_resources
    }

    pub(crate) fn into_command(self) -> io::Result<(Command, BoundedProcessLimits)> {
        self.validate()?;
        Ok((self.command, self.limits))
    }

    fn validate(&self) -> io::Result<()> {
        if !matches!(
            (self.stdin, self.stdout, self.stderr),
            (Some(_), Some(_), Some(_))
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} standard streams must each be explicitly null or piped",
                    self.boundary
                ),
            ));
        }
        if self.command.get_args().count() > MAXIMUM_COMMAND_ARGUMENTS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} command exceeds its argument-count ceiling",
                    self.boundary
                ),
            ));
        }
        if self.command.get_envs().count() > MAXIMUM_COMMAND_ENVIRONMENT_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} command exceeds its environment-entry ceiling",
                    self.boundary
                ),
            ));
        }

        let mut bytes = charge_bytes(
            0,
            os_str_bytes(self.command.get_program(), self.boundary)?,
            self.boundary,
        )?;
        for argument in self.command.get_args() {
            bytes = charge_bytes(bytes, os_str_bytes(argument, self.boundary)?, self.boundary)?;
        }
        for (name, value) in self.command.get_envs() {
            bytes = charge_bytes(bytes, os_str_bytes(name, self.boundary)?, self.boundary)?;
            if let Some(value) = value {
                bytes = charge_bytes(bytes, os_str_bytes(value, self.boundary)?, self.boundary)?;
            }
        }
        if let Some(directory) = self.command.get_current_dir() {
            let _ = charge_bytes(
                bytes,
                os_str_bytes(directory.as_os_str(), self.boundary)?,
                self.boundary,
            )?;
        }
        Ok(())
    }
}

fn charge_bytes(observed: usize, additional: usize, boundary: &str) -> io::Result<usize> {
    let observed = observed.checked_add(additional).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{boundary} command configuration byte count overflowed"),
        )
    })?;
    if observed > MAXIMUM_COMMAND_CONFIGURATION_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{boundary} command exceeds its configuration-byte ceiling"),
        ));
    }
    Ok(observed)
}

fn validate_limits(limits: BoundedProcessLimits) -> io::Result<()> {
    if limits.cpu_seconds == 0
        || limits.address_space_bytes == 0
        || limits.file_size_bytes == 0
        || limits.open_files < 3
        || limits.active_processes == 0
        || limits.process_memory_bytes == 0
        || limits.aggregate_memory_bytes < limits.process_memory_bytes
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bounded process limits are internally inconsistent",
        ));
    }
    Ok(())
}

fn os_str_bytes(value: &OsStr, _boundary: &str) -> io::Result<usize> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(value.as_bytes().len())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        value.encode_wide().count().checked_mul(2).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{_boundary} command string byte count overflowed"),
            )
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(value.to_string_lossy().len())
    }
}
