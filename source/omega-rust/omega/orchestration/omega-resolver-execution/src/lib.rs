//! Native process enforcement for compiler-owned package-source resolution.
//!
//! This crate owns the platform-specific launch mechanism. Callers choose one
//! closed resolver phase and provide compiler-selected executable and custody
//! paths; they cannot author sandbox policy text or containment claims.

#![deny(unsafe_op_in_unsafe_fn)]

mod network;

pub use network::{
    RESOLVER_CONNECT_BROKER_ENVIRONMENT, RESOLVER_CONNECT_HELPER_BASENAME,
    RESOLVER_CONNECT_TARGET_ENVIRONMENT, ResolverExecutionEndpointEvent,
    ResolverExecutionEndpointHost, ResolverExecutionEndpointObservation,
    ResolverExecutionEndpointOutcome, ResolverExecutionEndpointRoute,
    ResolverExecutionEndpointRoutePolicy, ResolverExecutionRequestedEndpoint,
    ResolverExecutionTransferBudget, run_resolver_connect_helper,
};

#[cfg(target_os = "macos")]
use sha2::{Digest, Sha256};
#[cfg(target_os = "macos")]
use std::collections::BTreeSet;
#[cfg(target_os = "macos")]
use std::ffi::OsString;
#[cfg(target_os = "macos")]
use std::fs::File;
use std::io;
#[cfg(target_os = "macos")]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

const RESOLVER_EXECUTION_OBSERVATION_SCHEMA_VERSION: u32 = 11;
const RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT: usize = 32;
const RESOLVER_EXECUTION_PATH_BYTE_LIMIT: usize = 32 * 1024;
const RESOLVER_EXECUTION_CANONICAL_BYTE_LIMIT: usize = 2 * 1024 * 1024;

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
#[cfg(target_os = "macos")]
const MACOS_NULL_DEVICE: &str = "/dev/null";
#[cfg(target_os = "macos")]
const MACOS_DIRECTORY_LOOKUP_SERVICE: &str = "com.apple.system.opendirectoryd.libinfo";
#[cfg(target_os = "macos")]
const MACOS_HOSTNAME_SYSCTL: &str = "kern.hostname";
#[cfg(target_os = "macos")]
const MACOS_RUST_RUNTIME_PAGE_SIZE_SYSCTL: &str = "hw.pagesize_compat";
#[cfg(target_os = "macos")]
const MACOS_TLS_CONFIGURATION_ROOT: &str = "/private/etc/ssl";
#[cfg(target_os = "macos")]
const MACOS_TLS_CONFIGURATION_ALIAS_ROOT: &str = "/etc/ssl";
#[cfg(target_os = "macos")]
const MACOS_CONFINED_METADATA_PATH_LIMIT: usize = 1024;

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

    const fn permits_descendant_processes(self) -> bool {
        self.permits_network()
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

/// Closed transport authority selected for a networked resolver phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionNetworkTransport {
    Https,
    Ssh,
}

