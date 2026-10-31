#![forbid(unsafe_code)]

//! Optimizer module role: crate map. abstract operations representation.
//!
//! Start at [`abstract_operations`], which defines the program root and maps the
//! subordinate control, value, call, storage, and evidence owners.

pub(crate) use optimization_core::codec_bytes;

pub mod abstract_operations;
pub use abstract_operations::{
    AbstractAtomicEvent, AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite,
    AbstractBlockEntry, AbstractBoundaryResult, AbstractCallSignature, AbstractCallTarget,
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractOperationPlanWithAccessViewInputs, AbstractParameter, AbstractResult,
    AbstractStructuralBinding, AbstractStructuralCallResult, AbstractStructuralCasePayloadBinding,
    AbstractStructuralCaseSuccessor, AbstractSuccessor, AtomicCoherenceViolation,
    AtomicResidencyEdge, CompletionClaimSource, IntegerArithmetic, IntegerArithmeticOperator,
    IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy, StructuralCallFormal,
    StructuralCallResult, StructuralTypeCatalog, ValueBinding, atomic, binding_arguments,
    call_source, calls, control_flow, happens_before_atomic_coherence_violation, operations,
    ownership, values,
};
pub use abstract_operations::{
    StructuralCopyPath, StructuralScalarStoreDestination, StructuralWindowRoot,
};

pub use abstract_operations::AccessLifecycleOperands;
pub use abstract_operations::quotients;
pub use abstract_operations::{
    AccessReferenceBacking, BorrowedExtentBacking, ResidentReferenceBacking,
};
