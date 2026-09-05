//! The current abstract operations program.
//!
//! This root describes program data at this resolution level. Its subordinate
//! areas own related facts; it does not contain transformation-stage objects.

use semantic_vocabulary::MachineId;
use terminal_psi::{
    BoundaryMachineDeclaration, ProviderCandidateConformance, StructuralTypeDeclaration,
    TerminalPsiIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub entry: MachineId,
    /// Concrete target-neutral carrier shapes retained for Omega-owned layout
    /// and ABI selection. These rows contain no source handles or target
    /// offsets.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    /// Exact bodyless boundary declarations available to Unit operations.
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    /// Complete verifier-approved checked provider catalog. Target/provider
    /// installation selects from these exact terminal IDs without changing
    /// terminal-Psi semantic identity.
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    pub functions: Vec<AbstractFunction>,
}

pub mod ownership;
pub use ownership::*;
pub mod calls;
pub use calls::*;
pub mod control_flow;
pub use control_flow::*;
pub mod values;
pub use values::*;
pub mod operations;
pub use operations::*;
