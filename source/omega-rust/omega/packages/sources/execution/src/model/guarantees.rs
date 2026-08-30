/// One optional native hardening property observed during package-source resolution.
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

    pub(super) const fn tag(self) -> u8 {
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

/// Whether one native hardening property was established for a phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionGuaranteeDisposition {
    Enforced,
    Unavailable,
    NotRequired,
}

impl ResolverExecutionGuaranteeDisposition {
    pub(super) const fn tag(self) -> u8 {
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
