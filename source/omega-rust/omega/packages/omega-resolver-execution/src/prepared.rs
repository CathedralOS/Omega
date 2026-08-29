use crate::ResolverExecutionPolicyObservation;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

const COMMAND_IDENTITY_DOMAIN: &[u8] = b"OMEGA-RESOLVER-PREPARED-COMMAND-V1\0";
const MAXIMUM_COMMAND_ARGUMENTS: usize = 4 * 1024;
const MAXIMUM_COMMAND_ENVIRONMENT_ENTRIES: usize = 1024;
const MAXIMUM_COMMAND_CONFIGURATION_BYTES: usize = 4 * 1024 * 1024;

/// Domain-separated identity of one prepared command's executable, arguments,
/// explicit environment, environment inheritance disposition, and working
/// directory.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResolverExecutionCommandIdentity([u8; 32]);

impl ResolverExecutionCommandIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for ResolverExecutionCommandIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for ResolverExecutionCommandIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// One resolver command whose executable, authority roots, native policy, and
/// resource limits were prepared together by [`crate::ResolverExecutionBackend`].
///
/// Callers may finish ordinary command configuration, but cannot extract or
/// replace the command. Spawning consumes this value so its policy cannot be
/// detached from the process it describes.
#[derive(Debug)]
pub struct ResolverPreparedExecution {
    command: Command,
    policy: ResolverExecutionPolicyObservation,
    environment_cleared: bool,
}

impl ResolverPreparedExecution {
    pub(crate) const fn new(command: Command, policy: ResolverExecutionPolicyObservation) -> Self {
        Self {
            command,
            policy,
            environment_cleared: false,
        }
    }

    pub fn env_clear(&mut self) -> &mut Self {
        self.command.env_clear();
        self.environment_cleared = true;
        self
    }

    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.command.env(key, value);
        self
    }

    pub fn current_dir(&mut self, directory: impl AsRef<Path>) -> &mut Self {
        self.command.current_dir(directory);
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

    pub fn stdin(&mut self, configuration: Stdio) -> &mut Self {
        self.command.stdin(configuration);
        self
    }

    pub fn stdout(&mut self, configuration: Stdio) -> &mut Self {
        self.command.stdout(configuration);
        self
    }

    pub fn stderr(&mut self, configuration: Stdio) -> &mut Self {
        self.command.stderr(configuration);
        self
    }

    pub fn program(&self) -> &OsStr {
        self.command.get_program()
    }

    pub fn get_program(&self) -> &OsStr {
        self.program()
    }

    pub fn arguments(&self) -> impl Iterator<Item = &OsStr> {
        self.command.get_args()
    }

    pub fn get_args(&self) -> impl Iterator<Item = &OsStr> {
        self.arguments()
    }

    pub fn environment(&self) -> impl Iterator<Item = (&OsStr, Option<&OsStr>)> {
        self.command.get_envs()
    }

    pub fn get_envs(&self) -> impl Iterator<Item = (&OsStr, Option<&OsStr>)> {
        self.environment()
    }

    pub fn current_directory(&self) -> Option<&Path> {
        self.command.get_current_dir()
    }

    pub fn get_current_dir(&self) -> Option<&Path> {
        self.current_directory()
    }

    /// Identify the exact executable/argument/environment/directory
    /// configuration that will be consumed by spawn. Standard-input content is
    /// deliberately bound by the protocol owner because `Stdio` does not expose
    /// a portable inspectable identity.
    pub fn command_identity(&self) -> io::Result<ResolverExecutionCommandIdentity> {
        command_identity(&self.command, self.environment_cleared)
    }

    pub(crate) fn into_parts(self) -> (Command, ResolverExecutionPolicyObservation) {
        (self.command, self.policy)
    }
}

fn command_identity(
    command: &Command,
    environment_cleared: bool,
) -> io::Result<ResolverExecutionCommandIdentity> {
    let argument_count = command.get_args().count();
    if argument_count > MAXIMUM_COMMAND_ARGUMENTS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "resolver command exceeds its argument-count ceiling",
        ));
    }
    let environment_count = command.get_envs().count();
    if environment_count > MAXIMUM_COMMAND_ENVIRONMENT_ENTRIES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "resolver command exceeds its environment-entry ceiling",
        ));
    }
    let environment = command
        .get_envs()
        .map(|(name, value)| (name.to_os_string(), value.map(OsStr::to_os_string)))
        .collect::<BTreeMap<_, _>>();

    let mut observed_bytes = 0usize;
    let mut digest = Sha256::new();
    digest.update(COMMAND_IDENTITY_DOMAIN);
    hash_os_str(&mut digest, command.get_program(), &mut observed_bytes)?;
    hash_count(&mut digest, argument_count);
    for argument in command.get_args() {
        hash_os_str(&mut digest, argument, &mut observed_bytes)?;
    }
    digest.update([u8::from(environment_cleared)]);
    hash_count(&mut digest, environment.len());
    for (name, value) in environment {
        hash_os_str(&mut digest, &name, &mut observed_bytes)?;
        match value {
            Some(value) => {
                digest.update([1]);
                hash_os_str(&mut digest, &value, &mut observed_bytes)?;
            }
            None => digest.update([0]),
        }
    }
    match command.get_current_dir() {
        Some(directory) => {
            digest.update([1]);
            hash_os_str(&mut digest, directory.as_os_str(), &mut observed_bytes)?;
        }
        None => digest.update([0]),
    }
    Ok(ResolverExecutionCommandIdentity(digest.finalize().into()))
}

fn hash_count(digest: &mut Sha256, count: usize) {
    digest.update(
        u64::try_from(count)
            .expect("bounded resolver command count fits u64")
            .to_le_bytes(),
    );
}

fn charge_bytes(observed: &mut usize, additional: usize) -> io::Result<()> {
    *observed = observed.checked_add(additional).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "resolver command configuration byte count overflowed",
        )
    })?;
    if *observed > MAXIMUM_COMMAND_CONFIGURATION_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "resolver command exceeds its configuration-byte ceiling",
        ));
    }
    Ok(())
}

fn hash_os_str(digest: &mut Sha256, value: &OsStr, observed_bytes: &mut usize) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let bytes = value.as_bytes();
        charge_bytes(observed_bytes, bytes.len())?;
        hash_count(digest, bytes.len());
        digest.update(bytes);
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let units = value.encode_wide().collect::<Vec<_>>();
        let byte_count = units.len().checked_mul(2).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver command string byte count overflowed",
            )
        })?;
        charge_bytes(observed_bytes, byte_count)?;
        hash_count(digest, units.len());
        for unit in units {
            digest.update(unit.to_le_bytes());
        }
    }
    Ok(())
}
