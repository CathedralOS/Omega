use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

const MAXIMUM_COMMAND_ARGUMENTS: usize = 4 * 1024;
const MAXIMUM_COMMAND_ENVIRONMENT_ENTRIES: usize = 1024;
const MAXIMUM_COMMAND_CONFIGURATION_BYTES: usize = 4 * 1024 * 1024;

/// One structured resolver command using the backend's frozen executable.
///
/// The raw command cannot be extracted or replaced. Callers may add bounded
/// arguments and environment changes and must explicitly select null or piped
/// custody for every standard stream before spawning consumes this value.
#[derive(Debug)]
pub struct ResolverPreparedExecution {
    command: Command,
    stdin: Option<ResolverStandardStreamDisposition>,
    stdout: Option<ResolverStandardStreamDisposition>,
    stderr: Option<ResolverStandardStreamDisposition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolverStandardStreamDisposition {
    Null,
    Piped,
}

impl ResolverPreparedExecution {
    pub(crate) const fn new(command: Command) -> Self {
        Self {
            command,
            stdin: None,
            stdout: None,
            stderr: None,
        }
    }

    pub fn env_clear(&mut self) -> &mut Self {
        self.command.env_clear();
        self
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

    pub fn stdin_null(&mut self) -> &mut Self {
        self.command.stdin(Stdio::null());
        self.stdin = Some(ResolverStandardStreamDisposition::Null);
        self
    }

    pub fn stdin_piped(&mut self) -> &mut Self {
        self.command.stdin(Stdio::piped());
        self.stdin = Some(ResolverStandardStreamDisposition::Piped);
        self
    }

    pub fn stdout_null(&mut self) -> &mut Self {
        self.command.stdout(Stdio::null());
        self.stdout = Some(ResolverStandardStreamDisposition::Null);
        self
    }

    pub fn stdout_piped(&mut self) -> &mut Self {
        self.command.stdout(Stdio::piped());
        self.stdout = Some(ResolverStandardStreamDisposition::Piped);
        self
    }

    pub fn stderr_null(&mut self) -> &mut Self {
        self.command.stderr(Stdio::null());
        self.stderr = Some(ResolverStandardStreamDisposition::Null);
        self
    }

    pub fn stderr_piped(&mut self) -> &mut Self {
        self.command.stderr(Stdio::piped());
        self.stderr = Some(ResolverStandardStreamDisposition::Piped);
        self
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

    pub(crate) fn into_command(self) -> io::Result<Command> {
        self.validate()?;
        Ok(self.command)
    }

    fn validate(&self) -> io::Result<()> {
        if !matches!(
            (self.stdin, self.stdout, self.stderr),
            (Some(_), Some(_), Some(_))
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver standard streams must each be explicitly null or piped",
            ));
        }
        if self.command.get_args().count() > MAXIMUM_COMMAND_ARGUMENTS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver command exceeds its argument-count ceiling",
            ));
        }
        if self.command.get_envs().count() > MAXIMUM_COMMAND_ENVIRONMENT_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver command exceeds its environment-entry ceiling",
            ));
        }

        let mut bytes = os_str_bytes(self.command.get_program())?;
        for argument in self.command.get_args() {
            bytes = charge_bytes(bytes, os_str_bytes(argument)?)?;
        }
        for (name, value) in self.command.get_envs() {
            bytes = charge_bytes(bytes, os_str_bytes(name)?)?;
            if let Some(value) = value {
                bytes = charge_bytes(bytes, os_str_bytes(value)?)?;
            }
        }
        if let Some(directory) = self.command.get_current_dir() {
            let _ = charge_bytes(bytes, os_str_bytes(directory.as_os_str())?)?;
        }
        Ok(())
    }
}

fn charge_bytes(observed: usize, additional: usize) -> io::Result<usize> {
    let observed = observed.checked_add(additional).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "resolver command configuration byte count overflowed",
        )
    })?;
    if observed > MAXIMUM_COMMAND_CONFIGURATION_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "resolver command exceeds its configuration-byte ceiling",
        ));
    }
    Ok(observed)
}

fn os_str_bytes(value: &OsStr) -> io::Result<usize> {
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
                "resolver command string byte count overflowed",
            )
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(value.to_string_lossy().len())
    }
}
