use super::ResolverExecutionBackend;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::confinement;
use crate::model::ResolverExecutionBackendIdentity;
use crate::network::{
    ResolverExecutionEndpointRoute, ResolverExecutionRequestedEndpoint,
    ResolverExecutionTransferBudget,
};
use std::io;
#[cfg(target_os = "macos")]
use std::path::PathBuf;

impl ResolverExecutionBackend {
    pub fn open() -> io::Result<Self> {
        #[cfg(target_os = "macos")]
        {
            let path = PathBuf::from(confinement::macos::MACOS_SANDBOX_EXECUTABLE);
            confinement::macos::verify_owned_native_executable(&path)?;
            let sandbox_metadata = confinement::macos::executable_metadata_identity(&path)?;
            let content_sha256 = confinement::macos::hash_executable(&path)?;
            if confinement::macos::executable_metadata_identity(&path)? != sandbox_metadata {
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
        #[cfg(target_os = "linux")]
        {
            let identity = if confinement::linux::backend_available() {
                ResolverExecutionBackendIdentity::LinuxLandlockV5
            } else {
                ResolverExecutionBackendIdentity::UnixResourceLimits
            };
            Ok(Self { identity })
        }
        #[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
        {
            Ok(Self {
                identity: ResolverExecutionBackendIdentity::UnixResourceLimits,
            })
        }
        #[cfg(windows)]
        {
            Ok(Self {
                identity: ResolverExecutionBackendIdentity::WindowsJobObject,
            })
        }
        #[cfg(not(any(unix, windows)))]
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
            confinement::macos::verify_owned_native_executable(executable)?;
            if confinement::macos::executable_metadata_identity(executable)?
                != self.sandbox_metadata
                || confinement::macos::hash_executable(executable)? != *content_sha256
            {
                return Err(io::Error::other(
                    "macOS resolver sandbox executable changed",
                ));
            }
        }
        #[cfg(target_os = "linux")]
        if matches!(
            self.identity,
            ResolverExecutionBackendIdentity::LinuxLandlockV5
        ) && !confinement::linux::backend_available()
        {
            return Err(io::Error::other(
                "Linux resolver Landlock v5 boundary became unavailable",
            ));
        }
        Ok(())
    }
}
