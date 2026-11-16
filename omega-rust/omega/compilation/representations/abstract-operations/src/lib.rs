#![forbid(unsafe_code)]

/*
Abstract operations describe the executable program consumed by Omega's optimizer
and target lowering. Terminal Psi is the portable semantic product, in memory as
well as on the wire; serializing it is not what creates this boundary.

Lowering extracts the admitted execution closure, erases mathematical arguments,
normalizes scalar operations and makes runtime storage and cleanup explicit.
Blocks own their operation sequences and edge transfers. The optimizer derives
SSA definitions, uses, effects and rewrite facts from these same blocks rather
than reconstructing another partition of a flat instruction stream.

The operation vocabulary remains target-neutral. Native admission can already
close layout and lifecycle choices and select the execution closure; physical
ABI placements, register classes and instruction encodings belong to later forms.
Retained contracts and semantic identities preserve the relationship with the
authorized Psi product; neither copied evidence nor these data records grant
execution authority.
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
    pub structural_types: StructuralTypeCatalog, // Structural declarations and admitted layout/lifecycle relationships retained for ABI planning.
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
        self.functions.iter().flat_map(AbstractFunction::operations)
    }
}

pub mod ownership;
pub use native_vocabulary::access_lifecycles::AccessLifecycleOperands;
pub use native_vocabulary::access_references::{
    AccessReferenceBacking, BorrowedExtentBacking, ResidentReferenceBacking,
};
pub use native_vocabulary::structural_type_catalog;
pub use native_vocabulary::structural_type_catalog::{
    StructuralTypeCatalog, closed_owned_sequence_bounds,
};
pub use ownership::CompletionClaimSource;
pub mod atomic;
pub use atomic::{
    AbstractAtomicEvent, AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite,
    AtomicCoherenceViolation, AtomicResidencyEdge, AtomicResidencySource,
    happens_before_atomic_coherence_violation,
};
pub mod calls;
pub use calls::{
    AbstractBoundaryResult, AbstractCallSignature, AbstractCallTarget,
    AbstractStructuralCallResult, StructuralCallFormal, StructuralCallResult, call_source,
};
pub mod control_flow;
pub use control_flow::{
    AbstractBlock, AbstractFunction, AbstractStructuralBinding,
    AbstractStructuralCasePayloadBinding, AbstractStructuralCaseSuccessor, AbstractSuccessor,
};
pub mod values;
pub use native_vocabulary::integer_arithmetic::{
    IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftDirection,
    IntegerShiftPolicy,
};
pub use values::{
    AbstractFunctionResult, AbstractParameter, AbstractResult, ValueBinding, binding_arguments,
};
pub mod operations;
pub use native_vocabulary::structural_stores::{
    StructuralCopyPath, StructuralScalarStoreDestination, StructuralWindowRoot,
};
pub use operations::{
    AbstractOperation, ModuleInvocation, StructuralResultMetadata, StructuralResultRole,
};

mod storage_mutations;

pub mod quotients;
