//! Restricted operations a Build asks its receiving host to authorize.
#![forbid(unsafe_code)]
use build_output::BuildCapturedSourceInventory;
use build_model::contracts::BuildEvaluationSponsorLimits;

/// The source occurrence identified by the host asking for a package decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestrictedBuildSubject {
    Root,
    Dependency {
        digest: [u8; 32],
        description: String,
    },
}

/// The checked executable and selected entry that would perform a restricted
/// operation during this review. Portable policy records the operation itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildSite {
    pub subject: Option<RestrictedBuildSubject>,
    pub program: [u8; 32],
    pub entry: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestrictedBuildOperation {
    ProgramLaunch(Box<RestrictedProgramLaunch>),
    SendDatagram { peer: BuildNetworkPeer },
    ScopedFilesystemExecution, // Execute against scoped real filesystem read/write roots.
    UnscopedFilesystemExecution, // Execute against the real filesystem without path grants.
}

/// The complete scope of a fixed-peer UDP installation. The reviewed endpoint
/// cannot be redirected by the caller of the send operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildNetworkPeer {
    address: std::net::SocketAddr,
    max_datagram_bytes: u16,
}

impl BuildNetworkPeer {
    pub const MAX_DATAGRAM_BYTES: u16 = 65_507;

    pub fn new(address: std::net::SocketAddr, max_datagram_bytes: u16) -> Result<Self, String> {
        if address.port() == 0 || address.ip().to_canonical().is_unspecified() {
            return Err(
                "a fixed UDP peer requires a destination address and a nonzero port".into(),
            );
        }
        if matches!(address, std::net::SocketAddr::V6(peer) if peer.flowinfo() != 0) {
            return Err("a fixed UDP peer cannot specify IPv6 flow information".into());
        }
        if max_datagram_bytes > Self::MAX_DATAGRAM_BYTES {
            return Err("the maximum UDP datagram payload is 65507 bytes".into());
        }
        Ok(Self {
            address,
            max_datagram_bytes,
        })
    }

    pub const fn address(self) -> std::net::SocketAddr {
        self.address
    }

