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

const RESOLVER_EXECUTION_OBSERVATION_SCHEMA_VERSION: u32 = 1;
const RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT: usize = 32;
const RESOLVER_EXECUTION_PATH_BYTE_LIMIT: usize = 32 * 1024;
const RESOLVER_EXECUTION_CANONICAL_BYTE_LIMIT: usize = 2 * 1024 * 1024;

#[cfg(target_os = "macos")]
const EXECUTABLE_BYTE_LIMIT: u64 = 256 * 1024 * 1024;
#[cfg(target_os = "macos")]
const POLICY_PROFILE_BYTE_LIMIT: u64 = 1024 * 1024;
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
#[cfg(target_os = "macos")]
const MACOS_SYSTEM_PROFILE: &str = "/System/Library/Sandbox/Profiles/system.sb";
#[cfg(target_os = "macos")]
const MACOS_DYLD_PROFILE: &str = "/System/Library/Sandbox/Profiles/dyld-support.sb";
#[cfg(target_os = "macos")]
const MACOS_NULL_DEVICE: &str = "/dev/null";
#[cfg(target_os = "macos")]
const MACOS_DIRECTORY_LOOKUP_SERVICE: &str = "com.apple.system.opendirectoryd.libinfo";
#[cfg(target_os = "macos")]
const MACOS_HOSTNAME_SYSCTL: &str = "kern.hostname";

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

    const fn uses_self_contained_macos_policy(self) -> bool {
        matches!(
            self,
            Self::TransportDiscovery | Self::RepositoryInitialization | Self::RepositoryInspection
        )
    }

    const fn tag(self) -> u8 {
        match self {
            Self::TransportDiscovery => 1,
            Self::RepositoryInitialization => 2,
            Self::Fetch => 3,
            Self::RepositoryInspection => 4,
        }
    }
}

/// One native guarantee required by strict package-source resolution.
///
/// This vocabulary describes only the process-isolation backend. The complete
/// source receipt must separately bind resolver configuration, endpoint trust,
/// object verification, and immutable snapshot publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResolverExecutionGuarantee {
    FilesystemWritesConfined,
    FilesystemReadsConfined,
    NetworkDenied,
    NetworkEndpointsConfined,
    ExecutablePathsConfined,
    DescendantProcessesContained,
    CoreDumpsDenied,
    CpuTimeConfined,
    SingleFileSizeConfined,
    OpenFilesConfined,
    AddressSpaceConfined,
    ProcessCountConfined,
    AggregateResourcesConfined,
}

impl ResolverExecutionGuarantee {
    const ALL: [Self; 13] = [
        Self::FilesystemWritesConfined,
        Self::FilesystemReadsConfined,
        Self::NetworkDenied,
        Self::NetworkEndpointsConfined,
        Self::ExecutablePathsConfined,
        Self::DescendantProcessesContained,
        Self::CoreDumpsDenied,
        Self::CpuTimeConfined,
        Self::SingleFileSizeConfined,
        Self::OpenFilesConfined,
        Self::AddressSpaceConfined,
        Self::ProcessCountConfined,
        Self::AggregateResourcesConfined,
    ];

    const fn tag(self) -> u8 {
        match self {
            Self::FilesystemWritesConfined => 1,
            Self::FilesystemReadsConfined => 2,
            Self::NetworkDenied => 3,
            Self::NetworkEndpointsConfined => 4,
            Self::ExecutablePathsConfined => 5,
            Self::DescendantProcessesContained => 6,
            Self::CoreDumpsDenied => 7,
            Self::CpuTimeConfined => 8,
            Self::SingleFileSizeConfined => 9,
            Self::OpenFilesConfined => 10,
            Self::AddressSpaceConfined => 11,
            Self::ProcessCountConfined => 12,
            Self::AggregateResourcesConfined => 13,
        }
    }
}

/// Whether one native guarantee was required and established for a phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionGuaranteeDisposition {
    Enforced,
    Unavailable,
    NotRequired,
}

impl ResolverExecutionGuaranteeDisposition {
    const fn tag(self) -> u8 {
        match self {
            Self::Enforced => 1,
            Self::Unavailable => 2,
            Self::NotRequired => 3,
        }
    }
}

/// One fixed-vocabulary row in a native execution policy observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolverExecutionGuaranteeRow {
    guarantee: ResolverExecutionGuarantee,
    disposition: ResolverExecutionGuaranteeDisposition,
}

impl ResolverExecutionGuaranteeRow {
    pub const fn guarantee(&self) -> ResolverExecutionGuarantee {
        self.guarantee
    }

    pub const fn disposition(&self) -> ResolverExecutionGuaranteeDisposition {
        self.disposition
    }
}

/// Canonical configuration observation from the verified local execution backend.
///
/// This value describes command construction, not execution. Fields are private
/// and there is intentionally no decoder or public constructor: persisted bytes
/// are bounded comparison material and cannot mint containment authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverExecutionPolicyObservation {
    backend: ResolverExecutionBackendIdentity,
    phase: ResolverExecutionPhase,
    generated_policy_sha256: Option<String>,
    resource_ceilings: ResolverExecutionResourceCeilings,
    executable: PathBuf,
    additional_executables: Vec<PathBuf>,
    mutable_root: Option<PathBuf>,
    guarantees: [ResolverExecutionGuaranteeRow; ResolverExecutionGuarantee::ALL.len()],
}

impl ResolverExecutionPolicyObservation {
    pub const fn backend(&self) -> &ResolverExecutionBackendIdentity {
        &self.backend
    }

    pub const fn phase(&self) -> ResolverExecutionPhase {
        self.phase
    }

    pub fn generated_policy_sha256(&self) -> Option<&str> {
        self.generated_policy_sha256.as_deref()
    }

