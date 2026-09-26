//! Input and value homes, writes, and stack execution evidence.
//! `frame_layout` holds raw geometry and prerequisite records; the backend
//! validates these before choosing or applying any executable frame protocol.

pub mod frame_application;
pub mod frame_identity;
pub mod frame_layout;
pub mod frame_protocol;
pub mod parameters;
pub mod scalars;
pub mod stack;
pub mod stores;

pub use frame_application::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationIdentity, function_fragment_frame_application_identity,
};
pub use frame_identity::{
    TargetFrameLayoutIdentity, TargetFrameProtocolEncodingIdentity, target_frame_layout_identity,
};
pub use frame_layout::{
    CalleeSaveFrameSlot, FrameUnwindPlan, FrameUnwindRestore,
    FunctionNonAuthoritativeCalleeSaveStorage, FunctionSpillFrameRequirements,
    FunctionTargetFrameLayout, LocalStorageFrameSlot, NonAuthoritativeCalleeSaveSlot,
    NonAuthoritativeCalleeSaveSlotId, NonAuthoritativeCalleeSaveStorageIdentity,
    NonAuthoritativeCalleeSaveStoragePlan, NonAuthoritativeCalleeSaveStoragePolicy,
    NonAuthoritativeSpillFrameRequirementIdentity, NonAuthoritativeSpillFrameRequirementPlan,
    NonAuthoritativeSpillFrameRequirementPolicy, OutgoingAbiFrameArea, ReturnAddressFrameCustody,
    StackProbePlan, TargetFrameLayoutPlan, TargetFrameLayoutPolicy,
};
pub use frame_protocol::{
    FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, target_frame_protocol_encoding_identity,
};
pub use parameters::{
    ParameterFunctionAbiRecord, StructuralSourceLocation, UnitEntryRegisterSpillRecord,
    UnitParameterHomeRecord, UnitParameterRecord, UnitScalarParameterLocationRecord,
};
pub use scalars::{
    UnitAffineScalarRecordEstablishmentRecord, UnitIntegerConstantRecord, UnitScalarHomeRecord,
};
pub use stack::{
    Aarch64ReturnLinkEvidence, ScalarCallStackEvidence, ScalarStackEvidence, ScalarStackMutation,
    ScalarStackMutationKind, StackAdjustmentPair, UnitCallStackEvidence, UnitStackEvidence,
};
pub use stores::{
    ScalarStructuralScalarFieldStoreRecord, UnitStructuralScalarFieldStoreRecord,
    UnitWriteOnlyPrimitiveStoreRecord, UnitWriteOnlyPrimitiveStoreSourceRecord,
};
