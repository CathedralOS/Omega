//! Native process enforcement for compiler-owned package-source resolution.
//!
//! This crate owns the platform-specific launch mechanism. Callers choose one
//! closed resolver phase and provide compiler-selected executable and custody
//! paths; they cannot author sandbox policy text or containment claims.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_os = "macos")]
use sha2::{Digest, Sha256};
#[cfg(target_os = "macos")]
use std::ffi::OsString;
#[cfg(target_os = "macos")]
use std::fs::File;
use std::io;
#[cfg(target_os = "macos")]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "macos")]
const EXECUTABLE_BYTE_LIMIT: u64 = 256 * 1024 * 1024;
#[cfg(unix)]
const CHILD_CPU_SECONDS: u64 = 120;
#[cfg(any(target_os = "linux", target_os = "android"))]
const CHILD_ADDRESS_SPACE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
#[cfg(unix)]
const CHILD_FILE_SIZE_BYTES: u64 = 1024 * 1024 * 1024;
#[cfg(unix)]
const CHILD_OPEN_FILE_LIMIT: u64 = 256;

#[cfg(target_os = "macos")]
const MACOS_SANDBOX_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

/// One compiler-owned source-resolution phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionPhase {
    /// Remote object-format and selector discovery. Network is available; disk
    /// mutation is not.
    TransportDiscovery,
    /// Creation of a new local bare repository. Only the supplied mutable root
    /// is writable; network is unavailable.
    RepositoryInitialization,
    /// Fetch into an existing quarantine. Network and the supplied mutable root
    /// are available.
    Fetch,
    /// Read-only object and tree inspection. Neither network nor filesystem
    /// mutation is available.
    RepositoryInspection,
}

impl ResolverExecutionPhase {
    const fn permits_network(self) -> bool {
        matches!(self, Self::TransportDiscovery | Self::Fetch)
    }

    const fn requires_mutable_root(self) -> bool {
        matches!(self, Self::RepositoryInitialization | Self::Fetch)
    }
}

/// Closed identity of the native enforcement backend selected by this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolverExecutionBackendIdentity {
    MacosSeatbelt {
        executable: PathBuf,
        content_sha256: String,
    },
    UnixResourceLimits,
    PortableProcessContainer,
}

/// A verified native launch backend.
#[derive(Debug)]
pub struct ResolverExecutionBackend {
    identity: ResolverExecutionBackendIdentity,
    #[cfg(target_os = "macos")]
    sandbox_metadata: ExecutableMetadataIdentity,
}