    pub const fn resource_ceilings(&self) -> &ResolverExecutionResourceCeilings {
        &self.resource_ceilings
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn additional_executables(&self) -> &[PathBuf] {
        &self.additional_executables
    }

    pub fn mutable_root(&self) -> Option<&Path> {
        self.mutable_root.as_deref()
    }

    pub const fn guarantees(&self) -> &[ResolverExecutionGuaranteeRow] {
        &self.guarantees
    }

    /// Reject unless every strict guarantee required for this phase was
    /// established by the selected backend.
    pub fn require_strict(&self) -> Result<(), ResolverStrictExecutionUnavailable> {
        for row in self.guarantees {
            if row.disposition == ResolverExecutionGuaranteeDisposition::Unavailable {
                return Err(ResolverStrictExecutionUnavailable {
                    phase: self.phase,
                    guarantee: row.guarantee,
                });
            }
        }
        Ok(())
    }

    /// Emit deterministic opaque bytes for later comparison and provenance.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"omega-resolver-execution-policy\0");
        bytes.extend_from_slice(&RESOLVER_EXECUTION_OBSERVATION_SCHEMA_VERSION.to_le_bytes());
        encode_backend_identity(&mut bytes, &self.backend);
        bytes.push(self.phase.tag());
        match &self.generated_policy_sha256 {
            Some(identity) => {
                bytes.push(1);
                encode_bytes(&mut bytes, identity.as_bytes());
            }
            None => bytes.push(0),
        }
        self.resource_ceilings.encode(&mut bytes);
        encode_path(&mut bytes, &self.executable);
        bytes.extend_from_slice(&(self.additional_executables.len() as u64).to_le_bytes());
        for executable in &self.additional_executables {
            encode_path(&mut bytes, executable);
        }
        match &self.mutable_root {
            Some(root) => {
                bytes.push(1);
                encode_path(&mut bytes, root);
            }
            None => bytes.push(0),
        }
        bytes.extend_from_slice(&(self.guarantees.len() as u32).to_le_bytes());
        for row in self.guarantees {
            bytes.push(row.guarantee.tag());
            bytes.push(row.disposition.tag());
        }
        assert!(bytes.len() <= RESOLVER_EXECUTION_CANONICAL_BYTE_LIMIT);
        bytes
    }
}

/// Compiler-owned upper ceilings configured on each child process. `None`
/// means this native backend does not configure that resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverExecutionResourceCeilings {
    core_dump_bytes: Option<u64>,
    cpu_seconds: Option<u64>,
    single_file_bytes: Option<u64>,
    open_files: Option<u64>,
    address_space_bytes: Option<u64>,
}

impl ResolverExecutionResourceCeilings {
    pub const fn core_dump_bytes(&self) -> Option<u64> {
        self.core_dump_bytes
    }

    pub const fn cpu_seconds(&self) -> Option<u64> {
        self.cpu_seconds
    }

    pub const fn single_file_bytes(&self) -> Option<u64> {
        self.single_file_bytes
    }

    pub const fn open_files(&self) -> Option<u64> {
        self.open_files
    }

    pub const fn address_space_bytes(&self) -> Option<u64> {
        self.address_space_bytes
    }

    fn encode(&self, bytes: &mut Vec<u8>) {
        for ceiling in [
            self.core_dump_bytes,
            self.cpu_seconds,
            self.single_file_bytes,
            self.open_files,
            self.address_space_bytes,
        ] {
            match ceiling {
                Some(value) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                None => bytes.push(0),
            }
        }
    }
}

/// The first required native guarantee unavailable from the selected backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolverStrictExecutionUnavailable {
    phase: ResolverExecutionPhase,
    guarantee: ResolverExecutionGuarantee,
}

impl ResolverStrictExecutionUnavailable {
    pub const fn phase(&self) -> ResolverExecutionPhase {
        self.phase
    }

    pub const fn guarantee(&self) -> ResolverExecutionGuarantee {
        self.guarantee
    }
}

impl std::fmt::Display for ResolverStrictExecutionUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "resolver phase {:?} lacks required native guarantee {:?}",
            self.phase, self.guarantee
        )
    }
}

impl std::error::Error for ResolverStrictExecutionUnavailable {}

/// Closed identity of the native enforcement backend selected by this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolverExecutionBackendIdentity {
    MacosSeatbelt {
        executable: PathBuf,
        content_sha256: String,
        system_profile: PathBuf,
        system_profile_sha256: String,
        dyld_profile: PathBuf,
        dyld_profile_sha256: String,
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
    #[cfg(target_os = "macos")]
    system_profile_metadata: ExecutableMetadataIdentity,
    #[cfg(target_os = "macos")]
    dyld_profile_metadata: ExecutableMetadataIdentity,
}