impl ResolverExecutionNetworkTransport {
    const fn tag(self) -> u8 {
        match self {
            Self::Https => 1,
            Self::Ssh => 2,
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
    network_transport: Option<ResolverExecutionNetworkTransport>,
    endpoint_route: Option<ResolverExecutionEndpointRoutePolicy>,
    generated_policy_sha256: Option<String>,
    resource_ceilings: ResolverExecutionResourceCeilings,
    executable: PathBuf,
    additional_executables: Vec<PathBuf>,
    discovery_read_root: Option<PathBuf>,
    inspection_read_root: Option<PathBuf>,
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

    pub const fn network_transport(&self) -> Option<ResolverExecutionNetworkTransport> {
        self.network_transport
    }

    pub const fn endpoint_route(&self) -> Option<&ResolverExecutionEndpointRoutePolicy> {
        self.endpoint_route.as_ref()
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

    pub fn discovery_read_root(&self) -> Option<&Path> {
        self.discovery_read_root.as_deref()
    }

    pub fn inspection_read_root(&self) -> Option<&Path> {
        self.inspection_read_root.as_deref()
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
        match self.network_transport {
            Some(transport) => {
                bytes.push(1);
                bytes.push(transport.tag());
            }
            None => bytes.push(0),
        }
        match &self.endpoint_route {
            Some(route) => {
                bytes.push(1);
                route.encode(&mut bytes);
            }
            None => bytes.push(0),
        }
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
        match &self.discovery_read_root {
            Some(root) => {
                bytes.push(1);
                encode_path(&mut bytes, root);
            }
            None => bytes.push(0),
        }
        match &self.inspection_read_root {
            Some(root) => {
                bytes.push(1);
                encode_path(&mut bytes, root);
            }
            None => bytes.push(0),
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

struct ResolverExecutionPolicyInputs<'a> {
    phase: ResolverExecutionPhase,
    network_transport: Option<ResolverExecutionNetworkTransport>,
    endpoint_route: Option<&'a ResolverExecutionEndpointRoutePolicy>,
    generated_policy_sha256: Option<String>,
    executable: &'a Path,
    additional_executables: &'a [PathBuf],
    discovery_read_root: Option<&'a Path>,
    inspection_read_root: Option<&'a Path>,
    mutable_root: Option<&'a Path>,
}

#[derive(Clone, Copy)]
struct ResolverExecutionAuthorityRoots<'a> {
    discovery_read_root: Option<&'a Path>,
    inspection_read_root: Option<&'a Path>,
    mutable_root: Option<&'a Path>,
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
                    "macOS resolver sandbox boundary changed while opening",
                ));
            }
            Ok(Self {
                identity: ResolverExecutionBackendIdentity::MacosSeatbelt {
                    executable: path,
                    content_sha256,
                },
                sandbox_metadata,
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

    /// Open a compiler-owned loopback broker for one already-validated remote
    /// destination. This does not establish transport trust or acceptance.
    pub fn open_endpoint_route(
        &self,
        requested_endpoint: ResolverExecutionRequestedEndpoint,
        transfer_budget: ResolverExecutionTransferBudget,
    ) -> io::Result<ResolverExecutionEndpointRoute> {
        self.verify()?;
        ResolverExecutionEndpointRoute::open(requested_endpoint, transfer_budget)
    }

    fn policy_observation(
        &self,
        inputs: ResolverExecutionPolicyInputs<'_>,
    ) -> io::Result<ResolverExecutionPolicyObservation> {
        self.verify()?;
        let guarantees =
            ResolverExecutionGuarantee::ALL.map(|guarantee| ResolverExecutionGuaranteeRow {
                guarantee,
                disposition: guarantee_disposition(
                    &self.identity,
                    inputs.phase,
                    inputs.network_transport,
                    inputs.endpoint_route.is_some(),
                    guarantee,
                ),
            });
        Ok(ResolverExecutionPolicyObservation {
            backend: self.identity.clone(),
            phase: inputs.phase,
            network_transport: inputs.network_transport,
            endpoint_route: inputs.endpoint_route.cloned(),
            generated_policy_sha256: inputs.generated_policy_sha256,
            resource_ceilings: configured_resource_ceilings(),
            executable: inputs.executable.to_path_buf(),
            additional_executables: inputs.additional_executables.to_vec(),
            discovery_read_root: inputs.discovery_read_root.map(Path::to_path_buf),
            inspection_read_root: inputs.inspection_read_root.map(Path::to_path_buf),
            mutable_root: inputs.mutable_root.map(Path::to_path_buf),
            guarantees,
        })
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
    /// already-verified primary executable may launch. `inspection_read_root`
    /// is required exactly for repository inspection. `mutable_root` is
    /// required exactly for the two mutating phases and rejected otherwise.
    #[cfg(test)]
    fn command(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<Command> {
        let network_transport = phase
            .permits_network()
            .then_some(ResolverExecutionNetworkTransport::Ssh);
        self.command_with_observation(
            executable,
            additional_executables,
            phase,
            network_transport,
            mutable_root,
        )
        .map(|(command, _observation)| command)
    }

    #[cfg(test)]
    fn command_with_inspection_read_root(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        inspection_read_root: &Path,
    ) -> io::Result<Command> {
        let (mut command, _observation) = self.command_with_inspection_read_root_observation(
            executable,
            additional_executables,
            inspection_read_root,
        )?;
        command.current_dir(inspection_read_root);
        Ok(command)
    }

    /// Construct one command and its opaque policy observation from the same
    /// validated inputs. The observation does not state that the command ran.
    pub fn command_with_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        mutable_root: Option<&Path>,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.command_with_endpoint_route_observation(
            executable,
            additional_executables,
            phase,
            network_transport,
            None,
            mutable_root,
        )
    }

    /// Construct one repository-inspection command bound to the exact retained
    /// repository whose file contents may be read.
    pub fn command_with_inspection_read_root_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        inspection_read_root: &Path,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.command_with_authority_roots_observation(
            executable,
            additional_executables,
            ResolverExecutionPhase::RepositoryInspection,
            None,
            None,
            ResolverExecutionAuthorityRoots {
                discovery_read_root: None,
                inspection_read_root: Some(inspection_read_root),
                mutable_root: None,
            },
        )
    }

    /// Construct one transport-discovery command bound to the exact working
    /// root whose file contents may be read by a narrowed transport policy.
    pub fn command_with_discovery_route_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        network_transport: ResolverExecutionNetworkTransport,
        endpoint_route: &ResolverExecutionEndpointRoute,
        discovery_read_root: &Path,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.command_with_authority_roots_observation(
            executable,
            additional_executables,
            ResolverExecutionPhase::TransportDiscovery,
            Some(network_transport),
            Some(endpoint_route),
            ResolverExecutionAuthorityRoots {
                discovery_read_root: Some(discovery_read_root),
                inspection_read_root: None,
                mutable_root: None,
            },
        )
    }

    /// Construct one command and bind its native policy to an endpoint route.
    /// Network phases require a route; nonnetwork phases reject one. Finish the
    /// route after execution to obtain its separate endpoint observation.
    pub fn command_with_endpoint_route_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        endpoint_route: Option<&ResolverExecutionEndpointRoute>,
        mutable_root: Option<&Path>,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.command_with_authority_roots_observation(
            executable,
            additional_executables,
            phase,
            network_transport,
            endpoint_route,
            ResolverExecutionAuthorityRoots {
                discovery_read_root: None,
                inspection_read_root: None,
                mutable_root,
            },
        )
    }

    fn command_with_authority_roots_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        endpoint_route: Option<&ResolverExecutionEndpointRoute>,
        roots: ResolverExecutionAuthorityRoots<'_>,
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
        match (phase.permits_network(), network_transport, endpoint_route) {
            (true, Some(_), Some(_)) | (false, None, None) => {}
            (true, None, _) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "networked resolver phase has no closed transport authority",
                ));
            }
            (true, Some(_), None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "networked resolver phase has no endpoint route",
                ));
            }
            (false, Some(_), _) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "nonnetwork resolver phase received transport authority",
                ));
            }
            (false, None, Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "nonnetwork resolver phase received an endpoint route",
                ));
            }
        }
        match (phase.requires_mutable_root(), roots.mutable_root) {
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
        match (phase, roots.inspection_read_root) {
            (ResolverExecutionPhase::RepositoryInspection, Some(root)) => {
                require_absolute(root, "resolver inspection read root")?;
                require_canonical_bounded_path(root, "resolver inspection read root")?;
            }
            (ResolverExecutionPhase::RepositoryInspection, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "repository inspection has no read root",
                ));
            }
            (_, Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "noninspection resolver phase received an inspection read root",
                ));
            }
            (_, None) => {}
        }
        match (phase, roots.discovery_read_root) {
            (ResolverExecutionPhase::TransportDiscovery, Some(root)) => {
                require_absolute(root, "resolver discovery read root")?;
                require_canonical_bounded_path(root, "resolver discovery read root")?;
            }
            (ResolverExecutionPhase::TransportDiscovery, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "transport discovery has no read root",
                ));
            }
            (_, Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "nondiscovery resolver phase received a discovery read root",
                ));
            }
            (_, None) => {}
        }

        #[cfg(target_os = "macos")]
        let (mut command, generated_policy_sha256) = self.macos_command(
            executable,
            &additional_executables,
            phase,
            network_transport,
            endpoint_route.map(ResolverExecutionEndpointRoute::policy),
            roots,
        )?;
        #[cfg(not(target_os = "macos"))]
        let mut command = Command::new(executable);
        #[cfg(not(target_os = "macos"))]
        let generated_policy_sha256 = None;

        configure_child_resource_limits(&mut command)?;
        let observation = self.policy_observation(ResolverExecutionPolicyInputs {
            phase,
            network_transport,
            endpoint_route: endpoint_route.map(ResolverExecutionEndpointRoute::policy),
            generated_policy_sha256,
            executable,
            additional_executables: &additional_executables,
            discovery_read_root: roots.discovery_read_root,
            inspection_read_root: roots.inspection_read_root,
            mutable_root: roots.mutable_root,
        })?;
        Ok((command, observation))
    }

    #[cfg(target_os = "macos")]
    fn macos_command(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        endpoint_route: Option<&ResolverExecutionEndpointRoutePolicy>,
        roots: ResolverExecutionAuthorityRoots<'_>,
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
        let mut profile = "(version 1) (deny default) ".to_owned();
        if phase.permits_descendant_processes() {
            profile.push_str("(allow process-fork) ");
        }
        profile.push_str("(allow signal) ");
        let confines_content_reads = matches!(
            phase,
            ResolverExecutionPhase::RepositoryInitialization
                | ResolverExecutionPhase::RepositoryInspection
        ) || (phase == ResolverExecutionPhase::Fetch
            && network_transport == Some(ResolverExecutionNetworkTransport::Https))
            || (phase == ResolverExecutionPhase::TransportDiscovery
                && network_transport == Some(ResolverExecutionNetworkTransport::Https));
        let confined_metadata = match (phase, network_transport) {
            (ResolverExecutionPhase::RepositoryInitialization, _) => {
                let mutable_root = roots.mutable_root.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "repository initialization requires its compiler-owned mutable root",
                    )
                })?;
                Some((
                    "MUTABLE_ROOT",
                    false,
                    Vec::new(),
                    macos_confined_metadata_paths(
                        executable,
                        additional_executables,
                        &[mutable_root],
                    )?,
                ))
            }
            (ResolverExecutionPhase::RepositoryInspection, _) => {
                let inspection_read_root = roots.inspection_read_root.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "repository inspection requires its compiler-owned read root",
                    )
                })?;
                Some((
                    "INSPECTION_READ_ROOT",
                    false,
                    Vec::new(),
                    macos_confined_metadata_paths(
                        executable,
                        additional_executables,
                        &[inspection_read_root],
                    )?,
                ))
            }
            (
                ResolverExecutionPhase::TransportDiscovery,
                Some(ResolverExecutionNetworkTransport::Https),
            ) => {
                let discovery_read_root = roots.discovery_read_root.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "HTTPS transport discovery requires its compiler-owned read root",
                    )
                })?;
                Some((
                    "DISCOVERY_READ_ROOT",
                    true,
                    macos_helper_metadata_roots(additional_executables)?,
                    macos_confined_metadata_paths(
                        executable,
                        additional_executables,
                        &[
                            discovery_read_root,
                            Path::new(MACOS_TLS_CONFIGURATION_ROOT),
                            Path::new(MACOS_TLS_CONFIGURATION_ALIAS_ROOT),
                        ],
                    )?,
                ))
            }
            (ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch, _) => None,
        };
        if confines_content_reads {
            let read_root_parameter = match phase {
                ResolverExecutionPhase::TransportDiscovery => "DISCOVERY_READ_ROOT",
                ResolverExecutionPhase::RepositoryInspection => "INSPECTION_READ_ROOT",
                ResolverExecutionPhase::RepositoryInitialization
                | ResolverExecutionPhase::Fetch => "MUTABLE_ROOT",
            };
            if let Some((
                metadata_root_parameter,
                includes_tls_root,
                metadata_subpaths,
                metadata_paths,
            )) = &confined_metadata
            {
                profile
                    .push_str("(allow file-read-metadata file-test-existence (subpath (param \"");
                profile.push_str(metadata_root_parameter);
                profile.push_str("\"))");
                if *includes_tls_root {
                    profile.push_str(&format!(
                        " (subpath \"{MACOS_TLS_CONFIGURATION_ROOT}\") \
                         (subpath \"{MACOS_TLS_CONFIGURATION_ALIAS_ROOT}\")"
                    ));
                }
                for index in 0..metadata_subpaths.len() {
                    profile.push_str(&format!(" (subpath (param \"METADATA_SUBPATH_{index}\"))"));
                }
                for index in 0..metadata_paths.len() {
                    profile.push_str(&format!(" (literal (param \"METADATA_PATH_{index}\"))"));
                }
                profile.push_str(") ");
            } else {
                profile.push_str("(allow file-read-metadata) ");
            }
            profile.push_str("(allow file-read-data (subpath (param \"");
            profile.push_str(read_root_parameter);
            profile.push_str("\")) (literal (param \"EXECUTABLE_0\"))");
            for index in 0..additional_executables.len() {
                profile.push_str(&format!(" (literal (param \"EXECUTABLE_{}\"))", index + 1));
            }
            if matches!(
                phase,
                ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
            ) && network_transport == Some(ResolverExecutionNetworkTransport::Https)
            {
                profile.push_str(&format!(" (subpath \"{MACOS_TLS_CONFIGURATION_ROOT}\")"));
            }
            profile.push_str(&format!(
                " (literal \"{}\") (literal \"{MACOS_NULL_DEVICE}\")) ",
                std::path::MAIN_SEPARATOR
            ));
        } else {
            profile.push_str("(allow file-read*) ");
        }
        profile.push_str(&format!(
            "(allow file-test-existence file-write-data (literal \"{MACOS_NULL_DEVICE}\")) \
             (allow process-exec (literal (param \"EXECUTABLE_0\"))"
        ));
        for index in 0..additional_executables.len() {
            profile.push_str(&format!(" (literal (param \"EXECUTABLE_{}\"))", index + 1));
        }
        profile.push(')');
        if endpoint_route.is_some() {
            profile.push_str(" (allow network-outbound (remote tcp (param \"BROKER_ENDPOINT\")))");
        }
        if network_transport == Some(ResolverExecutionNetworkTransport::Ssh) {
            profile.push_str(&format!(
                " (allow mach-lookup (global-name \"{MACOS_DIRECTORY_LOOKUP_SERVICE}\")) \
                 (allow sysctl-read (sysctl-name \"{MACOS_HOSTNAME_SYSCTL}\")) \
                 (allow sysctl-read (sysctl-name \"{MACOS_RUST_RUNTIME_PAGE_SIZE_SYSCTL}\"))"
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
        if let Some(root) = roots.mutable_root {
            command
                .arg("-D")
                .arg(definition_argument("MUTABLE_ROOT", root));
        }
        if let Some(root) = roots.inspection_read_root {
            command
                .arg("-D")
                .arg(definition_argument("INSPECTION_READ_ROOT", root));
        }
        if let Some(root) = roots.discovery_read_root {
            command
                .arg("-D")
                .arg(definition_argument("DISCOVERY_READ_ROOT", root));
        }
        if let Some((_, _, metadata_subpaths, metadata_paths)) = &confined_metadata {
            for (index, path) in metadata_subpaths.iter().enumerate() {
                command.arg("-D").arg(definition_argument(
                    &format!("METADATA_SUBPATH_{index}"),
                    path,
                ));
            }
            for (index, path) in metadata_paths.iter().enumerate() {
                command
                    .arg("-D")
                    .arg(definition_argument(&format!("METADATA_PATH_{index}"), path));
            }
        }
        if let Some(route) = endpoint_route {
            command.arg("-D").arg(format!(
                "BROKER_ENDPOINT=localhost:{}",
                route.broker_endpoint().port()
            ));
        }
        let profile_sha256 = format_sha256(Sha256::digest(profile.as_bytes()).as_slice());
        command.arg("-p").arg(profile).arg(executable);
        Ok((command, Some(profile_sha256)))
    }
}

#[cfg(target_os = "macos")]
fn macos_helper_metadata_roots(additional_executables: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut roots = BTreeSet::new();
    for executable in additional_executables {
        let parent = executable.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver helper executable has no metadata root",
            )
        })?;
        if parent == Path::new(std::path::MAIN_SEPARATOR_STR) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver helper metadata root cannot be the filesystem root",
            ));
        }
        roots.insert(parent.to_path_buf());
    }
    bounded_macos_metadata_paths(roots, "resolver helper metadata roots")
}