    /// A fixed UDP socket always uses the host stack. Only a destination confined
    /// to loopback proves that this provider cannot transmit off the machine.
    pub fn physical_authority(self) -> &'static [semantic_vocabulary::TerminalAuthorityClass] {
        use semantic_vocabulary::TerminalAuthorityClass::{
            HostNetworkStack, OffMachineNetworkTransmit,
        };
        if self.address.ip().to_canonical().is_loopback() {
            &[HostNetworkStack]
        } else {
            &[HostNetworkStack, OffMachineNetworkTransmit]
        }
    }

    pub const fn max_datagram_bytes(self) -> u16 {
        self.max_datagram_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestrictedBuildGrantRoot {
    SourceInventory, // Package's source inventory.
    StagedOutput,    // Activation's staged build-output tree.
    Other(u32),      // Compiler-issued grant root identity.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildGrant {
    root: RestrictedBuildGrantRoot,
    narrowed: bool, // Canonical metadata index narrows every operation to captured membership and kind.
    captured: Option<BuildCapturedSourceInventory>, // Immutable captured inventory bound to the root, when one exists.
}

impl RestrictedBuildGrant {
    #[doc(hidden)]
    pub const fn from_retained(
        root: RestrictedBuildGrantRoot,
        narrowed: bool,
        captured: Option<BuildCapturedSourceInventory>,
    ) -> Self {
        Self {
            root,
            narrowed,
            captured,
        }
    }

    pub const fn root(&self) -> RestrictedBuildGrantRoot {
        self.root
    }

    pub const fn narrowed(&self) -> bool {
        self.narrowed
    }

    pub const fn captured(&self) -> Option<BuildCapturedSourceInventory> {
        self.captured
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildBounds {
    filesystem_sponsor_limits: Option<filesystem_authority::FilesystemSponsorLimits>,
    evaluation_sponsor_limits: Option<BuildEvaluationSponsorLimits>,
    required_outputs: Vec<Vec<u8>>,
    artifact_only: bool,
}

impl RestrictedBuildBounds {
    #[doc(hidden)]
    pub const fn from_retained(
        filesystem_sponsor_limits: Option<filesystem_authority::FilesystemSponsorLimits>,
        evaluation_sponsor_limits: Option<BuildEvaluationSponsorLimits>,
        required_outputs: Vec<Vec<u8>>,
        artifact_only: bool,
    ) -> Self {
        Self {
            filesystem_sponsor_limits,
            evaluation_sponsor_limits,
            required_outputs,
            artifact_only,
        }
    }

    pub const fn filesystem_sponsor_limits(
        &self,
    ) -> Option<filesystem_authority::FilesystemSponsorLimits> {
        self.filesystem_sponsor_limits
    }

    pub const fn evaluation_sponsor_limits(&self) -> Option<BuildEvaluationSponsorLimits> {
        self.evaluation_sponsor_limits
    }

    pub fn required_outputs(&self) -> &[Vec<u8>] {
        &self.required_outputs
    }

    pub const fn artifact_only(&self) -> bool {
        self.artifact_only
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramDeclarationOwner {
    /// The request row is already scoped to the exact requesting occurrence.
    RequestingSource,
    Package([u8; 32]),
    Toolchain([u8; 32]),
}

/// Portable review meaning for one demanded program. The installed image
/// digest binds its runtime too on hosts admitting only static executables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedProgramLaunch {
    pub owner: ProgramDeclarationOwner,
    pub declaration: String,
    pub file: String,
    pub image_sha256: [u8; 32],
    pub readable: Vec<Vec<u8>>,
    pub writable: Vec<Vec<u8>>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildRequest {
    /// The checked path is evidence for the current proposal. Persistent
    /// approval compares the operation, resources, bounds and profiles.
    call_path: Option<String>,
    operation: RestrictedBuildOperation,
    read_grants: Vec<RestrictedBuildGrant>,
    write_grants: Vec<RestrictedBuildGrant>,
    bounds: RestrictedBuildBounds,
    build_execution_profile: Option<target::TargetProfile>,
    selected_target_profile: Option<target::TargetProfile>,
}

impl RestrictedBuildRequest {
    pub fn for_program(
        program: RestrictedProgramLaunch,
        bounds: RestrictedBuildBounds,
        profile: Option<target::TargetProfile>,
    ) -> Self {
        Self {
            call_path: None,
            operation: RestrictedBuildOperation::ProgramLaunch(Box::new(program)),
            read_grants: Vec::new(),
            write_grants: Vec::new(),
            bounds,
            build_execution_profile: profile,
            selected_target_profile: None,
        }
    }

    pub fn for_datagram(
        peer: BuildNetworkPeer,
        call_path: String,
        bounds: RestrictedBuildBounds,
        profile: Option<target::TargetProfile>,
    ) -> Self {
        Self {
            call_path: Some(call_path),
            operation: RestrictedBuildOperation::SendDatagram { peer },
            read_grants: Vec::new(),
            write_grants: Vec::new(),
            bounds,
            build_execution_profile: profile,
            selected_target_profile: None,
        }
    }

    pub fn call_path(&self) -> Option<&str> {
        self.call_path.as_deref()
    }

    pub const fn operation(&self) -> &RestrictedBuildOperation {
        &self.operation
    }

    pub fn read_grants(&self) -> &[RestrictedBuildGrant] {
        &self.read_grants
    }

    pub fn write_grants(&self) -> &[RestrictedBuildGrant] {
        &self.write_grants
    }

    pub const fn bounds(&self) -> &RestrictedBuildBounds {
        &self.bounds
    }

    pub const fn build_execution_profile(&self) -> Option<target::TargetProfile> {
        self.build_execution_profile
    }

    pub const fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.selected_target_profile
    }

    #[doc(hidden)]
    pub const fn from_retained(
        operation: RestrictedBuildOperation,
        read_grants: Vec<RestrictedBuildGrant>,
        write_grants: Vec<RestrictedBuildGrant>,
        bounds: RestrictedBuildBounds,
        build_execution_profile: Option<target::TargetProfile>,
        selected_target_profile: Option<target::TargetProfile>,
    ) -> Self {
        Self {
            call_path: None,
            operation,
            read_grants,
            write_grants,
            bounds,
            build_execution_profile,
            selected_target_profile,
        }
    }

    #[must_use]
    pub const fn for_target(mut self, profile: Option<target::TargetProfile>) -> Self {
        self.selected_target_profile = profile;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use target::TargetProfile::{LinuxArm64, LinuxX64};

    #[test]
    fn network_scope_requires_a_recoverable_destination_and_a_portable_udp_limit() {
        let peer = |address: &str, limit| BuildNetworkPeer::new(address.parse().unwrap(), limit);
        assert!(peer("127.0.0.1:8080", 0).is_ok());
        assert!(peer("[::1]:8080", 65_507).is_ok());
        assert!(peer("127.0.0.1:8080", 65_508).is_err());
        assert!(peer("127.0.0.1:0", 8).is_err());
        assert!(peer("0.0.0.0:8080", 8).is_err());
        assert!(peer("[::]:8080", 8).is_err());
        assert!(peer("[::ffff:0.0.0.0]:8080", 8).is_err());
        let routed = std::net::SocketAddrV6::new(std::net::Ipv6Addr::LOCALHOST, 8080, 1, 0);
        assert!(BuildNetworkPeer::new(routed.into(), 8).is_err());
    }

    #[test]
    fn assigning_a_product_target_preserves_the_required_grants_and_execution_profile() {
        let request = RestrictedBuildRequest::from_retained(
            RestrictedBuildOperation::ScopedFilesystemExecution,
            vec![RestrictedBuildGrant::from_retained(
                RestrictedBuildGrantRoot::SourceInventory,
                true,
                None,
            )],
            vec![RestrictedBuildGrant::from_retained(
                RestrictedBuildGrantRoot::StagedOutput,
                false,
                None,
            )],
            RestrictedBuildBounds::from_retained(None, None, vec![b"release.img".to_vec()], true),
            Some(LinuxX64),
            None,
        );
        let scoped = request.clone().for_target(Some(LinuxArm64));
        assert_eq!(scoped.selected_target_profile(), Some(LinuxArm64));
        assert_eq!(scoped.build_execution_profile(), Some(LinuxX64));
        assert_eq!(scoped.read_grants(), request.read_grants());
        assert_eq!(scoped.write_grants(), request.write_grants());
        assert_eq!(scoped.bounds(), request.bounds());
    }
}