impl ResolverExecutionBackend {
    pub fn open() -> io::Result<Self> {
        #[cfg(target_os = "macos")]
        {
            let path = PathBuf::from(MACOS_SANDBOX_EXECUTABLE);
            verify_owned_native_executable(&path)?;
            let sandbox_metadata = executable_metadata_identity(&path)?;
            let content_sha256 = hash_executable(&path)?;
            let system_profile = PathBuf::from(MACOS_SYSTEM_PROFILE);
            let dyld_profile = PathBuf::from(MACOS_DYLD_PROFILE);
            verify_owned_native_policy_file(&system_profile)?;
            verify_owned_native_policy_file(&dyld_profile)?;
            let system_profile_metadata = executable_metadata_identity(&system_profile)?;
            let dyld_profile_metadata = executable_metadata_identity(&dyld_profile)?;
            let system_profile_bytes =
                read_native_file(&system_profile, POLICY_PROFILE_BYTE_LIMIT)?;
            let dyld_profile_bytes = read_native_file(&dyld_profile, POLICY_PROFILE_BYTE_LIMIT)?;
            validate_seatbelt_import_topology(&system_profile_bytes, &dyld_profile_bytes)?;
            let system_profile_sha256 = format_sha256(&Sha256::digest(&system_profile_bytes));
            let dyld_profile_sha256 = format_sha256(&Sha256::digest(&dyld_profile_bytes));
            if executable_metadata_identity(&path)? != sandbox_metadata
                || executable_metadata_identity(&system_profile)? != system_profile_metadata
                || executable_metadata_identity(&dyld_profile)? != dyld_profile_metadata
            {
                return Err(io::Error::other(
                    "macOS resolver sandbox boundary changed while opening",
                ));
            }
            Ok(Self {
                identity: ResolverExecutionBackendIdentity::MacosSeatbelt {
                    executable: path,
                    content_sha256,
                    system_profile,
                    system_profile_sha256,
                    dyld_profile,
                    dyld_profile_sha256,
                },
                sandbox_metadata,
                system_profile_metadata,
                dyld_profile_metadata,
            })
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

    fn policy_observation(
        &self,
        phase: ResolverExecutionPhase,
        generated_policy_sha256: Option<String>,
        executable: &Path,
        additional_executables: &[PathBuf],
        mutable_root: Option<&Path>,
    ) -> io::Result<ResolverExecutionPolicyObservation> {
        self.verify()?;
        let guarantees =
            ResolverExecutionGuarantee::ALL.map(|guarantee| ResolverExecutionGuaranteeRow {
                guarantee,
                disposition: guarantee_disposition(&self.identity, phase, guarantee),
            });
        Ok(ResolverExecutionPolicyObservation {
            backend: self.identity.clone(),
            phase,
            generated_policy_sha256,
            resource_ceilings: configured_resource_ceilings(),
            executable: executable.to_path_buf(),
            additional_executables: additional_executables.to_vec(),
            mutable_root: mutable_root.map(Path::to_path_buf),
            guarantees,
        })
    }

    pub fn verify(&self) -> io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            let ResolverExecutionBackendIdentity::MacosSeatbelt {
                executable,
                content_sha256,
                system_profile,
                system_profile_sha256,
                dyld_profile,
                dyld_profile_sha256,
            } = &self.identity
            else {
                return Err(io::Error::other(
                    "macOS resolver selected a non-Seatbelt backend",
                ));
            };
            verify_owned_native_executable(executable)?;
            verify_owned_native_policy_file(system_profile)?;
            verify_owned_native_policy_file(dyld_profile)?;
            let system_profile_bytes = read_native_file(system_profile, POLICY_PROFILE_BYTE_LIMIT)?;
            let dyld_profile_bytes = read_native_file(dyld_profile, POLICY_PROFILE_BYTE_LIMIT)?;
            validate_seatbelt_import_topology(&system_profile_bytes, &dyld_profile_bytes)?;
            if executable_metadata_identity(executable)? != self.sandbox_metadata
                || hash_executable(executable)? != *content_sha256
                || executable_metadata_identity(system_profile)? != self.system_profile_metadata
                || executable_metadata_identity(dyld_profile)? != self.dyld_profile_metadata
                || format_sha256(&Sha256::digest(&system_profile_bytes)) != *system_profile_sha256
                || format_sha256(&Sha256::digest(&dyld_profile_bytes)) != *dyld_profile_sha256
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
    #[cfg(test)]
    fn command(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<Command> {
        self.command_with_observation(executable, additional_executables, phase, mutable_root)
            .map(|(command, _observation)| command)
    }

    /// Construct one command and its opaque policy observation from the same
    /// validated inputs. The observation does not state that the command ran.
    pub fn command_with_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.verify()?;
        require_absolute(executable, "resolver executable")?;
        if additional_executables.len() > RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver helper executable set exceeds its fixed limit",
            ));
        }
        require_canonical_bounded_path(executable, "resolver executable")?;
        for helper in additional_executables {
            require_absolute(helper, "resolver helper executable")?;
            require_canonical_bounded_path(helper, "resolver helper executable")?;
        }
        let mut additional_executables = additional_executables.to_vec();
        additional_executables.retain(|helper| helper != executable);
        additional_executables.sort();
        additional_executables.dedup();
        match (phase.requires_mutable_root(), mutable_root) {
            (true, Some(root)) => {
                require_absolute(root, "resolver mutable root")?;
                require_canonical_bounded_path(root, "resolver mutable root")?;
            }
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
        let (mut command, generated_policy_sha256) =
            self.macos_command(executable, &additional_executables, phase, mutable_root)?;
        #[cfg(not(target_os = "macos"))]
        let mut command = Command::new(executable);
        #[cfg(not(target_os = "macos"))]
        let generated_policy_sha256 = None;

        configure_child_resource_limits(&mut command)?;
        let observation = self.policy_observation(
            phase,
            generated_policy_sha256,
            executable,
            &additional_executables,
            mutable_root,
        )?;
        Ok((command, observation))
    }