#[cfg(target_os = "macos")]
fn macos_confined_metadata_paths(
    executable: &Path,
    additional_executables: &[PathBuf],
    confined_read_roots: &[&Path],
) -> io::Result<Vec<PathBuf>> {
    let mut paths = BTreeSet::new();
    for path in confined_read_roots
        .iter()
        .copied()
        .chain(std::iter::once(executable))
        .chain(additional_executables.iter().map(PathBuf::as_path))
        .chain(std::iter::once(Path::new(MACOS_NULL_DEVICE)))
    {
        for ancestor in path.ancestors() {
            paths.insert(ancestor.to_path_buf());
        }
    }
    bounded_macos_metadata_paths(paths, "resolver confined metadata paths")
}

#[cfg(target_os = "macos")]
fn bounded_macos_metadata_paths(paths: BTreeSet<PathBuf>, name: &str) -> io::Result<Vec<PathBuf>> {
    if paths.len() > MACOS_CONFINED_METADATA_PATH_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} exceed compiler count limit"),
        ));
    }
    let encoded_bytes = paths.iter().try_fold(0_usize, |total, path| {
        total.checked_add(path.as_os_str().as_encoded_bytes().len())
    });
    if !matches!(encoded_bytes, Some(total) if total <= RESOLVER_EXECUTION_CANONICAL_BYTE_LIMIT) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} exceed compiler byte limit"),
        ));
    }
    Ok(paths.into_iter().collect())
}

