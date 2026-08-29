use crate::model::*;

#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(windows)]
pub(crate) mod windows;

pub(crate) fn guarantee_disposition(
    backend: &ResolverExecutionBackendIdentity,
    phase: ResolverExecutionPhase,
    network_transport: Option<ResolverExecutionNetworkTransport>,
    has_endpoint_route: bool,
    guarantee: ResolverExecutionGuarantee,
) -> ResolverExecutionGuaranteeDisposition {
    use ResolverExecutionBackendIdentity::{
        LinuxLandlockV5, MacosSeatbelt, PortableProcessContainer, UnixResourceLimits,
        WindowsJobObject,
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
                    && network_transport == Some(ResolverExecutionNetworkTransport::Https))
                    || (phase == ResolverExecutionPhase::Fetch
                        && network_transport
                            == Some(ResolverExecutionNetworkTransport::Https))) =>
        {
            Enforced
        }
        ProcessCountConfined | AggregateResourcesConfined
            if matches!(backend, WindowsJobObject) =>
        {
            Enforced
        }
        ProcessCountConfined | AggregateResourcesConfined => Unavailable,
        FilesystemReadsConfined => Unavailable,
        DescendantProcessesContained
            if matches!(backend, MacosSeatbelt { .. }) && !phase.permits_descendant_processes() =>
        {
            Enforced
        }
        DescendantProcessesContained if matches!(backend, WindowsJobObject) => Enforced,
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
        CpuTimeConfined if matches!(backend, WindowsJobObject) => Enforced,
        CoreDumpsDenied | CpuTimeConfined | SingleFileSizeConfined | OpenFilesConfined => {
            match backend {
                LinuxLandlockV5 | MacosSeatbelt { .. } | UnixResourceLimits => Enforced,
                WindowsJobObject | PortableProcessContainer => Unavailable,
            }
        }
        AddressSpaceConfined => match backend {
            LinuxLandlockV5 => Enforced,
            UnixResourceLimits if cfg!(any(target_os = "linux", target_os = "android")) => Enforced,
            MacosSeatbelt { .. }
            | UnixResourceLimits
            | WindowsJobObject
            | PortableProcessContainer => Unavailable,
        },
    }
}