    #[cfg(target_os = "macos")]
    fn macos_command(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<(Command, Option<String>)> {
        let ResolverExecutionBackendIdentity::MacosSeatbelt {
            executable: sandbox_executable,
            ..
        } = &self.identity
        else {
            return Err(io::Error::other(
                "macOS resolver selected a non-Seatbelt backend",
            ));
        };
        let mut profile = if phase.uses_self_contained_macos_policy() {
            format!(
                "(version 1) (deny default) \
                 (allow process-fork) (allow signal) (allow file-read*) \
                 (allow file-test-existence file-write-data (literal \"{MACOS_NULL_DEVICE}\")) \
                 (allow process-exec (literal (param \"EXECUTABLE_0\"))"
            )
        } else {
            String::from(
                "(version 1) (deny default) (import \"system.sb\") \
                 (allow process-fork) (allow signal) (allow file-read*) \
                 (allow process-exec (literal (param \"EXECUTABLE_0\"))",
            )
        };
        for index in 0..additional_executables.len() {
            profile.push_str(&format!(" (literal (param \"EXECUTABLE_{}\"))", index + 1));
        }
        profile.push(')');
        if phase.permits_network() {
            profile.push_str(" (allow network-outbound)");
        }
        if phase == ResolverExecutionPhase::TransportDiscovery {
            profile.push_str(&format!(
                " (allow mach-lookup (global-name \"{MACOS_DIRECTORY_LOOKUP_SERVICE}\")) \
                 (allow sysctl-read (sysctl-name \"{MACOS_HOSTNAME_SYSCTL}\"))"
            ));
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
        let profile_sha256 = format_sha256(Sha256::digest(profile.as_bytes()).as_slice());
        command.arg("-p").arg(profile).arg(executable);
        Ok((command, Some(profile_sha256)))
    }
}

fn guarantee_disposition(
    backend: &ResolverExecutionBackendIdentity,
    phase: ResolverExecutionPhase,
    guarantee: ResolverExecutionGuarantee,
) -> ResolverExecutionGuaranteeDisposition {
    use ResolverExecutionBackendIdentity::{
        MacosSeatbelt, PortableProcessContainer, UnixResourceLimits,
    };
    use ResolverExecutionGuarantee::{
        AddressSpaceConfined, AggregateResourcesConfined, CoreDumpsDenied, CpuTimeConfined,
        DescendantProcessesContained, ExecutablePathsConfined, FilesystemReadsConfined,
        FilesystemWritesConfined, NetworkDenied, NetworkEndpointsConfined, OpenFilesConfined,
        ProcessCountConfined, SingleFileSizeConfined,
    };
    use ResolverExecutionGuaranteeDisposition::{Enforced, NotRequired, Unavailable};

    match guarantee {
        FilesystemWritesConfined | ExecutablePathsConfined
            if matches!(backend, MacosSeatbelt { .. })
                && phase.uses_self_contained_macos_policy() =>
        {
            Enforced
        }
        FilesystemWritesConfined | ExecutablePathsConfined => Unavailable,
        FilesystemReadsConfined
        | DescendantProcessesContained
        | ProcessCountConfined
        | AggregateResourcesConfined => Unavailable,
        NetworkDenied
            if matches!(backend, MacosSeatbelt { .. })
                && phase.uses_self_contained_macos_policy()
                && !phase.permits_network() =>
        {
            Enforced
        }
        NetworkDenied if phase.permits_network() => NotRequired,
        NetworkDenied => Unavailable,
        NetworkEndpointsConfined if !phase.permits_network() => NotRequired,
        NetworkEndpointsConfined => Unavailable,
        CoreDumpsDenied | CpuTimeConfined | SingleFileSizeConfined | OpenFilesConfined => {
            match backend {
                MacosSeatbelt { .. } | UnixResourceLimits => Enforced,
                PortableProcessContainer => Unavailable,
            }
        }
        AddressSpaceConfined => match backend {
            UnixResourceLimits if cfg!(any(target_os = "linux", target_os = "android")) => Enforced,
            MacosSeatbelt { .. } | UnixResourceLimits | PortableProcessContainer => Unavailable,
        },
    }
}

fn configured_resource_ceilings() -> ResolverExecutionResourceCeilings {
    #[cfg(unix)]
    {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let address_space_bytes = Some(CHILD_ADDRESS_SPACE_BYTES);
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let address_space_bytes = None;
        ResolverExecutionResourceCeilings {
            core_dump_bytes: Some(0),
            cpu_seconds: Some(CHILD_CPU_SECONDS),
            single_file_bytes: Some(CHILD_FILE_SIZE_BYTES),
            open_files: Some(CHILD_OPEN_FILE_LIMIT),
            address_space_bytes,
        }
    }
    #[cfg(not(unix))]
    {
        ResolverExecutionResourceCeilings {
            core_dump_bytes: None,
            cpu_seconds: None,
            single_file_bytes: None,
            open_files: None,
            address_space_bytes: None,
        }
    }
}

fn encode_backend_identity(bytes: &mut Vec<u8>, identity: &ResolverExecutionBackendIdentity) {
    match identity {
        ResolverExecutionBackendIdentity::MacosSeatbelt {
            executable,
            content_sha256,
            system_profile,
            system_profile_sha256,
            dyld_profile,
            dyld_profile_sha256,
        } => {
            bytes.push(1);
            encode_path(bytes, executable);
            encode_bytes(bytes, content_sha256.as_bytes());
            encode_path(bytes, system_profile);
            encode_bytes(bytes, system_profile_sha256.as_bytes());
            encode_path(bytes, dyld_profile);
            encode_bytes(bytes, dyld_profile_sha256.as_bytes());
        }
        ResolverExecutionBackendIdentity::UnixResourceLimits => bytes.push(2),
        ResolverExecutionBackendIdentity::PortableProcessContainer => bytes.push(3),
    }
}

fn encode_path(bytes: &mut Vec<u8>, path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        bytes.push(1);
        encode_bytes(bytes, path.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
        bytes.push(2);
        bytes.extend_from_slice(&(units.len() as u64).to_le_bytes());
        for unit in units {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
    }
}

fn encode_bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value);
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

fn require_canonical_bounded_path(path: &Path, name: &str) -> io::Result<()> {
    use std::path::Component;

    if path_encoding_length(path) > RESOLVER_EXECUTION_PATH_BYTE_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} exceeds its fixed encoding limit"),
        ));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(std::path::MAIN_SEPARATOR_STR),
            Component::Normal(component) => normalized.push(component),
            Component::CurDir | Component::ParentDir => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{name} is not lexically canonical"),
                ));
            }
        }
    }
    if normalized.as_os_str() != path.as_os_str() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} is not lexically canonical"),
        ));
    }
    Ok(())
}

