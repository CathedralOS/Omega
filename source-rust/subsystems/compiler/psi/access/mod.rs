/*
Structural ownership, multiplicity (`StructuralMultiplicity`), access capabilities
(`StructuralAccess`, `ReferenceAccessCapability`), placed loan regions, affine cleanup actions
(`TerminalAffineCleanupAction`), and claim transfer contracts in `TerminalModule`.
*/

pub(crate) mod access;
pub(crate) mod borrows;
pub(crate) mod claims;
pub(crate) mod cleanup;
pub(crate) mod placement;
pub(crate) mod suspension;

pub use access::{StructuralAccess, StructuralMultiplicity, structural_access_covers};
pub use borrows::{
    RetainedBorrowContentProjection, RetainedBorrowCustody, RetainedBorrowPlace,
    RetainedBorrowPlaceRoot, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalReborrowRestorationClass,
    TerminalReborrowRestoredCallUse, TerminalReborrowSharedCohortMember,
};
pub use claims::{ClaimTransfer, CompletionReceipt, EntryClaim, LoanCompletion};
pub use cleanup::{
    NominalCleanupSelection, OwnedCleanupInvocation, StructuralAffineDiscard,
    TerminalAffineCleanupAction,
};
pub use placement::{
    AccessRamCorrespondence, AccessRamFieldCorrespondence, AccessorScalarBoundaryDemand,
    ResidentReferenceSource, TerminalAccessFieldPlan, TerminalAccessLifecycleExecution,
    TerminalAccessLifecycleOperation, TerminalAccessLifecyclePlan, TerminalAccessOwner,
    TerminalAccessProfileOperand, TerminalAccessProfileRequirement, TerminalAccessStorage,
    TerminalAccessViewInput, TerminalAccessViewPlan, TerminalAccessor, TerminalAccessorKind,
    canonical_access_view_identity,
};
pub use suspension::{
    TerminalSuspensionCallPlan, TerminalSuspensionCallSite, TerminalSuspensionCallTarget,
    TerminalSuspensionLiveValue, TerminalSuspensionLoan, TerminalSuspensionLoanBacking,
    TerminalSuspensionLoanCapture, TerminalSuspensionLoanResource, TerminalSuspensionLoans,
    TerminalSuspensionPlace, TerminalSuspensionStorage, TerminalSuspensionValueLoans,
    TerminalSuspensionValueType, suspension_frontier_commitment,
};
