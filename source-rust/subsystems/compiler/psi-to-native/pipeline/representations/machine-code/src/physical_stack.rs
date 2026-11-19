//! Physical stack extents and call edges retained after register allocation.
use function_identity::MachineFunctionIdentity;
use semantic_vocabulary::MachineId;
use target::NativeTarget;
use target_vocabulary::CallSiteOwner;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalStackExtent {
    Finite(u64),
    Conflicting,
    Unaccounted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalStackCall {
    pub target: MachineId,
    pub owner: CallSiteOwner,
    pub caller_live_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignStackExtent {
    None,
    Admitted(provider_stack_contribution::AdmittedSameStackContribution),
    Hosted { receipt: Option<[u8; 32]> },
    CompilerImport(target_vocabulary::CompilerImport),
}

impl ForeignStackExtent {
    pub fn from_binding(binding: &target_vocabulary::NormalizedForeignCallBinding) -> Self {
        use target_vocabulary::NormalizedForeignCallMechanism as Mechanism;
        match &binding.mechanism {
            Mechanism::Import {
                same_stack_contribution,
                ..
            }
            | Mechanism::Component {
                same_stack_contribution,
                ..
            }
            | Mechanism::Table {
                same_stack_contribution,
                ..
            } => Self::Admitted(same_stack_contribution.clone()),
            Mechanism::CompilerImport { import, .. } => Self::CompilerImport(*import),
            Mechanism::HostedImport { receipt, .. } => Self::Hosted {
                receipt: Some(*receipt),
            },
            Mechanism::SystemCall(_) => Self::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalForeignStackCall {
    pub owner: CallSiteOwner,
    pub caller_live_bytes: u32,
    pub extent: ForeignStackExtent,
    pub invoked_callbacks: Vec<MachineFunctionIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalStackFunction {
    pub machine: MachineId,
    pub local: LocalStackExtent,
    pub calls: Vec<PhysicalStackCall>,
    pub foreign_calls: Vec<PhysicalForeignStackCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalStackProgram {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub functions: Vec<PhysicalStackFunction>,
}

mod demand;
pub use demand::{StackDemand, StackDemandError, derive_physical_stack_demand};
