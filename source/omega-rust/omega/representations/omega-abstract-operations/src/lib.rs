pub mod boundary;
pub mod data;
pub mod guard;
pub mod instruction;
pub mod ownership;
pub mod plan;
pub mod semantics;
pub mod values;

pub use boundary::{
    AbstractBoundaryEdge, AbstractBoundaryLink, AbstractBoundaryPolicyCheck,
    AbstractBoundaryPolicyVerdict, AbstractBoundarySummary, AbstractHostCallNativeArgument,
    AbstractHostCallOccurrence, AbstractHostCallSourceSite, AbstractSourceBoundaryEdge,
};
pub use data::{
    AbstractDataObject, AbstractDataObjectHandle, AbstractDataObjectKind, AbstractDataPlan,
    AbstractDynamicConformanceTable, AbstractDynamicConformanceTableRow, TargetDataObject,
    TargetDataObjectHandle, TargetDataObjectKind, TargetDataPlan,
};
pub use guard::{StateGuardLowering, StateGuardOperator};
pub use instruction::{
    AbstractFunctionPlan, AbstractHostFormalOperandBinding, AbstractHostOperationProvenance,
    AbstractOperation, AbstractOperationDomain, AbstractOperationKind, AbstractValueOperand,
    AbstractValueOperandHandle, CopyPlacesRole, FunctionInstructionPlan, InstructionOperand,
    InstructionOperandKind, PLACE_MAX_STEPS, Place, PlaceStep, RuntimeBitFieldFragment,
    RuntimeStorageRegion, RuntimeTextReadTarget, RuntimeValueOperand, RuntimeValueOperandHandle,
    SelectedInstruction, SelectedInstructionKind, ValueOperand, ValueOperandHandle,
};
pub use ownership::{
    AbstractOwnershipSummary, AbstractPermissionEvent, AbstractPermissionRealization,
    AbstractPermissionRealizationKind, CheckedNoCodePermissionReason,
    PermissionRealizationCandidate, PermissionRealizationCandidateKind, PermissionRealizationError,
};
pub use plan::{AbstractOperationCode, AbstractOperationPlan};
pub use semantics::AbstractSemanticSummary;
pub use values::{
    AbstractValueFact, AbstractValueFactHandle, AbstractValueOrigin, AbstractValueStatementRole,
    AbstractValueSummary,
};
mod boundary_footprints;
pub use boundary_footprints::*;
