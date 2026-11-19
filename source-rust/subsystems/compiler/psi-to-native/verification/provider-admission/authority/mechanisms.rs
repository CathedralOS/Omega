//! Role-tagged identities of physical operations and their canonical encoding.
use native_intrinsics::{
    CompilerIntrinsicExecutionIdentity, compiler_intrinsic_execution_identity_bytes,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedForeignArgumentContract {
    AdmittedPlan,
    Checked(CheckedSyscallArgumentContractIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedForeignTerminalMechanismIdentity {
    target: target::TargetProfile,
    locator_identity: target::ForeignLocatorIdentityDigest,
    implementation_contract: provider_plans::BoundaryCallingPlanCommitment,
    argument_contract: NormalizedForeignArgumentContract,
}

impl NormalizedForeignTerminalMechanismIdentity {
    pub fn from_normalized_locator(
        locator: &target::NormalizedForeignLocator,
        implementation_contract: provider_plans::BoundaryCallingPlanCommitment,
    ) -> Self {
        Self {
            target: locator.target(),
            locator_identity: locator.identity_digest(),
            implementation_contract,
            argument_contract: NormalizedForeignArgumentContract::AdmittedPlan,
        }
    }

    pub const fn with_checked_argument_contract(
        self,
        contract: CheckedSyscallArgumentContractIdentity,
    ) -> Self {
        Self {
            argument_contract: NormalizedForeignArgumentContract::Checked(contract),
            ..self
        }
    }

    pub const fn argument_contract(self) -> NormalizedForeignArgumentContract {
        self.argument_contract
    }

    pub const fn target(self) -> target::TargetProfile {
        self.target
    }

    pub const fn locator_identity(self) -> target::ForeignLocatorIdentityDigest {
        self.locator_identity
    }

    pub const fn implementation_contract(self) -> provider_plans::BoundaryCallingPlanCommitment {
        self.implementation_contract
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedSyscallArgumentContractIdentity([u8; 32]);

impl CheckedSyscallArgumentContractIdentity {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyscallTerminalMechanismIdentity {
    target: target::TargetProfile,
    number: u32,
    checked_argument_contract: CheckedSyscallArgumentContractIdentity,
}

impl SyscallTerminalMechanismIdentity {
    pub const fn new(
        target: target::TargetProfile,
        number: u32,
        checked_argument_contract: CheckedSyscallArgumentContractIdentity,
    ) -> Self {
        Self {
            target,
            number,
            checked_argument_contract,
        }
    }

    pub const fn target(self) -> target::TargetProfile {
        self.target
    }

    pub const fn number(self) -> u32 {
        self.number
    }

    pub const fn checked_argument_contract(self) -> CheckedSyscallArgumentContractIdentity {
        self.checked_argument_contract
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedPhysicalOperationIdentity {
    PortWrite { port: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedPhysicalTerminalMechanismIdentity {
    target: target::TargetProfile,
    operation: CheckedPhysicalOperationIdentity,
}

impl CheckedPhysicalTerminalMechanismIdentity {
    pub const fn port_write(target: target::TargetProfile, port: u16) -> Self {
        Self {
            target,
            operation: CheckedPhysicalOperationIdentity::PortWrite { port },
        }
    }

    pub const fn target(self) -> target::TargetProfile {
        self.target
    }

    pub const fn operation(self) -> CheckedPhysicalOperationIdentity {
        self.operation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMechanismIdentity {
    CompilerIntrinsic(CompilerIntrinsicExecutionIdentity),
    NormalizedForeign(NormalizedForeignTerminalMechanismIdentity),
    CheckedPhysical(CheckedPhysicalTerminalMechanismIdentity),
    Syscall(SyscallTerminalMechanismIdentity),
    Table {
        target: target::TargetProfile,
        provider: provider_plans::ProviderPlanDigest,
        calling: provider_plans::BoundaryCallingPlanCommitment,
        byte_offset: u32,
        dispatch_only: bool,
    },
}

impl From<CompilerIntrinsicExecutionIdentity> for TerminalMechanismIdentity {
    fn from(identity: CompilerIntrinsicExecutionIdentity) -> Self {
        Self::CompilerIntrinsic(identity)
    }
}

impl From<NormalizedForeignTerminalMechanismIdentity> for TerminalMechanismIdentity {
    fn from(identity: NormalizedForeignTerminalMechanismIdentity) -> Self {
        Self::NormalizedForeign(identity)
    }
}

impl From<CheckedPhysicalTerminalMechanismIdentity> for TerminalMechanismIdentity {
    fn from(identity: CheckedPhysicalTerminalMechanismIdentity) -> Self {
        Self::CheckedPhysical(identity)
    }
}

impl From<SyscallTerminalMechanismIdentity> for TerminalMechanismIdentity {
    fn from(identity: SyscallTerminalMechanismIdentity) -> Self {
        Self::Syscall(identity)
    }
}

pub fn terminal_mechanism_identity_bytes(identity: TerminalMechanismIdentity) -> Vec<u8> {
    match identity {
        TerminalMechanismIdentity::Table {
            target,
            provider,
            calling,
            byte_offset,
            dispatch_only,
        } => {
            let target = target.identity().as_str().as_bytes();
            let mut bytes = vec![4];
            bytes.extend_from_slice(&(target.len() as u32).to_be_bytes());
            bytes.extend_from_slice(target);
            bytes.extend_from_slice(provider.as_bytes());
            bytes.extend_from_slice(&calling.as_bytes());
            bytes.extend_from_slice(&byte_offset.to_be_bytes());
            bytes.push(u8::from(dispatch_only));
            bytes
        }
        TerminalMechanismIdentity::CompilerIntrinsic(intrinsic) => {
            let mut bytes = Vec::with_capacity(9);
            bytes.push(0);
            bytes.extend_from_slice(&compiler_intrinsic_execution_identity_bytes(intrinsic));
            bytes
        }
        TerminalMechanismIdentity::NormalizedForeign(foreign) => {
            let target = foreign.target().identity().as_str().as_bytes();
            let mut bytes = Vec::with_capacity(1 + 4 + target.len() + 32 + 32 + 1 + 32);
            bytes.push(1);
            bytes.extend_from_slice(
                &u32::try_from(target.len())
                    .expect("target-profile identity length fits u32")
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(target);
            bytes.extend_from_slice(&foreign.locator_identity().as_bytes());
            bytes.extend_from_slice(&foreign.implementation_contract().as_bytes());
            match foreign.argument_contract() {
                NormalizedForeignArgumentContract::AdmittedPlan => {}
                NormalizedForeignArgumentContract::Checked(contract) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&contract.as_bytes());
                }
            }
            bytes
        }
        TerminalMechanismIdentity::CheckedPhysical(physical) => {
            let target = physical.target().identity().as_str().as_bytes();
            let mut bytes = Vec::with_capacity(1 + 4 + target.len() + 3);
            bytes.push(2);
            bytes.extend_from_slice(
                &u32::try_from(target.len())
                    .expect("target-profile identity length fits u32")
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(target);
            match physical.operation() {
                CheckedPhysicalOperationIdentity::PortWrite { port } => {
                    bytes.push(0);
                    bytes.extend_from_slice(&port.to_be_bytes());
                }
            }
            bytes
        }
        TerminalMechanismIdentity::Syscall(syscall) => {
            let target = syscall.target().identity().as_str().as_bytes();
            let mut bytes = Vec::with_capacity(1 + 4 + target.len() + 4 + 32);
            bytes.push(3);
            bytes.extend_from_slice(
                &u32::try_from(target.len())
                    .expect("target-profile identity length fits u32")
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(target);
            bytes.extend_from_slice(&syscall.number().to_be_bytes());
            bytes.extend_from_slice(&syscall.checked_argument_contract().as_bytes());
            bytes
        }
    }
}