fn path_encoding_length(path: &Path) -> usize {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().len()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().count().saturating_mul(2)
    }
    #[cfg(not(any(unix, windows)))]
    {
        path.as_os_str().to_string_lossy().len()
    }
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
fn verify_owned_native_policy_file(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.mode() & 0o6000 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native resolver policy profile lacks root-owned immutable custody",
        ));
    }
    let profile = File::open(path)?;
    if omega_platform_custody::open_file_extended_acl_has_allow_entry(&profile)? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native resolver policy profile has an extended ACL allow entry",
        ));
    }
    for ancestor in path
        .parent()
        .ok_or_else(|| io::Error::other("native resolver policy profile has no parent"))?
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
                "native resolver policy profile ancestry lacks root-owned custody",
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn read_native_file(path: &Path, byte_limit: u64) -> io::Result<Vec<u8>> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > byte_limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native resolver policy file exceeds its byte ceiling",
        ));
    }
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| io::Error::other("native resolver policy file length is not addressable"))?;
    let mut bytes = Vec::with_capacity(capacity);
    File::open(path)?
        .take(byte_limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != metadata.len() || bytes.len() as u64 > byte_limit {
        return Err(io::Error::other(
            "native resolver policy file changed while reading",
        ));
    }
    Ok(bytes)
}

#[cfg(target_os = "macos")]
fn validate_seatbelt_import_topology(system: &[u8], dyld: &[u8]) -> io::Result<()> {
    let system_imports = seatbelt_imports(system)?;
    let dyld_imports = seatbelt_imports(dyld)?;
    if system_imports != ["dyld-support.sb"] || !dyld_imports.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native resolver Seatbelt profile is outside the accepted direct-import syntax subset",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn seatbelt_imports(bytes: &[u8]) -> io::Result<Vec<String>> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "native resolver Seatbelt profile is not UTF-8",
        )
    })?;
    let tokens = seatbelt_tokens(text)?;
    let mut imports = Vec::new();
    let mut lists = Vec::<SeatbeltListFrame>::new();
    for token in tokens {
        match token {
            SeatbeltToken::Open => {
                observe_seatbelt_list_datum(lists.last_mut())?;
                lists.push(SeatbeltListFrame::default());
            }
            SeatbeltToken::Close => {
                let list = lists.pop().ok_or_else(seatbelt_syntax_error)?;
                if list.is_import {
                    let imported = list.imported.ok_or_else(seatbelt_import_error)?;
                    if list.datums != 2 {
                        return Err(seatbelt_import_error());
                    }
                    imports.push(imported.to_owned());
                }
            }
            SeatbeltToken::Atom(atom) => {
                observe_seatbelt_atom(lists.last_mut(), atom)?;
            }
            SeatbeltToken::String { content, escaped } => {
                observe_seatbelt_string(lists.last_mut(), content, escaped)?;
            }
        }
    }
    if !lists.is_empty() {
        return Err(seatbelt_syntax_error());
    }
    Ok(imports)
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeatbeltToken<'text> {
    Open,
    Close,
    Atom(&'text str),
    String { content: &'text str, escaped: bool },
}

#[cfg(target_os = "macos")]
#[derive(Debug, Default)]
struct SeatbeltListFrame<'text> {
    datums: usize,
    is_import: bool,
    imported: Option<&'text str>,
}

#[cfg(target_os = "macos")]
fn seatbelt_tokens(text: &str) -> io::Result<Vec<SeatbeltToken<'_>>> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            byte if byte.is_ascii_whitespace() => cursor += 1,
            b';' => {
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
            }
            b'#' if bytes.get(cursor + 1) == Some(&b'|') => {
                cursor = skip_seatbelt_block_comment(bytes, cursor + 2)?;
            }
            b'#' if bytes.get(cursor + 1) == Some(&b';') => {
                return Err(seatbelt_syntax_error());
            }
            b'(' => {
                tokens.push(SeatbeltToken::Open);
                cursor += 1;
            }
            b')' => {
                tokens.push(SeatbeltToken::Close);
                cursor += 1;
            }
            b'"' => {
                let content_start = cursor + 1;
                cursor = content_start;
                let mut escaped = false;
                loop {
                    let Some(byte) = bytes.get(cursor).copied() else {
                        return Err(seatbelt_syntax_error());
                    };
                    match byte {
                        b'"' => {
                            let content = &text[content_start..cursor];
                            tokens.push(SeatbeltToken::String { content, escaped });
                            cursor += 1;
                            break;
                        }
                        b'\\' => {
                            escaped = true;
                            cursor = cursor
                                .checked_add(2)
                                .filter(|cursor| *cursor <= bytes.len())
                                .ok_or_else(seatbelt_syntax_error)?;
                        }
                        _ => cursor += 1,
                    }
                }
            }
            _ => {
                let start = cursor;
                while cursor < bytes.len() && !seatbelt_token_delimiter(bytes, cursor) {
                    cursor += 1;
                }
                if cursor == start {
                    return Err(seatbelt_syntax_error());
                }
                tokens.push(SeatbeltToken::Atom(&text[start..cursor]));
            }
        }
    }
    Ok(tokens)
}

#[cfg(target_os = "macos")]
fn seatbelt_token_delimiter(bytes: &[u8], cursor: usize) -> bool {
    bytes[cursor].is_ascii_whitespace()
        || matches!(bytes[cursor], b'(' | b')' | b'"' | b';')
        || bytes[cursor] == b'#' && bytes.get(cursor + 1) == Some(&b'|')
}

#[cfg(target_os = "macos")]
fn skip_seatbelt_block_comment(bytes: &[u8], mut cursor: usize) -> io::Result<usize> {
    let mut depth = 1_usize;
    while cursor < bytes.len() {
        if bytes[cursor] == b'#' && bytes.get(cursor + 1) == Some(&b'|') {
            depth = depth.checked_add(1).ok_or_else(seatbelt_syntax_error)?;
            cursor += 2;
        } else if bytes[cursor] == b'|' && bytes.get(cursor + 1) == Some(&b'#') {
            depth -= 1;
            cursor += 2;
            if depth == 0 {
                return Ok(cursor);
            }
        } else {
            cursor += 1;
        }
    }
    Err(seatbelt_syntax_error())
}

