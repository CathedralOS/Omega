//! The current abstract operations program.
//!
//! This root describes program data at this resolution level. Its subordinate
//! areas own related facts; it does not contain transformation-stage objects.

use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralTypeId};
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
    pub structural_types: StructuralTypeCatalog,
    /// Exact bodyless boundary declarations available to Unit operations.
    pub boundary_machines: Vec<BoundaryMachineDeclaration>,
    /// Complete verifier-approved checked provider catalog. Target/provider
    /// installation selects from these exact terminal IDs without changing
    /// terminal-Psi semantic identity.
    pub provider_candidates: Vec<ProviderCandidateConformance>,
    /// Complete requirement-owned interfaces and selectable implementation maps.
    /// The selector word chooses a table; ordinary call operands retain their
    /// independent storage and ownership identities.
    pub dynamic_dispatch: terminal_psi::TerminalDynamicDispatchCatalog,
    /// Identity text every `TerminalIdentityIndex` on this plan resolves
    /// against: the module discards nothing an index still names, so the
    /// sealed table travels with the plan verbatim.
    pub identities: terminal_psi::TerminalIdentityTable,
    pub functions: Vec<AbstractFunction>,
}

impl AbstractOperationPlan {
    /// The boundary machine keyed `id`, when the plan declares it.
    pub fn boundary_machine(&self, id: BoundaryMachineId) -> Option<&BoundaryMachineDeclaration> {
        self.boundary_machines
            .iter()
            .find(|declaration| declaration.id == id)
    }

    /// The structural type keyed `id`, when the plan declares it.
    pub fn structural_type(&self, id: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
        self.structural_types
            .as_slice()
            .iter()
            .find(|declaration| declaration.id == id)
    }

    /// The abstract function for `machine`, when the plan carries it.
    pub fn function(&self, machine: MachineId) -> Option<&AbstractFunction> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }
}

impl AbstractOperationPlan {
    /// Every function's operations, in function order.
    pub fn operations(&self) -> impl Iterator<Item = &AbstractOperation> + '_ {
        self.functions
            .iter()
            .flat_map(|function| &function.operations)
    }
}

pub mod ownership;
mod structural_type_catalog;
pub use ownership::CompletionClaimSource;
pub use structural_type_catalog::StructuralTypeCatalog;
pub mod atomic;
pub use atomic::{
    AbstractAtomicEvent, AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite,
    AtomicCoherenceViolation, AtomicModificationAfter, AtomicReadsFrom,
    happens_before_atomic_coherence_violation,
};
pub mod calls;
pub use calls::{
    AbstractBoundaryResult, AbstractCallSignature, AbstractCallTarget,
    AbstractOperationPlanWithPlacedViewInputs, AbstractStructuralCallResult, StructuralCallFormal,
    StructuralCallResult, call_source,
};
pub mod control_flow;
pub use control_flow::{
    AbstractBlockEntry, AbstractFunction, AbstractStructuralBinding,
    AbstractStructuralCasePayloadBinding, AbstractStructuralCaseSuccessor, AbstractSuccessor,
};
pub mod values;
pub use values::{
    AbstractFunctionResult, AbstractParameter, AbstractResult, ValueBinding, binding_arguments,
};
mod integer_arithmetic;
pub use integer_arithmetic::{
    IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftDirection,
    IntegerShiftPolicy,
};
pub mod operations;
pub use operations::AbstractOperation;
mod structural_stores;
pub use structural_stores::{StructuralScalarStoreDestination, StructuralWindowRoot};
mod structural_copy;
pub use structural_copy::StructuralCopyPath;

mod storage_mutations;

pub mod quotients;
