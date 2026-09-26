//! Access, borrow, suspension, claim-transfer, and cleanup vocabulary.

mod access;
mod borrows;
mod claims;
mod cleanup;
mod placement;
mod suspension;

pub use access::{StructuralAccess, StructuralMultiplicity};
pub use borrows::{
    RetainedBorrowContentProjection, RetainedBorrowCustody, RetainedBorrowPlace,
    RetainedBorrowPlaceRoot, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment, TerminalReborrowRestorationClass,
    TerminalReborrowRestoredCallUse, TerminalReborrowRootHandoff, TerminalReborrowRootHandoffStep,
    TerminalReborrowSharedCohortMember,
};
pub use claims::{ClaimTransfer, CompletionReceipt, EntryClaim, StructuralResultClaimTransfer};
pub use cleanup::{NominalAffineCleanup, StructuralAffineDiscard, TerminalAffineCleanupAction};
pub use placement::{TerminalPlacedViewInput, canonical_placed_view_identity};
pub use suspension::{
    TerminalSuspensionCallPlan, TerminalSuspensionCallSite, TerminalSuspensionCallTarget,
    TerminalSuspensionLiveValue, TerminalSuspensionPlace, TerminalSuspensionStorage,
    TerminalSuspensionValueType, suspension_frontier_commitment,
};