#[cfg(target_os = "macos")]
fn observe_seatbelt_list_datum(list: Option<&mut SeatbeltListFrame<'_>>) -> io::Result<()> {
    let Some(list) = list else {
        return Ok(());
    };
    list.datums = list
        .datums
        .checked_add(1)
        .ok_or_else(seatbelt_syntax_error)?;
    if list.is_import {
        return Err(seatbelt_import_error());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn observe_seatbelt_atom<'text>(
    list: Option<&mut SeatbeltListFrame<'text>>,
    atom: &'text str,
) -> io::Result<()> {
    if atom.contains(['\\', '|', '[', ']', '{', '}'])
        || atom.starts_with("#!")
        || atom != "import" && atom.eq_ignore_ascii_case("import")
        || seatbelt_reader_import(atom)
        || seatbelt_reflective_import_vocabulary(atom)
    {
        return Err(seatbelt_syntax_error());
    }
    let Some(list) = list else {
        if atom == "import" {
            return Err(seatbelt_import_error());
        }
        return Ok(());
    };
    if list.datums == 0 {
        list.is_import = atom == "import";
    } else if atom == "import" || list.is_import {
        return Err(seatbelt_import_error());
    }
    list.datums = list
        .datums
        .checked_add(1)
        .ok_or_else(seatbelt_syntax_error)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn seatbelt_reader_import(atom: &str) -> bool {
    atom.strip_suffix("import").is_some_and(|prefix| {
        !prefix.is_empty()
            && prefix
                .bytes()
                .all(|byte| matches!(byte, b'\'' | b'`' | b',' | b'#' | b'@'))
    })
}

#[cfg(target_os = "macos")]
fn seatbelt_reflective_import_vocabulary(atom: &str) -> bool {
    atom.contains("eval")
        || atom.contains("symbol")
        || atom.contains("syntax")
        || matches!(atom, "read" | "load" | "define-macro")
}

#[cfg(target_os = "macos")]
fn observe_seatbelt_string<'text>(
    list: Option<&mut SeatbeltListFrame<'text>>,
    content: &'text str,
    escaped: bool,
) -> io::Result<()> {
    let Some(list) = list else {
        return Ok(());
    };
    if list.is_import && list.datums == 1 {
        if escaped
            || content.is_empty()
            || !content
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        {
            return Err(seatbelt_import_error());
        }
        list.imported = Some(content);
    } else if list.is_import {
        return Err(seatbelt_import_error());
    }
    list.datums = list
        .datums
        .checked_add(1)
        .ok_or_else(seatbelt_syntax_error)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn seatbelt_syntax_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "native resolver Seatbelt profile has invalid bounded syntax",
    )
}

#[cfg(target_os = "macos")]
fn seatbelt_import_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "native resolver Seatbelt profile has a noncanonical import",
    )
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
    Ok(format_sha256(&hasher.finalize()))
}