impl ResolverExecutionBackend {
    pub fn open() -> io::Result<Self> {
        #[cfg(target_os = "macos")]
        {
            let path = PathBuf::from(MACOS_SANDBOX_EXECUTABLE);
            verify_owned_native_executable(&path)?;
            let sandbox_metadata = executable_metadata_identity(&path)?;
            let content_sha256 = hash_executable(&path)?;
            if executable_metadata_identity(&path)? != sandbox_metadata {
                return Err(io::Error::other(
                    "macOS resolver sandbox executable changed while opening",
                ));
            }
            return Ok(Self {
                identity: ResolverExecutionBackendIdentity::MacosSeatbelt {
                    executable: path,
                    content_sha256,
                },
                sandbox_metadata,
            });
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            Ok(Self {
                identity: ResolverExecutionBackendIdentity::UnixResourceLimits,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {
                identity: ResolverExecutionBackendIdentity::PortableProcessContainer,
            })
        }
    }

    pub const fn identity(&self) -> &ResolverExecutionBackendIdentity {
        &self.identity
    }

    pub fn verify(&self) -> io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            let ResolverExecutionBackendIdentity::MacosSeatbelt {
                executable,
                content_sha256,
            } = &self.identity
            else {
                return Err(io::Error::other(
                    "macOS resolver selected a non-Seatbelt backend",
                ));
            };
            verify_owned_native_executable(executable)?;
            if executable_metadata_identity(executable)? != self.sandbox_metadata
                || hash_executable(executable)? != *content_sha256
            {
                return Err(io::Error::other(
                    "macOS resolver sandbox executable changed",
                ));
            }
        }
        Ok(())
    }

    /// Construct a command under the host's selected native enforcement.
    ///
    /// `additional_executables` is the closed set of transport helpers the
    /// already-verified primary executable may launch. `mutable_root` is
    /// required exactly for the two mutating phases and rejected otherwise.
    pub fn command(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<Command> {
        self.verify()?;
        require_absolute(executable, "resolver executable")?;
        for helper in additional_executables {
            require_absolute(helper, "resolver helper executable")?;
        }
        match (phase.requires_mutable_root(), mutable_root) {
            (true, Some(root)) => require_absolute(root, "resolver mutable root")?,
            (true, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "mutating resolver phase has no mutable root",
                ));
            }
            (false, Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "read-only resolver phase received a mutable root",
                ));
            }
            (false, None) => {}
        }

        #[cfg(target_os = "macos")]
        let mut command =
            self.macos_command(executable, additional_executables, phase, mutable_root)?;
        #[cfg(not(target_os = "macos"))]
        let mut command = Command::new(executable);

        configure_child_resource_limits(&mut command)?;
        Ok(command)
    }

    #[cfg(target_os = "macos")]
    fn macos_command(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<Command> {
        let ResolverExecutionBackendIdentity::MacosSeatbelt {
            executable: sandbox_executable,
            ..
        } = &self.identity
        else {
            return Err(io::Error::other(
                "macOS resolver selected a non-Seatbelt backend",
            ));
        };
        let mut profile = String::from(
            "(version 1) (deny default) (import \"system.sb\") \
             (allow process-fork) (allow signal) (allow file-read*) \
             (allow process-exec (literal (param \"EXECUTABLE_0\"))",
        );
        for index in 0..additional_executables.len() {
            profile.push_str(&format!(" (literal (param \"EXECUTABLE_{}\"))", index + 1));
        }
        profile.push(')');
        if phase.permits_network() {
            profile.push_str(" (allow network-outbound)");
        }
        if phase.requires_mutable_root() {
            profile.push_str(" (allow file-write* (subpath (param \"MUTABLE_ROOT\")))");
        }

        let mut command = Command::new(sandbox_executable);
        command
            .arg("-D")
            .arg(definition_argument("EXECUTABLE_0", executable));
        for (index, helper) in additional_executables.iter().enumerate() {
            command.arg("-D").arg(definition_argument(
                &format!("EXECUTABLE_{}", index + 1),
                helper,
            ));
        }
        if let Some(root) = mutable_root {
            command
                .arg("-D")
                .arg(definition_argument("MUTABLE_ROOT", root));
        }
        command.arg("-p").arg(profile).arg(executable);
        Ok(command)
    }
}

fn require_absolute(path: &Path, name: &str) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} is not absolute"),
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn definition_argument(name: &str, value: &Path) -> OsString {
    let mut argument = OsString::from(name);
    argument.push("=");
    argument.push(value.as_os_str());
    argument
}

