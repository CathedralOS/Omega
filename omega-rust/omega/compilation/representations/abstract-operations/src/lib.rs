#![forbid(unsafe_code)]

//! Optimizer module role: crate map. abstract operations representation.
//!
//! Start at [`abstract_operations`], which defines the program root and maps the
//! subordinate control, value, call, storage, and evidence owners.

pub mod abstract_operations;
pub use abstract_operations::{
    AbstractAtomicEvent, AbstractAtomicFenceOrdering, AbstractAtomicReadModifyWrite,
    AbstractBlockEntry, AbstractBoundaryResult, AbstractDynamicCallResult,
    AbstractDynamicDescriptorArgument, AbstractDynamicDescriptorSource, AbstractDynamicDispatch,
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractOperationPlanWithPlacedViewInputs, AbstractParameter, AbstractParameterDynamicDispatch,
    AbstractReboundDynamicDispatch, AbstractResult, AbstractStoredDynamicDescriptor,
    AbstractStoredDynamicDispatch, AbstractStructuralBinding, AbstractStructuralCallResult,
    AbstractStructuralCasePayloadBinding, AbstractStructuralCaseSuccessor, AbstractSuccessor,
    AtomicCoherenceViolation, AtomicModificationAfter, AtomicModificationAfterViolation,
    AtomicReadsFrom, AtomicReadsFromViolation, CompletionClaimSource, IntegerArithmetic,
    IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
    StructuralTypeCatalog, ValueBinding, atomic, calls, control_flow,
    happens_before_atomic_coherence_violation, operations, ownership, values,
};
pub use abstract_operations::{
    StructuralCopyPath, StructuralScalarStoreDestination, StructuralWindowRoot,
};