#[cfg(target_os = "macos")]
fn format_sha256(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::CHILD_OPEN_FILE_LIMIT;
    use super::{
        RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT, ResolverExecutionBackend,
        ResolverExecutionGuarantee, ResolverExecutionGuaranteeDisposition, ResolverExecutionPhase,
    };
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

    #[test]
    fn policy_observation_is_complete_canonical_and_locally_fail_closed() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let executable = if cfg!(windows) {
            Path::new(r"C:\Windows\System32\cmd.exe")
        } else {
            Path::new("/bin/sh")
        };
        let mutable_root = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary root");
        let (_, inspection) = backend
            .command_with_observation(
                executable,
                &[],
                ResolverExecutionPhase::RepositoryInspection,
                None,
            )
            .expect("issue inspection policy observation");
        let (_, fetch) = backend
            .command_with_observation(
                executable,
                &[],
                ResolverExecutionPhase::Fetch,
                Some(&mutable_root),
            )
            .expect("issue fetch policy observation");

        assert_eq!(inspection.guarantees().len(), 13);
        assert!(
            inspection
                .guarantees()
                .windows(2)
                .all(|rows| rows[0].guarantee() < rows[1].guarantee())
        );
        assert_eq!(
            inspection.canonical_bytes(),
            backend
                .command_with_observation(
                    executable,
                    &[],
                    ResolverExecutionPhase::RepositoryInspection,
                    None,
                )
                .expect("reissue inspection policy observation")
                .1
                .canonical_bytes()
        );
        assert_ne!(inspection.canonical_bytes(), fetch.canonical_bytes());
        assert_eq!(inspection.executable(), executable);
        assert_eq!(fetch.mutable_root(), Some(mutable_root.as_path()));
        #[cfg(unix)]
        {
            assert_eq!(inspection.resource_ceilings().core_dump_bytes(), Some(0));
            assert_eq!(inspection.resource_ceilings().cpu_seconds(), Some(120));
            assert_eq!(
                inspection.resource_ceilings().single_file_bytes(),
                Some(1024 * 1024 * 1024)
            );
            assert_eq!(inspection.resource_ceilings().open_files(), Some(256));
        }
        assert!(inspection.require_strict().is_err());
        assert!(fetch.require_strict().is_err());
        for observation in [&inspection, &fetch] {
            let unavailable = observation
                .guarantees()
                .iter()
                .find(|row| row.disposition() == ResolverExecutionGuaranteeDisposition::Unavailable)
                .expect("current backend retains at least one unavailable strict guarantee");
            let error = observation
                .require_strict()
                .expect_err("strict policy rejects");
            assert_eq!(error.guarantee(), unavailable.guarantee());
        }
    }

    #[test]
    fn policy_observation_normalizes_and_bounds_executable_sets() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let executable = if cfg!(windows) {
            Path::new(r"C:\Windows\System32\cmd.exe")
        } else {
            Path::new("/bin/sh")
        };
        let first = if cfg!(windows) {
            Path::new(r"C:\Windows\System32\where.exe").to_path_buf()
        } else {
            Path::new("/bin/bash").to_path_buf()
        };
        let second = if cfg!(windows) {
            Path::new(r"C:\Windows\System32\whoami.exe").to_path_buf()
        } else {
            Path::new("/usr/bin/git").to_path_buf()
        };
        let (_, left) = backend
            .command_with_observation(
                executable,
                &[second.clone(), first.clone(), second.clone()],
                ResolverExecutionPhase::RepositoryInspection,
                None,
            )
            .expect("construct normalized policy observation");
        let (_, right) = backend
            .command_with_observation(
                executable,
                &[first.clone(), second.clone()],
                ResolverExecutionPhase::RepositoryInspection,
                None,
            )
            .expect("reconstruct normalized policy observation");
        assert_eq!(left.canonical_bytes(), right.canonical_bytes());
        assert_eq!(left.additional_executables(), &[first, second]);

        let excessive = vec![
            if cfg!(windows) {
                Path::new(r"C:\Windows\System32\where.exe").to_path_buf()
            } else {
                Path::new("/bin/bash").to_path_buf()
            };
            RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT + 1
        ];
        assert!(
            backend
                .command_with_observation(
                    executable,
                    &excessive,
                    ResolverExecutionPhase::RepositoryInspection,
                    None,
                )
                .is_err()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_observation_reports_exact_known_enforcement_and_gaps() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let executable = Path::new("/bin/sh");
        let mutable_root = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary root");
        let (inspection_command, inspection) = backend
            .command_with_observation(
                executable,
                &[],
                ResolverExecutionPhase::RepositoryInspection,
                None,
            )
            .expect("issue inspection policy observation");
        let (initialization_command, initialization) = backend
            .command_with_observation(
                executable,
                &[],
                ResolverExecutionPhase::RepositoryInitialization,
                Some(&mutable_root),
            )
            .expect("issue initialization policy observation");
        let (discovery_command, discovery) = backend
            .command_with_observation(
                executable,
                &[],
                ResolverExecutionPhase::TransportDiscovery,
                None,
            )
            .expect("issue discovery policy observation");
        let (fetch_command, fetch) = backend
            .command_with_observation(
                executable,
                &[],
                ResolverExecutionPhase::Fetch,
                Some(&mutable_root),
            )
            .expect("issue fetch policy observation");
        assert!(inspection.generated_policy_sha256().is_some());
        assert!(initialization.generated_policy_sha256().is_some());
        assert!(discovery.generated_policy_sha256().is_some());
        assert!(fetch.generated_policy_sha256().is_some());
        assert_ne!(
            inspection.generated_policy_sha256(),
            fetch.generated_policy_sha256()
        );

        let profile = |command: &std::process::Command| {
            let arguments = command.get_args().collect::<Vec<_>>();
            let profile_index = arguments
                .iter()
                .position(|argument| *argument == "-p")
                .expect("Seatbelt command carries an inline policy")
                + 1;
            arguments[profile_index]
                .to_str()
                .expect("compiler-generated policy is UTF-8")
                .to_owned()
        };
        let inspection_profile = profile(&inspection_command);
        assert!(!inspection_profile.contains("(import"));
        assert!(!inspection_profile.contains("network-outbound"));
        assert!(!inspection_profile.contains("file-write*"));
        assert!(
            inspection_profile
                .contains("(allow file-test-existence file-write-data (literal \"/dev/null\"))")
        );
        let initialization_profile = profile(&initialization_command);
        assert!(!initialization_profile.contains("(import"));
        assert!(!initialization_profile.contains("network-outbound"));
        assert!(
            initialization_profile
                .contains("(allow file-write* (subpath (param \"MUTABLE_ROOT\")))")
        );
        assert!(
            initialization_profile
                .contains("(allow file-test-existence file-write-data (literal \"/dev/null\"))")
        );
        let discovery_profile = profile(&discovery_command);
        assert!(!discovery_profile.contains("(import"));
        assert!(discovery_profile.contains("network-outbound"));
        assert!(!discovery_profile.contains("file-write*"));
        assert!(discovery_profile.contains(
            "(allow mach-lookup (global-name \"com.apple.system.opendirectoryd.libinfo\"))"
        ));
        assert!(discovery_profile.contains("(allow sysctl-read (sysctl-name \"kern.hostname\"))"));
        assert!(!discovery_profile.contains("(allow sysctl-read)"));
        assert!(profile(&fetch_command).contains("(import \"system.sb\")"));

        let disposition = |observation: &super::ResolverExecutionPolicyObservation, guarantee| {
            observation
                .guarantees()
                .iter()
                .find(|row| row.guarantee() == guarantee)
                .expect("complete guarantee row set")
                .disposition()
        };
        assert_eq!(
            disposition(
                &inspection,
                ResolverExecutionGuarantee::FilesystemWritesConfined
            ),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        for guarantee in [
            ResolverExecutionGuarantee::FilesystemWritesConfined,
            ResolverExecutionGuarantee::NetworkDenied,
            ResolverExecutionGuarantee::ExecutablePathsConfined,
        ] {
            assert_eq!(
                disposition(&initialization, guarantee),
                ResolverExecutionGuaranteeDisposition::Enforced
            );
        }
        assert_eq!(
            disposition(
                &initialization,
                ResolverExecutionGuarantee::FilesystemReadsConfined
            ),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
        assert_eq!(
            disposition(
                &discovery,
                ResolverExecutionGuarantee::FilesystemWritesConfined
            ),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(
                &discovery,
                ResolverExecutionGuarantee::ExecutablePathsConfined
            ),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(&discovery, ResolverExecutionGuarantee::NetworkDenied),
            ResolverExecutionGuaranteeDisposition::NotRequired
        );
        assert_eq!(
            disposition(
                &discovery,
                ResolverExecutionGuarantee::NetworkEndpointsConfined
            ),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
        assert_eq!(
            disposition(
                &inspection,
                ResolverExecutionGuarantee::FilesystemReadsConfined
            ),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
        assert_eq!(
            disposition(&inspection, ResolverExecutionGuarantee::NetworkDenied),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(
                &inspection,
                ResolverExecutionGuarantee::ExecutablePathsConfined
            ),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::NetworkDenied),
            ResolverExecutionGuaranteeDisposition::NotRequired
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::NetworkEndpointsConfined),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::ExecutablePathsConfined),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::CoreDumpsDenied),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::CpuTimeConfined),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::SingleFileSizeConfined),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::OpenFilesConfined),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::AddressSpaceConfined),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_direct_import_subset_is_fail_closed_and_profile_content_is_bound() {
        use super::{
            ResolverExecutionBackendIdentity, seatbelt_imports, validate_seatbelt_import_topology,
        };

        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let ResolverExecutionBackendIdentity::MacosSeatbelt {
            system_profile_sha256,
            dyld_profile_sha256,
            ..
        } = backend.identity()
        else {
            panic!("macOS must select Seatbelt");
        };
        assert_eq!(system_profile_sha256.len(), 64);
        assert_eq!(dyld_profile_sha256.len(), 64);
        assert!(
            validate_seatbelt_import_topology(
                b"(version 3)\n(import\n  \"dyld-support.sb\"\n)\n",
                b"(version 3)\n"
            )
            .is_ok()
        );
        assert_eq!(
            seatbelt_imports(
                br#"; (import "line-comment.sb")
                    #| (import "block-comment.sb")
                       #| (import "nested-comment.sb") |#
                    |#
                    (define spelling "(import \"string.sb\")")
                    ( import "dyld-support.sb" )"#
            )
            .expect("scan complete Seatbelt syntax"),
            ["dyld-support.sb"]
        );
        assert!(
            validate_seatbelt_import_topology(
                b"(version 3)\n(import \"dyld-support.sb\")\n(import \"extra.sb\")\n",
                b"(version 3)\n"
            )
            .is_err()
        );
        assert!(
            validate_seatbelt_import_topology(
                b"(version 3)\n(import \"dyld-support.sb\")\n",
                b"(import \"nested.sb\")\n"
            )
            .is_err()
        );
        for malformed in [
            b"(import \"dyld\\-support.sb\")".as_slice(),
            b"(import (identity \"dyld-support.sb\"))".as_slice(),
            b"(import \"dyld-support.sb\" \"extra.sb\")".as_slice(),
            b"(import \"dyld-support.sb\"".as_slice(),
            b"(|import| \"dyld-support.sb\")".as_slice(),
            b"(IMPORT \"dyld-support.sb\")".as_slice(),
            b"#!fold-case\n(IMPORT \"dyld-support.sb\")".as_slice(),
            b"(#; ignored\n import \"dyld-support.sb\")".as_slice(),
            b"[import \"dyld-support.sb\"]".as_slice(),
            b"(apply import (list \"extra.sb\"))".as_slice(),
            b"(define load-profile import)".as_slice(),
            b"(map import (list \"extra.sb\"))".as_slice(),
            b"(eval (list import \"extra.sb\"))".as_slice(),
            b"(eval (list (string->symbol \"import\") \"extra.sb\"))".as_slice(),
            b"(eval (list 'import \"extra.sb\"))".as_slice(),
            b"#| unterminated".as_slice(),
            b")".as_slice(),
        ] {
            assert!(seatbelt_imports(malformed).is_err());
        }
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
    fn seatbelt_denies_an_ordinary_write_outside_the_mutable_root() {
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
    fn seatbelt_inspection_allows_only_the_fixed_null_write_sink() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "omega-resolver-inspection-write-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&root).expect("create inspection canary root");
        let root = root.canonicalize().expect("canonicalize inspection root");
        let marker = root.join("marker");
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let helper_executables = [Path::new("/bin/bash").to_path_buf()];

        let mut allowed = backend
            .command(
                Path::new("/bin/sh"),
                &helper_executables,
                ResolverExecutionPhase::RepositoryInspection,
                None,
            )
            .expect("build inspection sandbox");
        let status = allowed
            .args(["-c", "printf allowed > /dev/null"])
            .status()
            .expect("write the fixed null sink");
        assert!(status.success());

        let mut denied = backend
            .command(
                Path::new("/bin/sh"),
                &helper_executables,
                ResolverExecutionPhase::RepositoryInspection,
                None,
            )
            .expect("build inspection sandbox");
        let status = denied
            .args(["-c", "printf denied > \"$1\"", "resolver-test"])
            .arg(&marker)
            .status()
            .expect("attempt ordinary inspection write");
        assert!(!status.success());
        assert!(!marker.exists());
        std::fs::remove_dir(root).expect("remove inspection canary root");
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
    fn seatbelt_denies_remote_tcp_in_nonnetwork_phases() {
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

        let mutable_root = std::env::temp_dir().join(format!(
            "omega-resolver-network-denial-{}-{port}",
            std::process::id(),
        ));
        std::fs::create_dir(&mutable_root).expect("create network-denial mutable root");
        let mutable_root = mutable_root
            .canonicalize()
            .expect("canonicalize network-denial mutable root");
        let mut denied = backend
            .command(
                Path::new("/usr/bin/nc"),
                &[],
                ResolverExecutionPhase::RepositoryInitialization,
                Some(&mutable_root),
            )
            .expect("build initialization network-denied sandbox");
        let status = denied
            .args(["127.0.0.1", &port.to_string()])
            .stdin(Stdio::null())
            .status()
            .expect("attempt denied initialization loopback connection");
        assert!(!status.success());
        assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));
        std::fs::remove_dir(mutable_root).expect("remove network-denial mutable root");

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
