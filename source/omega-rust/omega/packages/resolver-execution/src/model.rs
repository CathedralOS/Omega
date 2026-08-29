use crate::network::ResolverExecutionEndpointRoutePolicy;
use std::path::{Path, PathBuf};

const RESOLVER_EXECUTION_OBSERVATION_SCHEMA_VERSION: u32 = 13;
const RESOLVER_EXECUTION_CANONICAL_BYTE_LIMIT: usize = 2 * 1024 * 1024;

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
    pub(crate) const fn permits_network(self) -> bool {
        matches!(self, Self::TransportDiscovery | Self::Fetch)
    }

    pub(crate) const fn requires_mutable_root(self) -> bool {
        matches!(self, Self::RepositoryInitialization | Self::Fetch)
    }

    pub(crate) const fn permits_descendant_processes(self) -> bool {
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
    pub(crate) const ALL: [Self; 13] = [
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
    pub(crate) guarantee: ResolverExecutionGuarantee,
    pub(crate) disposition: ResolverExecutionGuaranteeDisposition,
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
    pub(crate) backend: ResolverExecutionBackendIdentity,
    pub(crate) phase: ResolverExecutionPhase,
    pub(crate) network_transport: Option<ResolverExecutionNetworkTransport>,
    pub(crate) endpoint_route: Option<ResolverExecutionEndpointRoutePolicy>,
    pub(crate) generated_policy_sha256: Option<String>,
    pub(crate) resource_ceilings: ResolverExecutionResourceCeilings,
    pub(crate) executable: PathBuf,
    pub(crate) additional_executables: Vec<PathBuf>,
    pub(crate) discovery_read_root: Option<PathBuf>,
    pub(crate) inspection_read_root: Option<PathBuf>,
    pub(crate) mutable_root: Option<PathBuf>,
    pub(crate) guarantees: [ResolverExecutionGuaranteeRow; ResolverExecutionGuarantee::ALL.len()],
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
    pub(crate) core_dump_bytes: Option<u64>,
    pub(crate) cpu_seconds: Option<u64>,
    pub(crate) single_file_bytes: Option<u64>,
    pub(crate) open_files: Option<u64>,
    pub(crate) address_space_bytes: Option<u64>,
    pub(crate) process_count: Option<u64>,
    pub(crate) per_process_memory_bytes: Option<u64>,
    pub(crate) aggregate_memory_bytes: Option<u64>,
    pub(crate) aggregate_cpu_seconds: Option<u64>,
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

    pub const fn process_count(&self) -> Option<u64> {
        self.process_count
    }

    pub const fn per_process_memory_bytes(&self) -> Option<u64> {
        self.per_process_memory_bytes
    }

    pub const fn aggregate_memory_bytes(&self) -> Option<u64> {
        self.aggregate_memory_bytes
    }

    pub const fn aggregate_cpu_seconds(&self) -> Option<u64> {
        self.aggregate_cpu_seconds
    }

    fn encode(&self, bytes: &mut Vec<u8>) {
        for ceiling in [
            self.core_dump_bytes,
            self.cpu_seconds,
            self.single_file_bytes,
            self.open_files,
            self.address_space_bytes,
            self.process_count,
            self.per_process_memory_bytes,
            self.aggregate_memory_bytes,
            self.aggregate_cpu_seconds,
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
    WindowsJobObject,
    PortableProcessContainer,
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
        ResolverExecutionBackendIdentity::WindowsJobObject => bytes.push(3),
        ResolverExecutionBackendIdentity::PortableProcessContainer => bytes.push(4),
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
