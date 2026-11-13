#![forbid(unsafe_code)]

/*
We define the target-neutral abstract operation representation (`AbstractOperationPlan`
and `AbstractOperation`) consumed by the Omega optimization and target-lowering
pipeline.

Terminal Psi (`PSITERM\0`) arrives with verified control flow, structural place
custody, loan lifetimes, and obligation certificates, but its serialized form is
organized as a wire artifact rather than an optimizer representation. We lower
verified Terminal Psi into flat per-machine operation plans where scalar values,
structural storage homes, call signatures, atomic memory events, and control
transfers carry explicit, self-contained semantic custody without target ABI
commitments.
*/

/*
Optimization and target-lowering stages need a single representation where
scalar SSA definitions, structural storage lifecycles, restoration windows,
and call contracts can be inspected without re-parsing Terminal Psi tables or
committing prematurely to physical ABIs. We partition the representation by
semantic domain—calls, control flow, atomic events, integer arithmetic,
quotients, access references, and structural stores—while keeping this crate
strictly target-neutral (`psi -> abstract-operations`): no register classes,
calling conventions, or instruction encodings appear here, and every projection
helper fails closed on duplicate or unverified declarations.
*/

use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, ProviderCandidateConformance, StructuralTypeDeclaration,
    TerminalPsiIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub entry: MachineId,
    pub structural_types: StructuralTypeCatalog, // Target-neutral carrier shapes retained for Omega layout and ABI selection.
    pub boundary_machines: Vec<BoundaryMachineDeclaration>, // Exact bodyless boundary declarations available to Unit operations.
    pub provider_candidates: Vec<ProviderCandidateConformance>, // Verifier-approved checked provider catalog.
    pub callables: terminal_psi::TerminalCallableCatalog,
    pub dynamic_dispatch: terminal_psi::TerminalDynamicDispatchCatalog, // Requirement-owned interfaces and selectable implementation maps.
    pub identities: terminal_psi::TerminalIdentityTable, // Sealed identity table every `TerminalIdentityIndex` on this plan resolves against.
    pub functions: Vec<AbstractFunction>,
}

impl AbstractOperationPlan {
    pub fn boundary_machine(&self, id: BoundaryMachineId) -> Option<&BoundaryMachineDeclaration> {
        self.boundary_machines
            .iter()
            .find(|declaration| declaration.id == id)
    }

    pub fn structural_type(&self, id: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
        self.structural_types
            .as_slice()
            .iter()
            .find(|declaration| declaration.id == id)
    }

    pub fn function(&self, machine: MachineId) -> Option<&AbstractFunction> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    pub fn operations(&self) -> impl Iterator<Item = &AbstractOperation> + '_ {
        self.functions
            .iter()
            .flat_map(|function| &function.operations)
    }
}

mod access_lifecycles;
mod access_references;
pub mod ownership;
pub use access_lifecycles::AccessLifecycleOperands;
pub use access_references::{
    AccessReferenceBacking, BorrowedExtentBacking, ResidentReferenceBacking,
};
mod structural_type_catalog;
pub use ownership::CompletionClaimSource;
pub use structural_type_catalog::StructuralTypeCatalog;
pub mod atomic;
pub use atomic::{
    AbstractAtomicEvent, AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite,
    AtomicCoherenceViolation, AtomicResidencyEdge, AtomicResidencySource,
    happens_before_atomic_coherence_violation,
};
pub mod calls;
pub use calls::{
    AbstractBoundaryResult, AbstractCallSignature, AbstractCallTarget,
    AbstractOperationPlanWithAccessViewInputs, AbstractStructuralCallResult, StructuralCallFormal,
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
pub use operations::{
    AbstractOperation, ModuleInvocation, StructuralResultMetadata, StructuralResultRole,
};
mod structural_stores;
pub use structural_stores::{
    StructuralCopyPath, StructuralScalarStoreDestination, StructuralWindowRoot,
};

mod storage_mutations;

pub mod quotients;