fn guarantee_disposition(
    backend: &ResolverExecutionBackendIdentity,
    phase: ResolverExecutionPhase,
    network_transport: Option<ResolverExecutionNetworkTransport>,
    has_endpoint_route: bool,
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
            if matches!(backend, MacosSeatbelt { .. }) =>
        {
            Enforced
        }
        FilesystemWritesConfined | ExecutablePathsConfined => Unavailable,
        FilesystemReadsConfined
            if matches!(backend, MacosSeatbelt { .. })
                && (matches!(
                    phase,
                    ResolverExecutionPhase::RepositoryInitialization
                        | ResolverExecutionPhase::RepositoryInspection
                ) || (phase == ResolverExecutionPhase::TransportDiscovery
                    && network_transport == Some(ResolverExecutionNetworkTransport::Https))) =>
        {
            Enforced
        }
        FilesystemReadsConfined | ProcessCountConfined | AggregateResourcesConfined => Unavailable,
        DescendantProcessesContained
            if matches!(backend, MacosSeatbelt { .. }) && !phase.permits_descendant_processes() =>
        {
            Enforced
        }
        DescendantProcessesContained => Unavailable,
        NetworkDenied if matches!(backend, MacosSeatbelt { .. }) && !phase.permits_network() => {
            Enforced
        }
        NetworkDenied if phase.permits_network() => NotRequired,
        NetworkDenied => Unavailable,
        NetworkEndpointsConfined if !phase.permits_network() => NotRequired,
        NetworkEndpointsConfined
            if matches!(backend, MacosSeatbelt { .. }) && has_endpoint_route =>
        {
            Enforced
        }
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
        } => {
            bytes.push(1);
            encode_path(bytes, executable);
            encode_bytes(bytes, content_sha256.as_bytes());
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
    #[cfg(target_os = "macos")]
    use super::{
        MACOS_CONFINED_METADATA_PATH_LIMIT, MACOS_TLS_CONFIGURATION_ALIAS_ROOT,
        MACOS_TLS_CONFIGURATION_ROOT, macos_confined_metadata_paths, macos_helper_metadata_roots,
    };
    use super::{
        RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT, ResolverExecutionAuthorityRoots,
        ResolverExecutionBackend, ResolverExecutionEndpointRoute, ResolverExecutionGuarantee,
        ResolverExecutionGuaranteeDisposition, ResolverExecutionNetworkTransport,
        ResolverExecutionPhase, ResolverExecutionRequestedEndpoint,
        ResolverExecutionTransferBudget,
    };
    use std::path::Path;
    #[cfg(target_os = "macos")]
    use std::path::PathBuf;
    #[cfg(target_os = "macos")]
    use std::process::Stdio;

    fn loopback_route(backend: &ResolverExecutionBackend) -> ResolverExecutionEndpointRoute {
        backend
            .open_endpoint_route(
                ResolverExecutionRequestedEndpoint::new("127.0.0.1", 9)
                    .expect("construct loopback endpoint"),
                ResolverExecutionTransferBudget::new(1024 * 1024)
                    .expect("construct transfer budget"),
            )
            .expect("open loopback endpoint route")
    }

    fn inspection_root() -> std::path::PathBuf {
        std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary inspection root")
    }

    #[test]
    fn mutability_is_derived_from_the_closed_phase() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let executable = if cfg!(windows) {
            Path::new(r"C:\Windows\System32\cmd.exe")
        } else {
            Path::new("/bin/sh")
        };
        let inspection_root = inspection_root();
        assert!(
            backend
                .command_with_authority_roots_observation(
                    executable,
                    &[],
                    ResolverExecutionPhase::RepositoryInspection,
                    None,
                    None,
                    ResolverExecutionAuthorityRoots {
                        discovery_read_root: None,
                        inspection_read_root: Some(&inspection_root),
                        mutable_root: Some(Path::new("/tmp")),
                    },
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
    fn endpoint_routes_are_required_exactly_for_network_phases() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let executable = if cfg!(windows) {
            Path::new(r"C:\Windows\System32\cmd.exe")
        } else {
            Path::new("/bin/sh")
        };
        let mutable_root = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary root");
        let route = loopback_route(&backend);
        let inspection_root = inspection_root();

        let discovery = backend
            .command_with_discovery_route_observation(
                executable,
                &[],
                ResolverExecutionNetworkTransport::Https,
                &route,
                &mutable_root,
            )
            .expect("construct discovery policy")
            .1;
        assert_eq!(
            discovery.discovery_read_root(),
            Some(mutable_root.as_path())
        );
        let alternate_discovery_root = mutable_root.join("alternate");
        let alternate_discovery = backend
            .command_with_discovery_route_observation(
                executable,
                &[],
                ResolverExecutionNetworkTransport::Https,
                &route,
                &alternate_discovery_root,
            )
            .expect("construct alternate discovery policy")
            .1;
        assert_ne!(
            discovery.canonical_bytes(),
            alternate_discovery.canonical_bytes()
        );
        assert!(
            backend
                .command_with_discovery_route_observation(
                    executable,
                    &[],
                    ResolverExecutionNetworkTransport::Https,
                    &route,
                    Path::new("relative"),
                )
                .is_err()
        );
        assert!(
            backend
                .command_with_endpoint_route_observation(
                    executable,
                    &[],
                    ResolverExecutionPhase::Fetch,
                    Some(ResolverExecutionNetworkTransport::Https),
                    Some(&route),
                    Some(&mutable_root),
                )
                .is_ok()
        );
        for (phase, mutable_root) in [
            (ResolverExecutionPhase::TransportDiscovery, None),
            (ResolverExecutionPhase::Fetch, Some(mutable_root.as_path())),
        ] {
            assert!(
                backend
                    .command_with_observation(
                        executable,
                        &[],
                        phase,
                        Some(ResolverExecutionNetworkTransport::Https),
                        mutable_root,
                    )
                    .is_err()
            );
        }
        for (phase, mutable_root) in [
            (ResolverExecutionPhase::RepositoryInspection, None),
            (
                ResolverExecutionPhase::RepositoryInitialization,
                Some(mutable_root.as_path()),
            ),
        ] {
            assert!(
                backend
                    .command_with_authority_roots_observation(
                        executable,
                        &[],
                        phase,
                        None,
                        Some(&route),
                        ResolverExecutionAuthorityRoots {
                            discovery_read_root: None,
                            inspection_read_root: (phase
                                == ResolverExecutionPhase::RepositoryInspection)
                                .then_some(inspection_root.as_path()),
                            mutable_root,
                        },
                    )
                    .is_err()
            );
        }
    }

    #[test]
    fn inspection_read_roots_are_required_exactly_for_inspection() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let executable = if cfg!(windows) {
            Path::new(r"C:\Windows\System32\cmd.exe")
        } else {
            Path::new("/bin/sh")
        };
        let inspection_root = inspection_root();

        assert!(
            backend
                .command_with_observation(
                    executable,
                    &[],
                    ResolverExecutionPhase::RepositoryInspection,
                    None,
                    None,
                )
                .is_err(),
            "inspection requires an explicit content-read root"
        );
        assert!(
            backend
                .command_with_authority_roots_observation(
                    executable,
                    &[],
                    ResolverExecutionPhase::RepositoryInitialization,
                    None,
                    None,
                    ResolverExecutionAuthorityRoots {
                        discovery_read_root: None,
                        inspection_read_root: Some(&inspection_root),
                        mutable_root: Some(&inspection_root),
                    },
                )
                .is_err(),
            "other phases reject inspection content-read authority"
        );
        assert!(
            backend
                .command_with_inspection_read_root_observation(
                    executable,
                    &[],
                    Path::new("relative"),
                )
                .is_err(),
            "inspection content-read roots must be absolute"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn confined_metadata_paths_are_derived_deduplicated_and_bounded() {
        let additional = [
            PathBuf::from("/bin/sh"),
            PathBuf::from("/usr/bin/git"),
            PathBuf::from("/usr/bin/git"),
        ];
        let paths = macos_confined_metadata_paths(
            Path::new("/usr/bin/git"),
            &additional,
            &[
                Path::new("/private/tmp/repository"),
                Path::new(MACOS_TLS_CONFIGURATION_ROOT),
                Path::new(MACOS_TLS_CONFIGURATION_ALIAS_ROOT),
            ],
        )
        .expect("derive metadata paths");
        assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
        for required in [
            "/",
            "/bin",
            "/bin/sh",
            "/dev",
            "/dev/null",
            "/private",
            "/private/tmp",
            "/private/tmp/repository",
            "/private/etc",
            "/private/etc/ssl",
            "/etc",
            "/etc/ssl",
            "/usr",
            "/usr/bin",
            "/usr/bin/git",
        ] {
            assert!(paths.iter().any(|path| path == Path::new(required)));
        }
        assert!(
            !paths
                .iter()
                .any(|path| path == Path::new("/private/tmp/sibling"))
        );

        let mut excessive = PathBuf::from("/");
        for _ in 0..MACOS_CONFINED_METADATA_PATH_LIMIT {
            excessive.push("a");
        }
        assert!(macos_confined_metadata_paths(Path::new("/bin/sh"), &[], &[&excessive]).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn helper_metadata_roots_are_derived_deduplicated_and_never_global() {
        let roots = macos_helper_metadata_roots(&[
            PathBuf::from("/opt/omega/libexec/git-remote-https"),
            PathBuf::from("/opt/omega/libexec/git-remote-http"),
        ])
        .expect("derive helper metadata roots");
        assert_eq!(roots, [PathBuf::from("/opt/omega/libexec")]);
        assert!(macos_helper_metadata_roots(&[PathBuf::from("/helper")]).is_err());
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
        let inspection_root = inspection_root();
        let (_, inspection) = backend
            .command_with_inspection_read_root_observation(executable, &[], &inspection_root)
            .expect("issue inspection policy observation");
        let fetch_route = loopback_route(&backend);
        let (_, fetch) = backend
            .command_with_endpoint_route_observation(
                executable,
                &[],
                ResolverExecutionPhase::Fetch,
                Some(ResolverExecutionNetworkTransport::Ssh),
                Some(&fetch_route),
                Some(&mutable_root),
            )
            .expect("issue fetch policy observation");
        assert_eq!(inspection.network_transport(), None);
        assert!(inspection.endpoint_route().is_none());
        assert_eq!(
            fetch.network_transport(),
            Some(ResolverExecutionNetworkTransport::Ssh)
        );
        assert_eq!(
            fetch
                .endpoint_route()
                .expect("fetch route policy")
                .requested_endpoint()
                .port(),
            9
        );
        assert!(
            backend
                .command_with_observation(
                    executable,
                    &[],
                    ResolverExecutionPhase::TransportDiscovery,
                    None,
                    None,
                )
                .is_err(),
            "networked phases require explicit transport authority"
        );
        assert!(
            backend
                .command_with_authority_roots_observation(
                    executable,
                    &[],
                    ResolverExecutionPhase::RepositoryInspection,
                    Some(ResolverExecutionNetworkTransport::Https),
                    None,
                    ResolverExecutionAuthorityRoots {
                        discovery_read_root: None,
                        inspection_read_root: Some(&inspection_root),
                        mutable_root: None,
                    },
                )
                .is_err(),
            "nonnetwork phases reject transport authority"
        );
        assert!(
            backend
                .command_with_authority_roots_observation(
                    executable,
                    &[],
                    ResolverExecutionPhase::RepositoryInspection,
                    None,
                    Some(&fetch_route),
                    ResolverExecutionAuthorityRoots {
                        discovery_read_root: None,
                        inspection_read_root: Some(&inspection_root),
                        mutable_root: None,
                    },
                )
                .is_err(),
            "nonnetwork phases reject endpoint routes"
        );

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
                .command_with_inspection_read_root_observation(executable, &[], &inspection_root,)
                .expect("reissue inspection policy observation")
                .1
                .canonical_bytes()
        );
        assert_ne!(inspection.canonical_bytes(), fetch.canonical_bytes());
        let alternate_inspection_root = inspection_root.join("alternate");
        let alternate_inspection = backend
            .command_with_inspection_read_root_observation(
                executable,
                &[],
                &alternate_inspection_root,
            )
            .expect("issue alternate inspection policy observation")
            .1;
        assert_ne!(
            inspection.canonical_bytes(),
            alternate_inspection.canonical_bytes()
        );
        assert_eq!(inspection.executable(), executable);
        assert_eq!(
            inspection.inspection_read_root(),
            Some(inspection_root.as_path())
        );
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
        let inspection_root = inspection_root();
        let (_, left) = backend
            .command_with_inspection_read_root_observation(
                executable,
                &[second.clone(), first.clone(), second.clone()],
                &inspection_root,
            )
            .expect("construct normalized policy observation");
        let (_, right) = backend
            .command_with_inspection_read_root_observation(
                executable,
                &[first.clone(), second.clone()],
                &inspection_root,
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
                .command_with_inspection_read_root_observation(
                    executable,
                    &excessive,
                    &inspection_root,
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
            .command_with_inspection_read_root_observation(executable, &[], &mutable_root)
            .expect("issue inspection policy observation");
        let (initialization_command, initialization) = backend
            .command_with_observation(
                executable,
                &[],
                ResolverExecutionPhase::RepositoryInitialization,
                None,
                Some(&mutable_root),
            )
            .expect("issue initialization policy observation");
        let discovery_route = loopback_route(&backend);
        let (discovery_command, discovery) = backend
            .command_with_discovery_route_observation(
                executable,
                &[],
                ResolverExecutionNetworkTransport::Ssh,
                &discovery_route,
                &mutable_root,
            )
            .expect("issue discovery policy observation");
        let https_discovery_route = loopback_route(&backend);
        let (https_discovery_command, https_discovery) = backend
            .command_with_discovery_route_observation(
                executable,
                &[PathBuf::from("/usr/bin/stat")],
                ResolverExecutionNetworkTransport::Https,
                &https_discovery_route,
                &mutable_root,
            )
            .expect("issue HTTPS discovery policy observation");
        let fetch_route = loopback_route(&backend);
        let (fetch_command, fetch) = backend
            .command_with_endpoint_route_observation(
                executable,
                &[],
                ResolverExecutionPhase::Fetch,
                Some(ResolverExecutionNetworkTransport::Ssh),
                Some(&fetch_route),
                Some(&mutable_root),
            )
            .expect("issue fetch policy observation");
        let https_fetch_route = loopback_route(&backend);
        let (https_fetch_command, https_fetch) = backend
            .command_with_endpoint_route_observation(
                executable,
                &[],
                ResolverExecutionPhase::Fetch,
                Some(ResolverExecutionNetworkTransport::Https),
                Some(&https_fetch_route),
                Some(&mutable_root),
            )
            .expect("issue HTTPS fetch policy observation");
        assert!(inspection.generated_policy_sha256().is_some());
        assert!(initialization.generated_policy_sha256().is_some());
        assert!(discovery.generated_policy_sha256().is_some());
        assert!(fetch.generated_policy_sha256().is_some());
        assert!(https_fetch.generated_policy_sha256().is_some());
        assert_ne!(
            inspection.generated_policy_sha256(),
            fetch.generated_policy_sha256()
        );
        assert_ne!(
            discovery.generated_policy_sha256(),
            https_discovery.generated_policy_sha256()
        );
        assert_ne!(
            discovery.canonical_bytes(),
            https_discovery.canonical_bytes()
        );
        assert_ne!(fetch.canonical_bytes(), https_fetch.canonical_bytes());
        assert_eq!(
            discovery.discovery_read_root(),
            Some(mutable_root.as_path())
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
        assert!(!inspection_profile.contains("process-fork"));
        assert!(!inspection_profile.contains("(allow file-read*)"));
        assert!(!inspection_profile.contains("(allow file-read-metadata)"));
        assert!(inspection_profile.contains(
            "file-read-metadata file-test-existence (subpath (param \"INSPECTION_READ_ROOT\"))"
        ));
        assert!(inspection_profile.contains("(literal (param \"METADATA_PATH_0\"))"));
        assert!(
            inspection_profile
                .contains("(allow file-read-data (subpath (param \"INSPECTION_READ_ROOT\"))")
        );
        assert!(
            inspection_profile
                .contains("(allow file-test-existence file-write-data (literal \"/dev/null\"))")
        );
        let initialization_profile = profile(&initialization_command);
        assert!(!initialization_profile.contains("(import"));
        assert!(!initialization_profile.contains("network-outbound"));
        assert!(!initialization_profile.contains("process-fork"));
        assert!(!initialization_profile.contains("(allow file-read*)"));
        assert!(!initialization_profile.contains("(allow file-read-metadata)"));
        assert!(
            initialization_profile.contains(
                "file-read-metadata file-test-existence (subpath (param \"MUTABLE_ROOT\"))"
            )
        );
        assert!(initialization_profile.contains("(literal (param \"METADATA_PATH_0\"))"));
        assert!(
            initialization_profile
                .contains("(allow file-read-data (subpath (param \"MUTABLE_ROOT\"))")
        );
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
        assert!(
            discovery_profile
                .contains("(allow network-outbound (remote tcp (param \"BROKER_ENDPOINT\")))")
        );
        assert!(!discovery_profile.contains("(allow network-outbound)"));
        assert!(!discovery_profile.contains("file-write*"));
        assert!(discovery_profile.contains("(allow process-fork)"));
        assert!(discovery_profile.contains(
            "(allow mach-lookup (global-name \"com.apple.system.opendirectoryd.libinfo\"))"
        ));
        assert!(discovery_profile.contains("(allow sysctl-read (sysctl-name \"kern.hostname\"))"));
        assert!(
            discovery_profile.contains("(allow sysctl-read (sysctl-name \"hw.pagesize_compat\"))")
        );
        assert!(!discovery_profile.contains("(allow sysctl-read)"));
        let https_discovery_profile = profile(&https_discovery_command);
        assert!(
            https_discovery_profile
                .contains("(allow network-outbound (remote tcp (param \"BROKER_ENDPOINT\")))")
        );
        assert!(!https_discovery_profile.contains("mach-lookup"));
        assert!(!https_discovery_profile.contains("sysctl-read"));
        assert!(!https_discovery_profile.contains("(allow file-read*)"));
        assert!(!https_discovery_profile.contains("(allow file-read-metadata)"));
        assert!(https_discovery_profile.contains(
            "file-read-metadata file-test-existence (subpath (param \"DISCOVERY_READ_ROOT\"))"
        ));
        assert!(https_discovery_profile.contains("(literal (param \"METADATA_PATH_0\"))"));
        assert!(https_discovery_profile.contains("(subpath (param \"METADATA_SUBPATH_0\"))"));
        assert!(https_discovery_profile.contains("(subpath \"/etc/ssl\")"));
        assert!(
            https_discovery_profile
                .contains("(allow file-read-data (subpath (param \"DISCOVERY_READ_ROOT\"))")
        );
        assert!(https_discovery_profile.contains("(subpath \"/private/etc/ssl\")"));
        let fetch_profile = profile(&fetch_command);
        assert!(!fetch_profile.contains("(import"));
        assert!(
            fetch_profile
                .contains("(allow network-outbound (remote tcp (param \"BROKER_ENDPOINT\")))")
        );
        assert!(fetch_profile.contains("(allow file-write* (subpath (param \"MUTABLE_ROOT\")))"));
        assert!(fetch_profile.contains("(allow process-fork)"));
        assert!(fetch_profile.contains(
            "(allow mach-lookup (global-name \"com.apple.system.opendirectoryd.libinfo\"))"
        ));
        assert!(fetch_profile.contains("(allow sysctl-read (sysctl-name \"kern.hostname\"))"));
        assert!(fetch_profile.contains("(allow sysctl-read (sysctl-name \"hw.pagesize_compat\"))"));
        assert!(fetch_profile.contains("(allow file-read*)"));
        let https_fetch_profile = profile(&https_fetch_command);
        assert!(!https_fetch_profile.contains("(allow file-read*)"));
        assert!(https_fetch_profile.contains("(allow file-read-metadata)"));
        assert!(
            https_fetch_profile
                .contains("(allow file-read-data (subpath (param \"MUTABLE_ROOT\"))")
        );
        assert!(https_fetch_profile.contains("(subpath \"/private/etc/ssl\")"));
        assert!(!https_fetch_profile.contains("mach-lookup"));
        assert!(!https_fetch_profile.contains("sysctl-read"));

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
            ResolverExecutionGuarantee::DescendantProcessesContained,
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
            ResolverExecutionGuaranteeDisposition::Enforced
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
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(
                &discovery,
                ResolverExecutionGuarantee::FilesystemReadsConfined
            ),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
        assert_eq!(
            disposition(
                &https_discovery,
                ResolverExecutionGuarantee::FilesystemReadsConfined
            ),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(
                &inspection,
                ResolverExecutionGuarantee::FilesystemReadsConfined
            ),
            ResolverExecutionGuaranteeDisposition::Enforced
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
            disposition(
                &inspection,
                ResolverExecutionGuarantee::DescendantProcessesContained
            ),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(
                &discovery,
                ResolverExecutionGuarantee::DescendantProcessesContained
            ),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::NetworkDenied),
            ResolverExecutionGuaranteeDisposition::NotRequired
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::NetworkEndpointsConfined),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::ExecutablePathsConfined),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
        assert_eq!(
            disposition(&fetch, ResolverExecutionGuarantee::FilesystemWritesConfined),
            ResolverExecutionGuaranteeDisposition::Enforced
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
        let inspection_root = inspection_root();
        let mut command = backend
            .command_with_inspection_read_root(
                Path::new("/bin/sh"),
                &helper_executables,
                &inspection_root,
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
        allowed.current_dir(&root);
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
        denied.current_dir(&root);
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
    fn seatbelt_initialization_confines_file_content_to_the_mutable_root() {
        use std::os::unix::fs::symlink;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let parent = std::env::temp_dir().join(format!(
            "omega-resolver-initialization-read-{}-{sequence}",
            std::process::id()
        ));
        let mutable_root = parent.join("mutable");
        std::fs::create_dir_all(&mutable_root).expect("create initialization mutable root");
        let mutable_root = mutable_root
            .canonicalize()
            .expect("canonicalize initialization mutable root");
        let inside = mutable_root.join("inside");
        let sibling = parent.join("sibling");
        let escaped_link = mutable_root.join("escaped-link");
        std::fs::write(&inside, b"inside").expect("write inside canary");
        std::fs::write(&sibling, b"sibling").expect("write sibling canary");
        symlink(&sibling, &escaped_link).expect("create escaping symlink");

        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let mut allowed = backend
            .command(
                Path::new("/bin/cat"),
                &[],
                ResolverExecutionPhase::RepositoryInitialization,
                Some(&mutable_root),
            )
            .expect("build initialization-content sandbox");
        allowed.current_dir(&mutable_root);
        let output = allowed.arg(&inside).output().expect("read inside content");
        assert!(
            output.status.success(),
            "inside read failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"inside");

        for denied_path in [&sibling, &escaped_link] {
            let mut denied = backend
                .command(
                    Path::new("/bin/cat"),
                    &[],
                    ResolverExecutionPhase::RepositoryInitialization,
                    Some(&mutable_root),
                )
                .expect("build initialization-content sandbox");
            denied.current_dir(&mutable_root);
            let output = denied
                .arg(denied_path)
                .output()
                .expect("attempt escaped content read");
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }

        std::fs::remove_file(escaped_link).expect("remove escaping symlink");
        std::fs::remove_file(inside).expect("remove inside canary");
        std::fs::remove_file(sibling).expect("remove sibling canary");
        std::fs::remove_dir(mutable_root).expect("remove initialization mutable root");
        std::fs::remove_dir(parent).expect("remove initialization parent");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_initialization_confines_metadata_to_the_mutable_root() {
        use std::os::unix::fs::symlink;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let parent = std::env::temp_dir().join(format!(
            "omega-resolver-initialization-metadata-{}-{sequence}",
            std::process::id()
        ));
        let raw_mutable_root = parent.join("mutable");
        let raw_inside = raw_mutable_root.join("inside");
        let raw_sibling = parent.join("sibling");
        let raw_escaped_link = raw_mutable_root.join("escaped-link");
        std::fs::create_dir_all(&raw_inside).expect("create initialization metadata root");
        std::fs::create_dir(&raw_sibling).expect("create initialization metadata sibling");
        symlink(&raw_sibling, &raw_escaped_link)
            .expect("create initialization escaping metadata symlink");
        let mutable_root = raw_mutable_root
            .canonicalize()
            .expect("canonicalize initialization metadata root");
        let inside = mutable_root.join("inside");
        let sibling = raw_sibling
            .canonicalize()
            .expect("canonicalize initialization metadata sibling");
        let escaped_link = mutable_root.join("escaped-link");

        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let run_stat = |arguments: &[&std::ffi::OsStr]| {
            let mut command = backend
                .command(
                    Path::new("/usr/bin/stat"),
                    &[],
                    ResolverExecutionPhase::RepositoryInitialization,
                    Some(&mutable_root),
                )
                .expect("build initialization-metadata sandbox");
            command.current_dir(&mutable_root);
            command
                .args(arguments)
                .output()
                .expect("run initialization metadata canary")
        };

        let inside_output = run_stat(&[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("%N"),
            inside.as_os_str(),
        ]);
        assert!(
            inside_output.status.success(),
            "inside metadata failed: {}",
            String::from_utf8_lossy(&inside_output.stderr)
        );
        let link_output = run_stat(&[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("%N"),
            escaped_link.as_os_str(),
        ]);
        assert!(
            link_output.status.success(),
            "reading the in-root symlink entry must remain allowed"
        );
        let sibling_output = run_stat(&[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("%N"),
            sibling.as_os_str(),
        ]);
        assert!(!sibling_output.status.success());
        let escaped_output = run_stat(&[std::ffi::OsStr::new("-L"), escaped_link.as_os_str()]);
        assert!(!escaped_output.status.success());

        std::fs::remove_file(escaped_link).expect("remove initialization metadata symlink");
        std::fs::remove_dir(inside).expect("remove initialization inside metadata canary");
        std::fs::remove_dir(sibling).expect("remove initialization sibling metadata canary");
        std::fs::remove_dir(mutable_root).expect("remove initialization metadata root");
        std::fs::remove_dir(parent).expect("remove initialization metadata parent");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_https_fetch_confines_file_content_to_mutable_and_tls_roots() {
        use std::os::unix::fs::symlink;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let parent = std::env::temp_dir().join(format!(
            "omega-resolver-https-fetch-read-{}-{sequence}",
            std::process::id()
        ));
        let mutable_root = parent.join("mutable");
        std::fs::create_dir_all(&mutable_root).expect("create HTTPS fetch mutable root");
        let mutable_root = mutable_root
            .canonicalize()
            .expect("canonicalize HTTPS fetch mutable root");
        let inside = mutable_root.join("inside");
        let sibling = parent.join("sibling");
        let escaped_link = mutable_root.join("escaped-link");
        std::fs::write(&inside, b"inside").expect("write inside canary");
        std::fs::write(&sibling, b"sibling").expect("write sibling canary");
        symlink(&sibling, &escaped_link).expect("create escaping symlink");

        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let route = loopback_route(&backend);
        let build_command = || {
            let (mut command, _observation) = backend
                .command_with_endpoint_route_observation(
                    Path::new("/bin/cat"),
                    &[],
                    ResolverExecutionPhase::Fetch,
                    Some(ResolverExecutionNetworkTransport::Https),
                    Some(&route),
                    Some(&mutable_root),
                )
                .expect("build HTTPS fetch content sandbox");
            command.current_dir(&mutable_root);
            command
        };

        let output = build_command()
            .arg(&inside)
            .output()
            .expect("read mutable-root content");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"inside");
        let output = build_command()
            .arg("/private/etc/ssl/openssl.cnf")
            .output()
            .expect("read fixed TLS configuration");
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());

        for denied_path in [&sibling, &escaped_link] {
            let output = build_command()
                .arg(denied_path)
                .output()
                .expect("attempt escaped HTTPS fetch content read");
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
        route.finish().expect("finish HTTPS fetch route");

        std::fs::remove_file(escaped_link).expect("remove escaping symlink");
        std::fs::remove_file(inside).expect("remove inside canary");
        std::fs::remove_file(sibling).expect("remove sibling canary");
        std::fs::remove_dir(mutable_root).expect("remove HTTPS fetch mutable root");
        std::fs::remove_dir(parent).expect("remove HTTPS fetch parent");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_https_discovery_confines_file_content_to_working_and_tls_roots() {
        use std::os::unix::fs::symlink;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let parent = std::env::temp_dir().join(format!(
            "omega-resolver-https-discovery-read-{}-{sequence}",
            std::process::id()
        ));
        let discovery_root = parent.join("working");
        std::fs::create_dir_all(&discovery_root).expect("create HTTPS discovery root");
        let discovery_root = discovery_root
            .canonicalize()
            .expect("canonicalize HTTPS discovery root");
        let inside = discovery_root.join("inside");
        let sibling = parent.join("sibling");
        let escaped_link = discovery_root.join("escaped-link");
        std::fs::write(&inside, b"inside").expect("write inside canary");
        std::fs::write(&sibling, b"sibling").expect("write sibling canary");
        symlink(&sibling, &escaped_link).expect("create escaping symlink");

        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let route = loopback_route(&backend);
        let build_command = || {
            let (mut command, _observation) = backend
                .command_with_discovery_route_observation(
                    Path::new("/bin/cat"),
                    &[],
                    ResolverExecutionNetworkTransport::Https,
                    &route,
                    &discovery_root,
                )
                .expect("build HTTPS discovery content sandbox");
            command.current_dir(&discovery_root);
            command
        };

        let output = build_command()
            .arg(&inside)
            .output()
            .expect("read discovery-root content");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"inside");
        let output = build_command()
            .arg("/private/etc/ssl/openssl.cnf")
            .output()
            .expect("read fixed TLS configuration");
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());

        for denied_path in [&sibling, &escaped_link] {
            let output = build_command()
                .arg(denied_path)
                .output()
                .expect("attempt escaped HTTPS discovery content read");
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
        route.finish().expect("finish HTTPS discovery route");

        std::fs::remove_file(escaped_link).expect("remove escaping symlink");
        std::fs::remove_file(inside).expect("remove inside canary");
        std::fs::remove_file(sibling).expect("remove sibling canary");
        std::fs::remove_dir(discovery_root).expect("remove HTTPS discovery root");
        std::fs::remove_dir(parent).expect("remove HTTPS discovery parent");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_https_discovery_confines_metadata_to_working_and_tls_roots() {
        use std::os::unix::fs::symlink;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let parent = std::env::temp_dir().join(format!(
            "omega-resolver-https-discovery-metadata-{}-{sequence}",
            std::process::id()
        ));
        let raw_discovery_root = parent.join("working");
        let raw_inside = raw_discovery_root.join("inside");
        let raw_sibling = parent.join("sibling");
        let raw_escaped_link = raw_discovery_root.join("escaped-link");
        std::fs::create_dir_all(&raw_inside).expect("create HTTPS discovery metadata root");
        std::fs::create_dir(&raw_sibling).expect("create HTTPS discovery metadata sibling");
        symlink(&raw_sibling, &raw_escaped_link)
            .expect("create HTTPS discovery escaping metadata symlink");
        let discovery_root = raw_discovery_root
            .canonicalize()
            .expect("canonicalize HTTPS discovery metadata root");
        let inside = discovery_root.join("inside");
        let sibling = raw_sibling
            .canonicalize()
            .expect("canonicalize HTTPS discovery metadata sibling");
        let escaped_link = discovery_root.join("escaped-link");

        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let route = loopback_route(&backend);
        let run_stat = |arguments: &[&std::ffi::OsStr]| {
            let (mut command, _observation) = backend
                .command_with_discovery_route_observation(
                    Path::new("/usr/bin/stat"),
                    &[],
                    ResolverExecutionNetworkTransport::Https,
                    &route,
                    &discovery_root,
                )
                .expect("build HTTPS discovery-metadata sandbox");
            command.current_dir(&discovery_root);
            command
                .args(arguments)
                .output()
                .expect("run HTTPS discovery metadata canary")
        };

        for allowed_path in [
            inside.as_path(),
            Path::new("/private/etc/ssl/openssl.cnf"),
            Path::new("/etc/ssl/cert.pem"),
        ] {
            let output = run_stat(&[
                std::ffi::OsStr::new("-f"),
                std::ffi::OsStr::new("%N"),
                allowed_path.as_os_str(),
            ]);
            assert!(
                output.status.success(),
                "allowed metadata failed for {}: {}",
                allowed_path.display(),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let link_output = run_stat(&[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("%N"),
            escaped_link.as_os_str(),
        ]);
        assert!(
            link_output.status.success(),
            "reading the in-root symlink entry must remain allowed"
        );
        let sibling_output = run_stat(&[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("%N"),
            sibling.as_os_str(),
        ]);
        assert!(!sibling_output.status.success());
        let escaped_output = run_stat(&[std::ffi::OsStr::new("-L"), escaped_link.as_os_str()]);
        assert!(!escaped_output.status.success());
        route
            .finish()
            .expect("finish HTTPS discovery metadata route");

        std::fs::remove_file(escaped_link).expect("remove HTTPS discovery metadata symlink");
        std::fs::remove_dir(inside).expect("remove HTTPS discovery inside metadata canary");
        std::fs::remove_dir(sibling).expect("remove HTTPS discovery sibling metadata canary");
        std::fs::remove_dir(discovery_root).expect("remove HTTPS discovery metadata root");
        std::fs::remove_dir(parent).expect("remove HTTPS discovery metadata parent");
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
            .command_with_inspection_read_root(Path::new("/bin/sh"), &helper_executables, &root)
            .expect("build inspection sandbox");
        let status = allowed
            .args(["-c", "printf allowed > /dev/null"])
            .status()
            .expect("write the fixed null sink");
        assert!(status.success());

        let mut denied = backend
            .command_with_inspection_read_root(Path::new("/bin/sh"), &helper_executables, &root)
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
    fn seatbelt_inspection_confines_file_content_to_the_retained_repository() {
        use std::os::unix::fs::symlink;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let parent = std::env::temp_dir().join(format!(
            "omega-resolver-inspection-read-{}-{sequence}",
            std::process::id()
        ));
        let repository = parent.join("repository");
        std::fs::create_dir_all(&repository).expect("create inspection repository");
        let repository = repository
            .canonicalize()
            .expect("canonicalize inspection repository");
        let inside = repository.join("inside");
        let sibling = parent.join("sibling");
        let escaped_link = repository.join("escaped-link");
        std::fs::write(&inside, b"inside").expect("write inside canary");
        std::fs::write(&sibling, b"sibling").expect("write sibling canary");
        symlink(&sibling, &escaped_link).expect("create escaping symlink");

        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let mut allowed = backend
            .command_with_inspection_read_root(Path::new("/bin/cat"), &[], &repository)
            .expect("build repository-content sandbox");
        let output = allowed.arg(&inside).output().expect("read inside content");
        assert!(
            output.status.success(),
            "inside read failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"inside");

        for denied_path in [&sibling, &escaped_link] {
            let mut denied = backend
                .command_with_inspection_read_root(Path::new("/bin/cat"), &[], &repository)
                .expect("build repository-content sandbox");
            let output = denied
                .arg(denied_path)
                .output()
                .expect("attempt escaped content read");
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }

        std::fs::remove_file(escaped_link).expect("remove escaping symlink");
        std::fs::remove_file(inside).expect("remove inside canary");
        std::fs::remove_file(sibling).expect("remove sibling canary");
        std::fs::remove_dir(repository).expect("remove inspection repository");
        std::fs::remove_dir(parent).expect("remove inspection parent");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_inspection_confines_metadata_to_the_retained_repository() {
        use std::os::unix::fs::symlink;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let parent = std::env::temp_dir().join(format!(
            "omega-resolver-inspection-metadata-{}-{sequence}",
            std::process::id()
        ));
        let repository = parent.join("repository");
        let raw_inside = repository.join("inside");
        let raw_sibling = parent.join("sibling");
        let raw_escaped_link = repository.join("escaped-link");
        std::fs::create_dir_all(&raw_inside).expect("create inspection repository metadata");
        std::fs::create_dir(&raw_sibling).expect("create sibling metadata canary");
        symlink(&raw_sibling, &raw_escaped_link).expect("create escaping metadata symlink");
        let repository = repository
            .canonicalize()
            .expect("canonicalize inspection repository");
        let inside = repository.join("inside");
        let sibling = raw_sibling
            .canonicalize()
            .expect("canonicalize sibling metadata canary");
        let escaped_link = repository.join("escaped-link");

        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let run_stat = |arguments: &[&std::ffi::OsStr]| {
            let mut command = backend
                .command_with_inspection_read_root(Path::new("/usr/bin/stat"), &[], &repository)
                .expect("build repository-metadata sandbox");
            command
                .args(arguments)
                .output()
                .expect("run metadata canary")
        };

        let inside_output = run_stat(&[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("%N"),
            inside.as_os_str(),
        ]);
        assert!(
            inside_output.status.success(),
            "inside metadata failed: {}",
            String::from_utf8_lossy(&inside_output.stderr)
        );
        let link_output = run_stat(&[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("%N"),
            escaped_link.as_os_str(),
        ]);
        assert!(
            link_output.status.success(),
            "reading the in-root symlink entry must remain allowed"
        );
        let sibling_output = run_stat(&[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("%N"),
            sibling.as_os_str(),
        ]);
        assert!(!sibling_output.status.success());
        let escaped_output = run_stat(&[std::ffi::OsStr::new("-L"), escaped_link.as_os_str()]);
        assert!(!escaped_output.status.success());

        std::fs::remove_file(escaped_link).expect("remove escaping metadata symlink");
        std::fs::remove_dir(inside).expect("remove inside metadata canary");
        std::fs::remove_dir(sibling).expect("remove sibling metadata canary");
        std::fs::remove_dir(repository).expect("remove inspection repository");
        std::fs::remove_dir(parent).expect("remove inspection parent");
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
        let route = backend
            .open_endpoint_route(
                ResolverExecutionRequestedEndpoint::new("127.0.0.1", 9)
                    .expect("construct executable-denial endpoint"),
                ResolverExecutionTransferBudget::new(1024 * 1024)
                    .expect("construct transfer budget"),
            )
            .expect("open executable-denial route");
        let mut command = backend
            .command_with_endpoint_route_observation(
                Path::new("/bin/sh"),
                &helper_executables,
                ResolverExecutionPhase::Fetch,
                Some(ResolverExecutionNetworkTransport::Https),
                Some(&route),
                Some(&output),
            )
            .map(|(command, _)| command)
            .expect("build closed-executable sandbox");
        command.current_dir(&output);
        let status = command
            .args(["-c", "/usr/bin/touch \"$1\"", "resolver-test"])
            .arg(&marker)
            .status()
            .expect("attempt unlisted descendant execution");
        assert!(!status.success());
        assert!(!marker.exists());
        route.finish().expect("finish executable-denial route");
        std::fs::remove_dir(output).expect("remove executable-denial root");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_denies_allowlisted_descendant_creation_during_inspection() {
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let helper_executables = [
            Path::new("/bin/bash").to_path_buf(),
            Path::new("/usr/bin/true").to_path_buf(),
        ];
        let inspection_root = inspection_root();
        let mut command = backend
            .command_with_inspection_read_root(
                Path::new("/bin/sh"),
                &helper_executables,
                &inspection_root,
            )
            .expect("build descendant-denial sandbox");
        let status = command
            .args(["-c", "/usr/bin/true & wait"])
            .status()
            .expect("attempt allowlisted descendant creation");
        assert!(!status.success());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_denies_nonnetwork_tcp_and_confines_network_phases_to_the_broker() {
        use std::io::ErrorKind;
        use std::net::TcpListener;

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback canary");
        listener
            .set_nonblocking(true)
            .expect("make canary listener nonblocking");
        let port = listener.local_addr().expect("read canary address").port();
        let backend = ResolverExecutionBackend::open().expect("open resolver backend");
        let inspection_root = inspection_root();
        let mut denied = backend
            .command_with_inspection_read_root(Path::new("/usr/bin/nc"), &[], &inspection_root)
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
        denied.current_dir(&mutable_root);
        let status = denied
            .args(["127.0.0.1", &port.to_string()])
            .stdin(Stdio::null())
            .status()
            .expect("attempt denied initialization loopback connection");
        assert!(!status.success());
        assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));
        std::fs::remove_dir(mutable_root).expect("remove network-denial mutable root");

        let route = backend
            .open_endpoint_route(
                ResolverExecutionRequestedEndpoint::new("127.0.0.1", port)
                    .expect("construct broker destination"),
                ResolverExecutionTransferBudget::new(1024 * 1024)
                    .expect("construct transfer budget"),
            )
            .expect("open endpoint route");
        let broker_port = route.policy().broker_endpoint().port();
        let discovery_read_root = inspection_root.clone();
        let (mut allowed, _) = backend
            .command_with_discovery_route_observation(
                Path::new("/usr/bin/nc"),
                &[],
                ResolverExecutionNetworkTransport::Https,
                &route,
                &discovery_read_root,
            )
            .expect("build network-enabled sandbox");
        let status = allowed
            .args(["127.0.0.1", &broker_port.to_string()])
            .stdin(Stdio::null())
            .status()
            .expect("connect to exact loopback broker");
        assert!(status.success());

        let (mut direct, _) = backend
            .command_with_discovery_route_observation(
                Path::new("/usr/bin/nc"),
                &[],
                ResolverExecutionNetworkTransport::Https,
                &route,
                &discovery_read_root,
            )
            .expect("build endpoint-confined sandbox");
        let status = direct
            .args(["127.0.0.1", &port.to_string()])
            .stdin(Stdio::null())
            .status()
            .expect("attempt direct second-loopback connection");
        assert!(!status.success());
        assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));

        let observation = route.finish().expect("finish endpoint route");
        assert_eq!(observation.events().len(), 1);
        assert_eq!(
            observation.events()[0].outcome(),
            super::ResolverExecutionEndpointOutcome::MalformedConnect
        );
    }
}
