//! Native policy realization and truthful guarantee classification.
//!
//! Each host implementation realizes only its own closed policy; this module
//! classifies unavailable guarantees instead of promoting best-effort controls.

use crate::model::{
    ResolverExecutionBackendIdentity, ResolverExecutionGuarantee,
    ResolverExecutionGuaranteeDisposition, ResolverExecutionNetworkTransport,
    ResolverExecutionPhase,
};

#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(windows)]
pub(crate) mod windows;

pub(crate) fn guarantee_disposition(
    backend: &ResolverExecutionBackendIdentity,
    phase: ResolverExecutionPhase,
    _network_transport: Option<ResolverExecutionNetworkTransport>,
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
        ProcessCountConfined | AggregateResourcesConfined
            if matches!(backend, WindowsJobObject) =>
        {
            Enforced
        }
        ProcessCountConfined | AggregateResourcesConfined => Unavailable,
        // Ambient Git/SSH configuration and authentication are ordinary host
        // inputs, so resolver child reads are intentionally not confined to a
        // compiler-owned path set on any backend.
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