#[cfg(unix)]
fn configure_child_resource_limits(command: &mut Command) -> io::Result<()> {
    use std::os::unix::process::CommandExt;

    // SAFETY: the pre-exec closure performs only fixed `setrlimit` syscalls and
    // captures no references. The limits are inherited by the complete helper
    // process tree after exec.
    unsafe {
        command.pre_exec(|| {
            set_limit(rustix::process::Resource::Core, 0)?;
            set_limit(rustix::process::Resource::Cpu, CHILD_CPU_SECONDS)?;
            #[cfg(any(target_os = "linux", target_os = "android"))]
            set_limit(rustix::process::Resource::As, CHILD_ADDRESS_SPACE_BYTES)?;
            set_limit(rustix::process::Resource::Fsize, CHILD_FILE_SIZE_BYTES)?;
            set_limit(rustix::process::Resource::Nofile, CHILD_OPEN_FILE_LIMIT)
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn configure_child_resource_limits(_command: &mut Command) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_limit(resource: rustix::process::Resource, value: u64) -> io::Result<()> {
    let limit = intersect_limit(rustix::process::getrlimit(resource), value);
    rustix::process::setrlimit(resource, limit).map_err(io::Error::from)
}

#[cfg(unix)]
fn intersect_limit(inherited: rustix::process::Rlimit, ceiling: u64) -> rustix::process::Rlimit {
    let maximum = inherited
        .maximum
        .map_or(ceiling, |limit| limit.min(ceiling));
    let current = inherited
        .current
        .map_or(maximum, |limit| limit.min(maximum));
    rustix::process::Rlimit {
        current: Some(current),
        maximum: Some(maximum),
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutableMetadataIdentity {
    length: u64,
    device: u64,
    inode: u64,
    mode: u32,
}

#[cfg(target_os = "macos")]
fn executable_metadata_identity(path: &Path) -> io::Result<ExecutableMetadataIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native resolver backend is not a concrete regular file",
        ));
    }
    Ok(ExecutableMetadataIdentity {
        length: metadata.len(),
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
    })
}

#[cfg(target_os = "macos")]
fn verify_owned_native_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.mode() & 0o6000 != 0
        || metadata.mode() & 0o111 == 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native resolver backend lacks root-owned executable custody",
        ));
    }
    let executable = File::open(path)?;
    if omega_platform_custody::open_file_extended_acl_has_allow_entry(&executable)? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native resolver backend has an extended ACL allow entry",
        ));
    }
    for ancestor in path
        .parent()
        .ok_or_else(|| io::Error::other("native resolver backend has no parent"))?
        .ancestors()
    {
        let metadata = std::fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0
            || omega_platform_custody::extended_acl_has_allow_entry(
                ancestor,
                omega_platform_custody::SymbolicLinkBehavior::Follow,
            )?
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "native resolver backend ancestry lacks root-owned custody",
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn hash_executable(path: &Path) -> io::Result<String> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > EXECUTABLE_BYTE_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native resolver backend exceeds its executable byte ceiling",
        ));
    }
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(u64::try_from(count).unwrap_or(u64::MAX))
            .ok_or_else(|| io::Error::other("native resolver backend length overflowed"))?;
        if observed > EXECUTABLE_BYTE_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "native resolver backend exceeds its executable byte ceiling",
            ));
        }
        hasher.update(&buffer[..count]);
    }
    if observed != metadata.len() {
        return Err(io::Error::other(
            "native resolver backend changed while hashing",
        ));
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::CHILD_OPEN_FILE_LIMIT;
    use super::{ResolverExecutionBackend, ResolverExecutionPhase};
    use std::path::Path;
    #[cfg(target_os = "macos")]
    use std::process::Stdio;

    #[test]
    fn mutability_is_derived_from_the_closed_phase() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let executable = if cfg!(windows) {
            Path::new(r"C:\Windows\System32\cmd.exe")
        } else {
            Path::new("/bin/sh")
        };
        assert!(
            backend
                .command(
                    executable,
                    &[],
                    ResolverExecutionPhase::RepositoryInspection,
                    Some(Path::new("/tmp")),
                )
                .is_err()
        );
        assert!(
            backend
                .command(executable, &[], ResolverExecutionPhase::Fetch, None,)
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn compiler_resource_ceilings_never_loosen_inherited_limits() {
        use super::intersect_limit;
        use rustix::process::Rlimit;

        assert_eq!(
            intersect_limit(
                Rlimit {
                    current: Some(64),
                    maximum: Some(1_024),
                },
                256,
            ),
            Rlimit {
                current: Some(64),
                maximum: Some(256),
            }
        );
        assert_eq!(
            intersect_limit(
                Rlimit {
                    current: Some(64),
                    maximum: Some(64),
                },
                256,
            ),
            Rlimit {
                current: Some(64),
                maximum: Some(64),
            }
        );
        assert_eq!(
            intersect_limit(
                Rlimit {
                    current: None,
                    maximum: None,
                },
                256,
            ),
            Rlimit {
                current: Some(256),
                maximum: Some(256),
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn child_open_file_limit_is_enforced() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        #[cfg(target_os = "macos")]
        let helper_executables = [Path::new("/bin/bash").to_path_buf()];
        #[cfg(not(target_os = "macos"))]
        let helper_executables = [];
        let mut command = backend
            .command(
                Path::new("/bin/sh"),
                &helper_executables,
                ResolverExecutionPhase::RepositoryInspection,
                None,
            )
            .expect("build limited shell");
        let output = command
            .args(["-c", "ulimit -n"])
            .output()
            .expect("run limited shell");
        assert!(output.status.success());
        let limit = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .expect("shell reports a numeric descriptor limit");
        assert!(limit <= CHILD_OPEN_FILE_LIMIT);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_confines_writes_to_the_mutable_root() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "omega-resolver-execution-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&root).expect("create sandbox root");
        let root = root.canonicalize().expect("canonicalize sandbox root");
        let inside = root.join("inside");
        let outside = root.with_extension("outside");
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let helper_executables = [Path::new("/bin/bash").to_path_buf()];
        let mut allowed = backend
            .command(
                Path::new("/bin/sh"),
                &helper_executables,
                ResolverExecutionPhase::RepositoryInitialization,
                Some(&root),
            )
            .expect("build writable sandbox");
        let status = allowed
            .args(["-c", "printf allowed > \"$1\"", "resolver-test"])
            .arg(&inside)
            .status()
            .expect("run allowed write");
        assert!(status.success());

        let mut denied = backend
            .command(
                Path::new("/bin/sh"),
                &helper_executables,
                ResolverExecutionPhase::RepositoryInitialization,
                Some(&root),
            )
            .expect("build confined sandbox");
        let status = denied
            .args(["-c", "printf denied > \"$1\"", "resolver-test"])
            .arg(&outside)
            .status()
            .expect("run denied write");
        assert!(!status.success());
        assert!(!outside.exists());
        std::fs::remove_file(inside).expect("remove sandbox output");
        std::fs::remove_dir(root).expect("remove sandbox root");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_rejects_unlisted_descendant_executables() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let output = std::env::temp_dir().join(format!(
            "omega-resolver-exec-denied-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&output).expect("create executable-denial root");
        let output = output.canonicalize().expect("canonicalize denial root");
        let marker = output.join("marker");
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let helper_executables = [Path::new("/bin/bash").to_path_buf()];
        let mut command = backend
            .command(
                Path::new("/bin/sh"),
                &helper_executables,
                ResolverExecutionPhase::RepositoryInitialization,
                Some(&output),
            )
            .expect("build closed-executable sandbox");
        let status = command
            .args(["-c", "/usr/bin/touch \"$1\"", "resolver-test"])
            .arg(&marker)
            .status()
            .expect("attempt unlisted descendant execution");
        assert!(!status.success());
        assert!(!marker.exists());
        std::fs::remove_dir(output).expect("remove executable-denial root");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_derives_network_access_from_the_phase() {
        use std::io::ErrorKind;
        use std::net::TcpListener;

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback canary");
        listener
            .set_nonblocking(true)
            .expect("make canary listener nonblocking");
        let port = listener.local_addr().expect("read canary address").port();
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let mut denied = backend
            .command(
                Path::new("/usr/bin/nc"),
                &[],
                ResolverExecutionPhase::RepositoryInspection,
                None,
            )
            .expect("build network-denied sandbox");
        let status = denied
            .args(["127.0.0.1", &port.to_string()])
            .stdin(Stdio::null())
            .status()
            .expect("attempt denied loopback connection");
        assert!(!status.success());
        assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));

        let acceptance = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            loop {
                match listener.accept() {
                    Ok((_connection, _address)) => return,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "network-enabled phase did not reach the loopback listener"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(error) => panic!("loopback listener failed: {error}"),
                }
            }
        });
        let mut allowed = backend
            .command(
                Path::new("/usr/bin/nc"),
                &[],
                ResolverExecutionPhase::TransportDiscovery,
                None,
            )
            .expect("build network-enabled sandbox");
        let status = allowed
            .args(["127.0.0.1", &port.to_string()])
            .stdin(Stdio::null())
            .status()
            .expect("connect to loopback canary");
        assert!(status.success());
        acceptance.join().expect("observe admitted connection");
    }
}
